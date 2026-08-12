//! `erebus-fees`: make a note, prove a spend, print the calldata.
//!
//! Deliberately offline. It reads the deposit set from a file rather than a
//! chain, and it prints arguments rather than sending a transaction, because the
//! whole point of the note is that whoever submits the spend does not have to be
//! the payer — and if this tool sent it, that would be the payer's IP on the
//! transaction.

use std::fs;
use std::path::PathBuf;

use ark_bn254::Fr;
use clap::{Parser, Subcommand};
use erebus_fees::{
    address_from_hex, even_split, mimc, payout_hash, proof_words, prove, setup, solidity_verifier,
    verify, Note, Tree,
};

#[derive(Parser)]
#[command(name = "erebus-fees", about = "Shielded fee notes for Erebus")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Draws a note and prints it with the commitment to deposit.
    NewNote,
    /// The commitment for a note, to check a deposit landed.
    Commitment {
        #[arg(long)]
        note: String,
    },
    /// Proves the right to spend a note and prints `FeePool.spend` arguments.
    Spend {
        #[arg(long)]
        note: String,
        /// JSON array of every commitment in the pool, deposit order.
        #[arg(long)]
        leaves: PathBuf,
        #[arg(long)]
        pool: String,
        #[arg(long, default_value_t = 31337)]
        chain_id: u64,
        /// Node payout addresses, comma separated.
        #[arg(long, value_delimiter = ',')]
        nodes: Vec<String>,
        /// Wei per node, comma separated. Defaults to an even split of
        /// `--denomination`, which is what the pool requires.
        #[arg(long, value_delimiter = ',')]
        amounts: Vec<u128>,
        /// The pool's denomination in wei, split across the nodes.
        #[arg(long, default_value_t = 10_000_000_000_000_000)]
        denomination: u128,
    },
    /// Writes the Solidity verifier for the current circuit.
    ExportVerifier {
        #[arg(long, default_value = "contracts/src/SpendVerifier.sol")]
        out: PathBuf,
    },
    /// The root of a deposit set, to compare against the pool's.
    Root {
        #[arg(long)]
        leaves: PathBuf,
    },
    /// Writes the JSON the Solidity tests replay, so the contract is checked
    /// against a proof and hashes this crate actually produced.
    Fixture {
        #[arg(long, default_value = "contracts/test/fixtures/spend.json")]
        out: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::NewNote => {
            let note = Note::random();
            println!("note       {}", note.to_hex());
            println!(
                "commitment 0x{}",
                hex::encode(mimc::field_to_be(&note.commitment()))
            );
            eprintln!("keep the note: it is the only way to spend the deposit");
        }
        Command::Commitment { note } => {
            let note = Note::from_hex(&note)?;
            println!("0x{}", hex::encode(mimc::field_to_be(&note.commitment())));
        }
        Command::Root { leaves } => {
            let tree = read_tree(&leaves)?;
            println!("leaves {}", tree.len());
            println!("root   0x{}", hex::encode(mimc::field_to_be(&tree.root())));
        }
        Command::ExportVerifier { out } => {
            let (_, vk) = setup()?;
            fs::write(&out, solidity_verifier(&vk))?;
            println!("wrote {}", out.display());
        }
        Command::Fixture { out } => {
            let json = fixture()?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, json)?;
            println!("wrote {}", out.display());
        }
        Command::Spend {
            note,
            leaves,
            pool,
            chain_id,
            nodes,
            amounts,
            denomination,
        } => {
            let note = Note::from_hex(&note)?;
            let tree = read_tree(&leaves)?;
            let pool = address_from_hex(&pool)?;
            let recipients: Vec<[u8; 20]> = nodes
                .iter()
                .map(|n| address_from_hex(n))
                .collect::<Result<_, _>>()?;
            let amounts = if amounts.is_empty() {
                even_split(denomination, recipients.len())
            } else {
                amounts
            };

            let payout = payout_hash(chain_id, pool, &recipients, &amounts)?;
            let (pk, vk) = setup()?;
            let spend = prove(&pk, &tree, &note, payout)?;
            verify(&vk, &spend)?;

            let words = proof_words(&spend.proof);
            println!(
                "root          0x{}",
                hex::encode(mimc::field_to_be(&spend.root))
            );
            println!(
                "nullifierHash 0x{}",
                hex::encode(mimc::field_to_be(&spend.nullifier_hash))
            );
            println!(
                "payout        0x{}",
                hex::encode(mimc::field_to_be(&payout))
            );
            println!(
                "amounts       [{}]",
                amounts
                    .iter()
                    .map(u128::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            println!("proof         [{}]", words.join(","));
        }
    }
    Ok(())
}

fn read_tree(path: &PathBuf) -> Result<Tree, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    let raw: Vec<String> = serde_json::from_str(&text)?;
    let mut leaves = Vec::with_capacity(raw.len());
    for entry in raw {
        let bytes = hex::decode(entry.trim().trim_start_matches("0x"))?;
        let mut word = [0u8; 32];
        if bytes.len() != 32 {
            return Err("a commitment is 32 bytes".into());
        }
        word.copy_from_slice(&bytes);
        leaves.push(field_or_fail(&word)?);
    }
    Ok(Tree::from_leaves(leaves))
}

fn field_or_fail(word: &[u8; 32]) -> Result<Fr, Box<dyn std::error::Error>> {
    mimc::field_from_be(word).ok_or_else(|| "a commitment has to be a reduced field element".into())
}

/// Chain id, pool address, and denomination the fixture is built for. The pool
/// address is in the payout preimage, so the Solidity test has to deploy there.
const FIXTURE_CHAIN_ID: u64 = 31337;
const FIXTURE_POOL: &str = "0x00000000000000000000000000000000000f0001";
const FIXTURE_DENOMINATION: u128 = 10_000_000_000_000_000; // 0.01 ether

fn fixture() -> Result<String, Box<dyn std::error::Error>> {
    let notes: Vec<Note> = (0..4).map(|_| Note::random()).collect();
    let tree = Tree::from_leaves(notes.iter().map(Note::commitment).collect());
    let spent = &notes[2];

    let pool = address_from_hex(FIXTURE_POOL)?;
    let recipients = [
        address_from_hex("0x00000000000000000000000000000000000000e1")?,
        address_from_hex("0x00000000000000000000000000000000000000e2")?,
        address_from_hex("0x00000000000000000000000000000000000000e3")?,
    ];
    let amounts = even_split(FIXTURE_DENOMINATION, recipients.len());

    let payout = payout_hash(FIXTURE_CHAIN_ID, pool, &recipients, &amounts)?;
    let (pk, vk) = setup()?;
    let spend = prove(&pk, &tree, spent, payout)?;
    verify(&vk, &spend)?;

    let word = |value: &Fr| format!("0x{}", hex::encode(mimc::field_to_be(value)));
    let commitments: Vec<String> = notes.iter().map(|n| word(&n.commitment())).collect();

    let json = serde_json::json!({
        "chainId": FIXTURE_CHAIN_ID,
        "pool": FIXTURE_POOL,
        "denomination": FIXTURE_DENOMINATION.to_string(),
        "commitments": commitments,
        "spentIndex": 2,
        "recipients": recipients.iter().map(|r| format!("0x{}", hex::encode(r))).collect::<Vec<_>>(),
        "amounts": amounts.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
        "root": word(&spend.root),
        "nullifierHash": word(&spend.nullifier_hash),
        "payout": word(&payout),
        "proof": proof_words(&spend.proof),
        "mimc": {
            "zeroOne": word(&mimc::hash(Fr::from(0u64), Fr::from(1u64))),
            "oneTwo": word(&mimc::hash(Fr::from(1u64), Fr::from(2u64))),
            "emptyLeaf": word(&erebus_fees::merkle::zeros()[0]),
            "emptyRoot": word(&erebus_fees::merkle::zeros()[erebus_fees::DEPTH]),
        }
    });

    Ok(serde_json::to_string_pretty(&json)? + "\n")
}
