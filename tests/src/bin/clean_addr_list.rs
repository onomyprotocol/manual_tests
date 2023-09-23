use common::contest::{RawRecord, Record};
use onomy_test_lib::{
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Result, StackableErr},
        FileOptions,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    let _args = onomy_std_init()?;

    let csv_file = FileOptions::read_to_string("./tests/resources/onex-testnet-trade-war.csv")
        .await
        .stack()?;
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

    let mut records: Vec<Record> = vec![];
    for raw_record in &raw_records {
        if let Some(record) = Record::from_raw_record(&raw_record) {
            records.push(record);
        }
    }
    drop(raw_records);
    dbg!(records.len());

    let records_s = ron::to_string(&records).stack()?;
    FileOptions::write_str(
        "./tests/resources/onex-testnet-trade-war-filtered.csv",
        &records_s,
    )
    .await
    .stack()?;

    Ok(())
}
