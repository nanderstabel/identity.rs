// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::convert::TryFrom;
use core::convert::TryInto;
use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;

use serde::Deserialize;
use serde::Serialize;

use identity_core::common::Object;
use identity_core::common::Url;
use identity_core::crypto::Ed25519;
use identity_core::crypto::JcsEd25519;
use identity_core::crypto::KeyPair;
use identity_core::crypto::PrivateKey;
use identity_core::crypto::PublicKey;
use identity_core::crypto::SetSignature;
use identity_core::crypto::Signer;
use identity_core::crypto::TrySignature;
use identity_did::did::CoreDIDUrl;
use identity_did::document::CoreDocument;
use identity_did::service::Service;
use identity_did::utils::OrderedSet;
use identity_did::verifiable::DocumentSigner;
use identity_did::verifiable::DocumentVerifier;
use identity_did::verification::MethodQuery;
use identity_did::verification::MethodRef;
use identity_did::verification::MethodScope;
use identity_did::verification::MethodType;
use identity_did::verification::MethodUriType;
use identity_did::verification::TryMethod;
use identity_did::verification::VerificationMethod;

use crate::did::IotaDID;
use crate::did::IotaDIDUrl;
use crate::document::DiffMessage;
use crate::document::IotaVerificationMethod;
use crate::error::Error;
use crate::error::Result;
use crate::tangle::MessageId;
use crate::tangle::MessageIdExt;
use crate::tangle::NetworkName;

pub type IotaDocumentSigner<'a, 'b, 'c> = DocumentSigner<'a, 'b, 'c, Object, Object, Object>;
pub type IotaDocumentVerifier<'a> = DocumentVerifier<'a, Object, Object, Object>;

/// A DID Document adhering to the IOTA DID method specification.
///
/// This is a thin wrapper around [`CoreDocument`].
#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[repr(transparent)]
#[serde(into = "CoreDocument", try_from = "CoreDocument")]
pub struct IotaDocument(CoreDocument);

impl TryMethod for IotaDocument {
  const TYPE: MethodUriType = MethodUriType::Absolute;
}

impl IotaDocument {
  /// Creates a new DID Document from the given [`IotaVerificationMethod`], inserting it as the
  /// default capability invocation method.
  ///
  /// NOTE: the generated document is unsigned, see [`IotaDocument::sign_self`].
  pub fn from_verification_method(method: IotaVerificationMethod) -> Result<Self> {
    Self::check_signing_method(&method)?;
    CoreDocument::builder(Default::default())
      .id(method.id_core().did().clone())
      .capability_invocation(MethodRef::Embed(method.into()))
      .build()
      .map(TryInto::try_into)?
  }

  /// Converts a generic DID [`CoreDocument`] to an IOTA DID Document.
  ///
  /// # Errors
  ///
  /// Returns `Err` if the document is not a valid IOTA DID Document.
  pub fn try_from_core(document: CoreDocument) -> Result<Self> {
    IotaDocument::validate_core_document(&document)?;

    Ok(Self(document))
  }

  /// Performs validation that a [`CoreDocument`] adheres to the IOTA spec.
  ///
  /// # Errors
  ///
  /// Returns `Err` if the document is not a valid IOTA DID Document.
  fn validate_core_document<T, U, V>(document: &CoreDocument<T, U, V>) -> Result<()> {
    // Validate that the DID conforms to the IotaDID specification.
    // This check is required to ensure the correctness of the `IotaDocument::id()` method which
    // creates an `IotaDID::new_unchecked_ref()` from the underlying DID.
    let _ = IotaDID::try_from_borrowed(document.id())?;

    // Validate that the document controller (if any) conforms to the IotaDID specification.
    // This check is required to ensure the correctness of the `IotaDocument::controller()` method
    // which creates an `IotaDID::new_unchecked_ref()` from the underlying controller.
    document.controller().map_or(Ok(()), |c| IotaDID::check_validity(c))?;

    // Validate that the verification methods conform to the IotaDID specification.
    // This check is required to ensure the correctness of the
    // - `IotaDocument::methods()`,
    // - `IotaDocument::resolve_method()`,
    // - `IotaDocument::try_resolve_method()`,
    // - `IotaDocument::resolve_method_mut()`,
    // - `IotaDocument::try_resolve_method_mut()`,
    // methods which create an `IotaDID::new_unchecked_ref()` from the underlying controller.
    //
    // We check `document.verification_method()` and `document.verification_relationships()`
    // separately because they have separate types.
    for verification_method in document.verification_method().iter() {
      IotaVerificationMethod::check_validity(&*verification_method)?;
    }
    for method_ref in document.verification_relationships() {
      match method_ref {
        MethodRef::Embed(method) => IotaVerificationMethod::check_validity(method)?,
        MethodRef::Refer(did_url) => IotaDID::check_validity(did_url.did())?,
      }
    }

    Ok(())
  }

  /// Validates whether the verification method is a valid [`IotaVerificationMethod`] and that
  /// its key type is allowed to sign document updates.
  pub(crate) fn check_signing_method<T>(method: &VerificationMethod<T>) -> Result<()> {
    IotaVerificationMethod::check_validity(method)?;

    // Ensure the verification method type is supported
    match method.key_type() {
      MethodType::Ed25519VerificationKey2018 => {}
      MethodType::MerkleKeyCollection2021 => return Err(Error::InvalidDocumentSigningMethodType),
    }

    Ok(())
  }

  /// Returns a reference to the underlying [`CoreDocument`].
  pub fn as_document(&self) -> &CoreDocument {
    &self.0
  }

  /// Returns a mutable reference to the underlying [`CoreDocument`].
  ///
  /// # Safety
  ///
  /// This function is unsafe because it does not check that modifications
  /// made to the [`CoreDocument`] maintain a valid IOTA DID Document.
  ///
  /// If this constraint is violated, it may cause issues with future uses of
  /// the DID Document.
  pub unsafe fn as_document_mut(&mut self) -> &mut CoreDocument {
    &mut self.0
  }

  // ===========================================================================
  // Properties
  // ===========================================================================

  /// Returns the DID document [`id`](IotaDID).
  pub fn id(&self) -> &IotaDID {
    // SAFETY: We checked the validity of the DID Document ID in the
    // DID Document constructors; we don't provide mutable references so
    // the value cannot change with typical "safe" Rust.
    unsafe { IotaDID::new_unchecked_ref(self.0.id()) }
  }

  /// Returns a reference to the `IotaDocument` controller.
  pub fn controller(&self) -> Option<&IotaDID> {
    // SAFETY: Validity of controller checked in DID Document constructors.
    unsafe { self.0.controller().map(|did| IotaDID::new_unchecked_ref(did)) }
  }

  /// Returns a reference to the [`CoreDocument`] alsoKnownAs set.
  pub fn also_known_as(&self) -> &[Url] {
    self.0.also_known_as()
  }

  /// Returns the first [`IotaVerificationMethod`] with a capability invocation relationship
  /// capable of signing this DID document.
  pub fn default_signing_method(&self) -> Result<&IotaVerificationMethod> {
    self
      .as_document()
      .capability_invocation()
      .head()
      .map(|method_ref| self.as_document().resolve_method_ref(method_ref))
      .flatten()
      .map(|method: &VerificationMethod<_>|
        // SAFETY: validity of methods checked in `IotaVerificationMethod::check_validity`.
        unsafe { IotaVerificationMethod::new_unchecked_ref(method) })
      .ok_or(Error::MissingSigningKey)
  }

  /// Returns a reference to the custom DID Document properties.
  pub fn properties(&self) -> &Object {
    self.0.properties()
  }

  /// Returns a mutable reference to the custom DID Document properties.
  pub fn properties_mut(&mut self) -> &mut Object {
    self.0.properties_mut()
  }

  // ===========================================================================
  // Services
  // ===========================================================================

  /// Return a set of all [`Service`]s in the document.
  pub fn service(&self) -> &OrderedSet<Service> {
    self.0.service()
  }

  /// Add a new [`Service`] to the document.
  pub fn insert_service(&mut self, service: Service) -> bool {
    if service.id().fragment().is_none() {
      false
    } else {
      self.0.service_mut().append(service)
    }
  }

  /// Remove a [`Service`] identified by the given [`IotaDIDUrl`] from the document.
  pub fn remove_service(&mut self, did_url: IotaDIDUrl) -> Result<()> {
    let core_did_url: CoreDIDUrl = CoreDIDUrl::from(did_url);
    self.0.service_mut().remove(&core_did_url);
    Ok(())
  }

  // ===========================================================================
  // Verification Methods
  // ===========================================================================

  /// Returns an iterator over all [`IotaVerificationMethods`][IotaVerificationMethod] in the DID Document.
  pub fn methods(&self) -> impl Iterator<Item=&IotaVerificationMethod> {
    self.0.methods().map(|m|
      // SAFETY: Validity of verification methods checked in `IotaVerificationMethod::check_validity`.
      unsafe { IotaVerificationMethod::new_unchecked_ref(m) })
  }

  /// Adds a new [`IotaVerificationMethod`] to the DID Document.
  pub fn insert_method(&mut self, method: IotaVerificationMethod, scope: MethodScope) -> bool {
    self.0.insert_method(method.into(), scope)
  }

  /// Removes all occurrences of and references to the specified [`VerificationMethod`]
  /// from this document.
  pub fn remove_method(&mut self, did_url: IotaDIDUrl) -> Result<()> {
    let core_did_url: CoreDIDUrl = CoreDIDUrl::from(did_url);
    self.0.remove_method(&core_did_url);
    Ok(())
  }

  /// Returns the first [`IotaVerificationMethod`] with an `id` property
  /// matching the provided `query`.
  pub fn resolve_method<'query, Q>(&self, query: Q) -> Option<&IotaVerificationMethod>
    where
      Q: Into<MethodQuery<'query>>,
  {
    // SAFETY: Validity of verification methods checked in `IotaVerificationMethod::check_validity`.
    unsafe {
      self
        .0
        .resolve_method(query)
        .map(|method| IotaVerificationMethod::new_unchecked_ref(method))
    }
  }

  /// Returns the first [`IotaVerificationMethod`] with an `id` property
  /// matching the provided `query`.
  ///
  /// # Errors
  ///
  /// Fails if no matching verification [`IotaVerificationMethod`] is found.
  pub fn try_resolve_method<'query, Q>(&self, query: Q) -> Result<&IotaVerificationMethod>
    where
      Q: Into<MethodQuery<'query>>,
  {
    // SAFETY: Validity of verification methods checked in `IotaVerificationMethod::check_validity`.
    unsafe {
      self
        .0
        .try_resolve_method(query)
        .map(|method| IotaVerificationMethod::new_unchecked_ref(method))
        .map_err(Error::InvalidDoc)
    }
  }

  #[doc(hidden)]
  pub fn try_resolve_method_mut<'query, Q>(&mut self, query: Q) -> Result<&mut VerificationMethod>
    where
      Q: Into<MethodQuery<'query>>,
  {
    self.0.try_resolve_method_mut(query).map_err(Into::into)
  }

  // ===========================================================================
  // Signatures
  // ===========================================================================

  /// Creates a new [`IotaDocumentSigner`] that can be used to create digital
  /// signatures from verification methods in this DID Document.
  pub fn signer<'base>(&'base self, private_key: &'base PrivateKey) -> IotaDocumentSigner<'base, 'base, 'base> {
    self.0.signer(private_key)
  }

  /// Creates a new [`IotaDocumentVerifier`] that can be used to verify signatures
  /// created with this DID Document.
  pub fn verifier(&self) -> IotaDocumentVerifier<'_> {
    self.0.verifier()
  }

  /// Signs the provided `data` with the verification method specified by `method_query`.
  ///
  /// NOTE: does not validate whether `private_key` corresponds to the verification method.
  /// See [`IotaDocument::verify_data`].
  ///
  /// # Errors
  ///
  /// Fails if an unsupported verification method is used, data
  /// serialization fails, or the signature operation fails.
  pub fn sign_data<'query, 's: 'query, X, Q>(
    &'s self,
    data: &mut X,
    private_key: &'query PrivateKey,
    method_query: Q,
  ) -> Result<()>
    where
      X: Serialize + SetSignature + TryMethod,
      Q: Into<MethodQuery<'query>>,
  {
    self
      .signer(private_key)
      .method(method_query)
      .sign(data)
      .map_err(Into::into)
  }

  /// Verifies the signature of the provided `data` was created using a verification method
  /// in this DID Document.
  ///
  /// NOTE: does not restrict which verification relationship signed the data.
  /// See [`IotaDocument::verify_data_with_scope`].
  ///
  /// # Errors
  ///
  /// Fails if an unsupported verification method is used, document
  /// serialization fails, or the verification operation fails.
  pub fn verify_data<X>(&self, data: &X) -> Result<()>
    where
      X: Serialize + TrySignature,
  {
    self.verifier().verify(data).map_err(Into::into)
  }

  /// Verifies the signature of the provided `data` was created using a verification method
  /// in this DID Document with the verification relationship specified by `scope`.
  ///
  /// # Errors
  ///
  /// Fails if an unsupported verification method is used or the verification operation fails.
  pub fn verify_data_with_scope<X>(&self, data: &X, scope: MethodScope) -> Result<()>
    where
      X: Serialize + TrySignature,
  {
    self.verifier().verify_with_scope(data, scope).map_err(Into::into)
  }

  // ===========================================================================
  // Publishing
  // ===========================================================================

  /// Returns the Tangle index of the integration chain for this DID.
  ///
  /// This is equivalent to the tag segment of the [`IotaDID`].
  ///
  /// E.g.
  /// For an [`IotaDocument`] `doc` with `"did:iota:1234567890abcdefghijklmnopqrstuvxyzABCDEFGHI"`,
  /// `doc.integration_index() == "1234567890abcdefghijklmnopqrstuvxyzABCDEFGHI"`
  pub fn integration_index(&self) -> &str {
    self.id().tag()
  }

  /// Returns the Tangle index of the DID diff chain. This should only be called on messages
  /// from documents published on the integration chain.
  ///
  /// This is the Base58-btc encoded SHA-256 digest of the hex-encoded message id.
  pub fn diff_index(message_id: &MessageId) -> Result<String> {
    if message_id.is_null() {
      return Err(Error::InvalidDocumentMessageId);
    }

    Ok(IotaDID::encode_key(message_id.encode_hex().as_bytes()))
  }
}

impl Display for IotaDocument {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    Display::fmt(&self.0, f)
  }
}

impl Debug for IotaDocument {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    Debug::fmt(&self.0, f)
  }
}

impl From<IotaDocument> for CoreDocument {
  fn from(other: IotaDocument) -> Self {
    other.0
  }
}

impl TryFrom<CoreDocument> for IotaDocument {
  type Error = Error;

  fn try_from(other: CoreDocument) -> Result<Self, Self::Error> {
    Self::try_from_core(other)
  }
}

#[cfg(test)]
mod tests {
  use std::collections::btree_map::BTreeMap;
  use std::str::FromStr;

  use iota_client::bee_message::MESSAGE_ID_LENGTH;

  use identity_core::common::Value;
  use identity_core::convert::FromJson;
  use identity_core::convert::SerdeInto;
  use identity_core::crypto::KeyCollection;
  use identity_core::crypto::KeyPair;
  use identity_core::crypto::KeyType;
  use identity_core::crypto::merkle_key::Sha256;
  use identity_core::crypto::PrivateKey;
  use identity_core::crypto::PublicKey;
  use identity_core::utils::encode_b58;
  use identity_did::did::CoreDID;
  use identity_did::did::CoreDIDUrl;
  use identity_did::did::DID;
  use identity_did::document::CoreDocument;
  use identity_did::service::Service;
  use identity_did::verification::MethodData;
  use identity_did::verification::MethodRef;
  use identity_did::verification::MethodScope;
  use identity_did::verification::MethodType;
  use identity_did::verification::VerificationMethod;

  use crate::did::IotaDID;
  use crate::did::IotaDIDUrl;
  use crate::document::IotaVerificationMethod;
  use crate::Error;
  use crate::tangle::MessageId;
  use crate::tangle::Network;
  use crate::tangle::TangleRef;

  use super::*;

  const DID_ID: &str = "did:iota:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M";
  const DID_METHOD_ID: &str = "did:iota:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M#sign-0";
  const DID_DEVNET_ID: &str = "did:iota:dev:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M";
  const DID_DEVNET_METHOD_ID: &str = "did:iota:dev:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M#sign-0";

  fn valid_did() -> CoreDID {
    DID_ID.parse().unwrap()
  }

  fn valid_properties() -> Object {
    BTreeMap::default()
  }

  fn core_verification_method(controller: &CoreDID, fragment: &str) -> VerificationMethod {
    VerificationMethod::builder(Default::default())
      .id(controller.to_url().join(fragment).unwrap())
      .controller(controller.clone())
      .key_type(MethodType::Ed25519VerificationKey2018)
      .key_data(MethodData::new_multibase(fragment.as_bytes()))
      .build()
      .unwrap()
  }

  fn iota_verification_method(controller: &CoreDID, fragment: &str) -> IotaVerificationMethod {
    let core_method = core_verification_method(controller, fragment);
    IotaVerificationMethod::try_from_core(core_method).unwrap()
  }

  fn iota_document_from_core(controller: &CoreDID) -> IotaDocument {
    let mut properties: BTreeMap<String, Value> = BTreeMap::default();
    properties.insert("created".to_string(), "2020-01-01T00:00:00Z".into());
    properties.insert("updated".to_string(), "2020-01-02T00:00:00Z".into());

    IotaDocument::try_from_core(
      CoreDocument::builder(properties)
        .id(controller.clone())
        .verification_method(core_verification_method(controller, "#key-1"))
        .verification_method(core_verification_method(controller, "#key-2"))
        .verification_method(core_verification_method(controller, "#key-3"))
        .authentication(core_verification_method(controller, "#auth-key"))
        .authentication(controller.to_url().join("#key-3").unwrap())
        .key_agreement(controller.to_url().join("#key-4").unwrap())
        .controller(controller.clone())
        .build()
        .unwrap(),
    )
      .unwrap()
  }

  fn generate_testkey() -> KeyPair {
    let private_key: Vec<u8> = vec![
      40, 185, 109, 70, 134, 119, 123, 37, 190, 254, 232, 186, 106, 48, 213, 63, 133, 223, 167, 126, 159, 43, 178, 4,
      190, 217, 52, 66, 92, 63, 69, 84,
    ];
    let public_key: Vec<u8> = vec![
      212, 151, 158, 35, 16, 178, 19, 27, 83, 109, 212, 138, 141, 134, 122, 246, 156, 148, 227, 69, 68, 251, 190, 31,
      25, 101, 230, 20, 130, 188, 121, 196,
    ];
    KeyPair::from((
      KeyType::Ed25519,
      PublicKey::from(public_key),
      PrivateKey::from(private_key),
    ))
  }

  fn compare_document(document: &IotaDocument) {
    assert_eq!(document.id().to_string(), DID_ID);
    let default_signing_method: &IotaVerificationMethod = document.default_signing_method().unwrap();

    assert_eq!(default_signing_method.id().to_string(), DID_METHOD_ID);
    assert_eq!(
      document.default_signing_method().unwrap().key_type(),
      MethodType::Ed25519VerificationKey2018
    );
    assert_eq!(
      document.default_signing_method().unwrap().key_data(),
      &MethodData::PublicKeyMultibase("zFJsXMk9UqpJf3ZTKnfEQAhvBrVLKMSx9ZeYwQME6c6tT".to_owned())
    );
  }

  fn compare_document_devnet(document: &IotaDocument) {
    assert_eq!(document.id().to_string(), DID_DEVNET_ID);
    assert_eq!(document.id().network_str(), Network::Devnet.name_str());
    assert_eq!(
      document.default_signing_method().unwrap().id().to_string(),
      DID_DEVNET_METHOD_ID
    );
    assert_eq!(
      document.default_signing_method().unwrap().key_type(),
      MethodType::Ed25519VerificationKey2018
    );
    assert_eq!(
      document.default_signing_method().unwrap().key_data(),
      &MethodData::PublicKeyMultibase("zFJsXMk9UqpJf3ZTKnfEQAhvBrVLKMSx9ZeYwQME6c6tT".to_owned())
    );
  }

  #[test]
  fn test_invalid_try_from_core_invalid_id() {
    let invalid_did: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        // INVALID
        .id(invalid_did)
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_no_created_field() {
    let mut properties: BTreeMap<String, Value> = BTreeMap::default();
    properties.insert("updated".to_string(), "2020-01-02T00:00:00Z".into());
    // INVALID - missing "created" field.

    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(properties)
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_no_updated_field() {
    let mut properties: BTreeMap<String, Value> = BTreeMap::default();
    properties.insert("created".to_string(), "2020-01-02T00:00:00Z".into());
    // INVALID - missing "updated" field.

    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(properties)
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_invalid_controller() {
    let invalid_controller: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        .id(valid_did())
        // INVALID - does not match document ID
        .authentication(core_verification_method(&invalid_controller, "#auth-key"))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_invalid_authentication_method_ref() {
    let invalid_ref: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        // INVALID - does not reference a verification method in the document
        .authentication(MethodRef::Refer(invalid_ref.into_url()))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_invalid_assertion_method_ref() {
    let invalid_ref: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        // INVALID - does not reference a verification method in the document
        .assertion_method(MethodRef::Refer(invalid_ref.into_url()))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_invalid_key_agreement_ref() {
    let invalid_ref: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        // INVALID - does not reference a verification method in the document
        .key_agreement(MethodRef::Refer(invalid_ref.into_url()))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_invalid_capability_delegation_ref() {
    let invalid_ref: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        // INVALID - does not reference a verification method in the document
        .capability_delegation(MethodRef::Refer(invalid_ref.into_url()))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_invalid_try_from_core_invalid_capability_invocation_ref() {
    let invalid_ref: CoreDID = "did:invalid:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M"
      .parse()
      .unwrap();
    let doc = IotaDocument::try_from_core(
      CoreDocument::builder(valid_properties())
        .id(valid_did())
        .authentication(core_verification_method(&valid_did(), "#auth-key"))
        // INVALID - does not reference a verification method in the document
        .capability_invocation(MethodRef::Refer(invalid_ref.into_url()))
        .build()
        .unwrap(),
    );

    assert!(doc.is_err());
  }

  #[test]
  fn test_new() {
    //from keypair
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    compare_document(&document);

    //from authentication
    let method = document.default_signing_method().unwrap().to_owned();
    let document: IotaDocument = IotaDocument::from_verification_method(method).unwrap();
    compare_document(&document);

    //from core
    let document: IotaDocument = IotaDocument::try_from_core(document.serde_into().unwrap()).unwrap();
    compare_document(&document);
  }

  #[test]
  fn test_new_with_options_network() {
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new_with_options(&keypair, Some(Network::Devnet.name()), None).unwrap();
    compare_document_devnet(&document);
  }

  #[test]
  fn test_new_with_options_fragment() {
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new_with_options(&keypair, None, Some("test-key")).unwrap();
    assert_eq!(
      document.default_signing_method().unwrap().try_into_fragment().unwrap(),
      "#test-key"
    );
  }

  #[test]
  fn test_new_with_options_empty_fragment() {
    let keypair: KeyPair = generate_testkey();
    let result: Result<IotaDocument, Error> = IotaDocument::new_with_options(&keypair, None, Some(""));
    assert!(matches!(result, Err(Error::InvalidMethodMissingFragment)));
  }

  #[test]
  fn test_no_controller() {
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    assert_eq!(document.controller(), None);
  }

  #[test]
  fn test_controller_from_core() {
    let controller: CoreDID = valid_did();
    let document: IotaDocument = iota_document_from_core(&controller);
    let expected_controller: Option<IotaDID> = Some(IotaDID::try_from_owned(controller).unwrap());
    assert_eq!(document.controller(), expected_controller.as_ref());
  }

  #[test]
  fn test_methods_new() {
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new(&keypair).unwrap();

    // An IotaDocument created from a keypair has a single verification method, namely an
    // Ed25519 signature.
    let expected = IotaVerificationMethod::try_from_core(
      VerificationMethod::builder(Default::default())
        .id(
          "did:iota:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1M#sign-0"
            .parse()
            .unwrap(),
        )
        .controller(valid_did())
        .key_type(MethodType::Ed25519VerificationKey2018)
        .key_data(MethodData::PublicKeyMultibase(
          "zFJsXMk9UqpJf3ZTKnfEQAhvBrVLKMSx9ZeYwQME6c6tT".into(),
        ))
        .build()
        .unwrap(),
    )
      .unwrap();

    let mut methods = document.methods();

    assert_eq!(methods.next(), Some(expected).as_ref());
    assert_eq!(methods.next(), None);
  }

  #[test]
  fn test_methods_from_core() {
    let controller: CoreDID = valid_did();
    let document: IotaDocument = iota_document_from_core(&controller);
    let expected: Vec<IotaVerificationMethod> = vec![
      iota_verification_method(&controller, "#key-1"),
      iota_verification_method(&controller, "#key-2"),
      iota_verification_method(&controller, "#key-3"),
      iota_verification_method(&controller, "#auth-key"),
    ];

    let mut methods = document.methods();
    assert_eq!(methods.next(), Some(&expected[0]));
    assert_eq!(methods.next(), Some(&expected[1]));
    assert_eq!(methods.next(), Some(&expected[2]));
    assert_eq!(methods.next(), Some(&expected[3]));
    assert_eq!(methods.next(), None);
  }

  #[test]
  fn test_sign_self() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    assert!(document.verify_self_signed().is_err());

    // Sign with the default capability invocation method.
    document
      .sign_self(keypair.private(), &document.default_signing_method().unwrap().id())
      .unwrap();
    assert!(document.verify_self_signed().is_ok());
  }

  #[test]
  fn test_sign_self_new_method() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    assert!(document.verify_self_signed().is_err());

    // Add a new capability invocation method directly
    let new_keypair: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
    let new_method: IotaVerificationMethod = IotaVerificationMethod::from_keypair(&new_keypair, "new_signer").unwrap();
    document.insert_method(new_method, MethodScope::CapabilityInvocation);

    // INVALID - try sign using the wrong private key
    document.sign_self(keypair.private(), "#new_signer").unwrap();
    assert!(document.verify_self_signed().is_err());

    // VALID - Sign with the new capability invocation method private key
    document.sign_self(new_keypair.private(), "#new_signer").unwrap();
    assert!(document.verify_self_signed().is_ok());
  }

  #[test]
  fn test_sign_self_fails() {
    fn generate_document() -> (IotaDocument, KeyPair) {
      let keypair: KeyPair = generate_testkey();
      let document: IotaDocument = IotaDocument::new(&keypair).unwrap();
      (document, keypair)
    }

    // INVALID - try sign referencing a non-existent verification method.
    {
      let (mut document, keypair) = generate_document();
      assert!(document.verify_self_signed().is_err());
      assert!(document.sign_self(keypair.private(), "#doesnotexist").is_err());
      assert!(document.verify_self_signed().is_err());
    }

    // INVALID - try sign using a random private key.
    {
      let (mut document, _) = generate_document();
      let random_keypair: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
      document
        .sign_self(
          random_keypair.private(),
          &document.default_signing_method().unwrap().id(),
        )
        .unwrap();
      assert!(document.verify_self_signed().is_err());
    }

    // INVALID - try sign using any verification relationship other than capability invocation.
    for method_scope in [
      MethodScope::VerificationMethod,
      MethodScope::AssertionMethod,
      MethodScope::CapabilityDelegation,
      MethodScope::Authentication,
      MethodScope::KeyAgreement,
    ] {
      let (mut document, _) = generate_document();
      // Add a new method unable to sign the document.
      let keypair_new: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
      let method_new: IotaVerificationMethod =
        IotaVerificationMethod::from_keypair(&keypair_new, "new_signer").unwrap();
      document.insert_method(method_new, method_scope);
      // Try sign the document using the new key.
      assert!(document.sign_self(keypair_new.private(), "#new_signer").is_err());
      assert!(document.verify_self_signed().is_err());
      assert!(IotaDocument::verify_root_document(&document).is_err());
    }

    // INVALID - try sign using a Merkle Key Collection
    {
      let (mut document, _) = generate_document();
      let key_collection: KeyCollection = KeyCollection::new_ed25519(8).unwrap();
      let merkle_key_method =
        IotaVerificationMethod::create_merkle_key::<Sha256>(document.id().clone(), &key_collection, "merkle-key")
          .unwrap();
      document.insert_method(merkle_key_method, MethodScope::CapabilityInvocation);
      assert!(document
        .sign_self(key_collection.private(0).unwrap(), "merkle-key")
        .is_err());
      assert!(document.verify_self_signed().is_err());
    }
  }

  #[test]
  fn test_diff() {
    // Ensure only capability invocation methods are allowed to sign a diff.
    for scope in [
      MethodScope::AssertionMethod,
      MethodScope::Authentication,
      MethodScope::CapabilityDelegation,
      MethodScope::CapabilityInvocation,
      MethodScope::KeyAgreement,
      MethodScope::VerificationMethod,
    ] {
      let key1: KeyPair = generate_testkey();
      let mut doc1: IotaDocument = IotaDocument::new(&key1).unwrap();
      // Add a new verification relationship.
      let key2: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
      let method_fragment = format!("{}-1", scope.as_str().to_ascii_lowercase());
      let method_new: IotaVerificationMethod =
        IotaVerificationMethod::from_keypair(&key2, method_fragment.as_str()).unwrap();
      assert!(doc1.insert_method(method_new, scope));
      assert!(doc1
        .as_document()
        .try_resolve_method_with_scope(method_fragment.as_str(), scope)
        .is_ok());
      doc1.set_message_id(MessageId::new([3_u8; 32]));

      // Add a service to an updated document.
      let mut doc2: IotaDocument = doc1.clone();
      let service: Service = Service::from_json(
        r#"{
        "id":"did:iota:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1N#linked-domain",
        "type": "LinkedDomains",
        "serviceEndpoint": "https://bar.example.com"
      }"#,
      )
        .unwrap();
      doc2.insert_service(service);

      // Try generate and sign a diff using the specified method.
      let diff_result = doc1.diff(&doc2, *doc1.message_id(), key2.private(), method_fragment.as_str());
      if scope == MethodScope::CapabilityInvocation {
        let diff = diff_result.unwrap();
        assert!(doc1.verify_data(&diff).is_ok());
        assert!(doc1.verify_diff(&diff).is_ok());
      } else {
        assert!(diff_result.is_err());
      }
    }
  }

  #[test]
  fn test_verify_data_with_scope() {
    fn generate_data() -> Object {
      use identity_core::json;
      let mut properties: Object = Object::default();
      properties.properties.insert("int_key".to_owned(), json!(1));
      properties.properties.insert("str".to_owned(), json!("some value"));
      properties
        .properties
        .insert("object".to_owned(), json!({ "inner": 42 }));
      properties
    }

    let key: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&key).unwrap();

    // Try sign using each type of verification relationship.
    for scope in [
      MethodScope::AssertionMethod,
      MethodScope::Authentication,
      MethodScope::CapabilityDelegation,
      MethodScope::CapabilityInvocation,
      MethodScope::KeyAgreement,
      MethodScope::VerificationMethod,
    ] {
      // Add a new method.
      let key_new: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
      let method_fragment = format!("{}-1", scope.as_str().to_ascii_lowercase());
      let method_new: IotaVerificationMethod =
        IotaVerificationMethod::from_keypair(&key_new, method_fragment.as_str()).unwrap();
      document.insert_method(method_new, scope);

      // Sign and verify data.
      let mut data = generate_data();
      document
        .sign_data(&mut data, key_new.private(), method_fragment.as_str())
        .unwrap();
      // Signature should still be valid for every scope.
      assert!(document.verify_data(&data).is_ok());

      // Ensure only the correct scope is valid.
      for scope_check in [
        MethodScope::AssertionMethod,
        MethodScope::Authentication,
        MethodScope::CapabilityDelegation,
        MethodScope::CapabilityInvocation,
        MethodScope::KeyAgreement,
        MethodScope::VerificationMethod,
      ] {
        let result = document.verify_data_with_scope(&data, scope_check);
        // Any other scope should fail validation.
        if scope_check == scope {
          assert!(result.is_ok());
        } else {
          assert!(result.is_err());
        }
      }
    }
  }

  #[test]
  fn test_root_document() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    assert!(IotaDocument::verify_root_document(&document).is_err());

    // VALID - root document signed using the default method.
    document
      .sign_self(keypair.private(), &document.default_signing_method().unwrap().id())
      .unwrap();
    assert!(document.verify_self_signed().is_ok());
    assert!(IotaDocument::verify_root_document(&document).is_ok());
  }

  #[test]
  fn test_root_document_invalid() {
    fn generate_root_document() -> (IotaDocument, KeyPair) {
      let keypair: KeyPair = generate_testkey();
      (IotaDocument::new(&keypair).unwrap(), keypair)
    }

    // INVALID - root document not signed.
    {
      let (document, _) = generate_root_document();
      assert!(IotaDocument::verify_root_document(&document).is_err());
    }

    // INVALID - root document previousMessageId not null.
    {
      let (mut document, keypair) = generate_root_document();
      document.set_previous_message_id(MessageId::new([3u8; MESSAGE_ID_LENGTH]));
      document
        .sign_self(keypair.private(), &document.default_signing_method().unwrap().id())
        .unwrap();
      assert!(document.verify_self_signed().is_ok());
      assert!(IotaDocument::verify_root_document(&document).is_err());
    }

    // INVALID - root document signed with a key not matching the DID tag.
    {
      let (document, keypair) = generate_root_document();
      // Replace the base58 encoded public key with that of a different key.
      let new_keypair: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
      let b58_old = encode_b58(keypair.public());
      let b58_new = encode_b58(new_keypair.public());
      let doc_json_modified = document.to_string().replace(&b58_old, &b58_new);
      // Sign the document using the new key.
      let mut new_document: IotaDocument = IotaDocument::from_json(&doc_json_modified).unwrap();
      new_document
        .sign_self(
          new_keypair.private(),
          &new_document.default_signing_method().unwrap().id(),
        )
        .unwrap();
      assert!(new_document.verify_self_signed().is_ok());
      assert!(IotaDocument::verify_root_document(&new_document).is_err());
    }

    // INVALID - root document signed using a different method that does not match the DID tag.
    {
      let (mut document, _) = generate_root_document();
      // Add a new method able to sign the document.
      let keypair_new: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
      let method_new: IotaVerificationMethod =
        IotaVerificationMethod::from_keypair(&keypair_new, "new_signer").unwrap();
      document.insert_method(method_new, MethodScope::CapabilityInvocation);
      // Sign the document using the new key.
      document.sign_self(keypair_new.private(), "#new_signer").unwrap();
      assert!(document.verify_self_signed().is_ok());
      assert!(IotaDocument::verify_root_document(&document).is_err());
    }
  }

  #[test]
  fn test_json() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();

    let json_doc: String = document.to_string();
    let document2: IotaDocument = IotaDocument::from_json(&json_doc).unwrap();
    assert_eq!(document, document2);

    assert!(document
      .sign_self(keypair.private(), &document.default_signing_method().unwrap().id())
      .is_ok());

    let json_doc: String = document.to_string();
    let document2: IotaDocument = IotaDocument::from_json(&json_doc).unwrap();
    assert_eq!(document, document2);
  }

  #[test]
  fn test_default_signing_method() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();

    let signing_method: IotaVerificationMethod = document.default_signing_method().unwrap().clone();
    assert!(IotaDocument::check_signing_method(&signing_method).is_ok());

    // Ensure signing method has a capability invocation relationship.
    let capability_invocation: IotaVerificationMethod = IotaVerificationMethod::try_from_core(
      document
        .as_document()
        .try_resolve_method_with_scope(signing_method.id(), MethodScope::CapabilityInvocation)
        .unwrap()
        .clone(),
    )
      .unwrap();
    assert_eq!(&signing_method, &capability_invocation);

    // Adding a new capability invocation method still returns the original method.
    let new_keypair: KeyPair = KeyPair::new(KeyType::Ed25519).unwrap();
    let new_method: IotaVerificationMethod = IotaVerificationMethod::from_keypair(&new_keypair, "new_signer").unwrap();
    let new_method_id: IotaDIDUrl = new_method.id();
    document.insert_method(new_method, MethodScope::CapabilityInvocation);
    assert_eq!(document.default_signing_method().unwrap().id(), signing_method.id());

    // Removing the original signing method returns the next one.
    document
      .remove_method(
        document
          .id()
          .to_url()
          .join(format!("#{}", IotaDocument::DEFAULT_METHOD_FRAGMENT))
          .unwrap(),
      )
      .unwrap();
    assert_eq!(document.default_signing_method().unwrap().id(), new_method_id);

    // Removing the last signing method causes an error.
    document.remove_method(new_method_id).unwrap();
    assert!(matches!(
      document.default_signing_method(),
      Err(Error::MissingSigningKey)
    ));
  }

  #[test]
  fn test_document_services() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    let service: Service = Service::from_json(
      r#"{
      "id":"did:iota:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1N#linked-domain",
      "type": "LinkedDomains",
      "serviceEndpoint": "https://bar.example.com"
    }"#,
    )
      .unwrap();
    document.insert_service(service);

    assert_eq!(1, document.service().len());

    document
      .remove_service(IotaDIDUrl::parse("did:iota:HGE4tecHWL2YiZv5qAGtH7gaeQcaz2Z1CR15GWmMjY1N#linked-domain").unwrap())
      .ok();
    assert_eq!(0, document.service().len());
  }

  #[test]
  fn test_relative_method_uri() {
    let keypair: KeyPair = generate_testkey();
    let mut document: IotaDocument = IotaDocument::new(&keypair).unwrap();

    assert!(document.proof().is_none());
    assert!(document
      .sign_self(keypair.private(), &document.default_signing_method().unwrap().id())
      .is_ok());

    assert_eq!(document.proof().unwrap().verification_method(), "#sign-0");
  }

  #[test]
  fn test_integration_index() {
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new(&keypair).unwrap();

    // The integration chain index should just be the tag of the DID
    let tag = document.id().tag();
    assert_eq!(document.integration_index(), tag);
  }

  #[test]
  fn test_diff_index() {
    let message_id = MessageId::from_str("c38d6c541f98f780ddca6ad648ff0e073cd86c4dee248149c2de789d84d42132").unwrap();
    let diff_index = IotaDocument::diff_index(&message_id).expect("failed to generate diff_index");
    assert_eq!(diff_index, "2g45GsCAmkvQfcrHGUgqwQJLbYY3Gic8f23wf71sGGGP");
  }

  #[test]
  fn test_new_document_verification_relationships() {
    let keypair: KeyPair = generate_testkey();
    let document: IotaDocument = IotaDocument::new(&keypair).unwrap();
    let verification_method: &IotaVerificationMethod = document.resolve_method("#sign-0").unwrap();
    let expected_did_url: IotaDIDUrl = document.id().to_url().join("#sign-0").unwrap();

    // Ensure capability invocation relationship.
    let capability_invocation_method_id: &CoreDIDUrl =
      document.as_document().capability_invocation().first().unwrap().id();
    assert_eq!(verification_method.id(), expected_did_url);
    assert_eq!(
      capability_invocation_method_id.to_string(),
      expected_did_url.to_string()
    );

    // Ensure fragment of the capability invocation method reference is `authentication`
    match document.as_document().capability_invocation().first().unwrap().clone() {
      MethodRef::Refer(_) => panic!("capability invocation method should be embedded"),
      MethodRef::Embed(method) => assert_eq!(method.id(), capability_invocation_method_id),
    }

    // `methods` returns all embedded verification methods, so only one is expected.
    assert_eq!(document.methods().count(), 1);
  }

  #[test]
  fn test_document_equality() {
    let keypair1: KeyPair = KeyPair::new_ed25519().unwrap();
    let method1: IotaVerificationMethod = IotaVerificationMethod::from_keypair(&keypair1, "test-0").unwrap();

    let original_doc = IotaDocument::from_verification_method(method1).unwrap();

    let mut doc1 = original_doc.clone();

    // Update the key material of the existing verification method test-0.
    let keypair2: KeyPair = KeyPair::new_ed25519().unwrap();
    let method2: IotaVerificationMethod =
      IotaVerificationMethod::from_did(doc1.id().to_owned(), &keypair2, "test-0").unwrap();

    doc1.remove_method(doc1.id().to_url().join("#test-0").unwrap()).unwrap();
    doc1.insert_method(method2, MethodScope::CapabilityInvocation);

    // Even though the method fragment is the same, the key material has been updated
    // so the two documents are expected to not be equal.
    assert_ne!(original_doc, doc1);

    let mut doc2 = doc1.clone();
    let keypair3: KeyPair = KeyPair::new_ed25519().unwrap();
    let method3: IotaVerificationMethod =
      IotaVerificationMethod::from_did(doc1.id().to_owned(), &keypair3, "test-0").unwrap();

    let was_inserted = doc2.insert_method(method3, MethodScope::CapabilityInvocation);

    // Nothing was inserted, because a method with the same fragment already existed.
    assert!(!was_inserted);
    assert_eq!(doc1, doc2);
  }
}
