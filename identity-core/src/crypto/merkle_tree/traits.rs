// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::crypto::key::KeyCouple;
use crate::crypto::merkle_tree::DigestExt;
use crate::crypto::merkle_tree::Hash;
use crate::crypto::PublicKey;

mod private {
  pub trait Sealed {}
}

/// A helper trait for computing hash values used in Merkle tree operations.
pub trait AsLeaf<D>: private::Sealed
where
  D: DigestExt,
{
  /// Hashes `self` with the digest implentation `D`.
  fn hash(&self, digest: &mut D) -> Hash<D>;
}

impl<D> private::Sealed for Hash<D> where D: DigestExt {}

impl<T> private::Sealed for T where T: AsRef<[u8]> {}

impl private::Sealed for KeyCouple {}

impl<D> AsLeaf<D> for KeyCouple
where
  D: DigestExt,
{
  fn hash(&self, digest: &mut D) -> Hash<D> {
    //Only work with the public key
    // and make this very explicit
    let public: &PublicKey = &self.0;
    digest.hash_leaf(public.as_ref())
  }
}

impl<T, D> AsLeaf<D> for T
where
  T: AsRef<[u8]>,
  D: DigestExt,
{
  fn hash(&self, digest: &mut D) -> Hash<D> {
    digest.hash_leaf(self.as_ref())
  }
}

impl<D> AsLeaf<D> for Hash<D>
where
  D: DigestExt,
{
  fn hash(&self, _: &mut D) -> Hash<D> {
    // SAFETY: `self` is already a `Hash` and should not require a length check
    unsafe { Self::from_slice_unchecked(self.as_slice()) }
  }
}
