extern crate serde_json;
extern crate serde;
extern crate bitcoin;

use std::fmt::Display;
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
use bdk::descriptor::policy::{Policy, SatisfiableItem, PkOrF};
use serde::{Serialize, Deserialize};
use serde_json::{json};

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

    Ok(())
}

fn get_policy_descriptions(policy: &Policy, depth: u32) -> serde_json::Value {
    let children: Option<serde_json::Value> = match &policy.item {
        SatisfiableItem::Thresh {items, threshold: _} => {
            let mut policy_descriptions:Vec<serde_json::Value> = Vec::new();
            for item in items {
                policy_descriptions.push(get_policy_descriptions(&item, depth + 1));
            }
            Some(json!(policy_descriptions))
        }
        _ => None,
    };

    let mut json = json!({
        "description": description(&policy),
        "depth": depth,
        "policy_id": &policy.id,
    });

    // include child policies if they exist
    // there HAS to be a better way to add a field to an existing json object
    if children.is_some() {
        json = match json {
            serde_json::Value::Object(m) => {
                let mut m = m.clone();
                m.insert(String::from("items"), children.unwrap());
                serde_json::Value::Object(m)
            },
            v => v.clone(),
        };
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
    let json: serde_json::Value = serde_json::from_str(&data).map_err(|err| err.to_string())?;
    
    let keys = json["keys"].as_object().ok_or_else(|| format!("keys not found in {INPUT_FILE}"))?;
    let mut policy = json["policy"].as_str().ok_or_else(|| format!("`policy` not found in {INPUT_FILE}"))?.to_owned();
    
    for (key, value) in keys.iter() {
        let placeholder = "$".to_owned() + &key.to_uppercase();
        policy = policy.replace(&placeholder, value.as_str().expect("`keys` must be a map of strings"));
    }

    Ok(policy.to_owned())
}

fn generate_output_files(wallet: &Wallet<MemoryDatabase>, segwit_policy: &Miniscript<String, Segwitv0>) -> Result<(), Box<dyn std::error::Error>> {

    // get addresses
    // Note: this code returns the same address every time unless you specify an extended key descriptor i.e. one that ends in \*
    // TODO distinguish and handle single key vs. extended key descriptors
    let mut addresses = Vec::new();
    (0..NUM_ADDRESSES).for_each(|_i: u32| {
        addresses.push(wallet.get_address(AddressIndex::New).expect("error retrieving next address").to_string())
    });

    let extern_policies = &wallet.policies(KeychainKind::External).expect("error retrieving external policies").unwrap();

    let json_output = Output {
        balance: &wallet.get_balance().expect("error retrieving balance"),
        transactions: &wallet.list_transactions(true).expect("error retrieving transactions"),
        extern_policies: &extern_policies.clone(),
        intern_policies: &wallet.policies(KeychainKind::Internal).expect("error retrieving internal policies").unwrap(),
        addresses: addresses,
        extern_descriptor: &wallet.public_descriptor(KeychainKind::External).expect("error retrieving external descriptor").unwrap(),
        intern_descriptor: &wallet.public_descriptor(KeychainKind::Internal).expect("error retrieving internal descriptor").unwrap(),
        policy_structure: &get_policy_descriptions(&extern_policies, 0),
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

pub fn description(policy: &Policy) -> String {
    match &policy.item {
        SatisfiableItem::EcdsaSignature(key) => format!("ECDSA Sig of {}", display_key(key)),
        SatisfiableItem::SchnorrSignature(key) => {
            format!("Schnorr Sig of {}", display_key(key))
        }
        SatisfiableItem::Sha256Preimage { hash } => {
            format!("SHA256 Preimage of {}", hash.to_string())
        }
        SatisfiableItem::Hash256Preimage { hash } => {
            format!("Double-SHA256 Preimage of {}", hash.to_string())
        }
        SatisfiableItem::Ripemd160Preimage { hash } => {
            format!("RIPEMD160 Preimage of {}", hash.to_string())
        }
        SatisfiableItem::Hash160Preimage { hash } => {
            format!("Double-RIPEMD160 Preimage of {}", hash.to_string())
        }
        SatisfiableItem::AbsoluteTimelock { value } => {
            format!("Absolute Timelock of {}", value.to_string())
        }
        SatisfiableItem::RelativeTimelock { value } => {
            format!("Relative Timelock of {}", value.to_string())
        }
        SatisfiableItem::Multisig { keys, threshold } => {
            format!("{} of {} MultiSig:", threshold, keys.len())
        }
        SatisfiableItem::Thresh { items, threshold } => {
            format!("{} of {} Threshold:", threshold, items.len())
        }
    }
}

pub struct DisplayKey<'a>(&'a PkOrF);

impl<'a> Display for DisplayKey<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", display_key(self.0))
    }
}

fn display_key(key: &PkOrF) -> String {
    // TODO: Use aliases
    match key {
        PkOrF::Pubkey(pk) => format!("<pk:{}>", pk.to_string()),
        PkOrF::XOnlyPubkey(pk) => format!("<xonly-pk:{}>", pk.to_string()),
        PkOrF::Fingerprint(f) => format!("<fingerprint:{}>", f.to_string()),
    }
}