use std::collections::VecDeque;

// system specs

#[derive(PartialEq, Debug)]
pub enum Protocol {
    MESI,
    Dragon,
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
            protocol: Protocol::MESI,
            word_size: 4,       // bytes
            address_size: 4,    // bytes
            mem_lat: 100,       // cpu cycles
            bus_word_tf_lat: 2, // cpu cycles
            block_size: 32,     // bytes
            cache_size: 4096,   // bytes
            cache_assoc: 2,     // blocks
        }
    }
}

impl SystemSpec {
    // timing
    pub fn t_cache_to_cache_msg(&self) -> i32 {
        // assuming immediate response through wired OR
        self.bus_word_tf_lat * self.address_size / self.word_size
    }
    pub fn t_cache_to_cache_transfer(&self) -> i32 {
        self.bus_word_tf_lat * self.block_size / self.word_size
    }
    pub fn t_flush(&self) -> i32 {
        self.mem_lat
    }
    pub fn t_mem_fetch(&self) -> i32 {
        self.mem_lat
    }
}

// addresses and blocks

#[derive(Clone, PartialEq, Debug)]
pub struct Addr(pub i32);

impl Addr {
    pub fn pos(&self, specs: &SystemSpec) -> (i32, i32) {
        // returns the index and tag of the address under given system specs
        let num_indices = specs.cache_size / (specs.block_size * specs.cache_assoc);
        let index = self.0 % num_indices;
        let tag = self.0 / num_indices;
        (index, tag)
    }
}

// instructions

#[derive(Clone)]
pub enum Instr {
    Read(Addr),
    Write(Addr),
    Other(i32),
}

pub type Instructions = VecDeque<Instr>;
