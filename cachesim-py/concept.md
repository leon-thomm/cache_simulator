### Detailed specifications and assumptions

- a processor executes instructions on every clock tick; if there's memory access it calls the cache and idles until it receives a response
- a cache communicates with other caches through the bus, and the bus is "owned" (locked) by a cache until it completely finished his transactions corresponding to one transition in the state diagram
- any cache can always immediately respond to bus requests. if the cache gets asked to deliver a block which is currently evicting, the block is assumed as invalid
- the system always prefers cache-to-cache transfer if any other cache has the line cached (Illionis style for MESI)
- if multiple caches could deliver a line, there is no additional time needed to select one - the selection algorithm is expected to terminate in the same cycle
- the time for sending a bus transaction (address and transaction code) to other caches takes 2 cycles - just as long as sending only an address
- while the penalty for loading a cache line from memory is 100 cycles, the MESI cache implements Illionis and thus first tries to get the line from another cache, which adds to these 100 cycles if no one has it
- arbitration policy: the processor/cache with lower id is preferred
    - e.g. if P0 and P1 want to write to the same block in the same cycle, P0 will proceed and P1 will have to evict
- the memory is word-addressible not, byte-addressible
- memory is generally only updated on eviction/flushing

### Key insights

1. The bus can only do one thing at a time, it serializes all requests.
2. All memory transfer goes through the bus.

    => When the bus is locked/ownded by some cache $C_j$ processing a processor request that requires communication with other parts (caches, memory), during the time of that communication, the system cannot undergo a state change initiated from another cache $C_k$ that would render $C_j$'s operations invalid, because any such state change would require communication over the bus, which is locked/owned by $C_j$.
    
    => When $C_j$ owns the bus, for any processor request we (the simulator) can determistically fully determine the time that the bus would be busy until all operations necessary to answer the request are finished.

3. Implementation suggests a lot of nested and bidirectional communication between the different parts
    
    => Clock-based message queueing system ? 
    
    => No, use locking! From the perspective of some cache $C_j$, the bus can be:
    - owned: $C_j$ can send bus requests
    - foreign owned: $C_j$ can respond to incoming bus requests - owner is responsible of preventing conflicts
    - free: $C_j$ can try to lock the bus in order to start sending requests

4. There are two types of processor events for the cache:
    - those that can be answered right away (unique transition)
    - those that require communication with other caches and possibly memory. The latter need to wait for the bus to be free (transition ambiguous, need more information), in order to lock it and then start talking to other components
5. This is not the case for bus updates in MESI and Dragon, these can always be handled immediately




New apporach:
- implement everything that happens during a cycle using funcion calls
- implement everything that takes a cycle with messages



```
processor.tick()
    // processor is ticked first
    
    // previous state 

    match state
    WaitingForCache =>
        return
    ReadyToProceed =>
        Ready
    ExecutingOther(n) =>
        if n-1 == 0
            Ready
        else
            ExecutingOther(n-1)
    
    // current state
    
    match state
    Ready =>
        [exec instr]
        on load(a)
            cache.pr_sig(Read(a))
            WaitingforCache
        on store(a)
            cache.pr_sig(Write(a))
            WaitingForCache
        on other(t)
            ExecutingOther(t-1)

processor.proceed()
    styte = ReadyToProceed

cache.tick()
    // cache is ticked second, after processor
    
    match state
    ResolvingRequest(r, t) =>
        if t-1 == 0
            // processor can proceed with next instruction *in the next cycle*
            processor.proceed()
            Idle
        else
            ResolvingRequest(r, t-1)

cache.pr_sig(sig)
    match Protocol
    MESI =>
        match block state
        I =>    // miss
            match sig
            Read(a) =>
                bus.acquire(self)
                WaitingForBus(sig)
            Write(a) =>
                processor.proceed()
                bus.send_tx(self, BusRdX(a))
                set_block_state(a, M)
                Idle
        S =>    // hit
            match sig
            Read(a) =>
                processor.proceed()
                Idle
            Write(a) =>
                processor.proceed()
                bus.send_tx(self, BusRdX(a))
                set_block_state(a, M)
                Idle
        E =>    // hit
            match sig
            Read(a) =>
                processor.proceed()
                Idle
            Write(a) =>
                processor.proceed()
                set_block_state(a, M)
                Idle
        M =>    // hit
            processor.proceed()
            Idle
    Dragon =>
        match block state
        I =>    // miss
            bus.acquire(self)
            WaitingForBus(sig)
        E =>    // hit
            match sig
            Read(a) =>
                processor.proceed()
                Idle
            Write(a) =>
                processor.proceed()
                self.set_block_state(a, M)
                Idle
        Sc =>   // hit
            match sig
            Read(a) =>
                processor.proceed()
                Idle
            Write(a) =>
                bus.acquire(self)
                WaitingForBus(sig)
        Sm =>   // hit
            match sig
            Read(a) =>
                processor.proceed()
                Idle
            Write(a) =>
                bus.acquire(self)
                WaitingForBus(sig)
        M =>    // hit
            processor.proceed()
            Idle

cache.on_bus_ready(msg) -> int
    // returns the number of cycles the bus will be busy

    match Protocol
    MESI =>
        match block state
        I =>
            match msg
            Read(a) =>
                if other cache has cache line -> transfer
                    t = times.ask_other_caches() + times.cache2cache_transfer()
                    set_block_state(a, S)
                else
                    t = times.ask_other_caches() + times.memory_fetch()
                    set_block_state(a, E)
            Write(a) => error
        _ => error
    Dragon =>
        match block state
        I =>
            match msg
            Read(a) =>
                if other cache has cache line -> transfer
                    t = times.ask_other_caches() + times.cache2cache_transfer()
                    bus.send_tx(self, BusRd(a))
                    set_block_state(a, Sc)
                else
                    t = times.ask_other_caches() + times.memory_fetch()
                    bus.send_tx(self, BusRd(a))
                    bus.send('transaction', (self, BusRd(a)))
                    set_block_state(a, E)
            Write(a) =>
                if other cache has line
                    t = times.ask_other_caches() + times.cache2cache_transfer()
                    set_block_state(a, Sm)
                    bus.send_tx(self, BusRd(a))
                    bus.send_tx(self, BusUpd(a))
                else
                    t = times.ask_other_caches() + times.memory_fetch()
                    set_block_state(a, M)
                    bus.send_tx(self, BusRd(a))
        E => error
        Sc =>
            match msg
            Read(a) => error
            Write(a) =>
                if other cache has cache line
                    t = times.ask_other_caches()
                    set_block_state(a, Sm)
                    bus.send_tx(self, BusUpd(a))
                else
                    t = times.ask_other_caches()
                    set_block_state(a, M)
                    bus.send_tx(self, BusUpd(a))
        Sm =>
            match msg
            Read(a) => error
            Write(a) => 
                if other cache has cashe ilne
                    t = times.ask_other_caches()
                    bus.send_tx(self, BusUpd(a))
                else
                    t = times.ask_other_caches()
                    bus.send_tx(self, BusUpd(a))
                    set_block_state(a, M)
        M => error
        
    state = ResolvingRequest(t)

    return t

cache.bus_sig(sig) -> int
    match Protocol
    MESI =>
        match block state
        I => pass
        S =>
            match sig
            BusRd => pass
            BusRdX =>
                set_block_state(a, I)
        E =>
            match sig
            BusRd =>
                // flush
                set_block_state(a, S)
                return times.flush_time()
            BusRdX =>
                // flush
                set_block_state(a, I)
                return flush_time()
        M =>
            BusRd =>
                // flush
                set_block_state(a, S)
                return flush_time()
            BusRdX =>
                // flush
                set_block_state(a, I)
                return flush_time()
    Dragon =>
        match block state
        I => pass
        E =>
            match sig
            BusRd =>
                set_block_state(a, Sc)
            BusUpd => error
        Sc => pass  // update local on BusUpd
        Sm =>
            match sig
            BusRd =>
                // flush
                return times.flush_time()
            BusUpd =>
                set_block_state(a, Sc)
        M =>
            match sig
            BusRd =>
                set_block_state(a, Sm)
            BusUpd => error
    return 0
        

bus.tick(cycle)
    // the bus is ticked last, after processor and after cache
    
    // previous state
    
    match state
    Busy(t) =>
        if t-1 == 0
            Idle
        else
            Busy(t-1)
    
    // current state
    
    match state
    Idle =>
        if fetch_msg('cache req') is Some(cache, payload)
            self.intermediate_busy_time = 0 // can get increased by other caches due to flushing
            t = cache.pr_sig_bus_ready(msg) + self.intermediate_busy_time
            Busy(t)
            // bus now implicitly owned by cache, until t turns 0

bus.send_tx(origin, cache)
    for c in caches except cache
        intermediate_busy_time += c.bus_sig(sig)

times {
    ask_other_caches:       2 * 32/WORD_SIZE
    cache2cache_transfer:   2 * BLOCK_SIZE
    flush:                  2 * BLOCK_SIZE
    memory_fetch:           100
}
```