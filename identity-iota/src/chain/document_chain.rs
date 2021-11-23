// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Display;
use core::fmt::Error as FmtError;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;

use serde::Deserialize;
use serde::Serialize;

use identity_core::convert::ToJson;

use crate::chain::DiffChain;
use crate::chain::IntegrationChain;
use crate::did::IotaDID;
use crate::document::{DiffMessage, IntegrationMessage};
use crate::error::Result;
use crate::tangle::MessageId;
use crate::tangle::TangleRef;

/// Holds an [`IntegrationChain`] and its corresponding [`DiffChain`] that can be used to resolve the
/// latest version of an [`IotaIdentityMessage`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentChain {
  chain_i: IntegrationChain,
  chain_d: DiffChain,
  #[serde(skip_serializing_if = "Option::is_none")]
  identity: Option<IntegrationMessage>,
}

impl DocumentChain {
  pub(crate) fn __diff_message_id<'a>(chain_i: &'a IntegrationChain, diff: &'a DiffChain) -> &'a MessageId {
    diff
      .current_message_id()
      .unwrap_or_else(|| chain_i.current_message_id())
  }

  pub(crate) fn __fold(chain_i: &IntegrationChain, chain_d: &DiffChain) -> Result<IntegrationMessage> {
    let mut current: IntegrationMessage = chain_i.current().clone();

    // Apply the changes from the diff chain.
    for diff in chain_d.iter() {
      current = Self::merge_diff_message(current, &diff)?;
    }

    Ok(current)
  }

  /// Applies the changes from a [`DiffMessage`] to an [`IntegrationMessage`], also updating its
  /// `diff_message_id`.
  ///
  /// Assumes the diff is validated by the `DiffChain` and does not make any disallowed changes.
  pub fn merge_diff_message(mut identity_message: IntegrationMessage, diff_message: &DiffMessage) -> Result<IntegrationMessage> {
    identity_message.identity.merge(&diff_message)?;
    identity_message.diff_message_id = *diff_message.message_id();
    Ok(identity_message)
  }

  /// Creates a new [`DocumentChain`] from the given [`IntegrationChain`].
  pub fn new(chain_i: IntegrationChain) -> Self {
    Self {
      chain_i,
      chain_d: DiffChain::new(),
      identity: None,
    }
  }

  /// Creates a new [`DocumentChain`] from given the [`IntegrationChain`] and [`DiffChain`].
  pub fn new_with_diff_chain(chain_i: IntegrationChain, chain_d: DiffChain) -> Result<Self> {
    let identity: Option<IntegrationMessage> = if chain_d.is_empty() {
      None
    } else {
      Some(Self::__fold(&chain_i, &chain_d)?)
    };

    Ok(Self {
      chain_d,
      chain_i,
      identity,
    })
  }

  /// Returns a reference to the [`IotaDID`] identifying this document chain.
  pub fn id(&self) -> &IotaDID {
    self.chain_i.current().identity.document.id()
  }

  /// Returns a reference to the [`IntegrationChain`].
  pub fn integration_chain(&self) -> &IntegrationChain {
    &self.chain_i
  }

  /// Returns a mutable reference to the [`IntegrationChain`].
  pub fn integration_chain_mut(&mut self) -> &mut IntegrationChain {
    &mut self.chain_i
  }

  /// Returns a reference to the [`DiffChain`].
  pub fn diff(&self) -> &DiffChain {
    &self.chain_d
  }

  /// Returns a mutable reference to the [`DiffChain`].
  pub fn diff_mut(&mut self) -> &mut DiffChain {
    &mut self.chain_d
  }

  /// Merges the changes from the [`DiffChain`] into the current [`IotaDocument`] from
  /// the [`IntegrationChain`].
  pub fn fold(self) -> Result<IntegrationMessage> {
    Self::__fold(&self.chain_i, &self.chain_d)
  }

  /// Returns a reference to the latest [`IotaDocument`].
  pub fn current(&self) -> &IntegrationMessage {
    self.identity.as_ref().unwrap_or_else(|| self.chain_i.current())
  }

  /// Returns a mutable reference to the latest [`IotaDocument`].
  pub fn current_mut(&mut self) -> &mut IntegrationMessage {
    self.identity.as_mut().unwrap_or_else(|| self.chain_i.current_mut())
  }

  /// Returns the Tangle [`MessageId`] of the latest integration [`IotaDocument`].
  pub fn integration_message_id(&self) -> &MessageId {
    self.chain_i.current_message_id()
  }

  /// Returns the Tangle [`MessageId`] of the latest diff or integration [`IotaDocument`].
  pub fn diff_message_id(&self) -> &MessageId {
    Self::__diff_message_id(&self.chain_i, &self.chain_d)
  }

  /// Adds a new integration document to the chain.
  ///
  /// # Errors
  ///
  /// Fails if the document is not a valid integration document.
  pub fn try_push_integration(&mut self, integration_message: IntegrationMessage) -> Result<()> {
    self.chain_i.try_push(integration_message)?;
    self.chain_d.clear();

    self.identity = None;

    Ok(())
  }

  /// Adds a new [`DocumentDiff`] to the chain.
  ///
  /// # Errors
  ///
  /// Fails if the diff is invalid.
  pub fn try_push_diff(&mut self, diff: DiffMessage) -> Result<()> {
    // Use the last integration chain document to validate the signature on the diff.
    let integration_message: &IntegrationMessage = self.chain_i.current();
    let expected_prev_message_id: &MessageId = self.diff_message_id();
    DiffChain::check_valid_addition(&diff, &integration_message.identity.document, expected_prev_message_id)?;

    // Merge the diff into the latest state.
    let mut current: IntegrationMessage = self.identity.take().unwrap_or_else(|| self.chain_i.current().clone());
    current = Self::merge_diff_message(current, &diff)?;

    // Extend the diff chain and store the merged result.
    self.chain_d.try_push(diff, &self.chain_i)?;
    self.identity = Some(current);

    Ok(())
  }
}

impl Display for DocumentChain {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    if f.alternate() {
      f.write_str(&self.to_json_pretty().map_err(|_| FmtError)?)
    } else {
      f.write_str(&self.to_json().map_err(|_| FmtError)?)
    }
  }
}
