mod test_protocol;
use test_protocol::processor::*;
use test_protocol::cache::*;
// use test_protocol::bus::*;

mod simulator;
use simulator::interface::*;
use simulator::sim::*;


fn main() {
    // // instantiate models
    // let mut p0 = TestProcessor::default();
    // let mut c0 = TestCache::default();
    // // create mailboxes
    // let mb_p0 = Mailbox::new();
    // let mb_c0 = Mailbox::new();
    // // connect models
    // p0.o_cache_req.connect(TestCache::on_proc_sig, &mb_c0);
    // c0.o_bus_resp.connect(TestProcessor::on_req_resolved, &mb_p0);
    // // handles
    // let inp = mb_p0.address();
    // // initialize simulation
    // let mut sim = SimInit::new()
    //     .add_model(p0, mb_p0)
    //     .add_model(c0, mb_c0)
    //     .init(MonotonicTime::EPOCH);
    // // run simulation
    // sim.send_event(TestProcessor::on_tick, (), &inp);
    // sim.step();

    let spec = simulator::interface::SystemSpec {
        word_size: 4,
        address_size: 4,
        mem_lat: 100,
        bus_word_tf_lat: 2,
        block_size: 32,
        cache_size: 4096,
        cache_assoc: 2,
    };

    // // seems like rustc can't infer that sufficiently
    // type ProcType = Vec<Box<dyn simulator::interface::Processor<
    //     test_protocol::signals::ProcToCacheSig, 
    //     test_protocol::signals::CacheToProcSig,
    // >>>;

    let procs = vec![TestProcessor { instr: vec![Instr::Read(Addr(0)), Instr::Read(Addr(0))] }];
    let caches = vec![TestCache::default()];
    simulate(spec, procs, caches);
}
