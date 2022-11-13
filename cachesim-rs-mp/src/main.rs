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

// addresses

#[derive(Clone)]
struct Addr(i32);

impl Addr {
    fn new(addr: i32) -> Addr {
        Addr(addr)
    }
    fn pos(&self, specs: &SystemSpec) -> (i32, i32) {
        // returns the index and of the address under given system specs
        let num_indices = specs.cache_size / (specs.block_size * specs.cache_assoc);
        let index = self.0 % num_indices;
        let tag = self.0 / num_indices;
        (index, tag)
    }
}

// messages

enum Msg {
    ProcToCache(i32, CacheMsg),
    CacheToProc(i32, ProcMsg),
    CacheToCache(i32, CacheMsg),
    CacheToBus(i32, BusMsg),
    BusToCache(i32, CacheMsg),
    BusToBus(i32, BusMsg),

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

type Instructions = Vec<Instr>;

// processors

enum ProcMsg {
    Tick,
    PostTick,
    RequestResolved,
}

#[derive(Clone, PartialEq)]
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
            state: ProcState::Ready,
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
                self.state = match self.instructions.pop().unwrap() {
                    Instr::Read(addr) => {
                        self.send_cache(CacheMsg::Read(addr), 0);
                        ProcState::WaitingForCache
                    }
                    Instr::Write(addr) => {
                        self.send_cache(CacheMsg::Write(addr), 0);
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
                    _ => panic!("Processor in invalid state"),
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
                    _ => panic!("Processor in invalid state"),
                }
            },
            ProcState::Done => (),
        }
    }
}

// caches

#[derive(Clone)]
enum CacheMsg {
    Tick,
    PostTick,
    PrSig,
    BusSig,
    BusLocked,
    BusReqResolved,
    PrReqResolved,
}

enum CacheState {
    Idle,

    WaitingForBus_PrSig(),
    ResolvingPrReq(),
    PrReqResolved,

    WaitingForBus_BusSig(),
    ResolvingBusReq(),
    BusReqResolved,
}

struct Cache<'a> {
    id: i32,
    state: CacheState,
    tx: DelQMsgSender,
    specs: &'a SystemSpec,
    proc_id: i32,
    bus_id: i32,
}

impl<'a> Cache<'a> {
    fn new(id: i32, proc_id: i32, bus_id: i32, tx: DelQMsgSender, specs: &'a SystemSpec) -> Self {
        Cache {
            id,
            state: CacheState::Idle,
            tx,
            specs,
            proc_id,
            bus_id,
        }
    }
    fn tick(&mut self) {
        todo!()
    }
    fn handle_msg(&mut self, msg: CacheMsg) {
        todo!()
    }
}

// bus

enum BusMsg {
    Tick,
    PostTick,
    Acquire,
    QBusSig,
    SignalSent,
    ReadyToFreeNext,
}

#[derive(Clone)]
enum BusSignal {
    BusRd(Addr),
    BusRdX(Addr),
    BusUpd(Addr),
}

#[derive(Clone)]
enum BusState {
    Unlocked_Idle,      // bus free / not locked
    Unlocked_Busy,      // bus free / not locked
    Locked(i32),   // bus is currently owned by a single cache
    FreeNext,
}

struct Bus<'a> {
    bus_id: i32,
    state: BusState,
    tx: DelQMsgSender,
    n: i32,
    specs: &'a SystemSpec,
    cache_ids: Vec<i32>,
    signal_queue: VecDeque<(BusSignal, Option<i32>)>,   // signals have higher priority than explicit locks by caches
    lock_queue: VecDeque<i32>,                          // explicit locks by caches
}

impl<'a> Bus<'a> {
    fn new(bus_id: i32, n: i32, cache_ids: Vec<i32>, tx: DelQMsgSender, specs: &'a SystemSpec) -> Self {
        Bus {
            bus_id,
            state: BusState::Free(BusFreeState::Idle),
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
    fn send_caches(&self, msg: CacheMsg, delay: i32, except: Option<i32>) {
        for cache_id in &self.cache_ids {
            if except.is_some() && except.unwrap() == *cache_id { continue; }
            self.send_cache(*cache_id, msg.clone(), delay);
        }
    }
    fn send_self(&self, msg: BusMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::BusToBus(self.bus_id, msg),
        }).unwrap();
    }
    fn fetch_queues(&mut self) -> BusState {
        // first check if there are pending bus signals to send
        if let Some((sig, cache_id)) = self.signal_queue.pop_front() {
            let t = self.specs.t_cache_to_cache_msg();
            self.send_caches(
                CacheMsg::BusSignal(sig.clone()),
                t,
                cache_id);
            self.send_self(
                BusMsg::SignalSent(cache_id, sig.clone()),
                t);
            BusState::Unlocked_Busy
        }
        // otherwise, free to be locked by a cache
        else if let Some(cache_id) = self.lock_queue.pop_front() {
            self.send_cache(cache_id, BusLocked, 0);
            BusState::Locked_Idle(cache_id)
        } else {
            BusState::Unlocked_Idle
        }
    }
    fn tick(&mut self) {
        match &self.state {
            BusState::Unlocked_Idle => self.state = self.fetch_queues(),
            _ => {}
        }
    }
    fn handle_msg(&mut self, msg: BusMsg) {
        self.state = match (self.state, msg) {

            // busy and idle
            (BusState::Unlocked_Idle, BusMsg::GoBusy) => BusState::Unlocked_Busy,
            (BusState::Unlocked_Busy, BusMsg::GoIdle) => BusState::Unlocked_Idle,
            (BusState::Locked_Idle(cache_id), BusMsg::GoBusy) => BusState::Locked_Busy(cache_id),
            (BusState::Locked_Busy(cache_id), BusMsg::GoIdle) => BusState::Locked_Idle(cache_id),

            // acquiring lock
            (_, BusMsg::Acquire(cache_id)) => {
                self.lock_queue.push_back(cache_id);
                match self.state {
                    BusState::Unlocked_Idle => self.fetch_queues(),
                    _ => self.state.clone(),
                }
            },

            // releasing lock
            (BusState::Locked_Idle(id), BusMsg::ReadyToFreeNext) => {
                BusState::FreeNext
            }

            // sending signals
            (BusState::Unlocked_Idle, BusMsg::QueueSignal(cache_id, sig)) => {
                self.signal_queue.push_back((sig, Some(cache_id)));
                self.fetch_queues()
            },
            (BusState::Unlocked_Busy, BusMsg::QueueSignal(cache_id, sig)) => {
                self.signal_queue.push_back((sig, Some(cache_id)));
                BusState::Unlocked_Busy
            },
            (BusState::Locked_Idle(owner_id), BusMsg::QueueSignal(cache_id, sig)) => {
                self.signal_queue.push_back((sig, Some(cache_id)));
                // send signal immediately if it came from owner
                if cache_id == owner_id { self.fetch_queues() }
                else { BusState::Locked_Idle(owner_id) }
            },

            _ => panic!("Invalid bus state"),
        }
        // match self.state {
        //     BusState::Free =>
        //         match msg {
        //             BusMsg::Acquire(cache_id) => {
        //                 self.state = BusState::Locked(cache_id);
        //                 self.send_cache(cache_id, CacheMsg::BusLocked, 0);
        //             },
        //             BusMsg::QueueSignal(cache_id, sig) => {
        //                 // start sending immediately
        //                 self.state = BusState::Locked(cache_id);
        //                 self.send_self(
        //                     BusMsg::SendSignal(cache_id, sig),
        //                     self.specs.t_cache_to_cache_msg() - 1);
        //             },
        //             _ => panic!("Bus received unexpected message"),
        //         },
        //     BusState::Locked(cache_id) =>
        //         match msg {
        //             BusMsg::SendSignal(cache_id, sig) => {
        //                 self.send_sig(cache_id, sig)
        //             },
        //             BusMsg::ReadyToFreeNext => {
        //                 self.state = BusState::FreeNext;
        //             },
        //         },
        //     BusState::FreeNext =>
        //         todo!()
        // }
    }
    fn post_tick(&mut self) {
        self.state = match self.state {
            BusState::FreeNext => BusState::Free,
            _ => self.state.clone(),
        };
    }
}

// simulator

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
            i+n,
            insts[i as usize].clone(),
            tx.clone(),
            &specs)
    }).collect::<Vec<_>>();

    let mut caches = (0..n).map(|i| {
        Cache::new(
            i+n,
            i,
            2*n,
            tx.clone(),
            &specs)
    }).collect::<Vec<_>>();

    let mut bus = Bus::new(
        2*n,
        n,
        (n..2*n).collect::<Vec<_>>(),
        tx.clone(),
        &specs);

    // simulate
    let mut cycle_count = 0;
    loop {
        // tick everyone -- THE ORDER SHOULD NOT MATTER!!
        for i in 0..n as usize {
            procs[i].tick();
            caches[i].tick();
        }
        bus.tick();

        // handle messages
        while let Some(msg) = dq.try_fetch() {
            match msg {
                Msg::ProcToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::CacheToProc(i, msg) => procs[i as usize].handle_msg(msg),
                Msg::CacheToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::CacheToBus(i, msg) => bus.handle_msg(msg),
                Msg::BusToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::BusToBus(i, msg) => bus.handle_msg(msg),
            }
            if !dq.msg_available() { dq.update_q() }
        }

        cycle_count += 1;
        dq.update_time(cycle_count);

        if procs.iter().all(|p| p.state == ProcState::Done) { break; }
    }
    println!("cycles: {}", cycle_count);
}


fn main() {
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
