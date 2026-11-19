use crate::{CrashCause, DeclaredPauseBounds, RequirementSlot, ServiceApplication, Site, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::DeclarationId;

/// Declared call contract shared by body checking, importing packages and downstream consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSignature {
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub machine: DeclarationId,
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
