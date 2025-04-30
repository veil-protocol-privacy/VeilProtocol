set -e

killall solana-test-validator || true

PROGRAM_DIR="." # Replace with your program's directory
DARKSOL_KEYPAIR="$PROGRAM_DIR/target/so/darksol-keypair.json"
DARKSOL_SO="$PROGRAM_DIR/target/so/darksol.so"
VERIFICATION_KEYPAIR="$PROGRAM_DIR/target/so/verification-keypair.json"
VERIFICATION_SO="$PROGRAM_DIR/target/so/verification.so"
if [ ! -f "$DARKSOL_SO" ] || [ ! -f "$VERIFICATION_SO" ]; then
    cargo build-sbf --sbf-out-dir=./target/so
fi

screen -S local -t local -d -m solana-test-validator --reset
sleep 5
echo "Deploying the Darksol program..."
OUTPUT=$(solana program deploy "$DARKSOL_SO" --program-id "$DARKSOL_KEYPAIR" || solana program deploy "$DARKSOL_SO")
echo "$OUTPUT"
DARKSOL_ID=$(echo "$OUTPUT" | awk -F'Program Id: ' '/Program Id:/ {print $2}')
# Output the program ID
echo "Program deployed successfully!"
echo "Darksol program ID: $DARKSOL_ID"

OUTPUT=$(solana program deploy "$VERIFICATION_SO" --program-id "$VERIFICATION_KEYPAIR" || solana program deploy "$VERIFICATION_SO")
echo "$OUTPUT"
VERIFICATION_ID=$(echo "$OUTPUT" | awk -F'Program Id: ' '/Program Id:/ {print $2}')

# Output the program ID
echo "Verification program deployed successfully!"
echo "Verification program ID: $VERIFICATION_ID"

# Run test
cargo test --package verification-test --release -- process::test_process_instruction