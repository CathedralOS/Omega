// External executable policy identity and binding vocabulary without evaluation receipts or executable artifacts.

mod signatures;
mod validation;
use super::{
    PackageReviewExternalBinding, PackageReviewForeignLocator, PackageReviewNominalIdentity,
};
use crate::record::accessors::field_projection_accessors;
use semantic_vocabulary::PackageKeyIdentity;
pub(crate) use signatures::{
    PackagePolicyExternalCallableSignature, PackagePolicyExternalRequirement,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyEvaluatedBindingProducer {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) package: Option<PackageKeyIdentity>,
    pub(crate) callable_identity: String,
}

impl PackagePolicyEvaluatedBindingProducer {
    field_projection_accessors!(borrowed declaration: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied package: Option<PackageKeyIdentity>);

    pub fn callable_identity(&self) -> &str {
        &self.callable_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyExternalBinding {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyExternalExecutableSupply {
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) target_context: crate::record::PackagePolicyTargetContext,
    pub(crate) signature: PackagePolicyExternalCallableSignature,
    pub(crate) requirement: PackagePolicyExternalRequirement,
    pub(crate) binding: PackagePolicyExternalBinding,
}

impl PackagePolicyExternalExecutableSupply {
    field_projection_accessors!(borrowed target_context: &crate::record::PackagePolicyTargetContext);
    field_projection_accessors!(borrowed callable: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed signature: &PackagePolicyExternalCallableSignature);
    field_projection_accessors!(borrowed requirement: &PackagePolicyExternalRequirement);
    field_projection_accessors!(borrowed binding: &PackagePolicyExternalBinding);
}
