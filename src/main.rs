extern crate serde_json;
extern crate serde;
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
use serde::{Serialize, Deserialize};

const INPUT_FILE: &str = "input.json";
const BALANCE_OUTPUT_FILE: &str = "balance.json";
const TRANSACTION_OUTPUT_FILE: &str = "transactions.json";
const EXTERNAL_POLICY_OUTPUT_FILE: &str = "external_address_policy.json";
const INTERNAL_POLICY_OUTPUT_FILE: &str = "internal_address_policy.json";
const NEXT_ADDRESS_OUTPUT_FILE: &str = "addresses.json";
const NUM_ADDRESSES: u32 = 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let policy = get_policy().map_err(|err| err.to_string())?;

    let policy = Concrete::<String>::from_str(&policy)?;
    let segwit_policy: Miniscript<String, Segwitv0> = policy
        .compile()
        .map_err(|e| Error::Generic(e.to_string()))?;

    let descriptor = Descriptor::new_wsh(segwit_policy).unwrap().to_string();

    // TODO should I output a descriptor?
    // println!("descriptor is {}", descriptor);

    let client = Client::new("ssl://electrum.blockstream.info:60002")?;
    let blockchain = ElectrumBlockchain::from(client);
    let wallet = Wallet::new(
        &descriptor,
        Some(&descriptor),
        bitcoin::Network::Testnet,
        MemoryDatabase::default(),
    )?;

    wallet.sync(&blockchain, SyncOptions::default())?;

    generate_output_files(wallet).expect("error generating output files");

    Ok(())
}

fn get_policy() -> Result<String, String> {
    // Open the file in read-only mode with buffer.
    let mut file = File::open(INPUT_FILE).map_err(|err| err.to_string())?;
    let mut data = String::new();
    file.read_to_string(&mut data).map_err(|err| err.to_string())?;

    // Parse the string of data into a JSON value.
    // TODO validate pubkey and policy inputs
    let v: serde_json::Value = serde_json::from_str(&data).map_err(|err| err.to_string())?;
    let pubkey1 = v["pubkey1"].as_str().ok_or_else(|| format!("`pubkey1` not found in {INPUT_FILE}"))?;
    let pubkey2 = v["pubkey2"].as_str().ok_or_else(|| format!("`pubkey2` not found in {INPUT_FILE}"))?;
    let policy = v["policy"].as_str().ok_or_else(|| format!("`policy` not found in {INPUT_FILE}"))?;

    // insert the pubkey values into the policy string
    // TODO come up with a better string replacement scheme than bash variables
    let policy = policy.replace("$MY_KEY", pubkey1).replace("$OTHER_KEY", pubkey2);

    // println!("pubkey1 is {}", pubkey1);
    // println!("pubkey2 is {}", pubkey2);
    // println!("policy is {}", policy);

    Ok(policy)
}

fn generate_output_files(wallet: Wallet<MemoryDatabase>) -> Result<(), Box<dyn std::error::Error>> {
    print_to_file(&wallet.get_balance().expect("error retrieving balance"), BALANCE_OUTPUT_FILE)?;
    print_to_file(&wallet.list_transactions(true).expect("error retrieving transactions"), TRANSACTION_OUTPUT_FILE)?;
    print_to_file(&wallet.policies(KeychainKind::External).expect("error retrieving external policies"), EXTERNAL_POLICY_OUTPUT_FILE)?;
    print_to_file(&wallet.policies(KeychainKind::Internal).expect("error retrieving internal policies"), INTERNAL_POLICY_OUTPUT_FILE)?;
    
    // get addresses
    // Note: this code returns the same address every time unless you specify an extended key descriptor i.e. one that ends in \*
    // TODO distinguish and handle single key vs. extended key descriptors
    let mut addresses = Vec::new();
    (0..NUM_ADDRESSES).for_each(|_i: u32| {
        addresses.push(wallet.get_address(AddressIndex::New).expect("error retrieving next address").to_string())
    });

    print_to_file(&Addresses { addresses }, NEXT_ADDRESS_OUTPUT_FILE)?;

    Ok(())
}

fn print_to_file<T>(contents: &T, filename: &str) -> Result<(), Box<dyn std::error::Error>> 
where 
    T:Serialize 
{
    let mut file = File::create(filename)?;
    let json = serde_json::to_string_pretty(&contents)?;
    file.write_all(json.as_bytes())?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Addresses {
    addresses: Vec<String>,
}