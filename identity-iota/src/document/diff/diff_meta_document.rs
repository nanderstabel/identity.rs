// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;

use serde::Deserialize;
use serde::Serialize;

use identity_core::convert::FmtJson;
use identity_core::crypto::{Ed25519, JcsEd25519, KeyPair, PrivateKey, PublicKey, SetSignature, Signature, Signer, TrySignature, TrySignatureMut, Verifier};
use identity_did::verification::{MethodQuery, MethodScope, MethodType, VerificationMethod};

use crate::did::IotaDID;
use crate::document::{DiffMessage, IotaDocument, IotaDocumentMetadata, IotaVerificationMethod};
use crate::Error;
use crate::Result;
use crate::tangle::{MessageIdExt, NetworkName, MessageId};
use identity_core::diff::Diff;
use identity_did::diff::DiffDocument;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct DiffMetaDocument {
  #[serde(skip_serializing_if = "Option::is_none")]
  document: Option<DiffDocument>,
  #[serde(skip_serializing_if = "Option::is_none")]
  metadata: Option<Option<DiffString>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  also_known_as: Option<DiffVec<Url>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  verification_method: Option<DiffVec<VerifiacationMethod<U>>>,
}

impl Diff for IotaMetaDocument {
  type Type = ();

  fn diff(&self, other: &Self) -> identity_core::diff::Result<Self::Type> {
    todo!()
  }

  fn merge(&self, diff: Self::Type) -> identity_core::diff::Result<Self> {
    todo!()
  }

  fn from_diff(diff: Self::Type) -> identity_core::diff::Result<Self> {
    todo!()
  }

  fn into_diff(self) -> identity_core::diff::Result<Self::Type> {
    todo!()
  }
}

impl TrySignature for IotaMetaDocument {
  fn signature(&self) -> Option<&Signature> {
    self.metadata.proof()
  }
}

impl TrySignatureMut for IotaMetaDocument {
  fn signature_mut(&mut self) -> Option<&mut Signature> {
    self.metadata.proof_mut()
  }
}

impl SetSignature for IotaMetaDocument {
  fn set_signature(&mut self, signature: Signature) {
    self.metadata.set_proof(signature)
  }
}

impl Display for IotaMetaDocument {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.fmt_json(f)
  }
}

#[cfg(test)]
mod tests {}
