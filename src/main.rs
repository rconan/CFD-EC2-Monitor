use aws_config::BehaviorVersion;
use aws_sdk_ec2::{Client, types::Filter};
use chrono::{DateTime, Local};
use ssh2::Session;
use std::env;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;
use tokio;

#[derive(Debug, Default)]
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
    timestep_result: Option<String>,
    csv_count: Option<i32>,
    free_disk_space: Option<String>,
    zcsvs_status: Option<bool>,
    finalize_status: Option<bool>,
    s3_sync_status: Option<bool>,
    connection_error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize AWS configuration
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new("sa-east-1"))
        .load()
        .await;

    let client = Client::new(&config);

    // Find all c8g.48xlarge instances
    let instances = find_target_instances(&client).await?;

    if instances.is_empty() {
        println!("No c8g.48xlarge instances found in sa-east-1 region");
        return Ok(());
    }

    println!("Found {} c8g.48xlarge instances:", instances.len());

    // Process each instance
    let mut results = Vec::new();
    for instance in instances {
        println!(
            "Processing instance: {} ({})",
            instance.name, instance.instance_id
        );

        let result = process_instance(&instance).await;
        results.push(result);
    }

    // Print summary report
    print_summary_report(&results);

    Ok(())
}

async fn find_target_instances(
    client: &Client,
) -> Result<Vec<InstanceInfo>, Box<dyn std::error::Error>> {
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

async fn process_instance(instance: &InstanceInfo) -> InstanceResults {
    let ip = match &instance.public_ip {
        Some(ip) => ip,
        None => {
            return InstanceResults {
                instance_id: instance.instance_id.clone(),
                public_ip: instance.public_ip.clone(),
                name: instance.name.clone(),
                connection_error: Some("No public IP available".to_string()),
                ..Default::default()
            };
        }
    };

    match connect_and_execute_commands(ip, &instance.name).await {
        Ok((timestep, csv_count, disk_space, zcsvs_running, finalize_running, s3_sync_running)) => {
            InstanceResults {
                instance_id: instance.instance_id.clone(),
                public_ip: instance.public_ip.clone(),
                name: instance.name.clone(),
                timestep_result: Some(timestep),
                csv_count: Some(csv_count),
                free_disk_space: Some(disk_space),
                zcsvs_status: Some(zcsvs_running),
                finalize_status: Some(finalize_running),
                s3_sync_status: Some(s3_sync_running),
                ..Default::default()
            }
        }
        Err(e) => InstanceResults {
            instance_id: instance.instance_id.clone(),
            public_ip: instance.public_ip.clone(),
            name: instance.name.clone(),
            connection_error: Some(e.to_string()),
            ..Default::default()
        },
    }
}

async fn connect_and_execute_commands(
    ip: &str,
    instance_name: &str,
) -> Result<(String, i32, String, bool, bool, bool), Box<dyn std::error::Error>> {
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

    // Check if zcsvs process is running
    let zcsvs_check = execute_ssh_command(&sess, "ps aux | grep '[z]csvs' | grep -v grep")?;
    let zcsvs_running = !zcsvs_check.is_empty();
    // Check if finalize process is running
    let finalize_check = execute_ssh_command(&sess, "ps aux | grep '[f]inalize' | grep -v grep")?;
    let finalize_running = !finalize_check.is_empty();
    // Check if finalize process is running
    let s3_sync_check = execute_ssh_command(&sess, "ps aux | grep '[s]3 sync' | grep -v grep")?;
    let s3_sync_running = !s3_sync_check.is_empty();

    Ok((
        timestep_result,
        csv_count,
        disk_space,
        zcsvs_running,
        finalize_running,
        s3_sync_running,
    ))
}

fn execute_ssh_command(
    sess: &Session,
    command: &str,
) -> Result<String, Box<dyn std::error::Error>> {
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

fn print_summary_report(results: &[InstanceResults]) -> Result<(), Box<dyn std::error::Error>> {
    let local: DateTime<Local> = Local::now();
    println!("{}", "\n".to_string() + "=".repeat(120).as_str());
    println!("SUMMARY REPORT @ {local}");
    println!("{}", "=".repeat(120));

    // Table headers
    println!(
        "{:<20} {:^15} {:^15} {:^12} {:^15} {:<12} {:<20}",
        "Instance Name",
        "Public IP",
        "TimeStep",
        "CSV Count",
        "Free Disk",
        "zcsvs Status",
        "Connection Status"
    );
    println!("{}", "-".repeat(120));

    for result in results {
        let instance_name = if result.name.len() > 18 {
            format!("{}...", &result.name[..15])
        } else {
            result.name.clone()
        };

        let public_ip = match &result.public_ip {
            Some(ip) => ip,
            None => "-",
        };
        // let public_ip = if result.public_ip.len() > 13 {
        //     format!("{}...", &result.public_ip[..10])
        // } else {
        //     result.public_ip.clone()
        // };

        let (timestep_display, csv_count_display, disk_display, zcsvs_display, connection_display) =
            if let Some(error) = &result.connection_error {
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
                    Some(ts) => {
                        let i = ts.find(':').unwrap();
                        let (a, b) = ts.split_at(i);
                        format!("({}){:8.2}", &a[10..], &b[6..].trim().parse::<f64>()?)
                        // if ts.len() > 33 {
                        //     format!("{}...", &ts[10..30])
                        // } else {
                        //     (&ts[10..]).to_string()
                        // }
                    }
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

                let zcsvs = match result.zcsvs_status {
                    Some(true) => "🟢 Running".to_string(),
                    Some(false) => "🔴 Stopped".to_string(),
                    None => "❌ Failed".to_string(),
                };

                (timestep, csv_count, disk, zcsvs, "✅ Success".to_string())
            };

        println!(
            "{:<20} {:>15} {:>15} {:>12} {:>15} {:<12} {:<20}",
            instance_name,
            public_ip,
            timestep_display,
            csv_count_display,
            disk_display,
            zcsvs_display,
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
    let running_zcsvs = results
        .iter()
        .filter(|r| r.zcsvs_status == Some(true))
        .count();
    let stopped_zcsvs = results
        .iter()
        .filter(|r| r.zcsvs_status == Some(false))
        .count();

    println!(
        "Summary: {} total instances | {} successful connections | {} running zcsvs | {} stopped zcsvs",
        total_instances, successful_connections, running_zcsvs, stopped_zcsvs
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
