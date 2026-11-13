// External executable policy identity and binding vocabulary without evaluation receipts or executable artifacts.

mod signatures;
mod validation;
use super::{PackageReviewForeignLocator, PackageReviewNominalIdentity};
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
