use std::collections::{VecDeque, HashMap};
use std::time::Duration;
use asynchronix::model::{Model, Output};
use asynchronix::time::Scheduler;
use super::common::*;

#[derive(PartialEq)]
pub enum BusState {
    Unlocked,
    Busy,
    Locked(u32),
    FreeNext,
}

pub struct Bus<const NUM_CACHES: usize> {
    state: BusState,
    specs: SystemSpec,
    pub o_bus_sig: HashMap<u32, Output<BusSignal>>,
    pub o_bus_acq: HashMap<u32, Output<()>>,
    signal_queue: VecDeque<(BusSignal, u32)>,   // signals have higher priority than explicit locks by caches
    lock_queue: VecDeque<u32>,                  // explicit locks by caches
}

impl<const NUM_CACHES: usize> Bus<NUM_CACHES> {
    pub fn new(specs: SystemSpec) -> Self {
        let mut o_bus_sig = HashMap::new();
        let mut o_bus_acq = HashMap::new();
        for i in 0..NUM_CACHES {
            o_bus_sig.insert(i as u32, Output::new());
            o_bus_acq.insert(i as u32, Output::new());
        }
        Bus {
            state: BusState::Unlocked,
            specs,
            o_bus_sig,
            o_bus_acq,
            signal_queue: VecDeque::new(),
            lock_queue: VecDeque::new(),
        }
    }

    // helper functions

    fn dispatch_bus_sig(&mut self, scheduler: &Scheduler<Self>) {
        assert!(self.state == BusState::Unlocked);
        if let Some((sig, id)) = self.signal_queue.pop_front() {
            let t = timing::c2c_msg(&self.specs);
            self.state = BusState::Busy;
            self.broadcast_in(t-1, (id, sig), scheduler);
            self.free_next_in(t-1, scheduler);
        }
    }
    fn dispatch_lock_req(&mut self, scheduler: &Scheduler<Self>) {
        assert!(self.state == BusState::Unlocked);
        assert!(self.signal_queue.is_empty());
        if let Some(id) = self.lock_queue.pop_front() {
            self.state = BusState::Locked(id);
            self.o_bus_acq
                .get_mut(&id)
                .expect("cache not found")
                .send(());
        }
    }
    fn broadcast_in(&mut self, d: u32, (id, sig): (u32, BusSignal), scheduler: &Scheduler<Self>) {
        scheduler.schedule_event(scheduler.time()+Duration::from_secs(d.into()), Self::_on_broadcast, (id, sig));
    }
    fn free_next_in(&mut self, d: u32, scheduler: &Scheduler<Self>) {
        scheduler.schedule_event(scheduler.time()+Duration::from_secs(d.into()), Self::_on_free_next, ());
    }
    
    //  inputs (internal inputs are prefixed with _)

    async fn _on_broadcast(&mut self, (id, sig): (u32, BusSignal)) {
        let snd = |(_,o)| o;
        let receivers = self.o_bus_sig
            .iter_mut()
            .filter(|(&k,_)|  k!=id)
            .map(snd);
        for out in receivers {
            out.send(sig.clone());
        }
    }
    fn _on_free_next(&mut self) {
        self.state = BusState::FreeNext;
    }
    pub fn on_tick(&mut self, _:(), scheduler: &Scheduler<Self>) {
        match self.state {
            BusState::Unlocked => {
                if !self.signal_queue.is_empty() {
                    self.dispatch_bus_sig(scheduler)
                } else {
                    self.dispatch_lock_req(scheduler)
                }
            },
            _ => (),
        }
    }
    pub fn on_post_tick(&mut self) {
        match self.state {
            BusState::FreeNext => {
                self.state = BusState::Unlocked;
            },
            _ => (),
        }
    }
    pub fn on_bus_sig(&mut self, (id, sig): (u32, BusSignal), scheduler: &Scheduler<Self>) {
        self.signal_queue.push_back((sig, id));
        match self.state {
            BusState::Unlocked => self.dispatch_bus_sig(scheduler),
            _ => (),
        }
    }
    pub fn on_acquire(&mut self, id: u32, scheduler: &Scheduler<Self>) {
        self.lock_queue.push_back(id);
        match self.state {
            BusState::Unlocked => self.dispatch_lock_req(scheduler),
            _ => (),
        }
    }
}

impl<const NUM_CACHES: usize> Model for Bus<NUM_CACHES> {}