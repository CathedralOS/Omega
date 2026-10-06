use crate::record::PackageReviewCallableContract;
use crate::record::PackageReviewCrashRoute;
use crate::record::PackageReviewDataProperties;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewSynchronousInvocation;
use crate::record::PackageReviewTermination;

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
pub enum PackageReviewTypeParameterKind {
    Type,
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

impl PackageReviewPropositionParameterSignature {
    pub fn parameters(&self) -> &[PackageReviewPropositionParameterValue] {
        &self.parameters
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionParameterValue {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewPropositionParameterValue {
    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewMachineParameterContract {
    Structural(PackageReviewMachineParameterSignature),
    Nominal {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    RequirementIdentity,
}

impl PackageReviewMachineParameterContract {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

impl PackageReviewMachineParameterSignature {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewMachineParameterValue {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewTypeParameter {
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
    pub const fn binder_ordinal(&self) -> u32 {
        self.binder_ordinal
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

impl PackageReviewTypeParameter {}
