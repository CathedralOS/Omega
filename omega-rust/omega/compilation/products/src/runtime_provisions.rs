//! Source-free provisions retained before native realization consumes them.
//!
//! A runtime driver needs the actual admitted entry and installation, not a
//! second source compilation or a guess from names in an emitted image. These
//! records identify those provisions; they do not instantiate storage, grant
//! process-domain authority, or license a host callback. The consuming driver
//! supplies those premises at its own explicit runtime boundary.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{CheckedProgramEntryReceiverEligibility, TerminalPsiIdentity};

/// An exact host operation admitted for the native product. Reference execution
/// may implement its semantic contract, but cannot substitute another slot or
/// manufacture authority merely because a realization was selected.
#[derive(Debug, Clone)]
pub struct TerminalRuntimeHostProvision {
    pub boundary: BoundaryMachineId,
    pub realization: target_operations::BoundarySettlementRealization,
}

/// Entry and provider facts for the exact produced Terminal program. Source
/// arena handles and physical wrapper storage do not cross this record.
#[derive(Debug, Clone)]
pub struct TerminalRuntimeProvisions {
    identity: TerminalPsiIdentity,
    entry: MachineId,
    source_signature_identity: [u8; 32],
    target_slot: target::ProgramEntrySlotDeclaration,
    receiver: Option<CheckedProgramEntryReceiverEligibility>,
    fused_services: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    /// Parallel to `fused_services`: exact retained requirement slots of the
    /// same admitted schema/plan, not a provider-name or trait-prefix lookup.
    fused_service_boundary_slots: Vec<Vec<BoundaryMachineId>>,
    installation: Option<terminal_interpreter::AdmittedProviderInstallation>,
    host: Vec<TerminalRuntimeHostProvision>,
}

impl TerminalRuntimeProvisions {
    /// Capture already admitted facts while they still accompany the image
    /// request. This is not another admission or a runtime authority grant.
    pub fn capture_native_admission(
        checked: &terminal_psi::CheckedProgramEntryTerminalReceipt,
        fused_services: &[program_entry_plan::ProgramEntryFusedServiceEstablishment],
        installation: Option<&terminal_interpreter::AdmittedProviderInstallation>,
        host: Vec<TerminalRuntimeHostProvision>,
        boundaries: &[terminal_psi::BoundaryMachineDeclaration],
        selected: &effects::SelectedProviderPlanFacts,
        target_slot: target::ProgramEntrySlotDeclaration,
    ) -> Result<Self, &'static str> {
        let mut fused_service_boundary_slots = Vec::with_capacity(fused_services.len());
        for establishment in fused_services {
            let mut matching_plans = selected.plans().iter().filter(|plan| {
                plan.identity_digest() == establishment.selected_provider_plan_digest()
                    && plan.schema.identity_digest() == establishment.service_schema_digest()
            });
            let plan = matching_plans
                .next()
                .ok_or("runtime Fused establishment lost its exact admitted plan/schema")?;
            if matching_plans.next().is_some() {
                return Err("runtime Fused establishment repeats its exact admitted plan/schema");
            }
            // The native admission already authenticated these identities and
            // contracts. Capture the existing coordinate relation before that
            // admission is consumed; do not reconstruct semantics or authority.
            let mut slots = Vec::new();
            for boundary in boundaries {
                let requirement = selected
                    .specialized_requirement_row(&boundary.identity)
                    .filter(|row| row.provider_plan_report_identity == plan.report_fingerprint())
                    .map_or(boundary.identity.as_str(), |row| {
                        row.selected_requirement_identity.as_str()
                    });
                if plan
                    .schema
                    .methods
                    .iter()
                    .any(|method| method.requirement_identity == requirement)
                {
                    slots.push(boundary.id);
                }
            }
            slots.sort_unstable();
            slots.dedup();
            fused_service_boundary_slots.push(slots);
        }
        Ok(Self {
            identity: checked.terminal_psi_identity(),
            entry: checked.terminal_entry(),
            source_signature_identity: checked.source_signature_identity(),
            target_slot,
            receiver: checked.receiver_eligibility().cloned(),
            fused_services: fused_services.to_vec(),
            fused_service_boundary_slots,
            installation: installation.cloned(),
            host,
        })
    }

    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.identity
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.source_signature_identity
    }

    /// The actual admitted root's slot/arrival contract, not recovered from
    /// the source-signature digest or inferred from selected host providers.
    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn receiver(&self) -> Option<&CheckedProgramEntryReceiverEligibility> {
        self.receiver.as_ref()
    }

    pub fn fused_services(&self) -> &[program_entry_plan::ProgramEntryFusedServiceEstablishment] {
        &self.fused_services
    }

    /// Each actual root establishment's exact retained requirement slots.
    /// This association identifies what the host must establish; it supplies
    /// neither runtime Binding custody nor containing-process-domain authority.
    pub fn fused_service_slots(
        &self,
    ) -> impl Iterator<
        Item = (
            &program_entry_plan::ProgramEntryFusedServiceEstablishment,
            &[BoundaryMachineId],
        ),
    > {
        self.fused_services
            .iter()
            .zip(&self.fused_service_boundary_slots)
            .map(|(establishment, slots)| (establishment, slots.as_slice()))
    }

    pub const fn installation(
        &self,
    ) -> Option<&terminal_interpreter::AdmittedProviderInstallation> {
        self.installation.as_ref()
    }

    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        &self.host
    }
}
