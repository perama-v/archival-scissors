//! Verifies an EIP-1186 style proof

use ethers::{
    types::{EIP1186ProofResponse, StorageProof, H256, U256, U64},
    utils::keccak256,
};

use rlp_derive::{RlpDecodable, RlpEncodable};
use serde::Deserialize;
use thiserror::Error;

use crate::{
    proof::{ProofError, SingleProofPath, Verified},
    utils::hex_encode,
};

#[derive(Default, Debug, Clone, PartialEq, Eq, Deserialize, RlpEncodable, RlpDecodable)]
pub struct Account {
    pub nonce: U64,
    pub balance: U256,
    pub storage_hash: H256,
    pub code_hash: H256,
}

impl Account {
    fn is_empty(&self) -> bool {
        let empty = Account::default();
        self.eq(&empty)
    }
}

#[derive(Debug, Error)]
pub enum VerifyProofError {
    #[error("Proof not valid for account {account}, AccountError {source} ")]
    AccountError {
        source: AccountError,
        account: String,
    },
    #[error(
        "Proof not valid for account {account} storage key {storage_key}, StorageError {source}"
    )]
    StorageError {
        source: StorageError,
        account: String,
        storage_key: String,
    },
    #[error("Proof is empty")]
    EmptyProof,
}

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("ProofError {0}")]
    ProofError(#[from] ProofError),
    #[error("Proof is empty")]
    EmptyProof,
    #[error("A valid exclusion proof exists, but the claimed account is not empty")]
    ExclusionProofForNonEmptyAccount,
    #[error("Unexpected inclusion proof for an empty account, expected exclusion proof")]
    InclusionProofForEmptyAccount,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("ProofError {0}")]
    ProofError(#[from] ProofError),
    #[error("A valid exclusion proof exists, but the claimed storage is not empty")]
    ExclusionProofForNonZeroValue,
    #[error("Unexpected inclusion proof for a storage value of zero, expected exclusion proof")]
    InclusionProofForZeroValue,
}

/// Verifies a single account proof with respect to a state roof. The
/// proof is of the form returned by eth_getProof.
pub fn verify_proof(
    block_state_root: &[u8],
    proof: &EIP1186ProofResponse,
) -> Result<(), VerifyProofError> {
    // Account
    verify_account_component(block_state_root, proof).map_err(|source| {
        VerifyProofError::AccountError {
            source,
            account: hex_encode(proof.address),
        }
    })?;

    // Storage proofs for this account
    for storage_proof in &proof.storage_proof {
        verify_account_storage_component(&proof.storage_hash.0, storage_proof.clone()).map_err(
            |source| VerifyProofError::StorageError {
                source,
                account: hex_encode(proof.address),
                storage_key: hex_encode(storage_proof.key),
            },
        )?;
    }
    Ok(())
}

pub fn verify_account_component(
    block_state_root: &[u8],
    proof: &EIP1186ProofResponse,
) -> Result<(), AccountError> {
    let claimed_account = Account {
        nonce: proof.nonce,
        balance: proof.balance,
        storage_hash: proof.storage_hash,
        code_hash: proof.code_hash,
    };

    let account_prover = SingleProofPath {
        proof: proof.account_proof.clone(),
        root: H256::from_slice(block_state_root).0,
        path: keccak256(proof.address.as_bytes()),
        claimed_value: rlp::encode(&claimed_account).to_vec(),
    };

    match account_prover.verify()? {
        Verified::Inclusion => {
            if claimed_account == Account::default() {
                return Err(AccountError::InclusionProofForEmptyAccount);
            }
        }
        Verified::Exclusion => match claimed_account.is_empty() {
            true => {}
            false => return Err(AccountError::ExclusionProofForNonEmptyAccount),
        },
    }
    Ok(())
}

/// Verfies a single storage proof with respect to a known storage hash.
fn verify_account_storage_component(
    storage_hash: &[u8; 32],
    storage_proof: StorageProof,
) -> Result<(), StorageError> {
    let rlp_value = rlp::encode(&storage_proof.value).to_vec();

    // TODO: See yellow paper (205). Account for cases where entire node is <32 bytes.

    let storage_prover = SingleProofPath {
        proof: storage_proof.proof,
        root: *storage_hash,
        path: keccak256(storage_proof.key),
        claimed_value: rlp_value,
    };

    match storage_prover.verify()? {
        Verified::Inclusion => {
            if storage_proof.value == U256::from(0) {
                return Err(StorageError::InclusionProofForZeroValue);
            }
        }
        Verified::Exclusion => match storage_proof.value.is_zero() {
            true => {}
            false => return Err(StorageError::ExclusionProofForNonZeroValue),
        },
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::utils::hex_decode;

    use super::*;
    use std::{fs::File, io::BufReader};

    fn load_proof(path: &str) -> EIP1186ProofResponse {
        let file = File::open(path).expect("no proof found");
        let reader = BufReader::new(&file);
        serde_json::from_reader(reader).expect("could not parse proof")
    }

    /// data src:
    /// https://github.com/ethereum/execution-apis/blob/main/tests/eth_getProof/get-account-proof-with-storage.io
    /// ```json
    /// {"jsonrpc":"2.0","id":1,"method":"eth_getProof","params":["0xaa00000000000000000000000000000000000000",["0x01"],"0x3"]}
    /// ```
    #[test]
    fn test_verify_inclusion_proof_of_storage_value_zero() {
        let account_proof = load_proof("data/test_proof_1.json");
        let state_root =
            hex_decode("0x61effbbcca94f0d3e02e5bd22e986ad57142acabf0cb3d129a6ad8d0f8752e94")
                .unwrap();
        verify_proof(&state_root, &account_proof).expect("could not verify proof");
    }

    /// data src: https://github.com/gakonst/ethers-rs/blob/master/ethers-core/testdata/proof.json
    #[test]
    fn test_verify_exclusion_proof_for_storage_key_zero() {
        let account_proof = load_proof("data/test_proof_2.json");
        let state_root =
            hex_decode("0x57e6e864257daf9d96aaca31edd0cfe4e3892f09061e727c57ab56197dd59287")
                .unwrap();
        verify_proof(&state_root, &account_proof).expect("could not verify proof");
    }

    /// data src: block 17190873
    #[test]
    fn test_verify_inclusion_proof_for_nonzero_storage_value() {
        let account_proof = load_proof("data/test_proof_3.json");
        let state_root =
            hex_decode("0x38e5e1dd67f7873cd8cfff08685a30734c18d0075318e9fca9ed64cc28a597da")
                .unwrap();
        verify_proof(&state_root, &account_proof).expect("could not verify proof");
    }
}
