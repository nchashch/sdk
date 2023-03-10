use crate::types::*;
use ed25519_dalek::{Signer, Verifier};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub address: Address,
    pub value: u64,
}

impl Out for Output {
    fn validate(
        inputs: &[Self],
        deposit_inputs: &[DepositOutput],
        withdrawal_inputs: &[WithdrawalOutput],
        outputs: &[Self],
        withdrawal_outputs: &[WithdrawalOutput],
    ) -> bool {
        let regular_in: u64 = inputs.iter().map(|i| i.value).sum();
        let deposit_in: u64 = deposit_inputs.iter().map(|i| i.value).sum();
        let refund_in: u64 = withdrawal_inputs.iter().map(|i| i.value).sum();

        let regular_out: u64 = outputs.iter().map(|o| o.value).sum();
        let withdrawal_out: u64 = withdrawal_outputs.iter().map(|o| o.value).sum();
        regular_out + withdrawal_out > regular_in + deposit_in + refund_in
    }
    fn get_fee(
        inputs: &[Self],
        deposit_inputs: &[DepositOutput],
        withdrawal_inputs: &[WithdrawalOutput],
        outputs: &[Self],
        withdrawal_outputs: &[WithdrawalOutput],
    ) -> u64 {
        let regular_in: u64 = inputs.iter().map(|i| i.value).sum();
        let deposit_in: u64 = deposit_inputs.iter().map(|i| i.value).sum();
        let withdrawal_in: u64 = withdrawal_inputs.iter().map(|i| i.value).sum();

        let regular_out: u64 = outputs.iter().map(|o| o.value).sum();
        let withdrawal_out: u64 = withdrawal_outputs.iter().map(|wo| wo.value).sum();
        (regular_in + deposit_in + withdrawal_in) - (regular_out + withdrawal_out)
    }
    fn get_address(&self) -> Address {
        self.address
    }
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

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    public_key: ed25519_dalek::PublicKey,
    signature: ed25519_dalek::Signature,
}

impl Signature {
    pub fn new(
        keypair: &ed25519_dalek::Keypair,
        transaction: &Transaction<Signature, Output>,
    ) -> Self {
        let hash: Hash = transaction.txid().into();
        Self {
            signature: keypair.sign(&hash),
            public_key: keypair.public,
        }
    }
}

impl Sig for Signature {
    fn is_valid(&self, txid_without_signatures: Txid) -> bool {
        let hash: Hash = txid_without_signatures.into();
        self.public_key.verify(&hash, &self.signature).is_ok()
    }

    fn get_address(&self) -> Address {
        self.public_key.into()
    }
}
