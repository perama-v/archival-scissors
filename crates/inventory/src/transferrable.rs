//! For representing critical state data while minimising duplication (intra-block and inter-block). State data is to be transmitted between untrusted parties.
//!
//! ## Encoding
//!
//! The data is SSZ encoded for consistency between implementations.
//!
//! ## Contents
//!
//! Large data items (contract code and merkle trie nodes) may be repeated within a block or
//! between blocks.
//! - Contract is called in different transactions/blocks without changes to its bytecode.
//! - Merkle trie nodes where state is accessed in the leafy-end of the proof. Such as very
//! populated accounts and storage items.
//!
//! Such an item can be referred to by the position in a separate list.

use std::collections::{HashMap, HashSet};

use ethers::types::{EIP1186ProofResponse, H160};
use ssz_rs::prelude::*;
use ssz_rs_derive::SimpleSerialize;
use thiserror::Error;

use crate::{
    cache::ContractBytes,
    ssz::{
        constants::{
            MAX_ACCOUNT_NODES_PER_BLOCK, MAX_ACCOUNT_PROOFS_PER_BLOCK, MAX_BYTES_PER_CONTRACT,
            MAX_BYTES_PER_NODE, MAX_CONTRACTS_PER_BLOCK, MAX_NODES_PER_PROOF,
            MAX_STORAGE_NODES_PER_BLOCK, MAX_STORAGE_PROOFS_PER_ACCOUNT,
        },
        types::{SszH160, SszH256, SszU256, SszU64},
    },
    types::BlockProofs,
    utils::{
        compress, h160_to_ssz_h160, h256_to_ssz_h256, u256_to_ssz_u256, u64_to_ssz_u64,
        usize_to_u16, UtilsError,
    },
};

#[derive(Debug, Error)]
pub enum TransferrableError {
    #[error("SSZ Error {0}")]
    SszError(#[from] SerializeError),
    #[error("SimpleSerialize Error {0}")]
    SimpleSerializeError(#[from] SimpleSerializeError),
    #[error("Utils error {0}")]
    UtilsError(#[from] UtilsError),
    #[error("Unable to find index for node")]
    NoIndexForNode,
}

/// State that has items referred to by their hash. This store represents the minimum
/// set of information that a peer should send to enable a block holder (eth_getBlockByNumber)
/// to trace the block.
///
/// Consists of:
/// - A collection of EIP-1186 style proofs with intermediate nodes referred to in a separate list.
/// EIP-1186 proofs consist of:
///     - address, balance, codehash, nonce, storagehash, accountproofnodehashes, storageproofs
///         - storageproofs: key, value, storageproofnodehashes
/// - contract code referred to by codehash.
/// - account trie node referred to by nodehash
/// - storage trie node referred to by nodehash
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct SlimBlockStateProof {
    slim_eip1186_proofs: SlimEip1186Proofs,
    contracts: Contracts,
    account_nodes: AccountNodes,
    storage_nodes: StorageNodes,
}

pub type SlimEip1186Proofs = List<SlimEip1186Proof, MAX_ACCOUNT_PROOFS_PER_BLOCK>;
pub type StorageNodes = List<TrieNode, MAX_STORAGE_NODES_PER_BLOCK>;
pub type AccountNodes = List<TrieNode, MAX_ACCOUNT_NODES_PER_BLOCK>;

/// RLP-encoded Merkle PATRICIA Trie node.
pub type TrieNode = List<u8, MAX_BYTES_PER_NODE>;

// Multiple contracts
pub type Contracts = List<Contract, MAX_CONTRACTS_PER_BLOCK>;

/// Contract bytecode.
pub type Contract = List<u8, MAX_BYTES_PER_CONTRACT>;

/// An EIP-1186 style proof with the trie nodes replaced by their keccak hashes.
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct SlimEip1186Proof {
    pub address: SszH160,
    pub balance: SszU256,
    pub code_hash: SszH256,
    pub nonce: SszU64,
    pub storage_hash: SszH256,
    pub account_proof: NodeIndices,
    pub storage_proofs: SlimStorageProofs,
}

pub type SlimStorageProofs = List<SlimStorageProof, MAX_STORAGE_PROOFS_PER_ACCOUNT>;

/// An EIP-1186 style proof with the trie nodes replaced by their keccak hashes.
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct SlimStorageProof {
    pub key: SszH256,
    pub value: SszU256,
    pub proof: NodeIndices,
}

/// An ordered list of indices that point to specific
/// trie nodes in a different ordered list.
///
/// The purpose is deduplication as some nodes appear in different proofs within
/// the same block.
pub type NodeIndices = List<u16, MAX_NODES_PER_PROOF>;

impl SlimBlockStateProof {
    /// Creates a slim proof by separating trie nodes and contract code from the proof data.
    pub fn create(
        block_proofs: BlockProofs,
        accessed_contracts: Vec<ContractBytes>,
    ) -> Result<Self, TransferrableError> {
        let node_set = get_trie_node_set(&block_proofs.proofs);
        // TODO Remove this clone.
        let node_map = get_node_map(node_set.clone());

        let proof = SlimBlockStateProof {
            slim_eip1186_proofs: get_slim_eip1186_proofs(&node_map, block_proofs)?,
            contracts: contracts_to_ssz(accessed_contracts),
            account_nodes: bytes_collection_to_ssz(node_set.account),
            storage_nodes: bytes_collection_to_ssz(node_set.storage),
        };
        Ok(proof)
    }
    pub fn to_ssz_snappy_bytes(self) -> Result<Vec<u8>, TransferrableError> {
        let mut buf = vec![];
        let ssz_kb = self.serialize(&mut buf)? / 1000;
        let compressed = compress(buf)?;
        let snappy_kb = compressed.len() / 1000;
        println!(
            "SSZ data compressed from {ssz_kb}KB to {snappy_kb}KB"
        );
        Ok(compressed)
    }
}

/// Returns a map of node -> index. The index is later used to replace nodes
/// so a map is made prior to the substitution.
fn get_node_map(node_set: TrieNodesSet) -> TrieNodesIndices {
    let mut account: HashMap<NodeBytes, usize> = HashMap::new();
    let mut storage: HashMap<NodeBytes, usize> = HashMap::new();

    for (index, node) in node_set.account.into_iter().enumerate() {
        account.insert(node, index);
    }
    for (index, node) in node_set.storage.into_iter().enumerate() {
        storage.insert(node, index);
    }
    TrieNodesIndices { account, storage }
}

/// Replace every node with a reference to the index in a list.
fn get_slim_eip1186_proofs(
    node_set: &TrieNodesIndices,
    block_proofs: BlockProofs,
) -> Result<SlimEip1186Proofs, TransferrableError> {
    let mut ssz_eip1186_proofs = SlimEip1186Proofs::default();
    for proof in block_proofs.proofs {
        // Account
        let account_indices = nodes_to_node_indices(proof.1.account_proof, &node_set.account)?;
        // Storage
        let mut slim_storage_proofs = SlimStorageProofs::default();
        for storage_proof in proof.1.storage_proof {
            let storage_indices = nodes_to_node_indices(storage_proof.proof, &node_set.storage)?;
            // key, value
            let slim_storage_proof = SlimStorageProof {
                key: h256_to_ssz_h256(storage_proof.key)?,
                value: u256_to_ssz_u256(storage_proof.value),
                proof: storage_indices,
            };
            slim_storage_proofs.push(slim_storage_proof);
        }

        let ssz_eip1186_proof = SlimEip1186Proof {
            address: h160_to_ssz_h160(proof.1.address)?,
            balance: u256_to_ssz_u256(proof.1.balance),
            code_hash: h256_to_ssz_h256(proof.1.code_hash)?,
            nonce: u64_to_ssz_u64(proof.1.nonce),
            storage_hash: h256_to_ssz_h256(proof.1.storage_hash)?,
            account_proof: account_indices,
            storage_proofs: slim_storage_proofs,
        };
        ssz_eip1186_proofs.push(ssz_eip1186_proof);
    }
    Ok(ssz_eip1186_proofs)
}

/// Turns a list of nodes in to a list of indices. The indices
/// come from a mapping.
fn nodes_to_node_indices(
    proof: Vec<ethers::types::Bytes>,
    map: &HashMap<NodeBytes, usize>,
) -> Result<NodeIndices, TransferrableError> {
    let mut indices = NodeIndices::default();
    // Substitute proof nodes with indices.
    for node in proof {
        // Find the index
        let index: &usize = map
            .get(node.0.as_ref())
            .ok_or(TransferrableError::NoIndexForNode)?;
        // Insert the index
        indices.push(usize_to_u16(*index)?);
    }
    Ok(indices)
}

/// Holds all node set present in a block state proof. Used to construct
/// deduplicated slim proof.
#[derive(Clone)]
struct TrieNodesSet {
    account: Vec<NodeBytes>,
    storage: Vec<NodeBytes>,
}

/// /// Maps node -> index for all nodes present in a block state proof. Used to construct
/// deduplicated slim proof.
struct TrieNodesIndices {
    account: HashMap<NodeBytes, usize>,
    storage: HashMap<NodeBytes, usize>,
}

type NodeBytes = Vec<u8>;

/// Finds all trie nodes and uses a HashSet to remove duplicates.
fn get_trie_node_set(proofs: &HashMap<H160, EIP1186ProofResponse>) -> TrieNodesSet {
    let mut account_set: HashSet<Vec<u8>> = HashSet::default();
    let mut storage_set: HashSet<Vec<u8>> = HashSet::default();

    for proof in proofs.values() {
        for node in &proof.account_proof {
            account_set.insert(node.0.clone().into());
        }
        for storage_proof in &proof.storage_proof {
            for node in &storage_proof.proof {
                storage_set.insert(node.0.clone().into());
            }
        }
    }
    let account = account_set.into_iter().collect();
    let storage = storage_set.into_iter().collect();
    TrieNodesSet { account, storage }
}

/// Turns a collection of contracts into an SSZ format.
fn contracts_to_ssz(input: Vec<ContractBytes>) -> Contracts {
    let mut contracts = Contracts::default();
    input
        .into_iter()
        .map(|c| {
            let mut list = Contract::default();
            c.into_iter().for_each(|byte| list.push(byte));
            list
        })
        .for_each(|contract| contracts.push(contract));
    contracts
}

/// Turns a collection of bytes into an SSZ format.
fn bytes_collection_to_ssz<const OUTER: usize, const INNER: usize>(
    collection: Vec<Vec<u8>>,
) -> List<List<u8, INNER>, OUTER> {
    let mut ssz_collection = List::<List<u8, INNER>, OUTER>::default();
    collection
        .into_iter()
        .map(|bytes| {
            let mut ssz_bytes = List::<u8, INNER>::default();
            bytes.into_iter().for_each(|byte| ssz_bytes.push(byte));
            ssz_bytes
        })
        .for_each(|contract| ssz_collection.push(contract));
    ssz_collection
}