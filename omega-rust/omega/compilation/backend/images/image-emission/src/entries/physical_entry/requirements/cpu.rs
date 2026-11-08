//! CPU prerequisites of the exact installed entry transitions.
//! No caller condition or target name establishes that the processor meets them.
use crate::ObjectArtifact;
use diagnostics::Diagnostic;
use semantic_vocabulary::OperationId;

/// Required throughout the physical invocation and its installed lifetimes.
/// Mapping/protection, backing, instruction support and delivery conditions
/// remain separate requirements; this is not a complete CPU conformance claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledPhysicalCpuRequirement {
    /// The emitted x86-64 transitions execute privileged descriptor/control
    /// operations at CPL0. Their stack switch, indirect entries and IRET do
    /// not implement CET shadow-stack or indirect-branch tracking protocols:
    /// CR4.CET must remain clear. No vector-state prerequisite is inferred.
    X86LongModeCpl0WithoutCet,
}

pub(super) fn derive(
    artifact: &ObjectArtifact,
    scopes: &[OperationId],
) -> Result<Option<InstalledPhysicalCpuRequirement>, Diagnostic> {
    if scopes.is_empty() {
        return Ok(None);
    }
    let invalid = super::super::invalid;
    if artifact.target().architecture != target::Architecture::X86_64 {
        return Err(invalid(
            "installed physical CPU requirements have no supported transition",
        ));
    }
    let mut observed = vec![false; scopes.len()];
    for private in artifact.private_functions() {
        let Some(bridge) = &private.entry_bridge else {
            continue;
        };
        let Some(plan) = bridge.installed_interrupt() else {
            continue;
        };
        let execution = private
            .body
            .installed_entry_execution()
            .ok_or_else(|| invalid("installed CPU transition has no admitted parent execution"))?;
        let operation = execution.scope().operation();
        let Ok(index) = scopes.binary_search(&operation) else {
            continue;
        };
        if bridge.request().target != artifact.target()
            || private.body.psi() != artifact.psi()
            || private.body.target() != artifact.target()
            || bridge.request().inbound.call.entry_control
                != calling_conventions::EntryControl::InterruptReturn
            || !plan.has_canonical_geometry()
        {
            return Err(invalid(
                "installed CPU transition changed its physical owner or protocol",
            ));
        }
        // These are the retained protocols of the emitted bridge, not a scan
        // for convenient opcode bytes. Ordinary artifact replay validates the
        // complete bridge bytes and exact parent Scope association.
        match plan.protocol {
            runtime_abi::static_interrupts::InstalledInterruptProtocol::Acknowledgement
            | runtime_abi::static_interrupts::InstalledInterruptProtocol::FatalContext { .. } => {
                observed[index] = true;
            }
        }
    }
    if observed.iter().any(|present| !present) {
        return Err(invalid(
            "installed CPU requirement has no actual entry transition",
        ));
    }
    Ok(Some(
        InstalledPhysicalCpuRequirement::X86LongModeCpl0WithoutCet,
    ))
}
