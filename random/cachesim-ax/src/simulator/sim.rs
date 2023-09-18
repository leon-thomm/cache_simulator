use super::interface::*;
use super::models::*;

use asynchronix::time::MonotonicTime;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use asynchronix::simulation::{Mailbox, SimInit};

// syntactic sugar; AX_OUT encapsulates all requirements
// for a type to be used as an asynchronix output.
// this capability (AX_OUT) is implemented for all traits
// that implement the requirements (Clone + Send + 'static)
#[allow(non_camel_case_types)]
pub trait AX_OUT: Clone + Send + 'static {}
impl<T: Clone + Send + 'static> AX_OUT for T {}

// a processing unit is a processor (model) and its associated cache (model)
struct ProcUnit<S: Signals, P: Processor<S>, C: Cache<S>> {
    pub id: i32,
    pub proc:  ProcModel<S, P>,
    pub proc_mbox: Mailbox<ProcModel<S, P>>,
    pub cache: CacheModel<S, C>,
    pub cache_mbox: Mailbox<CacheModel<S, C>>,
}

impl<S: Signals, P: Processor<S>, C: Cache<S>> ProcUnit<S, P, C> {
    pub fn new(id: i32, mut proc: ProcModel<S, P>, mut cache: CacheModel<S, C>) -> Self {
        // create mailboxes
        let proc_mbox = Mailbox::new();
        let cache_mbox = Mailbox::new();
        // connect processor with cache
        proc.o_cache.connect(CacheModel::on_proc_sig, &cache_mbox);
        cache.o_proc.connect(ProcModel::on_cache_sig, &proc_mbox);
        // construct unit
        Self {
            id, proc, cache, 
            proc_mbox, cache_mbox,
        }
    }
}

pub fn simulate<S: Signals, P: Processor<S>, C: Cache<S>> 
(
    spec: SystemSpec,
    processors: Vec<P>,
    caches: Vec<C>,
) {
    let n = processors.len();

    // wrap processors in _Processor, wrap caches in _Cache, create mailboxes, connect models
    let mut units: Vec<ProcUnit<S, P,C>> = processors
        .into_iter()
        .zip(caches.into_iter())
        .enumerate()
        .map(|(i, (p, c))| ProcUnit::new(
            i as i32,
            ProcModel::new(Box::new(p)),
            CacheModel::new(Box::new(c))))
        .collect::<Vec<_>>();
    
    // get mailbox addresses
    let proc_addr = units.iter().map(|u| u.proc_mbox.address()).collect::<Vec<_>>();
    let cache_addr = units.iter().map(|u| u.cache_mbox.address()).collect::<Vec<_>>();

    // store done flags
    let mut procs_done = HashMap::new();
    for u in &mut units {
        // store arc holding done flag and pass a handle to it to the processor
        let done = Arc::new(Mutex::new(false));
        procs_done.insert(u.id, done.clone());
        u.proc.on_done_set = Some(done);
    }

    // initialize simulation
    let mut simi = SimInit::new();
    for unit in units.into_iter() {
        simi = simi.add_model(unit.proc, unit.proc_mbox);
        simi = simi.add_model(unit.cache, unit.cache_mbox);
    }
    let mut sim = simi.init(MonotonicTime::EPOCH);

    // run simulation
    let mut cycle_count = 0;
    loop {
        // tick
        for i in 0..n {
            // tick
            sim.send_event(ProcModel::on_tick, (), &proc_addr[i]);
            sim.send_event(CacheModel::on_tick, (), &cache_addr[i]);
        }

        sim.step();

        // post tick
        for i in 0..n {
            sim.send_event(ProcModel::on_post_tick, (), &proc_addr[i]);
            sim.send_event(CacheModel::on_post_tick, (), &cache_addr[i]);
        }

        sim.step();

        cycle_count += 1;

        // check if all processors are done
        let mut all_done = true;
        for (_, done) in &procs_done {
            if !*(done.lock().unwrap()) {
                all_done = false;
                break;
            }
        }
        if all_done { break; }
    }

    println!("simulation done after {} cycles", cycle_count);
}