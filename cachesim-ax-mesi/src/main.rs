use asynchronix::simulation::{Mailbox, Address, SimInit, Simulation};
use asynchronix::time::MonotonicTime;
use log::{info, warn};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, Arc};
use std::time::Duration;
mod MESI;
use MESI::*;

#[macro_use]
extern crate log;

use env_logger::Env;

const SYSTEM: SystemSpec = SystemSpec {
    word_size: 4,
    address_size: 4,
    mem_lat: 100,
    bus_word_tf_lat: 2,
    block_size: 32,
    cache_size: 4096,
    cache_assoc: 2,
};
const NUM_PROCS: usize = 4;
const CACHE_SIZE: usize = 4096;   // must match SYSTEM.cache_size
const CACHE_ASSOC: usize = 2;     // must match SYSTEM.cache_assoc

fn main() {
    // logging
    let env = Env::default()
        .filter_or("MY_LOG_LEVEL", "trace")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);

    // load instructions
    let mut insts = vec![
        vec![Instr::Read(Addr(0)), Instr::Read(Addr(0))],
        vec![Instr::Read(Addr(0)), Instr::Read(Addr(0))],
        vec![Instr::Read(Addr(0)), Instr::Read(Addr(0))],
        vec![Instr::Read(Addr(0)), Instr::Read(Addr(0))],
    ];

    let mut done = (0..NUM_PROCS).map(|_| Arc::new(AtomicBool::new(false))).collect::<Vec<_>>();

    // create models
    let mut procs = (0u32..NUM_PROCS as u32).map(|i| {
        Processor::new(i, insts.remove(0), done[i as usize].clone())
    }).collect::<Vec<_>>();
    let mut caches = (0u32..NUM_PROCS as u32).map(|i| 
        Cache::<CACHE_SIZE, CACHE_ASSOC, NUM_PROCS>::new(i, SYSTEM.clone())
    ).collect::<Vec<_>>();
    let mut bus = Bus::new(SYSTEM.clone());
    
    // create mailboxes
    let mut procs_mbox = procs.iter().map(|p| 
        Mailbox::<Processor>::new()
    ).collect::<Vec<_>>();
    let mut caches_mbox = caches.iter().map(|c| 
        Mailbox::<Cache<CACHE_SIZE, CACHE_ASSOC, NUM_PROCS>>::new()
    ).collect::<Vec<_>>();
    let mut bus_mbox = Mailbox::<Bus::<NUM_PROCS>>::new();

    // addresses
    let tick_addr_bus = bus_mbox.address();
    let tick_addr_procs = procs_mbox.iter().map(|mb| mb.address()).collect::<Vec<_>>();
    let tick_addr_caches = caches_mbox.iter().map(|mb| mb.address()).collect::<Vec<_>>();

    // connect models
    for i in 0..NUM_PROCS {
        procs[i].o_cache_req.connect(Cache::on_proc_req, &caches_mbox[i]);
        caches[i].o_proc_resp.connect(Processor::on_cache_resp, &procs_mbox[i]);
        caches[i].o_bus_sig.connect(Bus::on_bus_sig, &bus_mbox);
        caches[i].o_bus_acq.connect(Bus::on_acquire, &bus_mbox);
        bus.o_bus_sig.get_mut(&(i as u32)).unwrap().connect(Cache::on_bus_sig, &caches_mbox[i]);
        bus.o_bus_acq.get_mut(&(i as u32)).unwrap().connect(Cache::on_bus_locked, &caches_mbox[i]);
        for j in 0..NUM_PROCS {
            if i != j {
                caches[i].r_cache.connect(Cache::on_cache_req, &caches_mbox[j]);
            }
        }
    }

    // initialize simulation
    let mut simi = SimInit::new()
        .add_model(bus, bus_mbox);
    for i in 0..NUM_PROCS {
        simi = simi.add_model(procs.remove(0), procs_mbox.remove(0));
        simi = simi.add_model(caches.remove(0), caches_mbox.remove(0));
    }
    let mut sim = simi.init(MonotonicTime::EPOCH);

    // run simulation
    for i in 0..10000 {
        if done.iter().all(|d| d.load(std::sync::atomic::Ordering::Relaxed)) { break; }

        // tick
        sim.send_event(Bus::on_tick, (), &tick_addr_bus);
        for j in 0..NUM_PROCS {
            sim.send_event(Processor::on_tick, (), &tick_addr_procs[j]);
            sim.send_event(Cache::on_tick, (), &tick_addr_caches[j]);
        }

        sim.step_by(Duration::from_secs(1));
        
        // post-tick
        sim.send_event(Bus::on_post_tick, (), &tick_addr_bus);
        for j in 0..NUM_PROCS {
            sim.send_event(Processor::on_post_tick, (), &tick_addr_procs[j]);
            sim.send_event(Cache::on_post_tick, (), &tick_addr_caches[j]);
        }

        sim.step();
    }

    // print stats
    println!("finished simulation in {} cycles", sim.time().as_secs());
}
