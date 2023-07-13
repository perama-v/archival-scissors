//! Main data types defined by the spec, for transferrable parcels required for historical
//! state execution.

use ssz_rs::prelude::*;
use ssz_rs_derive::SimpleSerialize;
use thiserror::Error;

use crate::{
    alias::{SszH160, SszH256, SszU256, SszU64},
    constants::{
        MAX_ACCOUNT_NODES_PER_BLOCK, MAX_ACCOUNT_PROOFS_PER_BLOCK, MAX_BYTES_PER_CONTRACT,
        MAX_BYTES_PER_NODE, MAX_CONTRACTS_PER_BLOCK, MAX_NODES_PER_PROOF,
        MAX_STORAGE_NODES_PER_BLOCK, MAX_STORAGE_PROOFS_PER_ACCOUNT,
    },
};

#[derive(Debug, Error)]
pub enum StateError {
    #[error("Deserialize Error {0}")]
    DerializeError(#[from] ssz_rs::DeserializeError),
    #[error("SSZ Error {0}")]
    SszError(#[from] SerializeError),
    #[error("SimpleSerialize Error {0}")]
    SimpleSerializeError(#[from] SimpleSerializeError),
    #[error("Unable to find index for node")]
    NoIndexForNode,
}

/// State that has items referred to using indices to deduplicate data.
///
/// This store represents the minimum
/// set of information that a peer should send to enable a block holder (eth_getBlockByNumber)
/// to trace the block.
///
/// Consists of:
/// - A collection of EIP-1186 style proofs with intermediate nodes referred to in a separate list.
/// EIP-1186 proofs consist of:
///     - address, balance, codehash, nonce, storagehash, accountproofnodeindices, storageproofs
///         - storageproofs: key, value, storageproofnodeindices
/// - contract code.
/// - account trie node.
/// - storage trie node.
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct RequiredBlockState {
    pub compact_eip1186_proofs: CompactEip1186Proofs,
    pub contracts: Contracts,
    pub account_nodes: AccountNodes,
    pub storage_nodes: StorageNodes,
    pub blockhashes: BlockHashes,
}

pub type CompactEip1186Proofs = List<CompactEip1186Proof, MAX_ACCOUNT_PROOFS_PER_BLOCK>;
pub type StorageNodes = List<TrieNode, MAX_STORAGE_NODES_PER_BLOCK>;
pub type AccountNodes = List<TrieNode, MAX_ACCOUNT_NODES_PER_BLOCK>;
pub type BlockHashes = List<RecentBlockHash, 256>;

/// RLP-encoded Merkle PATRICIA Trie node.
pub type TrieNode = List<u8, MAX_BYTES_PER_NODE>;

// Multiple contracts
pub type Contracts = List<Contract, MAX_CONTRACTS_PER_BLOCK>;

/// Contract bytecode.
pub type Contract = List<u8, MAX_BYTES_PER_CONTRACT>;

/// A block hash for a recent block, for use by the BLOCKHASH opcode.
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct RecentBlockHash {
    pub block_number: SszU64,
    pub block_hash: SszH256,
}

/// An EIP-1186 style proof with the trie nodes replaced by their keccak hashes.
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct CompactEip1186Proof {
    pub address: SszH160,
    pub balance: SszU256,
    pub code_hash: SszH256,
    pub nonce: SszU64,
    pub storage_hash: SszH256,
    pub account_proof: NodeIndices,
    pub storage_proofs: CompactStorageProofs,
}

pub type CompactStorageProofs = List<CompactStorageProof, MAX_STORAGE_PROOFS_PER_ACCOUNT>;

/// An EIP-1186 style proof with the trie nodes replaced by their keccak hashes.
#[derive(PartialEq, Eq, Debug, Default, SimpleSerialize)]
pub struct CompactStorageProof {
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

impl RequiredBlockState {
    pub fn to_ssz_bytes(self) -> Result<Vec<u8>, StateError> {
        let mut buf = vec![];
        let _ssz_bytes_len = self.serialize(&mut buf)?;
        Ok(buf)
    }
    pub fn from_ssz_bytes(ssz_data: Vec<u8>) -> Result<Self, StateError> {
        let proofs = self::deserialize(&ssz_data)?;
        Ok(proofs)
    }
}