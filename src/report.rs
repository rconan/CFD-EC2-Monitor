//! Report generation and terminal utilities

use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::io::{self, Write};

use crate::{MonitorError, InstanceResults};
use crate::eta::calculate_median_eta;

/// Clear terminal screen
pub fn clear_terminal() {
    // ANSI escape sequence to clear screen and move cursor to top
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().unwrap();
}

/// Print comprehensive summary report
pub fn print_summary_report(
    results: &[InstanceResults],
    instance_etas: &HashMap<String, Vec<f64>>,
) -> Result<(), MonitorError> {
    let local: DateTime<Local> = Local::now();
    println!("{}", "\n".to_string() + "=".repeat(125).as_str());
    println!("SUMMARY REPORT @ {local}");
    println!("{}", "=".repeat(125));

    // Table headers
    println!(
        "{:<20} {:^15} {:^15} {:^12} {:^15} {:<12} {:<20}",
        "Instance Name",
        "Median ETA",
        "TimeStep",
        "CSV Count",
        "Free Disk",
        "Current Process",
        "Connection Status"
    );
    println!("{}", "-".repeat(125));

    for result in results {
        let instance_name = if result.name.len() > 18 {
            format!("{}...", &result.name[..15])
        } else {
            result.name.clone()
        };

        let median_eta_display = match instance_etas.get(&result.name) {
            Some(etas) if etas.len() > 0 => match calculate_median_eta(etas) {
                Some(median) => median,
                None => "N/A".to_string(),
            },
            _ => "N/A".to_string(),
        };

        let (
            timestep_display,
            csv_count_display,
            disk_display,
            process_display,
            connection_display,
        ) = if let Some(error) = &result.connection_error {
            let error_msg = if error.len() > 18 {
                format!("{}...", &error[..15])
            } else {
                error.clone()
            };
            (
                "❌ Failed".to_string(),
                "❌ Failed".to_string(),
                "❌ Failed".to_string(),
                "❌ Failed".to_string(),
                error_msg,
            )
        } else {
            let timestep = match &result.timestep_result {
                Some(ts) => ts.to_string(),
                None => "❌ Failed".to_string(),
            };

            let csv_count = match result.csv_count {
                Some(count) => count.to_string(),
                None => "❌ Failed".to_string(),
            };

            let disk = match &result.free_disk_space {
                Some(space) => space.clone(),
                None => "❌ Failed".to_string(),
            };

            let process = match &result.current_process {
                Some(proc) => match proc.as_str() {
                    "zcsvs" => "🟢 zcsvs".to_string(),
                    "finalize" => "🟡 finalize".to_string(),
                    "s3 sync" => "🔵 s3 sync".to_string(),
                    "none" => "⚪ none".to_string(),
                    _ => proc.clone(),
                },
                None => "❌ Failed".to_string(),
            };

            (timestep, csv_count, disk, process, "✅ Success".to_string())
        };

        println!(
            "{:<20} {:>15} {:>15} {:>12} {:>15} {:<12} {:<20}",
            instance_name,
            median_eta_display,
            timestep_display,
            csv_count_display,
            disk_display,
            process_display,
            connection_display
        );
    }

    println!("{}", "-".repeat(125));

    // Summary statistics
    let total_instances = results.len();
    let successful_connections = results
        .iter()
        .filter(|r| r.connection_error.is_none())
        .count();
    let zcsvs_count = results
        .iter()
        .filter(|r| r.current_process.as_ref() == Some(&"zcsvs".to_string()))
        .count();
    let finalize_count = results
        .iter()
        .filter(|r| r.current_process.as_ref() == Some(&"finalize".to_string()))
        .count();
    let s3_sync_count = results
        .iter()
        .filter(|r| r.current_process.as_ref() == Some(&"s3 sync".to_string()))
        .count();
    let idle_count = results
        .iter()
        .filter(|r| r.current_process.as_ref() == Some(&"none".to_string()))
        .count();

    println!(
        "Summary: {} total instances | {} successful connections | {} zcsvs | {} finalize | {} s3 sync | {} idle",
        total_instances,
        successful_connections,
        zcsvs_count,
        finalize_count,
        s3_sync_count,
        idle_count
    );

    println!("{}", "=".repeat(125));
    Ok(())
}