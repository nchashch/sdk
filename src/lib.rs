use bitcoin::{Address, Txid};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{HashMap, HashSet};

pub type Uint256 = [u8; 32];

// Deposit outputs live on mainchain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deposit<A> {
    amount: u64,
    address: A,
}

// TODO: Consider writing an "Output" trait
impl<A: Clone> Deposit<A> {
    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn address(&self) -> A {
        self.address.clone()
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct DepositOutpoint;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepositInput<S> {
    pub outpoint: DepositOutpoint,
    pub signature: S,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Withdrawal<A> {
    amount: u64,
    fee: u64,
    mainchain_address: Address,
    sidechain_address: A,
}

impl<A: Clone> Withdrawal<A> {
    pub fn amount(&self) -> u64 {
        self.amount + self.fee
    }

    pub fn address(&self) -> A {
        self.sidechain_address.clone()
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawalOutpoint {
    block_hash: Uint256,
    index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RefundInput<S> {
    pub outpoint: WithdrawalOutpoint,
    pub signature: S,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Header<H> {
    prev_side_block_hash: Uint256,
    prev_main_block_hash: bitcoin::BlockHash,
    header: H,
}

struct Error;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct MainState<A: Unlockable> {
    claimable_deposits: HashSet<DepositOutpoint>,
    deposits: HashMap<DepositOutpoint, Deposit<A>>,
    refundable_withdrawals: HashSet<WithdrawalOutpoint>,
    withdrawals: HashMap<WithdrawalOutpoint, Withdrawal<A>>,
    pending_withdrawal_bundle: Option<(Txid, HashSet<WithdrawalOutpoint>)>,
    bmm_commitments: HashMap<Uint256, bitcoin::BlockHash>,
}

impl<A: Unlockable + Serialize + for<'a> Deserialize<'a> + Clone> MainState<A> {
    // called whenever a block is connected or disconnected on mainchain
    fn update(
        &mut self,
        deposits: &[Deposit<A>],
        spent_bundles: &[Txid],
        pending_bundles: &[Txid],
        failed_bundles: &[Txid],
    ) {
        unimplemented!();
    }

    pub fn get_deposit(&self, outpoint: &DepositOutpoint) -> Option<Deposit<A>> {
        self.deposits.get(outpoint).cloned()
    }

    pub fn get_withdrawal(&self, outpoint: &WithdrawalOutpoint) -> Option<Withdrawal<A>> {
        self.withdrawals.get(outpoint).cloned()
    }

    fn validate_block<H: Serialize, B: Body<A>>(&self, header: &Header<H>, body: &B) -> bool {
        let prev_main_block_hash = self.bmm_commitments.get(&header.hash()).unwrap();
        if header.prev_main_block_hash != *prev_main_block_hash {
            return false;
        }
        for refund_input in body.refund_inputs() {
            if let Some(refunded_withdrawal) = self.withdrawals.get(&refund_input.outpoint) {
                if !refunded_withdrawal
                    .sidechain_address
                    .check_signature(&refund_input.signature)
                {
                    return false;
                }
            } else {
                return false;
            }
            if !self.refundable_withdrawals.contains(&refund_input.outpoint) {
                return false;
            }
        }
        for deposit_input in body.deposit_inputs() {
            if let Some(claimed_deposit) = self.deposits.get(&deposit_input.outpoint) {
                if !claimed_deposit
                    .address
                    .check_signature(&deposit_input.signature)
                {
                    return false;
                }
            }
            if !self.claimable_deposits.contains(&deposit_input.outpoint) {
                return false;
            }
        }
        true
    }

    fn connect<H: Serialize, B: Body<A>>(
        &mut self,
        header: &Header<H>,
        body: &B,
    ) -> Result<(), Error> {
        let block_hash = header.hash();
        let withdrawal_outpoints: HashSet<WithdrawalOutpoint> = (0..body.withdrawals().len())
            .map(|index| WithdrawalOutpoint { block_hash, index })
            .collect();
        let withdrawals = withdrawal_outpoints
            .clone()
            .into_iter()
            .zip(body.withdrawals());
        self.refundable_withdrawals.extend(withdrawal_outpoints);
        self.withdrawals.extend(withdrawals);

        for refund_input in body.refund_inputs() {
            self.refundable_withdrawals.remove(&refund_input.outpoint);
        }

        for deposit_input in body.deposit_inputs() {
            self.claimable_deposits.remove(&deposit_input.outpoint);
        }
        Ok(())
    }

    fn disconnect<H: Serialize, B: Body<A>>(
        &mut self,
        header: &Header<H>,
        body: &B,
    ) -> Result<(), Error> {
        let block_hash = header.hash();
        let withdrawal_outpoints =
            (0..body.withdrawals().len()).map(|index| WithdrawalOutpoint { block_hash, index });
        for wo in withdrawal_outpoints {
            self.refundable_withdrawals.remove(&wo);
            self.withdrawals.remove(&wo);
        }

        let refund_inputs = body.refund_inputs();
        let refunded_withdrawal_outpoints = refund_inputs.iter().map(|r| r.outpoint.clone());
        self.refundable_withdrawals
            .extend(refunded_withdrawal_outpoints);

        let deposit_inputs = body.deposit_inputs();
        let claimed_deposit_outpoints = deposit_inputs.iter().map(|d| d.outpoint.clone());
        self.claimable_deposits.extend(claimed_deposit_outpoints);
        Ok(())
    }
}

pub trait Unlockable {
    type Signature;

    fn check_signature(&self, signature: &Self::Signature) -> bool;
}

pub trait Body<A: Unlockable + Serialize + for<'a> Deserialize<'a>> {
    type Digest;

    fn digest(&self) -> Self::Digest;
    fn withdrawals(&self) -> Vec<Withdrawal<A>>;
    fn refund_inputs(&self) -> Vec<RefundInput<<A as Unlockable>::Signature>>;
    fn deposit_inputs(&self) -> Vec<DepositInput<<A as Unlockable>::Signature>>;
}

pub trait SideState<A: Unlockable + Serialize + for<'a> Deserialize<'a>, B: Body<A>> {
    type Error;

    fn validate_block(
        &self,
        main_state: &MainState<A>,
        header: &Header<<B as Body<A>>::Digest>,
        body: &B,
    ) -> bool;
    fn connect(
        &mut self,
        header: &Header<<B as Body<A>>::Digest>,
        body: &B,
    ) -> Result<(), Self::Error>;
    fn disconnect(
        &mut self,
        header: &Header<<B as Body<A>>::Digest>,
        body: &B,
    ) -> Result<(), Self::Error>;
}

pub trait Sha256Hash {
    fn hash(&self) -> Uint256;
}

impl<T: Serialize> Sha256Hash for T {
    fn hash(&self) -> [u8; 32] {
        let mut hasher = sha2::Sha256::new();
        hasher.update(bincode::serialize(self).unwrap());
        hasher.finalize().into()
    }
}
