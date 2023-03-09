pub use crate::types::Output;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawalOutpoint {
    block_hash: [u8; 32],
    index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Withdrawal {
    pub value: u64,
    pub fee: u64,
    pub main_address: bitcoin::Address,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Deposit {
    pub outpoint: bitcoin::OutPoint,
    pub total: u64,
}

// Two Way Peg State
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct TwoWayPegState {
    // These two are never touched by sidechain code.
    outputs: HashMap<bitcoin::OutPoint, Output>,
    deposits: Vec<Deposit>,

    unspent_deposit_outputs: HashSet<bitcoin::OutPoint>,

    withdrawals: HashMap<WithdrawalOutpoint, Withdrawal>,
    unspent_withdrawal_outputs: HashSet<WithdrawalOutpoint>,
}

pub struct TwoWayPegChunk {
    withdrawals: HashMap<WithdrawalOutpoint, Withdrawal>,
    refund_inputs: Vec<WithdrawalOutpoint>,
    deposit_inputs: Vec<bitcoin::OutPoint>,
}

#[derive(Debug)]
pub struct DepositsChunk {
    pub outputs: HashMap<bitcoin::OutPoint, Output>,
    pub deposits: Vec<Deposit>,
}

impl TwoWayPegState {
    fn connect_deposits(&mut self, chunk: DepositsChunk) {
        self.outputs.extend(chunk.outputs);
        self.deposits.extend(chunk.deposits);
    }

    fn disconnect_deposits(&mut self, chunk: DepositsChunk) {
        for deposit in chunk.deposits {
            self.outputs.remove(&deposit.outpoint);
            self.deposits.pop();
        }
    }

    fn validate(&self, chunk: &TwoWayPegChunk) -> bool {
        for d_input in &chunk.deposit_inputs {
            if !self.unspent_deposit_outputs.contains(d_input) {
                return false;
            }
        }
        for r_input in &chunk.refund_inputs {
            if !self.unspent_withdrawal_outputs.contains(r_input) {
                return false;
            }
        }
        true
    }

    fn connect(&mut self, chunk: TwoWayPegChunk) -> Result<(), Error> {
        let TwoWayPegChunk {
            deposit_inputs,
            refund_inputs,
            withdrawals,
        } = chunk;
        for d_input in &deposit_inputs {
            self.unspent_deposit_outputs.remove(d_input);
        }
        for r_input in &refund_inputs {
            self.unspent_withdrawal_outputs.remove(r_input);
        }
        for outpoint in withdrawals.keys() {
            self.unspent_withdrawal_outputs.insert(outpoint.clone());
        }
        self.withdrawals.extend(withdrawals);
        Ok(())
    }

    fn disconnect(&mut self, chunk: TwoWayPegChunk) -> Result<(), Error> {
        let TwoWayPegChunk {
            deposit_inputs,
            refund_inputs,
            withdrawals,
        } = chunk;
        self.unspent_deposit_outputs.extend(deposit_inputs);
        self.unspent_withdrawal_outputs.extend(refund_inputs);
        for outpoint in withdrawals.keys() {
            self.withdrawals.remove(outpoint);
            self.unspent_withdrawal_outputs.remove(outpoint);
        }
        Ok(())
    }
}

struct Error;
