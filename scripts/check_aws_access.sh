#!/bin/bash
# Script to check and configure AWS security groups for external access

set -e

# Configuration
GRPC_PORT=${GRPC_PORT:-50053}
REGION=${AWS_REGION:-us-east-1}

echo "=== AWS Security Group Check for gRPC Access ==="
echo "Port: $GRPC_PORT"
echo "Region: $REGION"
echo

# Get instance ID and security groups
INSTANCE_ID=$(ec2-metadata --instance-id | cut -d' ' -f2)
echo "Instance ID: $INSTANCE_ID"

# Get security groups
SECURITY_GROUPS=$(aws ec2 describe-instances \
    --instance-ids $INSTANCE_ID \
    --region $REGION \
    --query 'Reservations[0].Instances[0].SecurityGroups[*].[GroupId,GroupName]' \
    --output text)

echo -e "\nSecurity Groups:"
echo "$SECURITY_GROUPS"

# Check current rules for the gRPC port
echo -e "\n=== Current Inbound Rules for Port $GRPC_PORT ==="

while IFS=$'\t' read -r sg_id sg_name; do
    echo -e "\nChecking $sg_name ($sg_id)..."
    
    RULES=$(aws ec2 describe-security-groups \
        --group-ids $sg_id \
        --region $REGION \
        --query "SecurityGroups[0].IpPermissions[?FromPort==\`$GRPC_PORT\`]" \
        --output json)
    
    if [ "$RULES" = "[]" ]; then
        echo "  ❌ No rules found for port $GRPC_PORT"
    else
        echo "  ✓ Rules found:"
        echo "$RULES" | jq -r '.[] | "    - From: \(.IpRanges[0].CidrIp // .UserIdGroupPairs[0].GroupId // "Unknown")"'
    fi
done <<< "$SECURITY_GROUPS"

# Get VPC info
VPC_ID=$(aws ec2 describe-instances \
    --instance-ids $INSTANCE_ID \
    --region $REGION \
    --query 'Reservations[0].Instances[0].VpcId' \
    --output text)

VPC_CIDR=$(aws ec2 describe-vpcs \
    --vpc-ids $VPC_ID \
    --region $REGION \
    --query 'Vpcs[0].CidrBlock' \
    --output text)

echo -e "\n=== Network Information ==="
echo "VPC ID: $VPC_ID"
echo "VPC CIDR: $VPC_CIDR"

# Get public IP
PUBLIC_IP=$(ec2-metadata --public-ipv4 | cut -d' ' -f2 2>/dev/null || echo "N/A")
PRIVATE_IP=$(ec2-metadata --local-ipv4 | cut -d' ' -f2)

echo "Private IP: $PRIVATE_IP"
echo "Public IP: $PUBLIC_IP"

# Provide recommendations
echo -e "\n=== Recommendations ==="

if [ "$RULES" = "[]" ]; then
    SG_ID=$(echo "$SECURITY_GROUPS" | head -n1 | cut -f1)
    
    echo "To allow access from other EC2 instances, run one of these commands:"
    echo
    echo "1. Allow from specific security group (recommended):"
    echo "   aws ec2 authorize-security-group-ingress \\"
    echo "       --group-id $SG_ID \\"
    echo "       --protocol tcp \\"
    echo "       --port $GRPC_PORT \\"
    echo "       --source-group sg-YOUR-CLIENT-SG \\"
    echo "       --region $REGION"
    echo
    echo "2. Allow from VPC CIDR (for same VPC):"
    echo "   aws ec2 authorize-security-group-ingress \\"
    echo "       --group-id $SG_ID \\"
    echo "       --protocol tcp \\"
    echo "       --port $GRPC_PORT \\"
    echo "       --cidr $VPC_CIDR \\"
    echo "       --region $REGION"
    echo
    echo "3. Allow from specific IP:"
    echo "   aws ec2 authorize-security-group-ingress \\"
    echo "       --group-id $SG_ID \\"
    echo "       --protocol tcp \\"
    echo "       --port $GRPC_PORT \\"
    echo "       --cidr YOUR.CLIENT.IP.0/32 \\"
    echo "       --region $REGION"
fi

# Test local port
echo -e "\n=== Testing Local Port ==="
if nc -zv localhost $GRPC_PORT 2>&1 | grep -q succeeded; then
    echo "✓ Port $GRPC_PORT is open locally"
else
    echo "❌ Port $GRPC_PORT is not listening locally"
    echo "   Make sure the orderbook service is running"
fi

# Connection string for clients
echo -e "\n=== Connection Strings for Clients ==="
echo "From same VPC:      $PRIVATE_IP:$GRPC_PORT"
if [ "$PUBLIC_IP" != "N/A" ]; then
    echo "From internet:      $PUBLIC_IP:$GRPC_PORT (requires security group rule)"
fi
echo "Via load balancer:  <your-alb-dns>:$GRPC_PORT"

echo -e "\n=== Testing Commands ==="
echo "From client EC2:"
echo "  nc -zv $PRIVATE_IP $GRPC_PORT"
echo "  python3 test_external_connection.py $PRIVATE_IP:$GRPC_PORT"