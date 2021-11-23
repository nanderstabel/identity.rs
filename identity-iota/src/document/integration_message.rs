// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;

use serde::Deserialize;
use serde::Serialize;

use identity_core::convert::FmtJson;

use crate::did::IotaDID;
use crate::document::iota_meta_document::IotaMetaDocument;
use crate::tangle::{MessageId, TangleRef};
use crate::tangle::MessageIdExt;

/// An IOTA Identity resolved from the Tangle.
///
/// NOTE: see [`DocumentChain`](crate::chain::DocumentChain) for how `diff_message_id` is set.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IntegrationMessage {
  pub identity: IotaMetaDocument,

  // TODO: combine these fields into `IntegrationMetadata`?

  /// Integration chain [`MessageId`].
  #[serde(
  rename = "messageId",
  default = "MessageId::null",
  skip_serializing_if = "MessageIdExt::is_null"
  )]
  pub message_id: MessageId,

  /// [`MessageId`] of the last diff chain message merged into this during resolution.
  /// See [`DocumentChain`](crate::chain::DocumentChain).
  #[serde(
  rename = "diffMessageId",
  default = "MessageId::null",
  skip_serializing_if = "MessageIdExt::is_null"
  )]
  pub diff_message_id: MessageId,

  // TODO: version_id but skip serializing it
}

impl TangleRef for IntegrationMessage {
  fn did(&self) -> &IotaDID {
    self.identity.document.id()
  }

  fn message_id(&self) -> &MessageId {
    &self.message_id
  }

  fn set_message_id(&mut self, message_id: MessageId) {
    self.message_id = message_id;
  }

  fn previous_message_id(&self) -> &MessageId {
    &self.identity.metadata.previous_message_id
  }

  fn set_previous_message_id(&mut self, message_id: MessageId) {
    self.identity.metadata.previous_message_id = message_id;
  }
}

impl Display for IntegrationMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    self.fmt_json(f)
  }
}

impl From<IotaMetaDocument> for IntegrationMessage {
  fn from(identity: IotaMetaDocument) -> Self {
    Self {
      identity,
      message_id: MessageId::null(),
      diff_message_id: MessageId::null(),
    }
  }
}
