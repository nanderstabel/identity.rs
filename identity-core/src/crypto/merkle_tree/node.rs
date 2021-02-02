// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Debug;
use core::fmt::Formatter;
use core::fmt::Result;
use digest::Digest;

use crate::crypto::merkle_tree::DigestExt;
use crate::crypto::merkle_tree::Hash;

/// A tagged [`struct@Hash`].
pub enum Node<D: Digest> {
  /// A node tagged with `L`.
  L(Hash<D>),
  /// A node tagged with `R`.
  R(Hash<D>),
}

impl<D: Digest> Node<D> {
  /// Returns the [`struct@Hash`] of the node.
  pub fn get(&self) -> &Hash<D> {
    match self {
      Self::L(hash) => hash,
      Self::R(hash) => hash,
    }
  }

  /// Computes the parent hash of `self` and `other` using a default digest.
  pub fn hash(&self, other: &Hash<D>) -> Hash<D> {
    self.hash_with(&mut D::new(), other)
  }

  /// Computes the parent hash of `self` and `other` using the given `digest`.
  pub fn hash_with(&self, digest: &mut D, other: &Hash<D>) -> Hash<D> {
    match self {
      Self::L(hash) => digest.hash_branch(hash, other),
      Self::R(hash) => digest.hash_branch(other, hash),
    }
  }
}

impl<D: Digest> Debug for Node<D> {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    match self {
      Self::L(hash) => f.write_fmt(format_args!("L({:?})", hash)),
      Self::R(hash) => f.write_fmt(format_args!("R({:?})", hash)),
    }
  }
}

#[cfg(test)]
mod tests {
  use digest::Digest;
  use sha2::Sha256;

  use crate::crypto::merkle_tree::DigestExt;
  use crate::crypto::merkle_tree::Hash;
  use crate::crypto::merkle_tree::Node;

  #[test]
  fn test_hash() {
    let mut digest: Sha256 = Sha256::new();

    let h1: Hash<Sha256> = digest.hash_leaf(b"A");
    let h2: Hash<Sha256> = digest.hash_leaf(b"B");

    assert_eq!(Node::L(h1).hash(&h2), digest.hash_branch(&h1, &h2));
    assert_eq!(Node::R(h1).hash(&h2), digest.hash_branch(&h2, &h1));

    assert_eq!(Node::L(h2).hash(&h1), digest.hash_branch(&h2, &h1));
    assert_eq!(Node::R(h2).hash(&h1), digest.hash_branch(&h1, &h2));
  }
}
