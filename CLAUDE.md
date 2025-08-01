# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an AWS EC2 monitoring tool written in Rust that monitors the status of computational simulation jobs running on specific EC2 instances. The tool connects to AWS EC2 API, finds running `c8g.48xlarge` instances in the `sa-east-1` region, SSH into each instance to check simulation progress, and generates formatted summary reports.

## Development Commands

### Build and Run
```bash
cargo build                    # Build the project
cargo run                      # Run the monitoring tool
cargo build --release          # Build optimized release version
```

### Code Quality
```bash
cargo check                    # Check code without building
cargo clippy                   # Run linter
cargo fmt                      # Format code
```

## Architecture

### Core Components

**Main Application Flow** (`main.rs:34-70`):
1. Initialize AWS SDK client for `sa-east-1` region
2. Query EC2 API for running `c8g.48xlarge` instances
3. Process each instance via SSH to gather metrics
4. Generate and display summary report

**Instance Discovery** (`main.rs:72-121`):
- Uses AWS EC2 filters to find specific instance types in running state
- Extracts instance metadata (ID, name from tags, public/private IPs)

**SSH Monitoring** (`main.rs:162-222`):
- Connects via SSH using keypair from `AWS_KEYPAIR` environment variable
- Executes remote commands to check:
  - Simulation timestep progress from `solve.out` files
  - CSV file counts in instance directories
  - Disk space usage
  - Process status for `zcsvs`, `finalize`, and `s3 sync` workflows

**Data Structures**:
- `InstanceInfo`: Basic EC2 instance metadata
- `InstanceResults`: Complete monitoring results including connection status and metrics

### Environment Requirements

- `AWS_KEYPAIR`: Path to SSH private key file for instance authentication
- AWS credentials configured (via AWS CLI, environment variables, or IAM roles)

### Dependencies

- `aws-sdk-ec2`: AWS EC2 API client
- `ssh2`: SSH connection and command execution
- `chrono`: Timestamp handling for reports
- `tokio`: Async runtime