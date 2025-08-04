use ec2_monitor::{init_aws_config, create_ec2_client, monitor_cycle, MonitorError, TimeStep};
use std::collections::HashMap;
use tokio::signal;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), MonitorError> {
    // Initialize AWS configuration
    let config = init_aws_config().await;
    let client = create_ec2_client(&config);
    let mut previous_timesteps: HashMap<String, TimeStep> = HashMap::new();
    let mut instance_etas: HashMap<String, Vec<f64>> = HashMap::new(); // Track ETAs per instance

    println!("🚀 Starting EC2 Monitor - Refreshing every 6 minutes");
    println!("Press Ctrl+C to stop monitoring\n");

    // Continuous monitoring loop
    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                println!("\n👋 Monitoring stopped by user");
                break;
            }
            _ = monitor_cycle(&client, &mut previous_timesteps, &mut instance_etas) => {
                // Sleep for 6 minutes before next cycle
                println!("\n⏰ Next update in 6 minutes...");
                sleep(Duration::from_secs(360)).await;
            }
        }
    }

    Ok(())
}
