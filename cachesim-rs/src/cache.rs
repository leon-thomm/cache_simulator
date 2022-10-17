// This module defines the necessary interface of caches to be used by
// Processor and Bus.

use std::borrow::Borrow;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use crate::processor;
use crate::processor::Address;
use crate::BusTransaction;

pub trait Cache {
    fn processor_request(&mut self, s: ProcessorSignal);
    fn get_proc_response(&self) -> &Option<CacheProcResponseMsg>;
    fn bus_transaction(&mut self, t: BusTransaction) -> CacheBusResponse;
}

/// a future response to a processor request
pub struct CacheProcResponse {
    request: Option < ProcessorSignal > ,
    response: Rc < RefCell < Option < CacheProcResponseMsg > > >,
}

impl CacheProcResponse {
    pub(crate) fn new() -> CacheProcResponse {
        CacheProcResponse{
            request: None,
            response: Rc::new(RefCell::new(None))
        }
    }
    pub fn get(&self) -> &Option<CacheProcResponseMsg> {
        self.response.borrow().borrow().deref()
    }
    fn set(&mut self, v: CacheProcResponseMsg) {
        *self.response.borrow().borrow_mut() = Some(v);
    }
}

struct CacheProcResponseMsg(i32);