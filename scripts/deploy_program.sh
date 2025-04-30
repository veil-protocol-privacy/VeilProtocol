#!/bin/bash

# Exit on any error
set -e

# Define paths
PROGRAM_DIR="." # Replace with your program's directory
PROGRAM_KEYPAIR="$PROGRAM_DIR/target/deploy/darksol-keypair.json"
PROGRAM_SO="$PROGRAM_DIR/target/deploy/darksol.so"

# # Check for large stack allocations before building
# echo "Checking for large stack allocations in the program..."
# cargo build-bpf --manifest-path "$PROGRAM_DIR/Cargo.toml" --features no-entrypoint 2>&1 | grep -i "stack" || true

# # Build the program with optimization flags
# echo "Building the Solana program with stack optimization..."
# RUSTFLAGS="-C opt-level=3 -C inline-threshold=1000" cargo build-bpf --manifest-path "$PROGRAM_DIR/Cargo.toml"

# Verify the build was successful
if [ ! -f "$PROGRAM_SO" ]; then
    echo "Error: Program compilation failed. Please check your code for large stack variables."
    echo "Common fixes include:"
    echo "  1. Replace large arrays/structs on the stack with heap allocations (Vec<T>, Box<T>)"
    echo "  2. Break up large functions into smaller ones"
    echo "  3. Pass large structures by reference rather than by value"
    echo "  4. Use static variables for large read-only data"
    exit 1
fi

# Deploy the program
echo "Deploying the Solana program..."
PROGRAM_ID=$(solana program deploy "$PROGRAM_SO" --program-id "$PROGRAM_KEYPAIR" | grep -oP '(?<=Program Id: )\w+' || solana program deploy "$PROGRAM_SO" | grep -oP '(?<=Program Id: )\w+')

# Output the program ID
echo "Program deployed successfully!"
echo "Program ID: $PROGRAM_ID"
