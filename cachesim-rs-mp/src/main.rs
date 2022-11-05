use std::sync::mpsc;

/*
    A MESI and Dragon cache coherence protocol simulator.
 */

// messages

enum Msg {
    ProcToCache(i32, CacheMsg),
    CacheToProc(i32, ProcMsg),
    CacheToBus(i32, BusMsg),
    BusToCache(i32, CacheMsg),

    TickProc(i32),
    TickCache(i32),
    TickBus,
}

enum ProcMsg {
    Tick,
    RequestResolved,
}

enum CacheMsg {
    Tick,
    Read(i32),
    Write(i32),
    BusSignal(BusSignal),
}

enum BusMsg {
    StayBusy(i32, i32),
    SendSignal(i32, BusSignal),
}

enum BusSignal {
    BusRd(Addr),
    BusRdX(Addr),
}

}

// optional callback receiver for the bus
type OptCb = Option<Component::Cache(i32)>;

enum BusMsg {
    Tick,
    StayBusy(i32, OptCb),
    SendSignal(BusSignal, OptCb, OptCb),
}


fn simulate(insts: Vec<Vec<Instruction>>) {

    let n = insts.len();

    // each component (processors, caches, bus) communicates to others by sending messages
    // to the simulator (main thread) via channels which will forward messages to the
    // intended recipient

    // implement everything single-threaded for now

    let (tx, rx) = mpsc::channel();

    let procs = (0..n).map(|i| {
        Processor::new(i, tx.clone(), insts[i].clone())
    }).collect::<Vec<_>>();

    let caches = (0..n).map(|i| {
        Cache::new(i, tx.clone())
    }).collect::<Vec<_>>();

    let bus = Bus::new(tx.clone());

    // simulate
    let mut cycle_count = 0;
    loop {
        // tick everyone -- THE ORDER SHOULD NOT MATTER!!
        for i in 0..n {
            tx.send(Msg::TickProc(i)).unwrap();
            tx.send(Msg::TickCache(i)).unwrap();
        }
        tx.send(Msg::TickBus).unwrap();

        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::ProcToCache(i, msg) => {
                    caches[i].handle_msg(msg);
                },
                Msg::CacheToProc(i, msg) => {
                    procs[i].handle_msg(msg);
                },
                Msg::CacheToBus(i, msg) => {
                    bus.handle_msg(i, msg);
                },
                Msg::BusToCache(i, msg) => {
                    caches[i].handle_msg(msg);
                },
                Msg::TickProc(i) => {
                    procs[i].tick();
                },
                Msg::TickCache(i) => {
                    caches[i].tick();
                },
                Msg::TickBus => {
                    bus.tick();
                },
            }
        }
        cycle_count += 1;

        // if all procs done, break
        if procs.iter().all(|p| p.done()) {
            break;
        }
    }
    println!("cycles: {}", cycle_count);
}


fn main() {
    let (tx, rx) = mpsc::channel();

    tx.send(MessageType::Msg("Hello".to_string())).unwrap();
    tx.send(MessageType::Msg("World".to_string())).unwrap();

    // loop {
    //     match rx.recv() {
    //         Ok(MessageType::Msg(msg)) => println!("{}", msg),
    //         Ok(MessageType::Quit) => break,
    //         Err(_) => break,
    //     }
    // }
}
