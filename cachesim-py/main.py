# A MESI and Dragon Cache Simulator

from pickle import PROTO


PROTOCOL = 'MESI'
WORD_SIZE = 4
BLOCK_SIZE = 32
CACHE_SIZE = 4096
CACHE_ASSOC = 2


class ClockedMsgQ:
    def __init__(self, channels):
        self.messages = {
            channel: []
            for channel in channels
        }
    
    def fetch_msg(self, channel):
        return self.messages[channel].pop(0)
    
    def peek_msg(self, channel):
        return self.messages[channel][0]
    
    def send(self, channel, msg):
        self.messages[channel].append(msg)


class Processor:
    def __init__(self, instructions):
        super().__init__(['cache'])

        self.instructions = instructions
        self.cache = Cache(self)
        self.state = ('Ready',)
    
    def tick(self):
        # processor is ticked first

        # previous state

        match self.state:
            case ('WaitingForCache',):
                if self.peek_msg('cache') == 'Proceed':
                    self.fetch_msg('cache')
                    self.state = ('Ready',)
                else:
                    return
            case ('ExecutingOther', n):
                if n == 0:
                    self.state = ('Ready',)
                else:
                    self.state = ('ExecutingOther', n-1)
            case ('Done',):
                return
        
        # new state

        if self.state == ('Ready',):
            if inst := self.instructions.pop(0):
                # execute next instruction
                match inst:
                    case ('Read', addr):
                        self.cache.pr_sig('PrRead', addr)
                    case ('Write', addr):
                        self.cache.pr_sig('PrWrite', addr)
                    case ('Other', time):
                        self.state = ('ExecutingOther', time-1)
            else:
                self.state = ('Done',)

class Cache:
    def __init__(self, proc) -> None:
        super().__init__(['proc', 'bus'])

        self.proc = proc
        self.bus = None
        self.data = {
            # addr: [(tag, last_used, state), ...]
            idx: [[None, 0, None]] * CACHE_ASSOC
            for idx in range(CACHE_SIZE // CACHE_ASSOC)
        }
        self.state = ('Idle',)
    
    def state_of(self, addr):
        num_indices = CACHE_SIZE // CACHE_ASSOC
        index = addr % num_indices
        tag = addr // num_indices
        for block in self.data[index]:
            if block[0] == tag:
                return block[2]
        return 'I'
    
    def set_state_of(self, addr, state):
        num_indices = CACHE_SIZE // CACHE_ASSOC
        index = addr % num_indices
        tag = addr // num_indices
        for block in self.data[index]:
            if block[0] == tag:
                block[2] = state
                break
        else:
            raise Exception('Block not found')
    
    def access(self, addr):
        # perform LRU on self.data

        num_indices = CACHE_SIZE // CACHE_ASSOC
        index = addr % num_indices
        tag = addr // num_indices

        cache_set = self.data[index]
        set_full = all([b[2] != 'I' for b in cache_set])
        addr_in_set = any([b[0] == tag for b in cache_set])

        if set_full and not addr_in_set:
            # evict block
            lru = min(cache_set, key=lambda b: b[1])
            self.evict(lru[0]*num_indices + index)
            lru[0] = tag
            lru[1] = max([b[1] for b in cache_set]) + 1
            lru[2] = 'I'
    
    def evict(self, addr):
        if PROTOCOL == 'MESI':
            match self.state_of(addr):
                case 'M':
                    self.bus.add_busy_time(2 * BLOCK_SIZE)
                case _:
                    pass
        elif PROTOCOL == 'Dragon':
            match self.state_of(addr):
                case 'M' | 'Sm':
                    self.bus.add_busy_time(2 * BLOCK_SIZE)
                case _:
                    pass
    
    def tick(self):
        # cache is ticked second, after processor
        match self.state:
            case ('ResolvingRequest', r, t):
                if t == 0:
                    self.proc.send('cache', 'Proceed')
                    self.state = ('Idle',)
                else:
                    self.state = ('ResolvingRequest', r, t-1)
    
    def pr_sig(self, event, addr):

        def pr_send(msg):
            self.proc.send('cache', msg)
        
        def bus_send(msg):
            self.bus.send('caches', (self, msg))
        
        def bus_send_sig(msg):
            self.bus.send_sig(self, msg)

        def transition(new_block_state):
            self.access(addr)
            self.set_state_of(addr, new_block_state)
        
        def idle():
            self.state = ('Idle',)
        def wait_for_bus():
            self.state = ('WaitingForBus',)

        if PROTOCOL == 'MESI':
            match self.state_of(addr):
                case 'I':
                    match event:
                        case 'PrRead':
                            bus_send(('PrRead', addr))
                            wait_for_bus()
                        case 'PrWrite':
                            pr_send('Proceed')
                            bus_send_sig(('BusRdX', addr))
                            transition('M')
                            idle()
                case 'S':
                    match event:
                        case 'PrRead':
                            pr_send('Proceed')
                            transition('S')
                            idle()
                        case 'PrWrite':
                            pr_send('Proceed')
                            bus_send_sig(('BusRdX', addr))
                            transition('M')
                            idle()
                case 'E':
                    match event:
                        case 'PrRead':
                            pr_send('Proceed')
                            transition('E')
                            idle()
                        case 'PrWrite':
                            pr_send('Proceed')
                            transition('M')
                            idle()
                case 'M':
                    match event:
                        case 'PrRead':
                            pr_send('Proceed')
                            transition('M')
                            idle()
                        case 'PrWrite':
                            pr_send('Proceed')
                            transition('M')
                            idle()

        elif PROTOCOL == 'Dragon':
            match self.state_of(addr):
                case 'I':
                    bus_send((event, addr))
                    wait_for_bus()
                case 'E':
                    match event:
                        case 'PrRead':
                            pr_send('Proceed')
                            transition('E')
                            idle()
                        case 'PrWrite':
                            pr_send('Proceed')
                            transition('M')
                            idle()
                case 'Sc':
                    match event:
                        case 'PrRead':
                            pr_send('Proceed')
                            transition('Sc')
                            idle()
                        case 'PrWrite':
                            bus_send(('PrWrite', addr))
                            wait_for_bus()
                case 'Sm':
                    match event:
                        case 'PrRead':
                            pr_send('Proceed')
                            transition('Sm')
                            idle()
                        case 'PrWrite':
                            bus_send(('PrWrite', addr))
                            wait_for_bus()
                case 'M':
                    pr_send('Proceed')
                    transition('M')
                    idle()


    def pr_sig_bus_ready(self, sig):
        event, addr = sig

        def bus_send_sig(msg):
            self.bus.send_sig(self, msg)

        def transition(new_block_state):
            self.access(addr)
            self.set_state_of(addr, new_block_state)
        
        def ask_other_caches_time():
            return 2 * 32//WORD_SIZE
        
        def c2c_transfer_time():
            return 2 * BLOCK_SIZE
        
        def error():
            raise Exception('Invalid state')

        others = [
            c
            for c in self.bus.get_caches(exclude=self)
            if c.state_of(addr) != 'I'
        ]

        others_have_block = len(others) > 0

        if PROTOCOL == 'MESI':
            match self.state_of(addr):
                case 'I':
                    match event:
                        case 'PrRead':
                            if others_have_block:
                                t = ask_other_caches_time() + c2c_transfer_time()
                                transition('S')
                            else:
                                t = ask_other_caches_time() + 100
                                transition('E')
                        case 'PrWrite':
                            error()
                case 'S':
                    error()
                case 'E':
                    error()
                case 'M':
                    error()
        
        elif PROTOCOL == 'Dragon':
            match self.state_of(addr):
                case 'I':
                    match event:
                        case 'PrRead':
                            if others_have_block:
                                t = ask_other_caches_time() + c2c_transfer_time()
                                bus_send_sig(('BusRd', addr))
                                transition('Sc')
                            else:
                                t = ask_other_caches_time() + 100
                                bus_send_sig(('BusRd', addr))
                                transition('E')
                        case 'PrWrite':
                            if others_have_block:
                                t = ask_other_caches_time() + c2c_transfer_time()
                                bus_send_sig(('BusRd', addr))
                                bus_send_sig(('BusUpd', addr))
                                transition('Sm')
                            else:
                                t = ask_other_caches_time() + 100
                                bus_send_sig(('BusRd', addr))
                                transition('M')
                case 'E':
                    error()
                case 'Sc':
                    match event:
                        case 'PrRead':
                            error()
                        case 'PrWrite':
                            if others_have_block:
                                t = ask_other_caches_time()
                                bus_send_sig(('BusUpd', addr))
                                transition('Sm')
                            else:
                                t = ask_other_caches_time()
                                bus_send_sig(('BusUpd', addr))
                                transition('M')
                case 'Sm':
                    match event:
                        case 'PrRead':
                            error()
                        case 'PrWrite':
                            if others_have_block:
                                t = ask_other_caches_time()
                                bus_send_sig(('BusUpd', addr))
                                transition('Sm')
                            else:
                                t = ask_other_caches_time()
                                bus_send_sig(('BusUpd', addr))
                                transition('M')
                case 'M':
                    error()
        
        self.state = ('ResolvingRequest', t)
        return t
    
    def bus_sig(self, sig):
        event, addr = sig

        def transition(new_block_state):
            # we do NOT access() here
            self.set_state_of(addr, new_block_state)
        
        def flush_time():
            return 2 * BLOCK_SIZE
        
        def error():
            raise Exception('Invalid state')

        
        if PROTOCOL == 'MESI':
            match self.state_of(addr):
                case 'I':
                    pass
                case 'S':
                    match event:
                        case 'BusRd':
                            pass
                        case 'BusRdX':
                            transition('I')
                case 'E':
                    match event:
                        case 'BusRd':
                            transition('S')
                            return flush_time()
                        case 'BusRdX':
                            transition('I')
                            return flush_time()
                case 'M':
                    match event:
                        case 'BusRd':
                            transition('S')
                            return flush_time()
                        case 'BusRdX':
                            transition('I')
                            return flush_time()
        
        elif PROTOCOL == 'Dragon':
            match self.state_of(addr):
                case 'I':
                    pass
                case 'E':
                    match event:
                        case 'BusRd':
                            transition('Sc')
                        case 'BusUpd':
                            error()
                case 'Sc':
                    pass
                case 'Sm':
                    match event:
                        case 'BusRd':
                            return flush_time()
                        case 'BusUpd':
                            transition('Sc')
                case 'M':
                    match event:
                        case 'BusRd':
                            transition('Sm')
                        case 'BusUpd':
                            error()
        
        return 0


class Bus:
    def __init__(self, caches):
        super().__init__(['caches'])
        
        self.caches = caches
        self.state = ('Idle',)
        self._interm_busy_time = 0
    
    def get_caches(self, exclude=None):
        return [cache for cache in self.caches if cache != exclude]
    
    def tick(self):
        # the bus is ticked last, after processor and after cache

        # previous state

        match self.state:
            case ('Busy', t):
                if t-1 == 0:
                    self.state = ('Idle',)
                else:
                    self.state = ('Busy', t-1)
        
        # new state

        match self.state:
            case ('Idle',):
                if msg := self.fetch_msg('caches'):
                    self._interm_busy_time = 0
                    cache, m = msg
                    t = cache.pr_sig_bus_ready(m) + self._interm_busy_time
                    self.state = ('Busy', t)
    
    def send_sig(self, origin_cache, sig):
        for cache in self.get_caches(exclude=origin_cache):
            self.add_busy_time(
                cache.bus_signal(sig)
            )
    
    def add_busy_time(self, t):
        self._interm_busy_time += t
