use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::cmp::max;
use std::ops::Deref;
use std::rc::Rc;

mod processor;
mod cache;
mod bus;

use processor::*;
use cache::*;


// address is 32 bit
struct Address(i32);


enum Protocol {
    MESI,
    Dragon,
}

// Notice, a cache responds to a processor request after it has been processed,
// whereas a response to the bus is immediate.

/// L1 cache simulator
/// the cache is:
///  - write-back
///  - write-allocate
///  - LRU
/// cache size is unknown at compile time
/// cache size is a power of 2
/// the cache is owned by the processor
/// all caches together own the bus
struct L1 {
    protocol: Protocol,
    proc_response: CacheProcResponse,
    data: Vec<CacheSet>,
    hit_count: i32,
    miss_count: i32,
    state: CacheState,
    snooper: Rc<RefCell<Snooper>>,
}

// impl Cache for L1 {
//     fn processor_request(&mut self, s: ProcessorSignal) {
//         match self.state.data {
//             CacheStateData::Ready => {
//                 match s {
//                     ProcessorSignal::Read(addr) => {
//                         // if hit, respond to processor
//                         // if miss, ask on bus
//
//                     }
//                 }
//             },
//             _ => panic!("processor request while cache is not ready"),
//         }
//     }
//
//     fn get_proc_response(&self) -> &Option<CacheProcResponseMsg> {
//         self.proc_response.get()
//     }
//
//     fn bus_transaction(&mut self, t: BusTransaction) -> CacheBusResponse {
//         todo!()
//     }
// }

impl L1 {
    fn new(protocol: Protocol, bus: Rc<RefCell<bus>>, cache_size: i32, assoc: i32, block_size: i32) -> L1 {
        L1 {
            protocol,
            proc_response: CacheProcResponse::new(),
            data: vec![CacheSet::new(assoc, block_size); cache_size as usize],
            hit_count: 0,
            miss_count: 0,
            state: CacheDataState,
            snooper: Rc::new(RefCell::new(Snooper::new(bus))),
        }
    }

    fn tick(&mut self) {
        self.state = match self.state {
            _ => todo!()
        }
    }
}

enum CacheDataState {
    Idle,
    WaitingForBus,
    LoadingFromMemory(i32),
    WritingToMemory(i32),
}

struct CacheSnooper {
    bus: Rc<RefCell<Bus>>,
    state: CacheSnooperState,
}

impl CacheSnooper {
    
}

enum CacheSnooperState {
    Snooping,
    Sending(i32),
}

// -------------------------------------------------------------------

// associativity is unknown
struct CacheSet(Vec<CacheLine>);

impl CacheSet {
    fn new(assoc: i32, block_size: i32) -> CacheSet {
        CacheSet(vec![CacheLine::new(block_size); assoc as usize])
    }
}

struct CacheLine {
    tag: Address,
    state: CacheLineState,
    data: Block,
}

impl CacheLine {
    fn new(block_size: i32) -> CacheLine {
        CacheLine {
            tag: Address(-1),
            state: CacheLineState::Invalid,
            data: Block::new(block_size),
        }
    }
}

enum CacheLineState {
    MESI(MESIState),
    Dragon(DragonState),
}

enum MESIState {
    Modified,
    Exclusive,
    Shared,
    Invalid,
}

enum DragonState {
    Exclusive,
    SharedClean,
    SharedModified,
    Dirty,
}

// block size is unknown
struct Block(Vec<Word>);

impl Block {
    fn new(block_size: i32) -> Block {
        Block(vec![Word(0); block_size as usize])
    }
}

// word size is 32 bit
struct Word(i32);

// -------------------------------------------------------------------





fn simulate(p0: processor, p1: processor, p2: processor, p3: processor, b: bus) {
    // all processors are already initialized and ready to start executing instructions

    /*
    unoptimized version:
        while not every processor has finished
            tick bus
            tick all processors

    optimized version:
        while not every processor has finished
            get smallest waiting count from processors
            forward all processors and the bus by that min waiting time
            tick each processor
     */
}

fn main() {
    println!("Hello, world!");
}
