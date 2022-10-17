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




"""
Detailed specifications and assumptions:

- a processor executes instructions on every clock tick; if there's memory access it calls the cache and idles until it receives a response
- a cache communicates with other caches through the bus, and the bus is "owned" by a cache until it completely finished his transactions corresponding to one transition in the state diagram
- a cache can always responds immediately to bus requests
- the system always prefers cache-to-cache transfer if any other cache has the line cached
- if multiple caches could deliver a line, there is no additional time needed to select one (e.g. they can all send it simultaneously)
- sending a bus transaction to other busses takes exactly 2 cycles (the time for sending the address)
- a cache can always fully answer bus requests, even if it is currently delivering a cache line that is getting evicted while sending (the line is then buffered)
- while the penalty for loading a cache line from memory is 100 cycles, the MESI cache implements Illionis and thus first tries to get the line from another cache, which adds to these 100 cycles if no one has it
- arbitration policy: the processor/cache with lower id is preferred
    - e.g. if P0 and P1 want to write to the same block in the same cycle, P0 will proceed and P1 will have to evict
- the memory is word-addressible not byte-addressible
- memory is only updated on eviction

Key insights:

1. The bus can only do one thing at a time.
2. All memory transfer goes through the bus.
=> When a bus transaction is processed we can already completely infer the whole time it takes to complete it, because the system cannot change in the meantime until the transaction is finished.
3. Implementation suggests a lot of nested and bidirectional communication between the different parts
=> Clock-based message queueing system ?
4. There are two types of processor events for the cache:
    - those that can be answered right away (unique transition)
    - those that require communication with other caches and possibly memory. The latter need to wait for the bus to be free (transition ambiguous, need more information)
5. This is not the case for bus updates in MESI and Dragon, these can always be handled immediately


processor.tick()
    if WaitingForCache
        if peek_msg('cache') is not Proceed:
            return
        else
            Ready
    if ExecutingOther(n):
        set state ExecutingOther(n-1)
    if ExecutingOther(0):
        Ready
    if Ready
        [exec instr]
        on ld instruction
            if cache.is_hit()   // cache hit is 1 cycle
                Ready
        on st instruction
            if cache.is_hit()   // cache hit is 1 cycle
                Ready
        on other instruction:
            ExecutingOther(x)

cache.tick()
    if ResolvingRequest(r, t)
        if t-1 == 0
            processor.send('cache', Proceed)
            Done
        else
            ResolvingRequest(r, t-1)
    else if WaitingForBus
        if fetch_msg('bus_response') is Some(msg)
            self.on_bus_ready()
    else (if Idle)
        if fetch_msg('processor') is Some(msg)
            self.pr_sig(msg)
    
    while fetch_msg('bus') is Some(msg):
        self.bus_sig(msg)

cache.pr_sig()
    Read =>
        on hit:
            [protocol]
            set cache state: ResolvingRequest(Wait(1))
        on miss:
            [protocol] queue bus request
            set cache state: ResolvingRequest(Unknown)
    Write =>
        on hit:
            [protocol]
            set cache state: ResolvingRequest(Wait(1))
        on miss:
            [protocol] queue bus request
            set cache state ResolvingRequest(Unknown)

cache.bus_sig()
    ...

cache.on_bus_ready()
    [protocol]
    
            on hit:
                set cache state: ResolvingRequest(transaction_time)
                update / notify other caches
                return transaction_time
            on miss:
                // load from memory
                set cache state: ResolvingRequest(100)
                return 100

bus.tick()
    if Busy(Unknown):
        pass
    else if Busy(t):
        t-1 == 0 ? Idle : Busy(t-1)
    else if Idle and fetch_msg('caches') is Some(msg):
        msg.cache.send('bus_response', msg.request)
        Busy(Unknown)


PLAN -------------------------------------------------------------------------------------------------
New apporach:
- implement everything that happens during a cycle using funcion calls
- implement everything that takes a cycle with messages




processor.tick()
    // processor is ticked first
    
    // previous state 
    
    if WaitingForCache:
        if peek_msg('cache') is Proceed:
            Ready
        else
            return
    if ExecutingOther(n)
        if n-1 == 0
            Ready
        else
            ExecutingOther(n-1)
    
    // current state
    
    if Ready
        [exec instr]
        on load(a)
            cache.pr_sig(Read(a))
            WaitingforCache
        on store(a)
            cache.pr_sig(Write(a))
            WaitingForCache
        on other(t)
            ExecutingOther(t-1)

cache.tick()
    // cache is ticked second, after processor
    
    if ResolvingRequest(r, t)
        if t-1 == 0
            // cache can proceed with next instruction in the next cycle
            processor.send('cache', Proceed)
            Idle
        else
            ResolvingRequest(r, t-1)

cache.pr_sig(sig)
    MESI =>
        I =>    // miss
            Read(a) =>
                bus.send('caches', sig)
                WaitingForBus
            Write(a) =>
                processor.send('cache', Proceed)
                bus.send_sig(self, BusRdX(a))
                self.set_block_state(a, M)
                Idle
        S =>    // hit
            Read(a) =>
                processor.send('cache', Proceed)
                Idle
            Write(a) =>
                processor.send('cache', Proceed)
                bus.send_sig(self, BusRdX(a))
                self.set_block_state(a, M)
                Idle
        E =>    // hit
            Read(a) =>
                processor.send('cache', Proceed)
                Idle
            Write(a) =>
                processor.send('cache', Proceed)
                self.set_block_state(a, M)
                Idle
        M =>    // hit
            processor.send('cache', Proceed)
            Idle
    Dragon =>
        I =>    // miss
            bus.send('caches', sig)
            WaitingForBus
        E =>    // hit
            Read(a) =>
                processor.send('cache', Proceed)
                Idle
            Write(a) =>
                processor.send('cache', Proceed)
                self.set_block_state(a, M)
                Idle
        Sc =>   // hit
            Read(a) =>
                processor.send('cache', Proceed)
                Idle
            Write(a) =>
                bus.send('caches', Write(a))
                WaitingForBus
        Sm =>   // hit
            Read(a) =>
                processor.send('cache', Proceed)
                Idle
            Write(a) =>
                bus.send('caches', Write(a))
                WaitingForBus
        M =>    // hit
            processor.send('cache', Proceed)
            Idle

cache.pr_sig_bus_ready(msg) -> int
    MESI =>
        I =>
            Read(a) =>
                if other cache has cache line -> transfer
                    t = ask_other_caches_time() + cache2cache_transfer_time()
                    self.set_block_state(a, S)
                else
                    t = ask_other_caches_time() + 100
                    self.set_block_state(a, E)
                    
            Write(a) => error
        S => error
        E => error
        M => error
    DrDragonagon =>
        I =>
            Read(a) =>
                if other cache has cache line -> transfer
                    t = ask_other_caches_time() + cache2cache_transfer_time()
                    bus.send_sig(self, BusRd(a))
                    self.set_block_state(a, Sc)
                else
                    t = ask_other_caches_time() + 100
                    bus.send_sig(self, BusRd(a))
                    self.set_block_state(a, E)
            Write(a) =>
                if other cache has line
                    t = ask_other_caches_time() + cache2cache_transfer_time()
                    self.set_block_state(a, Sm)
                    bus.send_sig(self, BusRd(a))
                    bus.send_sig(self, BusUpd(a))
                else
                    t = ask_other_caches_time() + 100
                    self.set_block_state(a, M)
                    bus.send_sig(self, BusRd(a))
        E => error
        Sc =>
            Read(a) => error
            Write(a) =>
                if other cache has cache line
                    t = ask_other_caches_time()
                    self.set_block_state(a, Sm)
                    bus.send_sig(a, BusUpd(a))
                else
                    t = ask_other_caches_time()
                    self.set_block_state(a, M)
                    bus.send_sig(a, BusUpd(a))
        Sm =>
            Read(a) => error
            Write(a) => 
                if other cache has cashe ilne
                    t = ask_other_caches_time()
                    bus.send_sig(a, BusUpd(a))
                else
                    t = ask_other_caches_time()
                    bus.send_sig(a, BusUpd(a))
                    self.set_block_state(a, M)
        M => error
        
    ResolvingRequest(t)
    return t

cache.bus_sig(sig) -> int
    MESI =>
        I => pass
        S =>
            BusRd => pass
            BusRdX =>
                set.set_block_state(a, I)
        E =>
            BusRd =>
                // flush
                self.set_block_state(a, S)
                return flush_time()
            BusRdX =>
                // flush
                self.set_block_state(a, I)
                return flush_time()
        M =>
            BusRd =>
                // flush
                self.set_block_state(a, S)
                return flush_time()
            BusRdX =>
                // flush
                self.set_block_state(a, I)
                return flush_time()
    Dragon =>
        I => pass
        E =>
            BusRd =>
                self.set_block_state(a, Sc)
            BusUpd => error
        Sc => pass  // update local on BusUpd
        Sm =>
            BusRd =>
                // flush
                return flush_time()
            BusUpd =>
                self.set_block_state(a, Sc)
        M =>
            BusRd =>
                self.set_block_state(a, Sm)
            BusUpd => error
    return 0
        

bus.tick(cycle)
    // the bus is ticked last, after processor and after cache
    
    // previous state
    
    if Busy(t)
        if t-1 == 0
            Idle
        else
            Busy(t-1)
    
    // current state
    
    if Idle
        if fetch_msg('caches') is Some(msg)
            self.intermediate_busy_time = 0
            t = msg.cache.pr_sig_bus_ready(msg) + self.intermediate_busy_time
            Busy(t)

bus.send_sig(origin, cache)
    for c in self.caches \ [cache]
        self.intermediate_busy_time +=
            c.bus_sig(sig)

ask_other_caches_time()
    2 * 32/WORD_SIZE
        
cache2cache_transfer_time()
    2 * BLOCK_SIZE

flush_time()
    2 * BLOCK_SIZE
"""