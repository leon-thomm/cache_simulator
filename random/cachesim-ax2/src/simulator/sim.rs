// struct ProcessorComponent { ... }
// impl Component for ProcessorComponent {
//     type Outputs = component_outpus!(ProcCacheRequest);
//     pub fn tick(&mut self, &mut Scheduler) { ... }
//     pub fn post_tick(&mut self, &mut Scheduler) { ... }
//     pub fn done() -> bool { ... }
// }
// impl ProcessorComponent {
//     pub fn inp_cache_response(&mut self, &mut Scheduler, msg: &CacheResponseMsg) { ... }
//     pub fn inp_manual_interrupt(&mut self, &mut Scheduler, msg: &ManualInterruptMsg) { ... }
// }

// struct CacheComponent { ... }
// impl Component for CacheComponent {
//     type Outputs = component_outpus!(CacheProcResponse, CacheBusRequest, CacheBusResponse);
//     pub fn tick(&mut self, &mut Scheduler) { ... }
//     pub fn post_tick(&mut self, &mut Scheduler) { ... }
//     pub fn done() -> bool { ... }
// }
// impl CacheComponent {
//     pub fn inp_cache_request_proc(&mut self, &mut Scheduler, msg: &CacheRequestFromProcMsg) { ... }
//     pub fn inp_cache_request_bus(&mut self, &mut Scheduler, msg: &CacheResponseFromBusMsg) { ... }
// }