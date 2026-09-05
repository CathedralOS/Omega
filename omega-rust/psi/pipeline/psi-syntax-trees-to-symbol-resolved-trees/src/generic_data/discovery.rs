//! Private discovered templates and pending rewrites; not a program representation.

use super::*;

pub(super) struct GenericData {
    pub(super) name: String,
    pub(super) origin_name: Identifier,
    pub(super) is_public: bool,
    pub(super) lifetime_parameters: Vec<Identifier>,
    pub(super) parameter_names: Vec<String>,
    pub(super) const_parameter_types: Vec<Option<TypeReferenceHandle>>,
    pub(super) where_facts: HandleSpan<ProofFact>,
    pub(super) members: HandleSpan<DataMember>,
    pub(super) properties: psi_syntax_trees::item::DataProperties,
    pub(super) supply_mode: psi_language_semantics::DataSupplyMode,
}

pub(super) struct PendingRewrite {
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) synthetic_name: String,
    pub(super) lifetime_arguments: Vec<Identifier>,
}

/// One discovered instantiation: the base generic definition and the argument
/// type references spelled for it, plus the plain name of the record to
/// synthesize.
pub(super) struct Instantiation {
    pub(super) synthetic_name: String,
    pub(super) base_name: String,
    pub(super) argument_handles: Vec<TypeReferenceHandle>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GenericDataShape {
    Record,
    PureSum,
    MixedSum,
}
