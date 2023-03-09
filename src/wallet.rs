use crate::types::*;
use anyhow::Result;
use ed25519_dalek::{Keypair, Signer};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Wallet {
    keypairs: HashMap<Address, Keypair>,
    pub outputs: BTreeMap<Output, OutPoint>,
}

struct Coins {
    outputs: HashMap<OutPoint, Output>,
    change: u64,
}

impl Wallet {
    pub fn create_transaction(
        &mut self,
        mut outputs: Vec<Output>,
        fee: u64,
    ) -> Option<Transaction> {
        let amount: u64 = outputs.iter().map(|o| o.value).sum();
        let coins = match self.select_coins(amount) {
            Some(coins) => coins,
            None => return None,
        };
        if coins.change > fee {
            let change = self.create_output(coins.change - fee);
            outputs.push(change);
        }
        let inputs: Vec<OutPoint> = coins.outputs.keys().copied().collect();
        let transaction = Transaction {
            inputs,
            signatures: vec![],
            outputs,
            withdrawal_outputs: vec![],
        };
        let signatures = transaction
            .inputs
            .iter()
            .map(|i| {
                let address = coins.outputs[i].address;
                let keypair = &self.keypairs[&address];
                Signature::new(keypair, &transaction)
            })
            .collect();
        let transaction = Transaction {
            signatures,
            ..transaction
        };
        for (vout, output) in transaction.outputs.iter().enumerate() {
            let vout = vout as u32;
            let outpoint = OutPoint::Regular {
                txid: transaction.txid(),
                vout,
            };
        }
        Some(transaction)
    }

    pub fn generate_address(&mut self) -> Address {
        let mut csprng = rand::thread_rng();
        let keypair: Keypair = Keypair::generate(&mut csprng);
        let address: Address = keypair.public.into();
        self.keypairs.insert(address.clone(), keypair);
        address
    }

    pub fn create_output(&mut self, value: u64) -> Output {
        Output {
            value,
            address: self.generate_address(),
        }
    }

    fn select_coins(&self, value: u64) -> Option<Coins> {
        let mut total: u64 = 0;
        let mut outputs: HashMap<OutPoint, Output> = HashMap::new();
        for (output, outpoint) in self.outputs.iter() {
            if total >= value {
                break;
            }
            total += output.value;
            outputs.insert(outpoint.clone(), output.clone());
        }
        if total < value {
            return None;
        }
        let change = total - value;
        Some(Coins { outputs, change })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = std::fs::File::create(path)?;
        file.write_all(&bincode::serialize(self)?)?;
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Wallet> {
        let mut file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = Vec::new();
        // Read file into vector.
        reader.read_to_end(&mut buffer)?;
        let wallet = bincode::deserialize::<Wallet>(&buffer)?;
        Ok(wallet)
    }

    pub fn get_addresses(&self) -> Vec<Address> {
        self.keypairs.keys().cloned().collect()
    }

    pub fn add_outputs(&mut self, outputs: &HashMap<OutPoint, Output>) {
        for (outpoint, output) in outputs {
            if self.keypairs.contains_key(&output.address) {
                self.outputs.insert(output.clone(), outpoint.clone());
            }
        }
    }
}
