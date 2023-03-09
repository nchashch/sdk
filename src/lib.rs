mod types;
mod client;
mod main_state;

// Sidechain State Machine
pub trait SSM {
    type Block;
    type Error;

    fn connect(&mut self, blocks: &[Self::Block]) -> Result<(), Self::Error>;
    fn disconnect(&mut self, blocks: &[Self::Block]) -> Result<(), Self::Error>;
}

pub trait Validator {
    type State: SSM;
    type Error;

    fn validate(&self, blocks: &[<Self::State as SSM>::Block]) -> Result<(), Self::Error>;
}
