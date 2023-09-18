use async_trait::async_trait;

use crate::test_protocol::signals;

pub enum Protocol {
    Test,
}

pub struct SystemSpec {
    pub protocol: Protocol,
    pub word_size: i32,
    pub address_size: i32,
    pub mem_lat: i32,
    pub bus_word_tf_lat: i32,
    pub block_size: i32,
    pub cache_size: i32,
    pub cache_assoc: i32,
}

impl Default for SystemSpec {
    fn default() -> Self {
        SystemSpec {
            protocol: Protocol::Test,
            word_size: 4,       // in bytes
            address_size: 4,    // in bytes
            mem_lat: 100,       // in cpu cycles
            bus_word_tf_lat: 2, // in cpu cycles
            block_size: 32,     // in bytes
            cache_size: 4096,   // in bytes
            cache_assoc: 2,     // in blocks
        }
    }
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


pub trait Processor<PrToCacheMsg, CacheToProcMsg> {
    // clock
    fn on_tick(&mut self, send_cache: fn(PrToCacheMsg, i32) -> ());
    fn on_post_tick(&mut self);
    // cache communication
    fn on_cache_sig(&mut self, sig: CacheToProcMsg);
}


struct _Processor<PrToCacheMsg, CacheToProcMsg> {
    proc: Processor<PrToCacheMsg, CacheToProcMsg>,
}

impl<PrToCacheMsg, CacheToProcMsg> _Processor<PrToCacheMsg, CacheToProcMsg> {
    // clock
    async fn on_tick(&mut self) {
        // invoke processor's on_tick
        self.proc.on_tick(|msg, delay| {
            // send message to cache
        });
    }
    async fn on_post_tick(&mut self) {
        // invoke processor's on_post_tick
        self.proc.on_post_tick();
    }
    // cache response
    async fn on_cache_sig(&mut self, sig: CacheToProcMsg) {
        // invoke processor's on_req_resolved
        self.proc.on_cache_sig(sig);
    }
}

// #[async_trait]
// pub trait Cache<PrSigT, BusSigT> {
//     // clock
//     async fn on_tick(&mut self);
//     async fn on_post_tick(&mut self);
//     // signals
//     async fn on_proc_sig(&mut self, sig: PrSigT);
//     async fn on_bus_sig(&mut self, sig: BusSigT);
// }

// #[async_trait]
// pub trait Bus<PrSigT, BusSigT> {
//     // clock
//     async fn on_tick(&mut self);
//     async fn on_post_tick(&mut self);
//     // cache locking
//     async fn on_acquire(&mut self, cache_id: i32);
//     async fn on_free(&mut self);
//     // signals
//     async fn on_send_sig(&mut self, sig: BusSigT);
// }