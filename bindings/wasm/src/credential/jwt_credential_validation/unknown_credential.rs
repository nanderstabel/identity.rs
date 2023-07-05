// Copyright 2020-2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use identity_iota::core::Object;
use identity_iota::credential::Credential;
use identity_iota::credential::Jwt;
use wasm_bindgen::prelude::*;

use crate::credential::WasmCredential;
use crate::credential::WasmJwt;

#[wasm_bindgen(js_name = UnknownCredential)]
pub struct WasmUnknownCredentialContainer(UnknownCredential);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum UnknownCredential {
  Jwt(Jwt),
  Credential(Credential),
  Other(Object),
}

#[wasm_bindgen(js_class = UnknownCredential)]
impl WasmUnknownCredentialContainer {
  pub(crate) fn new(inner: UnknownCredential) -> Self {
    WasmUnknownCredentialContainer(inner)
  }

  /// Returns a `Jwt` if the credential is of type string, `undefined` otherwise.
  #[wasm_bindgen(js_name = tryIntoJwt)]
  pub fn try_into_jwt(&self) -> Option<WasmJwt> {
    match &self.0 {
      UnknownCredential::Jwt(jwt) => Some(WasmJwt::new(jwt.clone())),
      _ => None,
    }
  }

  /// Returns a `Credential` if the credential is of said type, `undefined` otherwise.
  #[wasm_bindgen(js_name = tryIntoCredential)]
  pub fn try_into_credential(&self) -> Option<WasmCredential> {
    match &self.0 {
      UnknownCredential::Credential(credential) => Some(WasmCredential::from(credential.clone())),
      _ => None,
    }
  }

  /// Returns the contained value as JSON, if it can be converted, `undefined` otherwise.
  #[wasm_bindgen(js_name = tryIntoRaw)]
  pub fn try_into_raw(&self) -> JsValue {
    let js_value: Option<JsValue> = match &self.0 {
      UnknownCredential::Jwt(jwt) => JsValue::from_serde(jwt).ok(),
      UnknownCredential::Credential(credential) => JsValue::from_serde(credential).ok(),
      UnknownCredential::Other(object) => JsValue::from_serde(object).ok(),
    };

    js_value.unwrap_or_else(JsValue::undefined)
  }
}

impl_wasm_json!(WasmUnknownCredentialContainer, UnknownCredential);
impl_wasm_clone!(WasmUnknownCredentialContainer, UnknownCredential);
