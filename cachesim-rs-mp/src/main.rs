extern crate core;

mod delayed_q;

use std::collections::VecDeque;
use crate::delayed_q::*;

type DelQMsgSender = DelQSender<Msg>;

/*
    A MESI and Dragon cache coherence protocol simulator.
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

enum ProcMsg {
    // Tick,
    ReadyToProceedNext,
}

#[derive(Clone)]
enum CacheMsg {
    // Tick,
    Read(Addr),
    Write(Addr),
    BusSignal(BusSignal),           // incoming bus signal
    BusLocked,                      // bus is locked by the cache
}

enum BusMsg {
    // StayBusy(i32, i32),
    Acquire(i32),                   // locks the bus synchronously
    QueueSignal(i32, BusSignal),    // queues a signal, which will lock the bus asynchronously
    ReadyToFreeNext,                // frees the bus at the end of the cycle
}

#[derive(Clone)]
enum BusSignal {
    BusRd(Addr),
    BusRdX(Addr),
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

#[derive(Clone, PartialEq)]
enum ProcState {
    Ready,
    ExecutingOther(i32),
    WaitingForCache,
    Done,

    ProceedNext,
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
    fn tick(&mut self) {
        self.state = match self.state {
            ProcState::Ready => {
                match self.instructions.pop().unwrap() {
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
            }
            ProcState::ExecutingOther(time) => ProcState::ExecutingOther(time - 1),
            ProcState::WaitingForCache => ProcState::WaitingForCache,
            ProcState::Done => ProcState::Done,
            _ => panic!("Processor in invalid state"),
        }
    }
    fn handle_msg(&mut self, msg: ProcMsg) {
        match msg {
            ProcMsg::ReadyToProceedNext =>
                self.state = ProcState::ProceedNext
        }
    }
    fn post_tick(&mut self) {
        self.state = match self.state {
            ProcState::ExecutingOther(0) => ProcState::Ready,
            ProcState::ProceedNext => ProcState::Ready,
            _ => self.state.clone(),
        };
        if self.state == ProcState::Ready && self.instructions.len() == 0 {
            self.state = ProcState::Done;
        }
    }
}

// caches

enum CacheState {
    Idle,
    ResolvingRequest(i32),
    WaitingForBus(),
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

#[derive(Clone)]
enum BusState {
    Free,
    Locked(i32),
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
            state: BusState::Free,
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
    fn send_sig(&self, cache_id: i32, sig: BusSignal) {
        self.send_caches(
            CacheMsg::BusSignal(sig),
            self.specs.t_cache_to_cache_msg(),
            Some(cache_id));
    }
    fn tick(&mut self) {
        // match self.state {
        //     BusState::Free => {
        //         if let Some((sig, proc_id)) = self.signal_queue.pop_front() {
        //             // the signal is old, send it immediately
        //             self.state = BusState::Locked(proc_id.unwrap());
        //             self.send_sig(self.cache_ids[0], sig);
        //         }
        //     },
        //     _ => {}
        // }
    }
    fn handle_msg(&mut self, msg: BusMsg) {
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
        t: 1,
        msg: 43,
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
