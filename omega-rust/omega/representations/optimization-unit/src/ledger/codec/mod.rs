//! Optimizer module role: stage group. Versioned transformation-ledger persistence group.
//!
//! Encoding, decoding, and cursor mechanics are separate leaves. Decoding
//! rejoins canonical ledger construction before returning an admitted value.

use super::*;

mod cursor;
mod decoding;
mod encoding;

pub(super) const LEDGER_MAGIC: &[u8] = b"omega.psi-transformation-ledger.v4\0";

pub(super) use cursor::LedgerCursor;
pub(super) use encoding::encode_ledger;
