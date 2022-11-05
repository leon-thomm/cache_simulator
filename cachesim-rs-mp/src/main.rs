use std::sync::mpsc;

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
}

// addresses

#[derive(Clone)]
struct Addr(i32);

impl Addr {
    fn new(addr: i32) -> Addr {
        Addr(addr)
    }
    fn pos(&self, specs: &SystemSpec) -> (i32, i32) {
        // returns the index and tag and index of the address under given system specs
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

// instructions

#[derive(Clone)]
enum Instr {
    Read(Addr),
    Write(Addr),
    Other(i32),
}

type Instructions = Vec<Instr>;

// processors

enum ProcState {
    Idle,
    ExecutingOther(i32),
    WaitingForCache,
    Done,
}

struct Processor {
    id: i32,
    state: ProcState,
    instructions: Instructions,
    tx: mpsc::Sender<Msg>,
}

impl Processor {
    fn new(id: i32, instructions: Instructions, tx: mpsc::Sender<Msg>) -> Self {
        Processor {
            id,
            state: ProcState::Idle,
            instructions,
            tx,
        }
    }
    fn tick(&mut self) {
        todo!()
    }
    fn handle_msg(&mut self, msg: ProcMsg) {
        todo!()
    }
}

// caches

enum CacheState {
    Idle,
    ResolvingRequest(i32),
    WaitingForBus,
}

struct Cache {
    id: i32,
    state: CacheState,
    tx: mpsc::Sender<Msg>,
}

impl Cache {
    fn new(id: i32, tx: mpsc::Sender<Msg>) -> Self {
        Cache {
            id,
            state: CacheState::Idle,
            tx,
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

enum BusState {
    Idle,
    Busy(i32, i32),
}

struct Bus {
    state: BusState,
    tx: mpsc::Sender<Msg>,
    n: i32,
}

impl Bus {
    fn new(n: i32, tx: mpsc::Sender<Msg>) -> Self {
        Bus {
            state: BusState::Idle,
            tx,
            n
        }
    }
    fn tick(&mut self) {
        todo!()
    }
    fn handle_msg(&mut self, cache_id: i32, msg: BusMsg) {
        todo!()
    }
}

// simulator

fn simulate(insts: Vec<Instructions>) {


    let n = insts.len() as i32;

    // each component (processors, caches, bus) communicates to others by sending messages
    // to the simulator (main thread) via channels which will forward messages to the
    // intended recipient

    // implement everything single-threaded for now

    let (tx, rx) = mpsc::channel();

    let mut procs = (0..n).map(|i| {
        Processor::new(i, insts[i as usize].clone(), tx.clone())
    }).collect::<Vec<_>>();

    let mut caches = (0..n).map(|i| {
        Cache::new(i, tx.clone())
    }).collect::<Vec<_>>();

    let mut bus = Bus::new(n, tx.clone());

    // simulate
    let mut cycle_count = 0;
    loop {
        // tick everyone -- THE ORDER SHOULD NOT MATTER!!
        for i in 0..n {
            tx.send(Msg::TickProc(i)).unwrap();
            tx.send(Msg::TickCache(i)).unwrap();
        }
        tx.send(Msg::TickBus).unwrap();

        for i in 0..n as usize {
            procs[i].tick();
            caches[i].tick();
        }
        bus.tick();

        let mut msg_received = false;
        while let Ok(msg) = rx.try_recv() {
            msg_received = true;
            match msg {
                Msg::ProcToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::CacheToProc(i, msg) => procs[i as usize].handle_msg(msg),
                Msg::CacheToBus(i, msg) => bus.handle_msg(i, msg),
                Msg::BusToCache(i, msg) => caches[i as usize].handle_msg(msg),
                Msg::TickProc(i) => procs[i as usize].tick(),
                Msg::TickCache(i) => caches[i as usize].tick(),
                Msg::TickBus => bus.tick(),
            }
        }
        cycle_count += 1;

        if !msg_received { break; }
    }
    println!("cycles: {}", cycle_count);
}


fn main() {
    // let (tx, rx) = mpsc::channel();

    // tx.send(MessageType::Msg("Hello".to_string())).unwrap();
    // tx.send(MessageType::Msg("World".to_string())).unwrap();

    // loop {
    //     match rx.recv() {
    //         Ok(MessageType::Msg(msg)) => println!("{}", msg),
    //         Ok(MessageType::Quit) => break,
    //         Err(_) => break,
    //     }
    // }
}
