//! SSH operations for remote instance monitoring

use ssh2::Session;
use std::env;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::Path;

use crate::{MonitorError, InstanceInfo, InstanceResults, TimeStep};

/// Process a single instance by SSH connection and command execution
pub async fn process_instance(instance: &InstanceInfo) -> Result<InstanceResults, MonitorError> {
    let ip = match &instance.public_ip {
        Some(ip) => ip,
        None => {
            return Err(MonitorError::NoPublicIp);
        }
    };

    match connect_and_execute_commands(ip, &instance.name).await {
        Ok((timestep, csv_count, disk_space, current_process)) => Ok(InstanceResults {
            instance_id: instance.instance_id.clone(),
            public_ip: instance.public_ip.clone(),
            name: instance.name.clone(),
            timestep_result: Some(TimeStep::new(&instance.name, &timestep)?),
            csv_count: Some(csv_count),
            free_disk_space: Some(disk_space),
            current_process: Some(current_process),
            ..Default::default()
        }),
        Err(e) => Err(e),
    }
}

/// Connect to instance via SSH and execute monitoring commands
async fn connect_and_execute_commands(
    ip: &str,
    instance_name: &str,
) -> Result<(String, i32, String, String), MonitorError> {
    // Connect to SSH
    let tcp = TcpStream::connect(format!("{}:22", ip))?;
    let mut sess = Session::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    // Authenticate with key pair
    let keypair = env::var("AWS_KEYPAIR")?;
    let key_path = Path::new(&keypair);
    if !key_path.exists() {
        return Err(MonitorError::KeyFileNotFound { path: keypair });
    }

    // Try common usernames for different AMI types
    let username = "ubuntu";
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

/// Execute a command via SSH session
fn execute_ssh_command(sess: &Session, command: &str) -> Result<String, MonitorError> {
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
            return Err(MonitorError::SshCommandFailed {
                code: exit_status,
                stderr,
            });
        }
    }

    Ok(output.trim().to_string())
}
