# EC2 Monitor

AWS EC2 monitoring tool for computational simulation jobs running on `c8g.48xlarge` instances in the `sa-east-1` region.

## Overview

This Rust application connects to the AWS EC2 API to find running instances, SSH into each instance to check simulation progress, and generates formatted summary reports showing:

- Simulation timestep progress from `solve.out` files
- CSV file counts in instance directories  
- Disk space usage
- Process status for `zcsvs`, `finalize`, and `s3 sync` workflows

## Prerequisites

- AWS credentials configured (via AWS CLI, environment variables, or IAM roles)
- SSH private key for instance authentication
- Rust toolchain installed

## Environment Variables

```bash
export AWS_KEYPAIR=/path/to/your/ssh/private/key.pem
```

## Usage

```bash
# Build and run
cargo build
cargo run

# Development commands
cargo check    # Check code without building
cargo clippy   # Run linter  
cargo fmt      # Format code

# Release build
cargo build --release
```

## Architecture

### Core Components

- **Instance Discovery**: Uses EC2 API filters to find `c8g.48xlarge` instances in running state
- **SSH Monitoring**: Connects via SSH to execute remote commands and gather metrics
- **Report Generation**: Formats and displays comprehensive status summaries

### Data Flow

1. Initialize AWS SDK client for `sa-east-1` region
2. Query EC2 API for running instances
3. SSH into each instance to collect metrics
4. Generate formatted summary report

## Dependencies

- `aws-sdk-ec2`: AWS EC2 API client
- `ssh2`: SSH connection and command execution
- `chrono`: Timestamp handling
- `tokio`: Async runtime