use crate::types::{Deposit, DepositOutput, DepositsChunk, OutPoint};
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::util::psbt::serialize::Deserialize;
use std::collections::HashMap;
use ureq_jsonrpc::json;

// TODO: Implement mock client for running unit tests.
pub struct Client {
    pub this_sidechain: usize,
    pub client: ureq_jsonrpc::Client,
}

#[derive(Debug)]
pub struct VerifiedBMM {
    pub time: i64,
    pub txid: bitcoin::Txid,
}

impl Client {
    pub fn get_deposits(&self, last_deposit: Option<Deposit>) -> Result<DepositsChunk, Error> {
        let (outpoint, mut prev_value) = match last_deposit {
            Some(Deposit { outpoint, total }) => {
                (vec![json!(outpoint.txid), json!(outpoint.vout)], total)
            }
            None => (vec![], 0),
        };
        let params = &[vec![self.this_sidechain.into()], outpoint].concat();
        let json_deposits = self
            .client
            .send_request::<Vec<JsonDeposit>>("listsidechaindeposits", params)?;
        let mut outputs = HashMap::new();
        let mut outpoint_to_tx = HashMap::new();
        for deposit in json_deposits.iter().cloned().rev() {
            let tx = hex::decode(deposit.txhex)?;
            let tx = Transaction::deserialize(tx.as_slice())?;
            let outpoint = OutPoint::Deposit(bitcoin::OutPoint {
                txid: tx.txid(),
                vout: deposit.nburnindex as u32,
            });
            let value = tx.output[deposit.nburnindex].value;
            if value < prev_value {
                continue;
            }
            let output = DepositOutput {
                address: deposit.strdest.parse()?,
                value: value - prev_value,
            };
            prev_value = value;
            if let OutPoint::Deposit(outpoint) = outpoint {
                outpoint_to_tx.insert(outpoint, tx);
            }
            outputs.insert(outpoint, output);
        }
        let deposits = sort_deposits(&outpoint_to_tx);
        Ok(DepositsChunk { outputs, deposits })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("ureq error")]
    Ureq(#[from] ureq_jsonrpc::Error),
    #[error("failed to decode hex value")]
    Hex(#[from] hex::FromHexError),
    #[error("bitcoin encoding error")]
    BitcoinEncode(#[from] bitcoin::consensus::encode::Error),
    #[error("bs58 decode errro")]
    Bs58Decode(#[from] bs58::decode::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct JsonDeposit {
    hashblock: bitcoin::BlockHash,
    nburnindex: usize,
    nsidechain: usize,
    ntx: usize,
    strdest: String,
    txhex: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MainDeposit {
    address: String,
    tx: bitcoin::Transaction,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DepositOutpoint {
    txid: bitcoin::Txid,
    index: usize,
}

fn sort_deposits(deposits: &HashMap<bitcoin::OutPoint, bitcoin::Transaction>) -> Vec<Deposit> {
    if deposits.is_empty() {
        return vec![];
    }
    let mut spent_by = HashMap::<bitcoin::OutPoint, bitcoin::OutPoint>::new();
    let mut sorted_deposits = vec![];
    for (outpoint, tx) in deposits {
        let mut spent = false;
        for input in &tx.input {
            if deposits.contains_key(&input.previous_output) {
                spent_by.insert(input.previous_output, *outpoint);
                spent = true;
            }
        }
        if !spent {
            let total = tx.output[outpoint.vout as usize].value;
            sorted_deposits.push(Deposit {
                outpoint: outpoint.clone(),
                total,
            });
        }
    }
    let mut outpoint = sorted_deposits[0].outpoint;
    while let Some(next) = spent_by.get(&outpoint) {
        if deposits.contains_key(next) {
            let tx = &deposits[next];
            let total = tx.output[next.vout as usize].value;
            sorted_deposits.push(Deposit {
                outpoint: next.clone(),
                total,
            });
            outpoint = *next;
        }
    }
    sorted_deposits
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn format_deposit_address(sidechain_number: usize, address: &str) -> String {
        let deposit_address: String = format!("s{}_{}_", sidechain_number, address);
        let hash = sha256::digest(deposit_address.as_bytes()).to_string();
        let hash: String = hash[..6].into();
        format!("{}{}", deposit_address, hash)
    }

    #[test]
    fn it_works() -> anyhow::Result<()> {
        let client = Client {
            this_sidechain: 0,
            client: ureq_jsonrpc::Client {
                host: "localhost".into(),
                port: 18443,
                user: "user".into(),
                password: "password".into(),
                id: "sdk".into(),
            },
        };
        let deposits = client.get_deposits(None)?;
        dbg!(deposits);
        Ok(())
    }
}
