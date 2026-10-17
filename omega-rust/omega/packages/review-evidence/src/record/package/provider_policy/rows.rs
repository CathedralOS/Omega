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
    /// The authored realization machine, when one exists. A toolchain-settled
    /// plan row has no authored machine to name: its realization is a
    /// toolchain settlement table entry, so this stays `None`.
    pub(crate) realization: Option<PackageReviewNominalIdentity>,
    pub(crate) requirement_lifetime_partition: Vec<u32>,
    /// Authored same-stack demand in bytes. Zero means the realization wrote
    /// no `stack N bytes` clause.
    pub(crate) admitted_same_stack_bytes: u64,
    pub(crate) binding: PackagePolicyProviderBinding,
    pub(crate) compiler_intrinsic_execution: Option<PackageReviewCompilerIntrinsicExecution>,
    pub(crate) installation_reach: Option<PackageReviewSelectedInstallationReach>,
}

impl PackagePolicyProviderRow {
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }

    pub fn realization(&self) -> Option<&PackageReviewNominalIdentity> {
        self.realization.as_ref()
    }

    pub fn requirement_lifetime_partition(&self) -> &[u32] {
        &self.requirement_lifetime_partition
    }

    pub fn admitted_same_stack_bytes(&self) -> u64 {
        self.admitted_same_stack_bytes
    }

    pub fn binding(&self) -> &PackagePolicyProviderBinding {
        &self.binding
    }

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

/// Leaf mechanisms preserve evaluated producers but never evaluator receipts.
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
    },
    TableFunction {
        table: String,
        field: String,
        table_declaration: PackageReviewNominalIdentity,
    },
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<PackageKeyIdentity>,
    },
}
