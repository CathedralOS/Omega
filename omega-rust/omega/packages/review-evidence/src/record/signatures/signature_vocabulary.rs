// Canonical type identities, static parameter kinds, machine parameter contracts, and alpha-normalized conformance bounds.

use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewCrashRoute;
use crate::record::PackageReviewDataProperties;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewSynchronousInvocation;
use crate::record::PackageReviewTermination;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewTypeIdentity {
    pub(crate) canonical: String,
}

impl PackageReviewTypeIdentity {
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewTypeParameterKind {
    Type,
    Implementation, // Positional implementation selection whose contract lives in declaration-ordered conformance bounds.
    Const(PackageReviewTypeIdentity),
    Value(PackageReviewTypeIdentity), // Runtime-capable value binder kept distinct from `const`.
    Machine(PackageReviewMachineParameterContract),
    Proposition(PackageReviewPropositionParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionParameterSignature {
    pub(crate) parameters: Vec<PackageReviewPropositionParameterValue>,
}

impl PackageReviewPropositionParameterSignature {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewPropositionParameterValue {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewPropositionParameterValue {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewMachineParameterContract {
    Structural(PackageReviewMachineParameterSignature),
    Nominal {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
        trait_arguments: Vec<PackageReviewTypeIdentity>, // Binder's trait application, empty for a parameterless trait.
    },
    RequirementIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewMachineParameterSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) conformance_bounds: Vec<crate::record::PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackageReviewCrashRoute>,
    pub(crate) service_reach: Vec<PackageReviewNominalIdentity>,
    pub(crate) service_reach_is_installation_bound: bool,
    pub(crate) synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) suspends: bool,
    pub(crate) blocks: bool,
    pub(crate) termination: PackageReviewTermination,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewSignatureParameter {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewSignatureParameter {
    crate::record::accessors::signature_parameter_accessors!();
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewTypeParameter {
    pub(crate) kind: PackageReviewTypeParameterKind,
    pub(crate) bounds: PackageReviewDataProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConformanceBound {
    pub(crate) binder_ordinal: u32,
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
}

impl PackageReviewConformanceBound {
    field_projection_accessors!(copied binder_ordinal: u32);
    field_projection_accessors!(borrowed trait_identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced trait_lifetime_arguments: &[u32]);
    field_projection_accessors!(sliced arguments: &[PackageReviewTypeIdentity]);
}
