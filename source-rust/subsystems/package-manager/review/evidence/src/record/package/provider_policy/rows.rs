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
    pub method: String,
    pub requirement: PackageReviewNominalIdentity,
    pub requirement_application: Option<String>,
    pub realization: Option<PackageReviewNominalIdentity>, // Authored realization machine (`None` for toolchain-settled rows).
    pub native_build: Option<PackagePolicyEvaluatedBindingProducer>, // Actual Build producer when native supply has no realization body.
    pub requirement_lifetime_partition: Vec<u32>,
    pub admitted_same_stack_bytes: u64, // Authored same-stack demand in bytes (`0` when no `stack N bytes` clause was written).
    pub binding: PackagePolicyProviderBinding,
    pub compiler_intrinsic_execution: Option<PackageReviewCompilerIntrinsicExecution>,
    pub installation_reach: Option<PackageReviewSelectedInstallationReach>,
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
    pub target: String,
    pub producer: PackagePolicyEvaluatedBindingProducer,
}

impl PackagePolicyProviderEvaluatedSyscall {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyProviderBinding {
    FunctionField {
        target: String,
        selector: Box<semantic_vocabulary::FunctionFieldBinding>,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
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
        storage: Result<provider_plans::TableFieldStorage, String>,
    },
    TableFunction {
        table: String,
        field: String,
        table_declaration: PackageReviewNominalIdentity,
        storage: Result<provider_plans::TableFieldStorage, String>,
    },
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<PackageKeyIdentity>,
    },
}
