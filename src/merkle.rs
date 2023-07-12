use crate::kvpair::Hash;

use std::error::Error;
use std::fmt;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};
pub use utils::*;

pub mod utils {
    use super::*;
    use crate::proto::NodeType;

    pub fn get_offset(index: u32) -> u32 {
        let height = (index + 1).ilog2();
        let full = (1u32 << height) - 1;
        index - full
    }

    pub fn get_node_type(index: u32, height: usize) -> NodeType {
        assert!(height < 32);
        let height = height as u32;
        if index >= (2_u32.pow(height + 1) - 1) {
            NodeType::NodeInvalid
        } else if index >= (2_u32.pow(height) - 1) {
            NodeType::NodeLeaf
        } else {
            NodeType::NodeNonLeaf
        }
    }

    pub fn boundary_check(index: u32, height: usize) -> Result<(), MerkleError> {
        let node_type = get_node_type(index, height);
        if node_type == NodeType::NodeInvalid {
            Err(MerkleError::new(
                [0; 32].into(),
                index,
                MerkleErrorCode::InvalidIndex,
            ))
        } else {
            Ok(())
        }
    }

    /*
     * Check that an index is a leaf.
     * Example: Given D=2 and a merkle tree as follows:
     * 0
     * 1 2
     * 3 4 5 6
     * then leaf index >= 3 which is (2^D - 1)
     *
     * Moreover, nodes at depth k start at
     * first = 2^k-1, last = 2^{k+1}-2
     */
    pub fn leaf_check(index: u32, height: usize) -> Result<(), MerkleError> {
        let node_type = get_node_type(index, height);
        if node_type != NodeType::NodeLeaf {
            Err(MerkleError::new(
                [0; 32].into(),
                index,
                MerkleErrorCode::InvalidLeafIndex,
            ))
        } else {
            Ok(())
        }
    }

    pub fn get_sibling_index(index: u32) -> u32 {
        if index % 2 == 1 {
            index + 1
        } else {
            index - 1
        }
    }

    /// get the index from leaf to the root
    /// root index is not included in the result as root index is always 0
    /// Example: Given D=3 and a merkle tree as follows:
    /// 0
    /// 1 2
    /// 3 4 5 6
    /// 7 8 9 10 11 12 13 14
    /// get_path(7) = [3, 1]
    /// get_path(15) = [6, 2]
    pub fn get_path(index: u32, height: usize) -> Result<Vec<u32>, MerkleError> {
        leaf_check(index, height)?;
        let mut height = (index + 1).ilog2();
        let round = height;
        let full = (1u32 << height) - 1;
        let mut p = index - full;
        let mut path = vec![];
        for _ in 0..round {
            let full = (1u32 << height) - 1;
            // Calculate the index of current node
            let i = full + p;
            path.insert(0, i);
            height -= 1;
            // Caculate the offset of parent
            p /= 2;
        }
        assert!(p == 0);
        Ok(path)
    }
}

/*
const LEAF_SIG: u8 = 0u8;
const INTERNAL_SIG: u8 = 1u8;
*/

#[derive(Debug)]
pub enum MerkleErrorCode {
    InvalidLeafIndex,
    InvalidHash,
    InvalidDepth,
    InvalidIndex,
}

#[derive(Debug)]
pub struct MerkleError {
    source: Hash,
    index: u32,
    code: MerkleErrorCode,
}

impl MerkleError {
    pub fn new(source: Hash, index: u32, code: MerkleErrorCode) -> Self {
        MerkleError {
            source,
            index,
            code,
        }
    }
}

impl fmt::Display for MerkleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MerkleError {:?} {:?} {:?}",
            self.source, self.index, self.code
        )
    }
}

impl Error for MerkleError {}

pub trait MerkleNode<H: Debug + Clone + PartialEq> {
    fn hash(&self) -> H;
    fn index(&self) -> u32;
    fn set(&mut self, data: &Vec<u8>);
    fn left(&self) -> Option<H>; // hash of left child
    fn right(&self) -> Option<H>; // hash of right child
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MerkleProof<H: Debug + Clone + PartialEq + Serialize, const D: usize> {
    pub source: H,
    pub root: H, // last is root
    pub assist: Vec<H>,
    pub index: u32,
}

pub trait MerkleTree<H: Debug + Clone + PartialEq + Serialize, const D: usize> {
    type Node: MerkleNode<H>;
    type Id;
    type Root;

    /// Create a new merkletree and connect it with a given merkle root.
    /// If the root is None then the default root with all leafs are empty is used.
    fn construct(addr: Self::Id, id: Self::Root) -> Self;

    fn hash(a: &H, b: &H) -> H;
    fn set_parent(&mut self, index: u32, hash: &H, left: &H, right: &H) -> Result<(), MerkleError>;
    fn set_leaf(&mut self, leaf: &Self::Node) -> Result<(), MerkleError>;
    fn get_node_with_hash(&mut self, index: u32, hash: &H) -> Result<Self::Node, MerkleError>;

    fn get_root_hash(&self) -> H;
    fn update_root_hash(&mut self, hash: &H);

    fn boundary_check(&self, index: u32) -> Result<(), MerkleError> {
        boundary_check(index, D)
    }

    fn leaf_check(&self, index: u32) -> Result<(), MerkleError> {
        leaf_check(index, D)
    }

    fn get_sibling_index(&self, index: u32) -> u32 {
        get_sibling_index(index)
    }

    /// get the index from leaf to the root
    /// root index is not included in the result as root index is always 0
    /// Example: Given D=3 and a merkle tree as follows:
    /// 0
    /// 1 2
    /// 3 4 5 6
    /// 7 8 9 10 11 12 13 14
    /// get_path(7) = [3, 1]
    /// get_path(15) = [6, 2]
    fn get_path(&self, index: u32) -> Result<[u32; D], MerkleError> {
        Ok(get_path(index, D)?.try_into().unwrap())
    }

    fn get_leaf_with_proof(
        &mut self,
        index: u32,
    ) -> Result<(Self::Node, MerkleProof<H, D>), MerkleError> {
        self.leaf_check(index)?;
        let paths = self.get_path(index)?.to_vec();
        // We push the search from the top
        let hash = self.get_root_hash();
        let mut acc = 0;
        let mut acc_node = self.get_node_with_hash(acc, &hash)?;
        let assist: Vec<H> = paths
            .into_iter()
            .map(|child| {
                let (hash, sibling_hash) = if (acc + 1) * 2 == child + 1 {
                    // left child
                    (acc_node.left().unwrap(), acc_node.right().unwrap())
                } else {
                    assert!((acc + 1) * 2 == child);
                    (acc_node.right().unwrap(), acc_node.left().unwrap())
                };
                let sibling = self.get_sibling_index(child);
                let sibling_node = self.get_node_with_hash(sibling, &sibling_hash)?;
                acc = child;
                acc_node = self.get_node_with_hash(acc, &hash)?;
                Ok(sibling_node.hash())
            })
            .collect::<Result<Vec<H>, _>>()?;
        let hash = acc_node.hash();
        Ok((
            acc_node,
            MerkleProof {
                source: hash,
                root: self.get_root_hash(),
                assist: assist.try_into().unwrap(),
                index,
            },
        ))
    }

    fn set_leaf_with_proof(&mut self, leaf: &Self::Node) -> Result<MerkleProof<H, D>, MerkleError> {
        let index = leaf.index();
        let mut hash = leaf.hash();
        let (_, mut proof) = self.get_leaf_with_proof(index)?;
        proof.source = hash.clone();
        let mut p = get_offset(index);
        self.set_leaf(leaf)?;
        for i in 0..D {
            let cur_hash = hash;
            let depth = D - i - 1;
            let (left, right) = if p % 2 == 1 {
                (&proof.assist[depth], &cur_hash)
            } else {
                (&cur_hash, &proof.assist[depth])
            };
            hash = Self::hash(left, right);
            p /= 2;
            let index = p + (1 << depth) - 1;
            self.set_parent(index, &hash, left, right)?;
        }
        self.update_root_hash(&hash);
        proof.root = hash;
        Ok(proof)
    }

    fn update_leaf_data_with_proof(
        &mut self,
        index: u32,
        data: &Vec<u8>,
    ) -> Result<MerkleProof<H, D>, MerkleError> {
        let (mut leaf, _) = self.get_leaf_with_proof(index)?;
        leaf.set(data);
        self.set_leaf_with_proof(&leaf)
    }

    fn verify_proof(&mut self, proof: MerkleProof<H, D>) -> Result<bool, MerkleError> {
        let init = proof.source;
        let mut p = get_offset(proof.index);
        let hash = proof.assist.to_vec().iter().fold(init, |acc, x| {
            let (left, right) = if p % 2 == 1 { (x, &acc) } else { (&acc, x) };
            p /= 2;
            Self::hash(left, right)
        });
        Ok(proof.root == hash)
    }
}

#[cfg(test)]
mod tests {
    use crate::merkle::{MerkleError, MerkleNode, MerkleTree};
    struct MerkleAsArray {
        data: [u64; 127], // 2^7-1 and depth = 6
    }

    impl MerkleAsArray {
        fn debug(&self) {
            let mut start = 0;
            for i in 0..6 {
                let mut ns = vec![];
                for j in start..start + (1 << i) {
                    ns.push(self.data[j])
                }
                start += 1 << i;
                println!("dbg: {:?}", ns)
            }
        }
    }

    struct MerkleU64Node {
        pub value: u64,
        pub index: u32,
    }

    impl MerkleNode<u64> for MerkleU64Node {
        fn index(&self) -> u32 {
            self.index
        }
        fn hash(&self) -> u64 {
            self.value
        }
        fn set(&mut self, value: &Vec<u8>) {
            let v: [u8; 8] = value.clone().try_into().unwrap();
            self.value = u64::from_le_bytes(v);
        }
        fn right(&self) -> Option<u64> {
            Some(0)
        }
        fn left(&self) -> Option<u64> {
            Some(0)
        }
    }

    impl MerkleTree<u64, 6> for MerkleAsArray {
        type Id = String;
        type Root = String;
        type Node = MerkleU64Node;
        fn construct(_addr: Self::Id, _id: Self::Root) -> Self {
            MerkleAsArray { data: [0_u64; 127] }
        }
        fn hash(a: &u64, b: &u64) -> u64 {
            a + b
        }
        fn get_root_hash(&self) -> u64 {
            self.data[0]
        }
        fn update_root_hash(&mut self, _h: &u64) {}

        fn get_node_with_hash(
            &mut self,
            index: u32,
            _hash: &u64,
        ) -> Result<Self::Node, MerkleError> {
            self.boundary_check(index)?;
            Ok(MerkleU64Node {
                value: self.data[index as usize],
                index,
            })
        }

        fn set_parent(
            &mut self,
            index: u32,
            hash: &u64,
            _left: &u64,
            _right: &u64,
        ) -> Result<(), MerkleError> {
            self.boundary_check(index)?;
            self.data[index as usize] = *hash;
            Ok(())
        }
        fn set_leaf(&mut self, leaf: &Self::Node) -> Result<(), MerkleError> {
            self.leaf_check(leaf.index())?;
            self.data[leaf.index() as usize] = leaf.value;
            Ok(())
        }
    }

    #[test]
    fn test_merkle_path() {
        let mut mt = MerkleAsArray::construct("test".to_string(), "test".to_string());
        let (mut leaf, _) = mt.get_leaf_with_proof(2_u32.pow(6) - 1).unwrap();
        leaf.value = 1;
        let _proof = mt.set_leaf_with_proof(&leaf).unwrap();

        /* one update of 1 is 1 */
        let root = mt.get_root_hash();
        mt.debug();
        assert_eq!(root, 1_u64);

        let (mut leaf, _) = mt.get_leaf_with_proof(2_u32.pow(6) + 2).unwrap();
        leaf.value = 2;
        let _proof = mt.set_leaf_with_proof(&leaf).unwrap();

        /* two leaves hash needs to be 3 */
        let root = mt.get_root_hash();
        mt.debug();
        assert_eq!(root, 3_u64);

        let (mut leaf, _) = mt.get_leaf_with_proof(2_u32.pow(6) + 4).unwrap();
        leaf.value = 3;
        let _proof = mt.set_leaf_with_proof(&leaf).unwrap();
        /* two leaves hash needs to be 3 */
        let root = mt.get_root_hash();
        assert_eq!(root, 6_u64);
    }
}
