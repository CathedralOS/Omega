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
    /// A positional implementation selection; its exact contract is retained
    /// in the owner's declaration-ordered conformance bounds.
    Implementation,
    Const(PackageReviewTypeIdentity),
    /// A runtime-capable value binder; today only statically supplied
    /// arguments specialize it, but the kind must not be republished as
    /// `const`.
    Value(PackageReviewTypeIdentity),
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
        /// The binder's trait application, empty for a parameterless trait.
        trait_arguments: Vec<PackageReviewTypeIdentity>,
    },
    RequirementIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewMachineParameterSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
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

/// One evidence binder in a public signature: `Order satisfies Ranked<T>`.
///
/// The binder is alpha-normalized to `binder_ordinal`, its position among the
/// declaration's evidence binders. The trait application carries every data
/// type the interface names; there is no subject parameter.
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
