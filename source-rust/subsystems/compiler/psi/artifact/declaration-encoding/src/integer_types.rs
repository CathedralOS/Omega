use crate::CodecError;
use semantic_vocabulary::{ClosedIntegerType, IntegerType};
use std::collections::BTreeMap;

/// Integer descriptors recorded while encoding a declaration or a complete program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncodedIntegerTypes(BTreeMap<IntegerType, ClosedIntegerType>);

impl EncodedIntegerTypes {
    pub(crate) fn record(&mut self, integer: IntegerType) -> Result<ClosedIntegerType, CodecError> {
        if let Some(closed) = self.0.get(&integer) {
            return Ok(*closed);
        }
        let closed =
            ClosedIntegerType::try_from(integer).map_err(CodecError::IntegerWidthClosure)?;
        self.0.insert(integer, closed);
        Ok(closed)
    }

    /// A raw input must name a descriptor this module actually retains.
    /// Independently constructed native constants use the core closed types.
    pub fn integer(&self, integer: IntegerType) -> Result<ClosedIntegerType, CodecError> {
        self.0
            .get(&integer)
            .copied()
            .ok_or(CodecError::MalformedStructuralFoundation(
                "integer type is not retained by this closed module",
            ))
    }
}
