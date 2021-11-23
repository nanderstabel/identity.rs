// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::io::Read;

use crate::error::Error;
use crate::error::Result;

const BUFFER_SIZE: usize = 4096;
const QUALITY: u32 = 5;
// compression level
const WINDOWS_SIZE: u32 = 22;

pub(crate) fn compress_brotli<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>> {
  let mut buf = Vec::new();
  let mut compressor = brotli::CompressorReader::new(input.as_ref(), BUFFER_SIZE, QUALITY, WINDOWS_SIZE);
  compressor.read_to_end(&mut buf).map_err(|_| Error::CompressionError)?;
  Ok(buf)
}

pub(crate) fn decompress_brotli<T: AsRef<[u8]> + ?Sized>(input: &T) -> Result<Vec<u8>> {
  let mut decompressor = brotli::Decompressor::new(input.as_ref(), BUFFER_SIZE);
  let mut buf = Vec::new();
  decompressor
    .read_to_end(&mut buf)
    .map_err(|_| Error::CompressionError)?;
  Ok(buf)
}

#[cfg(test)]
mod test {
  use identity_core::convert::ToJson;
  use identity_core::crypto::KeyPair;

  use crate::document::IotaMetaDocument;

  use super::*;

  #[test]
  fn test_brotli() {
    let keypair: KeyPair = KeyPair::new_ed25519().unwrap();
    let mut metadoc = IotaMetaDocument::new(&keypair).unwrap();
    metadoc
      .sign_self(keypair.private(), &metadoc.document.default_signing_method().unwrap().id())
      .unwrap();

    let data = metadoc.to_json().unwrap();
    let compressed = compress_brotli(data.as_str()).unwrap();
    let decompressed = decompress_brotli(&compressed).unwrap();

    assert_eq!(decompressed, data.as_bytes());
  }
}
