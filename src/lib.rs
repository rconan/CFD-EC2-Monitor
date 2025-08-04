//! EC2 Monitor Library
//!
//! A library for monitoring AWS EC2 instances running computational simulation jobs.
//! Provides functionality for parallel instance monitoring, SSH command execution,
//! ETA tracking and median calculation, and formatted reporting.

use aws_config::BehaviorVersion;
use aws_sdk_ec2::Client;
use std::collections::HashMap;

pub mod aws;
pub mod error;
pub mod eta;
pub mod report;
pub mod ssh;
pub mod types;

pub use error::MonitorError;
pub use types::{InstanceInfo, InstanceResults, TimeStep};

/// Initialize AWS configuration for EC2 monitoring
pub async fn init_aws_config() -> aws_config::SdkConfig {
    aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new("sa-east-1"))
        .load()
        .await
}

/// Create AWS EC2 client
pub fn create_ec2_client(config: &aws_config::SdkConfig) -> Client {
    Client::new(config)
}

/// Run a complete monitoring cycle
pub async fn monitor_cycle(
    client: &Client,
    previous_timesteps: &mut HashMap<String, TimeStep>,
    instance_etas: &mut HashMap<String, Vec<f64>>,
) -> Result<(), MonitorError> {
    // Find all c8g.48xlarge instances
    let instances = aws::find_target_instances(client).await?;

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
            ssh::process_instance(&instance_clone).await
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
                            if let Some(previous_timestep) = previous_timesteps.get(&instance.name)
                            {
                                let step_increase =
                                    current_timestep.step.saturating_sub(previous_timestep.step);
                                current_timestep.step_increase = Some(step_increase);
                            }

                            // Calculate and store ETA
                            result.eta = current_timestep.calculate_eta();

                            // Collect ETA in minutes for median calculation per instance
                            if let Some(eta_str) = &result.eta {
                                if let Some(eta_minutes) = eta::parse_eta_to_minutes(eta_str) {
                                    instance_etas
                                        .entry(instance.name.clone())
                                        .or_insert_with(Vec::new)
                                        .push(eta_minutes);
                                }
                            }

                            // Store current timestep for next iteration
                            previous_timesteps
                                .insert(instance.name.clone(), current_timestep.clone());
                        }

                        results.push(result);
                    }
                    Err(e) => {
                        // Handle process_instance error - convert MonitorError to InstanceResults
                        let error_message = match &e {
                            MonitorError::NoPublicIp => "No public IP available".to_string(),
                            _ => format!("Processing error: {}", e),
                        };
                        results.push(InstanceResults {
                            instance_id: instance.instance_id.clone(),
                            public_ip: instance.public_ip.clone(),
                            name: instance.name.clone(),
                            connection_error: Some(error_message),
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
    report::clear_terminal();

    // Print summary report
    report::print_summary_report(&results, instance_etas)?;

    Ok(())
}
