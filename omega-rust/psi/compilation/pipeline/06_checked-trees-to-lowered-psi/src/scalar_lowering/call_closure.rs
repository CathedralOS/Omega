//! The Unit body a scalar call lowers, and the scalar helper catalogs embedded
//! beneath externally assembled control.
//!
//! `callee` selects the one Unit body that owns a scalar callee's completion.
//! Selected scalar roots use the same closure assembler as operation bodies in
//! `unit::attached_unit`, which owns their identities and qualification
//! namespace; `embedded` discovers the helper closure beneath a source-owned
//! root and carries the identities its composed operands resolve against.

use super::{
    CheckedTerminalSignatureEligibility, CheckedTrees, LoweringError, PrimitiveType, unsupported,
};

pub(crate) mod callee;
pub(crate) mod embedded;
