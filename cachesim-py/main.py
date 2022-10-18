# A MESI and Dragon Cache Simulator

from os import access
from pickle import PROTO


PROTOCOL = 'MESI'
WORD_SIZE = 4       # bytes
ADDRESS_SIZE = 4    # bytes
MEM_LAT = 100       # cpu cycles
CACHE_HIT_LAT = 1   # cpu cycles
BUS_WORD_TF_LAT = 2 # cpu cycles
BLOCK_SIZE = 32     # bytes
CACHE_SIZE = 4096   # bytes
CACHE_ASSOC = 2     # blocks

class Times:
    @staticmethod
    def ask_other_caches():
        # assuming immediate response through wired OR
        return BUS_WORD_TF_LAT * ADDRESS_SIZE//WORD_SIZE
    
    @staticmethod
    def cache_to_cache_transf():
        return BUS_WORD_TF_LAT * BLOCK_SIZE//WORD_SIZE
    
    @staticmethod
    def flush():
        return MEM_LAT
    
    @staticmethod
    def mem_fetch():
        return MEM_LAT
    
    @staticmethod
    def cache_hit():
        return CACHE_HIT_LAT


class Processor:
    def __init__(self, instructions):
        super().__init__()

        self.instructions = instructions
        self.cache = Cache(self)
        self.state = ('Ready',)
    
    def tick(self):
        # processor is ticked first

        # update state

        match self.state:
            case ('WaitingForCache',):
                return
            case ('Done',):
                return
            case ('ReadyToProceed',):
                self.state = ('Ready',)
            case ('ExecutingOther', n):
                if n == 0:
                    self.state = ('Ready',)
                else:
                    self.state = ('ExecutingOther', n-1)
        
        # proceed

        match self.state:
            case ('Ready',):
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
    
    def proceed(self):
        self.state = ('ReadyToProceed',)

class Cache:
    def __init__(self, proc) -> None:
        super().__init__()

        self.proc = proc
        self.bus = None
        self.data = {
            # addr: [(tag, state), ...]
            # last block is most recently used
            idx: [(None, None)] * CACHE_ASSOC
            for idx in range((CACHE_SIZE//WORD_SIZE) // CACHE_ASSOC)
        }
        self.state = ('Idle',)
    
    def dcache_pos(self, addr):
        """returns the index and tag of the cache block containing the address"""
        # notice memory is word-addressible, not byte addressible
        num_indices = (CACHE_SIZE//WORD_SIZE) // CACHE_ASSOC
        index = addr % num_indices
        tag = addr // num_indices
        return index, tag


    def state_of(self, addr):
        """returns the state of the block containing the address"""
        index, tag = self.dcache_pos(addr)
        for block in self.data[index]:
            if block[0] == tag:
                return block[1]
        return 'I'
    
    def set_state_of(self, addr, state):
        """sets the state of the block containing the address"""
        index, tag = self.dcache_pos(addr)
        for block in self.data[index]:
            if block[0] == tag:
                block[1] = state
                break
        else:
            raise Exception('Block not found')
    
    def access_cached(self, addr):
        """maintains the LRU policy"""

        index, tag = self.dcache_pos(addr)

        cache_set = self.data[index]
        addr_in_set = any([b[0] == tag for b in cache_set])

        if addr_in_set:
            # update MRU
            s = self.state_of(addr)
            cache_set.remove((tag, s))
            cache_set.append((tag, s))
        else:
            raise Exception('Address not cached')
    
    def add_block(self, addr, state) -> int:
        """caches a currently uncached and, if necessary, evicts and returns bus busy time for eviction"""

        index, tag = self.dcache_pos(addr)
        cache_set = self.data[index]
        if any([b[0] == tag for b in cache_set]):
            raise Exception('Address already cached')
        set_is_full = all([b[1] != 'I' for b in cache_set])
        s = self.state_of(addr)

        t = 0
        if set_is_full:
            # evict LRU
            match PROTOCOL:
                case 'MESI':
                    match s:
                        case 'M':
                            t += Times.flush()
                case 'Dragon':
                    match s:
                        case 'M' | 'Sm':
                            t += Times.flush()
        
        cache_set.append((tag, state))
        return t

    def tick(self):
        # cache is ticked second, after processor
        match self.state:
            case ('ResolvingRequest', r, t):
                if t-1 == 0:
                    self.proc.proceed()
                    self.state = ('Idle',)
                else:
                    self.state = ('ResolvingRequest', r, t-1)
    
    def pr_sig(self, event, addr):

        s = self.state_of(addr)

        # some helper functions to simplify syntax below

        # shorthands
        def proc_proceed():
            self.proc.proceed()
        
        def bus_send_tx(msg):
            self.bus.send_sig(self, msg)
        
        def transition(new_block_state):
            self.set_state_of(addr, new_block_state)
        
        # state transitions
        def idle():
            self.state = ('Idle',)
        
        def res_req(t):
            self.state = ('ResolvingRequest', event, t)
        
        def wait_for_bus():
            self.state = ('WaitingForBus', (event, addr))
        
        # aggregators
        def acquire_bus():
            self.bus.acquire(self)
            wait_for_bus()
        
        def hit_imm():
            """handle an immediate hit, i.e. without bus communication"""
            self.access(addr)
            match Times.cache_hit():
                case 0:
                    proc_proceed()
                    idle()
                case t:
                    res_req(t)

        # state machine
        if PROTOCOL == 'MESI':
            match s:
                case 'I':
                    match event:
                        case 'PrRead':
                            acquire_bus()
                        case 'PrWrite':
                            bus_send_tx(('BusRdX', addr))
                            transition('M')
                            # not a hit, but MESI proceeds immediately
                            proc_proceed()
                            idle():
                case 'S':
                    match event:
                        case 'PrRead':
                            hit_imm()
                        case 'PrWrite':
                            bus_send_tx(('BusRdX', addr))
                            transition('M')
                            hit_imm()
                case 'E':
                    match event:
                        case 'PrRead':
                            hit_imm()
                        case 'PrWrite':
                            transition('M')
                            hit_imm()
                case 'M':
                    hit_imm()

        elif PROTOCOL == 'Dragon':
            match s:
                case 'I':
                    acquire_bus()
                case 'E':
                    match event:
                        case 'PrRead':
                            hit_imm()
                        case 'PrWrite':
                            transition('M')
                            hit_imm()
                case 'Sc':
                    match event:
                        case 'PrRead':
                            hit_imm()
                        case 'PrWrite':
                            acquire_bus()
                case 'Sm':
                    match event:
                        case 'PrRead':
                            hit_imm()
                        case 'PrWrite':
                            acquire_bus()
                case 'M':
                    hit_imm()


    def pr_sig_bus_ready(self, sig) -> int:
        event, addr = sig
        s = self.state_of(addr)
        t = 0

        if s != 'I':
            self.access_cached(addr)
        else:
            t += self.add_block(addr)

        # shorthands
        def bus_send_tx(msg):
            self.bus.send_sig(self, msg)

        def transition(new_block_state):
            self.set_state_of(addr, new_block_state)
        
        def error():
            raise Exception('Invalid state')

        # list of other caches which have the block cached
        others = [
            c
            for c in self.bus.get_caches(exclude=self)
            if c.state_of(addr) != 'I'
        ]

        others_have_block = len(others) > 0

        # state machine
        if PROTOCOL == 'MESI':
            match s:
                case 'I':
                    match event:
                        case 'PrRead':
                            if others_have_block:
                                t += \
                                    Times.ask_other_caches() + \
                                    Times.cache_to_cache_transf()
                                transition('S')
                            else:
                                t += \
                                    Times.ask_other_caches_time() + \
                                    Times.mem_fetch()
                                transition('E')
                        case _:
                            error()
                case _:
                    error()
        
        elif PROTOCOL == 'Dragon':
            match s:
                case 'I':
                    match event:
                        case 'PrRead':
                            if others_have_block:
                                t += \
                                    Times.ask_other_caches() + \
                                    Times.cache_to_cache_transf()
                                bus_send_tx(('BusRd', addr))
                                transition('Sc')
                            else:
                                t += \
                                    Times.ask_other_caches() + \
                                    Times.mem_fetch()
                                bus_send_tx(('BusRdX', addr))
                                transition('E')
                        case 'PrWrite':
                            if others_have_block:
                                t += \
                                    Times.ask_other_caches_time() + \
                                    Times.cache_to_cache_transf()
                                bus_send_tx(('BusRd', addr))
                                bus_send_tx(('BusUpd', addr))
                                transition('Sm')
                            else:
                                t += \
                                    Times.ask_other_caches_time() + \
                                    Times.mem_fetch()
                                bus_send_tx(('BusRdX', addr))
                                transition('M')
                case 'E':
                    error()
                case 'Sc':
                    match event:
                        case 'PrRead':
                            error()
                        case 'PrWrite':
                            if others_have_block:
                                t += Times.ask_other_caches_time()
                                bus_send_tx(('BusUpd', addr))
                                transition('Sm')
                            else:
                                t += Times.ask_other_caches_time()
                                bus_send_tx(('BusUpd', addr))
                                transition('M')
                case 'Sm':
                    match event:
                        case 'PrRead':
                            error()
                        case 'PrWrite':
                            if others_have_block:
                                t += Times.ask_other_caches_time()
                                bus_send_tx(('BusUpd', addr))
                            else:
                                t += Times.ask_other_caches_time()
                                bus_send_tx(('BusUpd', addr))
                                transition('M')
                case 'M':
                    error()
        
        self.state = ('ResolvingRequest', t)

        return t
    
    def bus_sig(self, sig) -> int:
        event, addr = sig

        # shorthands
        def transition(new_block_state):
            self.set_state_of(addr, new_block_state)
        
        def error():
            raise Exception('Invalid state')

        
        if PROTOCOL == 'MESI':
            match self.state_of(addr):
                case 'I': pass
                case 'S':
                    match event:
                        case 'BusRd':   pass
                        case 'BusRdX':  transition('I')
                case 'E':
                    match event:
                        case 'BusRd':   transition('S'); return Times.flush()
                        case 'BusRdX':  transition('I'); return Times.flush()
                case 'M':
                    match event:
                        case 'BusRd':   transition('S'); return Times.flush()
                        case 'BusRdX':  transition('I'); return Times.flush()
        
        elif PROTOCOL == 'Dragon':
            match self.state_of(addr):
                case 'I': pass
                case 'E':
                    match event:
                        case 'BusRd':   transition('Sc')
                        case 'BusUpd':  error()
                case 'Sc': pass
                case 'Sm':
                    match event:
                        case 'BusRd':   return Times.flush()
                        case 'BusUpd':  transition('Sc')
                case 'M':
                    match event:
                        case 'BusRd':   transition('Sm')
                        case 'BusUpd':  error()
        
        return 0


class Bus:
    def __init__(self, caches):
        super().__init__()
        
        self.caches = caches
        self.state = ('Idle',)
        self._interm_busy_time = 0
        self._cache_requests_queue = []
    
    def get_caches(self, exclude=None):
        return [cache for cache in self.caches if cache != exclude]
    
    def tick(self):
        # the bus is ticked last, after processor and after cache

        # update state

        match self.state:
            case ('Busy', t):
                if t-1 == 0:
                    self.state = ('Idle',)
                else:
                    self.state = ('Busy', t-1)
        
        # proceed

        match self.state:
            case ('Idle',):
                if c := self._cache_requests_queue.pop(0):
                    self._interm_busy_time = 0
                    t = t.pr_sig_bus_ready() + self._interm_busy_time
                    self.state = ('Busy', t)
    
    def acquire(self, cache):
        self._cache_requests_queue.append(cache)
    
    def send_sig(self, origin_cache, sig):
        for cache in self.get_caches(exclude=origin_cache):
            self.intermediate_busy_time += cache.bus_signal(sig)
