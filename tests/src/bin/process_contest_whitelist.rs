//! processes a csv file for devnet or testnet contest purposes
//!
//! You should download as csv and rename and move it according to `CSV_INPUT`.
//! The output file can subsequently be used in `bulk_send.rs` or
//! `contest_genesis.rs`

use std::collections::{btree_map::Entry, BTreeMap};

use clap::Parser;
use common::{
    contest::{RawRecord, Record},
    MODULE_ACCOUNTS,
};
use log::{info, warn};
use onomy_test_lib::super_orchestrator::{
    stacked_errors::{Result, StackableErr},
    std_init, FileOptions,
};

#[derive(Parser, Debug, Clone)]
#[command(about)]
struct Args {
    #[arg(long, default_value_t = String::from("./tests/resources/contest_whitelist.csv"))]
    pub csv_input: String,
    #[arg(long, default_value_t = String::from("./tests/resources/contest_whitelist.ron"))]
    pub ron_output: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    std_init()?;
    let args = Args::parse();

    let csv_file = FileOptions::read_to_string(args.csv_input).await.stack()?;
    // remove the header line
    let csv_file = csv_file.lines().skip(1).fold(String::new(), |mut acc, s| {
        acc.push_str(s);
        acc.push('\n');
        acc
    });

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .trim(csv::Trim::All)
        .from_reader(csv_file.as_bytes());
    let mut raw_records = vec![];
    for result in rdr.deserialize() {
        // Notice that we need to provide a type hint for automatic
        // deserialization.
        let record: RawRecord = result.stack()?;
        //println!("{:?}", record);
        raw_records.push(record);
    }

    // so when duplicates are removed, we retain the earliest record
    raw_records.sort_by(|lhs, rhs| lhs.timestamp.cmp(&rhs.timestamp));

    let mut records: BTreeMap<String, Record> = BTreeMap::new();
    for raw_record in &raw_records {
        if let Some(record) = Record::from_raw_record(raw_record) {
            match records.entry(record.addr.clone()) {
                Entry::Vacant(v) => {
                    v.insert(record);
                }
                Entry::Occupied(o) => {
                    // avoid duplicates
                    info!("duplicates:\n{:?}\n{:?}", o.get(), record);
                }
            }
        }
    }
    // make sure there are no module accounts in there
    for module_account in MODULE_ACCOUNTS {
        if records.contains_key(*module_account) {
            warn!("a module account was in the set");
            records.remove(*module_account).unwrap();
        }
    }
    drop(raw_records);
    let records: Vec<Record> = records.values().cloned().collect();
    dbg!(records.len());

    let records_s = ron::to_string(&records).stack()?;
    FileOptions::write_str(args.ron_output, &records_s)
        .await
        .stack()?;

    Ok(())
}
