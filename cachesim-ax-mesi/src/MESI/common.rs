#[derive(Clone, Copy)]
pub struct SystemSpec {         // unit         reasonable defaults
    pub word_size: u32,         // bytes        4
    pub address_size: u32,      // bytes        4
    pub mem_lat: u32,           // cpu          100
    pub bus_word_tf_lat: u32,   // cpu          2
    pub block_size: u32,        // bytes        32
    pub cache_size: u32,        // bytes        4096
    pub cache_assoc: u32,       // blocks       2
}

pub mod timing {
    use super::SystemSpec;
    pub fn c2c_msg(spec: &SystemSpec) -> u32 {
        // assuming immediate response through wired OR
        spec.bus_word_tf_lat * spec.address_size / spec.word_size
    }
    pub fn c2c_transfer(spec: &SystemSpec) -> u32 {
        spec.bus_word_tf_lat * spec.block_size / spec.word_size
    }
    pub fn flush(spec: &SystemSpec) -> u32 {
        spec.mem_lat
    }
    pub fn mem_fetch(spec: &SystemSpec) -> u32 {
        spec.mem_lat
    }
}

#[derive(Clone)]
pub struct Addr(pub u32);

impl Addr {
    /// get cache index and tag of this address under given system specs
    pub fn pos(&self, specs: &SystemSpec) -> (u32, u32) {
        let num_indices = specs.cache_size / (specs.block_size * specs.cache_assoc);
        let index = self.0 % num_indices;
        let tag = self.0 / num_indices;
        (index, tag)
    }
}

#[derive(Clone)]
pub enum Instr {
    Read(Addr),
    Write(Addr),
    Other(u32),
}

pub type Insts = Vec<Instr>;

// MESSAGE TYPES

#[derive(Clone)]
pub enum ProcCacheReq {
    Read(Addr),
    Write(Addr),
}

#[derive(Clone)]
pub enum CacheProcResp {
    RequestResolved,
}

// the bus signals that caches can receive as defined by the protocol
#[derive(Clone)]
pub enum BusSignal {
    BusRd(Addr),
    BusRdX(Addr),
    BusUpd(Addr),
}

#[derive(Clone)]
pub enum CacheToCacheReq {
    CheckAddr(Addr),
}