// Based on the Processor, Cache, and Bus traits, this module defines wrappers
// which turn them into asynchronix models.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use super::interface::*;

use asynchronix::model::{Model, Output};
use asynchronix::time::Scheduler;

// Processor model
// notice Rust currently doesn't support async trait methods
// there's #[async_trait] but that doesn't work with asynchronix
pub struct ProcModel<S: Signals, P: Processor<S>>{
    pub proc: Box<P>,
    pub o_cache: Output<S::PCSig>,
    pub pcq: SignalQ<S::PCSig>, // cache request queue
    pub on_done_set: Option<Arc<Mutex<bool>>>,
}

impl<S: Signals, P: Processor<S>> Model for ProcModel<S, P> {}

impl<S: Signals, P: Processor<S>> ProcModel<S, P>
{
    pub fn new(proc: Box<P>) -> Self {
        Self {
            proc,
            o_cache: Output::new(),
            on_done_set: None,
            pcq: SignalQ::new(),
        }
    }
    async fn send_to_cache(&mut self, sig: S::PCSig) {
        // asynchronix currently doesn't support output scheduling
        // so we have to schedule ourselves and then send the signal
        self.o_cache.send(sig).await;
    }
    fn schedule_cache_requests(&mut self, scheduler: &Scheduler<Self>) {
        while let Some((sig, delay)) = self.pcq.pop_front() {
            let _ = scheduler.schedule_event(
                Duration::from_secs(delay as u64),
                Self::send_to_cache,
                sig
            );
        }
    }
    // clock
    pub async fn on_tick(&mut self, _:(), scheduler: &Scheduler<Self>) {
        self.proc.on_tick(&mut self.pcq);
        // schedule cache requests
        self.schedule_cache_requests(scheduler)
    }
    pub async fn on_post_tick(&mut self) {
        // invoke processor's on_post_tick
        self.proc.on_post_tick(|| {
            println!("done");
            loop {
                if let Some(done) = &self.on_done_set {
                    *(done.lock().unwrap()) = true;
                    break;
                } else {
                    println!("failed to acquire done flag mutex");
                }
            }
        });
    }
    // signal from associated cache
    pub async fn on_cache_sig(&mut self, sig: S::CPSig, scheduler: &Scheduler<Self>) {
        self.proc.on_cache_sig(sig, &mut self.pcq);
        self.schedule_cache_requests(scheduler);
    }
}


// Cache model
pub struct CacheModel<S: Signals, C: Cache<S>> {
    pub cache: Box<C>,
    pub o_proc: Output<S::CPSig>,
    pub cpq: SignalQ<S::CPSig>, // processor request queue
    pub o_bus: Output<S::CBSig>,
    pub cbq: SignalQ<S::CBSig>, // bus request queue
}

impl<S: Signals, C: Cache<S>> Model for CacheModel<S, C> {}

impl<S: Signals, C: Cache<S>> CacheModel<S, C> {
    pub fn new(cache: Box<C>) -> Self {
        Self {
            cache,
            o_proc: Output::new(),
            cpq: SignalQ::new(),
            o_bus: Output::new(),
            cbq: SignalQ::new(),
        }
    }
    async fn send_to_proc(&mut self, sig: S::CPSig) {
        self.o_proc.send(sig).await;
    }
    async fn send_to_bus(&mut self, sig: S::CBSig) {
        self.o_bus.send(sig).await;
    }
    fn schedule_proc_signals(&mut self, scheduler: &Scheduler<Self>) {
        while let Some((sig, delay)) = self.cpq.pop_front() {
            let _ = scheduler.schedule_event(
                Duration::from_secs(delay as u64),
                Self::send_to_proc,
                sig
            );
        }
    }
    fn schedule_bus_signals(&mut self, scheduler: &Scheduler<Self>) {
        while let Some((sig, delay)) = self.cbq.pop_front() {
            let _ = scheduler.schedule_event(
                Duration::from_secs(delay as u64),
                Self::send_to_bus,
                sig
            );
        }
    }
    // clock
    pub async fn on_tick(&mut self, _: (), scheduler: &Scheduler<Self>) {
        self.cache.on_tick(&mut self.cpq, &mut self.cbq);
        self.schedule_proc_signals(scheduler);
        self.schedule_bus_signals(scheduler);
    }
    pub async fn on_post_tick(&mut self) {
        self.cache.on_post_tick();
    }
    // processor and bus communication
    pub async fn on_proc_sig(&mut self, sig: S::PCSig, scheduler: &Scheduler<Self>) {
        self.cache.on_proc_sig(sig, &mut self.cpq, &mut self.cbq);
        self.schedule_proc_signals(scheduler);
        self.schedule_bus_signals(scheduler);
    }
    pub async fn on_bus_sig(&mut self, sig: S::BCSig, scheduler: &Scheduler<Self>) {
        // invoke cache's on_bus_sig
        self.cache.on_bus_sig(sig, &mut self.cpq, &mut self.cbq);
        self.schedule_proc_signals(scheduler);
        self.schedule_bus_signals(scheduler);
    }
}