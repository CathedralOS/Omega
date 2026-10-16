//! Bind physical arrival to an explicitly selected, checked source provider.
//!
//! The adapter receives firmware arguments. The semantic continuation remains
//! a separate checked declaration; ordinary calls into it must transfer its
//! required roots. Selecting this adapter does not mint storage authority.

use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalProgramEntry {
    terminal: terminal_psi::TerminalPsiIdentity,
    machine: MachineId,
    parameter_formals: Vec<u32>,
    candidate: terminal_psi::ProviderCandidateConformance,
}

impl PhysicalProgramEntry {
    pub const fn terminal(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal
    }
    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn parameter_formals(&self) -> &[u32] {
        &self.parameter_formals
    }

    pub(crate) fn bind(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        application: &calling_conventions::BoundaryCallingApplication,
        selected: &[crate::SelectedProviderAdapter],
        machine: MachineId,
    ) -> Result<Self, &'static str> {
        let module = artifact.semantic_module();
        let body = module
            .machine(machine)
            .ok_or("physical adapter body is absent")?;
        let mut candidates = module.provider_candidates.iter().filter(|candidate| {
            candidate.candidate == machine
                && module.identities.text(candidate.requirement_identity)
                    == Some(application.requirement_identity.as_str())
                && selected.iter().any(|selection| {
                    selection.requirement_identity == application.requirement_identity
                        && module.identities.text(candidate.candidate_identity)
                            == Some(selection.machine_identity.as_str())
                        && module.identities.text(candidate.provider_identity)
                            == Some(selection.provider_identity.as_str())
                        && module
                            .identities
                            .text(candidate.provider_declaration.identity())
                            == Some(selection.provider_declaration.as_str())
                })
        });
        let candidate = candidates
            .next()
            .ok_or("physical adapter has no exact selected checked conformance")?;
        if candidates.next().is_some() {
            return Err("physical adapter selection is ambiguous");
        }
        let boundary = module
            .boundary_machine(candidate.boundary)
            .ok_or("physical adapter requirement is absent")?;
        if boundary.scalar_parameters.len() != body.parameters.len()
            || boundary.structural_parameters.len() != body.structural_parameters.len()
        {
            return Err("physical adapter formals differ from its retained body");
        }
        if !boundary.has_valid_parameter_order()
            || boundary.parameter_order.len() != application.plan.plan().call.parameters.len()
        {
            return Err("physical adapter formals are not the complete arrival signature");
        }
        // The checked conformance preserves each lane's positional order.
        // Derive its interleave from the retained requirement, never accept
        // a caller-supplied permutation of equal-sized firmware arguments.
        let parameter_formals = boundary
            .parameter_positions(terminal_psi::BoundaryParameterKind::Scalar)
            .chain(boundary.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
            .map(|position| u32::try_from(position).map_err(|_| "physical formal index overflows"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            terminal: artifact.view().identity(),
            machine,
            parameter_formals,
            candidate: candidate.clone(),
        })
    }
}
