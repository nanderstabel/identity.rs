// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Display;
use core::fmt::Error as FmtError;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use core::mem;

use log::debug;
use log::trace;
use serde;
use serde::Deserialize;
use serde::Serialize;

use identity_core::convert::ToJson;

use crate::did::IotaDID;
use crate::document::{IntegrationMessage, IotaMetaDocument};
use crate::error::Error;
use crate::error::Result;
use crate::tangle::Message;
use crate::tangle::MessageExt;
use crate::tangle::MessageId;
use crate::tangle::MessageIdExt;
use crate::tangle::MessageIndex;
use crate::tangle::TangleRef;

/// Primary chain of [`IotaIdentityMessages`](IotaIdentityMessage) holding the latest full
/// DID document and its history.
///
/// See also [`DiffChain`](crate::chain::DiffChain)
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IntegrationChain {
  #[serde(skip_serializing_if = "Option::is_none")]
  history: Option<Vec<IntegrationMessage>>,
  current: IntegrationMessage,
}

impl IntegrationChain {
  /// Creates a new [`IntegrationChain`] with `current` as the root document message and no history.
  ///
  /// Use [`IntegrationChain::try_from_messages`] or [`IntegrationChain::try_from_index`] instead.
  pub fn new(current: IntegrationMessage) -> Result<Self> {
    if IotaMetaDocument::verify_root(&current.identity).is_err() {
      return Err(Error::ChainError {
        error: "Invalid Root Document",
      });
    }

    if current.message_id().is_null() {
      return Err(Error::ChainError {
        error: "Invalid Message Id",
      });
    }

    Ok(Self { current, history: None })
  }

  /// Constructs a new [`IntegrationChain`] from a slice of [`Message`]s.
  pub fn try_from_messages(did: &IotaDID, messages: &[Message]) -> Result<Self> {
    let index: MessageIndex<IntegrationMessage> = messages
      .iter()
      .flat_map(|message| message.try_extract_integration(did))
      .collect();

    debug!("[Int] Valid Messages = {}/{}", messages.len(), index.len());

    Self::try_from_index(index)
  }

  /// Constructs a new [`IntegrationChain`] from the given [`MessageIndex`].
  pub fn try_from_index(mut index: MessageIndex<IntegrationMessage>) -> Result<Self> {
    trace!("[Int] Message Index = {:#?}", index);

    // Extract root document.
    let current: IntegrationMessage = index
      .remove_where(&MessageId::null(), |message| {
        IotaMetaDocument::verify_root(&message.identity).is_ok()
      })
      .ok_or(Error::ChainError {
        error: "Invalid Root Document",
      })?;

    // Construct the document chain.
    let mut this: Self = Self::new(current)?;
    while let Some(mut list) = index.remove(this.current_message_id()) {
      'inner: while let Some(document) = list.pop() {
        if this.try_push(document).is_ok() {
          break 'inner;
        }
      }
    }

    Ok(this)
  }

  /// Returns a reference to the latest [`IotaIdentityMessage`].
  pub fn current(&self) -> &IntegrationMessage {
    &self.current
  }

  /// Returns a mutable reference to the latest [`IotaIdentityMessage`].
  pub fn current_mut(&mut self) -> &mut IntegrationMessage {
    &mut self.current
  }

  /// Returns the Tangle message id of the latest integration [`IotaDocument`].
  pub fn current_message_id(&self) -> &MessageId {
    self.current.message_id()
  }

  /// Returns a slice of [`IotaDocuments`](IotaDocument) in the integration chain, if present.
  /// This excludes the current document.
  pub fn history(&self) -> Option<&[IntegrationMessage]> {
    self.history.as_deref()
  }

  /// Adds a new [`IotaDocument`] to this [`IntegrationChain`].
  ///
  /// # Errors
  ///
  /// Fails if the [`IotaDocument`] is not a valid addition.
  /// See [`IntegrationChain::check_valid_addition`].
  pub fn try_push(&mut self, integration_message: IntegrationMessage) -> Result<()> {
    self.check_valid_addition(&integration_message)?;

    self
      .history
      .get_or_insert_with(Vec::new)
      .push(mem::replace(&mut self.current, integration_message));

    Ok(())
  }

  /// Returns `true` if the [`IotaDocument`] can be added to this [`IntegrationChain`].
  ///
  /// See [`IntegrationChain::check_valid_addition`].
  pub fn is_valid_addition(&self, integration_message: &IntegrationMessage) -> bool {
    self.check_valid_addition(integration_message).is_ok()
  }

  /// Checks if the [`IotaDocument`] can be added to this [`IntegrationChain`].
  ///
  /// NOTE: the checks here are not exhaustive (e.g. the document `message_id` is not verified to
  /// have been published and contain the same contents on the Tangle).
  ///
  /// # Errors
  ///
  /// Fails if the document signature is invalid or the Tangle message
  /// references within the [`IotaIdentityMessage`] are invalid.
  pub fn check_valid_addition(&self, integration_message: &IntegrationMessage) -> Result<()> {
    if integration_message.identity.document.id() != self.current.identity.document.id() {
      return Err(Error::ChainError { error: "Invalid DID" });
    }

    if integration_message.message_id().is_null() {
      return Err(Error::ChainError {
        error: "Missing Message Id",
      });
    }

    if integration_message.previous_message_id().is_null() {
      return Err(Error::ChainError {
        error: "Missing Previous Message Id",
      });
    }

    if self.current_message_id() != integration_message.previous_message_id() {
      return Err(Error::ChainError {
        error: "Invalid Previous Message Id",
      });
    }

    // Verify the next document was signed by a valid method from the previous document.
    if IotaMetaDocument::verify_meta_document(&integration_message.identity, &self.current.identity.document).is_err() {
      return Err(Error::ChainError {
        error: "Invalid Signature",
      });
    }

    Ok(())
  }
}

impl Display for IntegrationChain {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    if f.alternate() {
      f.write_str(&self.to_json_pretty().map_err(|_| FmtError)?)
    } else {
      f.write_str(&self.to_json().map_err(|_| FmtError)?)
    }
  }
}

/// Convert an [`IntegrationChain`] into an ordered list of documents with the current document
/// as the last entry.
impl From<IntegrationChain> for Vec<IntegrationMessage> {
  fn from(integration_chain: IntegrationChain) -> Self {
    let mut messages: Vec<IntegrationMessage> = integration_chain.history.unwrap_or_default();
    messages.push(integration_chain.current);
    messages
  }
}
