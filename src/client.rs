use crate::main_state::{Deposit, DepositsChunk};
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::util::psbt::serialize::{Deserialize, Serialize};
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
    pub fn get_deposits(
        &self,
        last_deposit: Option<bitcoin::OutPoint>,
    ) -> Result<DepositsChunk, Error> {
        let outpoint = match last_deposit {
            Some(outpoint) => vec![json!(outpoint.txid), json!(outpoint.vout)],
            None => vec![],
        };
        let params = &[vec![self.this_sidechain.into()], outpoint].concat();
        let json_deposits = self
            .client
            .send_request::<Vec<JsonDeposit>>("listsidechaindeposits", params)?;
        let mut deposits = HashMap::new();
        let mut outpoint_to_tx = HashMap::new();
        for deposit in json_deposits.iter().cloned().rev() {
            let tx = hex::decode(deposit.txhex)?;
            let tx = Transaction::deserialize(tx.as_slice())?;
            let outpoint = bitcoin::OutPoint {
                txid: tx.txid(),
                vout: deposit.nburnindex as u32,
            };
            let deposit = crate::main_state::Deposit {
                address: deposit.strdest,
                amount: tx.output[outpoint.vout as usize].value,
            };
            outpoint_to_tx.insert(outpoint, tx);
            deposits.insert(outpoint, deposit);
        }
        let deposits_order = sort_deposits(&outpoint_to_tx);
        Ok(DepositsChunk {
            deposits,
            deposits_order,
        })
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("ureq error")]
    Ureq(#[from] ureq_jsonrpc::Error),
    #[error("failed to decode hex value")]
    Hex(#[from] hex::FromHexError),
    #[error("bitcoin encoding error")]
    BitcoinEncode(#[from] bitcoin::consensus::encode::Error),
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

fn sort_deposits(
    deposits: &HashMap<bitcoin::OutPoint, bitcoin::Transaction>,
) -> Vec<bitcoin::OutPoint> {
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
            sorted_deposits.push(*outpoint);
        }
    }
    let mut outpoint = sorted_deposits[0];
    while let Some(next) = spent_by.get(&outpoint) {
        if deposits.contains_key(next) {
            sorted_deposits.push(*next);
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
        let address = format_deposit_address(client.this_sidechain, "sdk_address");
        dbg!(address);
        let deposits = client.get_deposits(None)?;
        dbg!(&deposits);
        let last = deposits.deposits_order[1];
        let deposits = client.get_deposits(Some(last))?;
        dbg!(deposits);
        Ok(())
    }
}
