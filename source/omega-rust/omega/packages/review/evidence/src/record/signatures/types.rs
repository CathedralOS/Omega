use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewTypeIdentity {
    pub(crate) canonical: String,
}

impl PackageReviewTypeIdentity {
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewTypeParameterKind {
    Type,
    Const(PackageReviewTypeIdentity),
    Machine(PackageReviewMachineParameterContract),
    Proposition(PackageReviewPropositionParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionParameterSignature {
    pub(crate) parameters: Vec<PackageReviewPropositionParameterValue>,
}

impl PackageReviewPropositionParameterSignature {
    pub fn parameters(&self) -> &[PackageReviewPropositionParameterValue] {
        &self.parameters
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionParameterValue {
    pub(crate) type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewPropositionParameterValue {
    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewMachineParameterContract {
    Structural(PackageReviewMachineParameterSignature),
    Nominal {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    RequirementIdentity,
}

impl PackageReviewMachineParameterContract {
    pub const fn structural(&self) -> Option<&PackageReviewMachineParameterSignature> {
        match self {
            Self::Structural(signature) => Some(signature),
            Self::Nominal { .. } | Self::RequirementIdentity => None,
        }
    }

    pub const fn nominal(
        &self,
    ) -> Option<(&PackageReviewNominalIdentity, &PackageReviewNominalIdentity)> {
        match self {
            Self::Structural(_) | Self::RequirementIdentity => None,
            Self::Nominal {
                trait_identity,
                requirement_identity,
            } => Some((trait_identity, requirement_identity)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewMachineParameterSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewMachineParameterValue>,
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

impl PackageReviewMachineParameterSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn parameters(&self) -> &[PackageReviewMachineParameterValue] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

    pub fn published_crash(&self) -> &[PackageReviewCrashRoute] {
        &self.published_crash
    }

    pub fn service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.service_reach
    }

    pub const fn service_reach_is_installation_bound(&self) -> bool {
        self.service_reach_is_installation_bound
    }

    pub fn synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.synchronous_invocations
    }

    pub const fn suspends(&self) -> bool {
        self.suspends
    }

    pub const fn blocks(&self) -> bool {
        self.blocks
    }

    pub const fn termination(&self) -> &PackageReviewTermination {
        &self.termination
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewMachineParameterValue {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
    pub(crate) is_self: bool,
}

impl PackageReviewMachineParameterValue {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    pub const fn is_self(&self) -> bool {
        self.is_self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTypeParameter {
    pub(crate) kind: PackageReviewTypeParameterKind,
    pub(crate) bounds: PackageReviewDataProperties,
}

/// One generic conformance requirement in a public signature.
///
/// An explicit proof-static binder is alpha-normalized to `binder_ordinal`;
/// `None` retains a binder-free `where T satisfies Trait` requirement without
/// fabricating evidence. The subject is the ordinal of an ordinary type
/// parameter in the containing declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConformanceBound {
    pub(crate) binder_ordinal: Option<u32>,
    pub(crate) subject_parameter: u32,
    pub(crate) selected_conformance: Option<PackageReviewNominalIdentity>,
    pub(crate) selected_lifetime_arguments: Vec<u32>,
    pub(crate) selected_arguments: Vec<PackageReviewContractStaticArgument>,
    pub(crate) selected_subject: Option<PackageReviewContractStaticArgument>,
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
}

impl PackageReviewConformanceBound {
    pub const fn binder_ordinal(&self) -> Option<u32> {
        self.binder_ordinal
    }

    pub const fn subject_parameter(&self) -> u32 {
        self.subject_parameter
    }

    pub const fn selected_conformance(&self) -> Option<&PackageReviewNominalIdentity> {
        self.selected_conformance.as_ref()
    }

    pub fn selected_lifetime_arguments(&self) -> &[u32] {
        &self.selected_lifetime_arguments
    }

    pub fn selected_arguments(&self) -> &[PackageReviewContractStaticArgument] {
        &self.selected_arguments
    }

    pub const fn selected_subject(&self) -> Option<&PackageReviewContractStaticArgument> {
        self.selected_subject.as_ref()
    }

    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub fn trait_lifetime_arguments(&self) -> &[u32] {
        &self.trait_lifetime_arguments
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }
}

impl PackageReviewTypeParameter {
    pub const fn kind(&self) -> &PackageReviewTypeParameterKind {
        &self.kind
    }

    pub const fn bounds(&self) -> PackageReviewDataProperties {
        self.bounds
    }
}
