mod delayed_q;

use std::collections::VecDeque;
use crate::delayed_q::*;

type DelQMsgSender = DelQSender<Msg>;

/*
    A Simulator for MESI (Illinois) and Dragon 4-state update-based cache coherence protocols.

Assumptions:

1. Memory address is 32-bit.
2. Each memory reference accesses 32-bit (4-bytes) of data. That is word size is 4-bytes.
3. We only model the data cache.
4. Each processor has its own L1 data cache.
5. L1 data cache uses write-back, write-allocate policy and LRU replacement policy.
6. L1 data caches are kept coherent using one of the implemented cache coherence protocols.
7. Initially, all the caches are empty.
8. The bus uses first come first serve arbitration policy when multiple processor
   attempt bus transactions simultaneously. Ties are broken arbitrarily.
9. The L1 data caches are backed up by main memory --- there is no L2 data cache.
10. L1 cache hit is 1 cycle. Fetching a block from memory to cache takes additional 100
    cycles. Sending a word from one cache to another (e.g., BusUpdate) takes only 2 cycles.
    However, sending a cache block with N words (each word is 4 bytes) to another cache
    takes 2N cycle. Assume that evicting a dirty cache block to memory when it gets replaced
    is 100 cycles.
11. There may be additional assumptions.

Also assume that the caches are blocking. That is, if there is a cache miss, the cache
cannot process further requests from the core and the core is completely halted (does not
process any instructions). However, the snooping transactions from the bus still need to
be processed in the cache.
In each cycle, each core can execute at most one memory reference. As per our
assumptions, you do not need to model L1 instruction cache. So the instruction address
trace is not included. But the core cycle counter still has to be incremented with the cycle
value for other instructions in between two load-store instructions.

The program should take an input file name and cache configurations as arguments.
The command line should be

`coherence <protocol> <input_file> <cache_size> <associativity> <block_size>`

where coherence is the executable file name and input parameters are
- <protocol>: either MESI or Dragon
- <input_file>: input benchmark name (e.g., bodytrack)
- <cache_size>: cache size in bytes
- <associativity>: associativity of the cache
- <block_size>: block size in bytes

Assume default parameters as 32-bit word size, 32-byte block size, and 4KB 2-way
set associative cache per processor.

Your program should generate the following output:
- Overall Execution Cycle (different core will complete at different cycles;
  report the maximum value across all cores) for the entire trace as well as
  execution cycle per core
- Number of compute cycles per core. These are the total number of cycles
  spent processing other instructions between load/store instructions
- Number of load/store instructions per core
- Number of idle cycles (these are cycles where the core is waiting for the
  request to the cache to be completed) per core
- Data cache miss rate for each core
- Amount of Data traffic in bytes on the bus (this is due to bus read, bus read
  exclusive, bus write-back, and bus update transactions). Only include the
  traffic for data and not for address. Thus invalidation requests do not
  contribute to the data traffic.
- Number of invalidations or updates on the bus
- Distribution of accesses to private data versus shared data (for example,
  access to modified state is private, while access to shared state is shared data)

 */

// system specs

struct SystemSpec {
    protocol: String,
    word_size: i32,
    address_size: i32,
    mem_lat: i32,
    cache_hit_lat: i32,
    bus_word_tf_lat: i32,
    block_size: i32,
    cache_size: i32,
    cache_assoc: i32,
}

impl SystemSpec {
    fn new() -> Self {
        SystemSpec {
            protocol: "MESI".into(),
            word_size: 4,       // bytes
            address_size: 4,    // bytes
            mem_lat: 100,       // cpu cycles
            cache_hit_lat: 1,   // cpu cycles
            bus_word_tf_lat: 2, // cpu cycles
            block_size: 32,     // bytes
            cache_size: 4096,   // bytes
            cache_assoc: 2,     // blocks
        }
    }
    // timing
    fn t_cache_to_cache_msg(&self) -> i32 {
        // assuming immediate response through wired OR
        self.bus_word_tf_lat * self.address_size / self.word_size
    }
    fn t_cache_to_cache_transfer(&self) -> i32 {
        self.bus_word_tf_lat * self.block_size / self.word_size
    }
    fn t_flush(&self) -> i32 {
        self.mem_lat
    }
    fn t_mem_fetch(&self) -> i32 {
        self.mem_lat
    }
    fn t_cache_hit(&self) -> i32 {
        self.cache_hit_lat
    }
}

// addresses and blocks

#[derive(Clone, Debug)]
struct Addr(i32);

impl Addr {
    fn pos(&self, specs: &SystemSpec) -> (i32, i32) {
        // returns the index and tag of the address under given system specs
        let num_indices = specs.cache_size / (specs.block_size * specs.cache_assoc);
        let index = self.0 % num_indices;
        let tag = self.0 / num_indices;
        (index, tag)
    }
}

// messages

enum Msg {
    ToProc(i32, ProcMsg),
    ToCache(i32, CacheMsg),
    ProcToCache(i32, CacheMsg),
    CacheToProc(i32, ProcMsg),
    CacheToCache(i32, CacheMsg),
    ToBus(BusMsg),
    CacheToBus(i32, BusMsg),
    BusToCache(i32, CacheMsg),
    BusToBus(BusMsg),
    CacheToSim(i32, SimMsg),
    SimToCache(i32, CacheMsg),

    // TickProc(i32),
    // TickCache(i32),
    // TickBus,
}

trait MsgHandler<MsgT> {
    fn handle_msg(&mut self, msg: MsgT);
}

// instructions

#[derive(Clone)]
enum Instr {
    Read(Addr),
    Write(Addr),
    Other(i32),
}

type Instructions = VecDeque<Instr>;

// processors

#[derive(Clone)]
enum ProcMsg {
    Tick,
    PostTick,
    RequestResolved,
}

#[derive(Clone, PartialEq, Debug)]
enum ProcState {
    Idle,
    WaitingForCache,
    ExecutingOther(i32),
    Done,

    RequestResolved,
}

struct Processor<'a> {
    id: i32,
    state: ProcState,
    instructions: Instructions,
    tx: DelQMsgSender,
    specs: &'a SystemSpec,
    cache_id: i32,
}

impl<'a> Processor<'a> {
    fn new(id: i32, cache_id: i32, instructions: Instructions, tx: DelQMsgSender, specs: &'a SystemSpec) -> Self {
        Processor {
            id,
            state: ProcState::Idle,
            instructions,
            tx,
            specs,
            cache_id,
        }
    }
    fn send_cache(&self, msg: CacheMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::ProcToCache(self.cache_id, msg),
        }).unwrap();
    }
}

impl MsgHandler<ProcMsg> for Processor<'_> {
    fn handle_msg(&mut self, msg: ProcMsg) {

        let proceed = |state: &mut ProcState| {
            if self.instructions.len() == 0 {
                *state = ProcState::Done;
            } else {
                *state = ProcState::Idle;
            }
        };

        match self.state {
            ProcState::Idle => {
                self.state = match self.instructions.pop_front().unwrap() {
                    Instr::Read(addr) => {
                        self.send_cache(CacheMsg::PrSig(PrReq::Read(addr)), 0);
                        ProcState::WaitingForCache
                    }
                    Instr::Write(addr) => {
                        self.send_cache(CacheMsg::PrSig(PrReq::Write(addr)), 0);
                        ProcState::WaitingForCache
                    }
                    Instr::Other(time) => {
                        ProcState::ExecutingOther(time - 1)
                    }
                }
            },
            ProcState::WaitingForCache => {
                match msg {
                    ProcMsg::RequestResolved =>
                        self.state = ProcState::RequestResolved,
                    ProcMsg::Tick |
                    ProcMsg::PostTick => (),
                }
            },
            ProcState::RequestResolved => {
                match msg {
                    ProcMsg::Tick => (),
                    ProcMsg::PostTick => proceed(&mut self.state),
                    ProcMsg::RequestResolved => panic!("Processor in invalid state"),
                }
            },
            ProcState::ExecutingOther(time) => {
                match msg {
                    ProcMsg::Tick => {
                        self.state = ProcState::ExecutingOther(time - 1);
                    },
                    ProcMsg::PostTick => {
                        if time == 0 { proceed(&mut self.state); }
                    },
                    ProcMsg::RequestResolved => panic!("Processor in invalid state"),
                }
            },
            ProcState::Done => (),
        }
    }
}

// caches

#[derive(Clone, Debug)]
enum PrReq {
    Read(Addr),
    Write(Addr),
}

#[derive(Clone)]
enum CacheMsg {
    Tick,
    PostTick,
    PrSig(PrReq),
    BusSig(BusSignal),
    BusLocked,
    BusReqResolved,
    PrReqResolved,
    CachesChecked(bool),
}

#[derive(Clone, Debug)]
enum CacheState {
    Idle,

    WaitingForBus_PrReq(PrReq),
    ResolvingPrReq(PrReq, Option<bool>),
    ResolvingPrReq_ProceedNext,
    AskingCaches(PrReq),
    AskingCaches_ProceedNext(PrReq, bool),

    WaitingForBus_BusSig(BusSignal),
    ResolvingBusReq(BusSignal),
    ResolvingBusReq_ProceedNext,
}

struct CacheBlock {
    tag: i32,
    state: BlockState,
}

#[derive(Clone, Debug)]
enum BlockState {
    Invalid,
    Shared,
    Exclusive,
    Modified,
}

struct CacheSet<'a> {
    blocks: Vec<CacheBlock>,
    specs: &'a SystemSpec,
}

impl<'a> CacheSet<'a> {
    fn new(specs: &'a SystemSpec) -> CacheSet {
        CacheSet {
            blocks: vec![],
            specs,
        }
    }
}

struct Cache<'a> {
    id: i32,
    state: CacheState,
    tx: DelQMsgSender,
    specs: &'a SystemSpec,
    proc_id: i32,
    bus_signals_queue: VecDeque<BusSignal>,
    pr_sig_buffer: Option<PrReq>,
    data: Vec<CacheSet<'a>>
}

impl<'a> Cache<'a> {
    fn new(id: i32, proc_id: i32, tx: DelQMsgSender, specs: &'a SystemSpec) -> Self {
        Cache {
            id,
            state: CacheState::Idle,
            tx,
            specs,
            proc_id,
            bus_signals_queue: VecDeque::new(),
            pr_sig_buffer: None,
            data: (0..specs.cache_size/specs.cache_assoc)
                .map(|_| CacheSet::new(specs))
                .collect(),
        }
    }
    // data cache access
    fn set_and_tag_of(&self, addr: &Addr) -> (&CacheSet, i32) {
        let (index, tag) = addr.pos(self.specs);
        (&self.data[index as usize], tag)
    }
    fn state_of(&self, addr: &Addr) -> BlockState {
        let (set, tag) = self.set_and_tag_of(addr);
        let block_state = set.blocks.iter()
            .find(|block| block.tag == tag)
            .map(|block| block.state.clone())
            .unwrap_or(BlockState::Invalid);
        block_state
    }
    fn access_causes_flush(&self, addr: &Addr) -> bool {
        let (set, tag) = self.set_and_tag_of(addr);
        let set_is_full = set.blocks.len() == self.specs.cache_assoc as usize;
        match self.state_of(addr) {
            BlockState::Invalid => set_is_full,
            _ => false
        }
    }
    fn access_uncached(&mut self, addr: &Addr, state: BlockState) {
        let (index, tag) = addr.pos(self.specs);
        match self.state_of(addr) {
            BlockState::Invalid => {
                self.data[index as usize].blocks.push(CacheBlock { tag, state, });
            },
            _ => panic!("Block is unexpectedly cached"),
        }
    }
    fn access_cached(&mut self, addr: &Addr) {
        // employ LRU policy
        let (index, tag) = addr.pos(self.specs);
        match self.state_of(addr) {
            BlockState::Invalid => panic!("Block is unexpectedly uncached"),
            _ => {
                let set = &mut self.data[index as usize];
                let addr_index = set.blocks.iter()
                    .position(|block| block.tag == tag)
                    .unwrap();
                // move block to the end of the set
                let block = set.blocks.remove(addr_index);
                set.blocks.push(block);
            },
        }
    }
    fn set_state_of(&mut self, addr: &Addr, state: BlockState) {
        let (index, tag) = addr.pos(self.specs);
        let set = &mut self.data[index as usize];

        let block = set.blocks.iter_mut()
            .find(|b| b.tag == tag)
            .unwrap();
        block.state = state;
    }
    // events and transitions
    fn handle_pr_req(&mut self, req: PrReq) {
        // invariant: self.state == CacheState::Idle
        let addr = match &req {
            PrReq::Read(addr) => addr.clone(),
            PrReq::Write(addr) => addr.clone(),
        };

        // shorthand helper functions

        let req_ = req.clone();
        let acquire_bus = |self_: &mut Self| {
            self_.state = CacheState::WaitingForBus_PrReq(req_);
            self_.send_bus(BusMsg::Acquire(self_.id), 0);
        };

        let send_bus_tx = |self_: &mut Self, signal: BusSignal| {
            self_.send_bus(BusMsg::BusSig(self_.id, signal), 0);
        };

        let proc_proceed = |self_: &mut Self| {
            self_.send_proc(ProcMsg::RequestResolved, 0);
        };

        let idle = |self_: &mut Self| {
            self_.state = CacheState::Idle;
        };

        let transition = |self_: &mut Self, state: BlockState| {
            self_.set_state_of(&addr, state);
        };

        // state machine
        match self.state_of(&addr) {
            BlockState::Invalid => {
                match req {
                    PrReq::Read(addr) => acquire_bus(self),
                    PrReq::Write(addr) => {
                        if self.access_causes_flush(&addr) {
                            acquire_bus(self);
                        } else {
                            send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                            self.access_uncached(&addr, BlockState::Modified);
                            proc_proceed(self);
                            idle(self);
                        }
                    }
                }
            },
            BlockState::Shared => {
                match req {
                    PrReq::Read(addr) => {
                        // send_bus_tx(BusSignal::BusRd(addr));
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                        transition(self, BlockState::Modified);
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
            },
            BlockState::Exclusive => {
                match req {
                    PrReq::Read(addr) => {
                        // send_bus_tx(BusSignal::BusRd(addr));
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        // send_bus_tx(BusSignal::BusRdX(addr));
                        transition(self, BlockState::Modified);
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
            },
            BlockState::Modified => {
                match req {
                    PrReq::Read(addr) => {
                        // send_bus_tx(BusSignal::BusRd(addr));
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        // send_bus_tx(BusSignal::BusRdX(addr));
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
            },
        }
    }
    fn handle_pr_req_bus_locked(&mut self, req: PrReq, others_have_block: Option<bool>) {
        // invariant: self.state == CacheState::ResolvingPrReq
        let addr = match &req {
            PrReq::Read(addr) => addr.clone(),
            PrReq::Write(addr) => addr.clone(),
        };

        let req_is_write = match &req {
            PrReq::Read(_) => false,
            PrReq::Write(_) => true,
        };
        let block_is_invalid = match self.state_of(&addr) {
            BlockState::Invalid => true,
            _ => false,
        };

        // Todo: also need to check other caches in Dragon on
        //  - Invalid, PrRead
        //  - Invalid, PrWrite
        //  - Shared Clean, PrWrite
        //  - Shared Modified, PrWrite  (theoretically, the snooper should keep track instead, but let's ignore that)
        if others_have_block.is_none() && !(block_is_invalid && req_is_write) {
            // if we are doing something that requires asking other caches, do that before proceeding
            self.send_sim(
                SimMsg::AskOtherCaches(addr.clone()),
                self.specs.t_cache_to_cache_msg() - 1
            );
            self.state = CacheState::AskingCaches(req);
            return;
        }

        // shorthand helper functions

        let transition = |self_: &mut Self, state: BlockState| {
            self_.set_state_of(&addr, state);
        };

        let send_bus_tx = |self_: &mut Self, signal: BusSignal| {
            self_.send_bus(BusMsg::BusSig(self_.id, signal), 0);
        };

        let resolve_in = |self_: &mut Self, time: i32| {
            self_.send_self(CacheMsg::PrReqResolved, time);
        };

        // state machine
        match self.state_of(&addr) {
            BlockState::Invalid => {
                match &req {
                    PrReq::Read(addr) => {
                        if let Some(true) = others_have_block {
                            send_bus_tx(self, BusSignal::BusRd(addr.clone()));
                            self.access_uncached(&addr, BlockState::Shared);
                            // transition(self, BlockState::Shared);
                            resolve_in(self, self.specs.t_cache_to_cache_transfer() - 1);
                        } else {
                            send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                            self.access_uncached(&addr, BlockState::Exclusive);
                            // transition(self, BlockState::Exclusive);
                            resolve_in(self, self.specs.t_mem_fetch() - 1);
                        }
                    },
                    PrReq::Write(addr) => {
                        // means we had to flush the block
                        send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                        self.access_uncached(&addr, BlockState::Modified);
                        // transition(self, BlockState::Modified);
                        resolve_in(self, self.specs.t_flush() - 1);
                    }
                };
                self.state = CacheState::ResolvingPrReq(req, None);
            },
            _ => panic!("Cache in invalid state"),
        }
    }
    fn handle_bus_sig(&mut self, sig: BusSignal) {
        // invariant: self.state == CacheState::Idle

        let addr = match &sig {
            BusSignal::BusRd(addr) => addr.clone(),
            BusSignal::BusRdX(addr) => addr.clone(),
            BusSignal::BusUpd(addr) => addr.clone(),
        };

        // shorthand helper functions

        let sig_ = sig.clone();
        let acquire_bus = |self_: &mut Self| {
            self_.state = CacheState::WaitingForBus_BusSig(sig_);
            self_.send_bus(BusMsg::Acquire(self_.id), 0);
        };

        let transition = |self_: &mut Self, state: BlockState| {
            self_.set_state_of(&addr, state);
        };

        match self.state_of(&addr) {
            BlockState::Invalid => {},
            BlockState::Shared => {
                match sig {
                    BusSignal::BusRd(addr) => {},
                    BusSignal::BusRdX(addr) => {
                        transition(self, BlockState::Invalid);
                    },
                    _ => {},
                }
            },
            BlockState::Exclusive => {
                match sig {
                    BusSignal::BusRd(addr) => {
                        transition(self, BlockState::Shared);
                    },
                    BusSignal::BusRdX(addr) => {
                        // need to flush
                        acquire_bus(self);
                    },
                    _ => {},
                }
            },
            BlockState::Modified => {
                match sig {
                    BusSignal::BusRd(addr) => {
                        // need to flush
                        acquire_bus(self);
                    },
                    BusSignal::BusRdX(addr) => {
                        // need to flush
                        acquire_bus(self);
                    },
                    _ => {},
                }
            },
        }
    }
    fn handle_bus_sig_bus_locked(&mut self, sig: BusSignal) {
        // invariant: self.state == CacheState::ResolvingBusReq

        let addr = match &sig {
            BusSignal::BusRd(addr) => addr.clone(),
            BusSignal::BusRdX(addr) => addr.clone(),
            BusSignal::BusUpd(addr) => addr.clone(),
        };

        // shorthand helper functions

        let transition = |self_: &mut Self, state: BlockState| {
            self_.set_state_of(&addr, state);
        };

        let resolve_in = |self_: &mut Self, time: i32| {
            self_.send_self(CacheMsg::BusReqResolved, time);
        };

        // state machine
        match self.state_of(&addr) {
            BlockState::Exclusive => {
                match sig {
                    BusSignal::BusRdX(addr) => {
                        transition(self, BlockState::Invalid);
                        resolve_in(self, self.specs.t_flush() - 1);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            BlockState::Modified => {
                match sig {
                    BusSignal::BusRd(addr) => {
                        transition(self, BlockState::Shared);
                        resolve_in(self, self.specs.t_flush() - 1);
                    },
                    BusSignal::BusRdX(addr) => {
                        transition(self, BlockState::Invalid);
                        resolve_in(self, self.specs.t_flush() - 1);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            _ => panic!("Cache in invalid state"),
        }
    }
    fn dispatch_signals(&mut self) {
        // check for queued bus signals
        if let Some(sig) = self.bus_signals_queue.pop_front() {
            self.handle_bus_sig(sig);
        }
        // check for queued processor signal
        else if let Some(sig) = self.pr_sig_buffer.take() {
            self.handle_pr_req(sig);
        }
        // otherwise, do nothing
    }
    // sending messages
    fn send_proc(&self, msg: ProcMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::CacheToProc(self.proc_id, msg),
        }).unwrap();
    }
    fn send_bus(&self, msg: BusMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::CacheToBus(self.id, msg),
        }).unwrap();
    }
    fn send_sim(&self, msg: SimMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::CacheToSim(self.id, msg),
        }).unwrap();
    }
    fn send_self(&self, msg: CacheMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::CacheToCache(self.id, msg),
        }).unwrap();
    }
}

impl MsgHandler<CacheMsg> for Cache<'_> {
    fn handle_msg(&mut self, msg: CacheMsg) {
        match &self.state {
            CacheState::Idle => {
                match msg {
                    CacheMsg::Tick => {
                        self.dispatch_signals();
                    },
                    CacheMsg::PostTick => (),
                    CacheMsg::PrSig(req) => {
                        self.pr_sig_buffer = Some(req);
                        self.dispatch_signals();
                    },
                    CacheMsg::BusSig(sig) => {
                        self.bus_signals_queue.push_back(sig);
                        self.dispatch_signals();
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::WaitingForBus_PrReq(req) => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => (),
                    CacheMsg::BusSig(sig) => {
                        self.bus_signals_queue.push_back(sig);
                    },
                    CacheMsg::BusLocked => {
                        let r = req.clone();
                        self.state = CacheState::ResolvingPrReq(r.clone(), None);
                        self.handle_pr_req_bus_locked(r, None);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::ResolvingPrReq(req, others_have_block) => {
                match msg {
                    CacheMsg::Tick => {
                        if others_have_block.is_some() {
                            self.handle_pr_req_bus_locked(req.clone(), others_have_block.clone());
                        }
                    },
                    CacheMsg::PostTick => (),
                    CacheMsg::PrReqResolved => {
                        self.state = CacheState::ResolvingPrReq_ProceedNext;
                        self.send_proc(ProcMsg::RequestResolved, 0);
                        self.send_bus(BusMsg::ReadyToFreeNext, 0);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::AskingCaches(req) => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => (),
                    CacheMsg::CachesChecked(others_have_block) => {
                        self.state = CacheState::AskingCaches_ProceedNext(
                            req.clone(),
                            others_have_block);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::AskingCaches_ProceedNext(req, others_have_block) => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => {
                        self.state = CacheState::ResolvingPrReq(req.clone(), Some(*others_have_block));
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::ResolvingPrReq_ProceedNext => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => {
                        self.state = CacheState::Idle;
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::WaitingForBus_BusSig(sig) => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => (),
                    CacheMsg::PrSig(req) => {
                        self.pr_sig_buffer = Some(req);
                    },
                    CacheMsg::BusSig(sig) => {
                        self.bus_signals_queue.push_back(sig);
                    },
                    CacheMsg::BusLocked => {
                        let s = sig.clone();
                        self.state = CacheState::ResolvingBusReq(s.clone());
                        self.handle_bus_sig_bus_locked(s);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::ResolvingBusReq(sig) => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => (),
                    CacheMsg::PrSig(req) => {
                        self.pr_sig_buffer = Some(req);
                    },
                    CacheMsg::BusReqResolved => {
                        self.state = CacheState::ResolvingBusReq_ProceedNext;
                        self.send_bus(BusMsg::ReadyToFreeNext, 0);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            CacheState::ResolvingBusReq_ProceedNext => {
                match msg {
                    CacheMsg::Tick => (),
                    CacheMsg::PostTick => {
                        self.state = CacheState::Idle;
                    },
                    CacheMsg::PrSig(req) => {
                        self.pr_sig_buffer = Some(req);
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
        }
    }
}

// bus

#[derive(Clone)]
enum BusMsg {
    Tick,
    PostTick,
    Acquire(i32),
    BusSig(i32, BusSignal),
    SignalSent(i32, BusSignal),
    ReadyToFreeNext,
}

#[derive(Clone, Debug)]
enum BusSignal {
    BusRd(Addr),
    BusRdX(Addr),
    BusUpd(Addr),
}

#[derive(Clone, Debug)]
enum BusState {
    Unlocked_Idle,      // bus free / not locked
    Unlocked_Busy,      // bus free / not locked
    Locked(i32),   // bus is currently owned by a single cache
    FreeNext,
}

struct Bus<'a> {
    state: BusState,
    tx: DelQMsgSender,
    n: i32,
    specs: &'a SystemSpec,
    cache_ids: Vec<i32>,
    signal_queue: VecDeque<(BusSignal, i32)>,   // signals have higher priority than explicit locks by caches
    lock_queue: VecDeque<i32>,                  // explicit locks by caches
}

impl<'a> Bus<'a> {
    fn new(n: i32, cache_ids: Vec<i32>, tx: DelQMsgSender, specs: &'a SystemSpec) -> Self {
        Bus {
            state: BusState::Unlocked_Idle,
            tx,
            n,
            specs,
            cache_ids,
            signal_queue: VecDeque::new(),
            lock_queue: VecDeque::new(),
        }
    }
    fn send_cache(&self, cache_id: i32, msg: CacheMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::BusToCache(cache_id, msg),
        }).unwrap();
    }
    fn send_caches(&self, msg: CacheMsg, delay: i32, except: i32) {
        for cache_id in &self.cache_ids {
            if except == *cache_id { continue; }
            self.send_cache(*cache_id, msg.clone(), delay);
        }
    }
    fn send_self(&self, msg: BusMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::BusToBus(msg),
        }).unwrap();
    }
}

impl MsgHandler<BusMsg> for Bus<'_> {
    fn handle_msg(&mut self, msg: BusMsg) {
        match self.state {
            BusState::Unlocked_Idle => {
                match msg {
                    BusMsg::Tick => {
                        // check if there's something in the bus signal queue
                        if let Some((sig, cache_id)) = self.signal_queue.pop_front() {
                            let t = self.specs.t_cache_to_cache_msg() - 1;
                            self.send_caches(
                                CacheMsg::BusSig(sig.clone()),
                                t,
                                cache_id);
                            self.send_self(
                                BusMsg::SignalSent(cache_id, sig.clone()),
                                t);
                            self.state = BusState::Unlocked_Busy;
                        }
                        // otherwise, free to be locked by a cache
                        else if let Some(cache_id) = self.lock_queue.pop_front() {
                            self.send_cache(cache_id, CacheMsg::BusLocked, 0);
                            self.state = BusState::Locked(cache_id);
                        }
                    },
                    BusMsg::PostTick => (),
                    BusMsg::Acquire(cache_id) => {
                        self.send_cache(cache_id, CacheMsg::BusLocked, 0);
                        self.state = BusState::Locked(cache_id);
                    },
                    BusMsg::BusSig(cache_id, sig) => {
                        let t = self.specs.t_cache_to_cache_msg() - 1;
                        self.send_self(BusMsg::SignalSent(cache_id, sig), t);
                        self.state = BusState::Unlocked_Busy;
                    },
                    _ => panic!("Invalid bus state"),
                }
            },
            BusState::Unlocked_Busy => {
                match msg {
                    BusMsg::Tick => (),
                    BusMsg::PostTick => (),
                    BusMsg::Acquire(cache_id) =>
                        self.lock_queue.push_back(cache_id),
                    BusMsg::BusSig(cache_id, sig) =>
                        self.signal_queue.push_back((sig, cache_id)),
                    BusMsg::SignalSent(cache_id, sig) => {
                        self.send_caches(CacheMsg::BusSig(sig), 0, cache_id);
                        self.state = BusState::FreeNext;
                    },
                    _ => panic!("Invalid bus state"),
                }
            },
            BusState::Locked(_) => {
                match msg {
                    BusMsg::Tick => (),
                    BusMsg::PostTick => (),
                    BusMsg::Acquire(cache_id) =>
                        self.lock_queue.push_back(cache_id),
                    BusMsg::BusSig(cache_id, sig) =>
                        self.signal_queue.push_back((sig, cache_id)),
                    BusMsg::SignalSent(cache_id, sig) => {
                        self.send_caches(CacheMsg::BusSig(sig), 0, cache_id);
                        self.state = BusState::Locked(cache_id);
                    },
                    BusMsg::ReadyToFreeNext =>
                        self.state = BusState::FreeNext,
                }
            },
            BusState::FreeNext => {
                match msg {
                    BusMsg::Tick => (),
                    BusMsg::PostTick =>
                        self.state = BusState::Unlocked_Idle,
                    BusMsg::Acquire(cache_id) =>
                        self.lock_queue.push_back(cache_id),
                    BusMsg::BusSig(cache_id, sig) =>
                        self.signal_queue.push_back((sig, cache_id)),
                    _ => panic!("Invalid bus state"),
                }
            }
        }
    }
}

// pretty printing

struct Printer{
    cycle_width: i32,
    proc_width: i32,
    cache_width: i32,
    bus_width: i32,
}
impl Printer {
    pub fn print_header(procs: &Vec<Processor>, caches: &Vec<Cache>, bus: &Bus) {
        let mut s = String::new();
        s.push_str(&format!("{:<6} | ", "cycle"));
        for proc in procs {
            s.push_str(&format!("P{:<20} | ", proc.id));
        }
        for cache in caches {
            s.push_str(&format!("C{:<30} | ", cache.id));
        }
        s.push_str(&format!("{:<15}", "Bus"));
        println!("{}", s);
    }
    pub fn print_row(cycle: i32, procs: &Vec<Processor>, caches: &Vec<Cache>, bus: &Bus) {
        // print a nice table of the states of all processors, caches, and the bus
        let mut s = String::new();
        s.push_str(&format!("{:<6} | ", cycle));
        for proc in procs {
            s.push_str(&format!("{:<20} | ", format!("{:?}", proc.state)));
        }
        for cache in caches {
            s.push_str(&format!("{: <30} | ", format!("{:?}", cache.state)));
        }
        s.push_str(&format!("{: <15}", format!("{:?}", bus.state)));
        println!("{}", s);
    }
}

// simulator

enum SimMsg {
    AskOtherCaches(Addr),  // provides interface to check info that requires broad access
}

fn simulate(specs: SystemSpec, insts: Vec<Instructions>) {

    let n = insts.len() as i32;

    // each component (processors, caches, bus) communicates to others by sending messages
    // to the simulator (main thread) via channels which will forward messages to the
    // intended recipient

    // implement everything single-threaded for now

    let (mut dq, tx) = DelayedQ::<Msg>::new();

    let mut procs = (0..n).map(|i| {
        Processor::new(
            i,
            i,
            insts[i as usize].clone(),
            tx.clone(),
            &specs)
    }).collect::<Vec<_>>();

    let mut caches = (0..n).map(|i| {
        Cache::new(
            i,
            i,
            tx.clone(),
            &specs)
    }).collect::<Vec<_>>();

    let mut bus = Bus::new(
        n,
        (0..n).collect::<Vec<_>>(),
        tx.clone(),
        &specs);

    let send_msg = move |msg: Msg| {
        tx.send(DelayedMsg {
            t: 0,
            msg,
        }).unwrap();
    };

    // simulate
    let mut cycle_count = 0;
    Printer::print_header(&procs, &caches, &bus);
    Printer::print_row(cycle_count, &procs, &caches, &bus);
    loop {
        print!("");
        // tick everyone -- THE ORDER SHOULD NOT MATTER!!
        for proc_id in 0..n {
            send_msg(Msg::ToProc(proc_id, ProcMsg::Tick));
        }
        for cache_id in 0..n {
            send_msg(Msg::ToCache(cache_id, CacheMsg::Tick));
        }
        send_msg(Msg::ToBus(BusMsg::Tick));

        dq.update_q();

        // handle messages
        while let Some(msg) = dq.try_fetch() {
            match msg {
                Msg::ToProc(i, msg) => procs[i as usize].handle_msg(msg),
                Msg::ToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::ProcToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::CacheToProc(i, msg) => procs[i as usize].handle_msg(msg),
                Msg::CacheToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::ToBus(msg) => bus.handle_msg(msg),
                Msg::CacheToBus(_, msg) => bus.handle_msg(msg),
                Msg::BusToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::BusToBus(msg) => bus.handle_msg(msg),
                Msg::CacheToSim(i, SimMsg::AskOtherCaches(addr)) => {
                    let hit = caches.iter().any(|c| {
                        c.id != i && (match c.state_of(&addr) {
                            BlockState::Invalid => false,
                            _ => true,
                        })
                    });
                    send_msg(Msg::SimToCache(i, CacheMsg::CachesChecked(hit)));
                },
                Msg::SimToCache(i, msg) => caches[i as usize].handle_msg(msg),
            }
            if !dq.msg_available() { dq.update_q() }
        }

        // post-tick everyone -- THE ORDER SHOULD NOT MATTER!!
        for proc_id in 0..n {
            procs[proc_id as usize].handle_msg(ProcMsg::PostTick);
        }
        for cache_id in 0..n {
            caches[cache_id as usize].handle_msg(CacheMsg::PostTick);
        }
        bus.handle_msg(BusMsg::PostTick);

        cycle_count += 1;
        dq.update_time(cycle_count);

        Printer::print_row(cycle_count, &procs, &caches, &bus);

        if procs.iter().all(|p| p.state == ProcState::Done) && dq.is_empty() { break; }

    }
    println!("cycles: {}", cycle_count);
}


fn main() {
    simulate(
        SystemSpec::new(),
        vec![
            VecDeque::from(vec![
                Instr::Read(Addr(0)),
                Instr::Other(10),
                Instr::Write(Addr(0)),
                // Instr::Other(2),
                // Instr::Read(Addr(1)),
                // Instr::Other(3),
                // Instr::Read(Addr(0)),
                // Instr::Other(4),
                // Instr::Write(Addr(1)),
            ])
        ]
    )
}


// testing

#[test]
fn test_delayed_queue() {
    // test delayed queue

    let (mut dq, tx) = DelayedQ::<i32>::new();

    tx.send(DelayedMsg {
        t: 0,
        msg: 42,
    }).unwrap();

    tx.send(DelayedMsg {
        t: 0,
        msg: 43,
    }).unwrap();

    tx.send(DelayedMsg {
        t: 1,
        msg: 44,
    }).unwrap();

    dq.update_q();
    let mut c = 0;
    let mut x = false;
    while dq.msg_available() {
        println!("messages after {} cycles:", c);
        while let Some(msg) = dq.try_fetch() {
            println!("msg: {}", msg);
            if !x {
                tx.send(DelayedMsg { t: 0, msg: 100 }).unwrap();
                dq.update_q();
                x = true;
                println!("appended another message in cycle {}", c);
            }
        }
        c += 1;
        dq.update_time(c);
    }

    println!("done, cycles: {}", c);
}