//! Selected boundary-operator structural-scalar return lowering.

use super::*;

pub(crate) fn lower_selected_operator_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedSelectedOperatorStructuralScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let realizations = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .filter(|candidate| {
            candidate.machine == plan.realization_machine
                && candidate.state == plan.realization_state
        })
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return unsupported(
            "selected structural operator does not rejoin one checked scalar realization",
        );
    };
    let origin = checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: plan.machine,
        state_symbol: plan.state,
        statement_index: usize::try_from(plan.return_statement_ordinal).map_err(|_| {
            LoweringError::Unsupported(
                "selected structural operator statement coordinate exceeds usize",
            )
        })?,
        role: checked_trees::CheckedValueStatementRole::Expression,
    };
    let exact_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .filter(|(_, operator_use)| {
            operator_use.origin == origin
                && operator_use.selected_operator_symbol == plan.requirement_operator
                && operator_use.provider_plan_report_fingerprint
                    == plan.provider_plan_report_fingerprint
                && operator_use.provider_plan_commitment == plan.provider_plan_commitment
        })
        .count()
        + checked
            .facts
            .operators
            .uses
            .iter()
            .filter(|(_, operator_use)| {
                operator_use.origin == origin
                    && operator_use.selected_operator_symbol == plan.requirement_operator
                    && operator_use.provider_plan_report_fingerprint
                        == plan.provider_plan_report_fingerprint
                    && operator_use.provider_plan_commitment == plan.provider_plan_commitment
            })
            .count();
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.realization_machine)
        .ok_or(LoweringError::Unsupported(
            "selected structural operator realization has no checked contract",
        ))?;
    let realization_reaches = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == plan.realization_machine
                && state.state_symbol == plan.realization_state
        })
        .map(|(_, state)| state.service_reach)
        .collect::<Vec<_>>();
    if exact_uses != 1
        || plan.provider_plan_report_fingerprint == 0
        || plan.provider_plan_commitment.is_empty()
        || contract.report_fingerprint != plan.realization_contract_report_fingerprint
        || contract.commitment != plan.realization_contract_commitment
        || realization_reaches.as_slice() != [plan.service_reach]
        || realization.result_type != plan.result_type
        || !realization.scalar_parameters.is_empty()
    {
        return unsupported(
            "selected structural operator drifted from its authored use, plan, realization, contract, result, or reach",
        );
    }

    let mut lowered = lower_structural_scalar_return_machine_in_namespace(
        checked,
        realization,
        machine_id(2),
        TERMINAL_MACHINE_IDENTITY_STRIDE,
        None,
    )?;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let mut positions = BTreeSet::new();
    for parameter in &plan.structural_parameters {
        if parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !positions.insert(parameter.position)
        {
            return unsupported(
                "selected structural operator caller is not claim-free affine custody",
            );
        }
        lookup_type_id(&type_ids, &parameter.type_identity)?;
    }
    if positions.len() != plan.structural_parameters.len()
        || positions
            .iter()
            .copied()
            .enumerate()
            .any(|(index, position)| u32::try_from(index).ok() != Some(position))
        || plan.argument_source_positions.len() != plan.structural_parameters.len()
        || plan
            .argument_source_positions
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != positions
    {
        return unsupported(
            "selected structural operator operands are not an exact parameter permutation",
        );
    }
    let result_type = terminal_scalar_type(plan.result_type)?;
    let mut next_place = 1_u64;
    let structural_parameters =
        lower_unit_parameters(&plan.structural_parameters, &type_ids, &[], &mut next_place)?;
    let callee = lowered
        .semantic_module
        .machines
        .first()
        .ok_or(LoweringError::Unsupported(
            "selected structural operator realization produced no terminal machine",
        ))?;
    if callee.id != machine_id(2)
        || !callee.parameters.is_empty()
        || callee.structural_parameters.len() != structural_parameters.len()
        || callee.result.scalar().map(|result| result.scalar_type) != Some(result_type)
        || !callee.contract.requires.is_empty()
        || !callee.contract.crash_routes.is_empty()
    {
        return unsupported(
            "selected structural operator realization has an incompatible terminal signature or contract",
        );
    }
    let structural_arguments = plan
        .argument_source_positions
        .iter()
        .zip(&callee.structural_parameters)
        .map(|(source_position, target)| {
            let source = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position == *source_position)
                .and_then(|index| structural_parameters.get(index))
                .ok_or(LoweringError::Unsupported(
                    "selected structural operator source operand is absent",
                ))?;
            if source.structural_type != target.structural_type
                || source.multiplicity != target.multiplicity
                || source.access != target.access
                || source.qualifications != target.qualifications
            {
                return Err(LoweringError::Unsupported(
                    "selected structural operator operand disagrees with its realization parameter",
                ));
            }
            Ok(StructuralArgument {
                place: source.place,
                path: Vec::new(),
                access: target.access,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let call_result = ValueDeclaration {
        id: value_id(1),
        scalar_type: result_type,
    };
    let machine_result = ValueDeclaration {
        id: value_id(2),
        scalar_type: result_type,
    };
    let call_operation = operation_id(1);
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: structural_parameters.clone(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(machine_result),
        structural_places: structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: call_operation,
                result: terminal_psi::OperationResult::Scalar(call_result),
                kind: OperationKind::CallStructuralScalar {
                    callee: machine_id(2),
                    arguments: Vec::new(),
                    structural_arguments,
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(1),
                value: call_result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    lowered
        .source_call_occurrences
        .push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: plan.state,
            statement_index: usize::try_from(plan.return_statement_ordinal).map_err(|_| {
                LoweringError::Unsupported(
                    "selected structural operator statement coordinate exceeds usize",
                )
            })?,
            call_ordinal: 0,
            terminal_operation: call_operation,
            source_target: plan.realization_machine,
            source_values_before_call: Vec::new(),
        });
    lowered.semantic_module.entry = caller.id;
    lowered.semantic_module.machines.insert(0, caller);
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}
