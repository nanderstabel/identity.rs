// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;

use serde::Deserialize;
use serde::Serialize;

use identity_core::common::{Object, Timestamp};
use identity_core::convert::FmtJson;
use identity_core::crypto::Signature;
use identity_did::verifiable::VerifiableProperties;

use crate::tangle::MessageId;
use crate::tangle::MessageIdExt;

/// Additional attributes related to an IOTA DID Document.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IotaDocumentMetadata {
  pub created: Timestamp,
  pub updated: Timestamp,
  #[serde(
  rename = "previousMessageId",
  default = "MessageId::null",
  skip_serializing_if = "MessageIdExt::is_null"
  )]
  pub previous_message_id: MessageId,
  /// `VerifiableProperties` contains the proof / signature section.
  #[serde(flatten)]
  pub properties: VerifiableProperties,
}

impl IotaDocumentMetadata {
  pub fn new() -> Self {
    let now: Timestamp = Timestamp::now_utc();
    Self {
      created: now,
      updated: now,
      previous_message_id: MessageId::null(),
      properties: VerifiableProperties::new(Object::new()),
    }
  }

  /// Returns a reference to the [`proof`][`Signature`].
  pub fn proof(&self) -> Option<&Signature> {
    self.properties.proof()
  }

  /// Returns a mutable reference to the [`proof`][`Signature`].
  pub fn proof_mut(&mut self) -> Option<&mut Signature> {
    self.properties.proof_mut()
  }

  /// Sets the value of the [`proof`][`Signature`].
  pub fn set_proof(&mut self, signature: Signature) {
    self.properties.set_proof(signature)
  }
}

impl Default for IotaDocumentMetadata {
  fn default() -> Self {
    Self::new()
  }
}

impl Display for IotaDocumentMetadata {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    self.fmt_json(f)
  }
}
