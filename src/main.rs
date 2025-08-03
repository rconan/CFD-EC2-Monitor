use aws_config::BehaviorVersion;
use aws_sdk_ec2::{Client, types::Filter};
use chrono::{DateTime, Local};
use ssh2::Session;
use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use tokio;
use tokio::signal;
use tokio::time::{Duration, sleep};

#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
struct InstanceInfo {
    instance_id: String,
    name: String,
    public_ip: Option<String>,
    private_ip: Option<String>,
}

#[derive(Debug, Default)]
struct InstanceResults {
    instance_id: String,
    public_ip: Option<String>,
    name: String,
    timestep_result: Option<TimeStep>,
    csv_count: Option<i32>,
    free_disk_space: Option<String>,
    current_process: Option<String>,
    eta: Option<String>,
    connection_error: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct TimeStep {
    step: usize,
    time: f64,
    total_step: usize,
    step_increase: Option<usize>,
}
impl TimeStep {
    pub fn new(case: &str, time_step: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let i = time_step.find(':').unwrap();
        let (a, b) = time_step.split_at(i);
        Ok(Self {
            step: a[10..].trim().parse::<usize>()?,
            time: b[6..].trim().parse::<f64>()?,
            total_step: match case.split('_').last().unwrap() {
                "2ms" => Ok(24_000),
                "7ms" | "12ms" | "17ms" => Ok(18_000),
                x => Err(format!(
                    "valid wind speeds are 2m/s, 7m/s, 12m/s or 17m/s, found {}m/s",
                    x
                )),
            }?,
            step_increase: None,
        })
    }

    pub fn calculate_eta(&self) -> Option<String> {
        // Only calculate ETA if we have a step increase (not the first run)
        if let Some(step_increase) = self.step_increase {
            if step_increase > 0 {
                let remaining_steps = self.total_step.saturating_sub(self.step);
                if remaining_steps == 0 {
                    return Some("Complete".to_string());
                }

                // Calculate minutes needed based on current rate
                // step_increase is over 2 minutes, so minutes per step = 2 / step_increase
                let minutes_per_step = 2.0 / step_increase as f64;
                let total_minutes = remaining_steps as f64 * minutes_per_step;

                // Convert to hours and minutes
                let hours = (total_minutes / 60.0).floor() as u64;
                let minutes = (total_minutes % 60.0).round() as u64;

                if hours > 0 {
                    return Some(format!("{}h {}m", hours, minutes));
                } else {
                    return Some(format!("{}m", minutes));
                }
            } else {
                return Some("Stalled".to_string());
            }
        }

        None // First run, no ETA available
    }
}

impl Display for TimeStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(inc) = self.step_increase {
            write!(f, "(+{:}){:8.2}", inc, self.time)
        } else {
            write!(f, "({}){:8.2}", self.step, self.time)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize AWS configuration
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new("sa-east-1"))
        .load()
        .await;

    let client = Client::new(&config);
    let mut previous_timesteps: HashMap<String, TimeStep> = HashMap::new();

    println!("🚀 Starting EC2 Monitor - Refreshing every 2 minutes");
    println!("Press Ctrl+C to stop monitoring\n");

    // Clear terminal for clean display
    clear_terminal();

    // Continuous monitoring loop
    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                println!("\n👋 Monitoring stopped by user");
                break;
            }
            _ = monitor_cycle(&client, &mut previous_timesteps) => {
                // Sleep for 2 minutes before next cycle
                println!("\n⏰ Next update in 2 minutes...");
                sleep(Duration::from_secs(120)).await;
            }
        }
    }

    Ok(())
}

async fn monitor_cycle(
    client: &Client,
    previous_timesteps: &mut HashMap<String, TimeStep>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Find all c8g.48xlarge instances
    let instances = find_target_instances(&client).await?;

    if instances.is_empty() {
        println!("No c8g.48xlarge instances found in sa-east-1 region");
        return Ok(());
    }

    println!("🔍 Found {} c8g.48xlarge instances:", instances.len());
    println!("🚀 Processing all instances in parallel...");

    // Process all instances in parallel using tokio::spawn
    let mut tasks = Vec::new();
    for instance in instances {
        let instance_clone = instance.clone();
        let task = tokio::spawn(async move {
            println!(
                "Processing instance: {} ({})",
                instance_clone.name, instance_clone.instance_id
            );
            process_instance(&instance_clone).await
        });
        tasks.push((instance, task));
    }

    // Wait for all tasks to complete and collect results
    let mut results = Vec::new();
    for (instance, task) in tasks {
        match task.await {
            Ok(process_result) => {
                match process_result {
                    Ok(mut result) => {
                        // Calculate step increase if we have a previous timestep
                        if let Some(current_timestep) = &mut result.timestep_result {
                            if let Some(previous_timestep) = previous_timesteps.get(&instance.name) {
                                let step_increase = current_timestep.step.saturating_sub(previous_timestep.step);
                                current_timestep.step_increase = Some(step_increase);
                            }

                            // Calculate and store ETA
                            result.eta = current_timestep.calculate_eta();

                            // Store current timestep for next iteration
                            previous_timesteps.insert(instance.name.clone(), current_timestep.clone());
                        }

                        results.push(result);
                    }
                    Err(e) => {
                        // Handle process_instance error
                        results.push(InstanceResults {
                            instance_id: instance.instance_id.clone(),
                            public_ip: instance.public_ip.clone(),
                            name: instance.name.clone(),
                            connection_error: Some(format!("Processing error: {}", e)),
                            ..Default::default()
                        });
                    }
                }
            }
            Err(e) => {
                // Handle tokio task join error
                results.push(InstanceResults {
                    instance_id: instance.instance_id.clone(),
                    public_ip: instance.public_ip.clone(),
                    name: instance.name.clone(),
                    connection_error: Some(format!("Task error: {}", e)),
                    ..Default::default()
                });
            }
        }
    }

    // Clear terminal for clean display
    clear_terminal();

    // Print summary report
    print_summary_report(&results)?;

    Ok(())
}

fn clear_terminal() {
    // ANSI escape sequence to clear screen and move cursor to top
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap();
}

async fn find_target_instances(
    client: &Client,
) -> Result<Vec<InstanceInfo>, Box<dyn std::error::Error + Send + Sync>> {
    let mut instances = Vec::new();

    // Create filters for instance type and running state
    let filters = vec![
        Filter::builder()
            .name("instance-type")
            .values("c8g.48xlarge")
            .build(),
        Filter::builder()
            .name("instance-state-name")
            .values("running")
            .build(),
    ];

    let resp = client
        .describe_instances()
        .set_filters(Some(filters))
        .send()
        .await?;

    for reservation in resp.reservations() {
        for instance in reservation.instances() {
            let instance_id = instance.instance_id().unwrap_or("unknown").to_string();

            // Extract instance name from tags
            let name = instance
                .tags()
                .iter()
                .find(|tag| tag.key().unwrap_or("") == "Name")
                .and_then(|tag| tag.value())
                .unwrap_or(&instance_id)
                .to_string();

            let public_ip = instance.public_ip_address().map(|ip| ip.to_string());
            let private_ip = instance.private_ip_address().map(|ip| ip.to_string());

            instances.push(InstanceInfo {
                instance_id,
                name,
                public_ip,
                private_ip,
            });
        }
    }

    Ok(instances)
}

async fn process_instance(
    instance: &InstanceInfo,
) -> Result<InstanceResults, Box<dyn std::error::Error + Send + Sync>> {
    let ip = match &instance.public_ip {
        Some(ip) => ip,
        None => {
            return Ok(InstanceResults {
                instance_id: instance.instance_id.clone(),
                public_ip: instance.public_ip.clone(),
                name: instance.name.clone(),
                connection_error: Some("No public IP available".to_string()),
                ..Default::default()
            });
        }
    };

    Ok(
        match connect_and_execute_commands(ip, &instance.name).await {
            Ok((timestep, csv_count, disk_space, current_process)) => InstanceResults {
                instance_id: instance.instance_id.clone(),
                public_ip: instance.public_ip.clone(),
                name: instance.name.clone(),
                timestep_result: Some(TimeStep::new(&instance.name, &timestep)?),
                csv_count: Some(csv_count),
                free_disk_space: Some(disk_space),
                current_process: Some(current_process),
                ..Default::default()
            },
            Err(e) => InstanceResults {
                instance_id: instance.instance_id.clone(),
                public_ip: instance.public_ip.clone(),
                name: instance.name.clone(),
                connection_error: Some(e.to_string()),
                ..Default::default()
            },
        },
    )
}

async fn connect_and_execute_commands(
    ip: &str,
    instance_name: &str,
) -> Result<(String, i32, String, String), Box<dyn std::error::Error + Send + Sync>> {
    // Connect to SSH
    let tcp = TcpStream::connect(format!("{}:22", ip))?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    // Authenticate with key pair
    let keypair = env::var("AWS_KEYPAIR")?;
    let key_path = Path::new(&keypair);
    if !key_path.exists() {
        return Err(format!("SSH key file {key_path:?} not found").into());
    }

    // Try common usernames for different AMI types
    let username = "ubuntu";
    // let mut authenticated = false;

    // if sess
    //     .userauth_pubkey_file(username, None, key_path, None)
    //     .is_ok()
    // {
    //     authenticated = true;
    // }

    // if !authenticated {
    //     return Err("SSH authentication failed with all common usernames".into());
    // }
    sess.userauth_pubkey_file(username, None, key_path, None)?;

    // Execute commands
    let timestep_result = execute_ssh_command(
        &sess,
        &format!("grep TimeStep {}/solve.out | tail -n1", instance_name),
    )?;
    let csv_count_str = execute_ssh_command(&sess, &format!("ls {}/*.csv | wc -l", instance_name))?;
    let csv_count = csv_count_str.trim().parse::<i32>().unwrap_or(0);
    let disk_space = execute_ssh_command(&sess, "df -h / | tail -n1 | awk '{print $4}'")?;

    // Check which process is currently running (priority: s3 sync > finalize > zcsvs)
    let s3_sync_check = execute_ssh_command(&sess, "ps aux | grep '[s]3 sync' | grep -v grep")?;
    let finalize_check = execute_ssh_command(&sess, "ps aux | grep '[f]inalize' | grep -v grep")?;
    let zcsvs_check = execute_ssh_command(&sess, "ps aux | grep '[z]csvs' | grep -v grep")?;

    let current_process = if !s3_sync_check.is_empty() {
        "s3 sync".to_string()
    } else if !finalize_check.is_empty() {
        "finalize".to_string()
    } else if !zcsvs_check.is_empty() {
        "zcsvs".to_string()
    } else {
        "none".to_string()
    };

    Ok((timestep_result, csv_count, disk_space, current_process))
}

fn execute_ssh_command(
    sess: &Session,
    command: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut channel = sess.channel_session()?;
    channel.exec(command)?;

    let mut output = String::new();
    channel.read_to_string(&mut output)?;

    channel.wait_close()?;
    let exit_status = channel.exit_status()?;

    if exit_status != 0 {
        let mut stderr = String::new();
        channel.stderr().read_to_string(&mut stderr)?;
        if !stderr.trim().is_empty() {
            return Err(
                format!("Command failed with exit code {}: {}", exit_status, stderr).into(),
            );
        }
    }

    Ok(output.trim().to_string())
}

fn print_summary_report(results: &[InstanceResults]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let local: DateTime<Local> = Local::now();
    println!("{}", "\n".to_string() + "=".repeat(120).as_str());
    println!("SUMMARY REPORT @ {local}");
    println!("{}", "=".repeat(120));

    // Table headers
    println!(
        "{:<20} {:^15} {:^15} {:^12} {:^15} {:<12} {:<20}",
        "Instance Name",
        "ETA",
        "TimeStep",
        "CSV Count",
        "Free Disk",
        "Current Process",
        "Connection Status"
    );
    println!("{}", "-".repeat(120));

    for result in results {
        let instance_name = if result.name.len() > 18 {
            format!("{}...", &result.name[..15])
        } else {
            result.name.clone()
        };

        let eta_display = match &result.eta {
            Some(eta) => eta.as_str(),
            None => "Calculating...",
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
            eta_display,
            timestep_display,
            csv_count_display,
            disk_display,
            process_display,
            connection_display
        );
    }

    println!("{}", "-".repeat(120));

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

    println!("{}", "=".repeat(120));
    Ok(())
}
/* fn print_summary_report(results: &[InstanceResults]) {
>>>>>>> Conflict 1 of 1 ends
    println!("{}", "\n".to_string() + "=".repeat(80).as_str());
    println!("SUMMARY REPORT @ {local}");
    println!("{}", "=".repeat(80));

    for result in results {
        println!(
            "\nInstance: {} ({})",
            result.name,
            result.public_ip.as_ref().unwrap_or(&result.instance_id)
        );
        println!("{}", "-".repeat(50));

        if let Some(error) = &result.connection_error {
            println!("❌ Connection Error: {}", error);
            continue;
        }

        if let Some(timestep) = &result.timestep_result {
            println!("🕐 TimeStep Result: {}", timestep);
        } else {
            println!("❌ TimeStep Result: Failed to retrieve");
        }

        if let Some(csv_count) = result.csv_count {
            println!("📊 CSV Files Count: {}", csv_count);
        } else {
            println!("❌ CSV Files Count: Failed to retrieve");
        }

        // if let Some(zcsvs_running) = result.zcsvs_status {
        //     if zcsvs_running {
        //         println!("🟢 zcsvs Process: Running");
        //     } else {
        //         println!("🔴 zcsvs Process: Not running");
        //     }
        // } else {
        //     println!("❌ zcsvs Process: Failed to check");
        // }
        if let Some(zcsvs_running) = result.zcsvs_status
            && zcsvs_running
        {
            println!("🟢 zcsvs Process: Running");
        }
        if let Some(finalize_running) = result.finalize_status
            && finalize_running
        {
            println!("🟢 finalize Process: Running");
        }
        if let Some(s3_sync_running) = result.s3_sync_status
            && s3_sync_running
        {
            println!("🟢 s3 sync Process: Running");
        }

        if let Some(disk_space) = &result.free_disk_space {
            println!("💾 Free Disk Space: {}", disk_space);
        } else {
            println!("❌ Free Disk Space: Failed to retrieve");
        }
    }

    println!("{}", "\n".to_owned() + "=".repeat(80).as_str());
    println!("Report completed for {} instances", results.len());
} */
