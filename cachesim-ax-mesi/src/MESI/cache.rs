use std::time::Duration;
use asynchronix::model::{Model, Output, Requestor};
use asynchronix::time::Scheduler;
use super::common::*;

// data cache

#[derive(Default, Clone, Copy)]
enum BlockState {
    #[default]
    Invalid,
    Shared,
    Exclusive,
    Modified,
}

#[derive(Clone, Copy)]
struct CacheSet<const ASSOC: usize> {
    //        tag   last used   block state
    blocks: [(u32,  u32,        BlockState); ASSOC],
    specs: SystemSpec,
    mru_ctr: u32,
}

impl<const ASSOC: usize> CacheSet<ASSOC> {
    pub fn new(specs: SystemSpec) -> Self {
        let mut blocks = [(0u32,0u32,BlockState::default()); ASSOC];
        for (i, b) in blocks.iter_mut().enumerate() {
            *b = (i as u32, 0u32, BlockState::default());
        }
        Self {blocks, specs, mru_ctr: 0}
    }
    pub fn get(&self, tag: &u32) -> Option<&(u32, u32, BlockState)> {
        self.blocks.iter().find(|(t,_,_)| t==tag)
    }
    pub fn set_state(&mut self, tag: &u32, state: BlockState) {
        let (_, last_used, _) = self.blocks.iter_mut().find(|(t,_,_)| t==tag).unwrap();
        *last_used = 0;
    }
    pub fn set_last_used(&mut self, tag: &u32, last_used: &u32) {
        let (_, last_used, _) = self.blocks.iter_mut().find(|(t,_,_)| t==tag).unwrap();
        *last_used = *last_used;
    }
    pub fn touch(&mut self, addr: &Addr) {
        let (index, tag) = addr.pos(&self.specs);
        let (_, last_used, _) = self.blocks.iter_mut().find(|(t,_,_)| *t==tag).unwrap();
        self.mru_ctr += 1;
        *last_used = self.mru_ctr;
    }
    pub fn replace_lru(&mut self, addr: &Addr, state: BlockState) {
        let (index, tag) = addr.pos(&self.specs);
        let lru_index = self.blocks
            .iter().enumerate()
            .min_by_key(|(_, (_, last_used, _))| last_used)
            .map(|(i, _)| i).unwrap();
        self.mru_ctr += 1;
        self.blocks[lru_index] = (tag, self.mru_ctr, state);
    }
}

// cache

#[derive(Clone)]
enum Req {
    Bus(BusSignal),
    Proc(ProcCacheReq),
}

enum CacheState {
    Idle,
    WaitingForBus(Req),             // waiting to aquire bus
    // ResolvingReq(Req),              // resolving a request
    AskingCaches(Req),              // for cache-to-cache transfer
    RequestResolvedProceedNext,     // request completed
}

pub struct Cache<const SIZE: usize, const ASSOC: usize, const NUM_CACHES: usize> {
    pub id: u32,
    state: CacheState,
    specs: SystemSpec,
    data: [CacheSet<ASSOC>; SIZE],
    pub o_proc_resp: Output<CacheProcResp>,
    pub o_bus_sig: Output<(u32, BusSignal)>,
    pub o_bus_acq: Output<u32>,
    pub r_cache: Requestor<CacheToCacheReq, bool>,
}

impl<const SIZE: usize, const ASSOC: usize, const NUM_CACHES: usize> Cache<SIZE, ASSOC, NUM_CACHES> {
    pub fn new(id: u32, specs: SystemSpec) -> Self {
        Self {
            id,
            state: CacheState::Idle,
            specs,
            data: [CacheSet::<ASSOC>::new(specs.clone()); SIZE],
            o_proc_resp: Output::new(),
            o_bus_sig: Output::new(),
            o_bus_acq: Output::new(),
            r_cache: Requestor::new(),
        }
    }

    // helper functions
    fn set_and_tag_of(&self, addr: &Addr) -> (&CacheSet<ASSOC>, u32) {
        let (index, tag) = addr.pos(&self.specs);
        (&self.data[index as usize], tag)
    }
    fn set_and_tag_of_mut(&mut self, addr: &Addr) -> (&mut CacheSet<ASSOC>, u32) {
        let (index, tag) = addr.pos(&self.specs);
        (&mut self.data[index as usize], tag)
    }
    fn state_of(&self, addr: &Addr) -> BlockState {
        let (set, tag) = self.set_and_tag_of(addr);
        set.get(&tag).unwrap().2
    }
    fn set_state_of(&mut self, addr: &Addr, state: BlockState) {
        let (set, tag) = self.set_and_tag_of_mut(addr);
        set.set_state(&tag, state);
    }
    fn access_causes_flush(&self, addr: &Addr) -> bool {
        let (set, tag) = self.set_and_tag_of(addr);
        let set_is_full = set.blocks.len() == self.specs.cache_assoc as usize;
        match self.state_of(addr) {
            BlockState::Invalid => set_is_full,
            _ => false
        }
    }
    fn read_cached(&mut self, addr: &Addr) {
        let (set, tag) = self.set_and_tag_of_mut(addr);
        set.touch(addr);
    }
    fn write_cached(&mut self, addr: &Addr) {
        let (set, tag) = self.set_and_tag_of_mut(addr);
        set.set_state(&tag, BlockState::Modified);
        set.touch(addr);
    }
    fn write_uncached(&mut self, addr: &Addr) {
        let (set, tag) = self.set_and_tag_of_mut(addr);
        set.replace_lru(addr, BlockState::Modified);    
    }

    // inputs

    pub async fn on_tick(&mut self, _:(), scheduler: &Scheduler<Self>) {}
    pub async fn on_post_tick(&mut self) {
        match self.state {
            CacheState::RequestResolvedProceedNext => {
                self.state = CacheState::Idle;
            },
            _ => (),
        }
    }
    pub async fn on_proc_req(&mut self, req: ProcCacheReq, scheduler: &Scheduler<Self>) {
        info!("received proc request");
        let addr = match &req {
            ProcCacheReq::Read(addr) | ProcCacheReq::Write(addr) => addr.clone(),
        };
        let req_ = req.clone();
        let acquire_bus = |self_: &mut Self| {
            self_.state = CacheState::WaitingForBus(Req::Proc(req_));
            self_.o_bus_acq.send(self_.id);
        };
        let send_bus_sig = |self_: &mut Self, sig: BusSignal| {
            self_.o_bus_sig.send((self_.id, sig));
        };
        let proceed_next = |self_: &mut Self| {
            self_.state = CacheState::RequestResolvedProceedNext;
            self_.o_proc_resp.send(CacheProcResp::RequestResolved);
        };
        match self.state_of(&addr) {
            BlockState::Invalid => {
                match req {
                    ProcCacheReq::Read(_) => acquire_bus(self),  // read miss
                    ProcCacheReq::Write(_) => { // write miss
                        if self.access_causes_flush(&addr) {
                            acquire_bus(self);  // replace (flush)
                        } else {
                            send_bus_sig(self, BusSignal::BusRdX(addr.clone()));  // upgrade
                            self.write_uncached(&addr);
                            proceed_next(self);
                        }
                    }
                }
            },
            BlockState::Shared => {
                match req {
                    ProcCacheReq::Read(_) => {  // read hit
                        self.read_cached(&addr);
                        proceed_next(self);
                    },
                    ProcCacheReq::Write(_) => { // write hit
                        send_bus_sig(self, BusSignal::BusRdX(addr.clone()));
                        self.write_cached(&addr);
                        proceed_next(self);
                    },
                }
            },
            BlockState::Exclusive => {
                match req {
                    ProcCacheReq::Read(_) => {  // read hit
                        self.read_cached(&addr);
                        proceed_next(self);
                    },
                    ProcCacheReq::Write(_) => { // write hit
                        self.write_cached(&addr);
                        proceed_next(self);
                    },
                }
            },
            BlockState::Modified => {
                match req {
                    ProcCacheReq::Read(_) => {  // read hit
                        self.read_cached(&addr);
                        proceed_next(self);
                    },
                    ProcCacheReq::Write(_) => { // write hit
                        self.write_cached(&addr);
                        proceed_next(self);
                    },
                }
            },
        }
    }
    pub async fn on_bus_sig(&mut self, sig: BusSignal, scheduler: &Scheduler<Self>) {
        // bus signals are answered immediately
        // we cannot receive bus signals when we currently own the bus lock
        let addr = match &sig {
            BusSignal::BusRd(addr) | BusSignal::BusRdX(addr) | BusSignal::BusUpd(addr) => addr,
        };
        let transition = |self_: &mut Self, s: BlockState| {
            self_.set_state_of(&addr, s);
        };
        let acquire_bus = |self_: &mut Self| {
            self_.state = CacheState::WaitingForBus(Req::Bus(sig.clone()));
            self_.o_bus_acq.send(self_.id);
        };
        match self.state_of(addr) {
            BlockState::Shared => {
                match sig {
                    BusSignal::BusRdX(_) => transition(self, BlockState::Invalid),      // invalidate block
                    BusSignal::BusUpd(_) => { /* implicitly replace data */},
                    _ => (),
                }
            },
            BlockState::Exclusive => {
                match sig {
                    BusSignal::BusRd(_) => transition(self, BlockState::Shared),        // downgrade to shared
                    BusSignal::BusRdX(_) => acquire_bus(self),                          // flush
                    _ => (),
                }
            },
            BlockState::Modified => {
                match sig {
                    BusSignal::BusRd(_) | BusSignal::BusRdX(_) => acquire_bus(self),    // flush
                    _ => ()
                }
            },
            _ => (),
        }
    }
    pub async fn on_bus_locked(&mut self, _: (), scheduler: &Scheduler<Self>) {
        let req = match &self.state {
            CacheState::WaitingForBus(req) => req.clone(),
            _ => panic!("cache not waiting for bus"),
        };
        let flush = |self_: &mut Self| {
            let t = timing::flush(&self_.specs);
            scheduler.schedule_event(
                scheduler.time()+Duration::from_secs((t-1).into()), 
                Self::on_flush_done, ()).unwrap();
        };
        let addr = match &req {
            Req::Bus(sig) => match sig {
                BusSignal::BusRd(addr) | BusSignal::BusRdX(addr) | BusSignal::BusUpd(addr) => addr.clone(),
            },
            Req::Proc(req) => match req {
                ProcCacheReq::Read(addr) | ProcCacheReq::Write(addr) => addr.clone(),
            },
        };

        if let Req::Bus(sig) = req {
            // this can only mean flush due to read or readX by another cache
            match sig {
                BusSignal::BusRd(_) => {
                    // downgrade to shared
                    self.set_state_of(&addr, BlockState::Shared);
                    flush(self);
                },
                BusSignal::BusRdX(_) => {
                    // flush
                    self.set_state_of(&addr, BlockState::Invalid);
                    flush(self);
                },
                _ => (),
            }
        } else if let Req::Proc(req) = req {
            match req {
                ProcCacheReq::Read(_) => {  // processor read miss
                    self.state = CacheState::AskingCaches(Req::Proc(req));
                    let block_cached_somewhere = self.r_cache
                        .send(CacheToCacheReq::CheckAddr(addr.clone())).await
                        .any(|b| b);
                    // calculate time until block is loaded
                    let mut t = timing::c2c_msg(&self.specs);
                    if block_cached_somewhere {
                        self.o_bus_sig.send((self.id, BusSignal::BusRd(addr.clone())));
                        t += timing::c2c_transfer(&self.specs);
                    } else {
                        // no need to send bus signal if no other cache holds the block
                        t += timing::mem_fetch(&self.specs);
                    };
                    if self.access_causes_flush(&addr) {
                        t += timing::flush(&self.specs);
                    }
                    scheduler.schedule_event(
                        scheduler.time()+Duration::from_secs((t-1).into()), 
                        Self::on_block_loaded, 
                        (addr, block_cached_somewhere)
                    ).unwrap();
                },
                ProcCacheReq::Write(_) => { // processor write miss with eviction
                    self.o_bus_sig.send((self.id, BusSignal::BusRdX(addr.clone())));
                    self.write_uncached(&addr);
                    flush(self);
                },
            }
        } else {
            panic!("cache not waiting for bus");
        }
    }
    pub async fn on_cache_req(&mut self, req: CacheToCacheReq) -> bool {
        let addr = match req {
            CacheToCacheReq::CheckAddr(addr) => addr,
        };
        let (set, tag) = self.set_and_tag_of(&addr);
        set.get(&tag).is_some()
    }
    pub async fn on_block_loaded(&mut self, (addr, c2c): (Addr, bool)) {
        // - bus is locked
        // - block has been loaded from another cache or from memory
        // - if we had to flush, this has already happened
        let (set, tag) = self.set_and_tag_of_mut(&addr);
        if c2c { set.set_state(&tag, BlockState::Shared);
        } else { set.set_state(&tag, BlockState::Exclusive); }
        self.state = CacheState::RequestResolvedProceedNext;
    }
    pub async fn on_flush_done(&mut self) {
        self.state = CacheState::RequestResolvedProceedNext;
    }
}

impl<const SIZE: usize, const ASSOC: usize, const NUM_CACHES: usize> Model for Cache<SIZE, ASSOC, NUM_CACHES> {}