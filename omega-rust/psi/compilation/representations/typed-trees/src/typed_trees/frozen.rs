//! Readonly ownership of the prepared typed form. Checking publishes facts,
//! but cannot change the tables whose occurrence handles those facts name.
//! This is an ownership seal on TypedTrees, not another pipeline program form.
//! A transform must consume the owner and construct a new result; there is
//! deliberately no mutable borrow or copy-on-write API.
//!
//! Freezing also seals the declaration indexes: no table can move while the
//! seal holds, so checking workers share one built copy without locking.

use super::TypedTrees;
use crate::service_declarations::signatures::PreparedBoundarySignatures;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Default)]
pub struct FrozenTypedTrees(TypedTrees, OnceLock<Arc<PreparedBoundarySignatures>>);

impl FrozenTypedTrees {
    /// One source-owned declaration census for this immutable snapshot. Its
    /// coordinates move with the typed owner; unlike the service catalog it
    /// contains no address-bound owner witness. Reopening source drops it.
    pub fn boundary_signatures(&self) -> &Arc<PreparedBoundarySignatures> {
        self.1
            .get_or_init(|| Arc::new(PreparedBoundarySignatures::prepare(&self.0)))
    }

    /// Transfer the program into construction scratch for another transform.
    pub fn into_inner(mut self) -> TypedTrees {
        self.0.unseal_declaration_indexes();
        self.0
    }
}

impl From<TypedTrees> for FrozenTypedTrees {
    fn from(mut program: TypedTrees) -> Self {
        program.seal_declaration_indexes();
        Self(program, OnceLock::new())
    }
}

impl std::fmt::Debug for FrozenTypedTrees {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("FrozenTypedTrees")
            .field(&self.0)
            .finish()
    }
}

impl PartialEq for FrozenTypedTrees {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for FrozenTypedTrees {}

impl std::ops::Deref for FrozenTypedTrees {
    type Target = TypedTrees;

    fn deref(&self) -> &TypedTrees {
        &self.0
    }
}

impl AsRef<TypedTrees> for FrozenTypedTrees {
    fn as_ref(&self) -> &TypedTrees {
        &self.0
    }
}

impl compiler_storage::Record for FrozenTypedTrees {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        compiler_storage::Record::write(&self.0, output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        <TypedTrees as compiler_storage::Record>::read(input).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_clones_share_census_but_reopened_source_does_not() {
        let frozen = FrozenTypedTrees::from(TypedTrees::default());
        let census = Arc::clone(frozen.boundary_signatures());
        let cloned = frozen.clone();
        assert!(Arc::ptr_eq(&census, cloned.boundary_signatures()));
        let reopened = FrozenTypedTrees::from(frozen.into_inner());
        assert!(!Arc::ptr_eq(&census, reopened.boundary_signatures()));
    }

    #[test]
    fn derived_census_does_not_change_program_equality() {
        let frozen = FrozenTypedTrees::from(TypedTrees::default());
        let unprepared = frozen.clone();
        frozen.boundary_signatures();
        assert_eq!(frozen, unprepared);
    }
}
