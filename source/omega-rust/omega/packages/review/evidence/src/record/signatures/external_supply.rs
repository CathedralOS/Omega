use super::*;

/// Closed structural identity of executable code supplied outside Omega.
/// String fields are foreign ABI identifiers, not package-authored policy or
/// capability classifications.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalBinding {
    Import { library: String, symbol: String },
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: String },
    TableFunction { field: String },
}

/// One trust-bearing association between an exact reviewed callable,
/// requirement application, and externally supplied executable mechanism.
/// This is not Terminal evidence and makes no implementation-correctness or
/// audit claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalRequirement {
    Trait(PackageReviewCallableConformance),
    Operator(PackageReviewOperatorCoordinate),
    TopLevelRequirement(PackageReviewNominalIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableParameter {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
    pub(crate) is_self: bool,
}

impl PackageReviewExternalCallableParameter {
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

/// Self-contained callable shape for executable code supplied outside Omega.
/// The static count currently represents only ordinary type parameters with
/// default properties; projection rejects richer static telescopes until their
/// exact structure has a stable carrier here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameter_count: usize,
    pub(crate) parameters: Vec<PackageReviewExternalCallableParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
}

impl PackageReviewExternalCallableSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub const fn type_parameter_count(&self) -> usize {
        self.type_parameter_count
    }

    pub fn parameters(&self) -> &[PackageReviewExternalCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalExecutableSupply {
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) signature: PackageReviewExternalCallableSignature,
    pub(crate) requirement: PackageReviewExternalRequirement,
    pub(crate) binding: PackageReviewExternalBinding,
}

impl PackageReviewExternalExecutableSupply {
    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn signature(&self) -> &PackageReviewExternalCallableSignature {
        &self.signature
    }

    pub const fn requirement(&self) -> &PackageReviewExternalRequirement {
        &self.requirement
    }

    pub const fn conformance(&self) -> Option<&PackageReviewCallableConformance> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(conformance) => Some(conformance),
            PackageReviewExternalRequirement::Operator(_)
            | PackageReviewExternalRequirement::TopLevelRequirement(_) => None,
        }
    }

    pub const fn operator(&self) -> Option<&PackageReviewOperatorCoordinate> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::TopLevelRequirement(_) => None,
            PackageReviewExternalRequirement::Operator(operator) => Some(operator),
        }
    }

    pub const fn top_level_requirement(&self) -> Option<&PackageReviewNominalIdentity> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::Operator(_) => None,
            PackageReviewExternalRequirement::TopLevelRequirement(requirement) => Some(requirement),
        }
    }

    pub const fn binding(&self) -> &PackageReviewExternalBinding {
        &self.binding
    }
}
