//! The partial affine unit cleanup machine and its residuals.

use crate::execution::terminal_unit::cleanup::anonymous;

use crate::execution::terminal_unit::calls::{
    ExpectedCallValueResult, build_call_operation, entry_claims,
    partial_affine_structural_signature,
};
use crate::execution::terminal_unit::cleanup::cleanup_evidence::{
    has_exact_symbol_affine_discard, machine_has_content_evidence, service_reach_is_empty,
    service_reach_plan_is_empty,
};
use crate::execution::terminal_unit::cleanup::residuals;
use crate::execution::terminal_unit::control::checked_unit_structural_result_local;
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, machine_binders, parameter_root_symbol, state_flow,
    type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    BTreeMap, CheckFacts, CheckedPartialAffineUnitCleanupMachinePlan, CheckedStructuralAccess,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, PrimitiveType, ScalarCalleePlans,
    SignatureContractKind, StatementNode, TypedTrees, control,
};

pub(crate) fn build_partial_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedPartialAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    // The general route: an ordinary body the roster declined because its
    // return edge owes the residual complement of one partially moved owned
    // parameter. The carrier exists for exactly this path-sensitive return
    // cleanup, so republish the machine here when the residual rebuild
    // confirms it. Scalar-callee, selected-operator and call-frame inputs
    // are not retained at this stage; a body needing them keeps its earlier
    // admission failure rather than silently dropping custody.
    if unit_effects.for_machine(machine.symbol).is_none()
        && facts.flow.ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.kind == PermissionEventKind::Transfer
                && !event.segments.is_empty()
        })
        && let Some((plan, residual_affine_discards, has_projected_parameter_moves)) =
            control::build_checked_machine_residual_parts(
                program,
                facts,
                ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns,
                },
                shapes,
                machine,
                &[],
                &[],
                false,
                None,
                &control::LocalConstructionTrace::default(),
            )
        && has_projected_parameter_moves
    {
        return Some(CheckedPartialAffineUnitCleanupMachinePlan {
            machine: plan,
            residual_affine_discards,
        });
    }
    // The carrier is disjoint from the ordinary roster: a body the roster
    // admitted — including a fully consumed root whose projections left no
    // residual — keeps its ordinary plan and needs no cleanup wrapper.
    if unit_effects.for_machine(machine.symbol).is_some() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let result_local = matches!(statements.first(), Some(StatementNode::LocalData(_)));
    if statements.is_empty()
        || statements
            .iter()
            .skip(usize::from(result_local))
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let runtime_arithmetic_requires_are_terminal =
        contract.crash.uses_structural_proof_gated_arithmetic()
            && contract.crash.structural_runtime_requirements().is_some();
    if !is_unit(program, state.return_type)
        || program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .any(|contract| match contract.kind {
                SignatureContractKind::Crashes { .. } => false,
                SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => {
                    false
                }
                SignatureContractKind::Requires
                | SignatureContractKind::Ensures
                | SignatureContractKind::EnsuresForResultCase { .. } => true,
            })
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        partial_affine_structural_signature(program, shapes, machine, state, &binders)?;
    let anonymous_binding = anonymous::binding(program, facts, shapes, machine, state);
    let result_root = result_local || anonymous_binding.is_some();
    if structural_parameters.len() > 1
        || program.state_parameters(state).len() != structural_parameters.len()
        || (!result_root && structural_parameters.len() != 1)
        || program
            .state_parameters(state)
            .iter()
            .zip(&structural_parameters)
            .any(|(source, checked)| {
                source.is_self
                    || checked.is_self
                    || checked.position != 0
                    || checked.multiplicity != Multiplicity::Affine
                    || checked.access != CheckedStructuralAccess::Owned
                    || !checked.qualifications.is_empty()
                    || type_graph_requires_nominal_drop(program, source.type_reference)
            })
    {
        return None;
    }
    let result_binding = if result_local {
        let (result, symbol) =
            checked_unit_structural_result_local(program, shapes, statements, &binders)?;
        let StatementNode::LocalData(local) = &statements[0] else {
            unreachable!()
        };
        if result.multiplicity != Multiplicity::Affine
            || shapes.add_partial_affine_type(local.type_reference, &binders)?
                != result.type_identity
            || !symbol.is_valid()
        {
            return None;
        }
        Some((result, facts::PlaceRoot::Symbol(symbol)))
    } else {
        anonymous_binding
    };
    let (root_source, root, root_type) = if let Some((result, root)) = &result_binding {
        (
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            *root,
            result.type_identity.clone(),
        )
    } else {
        (
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
            facts::PlaceRoot::Symbol(parameter_root_symbol(
                machine.symbol,
                &program.state_parameters(state)[0],
            )),
            structural_parameters[0].type_identity.clone(),
        )
    };
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    if !entry_claims.is_empty() {
        return None;
    }
    if facts
        .qualifications
        .for_machine(machine.symbol)
        .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }

    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    let anonymous_result = matches!(root, facts::PlaceRoot::Expression(_));
    if !anonymous_result
        && (calls.len() != statements.len()
            || calls.iter().enumerate().any(|(statement_index, call)| {
                call.statement_index != statement_index || call.call_ordinal != 0
            }))
    {
        return None;
    }
    if !result_root && !service_reach_is_empty(facts, state_flow.service_reach) {
        return None;
    }
    let mut operations = Vec::with_capacity(calls.len().saturating_add(1));
    let mut moved_paths =
        Vec::<(Vec<CheckedUnitStructuralPathSegment>, String)>::with_capacity(calls.len());
    let ordered_calls = if anonymous_result {
        vec![
            calls.iter().find(|call| call.call_ordinal == 1)?,
            calls.iter().find(|call| call.call_ordinal == 0)?,
        ]
    } else {
        calls.iter().collect()
    };
    for call in ordered_calls {
        if (result_local && call.statement_index == 0)
            || (anonymous_result && call.call_ordinal == 1)
        {
            let (result, _) = result_binding.as_ref()?;
            let expression = match (&statements[0], root) {
                (StatementNode::LocalData(local), _) => local.initial_value,
                (_, facts::PlaceRoot::Expression(expression)) => expression,
                _ => return None,
            };
            if call.authored_expression != expression {
                return None;
            }
            let operation = build_call_operation(
                program,
                facts,
                None,
                machine,
                state,
                &structural_parameters,
                &[],
                &entry_claims,
                call,
                false,
                Some(ExpectedCallValueResult::Structural(result)),
                &[],
                &control::LocalConstructionTrace::default(),
            )?;
            let mut operation = control::bind_structural_call_result(operation, result.clone())?;
            match &mut operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    structural_arguments,
                    scalar_arguments,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    structural_arguments,
                    scalar_arguments,
                    discard_result_on_return,
                    ..
                } => {
                    if !scalar_arguments.is_empty()
                        || structural_arguments.len() != structural_parameters.len()
                        || structural_arguments.iter().any(|argument| {
                            argument.source_parameter_index() != Some(0)
                                || !argument.path.is_empty()
                                || argument.access != CheckedStructuralAccess::Owned
                        })
                    {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
                _ => return None,
            }
            operations.push(operation);
            continue;
        }
        if !service_reach_is_empty(facts, call.service_reach) {
            return None;
        }
        let operation = build_call_operation(
            program,
            facts,
            None,
            machine,
            state,
            &structural_parameters,
            &[],
            &entry_claims,
            call,
            true,
            None,
            result_binding.as_slice(),
            &control::LocalConstructionTrace::default(),
        )?;
        let CheckedUnitEffectOperationPlan::CallUnit {
            target_machine,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation
        else {
            return None;
        };
        let [argument] = structural_arguments.as_slice() else {
            return None;
        };
        if !is_partial_affine_path(&argument.path) {
            return None;
        }
        if argument.source != root_source
            || argument.access != CheckedStructuralAccess::Owned
            || !claim_transfers.is_empty()
            || moved_paths.iter().any(|(earlier, _)| {
                earlier.starts_with(&argument.path) || argument.path.starts_with(earlier)
            })
        {
            return None;
        }

        let target = unit_effects.for_machine(*target_machine)?;
        let [target_parameter] = target.structural_parameters.as_slice() else {
            return None;
        };
        if target_parameter.type_identity != argument.type_identity
            || target_parameter.access != CheckedStructuralAccess::Owned
            || !target.scalar_parameters.is_empty()
            || target_parameter.is_self
            || target_parameter.multiplicity != Multiplicity::Affine
            || !target_parameter.qualifications.is_empty()
            || !target.entry_claims.is_empty()
            || !target.trivial_affine_locals.is_empty()
            || !target.body_qualifications.is_empty()
            || !service_reach_is_empty(facts, target.service_reach)
            || !service_reach_plan_is_empty(facts, target.contract_service_reach)
            || !matches!(
                target.operations.as_slice(),
                [CheckedUnitEffectOperationPlan::Complete {
                    trivial_affine_local_discard_ordinals,
                    trivial_affine_discards,
                    ..
                }] if trivial_affine_local_discard_ordinals.is_empty()
                    && trivial_affine_discards.as_slice() == [0]
            )
        {
            return None;
        }
        moved_paths.push((argument.path.clone(), argument.type_identity.clone()));
        operations.push(operation);
    }

    // Indexed cleanup keeps the existing contract-free source boundary,
    // including arrays reached through record fields.
    if moved_paths.iter().any(|(path, _)| {
        path.iter()
            .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))
    }) && (!program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || operations.iter().any(|operation| {
            let CheckedUnitEffectOperationPlan::CallUnit {
                target_machine,
                target_state,
                ..
            } = operation
            else {
                return !matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                );
            };
            !crate::lookup::machine_by_symbol(program, *target_machine)
                .is_some_and(|target| program.machine_contracts(target).is_empty())
                || !crate::semantic::calls::find_state(program, *target_state)
                    .is_some_and(|target| program.state_contracts(target).is_empty())
        }))
    {
        return None;
    }
    let residual_affine_discards =
        partial_affine_residuals(&shapes.types, &root_source, &root_type, &moved_paths)?;
    let provenance = if result_local {
        let provenance = language_semantics::PermissionProvenance::Established {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::Statement { statement_index: 0 },
        };
        let mut establishments = facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::Statement { statement_index: 0 }
                    && event.kind == PermissionEventKind::Establish
                    && event.root == root
            });
        let establishment = establishments.next()?;
        if establishments.next().is_some()
            || establishment.provenance != provenance
            || establishment.access != PermissionAccess::Owned
            || establishment.multiplicity != Multiplicity::Affine
            || establishment.claim_identity != PermissionClaimIdentity::Unknown
            || establishment.obligation_live
            || !facts
                .flow
                .ownership
                .segments
                .span_or_empty(establishment.segments)
                .is_empty()
        {
            return None;
        }
        provenance
    } else {
        language_semantics::PermissionProvenance::Unknown
    };
    match root {
        facts::PlaceRoot::Symbol(symbol) => {
            if !has_exact_symbol_affine_discard(facts, machine, state, symbol, provenance) {
                return None;
            }
        }
        facts::PlaceRoot::Expression(_) => {
            anonymous::validate_permissions(
                program,
                facts,
                machine,
                state,
                root,
                &residual_affine_discards,
            )?;
        }
        facts::PlaceRoot::Unknown | facts::PlaceRoot::TypeReference(_) => return None,
    }
    if !result_root
        && !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
    {
        return None;
    }
    operations.push(CheckedUnitEffectOperationPlan::Complete {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    });
    let erased_scalar_parameters =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, state)?;
    let erased_proof_parameters =
        crate::execution::terminal_unit::types::erased_proof_parameter_plans(program, state)?;
    Some(CheckedPartialAffineUnitCleanupMachinePlan {
        machine: CheckedUnitEffectMachinePlan {
            scalar_result: None,
            scalar_control: None,
            structural_result: None,
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity,
            structural_parameters,
            scalar_parameters: Vec::new(),
            erased_scalar_parameters,
            erased_proof_parameters,
            provider_attachment_requirements: Vec::new(),
            trivial_affine_locals: Vec::new(),
            entry_claims,
            body_qualifications: Vec::new(),
            contract_report_fingerprint: contract.report_fingerprint,
            contract_commitment: contract.commitment,
            contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
            service_reach: state_flow.service_reach,
            operations,
        },
        residual_affine_discards,
    })
}

pub(crate) fn partial_affine_residuals(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(Vec<CheckedUnitStructuralPathSegment>, String)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = moved_paths
        .iter()
        .map(|(path, moved_type)| (path.as_slice(), moved_type.as_str()))
        .collect::<Vec<_>>();
    residuals::reconstruct(types, source, root_type, &borrowed, usize::MAX)
}

fn is_partial_affine_path(path: &[CheckedUnitStructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::Field(_)
                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
            )
        })
}

pub(crate) fn is_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
    // Numeric restrictions remain on the retained field type for arithmetic
    // proofs, but do not introduce custody or executable cleanup. A bounded
    // integer is the same no-cleanup leaf as its raw scalar carrier when
    // reconstructing the untouched complement of a partial move.
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::BoundedInteger(_)
            | CheckedUnitStructuralFieldType::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
            )
            | CheckedUnitStructuralFieldType::Scalar(
                PrimitiveType::Bool
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Addr
                    | PrimitiveType::F32
                    | PrimitiveType::F64
            )
    )
}
