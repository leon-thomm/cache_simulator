use async_trait::async_trait;

use crate::commons::*;
use super::signals::*;

pub struct TestBus {
    pub system_spec: SystemSpec,
}

// #[async_trait]
impl /*Bus<PrSig, BusSig> for*/ TestBus {
    // clock
    pub async fn on_tick(&mut self) {
        println!("on_tick from TestBus");
    }
    pub async fn on_post_tick(&mut self) {
        println!("on_post_tick from TestBus");
    }
    // cache locking
    pub async fn on_acquire(&mut self, cache_id: i32) {
        println!("on_acquire from TestBus");
    }
    pub async fn on_free(&mut self) {
        println!("on_free from TestBus");
    }
    // signals
    pub async fn on_send_sig(&mut self, sig: BusSig) {
        println!("on_send_sig from TestBus");
    }
}