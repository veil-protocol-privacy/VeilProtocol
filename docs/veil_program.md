## Veil program

Veil program acts as a unify vault for all shielded assets. User only need to provide zk proof to prove the ownership of a merkle leaf or UTXO in order to spent it.

Veil program includes 3 main instructions for private transaction

### 1. Deposit

Deposit instruction is use to shield an asset. This is done by transfer the asset to a program owned account. This create a new ciphertext includes all information about the UTXO ( amount, token mint account address, ...etc ) and emits to an event for indexer to scan. Insert a new leaf represent the new UTXO to program merkle tree, updating its root and roots history.

```
leaf hash = hash(hash(master pubkey, random) token ID, amount)
```

### 2. Transfer

Transfer instruction is use to transfer shielded asset between users. Veil program takes inputs inlcuding list of new merkle leafs indicate new UTXOs, list of nullifiers indicate spent UTXOs, user current local merkle tree root and zk proofs.

Each merkle tree on the program store a list of nullifiers to prevent double spending. Check if the input nullifiers is already on the list, if not then update the list.

Check if the merkle roots send in instruction data has exist in the merkle roots history to ensure both user and program merkle roots is sync.

Verify the zk proofs to prove the ownership of spent UTXOs.

Emit ciphertext in events for indexer to scan. Ciphertext can be decrypt by using receiver viewing key so only the receiver can decrypt the ciphertext beside the sender making the transaction private.

### 3. Withdraw

Withdraw instruction is use to unshield an asset. Veil program take inputs including list of new merkle leaf ( in case there still some balance left in the shield assets ), list of nullifiers indicates spent UTXOs and zk proofs.

Handle logic the same as Transfer instruction.

Transfer token from program owned account to withdrawer token account.

