use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

fn main() {
    let mut csprng = rand::thread_rng();
    let keypair: Keypair = Keypair::generate(&mut csprng);
    let message: &[u8] = b"This is a test of the tsunami alert system.";
    let signature: Signature = keypair.sign(message);
    assert!(keypair.verify(message, &signature).is_ok());
}

const SHA256_LENGTH: usize = 32;
type Hash = [u8; SHA256_LENGTH];
type Address = Hash;
type Txid = Hash;
type BlockHash = Hash;
type MerkleRoot = Hash;

#[derive(Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
enum OutputKind {
    Regular,
    Withdrawal,
    Deposit,
}

#[derive(Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
struct OutPoint {
    output_kind: OutputKind,
    txid: Txid,
    vout: u32,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Input {
    outpoint: OutPoint,
    public_key: PublicKey,
    signature: Signature,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Output {
    address: Address,
    value: u64,
}

impl Ord for Output {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for Output {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for Output {}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct WithdrawalOutput {
    output: Output,
    fee: u64,
    main_address: bitcoin::Address,
}

// Implement Ord
// Ordering transactions by fee.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Transaction {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    withdrawal_outputs: Vec<WithdrawalOutput>,
}

impl Transaction {
    fn get_fee(&self) -> u64 {
        unimplemented!();
    }
}

impl Ord for Transaction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.get_fee().cmp(&other.get_fee())
    }
}

impl PartialOrd for Transaction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.get_fee().partial_cmp(&other.get_fee())
    }
}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.get_fee() == other.get_fee()
    }
}

impl Eq for Transaction {}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Header {
    prev_block_hash: BlockHash,
    merkle_root: MerkleRoot,
}

impl Header {
    fn new(prev_block_hash: &BlockHash, body: &Body) -> Self {
        Self {
            prev_block_hash: *prev_block_hash,
            merkle_root: body.compute_merkle_root(),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Body {
    transactions: Vec<Transaction>,
}

impl Body {
    fn new(transactions: &[Transaction]) -> Self {
        let transactions = Vec::from(transactions);
        Self { transactions }
    }

    fn compute_merkle_root(&self) -> MerkleRoot {
        unimplemented!();
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BlockChain {
    block_order: Vec<BlockHash>,
    headers: HashMap<BlockHash, Header>,
    bodies: HashMap<BlockHash, Body>,
    transactions: HashMap<Txid, Transaction>,
    outputs: HashMap<OutPoint, Output>,

    unspent_outpoints: HashSet<OutPoint>,
}

impl BlockChain {
    fn is_spent(&self, outpoint: &OutPoint) -> bool {
        !self.unspent_outpoints.contains(outpoint)
    }

    fn validate_block(&self, header: &Header, body: &Body) -> bool {
        let best_block = self.get_best_block_hash().unwrap();
        if header.prev_block_hash != best_block {
            return false;
        }
        if header.merkle_root != body.compute_merkle_root() {
            return false;
        }
        for tx in &body.transactions {
            for input in &tx.inputs {
                if self.is_spent(&input.outpoint) {
                    return false;
                }
                if let Some(spent_output) = self.outputs.get(&input.outpoint) {
                    let address: Address = hash(&input.public_key);
                    if spent_output.address != address {
                        return false;
                    }
                    let outputs_serialized = bincode::serialize(&tx.outputs).unwrap();
                    if input
                        .public_key
                        .verify(&outputs_serialized, &input.signature)
                        .is_err()
                    {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
        true
    }

    fn connect_block(&mut self, header: &Header, body: &Body) {
        for tx in &body.transactions {
            let txid = hash(tx);
            self.transactions.insert(txid, tx.clone());
            for input in &tx.inputs {
                self.unspent_outpoints.remove(&input.outpoint);
            }
            for (vout, output) in tx.outputs.iter().enumerate() {
                let vout = vout as u32;
                let outpoint = OutPoint {
                    output_kind: OutputKind::Regular,
                    txid,
                    vout,
                };
                self.outputs.insert(outpoint.clone(), output.clone());
                self.unspent_outpoints.insert(outpoint);
            }
            let block_hash = hash(header);
            self.headers.insert(block_hash, header.clone());
            self.bodies.insert(block_hash, body.clone());
            self.block_order.push(block_hash);
        }
    }

    fn disconnect_blocks(&mut self, header: &Header, body: &Body) {
        for tx in &body.transactions {
            let txid = hash(tx);
            for input in &tx.inputs {
                self.unspent_outpoints.insert(input.outpoint.clone());
            }
            for vout in 0..tx.outputs.len() {
                let vout = vout as u32;
                let outpoint = OutPoint {
                    output_kind: OutputKind::Regular,
                    txid,
                    vout,
                };
                self.outputs.remove(&outpoint);
                self.unspent_outpoints.remove(&outpoint);
            }
            self.transactions.remove(&txid);
        }
        let block_hash = hash(header);
        self.bodies.remove(&block_hash);
        self.headers.remove(&block_hash);
        self.block_order.pop();
    }

    fn get_best_block_hash(&self) -> Option<BlockHash> {
        self.block_order.last().copied()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MemPool {
    transactions: BTreeSet<Transaction>,
}

impl MemPool {
    fn select_transactions(&self, num: usize) -> Vec<Transaction> {
        self.transactions.iter().rev().take(num).cloned().collect()
    }

    fn add_transaction(&mut self, transaction: Transaction) -> bool {
        self.transactions.insert(transaction)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Wallet {
    keypairs: HashMap<Address, Keypair>,
    outputs: BTreeMap<Output, OutPoint>,
}

struct Coins {
    outputs: HashMap<OutPoint, Output>,
    change: u64,
}

impl Wallet {
    fn create_transaction(&mut self, mut outputs: Vec<Output>) -> Transaction {
        let amount: u64 = outputs.iter().map(|o| o.value).sum();
        let coins = self.select_coins(amount);
        let change = self.create_output(coins.change);
        outputs.push(change);
        let outputs_serialized = bincode::serialize(&outputs).unwrap();
        let inputs: Vec<Input> = coins
            .outputs
            .iter()
            .map(|(outpoint, output)| {
                let keypair = &self.keypairs[&output.address];
                let signature = keypair.sign(&outputs_serialized);
                Input {
                    outpoint: outpoint.clone(),
                    public_key: keypair.public,
                    signature,
                }
            })
            .collect();
        let transaction = Transaction {
            inputs,
            outputs,
            withdrawal_outputs: vec![],
        };
        let txid = hash(&transaction);
        for (vout, output) in transaction.outputs.iter().enumerate() {
            let vout = vout as u32;
            let outpoint = OutPoint {
                output_kind: OutputKind::Regular,
                txid,
                vout,
            };
            self.outputs.insert(output.clone(), outpoint);
        }
        transaction
    }

    fn create_output(&mut self, value: u64) -> Output {
        let mut csprng = rand::thread_rng();
        let keypair: Keypair = Keypair::generate(&mut csprng);
        let address: Address = hash(&keypair.public);
        self.keypairs.insert(address, keypair);
        Output { value, address }
    }

    fn select_coins(&self, amount: u64) -> Coins {
        unimplemented!();
    }

    fn sign(&self, address: &Address, data: &[u8]) -> Signature {
        self.keypairs[address].sign(data)
    }
}

fn hash<T: serde::Serialize>(data: &T) -> Hash {
    let mut hasher = sha2::Sha256::new();
    let data_serialized =
        bincode::serialize(data).expect("failed to serialize a type to compute a hash");
    hasher.update(data_serialized);
    hasher.finalize().into()
}
