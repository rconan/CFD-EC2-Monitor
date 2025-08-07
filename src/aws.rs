//! AWS EC2 operations

use aws_sdk_ec2::{Client, types::Filter};
use crate::{MonitorError, InstanceInfo};

/// Find all target instances (c8g.48xlarge and c6g.4xlarge) in running state
pub async fn find_target_instances(client: &Client) -> Result<Vec<InstanceInfo>, MonitorError> {
    let mut instances = Vec::new();

    // Create filters for instance type and running state
    let filters = vec![
        Filter::builder()
            .name("instance-type")
            .values("c8g.48xlarge")
            .values("c6g.4xlarge")
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
        .await
        .map_err(|e| MonitorError::AwsSdk(e.to_string()))?;

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

            let instance_type = instance.instance_type()
                .map(|t| t.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let public_ip = instance.public_ip_address().map(|ip| ip.to_string());
            let private_ip = instance.private_ip_address().map(|ip| ip.to_string());

            instances.push(InstanceInfo {
                instance_id,
                name,
                instance_type,
                public_ip,
                private_ip,
            });
        }
    }

    Ok(instances)
}