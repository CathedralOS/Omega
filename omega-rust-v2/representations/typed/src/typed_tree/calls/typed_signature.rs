use crate::{CrashCause, DeclaredPauseBounds, RequirementSlot, ServiceApplication, Site, ValueType};
use omega_resolved::DeclarationId;
use omega_tokens::SourceSpan;

/// The binding checked at the operator token, including the effective home inferred from its operands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedOperatorDeclaration {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub token: String,
    pub home: Option<DeclarationId>,
    pub public: bool,
}

/// Declared call contract shared by body checking, importing packages and downstream consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSignature {
    pub node: omega_resolved::TreeNodeId,
    pub site: Site,
    pub machine: DeclarationId,
    /// The defining toolchain selected a fixed catalog operation in place of a source body.
    pub catalog_supplied: bool,
    pub operator: Option<TypedOperatorDeclaration>,
    pub parameters: Vec<(SourceSpan, ValueType)>,
    pub result: ValueType,
    pub requires: bool,
    pub has_guarantee: bool,
    pub terminates: bool,
    pub satisfies: Option<RequirementSlot>,
    pub crash_ceiling: Option<Vec<CrashCause>>,
    pub pause: DeclaredPauseBounds,
    pub reach: Option<Vec<ServiceApplication>>,
    pub multiplicity_bounds: Vec<(usize, crate::Multiplicity)>,
}
