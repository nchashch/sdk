use crate::types::*;
use crate::concrete::*;
use std::collections::BTreeMap;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MemPool {
    transactions: BTreeMap<u64, Transaction<Signature, Output>>,
}

impl MemPool {
    pub fn create_body(&self, coinbase_address: Address, num: usize) -> Body<Signature, Output> {
        let transactions = self.transactions.iter().rev().take(num);
        let fee: u64 = transactions.clone().map(|(fee, _)| fee).sum();
        let transactions = transactions.map(|(_, tx)| tx.clone()).collect();
        let coinbase = vec![Output {
            address: coinbase_address,
            value: fee,
        }];
        Body {
            coinbase,
            transactions,
        }
    }

    pub fn insert(&mut self, fee: u64, transaction: Transaction<Signature, Output>) -> bool {
        self.transactions.insert(fee, transaction).is_some()
    }
}
