use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use log::info;
use asynchronix::model::{Model, Output};
use asynchronix::time::Scheduler;

use super::common::*;

pub enum ProcState {
    Idle,
    Done,
    ExecutingOther,
    WaitingForCache,
    ContinueNext,
}

impl Default for ProcState {
    fn default() -> Self { ProcState::Idle }
}

pub struct Processor {
    pub id: u32,
    pub state: ProcState,
    pub o_cache_req: Output<ProcCacheReq>,
    pub insts: Insts,
    done: Arc<AtomicBool>,
}

impl Processor {
    pub fn new(id: u32, insts: Insts, done: Arc<AtomicBool>) -> Self {
        Processor {
            id,
            state: ProcState::Idle,
            o_cache_req: Output::new(),
            insts,
            done,
        }
    }
    async fn send_cache_req(&mut self, msg: ProcCacheReq) {
        info!("sending cache request");
        self.o_cache_req.send(msg).await;
    }
    async fn continue_next(&mut self) {
        self.state = ProcState::ContinueNext;
    }
    pub async fn on_tick(&mut self, _: (), scheduler: &Scheduler<Self>) {
        let coninue_in = |d: u32| {
            scheduler.schedule_event(scheduler.time()+Duration::from_secs(d.into()), Self::continue_next, ()).unwrap();
        };
        match self.state {
            ProcState::Idle => {
                info!("fetching instruction");
                match self.insts.pop().expect("no more instructions") {
                    Instr::Read(addr) => {
                        self.send_cache_req(ProcCacheReq::Read(addr));
                        self.state = ProcState::WaitingForCache;
                    },
                    Instr::Write(addr) => {
                        self.send_cache_req(ProcCacheReq::Write(addr));
                        self.state = ProcState::WaitingForCache;
                    },
                    Instr::Other(d) => {
                        self.state = ProcState::ExecutingOther;
                        if d-1 > 0 { coninue_in(d-1); }
                        else { self.state = ProcState::ContinueNext; }
                    },
                }
            },
            _ => (),
        }
    }
    pub async fn on_post_tick(&mut self) {
        match self.state {
            ProcState::ExecutingOther | ProcState::ContinueNext => {
                self.state = if self.insts.is_empty() {
                    self.done.store(true, std::sync::atomic::Ordering::Relaxed);
                    ProcState::Done
                } else {
                    ProcState::Idle
                }
            },
            _ => (),
        }
    }
    pub async fn on_cache_resp(&mut self, _: CacheProcResp) {
        info!("cache response received");
        self.state = ProcState::ContinueNext;
    }
}

impl Model for Processor {}