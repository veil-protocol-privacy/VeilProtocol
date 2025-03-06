use solana_poseidon::{PoseidonHash, Parameters, Endianness};
use solana_poseidon::hashv;
use std::collections::HashMap;
use std::fmt;
use std::clone;
use crate::lib::{ZERO_VALUE, u256_to_bytes};
use borsh::{BorshSerialize, BorshDeserialize};

const TREE_DEPTH: usize = 16;

fn hash_left_right(left: Vec<u8>, right: Vec<u8>) -> Result<Vec<u8>, String> {
    let result: Result<PoseidonHash, solana_poseidon::PoseidonSyscallError> = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&left, &right]);

    match result {
        Ok(hash) => {
            let bytes = hash.to_bytes();
            return Ok(bytes.to_vec());        
        }
        Err(err) => {
            return Err(format!("fail to create hash: {}", err.to_string()));
        }
    }
}

// Batch Incremental Merkle Tree for commitments
// each account store a single tree indicate by its 
// tree number
#[derive(BorshSerialize, BorshDeserialize)]
pub struct CommitmentsAccount {
    next_leaf_index: usize,
    merkle_root: Vec<u8>,
    new_tree_root: Vec<u8>,
    tree_number: u64,
    zeros: Vec<Vec<u8>>,
    filled_sub_trees: Vec<Vec<u8>>,
    root_history: HashMap<Vec<u8>, bool>, // root -> seen
}

// InsertResp return the tree number the insertion occur
// the leaf index, updated commitments data and the address
// that store the data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct InsertResp {
    commitments_data: CommitmentsAccount,
}

impl CommitmentsAccount { 
    /// Batch insert multiple commitments
    pub fn insert_commitments(&mut self, commitments: &mut Vec<Vec<u8>>) -> Result<InsertResp, String> {
        let result = insert_commitments(self, commitments);
        match result {
            Ok(resp) => {
                Ok(resp)
            }
            Err(err) => {
                return Err(format!("fail to insert commitments: {}", err.to_string()));
            }
        }
    }

    /// Get the Merkle root
    pub fn root(&self) -> Vec<u8> {
        self.merkle_root.clone()
    }

}

/// Batch insert multiple commitments
fn insert_commitments(commitments_data: &mut CommitmentsAccount, commitments: &mut Vec<Vec<u8>>) -> Result<InsertResp, String> {
    // this check is just double check to make sure the leaf count does not exceed the limit
    // as above logic must also check this in order to create another data account
    // for a new tree if insertion exceeds the max tree dept.
    let mut count = commitments.len();

    let base: usize = 2; // an explicit type is required
    // if exceeding max tree depth create a new tree
    if count + commitments_data.next_leaf_index > base.pow(TREE_DEPTH as u32) {
        return Err(format!("exceed max tree dept"));
    }

    let mut level_insertion_index: usize = commitments_data.next_leaf_index;

    commitments_data.next_leaf_index += count;

    // Variables for starting point at next tree level
    let mut next_level_hash_index: usize = 0;
    let mut next_level_start_index: usize;

    // Loop through each level of the merkle tree and update
    for level in 0..TREE_DEPTH {
        // Calculate the index to start at for the next level
        // >> is equivalent to / 2 rounded down
        next_level_start_index = level_insertion_index >> 1;

        let mut insertion_element = 0;

        // If we're on the right, hash and increment to get on the left
        if level_insertion_index % 2 == 1 {
            // Calculate index to insert hash into leafHashes[]
            // >> is equivalent to / 2 rounded down
            next_level_hash_index = (level_insertion_index >> 1) - next_level_start_index;

            // Calculate the hash for the next level
            let result: Result<Vec<u8>, String> = hash_left_right(
            commitments_data.filled_sub_trees[level].clone(),
            commitments[insertion_element].clone()
            );

            match result {
                Ok(hash) => {
                    commitments[next_level_hash_index] = hash;
                }

                Err(e) => {
                    return Err(format!("fail to create hash from left and right leaf: {}", e.to_string()))
                }
            }

            // Increment
            insertion_element += 1;
            level_insertion_index += 1;
        }

        // We'll always be on the left side now
        for insertion_element in (insertion_element..count).step_by(2){
            let &mut right: &mut Vec<u8>;

            // Calculate right value
            if insertion_element < count - 1 {
                right = commitments[insertion_element + 1].clone();
            } else {
                right = commitments_data.zeros[level].clone();
            }

            // If we've created a new subtree at this level, update
            if insertion_element == count - 1 || insertion_element == count - 2 {
                commitments_data.filled_sub_trees[level] = commitments[insertion_element].clone();
            }

            // Calculate index to insert hash into leafHashes[]
            // >> is equivalent to / 2 rounded down
            next_level_hash_index = (level_insertion_index >> 1) - next_level_start_index;

            // Calculate the hash for the next level
            let result = hash_left_right(commitments[insertion_element].clone(), right);
            match result {
                Ok(hash) => {
                    commitments[next_level_hash_index] = hash
                }
                Err(err) => {
                    return Err(format!("fail to create hash for the next level: {}", err.to_string()))
                }
            }

            // Increment level insertion index
            level_insertion_index += 2;
        }

            // Get starting levelInsertionIndex value for next level
            level_insertion_index = next_level_start_index;

            // Get count of elements for next level
            count = next_level_hash_index + 1;
    }

    // Update the Merkle tree root
    commitments_data.merkle_root = commitments[0].clone();
    commitments_data.root_history.insert(commitments_data.merkle_root.clone(), true);
    
    Ok(InsertResp{
        commitments_data: commitments_data.clone(),
    })
}

/// Create a new empty Merkle Tree
pub fn new_commitments_account(tree_number: u64) -> CommitmentsAccount {
    let zero_value = u256_to_bytes(ZERO_VALUE).to_vec();
    let mut root_history: HashMap<Vec<u8>, bool>  = HashMap::new();
    let mut zeros: Vec<Vec<u8>> = Vec::with_capacity(TREE_DEPTH);
    let mut filled_sub_trees: Vec<Vec<u8>> = Vec::with_capacity(TREE_DEPTH);

    zeros[0] = zero_value.clone();

    let mut current_zero = zero_value.clone();

    for i in 0..TREE_DEPTH {
        // Push it to zeros array
        zeros[i] = current_zero.clone();

        filled_sub_trees[i] = current_zero.clone();
       
        // Calculate the zero value for this level
        current_zero = hash_left_right(current_zero.clone(), current_zero.clone()).unwrap();
    }
    
    // Now safely insert into the inner HashMap
    root_history.insert(current_zero.clone(), true);
    
    CommitmentsAccount {
        next_leaf_index: 0,
        merkle_root: current_zero.clone(),
        new_tree_root: current_zero.clone(),
        tree_number,
        zeros,
        filled_sub_trees,
        root_history,
    }
}

impl fmt::Debug for CommitmentsAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DarkSolMerkleTree")
            .field("root", &self.merkle_root)
            .field("tree_number", &self.tree_number)
            .finish()
    }
}

impl clone::Clone for CommitmentsAccount {
    fn clone(&self) -> CommitmentsAccount {
        return CommitmentsAccount{
            new_tree_root: self.new_tree_root.clone(),
            next_leaf_index: self.next_leaf_index,
            tree_number: self.tree_number,
            filled_sub_trees: self.filled_sub_trees.clone(),
            merkle_root: self.merkle_root.clone(),
            zeros: self.zeros.clone(),
            root_history: self.root_history.clone(),
        }   
    }
}