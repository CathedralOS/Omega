//! Executable-scope manifest evidence types retained by live callers.

use crate::provider_plan::ProviderPlanDigest;
/// The execution scope whose known executable entries are being described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionScope {
    CallerAddressSpace,
    IsolatedProvider(u64),
}
/// Containment guarantees remain independent; one receipt cannot imply the
/// others merely because it names the same mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ContainmentGuarantee {
    MemoryIsolation,
    ForcibleTermination,
    FaultContainment,
    BoundedResources,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContainmentEvidence {
    pub guarantee: ContainmentGuarantee,
    pub evidence_identity: String,
}
/// Policy/admission input for one exact opaque selected-provider row.
///
/// Loader spellings remain only a binding match. `executable_identity` must
/// pin content, a signer/profile-owned platform identity, or another stable
/// artifact identity supplied by the admitting policy. Scope completeness is
/// independent: only `executable_closure_evidence_identity` claims that this
/// opaque executable cannot introduce unreported code in the named scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpaqueExecutableAdmissionCandidate {
    /// Compact report coordinate; never sufficient to select a plan.
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub method: String,
    pub requirement_identity: String,
    pub binding: OpaqueInProcessBinding,
    pub executable_identity: String,
    pub implementation_evidence_identity: String,
    pub execution_scope: ExecutionScope,
    pub containment: Vec<ContainmentEvidence>,
    pub executable_closure_evidence_identity: Option<String>,
}
/// Sealed admission whose provider, requirement, binding, and evidence have
/// been matched against the selected-provider closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedOpaqueExecutableAdmission(OpaqueExecutableAdmissionCandidate);

impl ValidatedOpaqueExecutableAdmission {
    pub const fn candidate(&self) -> &OpaqueExecutableAdmissionCandidate {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpaqueInProcessBinding {
    Import {
        evaluated: crate::provider_plan::EvaluatedForeignImport,
    },
    VtableSlot {
        index: i64,
    },
    VtableField {
        table: String,
        field: String,
    },
    TableFunction {
        table: String,
        field: String,
    },
}
