//! for calculations of UTC times

use std::{str::FromStr, time::Duration};

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use clap::Parser;
use onomy_test_lib::super_orchestrator::{
    stacked_errors::{Result, StackableErr},
    std_init,
};

#[derive(Parser, Debug)]
#[command(about)]
struct Args {}

#[tokio::main]
async fn main() -> Result<()> {
    std_init()?;
    let _args = Args::parse();

    let local_target_time: DateTime<Tz> = chrono_tz::US::Central
        .with_ymd_and_hms(2023, 9, 20, 10, 0, 0)
        .single()
        .stack()?;
    let utc_target_time = local_target_time.with_timezone(&Utc);
    let formatted_utc_target_time =
        &utc_target_time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let blocks_per_year: u64 = 6311520;
    let current_height: u64 = 70817;
    let current_time: std::result::Result<DateTime<Utc>, _> =
        DateTime::from_str("2023-09-17T21:00:48.00Z");
    let current_time = current_time.stack()?;
    let time_diff_chrono = utc_target_time - current_time;
    let time_diff = time_diff_chrono.to_std().stack()?;

    println!(
        "CURRENT TIME: {}",
        current_time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    );
    println!("TARGET TIME: {formatted_utc_target_time}");

    println!("TIME DIFF: {} hours", time_diff_chrono.num_hours());

    let blocks_per_ms = (blocks_per_year as f64) / (365.0 * 24.0 * 60.0 * 60.0 * 1000.0);
    let time_diff_ms = time_diff.as_millis() as f64;
    let blocks_to_wait = blocks_per_ms * time_diff_ms;
    let blocks_to_wait = blocks_to_wait as u64;
    let target_height_estimate = current_height + blocks_to_wait;
    println!("CURRENT HEIGHT: {current_height}");
    println!("TARGET HEIGHT ESTIMATE: {target_height_estimate}");

    let duration = Duration::from_millis(((blocks_to_wait as f64) / blocks_per_ms) as u64);
    let duration = chrono::Duration::from_std(duration).stack()?;
    let estimate_reach_time = current_time.checked_add_signed(duration).unwrap();
    println!("ESTIMATE REACH TIME: {estimate_reach_time}");

    Ok(())
}
