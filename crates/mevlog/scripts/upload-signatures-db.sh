#!/bin/bash

set -e  # Exit on any error

# Configuration
if [ -z "$AWS_PROFILE" ]; then
    print_error "AWS_PROFILE environment variable is not set"
    exit 1
fi

if [ -z "$S3_BUCKET" ]; then
    print_error "S3_BUCKET environment variable is not set"
    exit 1
fi

if [ -z "$CLOUDFRONT_DISTRIBUTION_ID" ]; then
    print_error "CLOUDFRONT_DISTRIBUTION_ID environment variable is not set"
    exit 1
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if required environment variable is set
if [ -z "$DATABASE_PATH" ]; then
    print_error "DATABASE_PATH environment variable is not set"
    print_error "Usage: DATABASE_PATH=/path/to/database.db $0"
    exit 1
fi

# Check if database file exists
if [ ! -f "$DATABASE_PATH" ]; then
    print_error "Database file not found: $DATABASE_PATH"
    exit 1
fi

# Check if AWS CLI is installed
if ! command -v aws &> /dev/null; then
    print_error "AWS CLI is not installed"
    exit 1
fi

# Check if the AWS profile exists
if ! aws configure list-profiles | grep -q "^$AWS_PROFILE$"; then
    print_error "AWS profile '$AWS_PROFILE' not found"
    print_error "Please configure the profile first: aws configure --profile $AWS_PROFILE"
    exit 1
fi

# Extract filename and create zstd compressed version
DB_FILENAME=$(basename "$DATABASE_PATH")
DB_DIRNAME=$(dirname "$DATABASE_PATH")
COMPRESSED_FILE="${DB_DIRNAME}/${DB_FILENAME}.zst"

print_status "Starting database upload process..."
print_status "Database file: $DATABASE_PATH"
print_status "Compressed file: $COMPRESSED_FILE"

# Step 1: Compress the database file with zstd
print_status "Compressing database file with zstd -19..."
if zstd -19 -c "$DATABASE_PATH" > "$COMPRESSED_FILE"; then
    print_status "Database compressed successfully"
    
    # Show compression ratio
    ORIGINAL_SIZE=$(stat -f%z "$DATABASE_PATH" 2>/dev/null || stat -c%s "$DATABASE_PATH" 2>/dev/null)
    COMPRESSED_SIZE=$(stat -f%z "$COMPRESSED_FILE" 2>/dev/null || stat -c%s "$COMPRESSED_FILE" 2>/dev/null)
    RATIO=$(echo "scale=1; $COMPRESSED_SIZE * 100 / $ORIGINAL_SIZE" | bc 2>/dev/null || echo "N/A")
    print_status "Compression ratio: ${RATIO}% (${ORIGINAL_SIZE} -> ${COMPRESSED_SIZE} bytes)"
else
    print_error "Failed to compress database file"
    exit 1
fi

# Step 2: Upload to S3
S3_KEY="${DB_FILENAME}.zst"
print_status "Uploading to S3: s3://$S3_BUCKET/$S3_KEY"

if aws s3 cp "$COMPRESSED_FILE" "s3://$S3_BUCKET/$S3_KEY" \
    --profile "$AWS_PROFILE" \
    --content-encoding zstd \
    --content-type "application/octet-stream" \
    --metadata "uncompressed-size=$ORIGINAL_SIZE"; then
    print_status "File uploaded successfully to S3"
else
    print_error "Failed to upload file to S3"
    rm -f "$COMPRESSED_FILE"
    exit 1
fi

# Step 3: Create CloudFront invalidation
print_status "Creating CloudFront invalidation for /$S3_KEY"

INVALIDATION_OUTPUT=$(aws cloudfront create-invalidation \
    --distribution-id "$CLOUDFRONT_DISTRIBUTION_ID" \
    --paths "/$S3_KEY" \
    --profile "$AWS_PROFILE" \
    --output json)

if [ $? -eq 0 ]; then
    INVALIDATION_ID=$(echo "$INVALIDATION_OUTPUT" | grep -o '"Id": *"[^"]*"' | cut -d'"' -f4)
    print_status "CloudFront invalidation created successfully"
    print_status "Invalidation ID: $INVALIDATION_ID"
else
    print_error "Failed to create CloudFront invalidation"
    rm -f "$COMPRESSED_FILE"
    exit 1
fi

# Cleanup: Remove local compressed file
print_status "Cleaning up local compressed file..."
rm -f "$COMPRESSED_FILE"

print_status "✅ Database upload and CloudFront invalidation completed successfully!"
print_status "File available at: https://$(aws cloudfront get-distribution --id $CLOUDFRONT_DISTRIBUTION_ID --profile $AWS_PROFILE --query 'Distribution.DomainName' --output text)/$S3_KEY"
