// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use self::iota_did::IotaDID;
pub use self::iota_did::IotaDIDUrl;
pub use self::segments::Segments;

mod iota_did;
mod segments;

#[macro_use]
mod macros;
