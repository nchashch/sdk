use crate::types::*;
use ed25519_dalek::Verifier;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct BlockChain {
    block_order: Vec<BlockHash>,
    headers: HashMap<BlockHash, Header>,
    bodies: HashMap<BlockHash, Body>,
    transactions: HashMap<Txid, Transaction>,

    pub outputs: HashMap<OutPoint, Output>,
    pub unspent_outpoints: HashSet<OutPoint>,
}

impl BlockChain {
    fn is_spent(&self, outpoint: &OutPoint) -> bool {
        !self.unspent_outpoints.contains(outpoint)
    }

    pub fn add_deposits(&mut self, deposit_outputs: HashMap<OutPoint, Output>) {
        self.unspent_outpoints
            .extend(deposit_outputs.keys().cloned());
        self.outputs.extend(deposit_outputs);
    }

    pub fn validate_transaction(&self, transaction: &Transaction) -> Result<(), String> {
        let value_in: u64 = transaction
            .inputs
            .iter()
            .map(|i| self.outputs[&i.outpoint].value)
            .sum();
        let value_out: u64 = transaction.outputs.iter().map(|o| o.value).sum();
        if value_out > value_in {
            return Err("value out > value in".into());
        }
        for input in &transaction.inputs {
            if self.is_spent(&input.outpoint) {
                return Err("output spent".into());
            }
            if let Some(spent_output) = self.outputs.get(&input.outpoint) {
                let address: Address = input.public_key.into();
                if spent_output.address != address {
                    return Err("addresses don't match".into());
                }
                let inputless_hash = hash(&transaction.without_inputs());
                if input
                    .public_key
                    .verify(&inputless_hash, &input.signature)
                    .is_err()
                {
                    return Err("wrong signature".into());
                }
            } else {
                return Err("output doesn't exist".into());
            }
        }
        Ok(())
    }

    pub fn validate_block(&self, header: &Header, body: &Body) -> bool {
        let best_block = self.get_best_block_hash().unwrap_or(Hash::default().into());
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

    pub fn connect_block(&mut self, header: &Header, body: &Body) {
        for tx in &body.transactions {
            let txid = tx.txid();
            self.transactions.insert(txid, tx.clone());
            for input in &tx.inputs {
                self.unspent_outpoints.remove(&input.outpoint);
            }
            for (vout, output) in tx.outputs.iter().enumerate() {
                let vout = vout as u32;
                let outpoint = OutPoint::Regular { txid, vout };
                self.outputs.insert(outpoint.clone(), output.clone());
                self.unspent_outpoints.insert(outpoint);
            }
            let block_hash = header.hash();
            self.headers.insert(block_hash, header.clone());
            self.bodies.insert(block_hash, body.clone());
            self.block_order.push(block_hash);
        }
    }

    pub fn disconnect_blocks(&mut self, header: &Header, body: &Body) {
        for tx in &body.transactions {
            let txid = tx.txid();
            for input in &tx.inputs {
                self.unspent_outpoints.insert(input.outpoint.clone());
            }
            for vout in 0..tx.outputs.len() {
                let vout = vout as u32;
                let outpoint = OutPoint::Regular { txid, vout };
                self.outputs.remove(&outpoint);
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

    pub fn get_fee(&self, transaction: &Transaction) -> u64 {
        let spent: u64 = transaction
            .inputs
            .iter()
            .map(|i| self.outputs[&i.outpoint].value)
            .sum();
        let regular_out: u64 = transaction.outputs.iter().map(|o| o.value).sum();
        let withdrawal_out: u64 = transaction
            .withdrawal_outputs
            .iter()
            .map(|wo| wo.value)
            .sum();
        spent - (regular_out + withdrawal_out)
    }
}
