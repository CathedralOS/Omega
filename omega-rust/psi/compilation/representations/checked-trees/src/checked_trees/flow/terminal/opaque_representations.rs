//! The exact selected carrier relationship beside checked storage geometry.

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedOpaqueRepresentation {
    pub opaque: typed_trees::type_identity::TypeIdentityId,
    pub carrier: typed_trees::type_identity::TypeIdentityId,
    pub application: language_semantics::opaque_representations::SelectedOpaqueApplication,
}
