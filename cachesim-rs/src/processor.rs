use crate::Address;
use crate::cache::Cache;


enum Instruction {
    Load(Address),
    Store(Address),
    Other(i32),
}

struct Processor<C>
where C: Cache {
    cache: C,
    instructions: Vec<Instruction>,

    // cycle counters
    cycle_count: i32,
    idle_cycle_count: i32,

    // instruction counters
    load_count: i32,
    store_count: i32,

    state: ProcessorState,
}

impl Processor<C>
where C: Cache {
    fn new(cache: C) -> Self {
        Processor {
            cache,
            instructions: vec![],
            cycle_count: 0,
            idle_cycle_count: 0,
            load_count: 0,
            store_count: 0,
            state: ProcessorState::Ready,
        }
    }

    // execute the next instruction
    fn tick(&mut self) {

        // the cache does not need to send anything to the processor, except
        // for the answers to the processor's queries which are wrapped
        // in the state WaitingForCache...(CacheResponse)
        // so we tick the cache first

        self.cache.tick();

        self.state = match &self.state {
            &ProcessorState::Done => ProcessorState::Done,
            &ProcessorState::Ready => self.exec_instr(),
            &ProcessorState::WaitingForCacheLoad => self.tick_load(),
            &ProcessorState::WaitingForCacheStore => self.tick_store(),
            &ProcessorState::ExecutingOtherInstruction(t) => self.tick_other(t),
        }
    }

    fn exec_instr(&mut self) -> ProcessorState {
        self.cycle_count += 1;
        match self.instructions.pop() {
            Some(Instruction::Load(addr)) => {
                self.load_count += 1;
                self.cache.processor_request(ProcessorSignal::Read(addr));
                ProcessorState::WaitingForCacheLoad
            }
            Some(Instruction::Store(addr)) => {
                self.store_count += 1;
                self.cache.processor_request(ProcessorSignal::Write(addr));
                ProcessorState::WaitingForCacheStore
            }
            Some(Instruction::Other(t)) => {
                ProcessorState::ExecutingOtherInstruction(t - 1)
            },
            None => ProcessorState::Done,
        }
    }

    fn tick_load(&mut self) -> ProcessorState {
        self.cycle_count += 1;
        self.idle_cycle_count += 1;
        match self.cache.get_proc_response() {
            &Some(_) => ProcessorState::Ready,
            &None => ProcessorState::WaitingForCacheLoad,
        }
    }

    fn tick_store(&mut self) -> ProcessorState {
        self.cycle_count += 1;
        self.idle_cycle_count += 1;
        match &(self.cache.proc_response).get() {
            &Some(_) => ProcessorState::Ready,
            &None => ProcessorState::WaitingForCacheStore
        }
    }

    fn tick_other(&mut self, t: i32) -> ProcessorState {
        self.cycle_count += 1;
        match t {
            1 => ProcessorState::Ready,
            _ => ProcessorState::ExecutingOtherInstruction(t - 1),
        }
    }
}

enum ProcessorState {
    // the integer determines the time remaining
    ExecutingOtherInstruction(i32),
    WaitingForCacheLoad,
    WaitingForCacheStore,
    Ready,
    Done,
}

enum ProcessorSignal {
    Read(Address),
    Write(Address),
}

