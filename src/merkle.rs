use solana_poseidon::{PoseidonHash, Parameters, Endianness};
use solana_poseidon::{hashv};
use solana_program::blake3::Hash;
use std::collections::HashMap;
use std::fmt;
use primitive_types::U256;
use crate::util::{ZERO_VALUE};

const TREE_DEPTH: usize = 16;


fn hash_left_right(left: Vec<u8>, right: Vec<u8>) -> Vec<u8> {
    let poseidon_hash = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&left, &right]).unwrap();
    
    poseidon_hash.to_bytes().to_vec()
}


/// Batch Incremental Merkle Tree for commitments
struct Commitments {
    depth: usize,
    parameters: Parameters,
    nullifiers: HashMap<u64, HashMap<U256, bool>>, // tree number -> nullifier -> seen
    next_leaf_index: usize,
    merkle_root: U256,
    new_tree_root: U256,
    tree_number: u64,
    zeros: Vec<U256>,
    filled_sub_trees: [U256; TREE_DEPTH],
    root_history: HashMap<U256, HashMap<U256, bool>>, // treeNumber -> root -> seen
}

impl Commitments {
    /// Create a new empty Merkle Tree
    fn new(depth: usize, parameters: Parameters) -> Self {
        let zero_value: Vec<[u8; 4]> = ZERO_VALUE;
        let root_history:HashMap<U256, HashMap<Vec<u8>, bool>>  = HashMap::new();
        let zeros = [zero_value; TREE_DEPTH];
        let filled_sub_trees:[Vec<u8>; TREE_DEPTH] = [zero_value; TREE_DEPTH];

        zeros[0] = &zero_value;

        let current_zero = zero_value;

        for i in 0..TREE_DEPTH {
            // Push it to zeros array
            zeros[i] = &current_zero;

            filled_sub_trees[i] = &current_zero;
           
            // Calculate the zero value for this level
            current_zero = hash_left_right(current_zero, current_zero);
        }
        
        root_history[&U256::zero()][&current_zero] = true;
        Self {
            depth,
            parameters,
            nullifiers: HashMap::new(),
            next_leaf_index: 0,
            merkle_root: current_zero,
            new_tree_root: current_zero,
            tree_number: 0,
            zeros,
            filled_sub_trees,
            root_history: root_history,
        }
    }


    /// Insert a single commitment
    fn insert(&mut self, commitment: Vec<u8>) {
        let index = self.next_leaf_index;
        self.next_leaf_index += 1;
        let mut current_index = index;
        let commitment_hash = hashv(&self.parameters, &[commitment]);
        self.nodes.insert((self.depth, current_index), commitment_hash.clone());

        for level in (0..self.depth).rev() {
            let parent_index = current_index / 2;
            let left = *self.nodes.get(&(level + 1, parent_index * 2)).unwrap_or(&self.zeros[level]);
            let right = *self.nodes.get(&(level + 1, parent_index * 2 + 1)).unwrap_or(&self.zeros[level]);
            let parent_hash = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&left, &right])
            
            self.nodes.insert((level, parent_index), parent_hash.clone());
            current_index = parent_index;
        }

        self.merkle_root = *self.nodes.get(&(0, 0)).unwrap();
    }

    /// Batch insert multiple commitments
    fn batch_insert(&mut self, commitments: Vec<u32>) {
        for commitment in commitments {
            self.insert(commitment);
        }
    }



    /// Get the Merkle root
    fn root(&self) -> Vec<u32> {
        self.merkle_root.clone()
    }

}

impl fmt::Debug for Commitments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RailgunMerkleTree")
            .field("depth", &self.depth)
            .field("nodes", &format!("<{} nodes>", self.nodes.len()))
            .field("root", &self.merkle_root)
            .field("tree_number", &self.tree_number)
            .finish()
    }
}