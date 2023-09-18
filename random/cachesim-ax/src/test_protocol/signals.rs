use crate::simulator::interface::*;

#[derive(Clone)]
pub enum ProcToCacheSig {
    Req(Addr),
}

#[derive(Clone)]
pub enum CacheToProcSig {
    Res,
}

#[derive(Clone)]
pub enum CacheToBusSig {}

#[derive(Clone)]
pub enum BusToCacheSig {}

#[derive(Clone)]
pub struct SignalTypes {}

impl Signals for SignalTypes {
    type PCSig = ProcToCacheSig;
    type CPSig = CacheToProcSig;
    type CBSig = CacheToBusSig;
    type BCSig = BusToCacheSig;
}