use crate::TypedTrees;
use crate::source_state::service_declarations::signatures::PreparedBoundarySignatures;
use std::sync::{Arc, OnceLock};

/// Exact invocation-local custody of one immutable typed source owner.
/// This is an association, never a checking verdict or a content identity.
#[derive(Clone, Debug)]
pub struct ProductSourceIdentity(std::sync::Weak<FrozenTypedTrees>);

impl ProductSourceIdentity {
    pub fn new(program: &Arc<FrozenTypedTrees>) -> Self {
        Self(Arc::downgrade(program))
    }

    pub fn same_source(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.0, &other.0)
    }

    pub fn is_for(&self, program: &Arc<FrozenTypedTrees>) -> bool {
        self.same_source(&Self::new(program))
    }
}

impl PartialEq for ProductSourceIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.same_source(other)
    }
}

impl Eq for ProductSourceIdentity {}

/// An immutable representation snapshot with sealed declaration indexes.
/// Stage 04 retains its completed output in a separate prepared source owner.
#[derive(Clone, Default)]
pub struct FrozenTypedTrees(TypedTrees, OnceLock<Arc<PreparedBoundarySignatures>>);

impl FrozenTypedTrees {
    pub fn boundary_signatures(&self) -> &Arc<PreparedBoundarySignatures> {
        self.1
            .get_or_init(|| Arc::new(PreparedBoundarySignatures::prepare(&self.0)))
    }

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

impl record_storage::Record for FrozenTypedTrees {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        record_storage::Record::write(&self.0, output)
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        let recovered = <TypedTrees as record_storage::Record>::read(input)?;
        recovered
            .validate_recovered_callback_contracts()
            .map_err(|reason| input.error(reason))?;
        Ok(Self::from(recovered))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_identity_rejects_equal_content_in_another_allocation() {
        let owner = Arc::new(FrozenTypedTrees::from(TypedTrees::default()));
        let other = Arc::new(owner.as_ref().clone());
        let identity = ProductSourceIdentity::new(&owner);
        assert!(identity.is_for(&owner));
        assert!(!identity.is_for(&other));
        assert_ne!(identity, ProductSourceIdentity::new(&other));
    }

    #[test]
    fn source_identity_does_not_keep_the_source_alive() {
        let owner = Arc::new(FrozenTypedTrees::from(TypedTrees::default()));
        let weak = Arc::downgrade(&owner);
        let identity = ProductSourceIdentity::new(&owner);
        let retained = identity.clone();
        drop(owner);
        assert!(weak.upgrade().is_none());
        assert_eq!(identity, retained);
    }

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
