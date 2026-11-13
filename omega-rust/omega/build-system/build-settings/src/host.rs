/*
Host import adapters for an evaluated Build program.

A running Build machine interacts with the compiler through three bounded host
surfaces: declaration and provider binding effects (`bindings`), captured-input
and output filesystem calls (`filesystem`), and project/dependency/log host
requests (`requests`). Each adapter authenticates its compiler-issued facet or
activation ticket before recording or executing an effect.
*/

pub mod bindings;
pub(crate) mod filesystem;
pub mod requests;

/// Whether `effect` is the emitted call a host row was published for. The
/// three adapters find an effect's row by this and by nothing else. A row
/// names its call by machine, operation and boundary because an operation
/// identity is unique within its machine only: two helper machines of one
/// Build script make their calls at the same operation numbers.
pub(crate) fn is_call_at(
    effect: &terminal_interpreter::TerminalEffect,
    site: psi_compiler::BuildCallSite,
) -> bool {
    matches!(
        effect,
        terminal_interpreter::TerminalEffect::BoundaryCall {
            site: actual,
            ..
        } if site == psi_compiler::BuildCallSite {
            machine: actual.machine,
            operation: actual.operation,
            boundary: actual.boundary,
        }
    )
}

/// A row's local coordinates are meaningful only in the code and invocation
/// that published it. Another invocation of identical code must not use this
/// activation's receiver, tickets or filesystem scope.
pub(crate) fn require_effect_owner(
    effect: &terminal_interpreter::TerminalEffect,
    artifact: &terminal_codec::TerminalExecutionIdentity,
) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
    let terminal_interpreter::TerminalEffect::BoundaryCall { site, .. } = effect else {
        return Err(terminal_interpreter::TerminalEffectRejection::new(
            "not a Build host effect",
        ));
    };
    if &site.program != artifact {
        return Err(terminal_interpreter::TerminalEffectRejection::new(
            "Build host effect belongs to another accepted artifact",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_call_at;
    use psi_compiler::BuildCallSite;
    use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
    use terminal_interpreter::TerminalEffect;

    fn site(machine: u64, operation: u64, boundary: u64) -> BuildCallSite {
        BuildCallSite {
            machine: MachineId::new(machine).unwrap(),
            operation: OperationId::new(operation).unwrap(),
            boundary: BoundaryMachineId::new(boundary).unwrap(),
        }
    }

    fn call(at: BuildCallSite) -> TerminalEffect {
        TerminalEffect::BoundaryCall {
            site: terminal_interpreter::TerminalEffectWait {
                program: terminal_codec::TerminalExecutionIdentity::portable(
                    terminal_psi::TerminalPsiIdentity {
                        vocabulary_marker: terminal_psi::TerminalModule::for_entry(at.machine)
                            .vocabulary_marker,
                        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0; 32]),
                    },
                ),
                machine: at.machine,
                block: semantic_vocabulary::BlockId::new(1).unwrap(),
                operation: at.operation,
                boundary: at.boundary,
            },
            arguments: vec![],
            structural_arguments: vec![],
            byte_sequence_arguments: vec![],
            completion_receipts: vec![],
            result: terminal_psi::BoundaryMachineResult::Unit,
        }
    }

    /// Two helper machines call the same boundary at the same operation
    /// number. A row published for one is not the other's call.
    #[test]
    fn a_row_answers_only_for_the_call_of_its_own_machine() {
        let row = site(2, 1, 3);
        assert!(is_call_at(&call(row), row));
        assert!(!is_call_at(&call(site(4, 1, 3)), row));
        assert!(!is_call_at(&call(site(2, 5, 3)), row));
        assert!(!is_call_at(&call(site(2, 1, 6)), row));
    }
}
