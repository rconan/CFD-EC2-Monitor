# EC2 Monitor

AWS EC2 monitoring tool for computational simulation jobs running on `c8g.48xlarge` instances in the `sa-east-1` region.

## Overview

This Rust application connects to the AWS EC2 API to find running instances, SSH into each instance in parallel to check simulation progress, and generates formatted summary reports showing:

- Simulation timestep progress from `solve.out` files with median ETA calculations (displayed in days, hours, and minutes)
- CSV file counts in instance directories  
- Disk space usage
- Process status for `zcsvs`, `finalize`, and `s3 sync` workflows
- Step increase tracking between monitoring cycles

The tool refreshes every 6 minutes and processes all instances concurrently for improved performance.

## Features

- **Parallel Processing**: All instances are monitored simultaneously using async tasks
- **Custom Error Handling**: Comprehensive error types using `thiserror` for better debugging
- **Median ETA Calculations**: Tracks and displays median completion time per instance across monitoring cycles (format: days, hours, minutes)
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

### Starting Processes

Use the included `run_ec2_ps.sh` script to start any process on a specific instance:

```bash
# Make the script executable (first time only)
chmod +x run_ec2_ps.sh

# Start a process on an instance
./run_ec2_ps.sh <process_name> <instance_name>

# Examples
./run_ec2_ps.sh zcsvs zen00az180_OS_2ms
./run_ec2_ps.sh finalize zen00az180_OS_2ms
```

The script will:
1. Look up the instance's public IP address using the AWS EC2 API
2. SSH into the instance using the `AWS_KEYPAIR` environment variable
3. Start the specified process in a detached tmux session (named after the process)
4. Provide instructions for connecting to the tmux session

## Architecture

The codebase is modularized into separate modules for better organization:

### Module Structure

- **`lib.rs`**: Library interface and main monitoring cycle coordination
- **`aws.rs`**: EC2 instance discovery and AWS API interactions
- **`ssh.rs`**: SSH connection management and remote command execution  
- **`eta.rs`**: ETA calculation logic and time formatting
- **`report.rs`**: Report generation and terminal output formatting
- **`types.rs`**: Data structure definitions (`InstanceInfo`, `InstanceResults`, `TimeStep`)
- **`error.rs`**: Custom error types using `thiserror`
- **`main.rs`**: Application entry point and monitoring loop

### Core Components

- **Instance Discovery**: Uses EC2 API filters to find `c8g.48xlarge` instances in running state
- **Parallel SSH Monitoring**: Connects via SSH using `tokio::spawn` to execute remote commands concurrently
- **Custom Error Handling**: `MonitorError` enum with specific variants for different failure modes
- **Median ETA Calculation**: Tracks timestep progression and calculates median completion time per instance
- **Report Generation**: Formats and displays comprehensive status summaries with statistics

### Data Flow

1. **Initialization** (`lib.rs`): Initialize AWS SDK client for `sa-east-1` region
2. **Instance Discovery** (`aws.rs`): Query EC2 API for running instances
3. **Parallel Processing** (`lib.rs`): Launch parallel SSH connections to collect metrics from all instances
4. **SSH Monitoring** (`ssh.rs`): Execute remote commands on each instance
5. **ETA Calculation** (`eta.rs`): Calculate step increases and track ETAs for median calculation
6. **Report Generation** (`report.rs`): Generate formatted summary report with statistics
7. **Cycle Repeat**: Wait 6 minutes and repeat the monitoring cycle

### Median ETA Feature

The application tracks ETAs for each instance across multiple monitoring cycles and displays the median completion time:

- **Per-Instance Tracking**: Each instance's ETA history is maintained separately throughout the session
- **Median Calculation**: Provides a more stable completion estimate compared to current ETA snapshots
- **Data Persistence**: ETA history persists for the entire monitoring session
- **Format**: Displays in human-readable format (e.g., "2d 5h 30m", "8h 15m", "45m")
- **Reliability**: Helps identify instances with consistent vs. variable performance patterns
- **Completion Handling**: When a simulation completes (current_step = total_step), the median ETA is automatically set to "0m"

The median ETA appears as "N/A" until sufficient data points are collected for each instance. Once a simulation completes, the median ETA will display "0m" regardless of previous ETA history.

### Error Handling

The application uses custom error types for better error reporting:
- `AwsSdk`: AWS API errors
- `SshConnection`: SSH connection failures
- `NoPublicIp`: Missing public IP address
- `KeyFileNotFound`: SSH key file issues
- `SshCommandFailed`: Remote command execution failures
- And more...

## Utility Scripts

### `run_ec2_ps.sh`

A shell script for remotely starting any process on EC2 instances:

**Features:**
- Accepts process name as first argument for flexibility (zcsvs, finalize, etc.)
- Automatically looks up instance IP address by name using AWS EC2 API
- Uses `AWS_KEYPAIR` environment variable for SSH authentication
- Starts the specified process in a detached tmux session for persistence
- Names tmux session after the process being started
- Includes comprehensive error handling and user feedback

**Requirements:**
- AWS CLI configured with appropriate permissions
- `AWS_KEYPAIR` environment variable set to SSH private key path
- Target instance must be running and have a public IP address

## Dependencies

- `aws-sdk-ec2`: AWS EC2 API client
- `ssh2`: SSH connection and command execution
- `chrono`: Timestamp handling for reports
- `tokio`: Async runtime for parallel processing
- `thiserror`: Custom error type definitions