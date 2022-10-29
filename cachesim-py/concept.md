### Detailed specifications and assumptions

- a processor executes instructions on every clock tick; if there's memory access it calls the cache and idles until it receives a response (*)
- a cache communicates with other caches through the bus, and the bus is "owned" (locked) by a cache until it completely finished his transactions corresponding to one transition in the state diagram
- any cache can always immediately respond to bus requests (*). if the cache gets asked to deliver a block which is pending eviction, the block is assumed to be invalid and cannot be delivered anymore
- the system always prefers cache-to-cache transfer if any other cache has the line cached (Illionis style for MESI)
- if multiple caches could deliver a line, there is no additional time needed to select one - the selection algorithm is expected to terminate in the same cycle
- the time for sending a bus transaction (address and transaction code) to other caches takes 2 cycles - just as long as sending only an address
- while the penalty for loading a cache line from memory is 100 cycles, the MESI cache implements Illionis and thus first tries to get the line from another cache, which adds to these 100 cycles if no one has it
- arbitration policy: the processor/cache with lower id is preferred
    - e.g. if P0 and P1 want to write to the same block in the same cycle, P0 will proceed and P1 will have to evict
- the memory is word-addressible not, byte-addressible
- memory is generally only updated on eviction/flushing
- the lecture slides about MESI show a `Flush` operation on $E\overset{\text{BusRd}}{\longrightarrow}S$, which does not make sense to me and does not seem to be the usual case, see also [wikipedia](https://en.wikipedia.org/wiki/MESI_protocol). I am assuming the state transitions on wikipedia
- however, also notice that the state diagram on wikipedia for the Dragon protocol seems to be wrong as well - I reported it on the talk page
- bus locking must be fair between the caches (request queue)

(*): as stated in the task description

### Key insights

1. The bus can only do one thing at a time, it serializes all requests.
2. All memory transfer goes through the bus.
3. There are two types of *processor events* for the cache:
    - those that can be answered right away (unique transition)
    - those that require communication with other caches and possibly memory. The latter need to wait for the bus to be free (transition ambiguous, need more information), in order to lock it and then start talking to other components
4. This is not the case for *bus updates* in MESI and Dragon, these can always be handled immediately

=> Let $C_0$ be a cache processing a processor request on $a$ in block $b$ requiring commmunication over the bus, and assume $C_0$ is owning the bus now and performing its operations. Let $C_1$ be another cache attempting a state transition on $b$ as well. By owning the bus, $C_0$ ensures the following

* **either** $C_1$ is blocked from its transition (if it requires sending and receiving on bus) until $C_0$ is done and the system never enters an invalid state
* **or** $C_1$'s transition does not require sending and receiving on bus (i.e. only sending, e.g. MESI Invalid PrWrMiss) which might put the system into a temporarily invalid state (e.g. $b$ in Modified in $C_0$ and $C_1$) but this inconsistency will be serially resolved once $C_0$ is done and the bus is freed

=> When $C_0$ owns the bus, for any processor request the simulator can determistically fully determine the time that the bus would be busy until all operations necessary to answer the request are finished.

From the perspective of some cache $C_i$, the bus can be:

* owned: $C_i$ can send bus requests
* foreign owned: $C_i$ can respond to incoming bus requests - owner is responsible of preventing conflicts
* free: $C_i$ can try to lock the bus in order to start sending requests


***


New apporach:
- implement everything that happens during a cycle using funcion calls
- implement everything that takes a cycle with messages



```
processor.tick()
    // processor is ticked first
    
    // update state

    match state
    WaitingForCache =>
        return
    Done =>
        return
    ReadyToProceed =>
        Ready
    ExecutingOther(n) =>
        if n-1 == 0
            Ready
        else
            ExecutingOther(n-1)
    
    // proceed
    
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
        on finished
            Done

processor.proceed()
    state = ReadyToProceed

cache.tick()
    // cache is ticked second, after processor
    
    match state
    ResolvingRequest(t) =>
        if t-1 == 0
            // processor can proceed with next instruction *in the next cycle*
            processor.proceed()
            Idle
        else
            ResolvingRequest(t-1)

cache.access(a)
    // access a *cached* address - updates the LRU state

    b = block_of_addr(a)
    s = set_of_block(b)
    if b not in s:
        error
    // shift b to end to indicate it was recently used
    s.remove(b)
    s.append(b)
    // no replacement necessary

cache.add(a, state)
    // employ LRU replacement policy

    b = block_of_addr(a)
    s = set_of_block(b)
    
    s.remove(b)
    match Protocol
    MESI =>
        match block.state
        Modified =>
            t = times.flush()
    Dragon =>
        match block.state
        Modified, SharedModified =>
            t = times.flush()

    s.append(b, state)
    return t

cache.pr_sig(sig)
    // there are two types of signals:
    // * those which can be handled immediately
    // * those which require communication with other caches and possibly memory over the bus
    // the latter type of signals are handled once we own the bus

    def hit_imm():
        """handle an immediate hit, i.e. without bus communication"""
        access(a)
        match times.cache_hit()
        0 =>
            processor.proceed()
            Idle
        t =>
            ResolvingRequest(t)
    
    def acquire_bus():
        bus.acquire(self)
        WaitingForBus(sig)


    match Protocol
    MESI =>
        match block state
        I =>    // miss
            match sig
            Read(a) =>
                acquire_bus()
            Write(a) =>
                bus.send_tx(self, BusRdX(a))
                transition(a, M)
                // not a hit, but MESI proceeds immediately
                processor.proceed()
                access(a)
                state = Idle
        S =>    // hit
            match sig
            Read(a) =>
                state = hit_imm()
            Write(a) =>
                bus.send_tx(self, BusRdX(a))
                transition(a, M)
                state = hit_imm()
        E =>    // hit
            match sig
            Read(a) =>
                state = hit_imm()
            Write(a) =>
                transition(a, M)
                state = hit_imm()
        M =>    // hit
            state = hit_imm()
    Dragon =>
        match block state
        I =>    // miss
            acquire_bus()
        E =>    // hit
            match sig
            Read(a) =>
                state = hit_imm()
            Write(a) =>
                self.transition(a, M)
                state = hit_imm()
        Sc =>   // hit
            match sig
            Read(a) =>
                state = hit_imm()
            Write(a) =>
                acquire_bus()
        Sm =>   // hit
            match sig
            Read(a) =>
                state = hit_imm()
            Write(a) =>
                acquire_bus()
        M =>    // hit
            state = hit_imm()

cache.on_bus_ready(msg) -> int
    // returns the number of cycles the bus will be busy
    t = 0

    if a is cached:
        access(a)
    else:
        t += add(a, I)

    match Protocol
    MESI =>
        match block state
        I =>
            match msg
            Read(a) =>
                if other cache has cache line -> transfer
                    t += 
                        times.ask_other_caches() + 
                        times.cache2cache_transfer() +
                    bus.send_tx(self, BusRd(a))
                    transition(a, S)
                else
                    t += 
                        times.ask_other_caches() + 
                        times.memory_fetch() +
                    transition(a, E)
            Write(a) => error
        _ => error
    Dragon =>
        match block state
        I =>
            match msg
            Read(a) =>
                if other cache has cache line -> transfer
                    t += 
                        times.ask_other_caches() + 
                        times.cache2cache_transfer()
                    bus.send_tx(self, BusRd(a))
                    transition(a, Sc)
                else
                    t += 
                        times.ask_other_caches() + 
                        times.memory_fetch()
                    bus.send_tx(self, BusRdX(a))
                    transition(a, E)
            Write(a) =>
                if other cache has line
                    t += 
                        times.ask_other_caches() + 
                        times.cache2cache_transfer()
                    transition(a, Sm)
                    bus.send_tx(self, BusRd(a))
                    bus.send_tx(self, BusUpd(a))
                else
                    t += times.ask_other_caches() + times.memory_fetch()
                    transition(a, M)
                    bus.send_tx(self, BusRdX(a))
        E => error
        Sc =>
            match msg
            Read(a) => error
            Write(a) =>
                if other cache has cache line
                    t += times.ask_other_caches()
                    transition(a, Sm)
                    bus.send_tx(self, BusUpd(a))
                else
                    t += times.ask_other_caches()
                    transition(a, M)
                    bus.send_tx(self, BusUpd(a))
        Sm =>
            match msg
            Read(a) => error
            Write(a) => 
                if other cache has cashe ilne
                    t += times.ask_other_caches()
                    bus.send_tx(self, BusUpd(a))
                else
                    t += times.ask_other_caches()
                    bus.send_tx(self, BusUpd(a))
                    transition(a, M)
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
                transition(a, I)
        E =>
            match sig
            BusRd =>
                // writeback
                transition(a, S)
                return times.flush()
            BusRdX =>
                // flush
                transition(a, I)
                return times.flush()
        M =>
            BusRd =>
                // writeback
                transition(a, S)
                return times.flush()
            BusRdX =>
                // flush
                transition(a, I)
                return times.flush()
    Dragon =>
        match block state
        I => pass
        E =>
            match sig
            BusRd =>
                transition(a, Sc)
            BusUpd => error
        Sc => pass  // update local on BusUpd
        Sm =>
            match sig
            BusRd =>
                // writeback
                return times.flush()
            BusUpd =>
                transition(a, Sc)
        M =>
            match sig
            BusRd =>
                transition(a, Sm)
            BusUpd => error
    return 0
        

bus.tick(cycle)
    // the bus is ticked last, after processor and after cache
    
    // update state
    
    match state
    Busy(t) =>
        if t-1 == 0
            Idle
        else
            Busy(t-1)
    
    // proceed
    
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
    ask_other_caches:       2 * 32//WORD_SIZE
    cache2cache_transfer:   2 * BLOCK_SIZE
    flush:                  2 * BLOCK_SIZE
    memory_fetch:           100
    cache_hit:              0
}
```