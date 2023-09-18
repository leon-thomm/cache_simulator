use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io::Read;

use crate::commons::{Addr, Instr, Instructions};

pub fn read_testfiles(testname: &str) -> Vec<Instructions> {
    // reads all files that begin with testname from the tests directory
    // and returns a vector of instructions for each file
    // the order is currently undefined
    let mut insts = Vec::new();
    let paths = fs::read_dir("../datasets/").unwrap();
    // iterate all files that start with `testname`
    for path in paths.filter_map(|p| p.ok()).filter(|p| {
        p.file_name().to_str().unwrap().starts_with(testname) &&
            p.file_name().to_str().unwrap().ends_with(".data")
    }) {
        println!("reading file: {:?}", path.file_name());
        let mut f = File::open(path.path()).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        let mut insts_for_proc = VecDeque::new();
        for line in s.lines() {
            let mut parts = line.split_whitespace();
            let inst = parts.next().unwrap().parse::<i32>().unwrap();
            let val = i32::from_str_radix(
                parts.next().unwrap().trim_start_matches("0x"),
                16).unwrap();
            insts_for_proc.push_back(match inst {
                0 => Instr::Read(Addr(val)),
                1 => Instr::Write(Addr(val)),
                2 => Instr::Other(val),
                _ => panic!("invalid instruction"),
            });
        }
        insts.push(insts_for_proc);
    }
    println!("done");
    insts
}
