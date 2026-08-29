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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalExecutableSupply {
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) requirement: PackageReviewExternalRequirement,
    pub(crate) binding: PackageReviewExternalBinding,
}

impl PackageReviewExternalExecutableSupply {
    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn requirement(&self) -> &PackageReviewExternalRequirement {
        &self.requirement
    }

    pub const fn conformance(&self) -> Option<&PackageReviewCallableConformance> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(conformance) => Some(conformance),
            PackageReviewExternalRequirement::Operator(_) => None,
        }
    }

    pub const fn operator(&self) -> Option<&PackageReviewOperatorCoordinate> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_) => None,
            PackageReviewExternalRequirement::Operator(operator) => Some(operator),
        }
    }

    pub const fn binding(&self) -> &PackageReviewExternalBinding {
        &self.binding
    }
}
