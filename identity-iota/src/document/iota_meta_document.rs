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

/// An IOTA DID Document and its metadata.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct IotaMetaDocument {
  pub document: IotaDocument,
  pub metadata: IotaDocumentMetadata,
}

impl IotaMetaDocument {
  pub const DEFAULT_METHOD_FRAGMENT: &'static str = "sign-0";

  /// Creates a new DID Document from the given [`KeyPair`].
  ///
  /// The DID Document will be pre-populated with a single verification method
  /// derived from the provided [`KeyPair`] embedded as a capability invocation
  /// verification relationship. This method will have the DID URL fragment
  /// `#sign-0` and can be easily retrieved with [`IotaDocument::default_signing_method`].
  ///
  /// NOTE: the generated document is unsigned, see [`IotaMetaDocument::sign_self`].
  ///
  /// Example:
  ///
  /// ```
  /// # use identity_core::crypto::KeyPair;
  /// # use identity_iota::document::IotaMetaDocument;
  /// #
  /// // Create a DID Document from a new Ed25519 keypair.
  /// let keypair = KeyPair::new_ed25519().unwrap();
  /// let metadoc = IotaMetaDocument::new(&keypair).unwrap();
  /// ```
  pub fn new(keypair: &KeyPair) -> Result<Self> {
    Self::new_with_options(keypair, None, None)
  }

  /// Creates a new DID Document from the given [`KeyPair`], network, and verification method
  /// fragment name.
  ///
  /// See [`IotaMetaDocument::new`].
  ///
  /// Arguments:
  ///
  /// * keypair: the initial verification method is derived from the public key of this [`KeyPair`].
  /// * network: Tangle network to use for the DID; default [`Network::Mainnet`](crate::tangle::Network::Mainnet).
  /// * fragment: name of the initial verification method; default [`DEFAULT_METHOD_FRAGMENT`].
  ///
  /// Example:
  ///
  /// ```
  /// # use identity_core::crypto::KeyPair;
  /// # use identity_iota::document::IotaMetaDocument;
  /// # use identity_iota::tangle::Network;
  /// #
  /// // Create a new DID Document for the devnet from a new Ed25519 keypair.
  /// let keypair = KeyPair::new_ed25519().unwrap();
  /// let metadoc = IotaMetaDocument::new_with_options(&keypair, Some(Network::Devnet.name()), Some("auth-key")).unwrap();
  /// assert_eq!(metadoc.document.id().network_str(), "dev");
  /// assert_eq!(
  ///   metadoc.document.default_signing_method().unwrap().try_into_fragment().unwrap(),
  ///   "#auth-key"
  /// );
  /// ```
  pub fn new_with_options(keypair: &KeyPair, network: Option<NetworkName>, fragment: Option<&str>) -> Result<Self> {
    let public_key: &PublicKey = keypair.public();

    let did: IotaDID = if let Some(network_name) = network {
      IotaDID::new_with_network(public_key.as_ref(), network_name)?
    } else {
      IotaDID::new(public_key.as_ref())?
    };

    let method: IotaVerificationMethod =
      IotaVerificationMethod::from_did(did, keypair, fragment.unwrap_or(Self::DEFAULT_METHOD_FRAGMENT))?;

    let document = IotaDocument::from_verification_method(method)?;
    let metadata = IotaDocumentMetadata::new();
    Ok(Self {
      document,
      metadata,
    })
  }

  /// Signs the meta-document with the verification method specified by `method_query`.
  /// The `method_query` may be the full [`IotaDIDUrl`] of the method or just its fragment,
  /// e.g. "#sign-0". The signing method must have a capability invocation verification
  /// relationship.
  ///
  /// NOTE: does not validate whether `private_key` corresponds to the verification method.
  /// See [`IotaMetaDocument::verify_meta_document`].
  ///
  /// # Errors
  ///
  /// Fails if an unsupported verification method is used or the signature operation fails.
  pub fn sign_self<'query, Q>(&mut self, private_key: &PrivateKey, method_query: Q) -> Result<()>
    where
      Q: Into<MethodQuery<'query>>,
  {
    // Ensure signing method has a capability invocation verification relationship.
    let method: &VerificationMethod<_> = self
      .document
      .as_document()
      .try_resolve_method_with_scope(method_query.into(), MethodScope::CapabilityInvocation)?;
    let _ = IotaDocument::check_signing_method(method)?;

    // Specify the full method DID Url if the verification method id does not match the document id.
    let method_did: &IotaDID = IotaDID::try_from_borrowed(method.id().did())?;
    let method_id: String = if method_did == self.document.id() {
      method.try_into_fragment()?
    } else {
      method.id().to_string()
    };

    // Sign document.
    match method.key_type() {
      MethodType::Ed25519VerificationKey2018 => {
        JcsEd25519::<Ed25519>::create_signature(self, method_id, private_key.as_ref())?;
      }
      MethodType::MerkleKeyCollection2021 => {
        // Merkle Key Collections cannot be used to sign documents.
        return Err(Error::InvalidDocumentSigningMethodType);
      }
    }

    Ok(())
  }

  /// Verifies that the signature on the `signed` identity was generated by a valid method from
  /// the `signer` DID document.
  ///
  /// # Errors
  ///
  /// Fails if:
  /// - The signature proof section is missing in the `signed` document.
  /// - The method is not found in the `signer` document.
  /// - An unsupported verification method is used.
  /// - The signature verification operation fails.
  pub fn verify_meta_document(signed: &IotaMetaDocument, signer: &IotaDocument) -> Result<()> {
    // Ensure signing key has a capability invocation verification relationship.
    let signature: &Signature = signed.try_signature()?;
    let method: &VerificationMethod<_> = signer
      .as_document()
      .try_resolve_method_with_scope(signature, MethodScope::CapabilityInvocation)?;

    // Verify signature.
    let public: PublicKey = method.key_data().try_decode()?.into();
    match method.key_type() {
      MethodType::Ed25519VerificationKey2018 => {
        JcsEd25519::<Ed25519>::verify_signature(signed, public.as_ref())?;
      }
      MethodType::MerkleKeyCollection2021 => {
        // Merkle Key Collections cannot be used to sign documents.
        return Err(identity_did::error::Error::InvalidMethodType.into());
      }
    }

    Ok(())
  }

  /// Verifies a self-signed signature on this DID document.
  ///
  /// Equivalent to `IotaMetaDocument::verify_meta_document(&this, &this.document)`.
  ///
  /// See [`IotaMetaDocument::verify_meta_document`].
  pub fn verify_self_signed(&self) -> Result<()> {
    Self::verify_meta_document(self, &self.document)
  }

  /// Verifies whether `identity` is a valid root DID document according to the IOTA DID method
  /// specification.
  ///
  /// It must be signed using a verification method with a public key whose BLAKE2b-256 hash matches
  /// the DID tag.
  pub fn verify_root(identity: &IotaMetaDocument) -> Result<()> {
    // The previous message id must be null.
    if !identity.metadata.previous_message_id.is_null() {
      return Err(Error::InvalidRootDocument);
    }

    // Validate the hash of the public key matches the DID tag.
    let document: &IotaDocument = &identity.document;
    let signature: &Signature = identity.try_signature()?;
    let method: &VerificationMethod<_> = document.as_document().try_resolve_method(signature)?;
    let public: PublicKey = method.key_data().try_decode()?.into();
    if document.id().tag() != IotaDID::encode_key(public.as_ref()) {
      return Err(Error::InvalidRootDocument);
    }

    // Validate the document is signed correctly.
    identity.verify_self_signed()
  }

  // ===========================================================================
  // Diffs
  // ===========================================================================

  /// Creates a `DocumentDiff` representing the changes between `self` and `other`.
  ///
  /// The returned `DocumentDiff` will have a digital signature created using the
  /// specified `private_key` and `method_query`.
  ///
  /// NOTE: the method must be a capability invocation method.
  ///
  /// # Errors
  ///
  /// Fails if the diff operation or signature operation fails.
  pub fn diff<'query, 's: 'query, Q>(
    &'query self,
    other: &Self,
    message_id: MessageId,
    private_key: &'query PrivateKey,
    method_query: Q,
  ) -> Result<DiffMessage>
    where
      Q: Into<MethodQuery<'query>>,
  {
    let mut diff: DiffMessage = DiffMessage::new(self, other, message_id)?;

    // Ensure the signing method has a capability invocation verification relationship.
    let method_query = method_query.into();
    let _ = self
      .as_document()
      .try_resolve_method_with_scope(method_query.clone(), MethodScope::CapabilityInvocation)?;

    self.sign_data(&mut diff, private_key, method_query)?;

    Ok(diff)
  }

  /// Verifies the signature of the `diff` was created using a capability invocation method
  /// in this DID Document.
  ///
  /// # Errors
  ///
  /// Fails if an unsupported verification method is used or the verification operation fails.
  pub fn verify_diff(&self, diff: &DiffMessage) -> Result<()> {
    self.verify_data_with_scope(diff, MethodScope::CapabilityInvocation)
  }

  /// Verifies a `DocumentDiff` signature and merges the changes into `self`.
  ///
  /// If merging fails `self` remains unmodified, otherwise `self` represents
  /// the merged document state.
  ///
  /// See [`IotaDocument::verify_diff`].
  ///
  /// # Errors
  ///
  /// Fails if the merge operation or signature operation fails.
  pub fn merge_diff(&mut self, diff: &DiffMessage) -> Result<()> {
    self.verify_diff(diff)?;

    todo!("META DOCUMENT DIFFS");
    // *self = diff.merge(self)?;

    Ok(())
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
