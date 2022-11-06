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
    ReadyToProceed,
    Done,
}

struct Processor<'a> {
    id: i32,
    state: ProcState,
    instructions: Instructions,
    tx: mpsc::Sender<Msg>,
    specs: &'a SystemSpec,
    cache_id: i32,
}

impl<'a> Processor<'a> {
    fn new(id: i32, cache_id: i32, instructions: Instructions, tx: mpsc::Sender<Msg>, specs: &'a SystemSpec) -> Self {
        Processor {
            id,
            state: ProcState::Idle,
            instructions,
            tx,
            specs,
            cache_id,
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

struct Cache<'a> {
    id: i32,
    state: CacheState,
    tx: mpsc::Sender<Msg>,
    specs: &'a SystemSpec,
    proc_id: i32,
    bus_id: i32,
}

impl<'a> Cache<'a> {
    fn new(id: i32, proc_id: i32, bus_id: i32, tx: mpsc::Sender<Msg>, specs: &'a SystemSpec) -> Self {
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

enum BusState {
    Idle,
    Busy(i32, i32),
}

struct Bus<'a> {
    state: BusState,
    tx: mpsc::Sender<Msg>,
    n: i32,
    specs: &'a SystemSpec,
    cache_ids: Vec<i32>,
}

impl<'a> Bus<'a> {
    fn new(n: i32, cache_ids: Vec<i32>, tx: mpsc::Sender<Msg>, specs: &'a SystemSpec) -> Self {
        Bus {
            state: BusState::Idle,
            tx,
            n,
            specs,
            cache_ids
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

fn simulate(specs: SystemSpec, insts: Vec<Instructions>) {


    let n = insts.len() as i32;

    // each component (processors, caches, bus) communicates to others by sending messages
    // to the simulator (main thread) via channels which will forward messages to the
    // intended recipient

    // implement everything single-threaded for now

    let (tx, rx) = mpsc::channel();

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
