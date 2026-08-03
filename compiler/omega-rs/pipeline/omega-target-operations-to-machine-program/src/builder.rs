use omega_machine_program::MachineProgram;
use omega_target_operations::InstructionPlan;
use psi_diagnostics::Diagnostic;

pub(crate) fn build_machine_program(
    instructions: &InstructionPlan,
) -> Result<MachineProgram, Diagnostic> {
    let assigned_target_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            instructions,
        );
    let machine_instructions =
        omega_assigned_target_operations_to_machine_instructions::build_machine_instructions(
            &assigned_target_operations,
        )?;

    Ok(MachineProgram::from(machine_instructions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{
        AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractPermissionEvent,
        AbstractSourceBoundaryEdge, AbstractValueFact, AbstractValueOrigin,
        AbstractValueStatementRole,
    };
    use psi_symbols::SymbolHandle;

    #[test]
    fn preserves_target_value_summary_into_machine_program() {
        let mut target_operations = InstructionPlan::default();
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);

        target_operations
            .semantics
            .values
            .values
            .insert(AbstractValueFact {
                source_key: Default::default(),
                machine_symbol,
                state_symbol,
                expression: Default::default(),
                origin: AbstractValueOrigin::Statement {
                    statement_index: 13,
                    role: AbstractValueStatementRole::TransitionTargetValue,
                },
                arithmetic_policy_adapter: None,
                operator_provider_plan_identity: None,
            });

        let machine_program = build_machine_program(&target_operations).expect("machine program");

        assert_eq!(machine_program.semantics.values.values.len(), 1);
        let value = machine_program
            .semantics
            .values
            .values
            .iter()
            .next()
            .map(|(_, value)| value)
            .expect("machine-program value");
        assert_eq!(
            value.origin,
            AbstractValueOrigin::Statement {
                statement_index: 13,
                role: AbstractValueStatementRole::TransitionTargetValue,
            }
        );
    }

    #[test]
    fn preserves_target_source_boundary_edges_into_machine_program() {
        let mut target_operations = InstructionPlan::default();
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let trait_symbol = SymbolHandle::from_arena_index(3);
        let signature_symbol = SymbolHandle::from_arena_index(4);

        target_operations
            .semantics
            .boundaries
            .source_edges
            .insert(AbstractSourceBoundaryEdge {
                source_key: Default::default(),
                statement_index: 21,
                call_ordinal: 2,
                receiver_symbol: machine_symbol,
                target_symbol: state_symbol,
                boundary_trait_symbol: trait_symbol,
                boundary_signature_symbol: signature_symbol,
            });
        target_operations
            .semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x4567);

        let machine_program = build_machine_program(&target_operations).expect("machine program");

        assert_eq!(machine_program.semantics.boundaries.source_edges.len(), 1);
        let edge = machine_program
            .semantics
            .boundaries
            .source_edges
            .iter()
            .next()
            .map(|(_, edge)| edge)
            .expect("machine-program source boundary edge");
        assert_eq!(edge.statement_index, 21);
        assert_eq!(edge.call_ordinal, 2);
        assert_eq!(edge.boundary_trait_symbol, trait_symbol);
        assert_eq!(edge.boundary_signature_symbol, signature_symbol);
        assert_eq!(
            machine_program
                .semantics
                .boundaries
                .footprints
                .boundary_contract_fingerprint,
            Some(0x4567)
        );
    }

    #[test]
    fn preserves_target_permission_summary_into_machine_program() {
        let mut target_operations = InstructionPlan::default();
        let target_symbol = SymbolHandle::from_arena_index(1);

        target_operations
            .semantics
            .ownership
            .permissions
            .insert(AbstractPermissionEvent {
                source: psi_language_semantics::PermissionEventSource::Call {
                    statement_index: 22,
                    call_ordinal: 3,
                    target_symbol,
                },
                ..AbstractPermissionEvent::default()
            });

        let machine_program = build_machine_program(&target_operations).expect("machine program");

        assert_eq!(machine_program.semantics.ownership.permissions.len(), 1);
        let event = machine_program
            .semantics
            .ownership
            .permissions
            .iter()
            .next()
            .map(|(_, event)| event)
            .expect("machine-program ownership event");
        assert_eq!(
            event.source,
            psi_language_semantics::PermissionEventSource::Call {
                statement_index: 22,
                call_ordinal: 3,
                target_symbol,
            }
        );
    }

    #[test]
    fn preserves_target_boundary_policy_checks_into_machine_program() {
        let mut target_operations = InstructionPlan::default();
        target_operations
            .semantics
            .boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: "omega::host::targets::linux".into(),
                verdict: AbstractBoundaryPolicyVerdict::DisallowedBoundaryPolicy,
                ..Default::default()
            });

        let machine_program = build_machine_program(&target_operations).expect("machine program");

        let check = machine_program
            .semantics
            .boundaries
            .policy_checks
            .iter()
            .next()
            .map(|(_, check)| check)
            .expect("machine-program boundary policy check");
        assert_eq!(machine_program.semantics.boundaries.policy_checks.len(), 1);
        assert_eq!(
            check.verdict,
            AbstractBoundaryPolicyVerdict::DisallowedBoundaryPolicy
        );
        assert_eq!(
            check.boundary_policy.as_ref(),
            "omega::host::targets::linux"
        );
    }
}
