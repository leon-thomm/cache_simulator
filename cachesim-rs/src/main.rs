pub mod simulator;
pub mod commons;
pub mod utils;
mod delayed_q_unhashed;

use crate::commons::*;
use simulator::simulate;

use std::time::Instant;
use std::env;


fn main() {
    let args: Vec<String> = env::args().collect();
    let specs;
    let testname;

    if args.len() > 1 {
        specs = SystemSpec {
            protocol: match args[1].as_str() {
                "MESI" => Protocol::MESI,
                "Dragon" => Protocol::Dragon,
                _ => panic!("invalid protocol argument"),
            },
            cache_size: args[3].parse().unwrap(),
            cache_assoc: args[4].parse().unwrap(),
            block_size: args[5].parse().unwrap(),
            ..Default::default()
        };
        testname = args[2].as_str();
    } else {
        specs = SystemSpec { ..Default::default() };
        testname = "tiny_blackscholes";
    }

    let t0 = Instant::now();
    simulate(
        specs,
        utils::read_testfiles(testname),
        false,
        None, // Some(400000),
    );
    let t1 = Instant::now();
    println!("execution time {:?}", t1-t0);
}