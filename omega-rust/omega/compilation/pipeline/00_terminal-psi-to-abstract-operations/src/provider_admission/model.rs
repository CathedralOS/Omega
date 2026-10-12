//! Admitted provider and callback facts consumed by target lowering.
//! These carry execution evidence, not another program representation.

use calling_conventions::NativePlace;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use target_operations::BoundarySettlementRealization;

/// Owned target-side input for one compiler-private callback argument.
///
/// The exact Terminal operation is the join to the unchanged abstract
/// `BoundaryCall`. The application and complete registrar plan/context remain
/// data until this stage independently validates their one-slot relation. The
/// application commitment is retained compiler provenance, not authority this
/// reduced tuple can recompute; callers must supply it from the exact retained
/// placement owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackArgument {
    pub registrar_invokes_callback: bool,
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    /// The private materialization destination this callback binds. For a
    /// field destination, `application` is the root record parameter's
    /// application — the parameter slot the private-slot write flows through
    /// — and `field_byte_offset` names the slot inside that record.
    pub destination: NativePlace,
    pub field_byte_offset: Option<usize>,
    pub application: calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    pub registrar_context: calling_conventions::CallbackMaterializationContext,
    /// Nonempty compiler-origin application-v3 commitment projection.
    pub registrar_application_commitment: [u8; 32],
}

/// One boundary realization sourced from a validated, admitted provider
/// execution or the consuming lowerer's closed compiler-builtin catalog.
#[derive(Debug, Clone)]
pub struct AdmittedBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub execution: AdmittedBoundaryExecution<'execution>,
    pub realization: BoundarySettlementRealization,
}

/// Owned correspondence issued from one actually admitted provider execution.
///
/// Public identities index the canonical program table; selected requirement
/// identities index the selected-provider table retained beside this receipt.
/// The private constructor is reached only after complete schema and physical
/// binding admission. This transports that cut through optimization; it does
/// not promote a planning row or replay provider admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedProviderBoundaryCorrespondence {
    psi: terminal_psi::TerminalPsiIdentity,
    boundary: BoundaryMachineId,
    public_requirement: terminal_psi::TerminalIdentityIndex,
    selected_requirement: terminal_psi::TerminalIdentityIndex,
    target: target::NativeTarget,
    provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
    provider_plan_report_identity: u64,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

impl AdmittedProviderBoundaryCorrespondence {
    pub fn matches_public_boundary(
        self,
        psi: terminal_psi::TerminalPsiIdentity,
        boundary: BoundaryMachineId,
        public_requirement: terminal_psi::TerminalIdentityIndex,
    ) -> bool {
        self.psi == psi
            && self.boundary == boundary
            && self.public_requirement == public_requirement
    }

    /// Preserve every report field projected by target lowering from the
    /// same borrowed admitted execution, rather than accepting a plan tuple.
    pub fn matches_execution(self, execution: target_operations::ProviderExecutionBinding) -> bool {
        execution.direct_foreign_realization().is_none()
            && self.provider_plan_report_identity == execution.provider_plan_report_identity().get()
            && self.provider_execution_report_identity
                == execution.provider_execution_report_identity()
            && self.provider_execution_report_fingerprint
                == execution.provider_execution_report_fingerprint()
            && self.normalized_root_report_identity == execution.normalized_root_report_identity()
            && self.boundary_contract_report_fingerprint
                == execution.boundary_contract_report_fingerprint()
    }

    pub const fn selected_requirement(self) -> terminal_psi::TerminalIdentityIndex {
        self.selected_requirement
    }

    pub const fn target(self) -> target::NativeTarget {
        self.target
    }

    pub const fn provider_plan_digest(self) -> effects::provider_plan::ProviderPlanDigest {
        self.provider_plan_digest
    }

    pub const fn provider_plan_report_identity(self) -> u64 {
        self.provider_plan_report_identity
    }
}

/// The exact public boundary admitted against unchanged original provider evidence.
/// The owned correspondence and borrowed evidence are issued together, after
/// the selected plan, original row, public contract and physical realization join.
#[derive(Debug, Clone, Copy)]
pub struct AdmittedProviderBoundaryExecution<'execution> {
    correspondence: AdmittedProviderBoundaryCorrespondence,
    provider_execution: &'execution dyn installation_evidence::ProviderExecutionEvidence,
}

impl<'execution> AdmittedProviderBoundaryExecution<'execution> {
    pub(crate) fn new(
        psi: terminal_psi::TerminalPsiIdentity,
        boundary: BoundaryMachineId,
        public_requirement: terminal_psi::TerminalIdentityIndex,
        selected_requirement: terminal_psi::TerminalIdentityIndex,
        target: target::NativeTarget,
        provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
        provider_execution: &'execution dyn installation_evidence::ProviderExecutionEvidence,
    ) -> Self {
        Self {
            correspondence: AdmittedProviderBoundaryCorrespondence {
                psi,
                boundary,
                public_requirement,
                selected_requirement,
                target,
                provider_plan_digest,
                provider_plan_report_identity: provider_execution.provider_plan_report_identity(),
                provider_execution_report_identity: provider_execution
                    .provider_execution_report_identity(),
                provider_execution_report_fingerprint: provider_execution
                    .provider_execution_report_fingerprint(),
                normalized_root_report_identity: provider_execution
                    .normalized_root_report_identity(),
                boundary_contract_report_fingerprint: provider_execution
                    .boundary_contract_report_fingerprint(),
            },
            provider_execution,
        }
    }

    pub fn matches_public_boundary(
        self,
        psi: terminal_psi::TerminalPsiIdentity,
        boundary: BoundaryMachineId,
        public_requirement: terminal_psi::TerminalIdentityIndex,
    ) -> bool {
        self.correspondence
            .matches_public_boundary(psi, boundary, public_requirement)
    }

    pub(crate) const fn correspondence(self) -> AdmittedProviderBoundaryCorrespondence {
        self.correspondence
    }

    pub fn provider_execution(
        self,
    ) -> &'execution dyn installation_evidence::ProviderExecutionEvidence {
        self.provider_execution
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AdmittedBoundaryExecution<'execution> {
    Provider(AdmittedProviderBoundaryExecution<'execution>),
    CompilerBuiltin(target_operations::CompilerBuiltinExecution),
    DirectForeign(&'execution std::sync::Arc<effects::DirectForeignRealization>),
    DirectCompilerIntrinsic(
        &'execution std::sync::Arc<effects::DirectForeignRealization>,
        target_operations::CompilerBuiltinExecution,
    ),
}
