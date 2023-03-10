mod blockchain;
mod client;
mod mempool;
mod types;
mod wallet;

use blockchain::*;
use client::Client;
use mempool::*;
use types::*;
use wallet::*;

use anyhow::Result;

fn main() -> Result<()> {
    let mut blockchain = BlockChain::new();
    let mut mempool = MemPool::default();
    let mut wallet = Wallet::load("./fake_wallet.dat").unwrap_or_default();
    // for address in wallet.get_addresses() {
    //     dbg!(address.to_deposit_string());
    // }
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
    blockchain.add_deposits(deposits);
    wallet.add_outputs(&blockchain.outputs);
    dbg!(&blockchain.outputs);
    dbg!(&wallet.outputs);

    let output = wallet.create_output(100);
    let transaction = wallet.create_transaction(vec![output], 1).unwrap();
    let fee = blockchain.get_fee(&transaction);
    mempool.insert(fee, transaction.clone());
    let body = mempool.create_body(wallet.generate_address(), 1);
    let header = Header::new(&Hash::default().into(), &body);
    dbg!(blockchain.validate_block(&header, &body));

    dbg!(&blockchain.unspent_outpoints);
    dbg!(blockchain.connect_block(&header, &body));
    dbg!(&blockchain.unspent_outpoints);

    dbg!(&header, &body);
    wallet.save("./fake_wallet.dat")?;
    Ok(())
}
