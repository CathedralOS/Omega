// Provider plan realization rows and leaf binding mechanisms (preserving evaluated producers without evaluator receipts).

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackageReviewCompilerIntrinsicExecution,
    PackageReviewForeignLocator, PackageReviewNominalIdentity,
    PackageReviewSelectedInstallationReach,
};
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderRow {
    pub(crate) method: String,
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) requirement_application: Option<String>,
    pub(crate) realization: Option<PackageReviewNominalIdentity>, // Authored realization machine (`None` for toolchain-settled rows).
    pub(crate) native_build: Option<PackagePolicyEvaluatedBindingProducer>, // Actual Build producer when native supply has no realization body.
    pub(crate) requirement_lifetime_partition: Vec<u32>,
    pub(crate) admitted_same_stack_bytes: u64, // Authored same-stack demand in bytes (`0` when no `stack N bytes` clause was written).
    pub(crate) binding: PackagePolicyProviderBinding,
    pub(crate) compiler_intrinsic_execution: Option<PackageReviewCompilerIntrinsicExecution>,
    pub(crate) installation_reach: Option<PackageReviewSelectedInstallationReach>,
}

impl PackagePolicyProviderRow {
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn realization(&self) -> Option<&PackageReviewNominalIdentity> {
        self.realization.as_ref()
    }

    field_projection_accessors!(sliced requirement_lifetime_partition: &[u32]);
    field_projection_accessors!(copied admitted_same_stack_bytes: u64);

    field_projection_accessors!(borrowed binding: &PackagePolicyProviderBinding);

    pub(crate) fn compiler_intrinsic_execution(
        &self,
    ) -> Option<PackageReviewCompilerIntrinsicExecution> {
        self.compiler_intrinsic_execution
    }

    pub fn installation_reach(&self) -> Option<&PackageReviewSelectedInstallationReach> {
        self.installation_reach.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderEvaluatedSyscall {
    pub(crate) target: String,
    pub(crate) producer: PackagePolicyEvaluatedBindingProducer,
}

impl PackagePolicyProviderEvaluatedSyscall {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyProviderBinding {
    Import {
        target: String,
        locator: PackageReviewForeignLocator,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
    Syscall {
        number: i64,
        evaluated: Option<PackagePolicyProviderEvaluatedSyscall>,
    },
    CompilerIntrinsic {
        machine: String,
    },
    VtableSlot {
        index: i64,
    },
    VtableField {
        table: String,
        field: String,
        table_declaration: PackageReviewNominalIdentity,
        storage: Result<effects::provider_plan::TableFieldStorage, String>,
    },
    TableFunction {
        table: String,
        field: String,
        table_declaration: PackageReviewNominalIdentity,
        storage: Result<effects::provider_plan::TableFieldStorage, String>,
    },
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<PackageKeyIdentity>,
    },
}
