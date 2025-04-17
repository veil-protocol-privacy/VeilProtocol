## Zero knowledge proofs:

Verfication program take the setup of the circuit and use them to verify using ```groth16_solana```.

### Key management

When users create or import a wallet, our system client derived three keys from the wallet private key which are:

1. **Spending Key** – Used to sign transactions.
2. **Viewing Key** – Used to derive the nullifying key.
3. **Deposit Key** - Used to encrypt deposit transaction.

The nullifying key is the hash of the viewing key. The master public key is then derived by converting the signing key from private to public and hashing it together with the nullifying key.

![Screenshot 2025-03-20 at 17.01.58](https://hackmd.io/_uploads/S1nnwvF31x.png)

We utilize a Merkle tree to store UTXOs, where each leaf node represents a commitment. This commitment is the hash of the UTXO key, the amount, and the token information. The UTXO key itself is derived by hashing the master public key with random bytes.

![Screenshot 2025-03-20 at 17.04.58](https://hackmd.io/_uploads/BktLdPtnJg.png)

The verificaion circuit contains of 3 components:

### 1.Verify merkle tree

Payer need to prove that they know where all the UTXOs they use to transfer located in merkle tree.
Public inputs:
- Path to Merkle leaf: root node, leaf node and sibling hashes at each level.
- Leaf index: The position of the UTXO in the tree.
    
Process: 
- Hash the leaf node with its corresponding sibling node to compute the next-level hash.
- Repeat the process iteratively until reaching the root node.
- Verify that the computed root hash matches the given root node.

Currently, the Merkle tree verification supports only **single-tree proofs**. Future improvements may include support for verifying UTXOs across multiple trees.

### 2. Nullifier check

Payer need to prove that they are the owner of the UTXO (which mean they own the secret key of that leaf's nulifier). That info will be store in program in order to prevent futher transaction on that leaf node.

Public inputs:
- Nullifying key (hash of viewing key)
- Leaf index
- Nullifier

The nullifier check only need to prove that the hash of nullifying key and leaf index equal to nullifier.

### 3. Signature & sum check

Payer need to sign a nullifier message to prove that they have the right to use that leaf node. As long as the **master public key** is stored in the leaf, the signing key must be correct.

Public inputs:
- Signing publicKey (Convert signing key from private to public)
- Signature
- Nulifier of inputs
- UXTO outputs
- Message hash
- Amount In/Out

Process:
- Construct message hash by combining: 
    - Merkle root
    - Bound params
    - Nulifier of inputs
    - UTXO outputs
- Verify message hash with signature
- Sum up the input amount, output amount. Check if sumIn == sumOut

### Encrypt & Decrypt

For every transaction, the user must specify the UTXO inputs and outputs. To do so, they need to know the receiver's viewing public key and share some relevant information with the receiver.

The system will generate a share_key using the sender's private key and the receiver's public key. This share_key will be used to decrypt the information necessary to identify the received UTXO.

![Screenshot 2025-04-04 at 11.14.25](https://hackmd.io/_uploads/rypi3R3pJx.png)

On the other hand, the receiver can also use their private key along with the sender's public key to generate a share_key, which they can use to decrypt the data.

![image](https://hackmd.io/_uploads/BytQ0A3Tkl.png =500x400)
### Encrypt shield & Decrypt shield

For depositing token into Veil program ( or shielding that asset ), the user must specify the amount and token mint account as input to generate a UTXO output. 

The system will generate a share_key using the depositor's deposit pubkey and depositor's viewing private key. This share_key will be used to encrypt the UTXO random value. And the receiver viewing pubkey is encrypted using depositor viewing private key.

![image](https://hackmd.io/_uploads/HkKTM7aaJl.png)

Share key is generated from user viewing private key and shield key ( deposit pubkey ) then is used to decrypt the random value to build UTXO entry. 

![image](https://hackmd.io/_uploads/ByX14fTTyx.png)

## UTXO indexer

Veil program emits events when an transaction instructions (e.g: deposit, transfer or withdraw) is submitted. This events contains all the ciphertexts. Veil indexer is created to scan for this type of events and user try to decrypt the ciphertexts using their private keys. Once decrypted successfully, the user can store these decrypted UTXOs on their local machine. 

Unlike the merkle tree store on the program which only tracks roots history to save account space, Merkle tree store on the clients side is a full fletch merkle tree and is used to determine the path and root for each UTXO, helps generate zk proof when user decide to spent an UTXO. All the newly created leafs ( or hashed UTXO ) is also provided in the events, the indexer will insert those to the local merkle tree so that the root is sync with Veil program merkle tree.

The total amount of decrypted UTXOs the user have is corresponds to their total shield asset amount an can be spent privately in our protocol.

When user decide to spent an UTXO, the events also includes a nullifers hash which notify an UTXO has been spent, indexer will collect this nullifiers and update the available UTXOs list which essentially like updating user balance.

The indexer exposes an api for Veil cli to query informations likes 
- User available UTXOs 
- Current merkle root