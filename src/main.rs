extern crate serde_json;
extern crate bitcoin;

use std::fs::File;
use std::str::FromStr;
use std::io::prelude::*;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::{Client};
use bdk::miniscript::policy::Concrete;
use bdk::descriptor::{Descriptor, Miniscript};
use bdk::descriptor::Segwitv0;
use bdk::{wallet::AddressIndex, Error, KeychainKind, Wallet, SyncOptions};
use bdk::blockchain::{ElectrumBlockchain};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open the file in read-only mode with buffer.
    let mut file = File::open("data.json")?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;

    // Parse the string of data into a JSON value.
    let v: serde_json::Value = serde_json::from_str(&data)?;

    let pubkey1 = v["pubkey1"].as_str().unwrap();
    let pubkey2 = v["pubkey2"].as_str().unwrap();
    let policy = v["policy"].as_str().unwrap().replace("$MY_KEY", pubkey1).replace("$OTHER_KEY", pubkey2);

    println!("pubkey1 is {}", pubkey1);
    println!("pubkey2 is {}", pubkey2);
    println!("policy is {}", policy);

    let policy = Concrete::<String>::from_str(policy.as_str())?;
    let segwit_policy: Miniscript<String, Segwitv0> = policy
        .compile()
        .map_err(|e| Error::Generic(e.to_string()))?;

    let descriptor = Descriptor::new_wsh(segwit_policy).unwrap().to_string();

    println!("descriptor is {}", descriptor);

    let client = Client::new("ssl://electrum.blockstream.info:60002")?;
    let blockchain = ElectrumBlockchain::from(client);
    let wallet = Wallet::new(
        &descriptor,
        Some(&descriptor),
        bitcoin::Network::Testnet,
        MemoryDatabase::default(),
    )?;

    wallet.sync(&blockchain, SyncOptions::default())?;

    println!("balance: {}", wallet.get_balance()?);
    println!("transactions: {:#?}", wallet.list_transactions(true)?);
    println!("external policies: {:#?}", wallet.policies(KeychainKind::External)?);
    println!("internal policies: {:#?}", wallet.policies(KeychainKind::Internal)?);
    println!("next address: {:#?}", wallet.get_address(AddressIndex::LastUnused)?);

    Ok(())
}