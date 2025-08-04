# EC2 Monitor

AWS EC2 monitoring tool for computational simulation jobs running on `c8g.48xlarge` instances in the `sa-east-1` region.

## Overview

This Rust application connects to the AWS EC2 API to find running instances, SSH into each instance in parallel to check simulation progress, and generates formatted summary reports showing:

- Simulation timestep progress from `solve.out` files with ETA calculations
- CSV file counts in instance directories  
- Disk space usage
- Process status for `zcsvs`, `finalize`, and `s3 sync` workflows
- Step increase tracking between monitoring cycles

The tool refreshes every 6 minutes and processes all instances concurrently for improved performance.

## Features

- **Parallel Processing**: All instances are monitored simultaneously using async tasks
- **Custom Error Handling**: Comprehensive error types using `thiserror` for better debugging
- **ETA Calculations**: Estimates completion time based on timestep progression
- **Real-time Monitoring**: Continuous monitoring with automatic refresh every 6 minutes
- **Formatted Reports**: Clean, tabular output with color-coded status indicators

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

The application will:
1. Start monitoring and display "🚀 Starting EC2 Monitor - Refreshing every 6 minutes"
2. Find all running `c8g.48xlarge` instances
3. Process all instances in parallel 
4. Display a comprehensive summary table
5. Wait 6 minutes before the next monitoring cycle
6. Press Ctrl+C to stop monitoring

## Architecture

### Core Components

- **Instance Discovery**: Uses EC2 API filters to find `c8g.48xlarge` instances in running state
- **Parallel SSH Monitoring**: Connects via SSH using `tokio::spawn` to execute remote commands concurrently
- **Custom Error Handling**: `MonitorError` enum with specific variants for different failure modes
- **ETA Calculation**: Tracks timestep progression and estimates completion time
- **Report Generation**: Formats and displays comprehensive status summaries with statistics

### Data Flow

1. Initialize AWS SDK client for `sa-east-1` region
2. Query EC2 API for running instances
3. Launch parallel SSH connections to collect metrics from all instances
4. Calculate step increases and ETAs based on previous monitoring cycles
5. Generate formatted summary report with statistics
6. Wait 6 minutes and repeat

### Error Handling

The application uses custom error types for better error reporting:
- `AwsSdk`: AWS API errors
- `SshConnection`: SSH connection failures
- `NoPublicIp`: Missing public IP address
- `KeyFileNotFound`: SSH key file issues
- `SshCommandFailed`: Remote command execution failures
- And more...

## Dependencies

- `aws-sdk-ec2`: AWS EC2 API client
- `ssh2`: SSH connection and command execution
- `chrono`: Timestamp handling for reports
- `tokio`: Async runtime for parallel processing
- `thiserror`: Custom error type definitions