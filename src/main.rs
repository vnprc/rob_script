extern crate serde_json;
extern crate serde;
extern crate bitcoin;

use std::fs::File;
use std::str::FromStr;
use std::io::prelude::*;
use bdk::database::MemoryDatabase;
use bdk::electrum_client::{Client};
use bdk::miniscript::policy::Concrete;
use bdk::descriptor::{Descriptor, Miniscript, ExtendedDescriptor};
use bdk::descriptor::Segwitv0;
use bdk::{wallet::AddressIndex, Error, KeychainKind, Wallet, SyncOptions};
use bdk::blockchain::{ElectrumBlockchain};
use bdk::{Balance, TransactionDetails};
use bdk::descriptor::policy::Policy;
use bdk::miniscript::Terminal;
use serde::{Serialize, Deserialize};
use serde_json::json;

const INPUT_FILE: &str = "input.json";
const OUTPUT_FILE: &str = "output.json";
const NUM_ADDRESSES: u32 = 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let policy = get_policy().map_err(|err| err.to_string())?;

    let policy = Concrete::<String>::from_str(&policy)?;
    let segwit_policy: Miniscript<String, Segwitv0> = policy
        .compile()
        .map_err(|e| Error::Generic(e.to_string()))?;


    let descriptor = Descriptor::new_wsh(segwit_policy.clone()).unwrap().to_string();

    let client = Client::new("ssl://electrum.blockstream.info:60002")?;
    let blockchain = ElectrumBlockchain::from(client);
    let wallet = Wallet::new(
        &descriptor,
        Some(&descriptor),
        bitcoin::Network::Testnet,
        MemoryDatabase::default(),
    )?;

    wallet.sync(&blockchain, SyncOptions::default())?;

    generate_output_files(&wallet, &segwit_policy).expect("error generating output files");
    // generate_ui_helper(&wallet).expect("error generating ui helper json");

    Ok(())
}

fn recursive_branch(node: &Miniscript<String, Segwitv0>, depth: u32) -> serde_json::Value {
    let mut child_json = json!([]);
    for branch in node.branches() {
        child_json.as_array_mut().unwrap().push(recursive_branch(&branch, depth + 1));
    }
    
    let mut json = print_readable_policy(&node.node, depth);

    if child_json != json!([]) {
            json["zchildren"] = child_json;
    }
    json
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

    Ok(policy)
}

fn generate_output_files(wallet: &Wallet<MemoryDatabase>, segwit_policy: &Miniscript<String, Segwitv0>) -> Result<(), Box<dyn std::error::Error>> {

    // get addresses
    // Note: this code returns the same address every time unless you specify an extended key descriptor i.e. one that ends in \*
    // TODO distinguish and handle single key vs. extended key descriptors
    let mut addresses = Vec::new();
    (0..NUM_ADDRESSES).for_each(|_i: u32| {
        addresses.push(wallet.get_address(AddressIndex::New).expect("error retrieving next address").to_string())
    });

    let json_output = Output {
        balance: &wallet.get_balance().expect("error retrieving balance"),
        transactions: &wallet.list_transactions(true).expect("error retrieving transactions"),
        extern_policies: &wallet.policies(KeychainKind::External).expect("error retrieving external policies").unwrap(),
        intern_policies: &wallet.policies(KeychainKind::Internal).expect("error retrieving internal policies").unwrap(),
        addresses: addresses,
        extern_descriptor: &wallet.public_descriptor(KeychainKind::External).expect("error retrieving external descriptor").unwrap(),
        intern_descriptor: &wallet.public_descriptor(KeychainKind::Internal).expect("error retrieving internal descriptor").unwrap(),
        policy_structure: &recursive_branch(&segwit_policy, 0),
    };

    let mut file = File::create(OUTPUT_FILE)?;

    serde_json::to_writer_pretty(&mut file, &json_output)?;

    Ok(())
}

#[derive(Serialize)]
struct Output<'a> {
    balance: &'a Balance,
    transactions: &'a Vec<TransactionDetails>,
    extern_policies: &'a Policy,
    intern_policies: &'a Policy,
    addresses: Vec<String>,
    extern_descriptor: &'a ExtendedDescriptor,
    intern_descriptor: &'a ExtendedDescriptor,
    policy_structure: &'a serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct Addresses {
    addresses: Vec<String>,
}

fn print_readable_policy(node: &Terminal<String, Segwitv0>, depth: u32) -> serde_json::Value {
    match node {
        Terminal::True => json!({"name": "True", "depth": depth, "depth": depth}),
        Terminal::False => json!({"name": "False", "depth": depth}),
        Terminal::PkK(pk) => json!({"name": "PkK", "value": pk, "depth": depth}),
        Terminal::PkH(pk) => json!({"name": "PkH", "value": pk, "depth": depth}),
        Terminal::RawPkH(hash) => json!({"name": "RawPkH", "value": hash, "depth": depth}),
        Terminal::After(locktime) => json!({"name": "After", "value": locktime, "depth": depth}),
        Terminal::Older(sequence) => json!({"name": "Older", "value": sequence, "depth": depth}),
        Terminal::Sha256(hash) => json!({"name": "Sha256", "value": hash, "depth": depth}),
        Terminal::Hash256(hash) => json!({"name": "Hash256", "value": hash, "depth": depth}),
        Terminal::Ripemd160(hash) => json!({"name": "Ripemd160", "value": hash, "depth": depth}),
        Terminal::Hash160(hash) => json!({"name": "Hash160", "value": hash, "depth": depth}),
        Terminal::Alt(_miniscript) => json!({"name": "Alt", "depth": depth}),
        Terminal::Swap(_miniscript) => json!({"name": "Swap", "depth": depth}),
        Terminal::Check(_miniscript) => json!({"name": "Check", "depth": depth}),
        Terminal::DupIf(_miniscript) => json!({"name": "DupIf", "depth": depth}),
        Terminal::Verify(_miniscript) => json!({"name": "Verify", "depth": depth}),
        Terminal::NonZero(_miniscript) => json!({"name": "NonZero", "depth": depth}),
        Terminal::ZeroNotEqual(_miniscript) => json!({"name": "ZeroNotEqual", "depth": depth}),
        Terminal::AndV(_miniscript1, _miniscript2) => json!({"name": "AndV", "depth": depth}),
        Terminal::AndB(_miniscript1, _miniscript2) => json!({"name": "AndB", "depth": depth}),
        Terminal::AndOr(_miniscript1, _miniscript2, _miniscript3) => json!({"name": "AndOr", "depth": depth}),
        Terminal::OrB(_miniscript1, _miniscript2) => json!({"name": "OrB", "depth": depth}),
        Terminal::OrD(_miniscript1, _miniscript2) => json!({"name": "OrD", "depth": depth}),
        Terminal::OrC(_miniscript1, _miniscript2) => json!({"name": "OrC", "depth": depth}),
        Terminal::OrI(_miniscript1, _miniscript2) => json!({"name": "OrI", "depth": depth}),
        Terminal::Thresh(m, _miniscripts) => json!({"m": m, "name": "Thresh"}),
        Terminal::Multi(m, miniscript) => json!({"m": m, "name": "Multi", "value": miniscript}),
        Terminal::MultiA(m, miniscript) => json!({"m": m, "name": "MultiA", "value": miniscript}),
        _ => json!({"name": "Unmatched"})
    }
}