#!/bin/bash

# Script to start a process on an EC2 instance via SSH
# Usage: ./run_ec2_ps.sh <process_name> <instance_name>

set -e

# Check if both process name and instance name are provided
if [ $# -ne 2 ]; then
    echo "Usage: $0 <process_name> <instance_name>"
    echo "Example: $0 zcsvs zen00az180_OS_2ms"
    echo "Example: $0 finalize zen00az180_OS_2ms"
    exit 1
fi

PROCESS_NAME="$1"
INSTANCE_NAME="$2"

# Check if AWS_KEYPAIR environment variable is set
if [ -z "$AWS_KEYPAIR" ]; then
    echo "Error: AWS_KEYPAIR environment variable is not set"
    echo "Please set it to the path of your SSH private key file"
    echo "Example: export AWS_KEYPAIR=~/Documents/AWS/awssaopaulo.pem"
    exit 1
fi

# Check if the key file exists
if [ ! -f "$AWS_KEYPAIR" ]; then
    echo "Error: SSH key file not found at $AWS_KEYPAIR"
    exit 1
fi

echo "Looking up IP address for instance: $INSTANCE_NAME"

# Get the public IP address of the instance from AWS EC2
PUBLIC_IP=$(aws ec2 describe-instances \
    --region sa-east-1 \
    --filters "Name=tag:Name,Values=$INSTANCE_NAME" "Name=instance-state-name,Values=running" \
    --query 'Reservations[*].Instances[*].PublicIpAddress' \
    --output text)

# Check if we found an IP address
if [ -z "$PUBLIC_IP" ] || [ "$PUBLIC_IP" = "None" ]; then
    echo "Error: Could not find running instance with name '$INSTANCE_NAME' in sa-east-1 region"
    echo "Make sure the instance exists, is running, and has a public IP address"
    exit 1
fi

echo "Found instance $INSTANCE_NAME with IP: $PUBLIC_IP"
echo "Starting $PROCESS_NAME process in tmux session..."

# Execute the SSH command with the retrieved IP
ssh -o StrictHostKeyChecking=no \
    -o UserKnownHostsFile=/dev/null \
    -i "$AWS_KEYPAIR" \
    ubuntu@"$PUBLIC_IP" \
    -t "tmux new-session -d -s $PROCESS_NAME \"cd $INSTANCE_NAME && sh $PROCESS_NAME\""

echo "$PROCESS_NAME process started successfully on $INSTANCE_NAME ($PUBLIC_IP)"
echo "To check the tmux session, run:"
echo "ssh -i $AWS_KEYPAIR ubuntu@$PUBLIC_IP -t 'tmux attach -t $PROCESS_NAME'"