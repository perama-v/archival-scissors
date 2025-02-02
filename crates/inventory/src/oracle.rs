//! For aggregating trie nodes that are required for a post-state trie node oracle.
//!
//! The oracle provides nodes for scenarios where key deletion involves node removal and trie
//! rearrangements that are otherwise incomputable.

use std::{collections::HashMap, str::FromStr};

use archors_multiproof::{
    eip1186::MultiProofError,
    oracle::{OracleTask, TaskType},
    proof::{Intent, MultiProof, ProofError, ProofOutcome},
    EIP1186MultiProof,
};
use archors_types::oracle::TrieNodeOracle;
use archors_verify::path::{NibblePath, PathError};
use ethers::{
    types::{H160, H256, U256},
    utils::{keccak256, rlp},
};
use thiserror::Error;

use crate::{
    types::BlockProofs,
    utils::{hex_decode, hex_encode},
};

#[derive(Debug, Error)]
pub enum OracleError {
    #[error("Unable to find address in post-state proof {0}")]
    NoPostStateAddress(String),
    #[error("Unable to find address in pre-state proof {0}")]
    NoPreStateAddress(String),
    #[error("Unable to find key {key} in post-state proof for address {address}")]
    NoPostStateKey { address: String, key: String },
    #[error("Multiproof Error {0}")]
    MultiProofError(#[from] MultiProofError),
    #[error("Path Error {0}")]
    PathError(#[from] PathError),
    #[error("Proof Error {0}")]
    ProofError(#[from] ProofError),
}
// traversal index 1
// path a94cbb29e9e040ea0451a17e489cd2b1b66a862b497352538b80d4240421919d
const DEMO_ACCOUNT: &str = "0x0a6dd5d5a00d6cb0678a4af507ba79a517d5eb64";
const DEMO_KEY: &str = "0x0381163500ec1bb2a711ed278aa3caac8cd61ce95bc6c4ce50958a5e1a83494b";
const DEMO_NODES: [&str; 2] = [
    "0xf9015180a0b6ff53997cdd0c1f088a13f81afb42724cfcea9a07f14a74bb7d1bf4991e1fe2808080a0830370b134144289bda9480169139c6b8f25ee03be7ed111b337c582778cb0e9a097d0df63fab694add277023d143b0e0514d72d8b39954c3e69c622dd0be1be27a05a18babcf477be08eaab47baaa7653f20bd1b736cb7a2c87a112fbcaf9d2f265a0a21b0e909676a0eaf650780fda8a442fa96c1cb75a148d0fdfb9605fba7d448ea03a297ff8508794992a9face497a7b51cc8f191bab147402429e6cd637ed972eea0f9578cbf15296164371c8deb5ccc2269029f5c10add7b9a3130ec836ee3eea99a0429142fd545a0147432a3a60ed59e7254d356b5eff9a8fb99e1bf38a8f11cf178080a06f9f472ad4ca9d97072e42c9c8cb6234d7135e7707f2404692bc3ccf928ca783a05c69391c6bd1ff415dbeeb367634de47152d9a04182c1f051ab91b69c7b2c07680",
    "0xf842a020b0e912134ecbc560d9962cb73786257d580cc958a163dd71783e3745403011a09f69af7c6a4c6753666bdd5418a7942156d49a5823f33fb8c5fea1ce0052270d"
];

// traversal index 3
// path 86652138dcf3b3a33c63c504a61cfb20dacb1b1cec6f7d4ac93b9c5646f744a6
const DEMO_ACCOUNT_2: &str = "0x1d8f8f00cfa6758d7be78336684788fb0ee0fa46";
const DEMO_KEY_2: &str = "0x018cadc03de393878df6974d0ec421346ba20241e63eb680292cc02c5862d3d3";
const DEMO_NODES_2: [&str; 2] = [
    "0xf90211a02625efe6d51c9a0f9d024c0fd2c487a48dcc0e139acf7dda6c28573d6506e6eca0626b397b31893d14e79b62a8afe04472da3e9fc7d00add190e6564462b5b7ec1a0b2b71afd40d886d4c517ced4ad35e8130a05d24590be0efe0db1bc168d998927a09de3b92aacb1687cd85ab725fa18f67d2743cbdb1cf74e91b653aa76f8640965a021ee5d9914954721504789e8905778052e291cba253cc28b74dcbb18b0346b44a0d1163778da96373114c8d6fc8b7f1248964b5d56c41dcf203f0649f3bb983d9da0f70e99d768ccfd8f581ea25befcc2cab225075d1b7c57ae8ae45b88dbcbb175ea056c41af319bb204b586539dcd1f64d4d7781c25e9157b2b8dc200c7f1756eee6a00d18e4451b4b9870183fbb9b4f356bca78e58e3255ee3c81792661946cea01efa04526e50b40c53a032ec2e54343b6a49462003c0c22b39a0a0eaface1474050eea0e02621370e695e9f94ca0d2922051e886b0f4dc7e7a0ede364ac8543e1cb66bba09ceab93e5804744e5f00e70309de8ac9276c1250ad39edab1dc3a5b4a060150ea0179a2a0348e73ac975007b4e6043328f3b1aacadbff9335fa65dc628a268a9b5a0935590f0f6682797b53b81d3af29c8ac41b40366c3f102e57867f3f6195fd46aa09e8a9f26606e3a800687538a6753369c47cc52969664ab8058ceffc22978db63a0d1b69d3dca902ddd7cb67e5f1b8c030778ff32ff622cb435931792f9025d56ea80", "0xe19f204266e9b528b875871f619b12e210715278d47186494715a71a6e20f4b51c02"
    ];

// traversal index 1
// path 57bc10c9342c76f80b2b68dae8fff0d8ac10f7a4f504244eab861bafc4781926
const DEMO_ACCOUNT_3: &str = "0x47110d43175f7f2c2425e7d15792acc5817eb44f";
const DEMO_KEY_3: &str = "0xfef8821866ab107ed3e1654723c8fe7c08a27460f47ec044a0eacd01452ad076";
const DEMO_NODES_3: [&str; 3] = [
    "0xf90211a0233f6885334e12e1bcbeeeda42ea861c92507598769fd942d38d5b9e7902f6d7a0d86d50b588189e2c09ab220a965ba3143747c439bd2f6528e532afe71b6dfa3ba028352cc2596ca4f4b5dfd0d2d0965946d7a8d4b720a583e30b93eb9930f1338da0e129bd2cd22050c51a0f69c92a4ff6fdb4284f0dec5ce278c9445119ec879ce4a0ccbd0ad794f6633ea48972ba9d87ff27f4857fd0666fb597e98f0d61d5a45b80a0c16781ce207166ff8063f61b4dd88e5b2ff04eaf544c0ef5d3986fe684d9b0b1a03159f613ed2499aa20b5461d6290fc82923b7ad3e6ee367aa03b590d6d4ad002a0fdfd3a7b30fda6c1616144242540dcf8eb119e0c0ae6fa4873a0311084ca8a7ca03346c049f757d2bed7c3fbad823d6a6fdd5efd7389a79ab735a1ce582477e1f6a0c517a874ed108aea46a747aae2d3a526bd8b2f5edd2166382b6ac2da7f307584a0d84deefb0c7714de7f4fccfae37c3ba58bae6f2be2650d3fd805d75ec95875e5a07a9da4aeccf0403448cb37465f21d01bf619de69ae1ce9a328a989a584de1ab8a0afc90381c778d9020234e7948b3c22b55068a5b2373f563cf35c179a88fbaa6ba093dd930eedcc89fe44af5632d931c077a8a69e291cb2d94a376473495aeb55baa0b6120f3877e8e57843a352ba5803740755a20d233825d8b34df5ba6edbf80308a077bbe94dce0fb0fc87896aeb67a9e033915166dcb3564dfa323e147f8fb5793780",
    "0xf8718080808080a080cb34d3fe14a8d766b0f0f19b88f2c2364716bd9ab9086500f0496add2550b38080808080a0239f25c924a4ada3bf157fc12ddeb5d0dfc5610ae7578b2e27ea2793cd8f35d6a0da4a968fae1c325071293c78127fdb0f962f90c3d6ec34e7ec68ca3987764ca280808080",
    "0xf8429f344d312eadbfd5914e85e3babbe0cecf16cfed98ddbf641ddf3f021b46d7e9a1a0ffffffffffffffffffffffffffffffffffffffffffffffff51044efbaf280436"
];

// traversal index 3
// path b6a07f0447c66d22bcc56ca5c98397e3cb3fa5116a622ccf1522bd7b28dc787a
const DEMO_ACCOUNT_4: &str = "0x6982508145454ce325ddbe47a25d4ec3d2311933";
const DEMO_KEY_4: &str = "0x0faf7484b417b74770f55e6641733402d3d02194fd201be2c1057f4c3a4c34a8";
const DEMO_NODES_4: [&str; 2] = [
    "0xf901b1a0b2d70de2c5586bfebf51da1cf92419d937068bd41ba88b93208e5d686e2dd1d8a0ed99508941af972ee3b0bb1c0795cd183dfa9681c54f739d1a816c3a63db9eaea00b69fb19f75e43b46fa84632b833537ce7219ac235af94598cc1752aa8ed7c17a033f4f3cf6c3d3c0f7b9e869a778dd979c430ce5a39fba35d5108ca8673a5346fa074402b4ac0125534364db6fc0c1534519ff1103c28bb750ef04f6a8dfc05f5f7a0e91d62ed5bbc829ebd2744e26962b90dee54de08667cd34af8d3b828184854a780a061019de3b3d1f6c733d4913b3772808e599d2e8d980470aa81c3e48243dc633ba0416a436e0408a27328b6fd0e86a388109e3bad694a91fcea33d2d96289d0100ea0d13ecb788ab0be734b28cf9f4325e7a919089a430a68cfc480cbe26f865cd4f4a0afadb1453cdb5ecf7f00aba20613c01c1e13b074cc1a7fe7bda985bd87be7f29a0f41f807d33b81e9242ef36e78d76535eacf6a56d4e76c90172f402c4b2c3d49ba0b7049a5f24d99db4e56ef232123ef57b4e914ae248ef90d7669c509950aa461d80a0d5034c2de38cc8a03897b2c05ec10b60a967a680609d847c86979dd63b790d198080",
    "0xed9f20009618bfc1e28856249ac8de09d8fb0e08429898a208e1a3c7733d9ef8a98c8b807d22dda6f6d532594bdc"
];

/// Looks for situations where storage keys are removed by a block. Returns internal nodes
/// critical for trie updates in those scenarios.
pub fn demo_detect_removed_storage(_pre: BlockProofs, _post: BlockProofs) -> TrieNodeOracle {
    // Look for storage keys that have value = 0x0 post-state.
    // let at_risk_keys = todo!(); // in post

    // Find at risk grandparent nodes. This is the node whose child is likely to be deleted
    // (branch node with two children - but could be more if many deletions occur.). So basically
    // the lowest node whose child is a branch node.
    // Find the traversal index of that node.
    // . let at_risk_keys_with_traversal_indices = todo!(); // in pre

    // Find the node in post-state that exists at that spot in the traversal.
    // in post
    let mut oracle = TrieNodeOracle::default();

    let address = H160::from_str(DEMO_ACCOUNT).unwrap();
    let nodes = DEMO_NODES
        .into_iter()
        .map(|node_string| hex_decode(node_string).unwrap())
        .collect();
    oracle.insert_nodes(address, vec![0xa, 0x9], nodes);

    let address = H160::from_str(DEMO_ACCOUNT_2).unwrap();

    let nodes = DEMO_NODES_2
        .into_iter()
        .map(|node_string| hex_decode(node_string).unwrap())
        .collect();
    oracle.insert_nodes(address, vec![0x8, 0x6, 0x6, 0x5], nodes);

    let address = H160::from_str(DEMO_ACCOUNT_3).unwrap();
    let nodes = DEMO_NODES_3
        .into_iter()
        .map(|node_string| hex_decode(node_string).unwrap())
        .collect();
    oracle.insert_nodes(address, vec![0x5, 0x7], nodes);

    let address = H160::from_str(DEMO_ACCOUNT_4).unwrap();
    let nodes = DEMO_NODES_4
        .into_iter()
        .map(|node_string| hex_decode(node_string).unwrap())
        .collect();
    oracle.insert_nodes(address, vec![0xb, 0x6, 0xa, 0x0], nodes);

    oracle
}

/// Tries to apply known state transition to a multiproof. The oracle is then constructed from
/// the known values.
///
/// This is a "simulation" because the state is not coming from running the EVM, it is coming
/// from pre- and post- block state proofs.
pub fn oracle_from_simulated_state_update(
    pre: BlockProofs,
    post: BlockProofs,
) -> Result<TrieNodeOracle, OracleError> {
    // Detect places where an oracle is required.
    let mut updates: Vec<InterestingUpdate> = vec![];
    for (address, account) in post.proofs.iter() {
        for storage_proof_post in &account.storage_proof {
            let key = storage_proof_post.key;

            let acc_proof_pre = pre
                .proofs
                .get(address)
                .ok_or_else(|| OracleError::NoPreStateAddress(hex_encode(address)))?;
            let val_pre: U256 = acc_proof_pre
                .storage_proof
                .iter()
                .find(|x| x.key == key)
                .ok_or_else(|| OracleError::NoPostStateKey {
                    address: hex_encode(address),
                    key: hex_encode(key),
                })?
                .value;
            let val_post = storage_proof_post.value;

            if !storage_created_or_destroyed(&val_pre, &val_post) {
                continue;
            }
            updates.push(InterestingUpdate {
                address: address.to_owned(),
                key: key.to_owned(),
                value: storage_proof_post.value,
            });
        }
    }
    // sort updates by storage key for consistency. If keys are sorted while executing the
    // post-execution changes, trie updates that require an oracle will be simpler.
    updates.sort_by_key(|x| x.key);

    // set up multiproof for pre-block state.
    let mut multiproof_pre = EIP1186MultiProof::from_separate(
        pre.proofs.into_values().collect(),
        HashMap::new(),
        HashMap::new(),
        TrieNodeOracle::default(),
    )?;
    let mut tasks: Vec<OracleTask> = vec![];
    for update in updates {
        let purpose = match update.value.is_zero() {
            true => TaskType::ForExclusion,
            false => TaskType::ForInclusion(rlp::encode(&update.value).to_vec()),
        };
        match multiproof_pre.update_storage_proof(&update.address, update.key, update.value)? {
            ProofOutcome::Root(_) => {
                // Storage update did not require oracle - ignore.
                continue;
            }
            ProofOutcome::IndexForOracle(traversal_index) => tasks.push(OracleTask {
                address: update.address.clone(),
                key: update.key,
                traversal_index,
                purpose,
            }),
        }
    }

    let mut oracle = TrieNodeOracle::default();
    // Populate the oracle
    for task in &tasks {
        let account = post
            .proofs
            .get(&task.address)
            .ok_or(OracleError::NoPostStateAddress(hex_encode(task.address)))?;
        let storage = account
            .storage_proof
            .iter()
            .find(|x| x.key == task.key)
            .ok_or(OracleError::NoPostStateKey {
                address: hex_encode(task.address),
                key: hex_encode(task.key),
            })?;
        // Create a representation of the proof that is easy to traverse.
        let mut proof = MultiProof::init(account.storage_hash);
        proof
            .insert_proof(storage.proof.to_owned())
            .expect("Cachecd proof from RPC expected to be valid.");
        let path = keccak256(task.key);
        // Verify that the oracle-based update resulted in a valid proof.
        let intent = match storage.value.is_zero() {
            true => Intent::VerifyExclusion,
            false => Intent::VerifyInclusion(rlp::encode(&storage.value).to_vec()),
        };
        let visited = proof
            .traverse(path.into(), &intent)
            .expect("All tasks should be for exclusion proofs (in post-block state)");

        // Skip the first part of the proof. Only include the required nodes.
        let mut proof_subset: Vec<Vec<u8>> = vec![];
        for node in visited {
            if node.traversal_record.visiting_index() >= task.traversal_index {
                let node_bytes = proof.get_node(&node.node_hash)?;
                proof_subset.push(node_bytes.to_vec())
            }
        }

        let path_nibbles = NibblePath::init(&path);
        let nibbles_to_target = path_nibbles.traversal_to_index(task.traversal_index)?;
        oracle.insert_nodes(task.address, nibbles_to_target.to_vec(), proof_subset)
    }
    Ok(oracle)
}

/// Detects if storage went (pre- and post- block) from absent to present, or from present to absent. That is, from exclusion proof to inclusion proof or vice versa.
fn storage_created_or_destroyed(val_pre: &U256, val_post: &U256) -> bool {
    let e_to_i = val_pre.is_zero() && !val_post.is_zero();
    let i_to_e = !val_pre.is_zero() && val_post.is_zero();
    e_to_i | i_to_e
}

/// A storage update that occurs in a block that may require an oracle.
///
/// Interesting == exclusion to inclusion proof or vice versa
struct InterestingUpdate {
    address: H160,
    key: H256,
    value: U256,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_storage_created_or_destroyed() {
        let zero = U256::from_str("0x0").unwrap();
        let one = U256::from_str("0x1").unwrap();
        let two = U256::from_str("0x2").unwrap();
        // Oracle may be required.
        assert!(storage_created_or_destroyed(&zero, &one));
        assert!(storage_created_or_destroyed(&one, &zero));
        // Oracle would not be required.
        assert!(!storage_created_or_destroyed(&zero, &zero));
        assert!(!storage_created_or_destroyed(&one, &one));
        assert!(!storage_created_or_destroyed(&one, &two));
    }
}
