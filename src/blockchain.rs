use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockChain<S, O> {
    block_order: Vec<BlockHash>,
    headers: HashMap<BlockHash, Header>,
    bodies: HashMap<BlockHash, Body<S, O>>,
    transactions: HashMap<Txid, Transaction<S, O>>,

    pub outputs: HashMap<OutPoint, O>,
    pub deposit_outputs: HashMap<OutPoint, DepositOutput>,
    deposits: Vec<Deposit>,
    pub withdrawal_outputs: HashMap<OutPoint, WithdrawalOutput>,
    pub unspent_outpoints: HashSet<OutPoint>,
}

impl<S: Sig + Serialize + Clone, O: Out + Serialize + Clone> BlockChain<S, O> {
    pub fn new() -> Self {
        BlockChain {
            block_order: vec![],
            headers: HashMap::new(),
            bodies: HashMap::new(),
            transactions: HashMap::new(),
            outputs: HashMap::new(),
            deposit_outputs: HashMap::new(),
            deposits: vec![],
            withdrawal_outputs: HashMap::new(),
            unspent_outpoints: HashSet::new(),
        }
    }

    fn is_spent(&self, outpoint: &OutPoint) -> bool {
        !self.unspent_outpoints.contains(outpoint)
    }

    pub fn add_deposits(&mut self, deposits_chunk: DepositsChunk) {
        self.unspent_outpoints
            .extend(deposits_chunk.outputs.keys().cloned());
        self.deposit_outputs.extend(deposits_chunk.outputs);
        self.deposits.extend(deposits_chunk.deposits);
    }

    pub fn validate_transaction(&self, transaction: &Transaction<S, O>) -> Result<(), String> {
        let (inputs, deposit_inputs, withdrawal_inputs) = self.get_inputs(transaction);
        if O::validate(
            &inputs,
            &deposit_inputs,
            &withdrawal_inputs,
            &transaction.outputs,
            &transaction.withdrawal_outputs,
        ) {
            return Err("value out > value in".into());
        }
        let txid_without_signatures = transaction.without_signatures().txid();
        for (outpoint, signature) in transaction.inputs.iter().zip(transaction.signatures.iter()) {
            if self.is_spent(&outpoint) {
                return Err("output spent".into());
            }
            if !signature.is_valid(txid_without_signatures) {
                return Err("wrong signature".into());
            }
            if let Some(spent_output) = self.outputs.get(&outpoint) {
                if spent_output.get_address() != signature.get_address() {
                    return Err("addresses don't match".into());
                }
            } else if let Some(spent_output) = self.withdrawal_outputs.get(&outpoint) {
                if spent_output.side_address != signature.get_address() {
                    return Err("addresses don't match".into());
                }
            } else if let Some(spent_output) = self.deposit_outputs.get(&outpoint) {
                if spent_output.address != signature.get_address() {
                    return Err("addresses don't match".into());
                }
            } else {
                return Err("output doesn't exist".into());
            }
        }
        Ok(())
    }

    pub fn validate_block(&self, header: &Header, body: &Body<S, O>) -> bool {
        let best_block = self
            .get_best_block_hash()
            .unwrap_or_else(|| Hash::default().into());
        if header.prev_block_hash != best_block {
            return false;
        }
        if header.merkle_root != body.compute_merkle_root() {
            return false;
        }
        for tx in &body.transactions {
            if self.validate_transaction(tx).is_err() {
                return false;
            }
        }
        true
    }

    pub fn connect_block(&mut self, header: &Header, body: &Body<S, O>) {
        for tx in &body.transactions {
            let txid = tx.txid();
            self.transactions.insert(txid, tx.clone());
            for outpoint in &tx.inputs {
                self.unspent_outpoints.remove(outpoint);
            }
            for (vout, output) in tx.outputs.iter().enumerate() {
                let vout = vout as u32;
                let outpoint = OutPoint::Regular { txid, vout };
                self.outputs.insert(outpoint, output.clone());
                self.unspent_outpoints.insert(outpoint);
            }
            for (vout, output) in tx.withdrawal_outputs.iter().enumerate() {
                let vout = vout as u32;
                let outpoint = OutPoint::Withdrawal { txid, vout };
                self.withdrawal_outputs.insert(outpoint, output.clone());
                self.unspent_outpoints.insert(outpoint);
            }
            let block_hash = header.hash();
            self.headers.insert(block_hash, header.clone());
            self.bodies.insert(block_hash, body.clone());
            self.block_order.push(block_hash);
        }
    }

    pub fn disconnect_block(&mut self, header: &Header, body: &Body<S, O>) {
        for tx in &body.transactions {
            let txid = tx.txid();
            for outpoint in &tx.inputs {
                self.unspent_outpoints.insert(*outpoint);
            }
            for vout in 0..tx.outputs.len() {
                let vout = vout as u32;
                let outpoint = OutPoint::Regular { txid, vout };
                self.outputs.remove(&outpoint);
                self.unspent_outpoints.remove(&outpoint);
            }
            for vout in 0..tx.withdrawal_outputs.len() {
                let vout = vout as u32;
                let outpoint = OutPoint::Withdrawal { txid, vout };
                self.withdrawal_outputs.remove(&outpoint);
                self.unspent_outpoints.remove(&outpoint);
            }
            self.transactions.remove(&txid);
        }
        let block_hash = header.hash();
        self.bodies.remove(&block_hash);
        self.headers.remove(&block_hash);
        self.block_order.pop();
    }

    fn get_best_block_hash(&self) -> Option<BlockHash> {
        self.block_order.last().copied()
    }

    pub fn get_fee(&self, transaction: &Transaction<S, O>) -> u64 {
        let (inputs, deposit_inputs, withdrawal_inputs) = self.get_inputs(transaction);
        O::get_fee(
            &inputs,
            &deposit_inputs,
            &withdrawal_inputs,
            &transaction.outputs,
            &transaction.withdrawal_outputs,
        )
    }

    fn get_inputs(
        &self,
        transaction: &Transaction<S, O>,
    ) -> (Vec<O>, Vec<DepositOutput>, Vec<WithdrawalOutput>) {
        let inputs: Vec<O> = transaction
            .inputs
            .iter()
            .filter(|outpoint| self.outputs.contains_key(outpoint))
            .map(|outpoint| self.outputs[outpoint].clone())
            .collect();
        let deposit_inputs: Vec<DepositOutput> = transaction
            .inputs
            .iter()
            .filter(|outpoint| self.deposit_outputs.contains_key(outpoint))
            .map(|outpoint| self.deposit_outputs[outpoint].clone())
            .collect();
        let withdrawal_inputs: Vec<WithdrawalOutput> = transaction
            .inputs
            .iter()
            .filter(|outpoint| self.withdrawal_outputs.contains_key(outpoint))
            .map(|outpoint| self.withdrawal_outputs[outpoint].clone())
            .collect();
        (inputs, deposit_inputs, withdrawal_inputs)
    }
}
