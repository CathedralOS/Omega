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
    TopLevelRequirement {
        identity: PackageReviewNominalIdentity,
        signature: PackageReviewExternalCallableSignature,
    },
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalStaticParameter {
    Type {
        properties: PackageReviewDataProperties,
    },
    Const {
        type_identity: PackageReviewTypeIdentity,
    },
}

impl PackageReviewExternalStaticParameter {
    pub const fn type_properties(&self) -> Option<PackageReviewDataProperties> {
        match self {
            Self::Type { properties } => Some(*properties),
            Self::Const { .. } => None,
        }
    }

    pub const fn const_type_identity(&self) -> Option<&PackageReviewTypeIdentity> {
        match self {
            Self::Type { .. } => None,
            Self::Const { type_identity } => Some(type_identity),
        }
    }
}

/// Self-contained callable shape for executable code supplied outside Omega.
/// The static telescope currently represents ordinary type parameters with
/// their exact property bounds and const parameters with their exact carrier;
/// projection rejects other static kinds until their exact structure has a
/// stable carrier here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalCallableSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) static_parameters: Vec<PackageReviewExternalStaticParameter>,
    pub(crate) parameters: Vec<PackageReviewExternalCallableParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
}

impl PackageReviewExternalCallableSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn static_parameters(&self) -> &[PackageReviewExternalStaticParameter] {
        &self.static_parameters
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
            | PackageReviewExternalRequirement::TopLevelRequirement { .. } => None,
        }
    }

    pub const fn operator(&self) -> Option<&PackageReviewOperatorCoordinate> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::TopLevelRequirement { .. } => None,
            PackageReviewExternalRequirement::Operator(operator) => Some(operator),
        }
    }

    pub const fn top_level_requirement(&self) -> Option<&PackageReviewNominalIdentity> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::Operator(_) => None,
            PackageReviewExternalRequirement::TopLevelRequirement { identity, .. } => {
                Some(identity)
            }
        }
    }

    pub const fn top_level_requirement_signature(
        &self,
    ) -> Option<&PackageReviewExternalCallableSignature> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_)
            | PackageReviewExternalRequirement::Operator(_) => None,
            PackageReviewExternalRequirement::TopLevelRequirement { signature, .. } => {
                Some(signature)
            }
        }
    }

    pub const fn binding(&self) -> &PackageReviewExternalBinding {
        &self.binding
    }
}
