use crate::record::*;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderRow {
    pub(crate) method: String,
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) realization: PackageReviewNominalIdentity,
    pub(crate) requirement_lifetime_partition: Vec<u32>,
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

    pub fn realization(&self) -> &PackageReviewNominalIdentity {
        &self.realization
    }

    pub fn requirement_lifetime_partition(&self) -> &[u32] {
        &self.requirement_lifetime_partition
    }

    pub fn binding(&self) -> &PackagePolicyProviderBinding {
        &self.binding
    }

    pub fn compiler_intrinsic_execution(&self) -> Option<PackageReviewCompilerIntrinsicExecution> {
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

impl PackagePolicyProviderEvaluatedSyscall {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn producer(&self) -> &PackagePolicyEvaluatedBindingProducer {
        &self.producer
    }
}

/// Leaf mechanisms preserve evaluated producers but never evaluator receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyProviderBinding {
    Import {
        target: String,
        locator: PackageReviewForeignLocator,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
    StringBackedImportBootstrap {
        library: String,
        symbol: String,
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
