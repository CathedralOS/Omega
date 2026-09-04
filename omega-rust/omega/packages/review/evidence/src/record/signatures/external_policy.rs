//! External executable policy identity without evaluation receipts.

use super::{
    PackageReviewExternalBinding, PackageReviewExternalCallableSignature,
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewForeignLocator, PackageReviewNominalIdentity,
};
use psi_core::PackageKeyIdentity;

/// Exact producer identity retained independently of its evaluation history.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyEvaluatedBindingProducer {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) package: Option<PackageKeyIdentity>,
    pub(crate) callable_identity: String,
}

impl PackagePolicyEvaluatedBindingProducer {
    pub const fn declaration(&self) -> &PackageReviewNominalIdentity {
        &self.declaration
    }

    pub const fn package(&self) -> Option<PackageKeyIdentity> {
        self.package
    }

    pub fn callable_identity(&self) -> &str {
        &self.callable_identity
    }
}

/// Complete external binding vocabulary for policy comparison.
///
/// Typed binding results remain distinct from legacy string and integer
/// carriers. No variant retains an evaluator receipt or executable artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyExternalBinding {
    Import {
        library: String,
        symbol: String,
    },
    NormalizedImport {
        target: String,
        locator: PackageReviewForeignLocator,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
    NormalizedSyscall {
        target: String,
        number: i64,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
    Syscall {
        number: i64,
    },
    CompilerIntrinsic,
    VtableSlot {
        index: i64,
    },
    VtableField {
        field: String,
    },
    TableFunction {
        field: String,
    },
}

impl From<&PackageReviewExternalBinding> for PackagePolicyExternalBinding {
    fn from(binding: &PackageReviewExternalBinding) -> Self {
        match binding {
            PackageReviewExternalBinding::Import { library, symbol } => Self::Import {
                library: library.clone(),
                symbol: symbol.clone(),
            },
            PackageReviewExternalBinding::NormalizedImport(import) => Self::NormalizedImport {
                target: import.target.clone(),
                locator: import.locator.clone(),
                producer: PackagePolicyEvaluatedBindingProducer {
                    declaration: import.producer.clone(),
                    package: import.producer_package,
                    callable_identity: import.producer_callable_identity.clone(),
                },
            },
            PackageReviewExternalBinding::NormalizedSyscall(syscall) => Self::NormalizedSyscall {
                target: syscall.target.clone(),
                number: syscall.number,
                producer: PackagePolicyEvaluatedBindingProducer {
                    declaration: syscall.producer.clone(),
                    package: syscall.producer_package,
                    callable_identity: syscall.producer_callable_identity.clone(),
                },
            },
            PackageReviewExternalBinding::Syscall { number } => Self::Syscall { number: *number },
            PackageReviewExternalBinding::CompilerIntrinsic => Self::CompilerIntrinsic,
            PackageReviewExternalBinding::VtableSlot { index } => {
                Self::VtableSlot { index: *index }
            }
            PackageReviewExternalBinding::VtableField { field } => Self::VtableField {
                field: field.clone(),
            },
            PackageReviewExternalBinding::TableFunction { field } => Self::TableFunction {
                field: field.clone(),
            },
        }
    }
}

/// Normalized external-supply finding, not acceptance or proof authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyExternalExecutableSupply {
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) signature: PackageReviewExternalCallableSignature,
    pub(crate) requirement: PackageReviewExternalRequirement,
    pub(crate) binding: PackagePolicyExternalBinding,
}

impl PackagePolicyExternalExecutableSupply {
    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn signature(&self) -> &PackageReviewExternalCallableSignature {
        &self.signature
    }

    pub const fn requirement(&self) -> &PackageReviewExternalRequirement {
        &self.requirement
    }

    pub const fn binding(&self) -> &PackagePolicyExternalBinding {
        &self.binding
    }
}

impl PackageReviewExternalExecutableSupply {
    /// Retain the exact contract, requirement and selected binding while
    /// discarding evaluator accounting and reconstruction receipts.
    /// This projection records no decision and cannot license an invocation.
    pub fn policy_projection(&self) -> PackagePolicyExternalExecutableSupply {
        PackagePolicyExternalExecutableSupply {
            callable: self.callable.clone(),
            signature: self.signature.clone(),
            requirement: self.requirement.clone(),
            binding: PackagePolicyExternalBinding::from(&self.binding),
        }
    }
}

#[cfg(test)]
mod tests;
