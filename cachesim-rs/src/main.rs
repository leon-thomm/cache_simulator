/*
    A Simulator for MESI (Illinois) and Dragon 4-state update-based cache coherence protocols.
 */

/* uncomment to use hashed, binary heap-based queue */
// mod delayed_q;
// use crate::delayed_q::*;

/* uncomment to use un-hashed, vector-based queue */
mod delayed_q_unhashed;
use crate::delayed_q_unhashed::*;

extern crate core;
use std::time::Instant;
use std::collections::{HashMap, VecDeque};
use std::{env, fs};
use std::fs::File;
use std::io::Read;

type DelQMsgSender = DelQSender<Msg>;

// system specs

#[derive(PartialEq, Debug)]
enum Protocol {
    MESI,
    Dragon,
}

struct SystemSpec {
    protocol: Protocol,
    word_size: i32,
    address_size: i32,
    mem_lat: i32,
    bus_word_tf_lat: i32,
    block_size: i32,
    cache_size: i32,
    cache_assoc: i32,
}

impl Default for SystemSpec {
    fn default() -> Self {
        SystemSpec {
            protocol: Protocol::MESI,
            word_size: 4,       // bytes
            address_size: 4,    // bytes
            mem_lat: 100,       // cpu cycles
            bus_word_tf_lat: 2, // cpu cycles
            block_size: 32,     // bytes
            cache_size: 4096,   // bytes
            cache_assoc: 2,     // blocks
        }
    }
}

impl SystemSpec {
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
}

// addresses and blocks

#[derive(Clone, PartialEq, Debug)]
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

#[derive(Clone)]
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
    RequestResolved,
    ExecutingOther(i32),
    Done,
}

struct Processor<'a> {
    id: i32,
    state: ProcState,
    instructions: Instructions,
    tx: DelQMsgSender,
    specs: &'a SystemSpec,
    cache_id: i32,

    num_loads: i32,
    num_stores: i32,
    num_wait_cycles: i32,
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

            num_loads: 0,
            num_stores: 0,
            num_wait_cycles: 0,
        }
    }
    fn send_cache(&self, msg: CacheMsg, delay: i32) {
        self.tx.send(DelayedMsg {
            t: delay,
            msg: Msg::ProcToCache(self.cache_id, msg),
        }).unwrap();
    }
    fn proceed(&mut self) {
        if self.instructions.len() == 0 {
            self.state = ProcState::Done;
        } else {
            self.state = ProcState::Idle;
        }
    }
}

impl MsgHandler<ProcMsg> for Processor<'_> {
    fn handle_msg(&mut self, msg: ProcMsg) {

        match self.state {
            ProcState::Idle => {
                self.state = match self.instructions.pop_front().unwrap() {
                    Instr::Read(addr) => {
                        self.num_loads += 1;
                        self.send_cache(CacheMsg::PrSig(PrReq::Read(addr)), 0);
                        ProcState::WaitingForCache
                    }
                    Instr::Write(addr) => {
                        self.num_stores += 1;
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
                    ProcMsg::PostTick => self.num_wait_cycles += 1,
                }
            },
            ProcState::RequestResolved => {
                match msg {
                    ProcMsg::Tick => (),
                    ProcMsg::PostTick => self.proceed(),
                    ProcMsg::RequestResolved => panic!("Processor in invalid state"),
                }
            },
            ProcState::ExecutingOther(time) => {
                match msg {
                    ProcMsg::Tick => {
                        self.state = ProcState::ExecutingOther(time - 1);
                    },
                    ProcMsg::PostTick => {
                        if time == 0 { self.proceed(); }
                    },
                    ProcMsg::RequestResolved => panic!("Processor in invalid state"),
                }
            },
            ProcState::Done => (),
        }
    }
}

// caches

#[derive(Clone, PartialEq, Debug)]
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

#[derive(Clone, PartialEq, Debug)]
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
    MESI_Invalid,
    MESI_Shared,
    MESI_Exclusive,
    MESI_Modified,

    Dragon_Invalid,
    Dragon_SharedClean,
    Dragon_SharedModified,
    Dragon_Exclusive,
    Dragon_Modified,
}

struct CacheSet<'a> {
    //               tag    last_used,  block
    blocks: HashMap<i32,    (i32,       CacheBlock)>,
    specs: &'a SystemSpec,
    mru_ctr: i32,
}

impl<'a> CacheSet<'a> {
    fn new(specs: &'a SystemSpec) -> CacheSet {
        let map = HashMap::with_capacity(specs.cache_assoc as usize);
        CacheSet {
            blocks: map,
            specs,
            mru_ctr: -1,
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
    data: Vec<CacheSet<'a>>,

    num_misses: i32,
    num_hits: i32,
    amt_issued_bus_traffic: i32,
    num_invalidations: i32,
    num_private_accesses: i32,
    num_shared_accesses: i32,
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

            num_misses: 0,
            num_hits: 0,
            amt_issued_bus_traffic: 0,
            num_invalidations: 0,
            num_private_accesses: 0,
            num_shared_accesses: 0,
        }
    }
    // stats
    fn inc_misses(&mut self) { self.num_misses += 1; }
    fn inc_hits(&mut self) { self.num_hits += 1; }
    fn inc_traffic(&mut self) { self.amt_issued_bus_traffic += self.specs.block_size; }
    fn inc_invalidations(&mut self) { self.num_invalidations += 1; }
    fn inc_priv_acc(&mut self) { self.num_private_accesses += 1; }
    fn inc_shared_acc(&mut self) { self.num_shared_accesses += 1; }
    // data cache access
    fn set_and_tag_of(&self, addr: &Addr) -> (&CacheSet, i32) {
        let (index, tag) = addr.pos(self.specs);
        (&self.data[index as usize], tag)
    }
    fn state_of(&self, addr: &Addr) -> BlockState {
        let (set, tag) = self.set_and_tag_of(addr);
        let block_state = set.blocks
            .get(&tag)
            .map(|(_, b)| b.state.clone())
            .unwrap_or(match self.specs.protocol {
                Protocol::MESI => BlockState::MESI_Invalid,
                Protocol::Dragon => BlockState::Dragon_Invalid,
            });
        block_state
    }
    fn access_causes_flush(&mut self, addr: &Addr) -> bool {
        let (set, tag) = self.set_and_tag_of(addr);
        let set_is_full = set.blocks.len() == self.specs.cache_assoc as usize;
        let ret = match self.state_of(addr) {
            BlockState::MESI_Invalid | BlockState::Dragon_Invalid => set_is_full,
            _ => false
        };
        ret
    }
    fn access_uncached(&mut self, addr: &Addr, state: BlockState) {
        self.inc_misses();
        let (index, tag) = addr.pos(self.specs);
        if self.data[index as usize].blocks.len() == self.specs.cache_assoc as usize {
            // evict lru
            let lru_tag = self.data[index as usize].blocks
                .iter()
                .min_by_key(|(_, (last_used, _))| last_used)
                .map(|(tag, _)| *tag)
                .unwrap();
            self.data[index as usize].blocks.remove(&lru_tag);
            self.inc_invalidations();
        }
        self.data[index as usize].mru_ctr += 1;
        let new_key = self.data[index as usize].mru_ctr;
        self.data[index as usize].blocks.insert(tag, (new_key, CacheBlock { tag, state, }));
    }
    fn access_cached(&mut self, addr: &Addr) {
        self.inc_hits();
        let (index, tag) = addr.pos(self.specs);
        self.data[index as usize].mru_ctr += 1;
        self.data[index as usize].blocks.get_mut(&tag).unwrap().0 =
            self.data[index as usize].mru_ctr;
    }
    fn set_state_of(&mut self, addr: &Addr, state: BlockState) {
        let (index, tag) = addr.pos(self.specs);
        let set = &mut self.data[index as usize];
        let (_, block) = set.blocks.get_mut(&tag).unwrap();
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

        // for tracking private or shared access:
        // if the addr is cached, then we know immediately.
        // only if it is invalid we need to figure it out when the bus is locked
        // (except MESI Invalid Write, we count that as private)

        // state machine
        match self.state_of(&addr) {
            BlockState::MESI_Invalid => {
                match req {
                    PrReq::Read(addr) =>
                        acquire_bus(self),
                    PrReq::Write(addr) => {
                        if self.access_causes_flush(&addr) {
                            acquire_bus(self);
                        } else {
                            send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                            self.access_uncached(&addr, BlockState::MESI_Modified);
                            proc_proceed(self);
                            idle(self);
                            self.inc_priv_acc();
                        }
                    }
                }
            },
            BlockState::MESI_Shared => {
                match req {
                    PrReq::Read(addr) => {
                        // send_bus_tx(BusSignal::BusRd(addr));
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                        transition(self, BlockState::MESI_Modified);
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
                self.inc_shared_acc();
            },
            BlockState::MESI_Exclusive => {
                match req {
                    PrReq::Read(addr) => {
                        // send_bus_tx(BusSignal::BusRd(addr));
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        // send_bus_tx(BusSignal::BusRdX(addr));
                        transition(self, BlockState::MESI_Modified);
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
                self.inc_priv_acc();
            },
            BlockState::MESI_Modified => {
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
                self.inc_priv_acc();
            },

            BlockState::Dragon_Invalid => acquire_bus(self),
            BlockState::Dragon_SharedClean => {
                match req {
                    PrReq::Read(addr) => {
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => acquire_bus(self),
                }
                self.inc_shared_acc();
            },
            BlockState::Dragon_SharedModified => {
                match req {
                    PrReq::Read(addr) => {
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => acquire_bus(self),
                }
                self.inc_shared_acc();
            },
            BlockState::Dragon_Exclusive => {
                match req {
                    PrReq::Read(addr) => {
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        transition(self, BlockState::Dragon_Modified);
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
                self.inc_priv_acc();
            },
            BlockState::Dragon_Modified => {
                match req {
                    PrReq::Read(addr) => {
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    },
                    PrReq::Write(addr) => {
                        self.access_cached(&addr);
                        proc_proceed(self);
                        idle(self);
                    }
                }
                self.inc_priv_acc();
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
            BlockState::MESI_Invalid | BlockState::Dragon_Invalid => true,
            _ => false,
        };
        let block_is_shared = match self.state_of(&addr) {
            BlockState::MESI_Shared |
            BlockState::Dragon_SharedClean |
            BlockState::Dragon_SharedModified =>
                true,
            _ => false,
        };
        let mesi = self.specs.protocol == Protocol::MESI;
        let dragon = self.specs.protocol == Protocol::Dragon;

        // if we are doing something that requires asking other caches, do that before proceeding
        let need_to_ask_other_caches = others_have_block.is_none() && (
            (mesi && !(block_is_invalid && req_is_write)) ||
            (dragon && (block_is_invalid || (req_is_write && block_is_shared)))
        );
        // for Dragon, we naively ask other caches every time we write to SharedModified
        if need_to_ask_other_caches {
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

        // for tracking private and shared access: see comment in handle_pr_req()

        // state machine
        match self.state_of(&addr) {
            BlockState::MESI_Invalid => {
                match &req {
                    PrReq::Read(addr) => {
                        if let Some(true) = others_have_block {
                            send_bus_tx(self, BusSignal::BusRd(addr.clone()));
                            self.access_uncached(&addr, BlockState::MESI_Shared);
                            resolve_in(self, self.specs.t_cache_to_cache_transfer() - 1);
                            self.inc_shared_acc();
                        } else {
                            send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                            self.access_uncached(&addr, BlockState::MESI_Exclusive);
                            resolve_in(self, self.specs.t_mem_fetch() - 1);
                            self.inc_priv_acc();
                        }
                    },
                    PrReq::Write(addr) => {
                        // means we had to flush the block
                        send_bus_tx(self, BusSignal::BusRdX(addr.clone()));
                        self.access_uncached(&addr, BlockState::MESI_Modified);
                        resolve_in(self, self.specs.t_flush() - 1);
                    }
                };
                self.inc_traffic();
                self.state = CacheState::ResolvingPrReq(req, None);
            },

            BlockState::Dragon_Invalid => {
                match &req {
                    PrReq::Read(addr) => {
                        if let Some(true) = others_have_block {
                            send_bus_tx(self, BusSignal::BusRd(addr.clone()));
                            self.access_uncached(&addr, BlockState::Dragon_SharedClean);
                            resolve_in(self, self.specs.t_cache_to_cache_transfer() - 1);
                            self.inc_shared_acc();
                        } else {
                            send_bus_tx(self, BusSignal::BusRd(addr.clone()));
                            self.access_uncached(&addr, BlockState::Dragon_Exclusive);
                            resolve_in(self, self.specs.t_mem_fetch() - 1);
                            self.inc_priv_acc();
                        }
                        self.inc_traffic();
                    },
                    PrReq::Write(addr) => {
                        if let Some(true) = others_have_block {
                            send_bus_tx(self, BusSignal::BusRd(addr.clone()));
                            send_bus_tx(self, BusSignal::BusUpd(addr.clone()));
                            self.access_uncached(&addr, BlockState::Dragon_SharedModified);
                            resolve_in(self, 0);
                            self.inc_shared_acc();
                            self.inc_traffic();
                        } else {
                            send_bus_tx(self, BusSignal::BusRd(addr.clone()));
                            self.access_uncached(&addr, BlockState::Dragon_Modified);
                            resolve_in(self, 0);
                            self.inc_priv_acc();
                        }
                    }
                };
                self.state = CacheState::ResolvingPrReq(req, None);
            },
            BlockState::Dragon_SharedClean => {
                match &req {
                    PrReq::Write(addr) => {
                        if let Some(true) = others_have_block {
                            send_bus_tx(self, BusSignal::BusUpd(addr.clone()));
                            self.access_cached(&addr);
                            transition(self, BlockState::Dragon_SharedModified);
                            resolve_in(self, 0);
                        } else {
                            send_bus_tx(self, BusSignal::BusUpd(addr.clone()));
                            self.access_cached(&addr);
                            transition(self, BlockState::Dragon_Modified);
                            resolve_in(self, 0);
                        }
                    },
                    _ => panic!("Cache in invalid state"),
                };
                self.inc_traffic();
            },
            BlockState::Dragon_SharedModified => {
                match &req {
                    PrReq::Write(addr) => {
                        if let Some(true) = others_have_block {
                            send_bus_tx(self, BusSignal::BusUpd(addr.clone()));
                            self.access_cached(&addr);
                            resolve_in(self, 0);
                        } else {
                            send_bus_tx(self, BusSignal::BusUpd(addr.clone()));
                            self.access_cached(&addr);
                            transition(self, BlockState::Dragon_Modified);
                            resolve_in(self, 0);
                        }
                    },
                    _ => panic!("Cache in invalid state"),
                };
                self.inc_traffic();
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

        // state machine
        match self.state_of(&addr) {
            BlockState::MESI_Invalid => {},
            BlockState::MESI_Shared => {
                match sig {
                    BusSignal::BusRd(addr) => {},
                    BusSignal::BusRdX(addr) => {
                        transition(self, BlockState::MESI_Invalid);
                    },
                    _ => {},
                }
            },
            BlockState::MESI_Exclusive => {
                match sig {
                    BusSignal::BusRd(addr) => {
                        transition(self, BlockState::MESI_Shared);
                    },
                    BusSignal::BusRdX(addr) => {
                        // need to flush
                        acquire_bus(self);
                    },
                    _ => {},
                }
            },
            BlockState::MESI_Modified => {
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

            BlockState::Dragon_SharedModified => {
                match sig {
                    // BusRd requires us to deliver cache line - we can ignore this here
                    BusSignal::BusUpd(addr) => {
                        transition(self, BlockState::Dragon_SharedClean);
                    },
                    _ => {},
                }
            },
            BlockState::Dragon_Modified => {
                match sig {
                    BusSignal::BusRd(addr) => {
                        transition(self, BlockState::Dragon_SharedModified);
                    },
                    BusSignal::BusUpd(addr) => {
                        transition(self, BlockState::Dragon_SharedClean);
                    },
                    _ => {},
                }
            },
            _ => {}
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
            BlockState::MESI_Exclusive => {
                match sig {
                    BusSignal::BusRdX(addr) => {
                        transition(self, BlockState::MESI_Invalid);
                        resolve_in(self, self.specs.t_flush() - 1);
                        self.inc_traffic();
                    },
                    _ => panic!("Cache in invalid state"),
                }
            },
            BlockState::MESI_Modified => {
                match sig {
                    BusSignal::BusRd(addr) => {
                        transition(self, BlockState::MESI_Shared);
                        resolve_in(self, self.specs.t_flush() - 1);
                    },
                    BusSignal::BusRdX(addr) => {
                        transition(self, BlockState::MESI_Invalid);
                        resolve_in(self, self.specs.t_flush() - 1);
                    },
                    _ => panic!("Cache in invalid state"),
                }
                self.inc_traffic();
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
                            self.handle_pr_req_bus_locked(
                                req.clone(),
                                others_have_block.clone());
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
                        self.state = CacheState::ResolvingPrReq(
                            req.clone(),
                            Some(*others_have_block));
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

#[derive(Clone, PartialEq, Debug)]
enum BusSignal {
    BusRd(Addr),
    BusRdX(Addr),
    BusUpd(Addr),
}

#[derive(Clone, PartialEq, Debug)]
enum BusState {
    Unlocked_Idle,      // bus free / not locked
    Unlocked_Busy,      // bus free / not locked, busy sending signals signal
    Locked(i32),        // bus is owned by a cache
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

struct Printer{}
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
    pub fn format_row<T>(v: Vec<T>) -> String
        where T: std::fmt::Display
    {
        v.iter().map(|x| format!("{:<15}", x)).collect::<Vec<_>>().join(" | ")
    }
}

// simulator

#[derive(PartialEq, Debug)]
struct SystemState {
    proc_states: Vec<ProcState>,
    cache_states: Vec<CacheState>,
    bus_state: BusState,
}
impl SystemState {
    fn new() -> Self {
        SystemState {
            proc_states: Vec::new(),
            cache_states: Vec::new(),
            bus_state: BusState::Unlocked_Idle,
        }
    }
}

#[derive(Clone)]
enum SimMsg {
    AskOtherCaches(Addr),  // provides interface to check info that requires broad access
}

fn simulate(
    specs: SystemSpec,
    insts: Vec<Instructions>,
    print_states: bool,
    print_cycle_infos: Option<i32>) {

    println!("initializing simulation...");

    let n = insts.len() as i32;

    // each component (processors, caches, bus) communicates to others by sending messages
    // to the simulator via channels which will forward messages to the receiver component

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

    let mut proc_done_cycles = vec![None; n as usize];

    println!("done. starting simulation");

    // simulate
    let mut cycle_count = -1;
    let mut last_printed_cycle_count = 0;
    if print_states { Printer::print_header(&procs, &caches, &bus); }

    loop {
        cycle_count += 1;
        dq.update_time(cycle_count);

        let all_procs_done = procs.iter().all(|p| p.state == ProcState::Done);
        if all_procs_done && dq.is_empty() { break; }

        // tick everyone -- the order does not matter!!
        for proc_id in 0..n {
            match procs[proc_id as usize].state {
                // some optional optimizations
                ProcState::Done => {
                    if proc_done_cycles[proc_id as usize].is_none() {
                        proc_done_cycles[proc_id as usize] = Some(cycle_count);
                    }
                    continue;
                },
                ProcState::WaitingForCache => continue,
                _ => send_msg(Msg::ToProc(proc_id, ProcMsg::Tick)),
            };
        }
        for cache_id in 0..n {
            // some optional optimizations
            match &caches[cache_id as usize].state {
                CacheState::Idle | CacheState::ResolvingPrReq(_, Some(_)) =>
                    send_msg(Msg::ToCache(cache_id, CacheMsg::Tick)),
                _ => continue,
            };
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
                            BlockState::MESI_Invalid | BlockState::Dragon_Invalid => false,
                            _ => true,
                        })
                    });
                    send_msg(Msg::SimToCache(i, CacheMsg::CachesChecked(hit)));
                },
                Msg::SimToCache(i, msg) => caches[i as usize].handle_msg(msg),
            }
            if !dq.msg_available() { dq.update_q() }
        }

        // post-tick everyone -- again, the order does not matter!!
        for proc_id in 0..n {
            procs[proc_id as usize].handle_msg(ProcMsg::PostTick);
        }
        for cache_id in 0..n {
            caches[cache_id as usize].handle_msg(CacheMsg::PostTick);
        }
        bus.handle_msg(BusMsg::PostTick);

        // print runtime infos
        if print_states {
            Printer::print_row(cycle_count, &procs, &caches, &bus);
        } else if let Some(i) = print_cycle_infos {
            if cycle_count - last_printed_cycle_count >= i {
                let instruction_counts =
                    procs.iter().map(|p|
                        format!("{:<15}", p.instructions.len())
                    ).collect::<Vec<_>>();
                let cycle_count_str = {
                    let mut s = String::new();
                    for (i, c) in format!("{}", cycle_count).chars().rev().enumerate() {
                        if i>0 && i %3 == 0 { s.push('-'); }
                        s.push(c);
                    }
                    s.chars().rev().collect::<String>()
                };
                println!("{}\t\t{}", cycle_count_str, instruction_counts.join("\t"));
                last_printed_cycle_count = cycle_count;
            }
        }

    }

    // print stats

    let proc_done_cycles_cleaned = proc_done_cycles.iter()
        .map(|x|
            x.unwrap_or(cycle_count)
        ).collect::<Vec<_>>();

    println!("done! detailed stats:");
    println!("total cycles: {}", cycle_count);
    println!("detailed stats\n{}", Printer::format_row(
        (0..n).map(|i| format!("core {}", i)).collect::<Vec<_>>()
    ));
    println!("{:}\t\t cycles per core", Printer::format_row(
        proc_done_cycles_cleaned
    ));
    println!("{:}\t\t load instructions", Printer::format_row(
        procs.iter().map(|p| p.num_loads).collect::<Vec<_>>()
    ));
    println!("{:}\t\t store instructions", Printer::format_row(
        procs.iter().map(|p| p.num_stores).collect::<Vec<_>>()
    ));
    println!("{:}\t\t wait cycles", Printer::format_row(
        procs.iter().map(|p| p.num_wait_cycles).collect::<Vec<_>>()
    ));
    println!("{:}\t\t miss rate", Printer::format_row(
        caches.iter().map(|c| c.num_misses as f32 / (c.num_misses + c.num_hits) as f32)
            .collect::<Vec<_>>()
    ));
    println!("{:}\t\t private access rate", Printer::format_row(
        caches.iter().map(|c|
            c.num_private_accesses as f32 / (c.num_private_accesses + c.num_shared_accesses) as f32)
            .collect::<Vec<_>>()
    ));
    println!("{:}\t\t invalidations", Printer::format_row(
        caches.iter().map(|c| c.num_invalidations).collect::<Vec<_>>()
    ));
    println!("{:}\t\t issued bus traffic [bytes]", Printer::format_row(
        caches.iter().map(|c| c.amt_issued_bus_traffic).collect::<Vec<_>>()
    ));
}


fn read_testfiles(testname: &str) -> Vec<Instructions> {
    // reads all files that begin with testname from the tests directory
    // and returns a vector of instructions for each file
    // the order is currently undefined
    let mut insts = Vec::new();
    let paths = fs::read_dir("../tests/").unwrap();
    // iterate all files that start with `testname`
    for path in paths.filter_map(|p| p.ok()).filter(|p| {
        p.file_name().to_str().unwrap().starts_with(testname) &&
            p.file_name().to_str().unwrap().ends_with(".data")
    }) {
        println!("reading file: {:?}", path.file_name());
        let mut f = File::open(path.path()).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        let mut insts_for_proc = VecDeque::new();
        for line in s.lines() {
            let mut parts = line.split_whitespace();
            let inst = parts.next().unwrap().parse::<i32>().unwrap();
            let val = i32::from_str_radix(
                parts.next().unwrap().trim_start_matches("0x"),
                16).unwrap();
            insts_for_proc.push_back(match inst {
                0 => Instr::Read(Addr(val)),
                1 => Instr::Write(Addr(val)),
                2 => Instr::Other(val),
                _ => panic!("invalid instruction"),
            });
        }
        insts.push(insts_for_proc);
    }
    println!("done");
    insts
}


fn main() {
    let args: Vec<String> = env::args().collect();
    let specs;
    let testname;

    if args.len() > 1 {
        specs = SystemSpec {
            protocol: match args[1].as_str() {
                "MESI" => Protocol::MESI,
                "Dragon" => Protocol::Dragon,
                _ => panic!("invalid protocol argument"),
            },
            cache_size: args[3].parse().unwrap(),
            cache_assoc: args[4].parse().unwrap(),
            block_size: args[5].parse().unwrap(),
            ..Default::default()
        };
        testname = args[2].as_str();
    } else {
        specs = SystemSpec { ..Default::default() };
        testname = "custom";
    }

    let t0 = Instant::now();
    simulate(
        specs,
        read_testfiles(testname),
        false,
        Some(400000),
    );
    let t1 = Instant::now();
    println!("execution time {:?}", t1-t0);
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