use std::collections::VecDeque;

use super::AX_OUT;

pub struct SystemSpec {         // unit         reasonable defaults
    pub word_size: i32,         // bytes        4
    pub address_size: i32,      // bytes        4
    pub mem_lat: i32,           // cpu          100
    pub bus_word_tf_lat: i32,   // cpu          2
    pub block_size: i32,        // bytes        32
    pub cache_size: i32,        // bytes        4096
    pub cache_assoc: i32,       // blocks       2
}

#[derive(Clone)]
pub struct Addr(pub i32);

impl Addr {
    /// get cache index and tag of this address under given system specs
    pub fn pos(&self, specs: &SystemSpec) -> (i32, i32) {
        let num_indices = specs.cache_size / (specs.block_size * specs.cache_assoc);
        let index = self.0 % num_indices;
        let tag = self.0 / num_indices;
        (index, tag)
    }
}

pub enum Instr {
    Read(Addr),
    Write(Addr),
    Other(i32),
}

pub type Insts = Vec<Instr>;

// pub type ProcID = i32;
// pub type CacheID = i32;
pub type Delay = i32;


pub trait Signals: 'static {
    type PCSig: AX_OUT;
    type CPSig: AX_OUT;
    type CBSig: AX_OUT;
    type BCSig: AX_OUT;
}


pub type SignalQ<SigT> = VecDeque<(SigT, Delay)>;


// any type that implements this trait can be used as a processor in the simulator
pub trait Processor<S: Signals>: Send + 'static {
    // type F_D: Fn() -> ();                   // done helper function type
    // type F_C: FnMut(S::PCSig, i32) -> ();   // cache-to-proc helper function type

    // clock
    fn on_tick      (&mut self, send_cache_q: &mut SignalQ<S::PCSig>); // rising clock edge
    fn on_post_tick<F: Fn() -> ()> (&mut self, done: F);       // falling clock edge
    // cache communication
    fn on_cache_sig (&mut self, sig: S::CPSig, send_cache_q: &mut SignalQ<S::PCSig>);         // signal from associated cache
}


pub trait Cache<S: Signals>: Send + 'static {
    // type F_P = dyn FnMut(S::PCSig, i32) -> ();   // cache-to-proc helper function type
    // type F_B: FnMut(S::CBSig, i32) -> ();   // cache-to-bus helper function type

    // clock
    fn on_tick      (&mut self, send_proc_q: &mut SignalQ<S::CPSig>, send_bus: &mut SignalQ<S::CBSig>);
    fn on_post_tick (&mut self);
    // processor and bus communication
    fn on_proc_sig  (&mut self, sig: S::PCSig, send_proc: &mut SignalQ<S::CPSig>, send_bus: &mut SignalQ<S::CBSig>);
    fn on_bus_sig   (&mut self, sig: S::BCSig, send_proc: &mut SignalQ<S::CPSig>, send_bus: &mut SignalQ<S::CBSig>);
}

// pub trait Bus<CacheToBusMsg, BusToCacheMsg> {
//     // clock
//     fn on_tick(&mut self, send_cache: fn(BusToCacheMsg, CacheID, i32) -> ());
//     fn on_post_tick(&mut self);
//     // signals
//     fn on_cache_sig(&mut self, sig: CacheToBusMsg);
// }
