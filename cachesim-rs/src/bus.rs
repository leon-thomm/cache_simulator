use std::cell::RefCell;
use std::rc::Rc;
use crate::{Address, Cache, L1};

struct Bus<C>
where C: Cache {
    data_traffic_count: i32,
    invalidations_count: i32,
    state: BusState,
    caches: Vec<Rc<RefCell<C>>>,
}

impl Bus<C>
where C: Cache {

}

enum BusTransaction {
    ReadShared(Address),
    ReadExclusive(Address),
}