//! Collecting, retaining and materializing dynamic realizations per lane.

use crate::unit::dynamic_composed_unit::applications::{
    exact_empty_machine_service_ceiling, exact_machine_service_summary, terminal_callable_result,
    validate_empty_contract, validate_empty_service_summary,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    DynamicLoweringLane, LoweredDynamicRealization,
};
use crate::unit::dynamic_composed_unit::store_operations::{
    empty_terminal_contract, lower_realization_operations,
};
use crate::unit::dynamic_composed_unit::structural_types::terminal_projected_source_multiplicity_for;
use crate::unit::{
    CheckedTrees, LoweringError, allocate_dense, block_id, edge_id, evidence_lowering,
    lower_installation_machine_service_ceiling, machine_id, place_id, terminal_scalar_type,
    unsupported, value_id,
};
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedDynamicRealizationBodyPlan, CheckedDynamicRealizationCallablePlan,
    CheckedDynamicScalarCallPlan, CheckedDynamicUnitCallPlan, CheckedStructuralAccess,
    DynamicConformanceBindingFact,
};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Block, ClosedConformanceCallableResult, StructuralAccess, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, TerminalMachine, TerminalMachineResult, Terminator,
    ValueDeclaration,
};

/// Borrowed table inputs shared by scalar and Unit call sites. Invocation/result
/// binding stays with the caller; table members retain their own result kinds.
pub(crate) struct DynamicCallableTable<'a> {
    pub(crate) selection: &'a DynamicConformanceBindingFact,
    pub(crate) source_type_identity: &'a str,
    pub(crate) selected_conformance: symbols::SymbolHandle,
    pub(crate) target_trait: symbols::SymbolHandle,
    pub(crate) declaring_trait: symbols::SymbolHandle,
    pub(crate) requirement: symbols::SymbolHandle,
    pub(crate) requirement_identity: &'a str,
    pub(crate) family_tuple: &'a [String],
    pub(crate) realization_machine: symbols::SymbolHandle,
    pub(crate) realization_state: symbols::SymbolHandle,
    pub(crate) realization_callables: &'a [CheckedDynamicRealizationCallablePlan],
    pub(crate) caller_multiplicity: language_semantics::Multiplicity,
    pub(crate) source_access: CheckedStructuralAccess,
    pub(crate) forwarded: bool,
    pub(crate) checked_call_service_reach: language_semantics::ServiceReachSummary,
}

impl<'a> From<&'a CheckedDynamicScalarCallPlan> for DynamicCallableTable<'a> {
    fn from(plan: &'a CheckedDynamicScalarCallPlan) -> Self {
        Self {
            selection: &plan.selection,
            source_type_identity: &plan.source_type_identity,
            selected_conformance: plan.selected_conformance,
            target_trait: plan.target_trait,
            declaring_trait: plan.declaring_trait,
            requirement: plan.requirement,
            requirement_identity: &plan.requirement_identity,
            family_tuple: &plan.family_tuple,
            realization_machine: plan.realization_machine,
            realization_state: plan.realization_state,
            realization_callables: &plan.realization_callables,
            caller_multiplicity: plan.caller_multiplicity,
            source_access: plan.source_access,
            forwarded: matches!(
                plan.origin,
                checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
            ),
            checked_call_service_reach: plan.checked_call_service_reach,
        }
    }
}

impl<'a> From<&'a CheckedDynamicUnitCallPlan> for DynamicCallableTable<'a> {
    fn from(plan: &'a CheckedDynamicUnitCallPlan) -> Self {
        Self {
            selection: &plan.selection,
            source_type_identity: &plan.source_type_identity,
            selected_conformance: plan.selected_conformance,
            target_trait: plan.target_trait,
            declaring_trait: plan.declaring_trait,
            requirement: plan.requirement,
            requirement_identity: &plan.requirement_identity,
            family_tuple: &plan.family_tuple,
            realization_machine: plan.realization_machine,
            realization_state: plan.realization_state,
            realization_callables: &plan.realization_callables,
            caller_multiplicity: plan.caller_multiplicity,
            source_access: plan.source_access,
            forwarded: matches!(
                plan.origin,
                checked_trees::CheckedDynamicUnitCallOrigin::Forwarded { .. }
            ),
            checked_call_service_reach: plan.checked_call_service_reach,
        }
    }
}

pub(crate) fn collect_dynamic_realizations(
    checked: &CheckedTrees,
    plan: &DynamicCallableTable<'_>,
    first_machine: u64,
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    if plan.realization_callables.is_empty() {
        return unsupported("dynamic conformance has no checked realization callables");
    }
    plan.realization_callables
        .iter()
        .enumerate()
        .map(|(ordinal, callable)| {
            let ordinal = u64::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("dynamic realization ordinal exceeds u64")
            })?;
            let checked_identity = evidence_lowering::checked_dynamic_machine_identity(
                checked,
                callable.realization_machine,
            )?;
            if checked_identity != callable.realization_identity {
                return unsupported("dynamic realization callable identity drifted");
            }
            let callable_identity = evidence_lowering::checked_evidence_machine_identity(
                checked,
                callable.realization_machine,
            )?;
            let result = match &callable.body {
                CheckedDynamicRealizationBodyPlan::Unit => ClosedConformanceCallableResult::Unit,
                CheckedDynamicRealizationBodyPlan::Scalar { result_type, .. } => {
                    terminal_callable_result(*result_type)?
                }
            };
            validate_callable_result(checked, callable)?;
            let machine = machine_id(ordinal.checked_add(first_machine).ok_or(
                LoweringError::Unsupported("dynamic realization machine identity overflowed"),
            )?);
            Ok(LoweredDynamicRealization {
                source_machine: callable.realization_machine,
                source_state: callable.realization_state,
                checked_identity,
                callable_identity,
                machine,
                result,
            })
        })
        .collect()
}

pub(crate) fn retain_realizations_for_lane(
    all: &[LoweredDynamicRealization],
    plan: &DynamicCallableTable<'_>,
    lane: DynamicLoweringLane<'_>,
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    // Rebound descriptors and forwarded descriptor parameters both expose the
    // complete table: every expanded family row must bind a callable, so the
    // full roster is retained. A strictly local direct dispatch names its
    // selected callable outright; its application keeps the unselected family
    // rows as evidence without materializing their instances.
    let retains_full_roster = matches!(lane, DynamicLoweringLane::Rebound(_)) || plan.forwarded;
    let retained = all
        .iter()
        .filter(|candidate| {
            retains_full_roster
                || (candidate.source_machine == plan.realization_machine
                    && candidate.source_state == plan.realization_state)
        })
        .cloned()
        .collect::<Vec<_>>();
    if retained.is_empty() {
        return unsupported("dynamic selected realization callable is absent");
    }
    Ok(retained)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn materialize_dynamic_realizations(
    checked: &CheckedTrees,
    plan: &DynamicCallableTable<'_>,
    lowered: &[LoweredDynamicRealization],
    source_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_block: &mut u64,
    next_place: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<TerminalMachine>, LoweringError> {
    lowered
        .iter()
        .map(|realization| {
            let matching = plan
                .realization_callables
                .iter()
                .filter(|candidate| {
                    candidate.realization_machine == realization.source_machine
                        && candidate.realization_state == realization.source_state
                        && candidate.realization_identity == realization.checked_identity
                })
                .collect::<Vec<_>>();
            let [callable] = matching.as_slice() else {
                return unsupported("dynamic realization checked body is absent or ambiguous");
            };
            validate_empty_contract(
                checked,
                callable.realization_machine,
                callable.contract_report_fingerprint,
                callable.contract_commitment,
            )?;
            let published_service_ceiling = if callable.realization_machine
                == plan.realization_machine
                && callable.realization_state == plan.realization_state
            {
                exact_empty_machine_service_ceiling(
                    checked,
                    callable.realization_machine,
                    plan.checked_call_service_reach,
                )?
            } else {
                let summary = exact_machine_service_summary(checked, callable.realization_machine)?;
                validate_empty_service_summary(checked, summary)?;
                let contract = checked
                    .facts
                    .service_reaches
                    .plan_for_machine(callable.realization_machine)
                    .ok_or(LoweringError::Unsupported(
                        "dynamic realization has no service contract",
                    ))?;
                lower_installation_machine_service_ceiling(
                    checked,
                    callable.realization_machine,
                    contract,
                    summary,
                    &[],
                )?
            };
            let block = block_id(allocate_dense(next_block)?);
            let place = place_id(allocate_dense(next_place)?);
            let edge = edge_id(allocate_dense(next_edge)?);
            let parameter = StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: true,
                structural_type: source_type,
                multiplicity: terminal_projected_source_multiplicity_for(plan.caller_multiplicity),
                access: match plan.source_access {
                    CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                    CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
                    _ => return unsupported("dynamic realization requires a borrowed source"),
                },
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            let (operations, result, terminator) = match &callable.body {
                CheckedDynamicRealizationBodyPlan::Unit => (
                    Vec::new(),
                    TerminalMachineResult::Unit,
                    Terminator::ReturnUnit {
                        edge,
                        trivial_affine_discards: Vec::new(),
                    },
                ),
                CheckedDynamicRealizationBodyPlan::Scalar {
                    result_type,
                    structural_scalar_field_stores,
                    return_expression,
                } => {
                    let scalar_type = terminal_scalar_type(*result_type)?;
                    let operations = lower_realization_operations(
                        structural_scalar_field_stores,
                        return_expression,
                        scalar_type,
                        &parameter,
                        structural_types,
                        next_operation,
                        next_value,
                    )?;
                    let returned = operations
                        .last()
                        .and_then(|operation| operation.result.scalar())
                        .map(|value| value.id)
                        .ok_or(LoweringError::Unsupported(
                            "dynamic realization did not emit one scalar result",
                        ))?;
                    let result_value = value_id(allocate_dense(next_value)?);
                    (
                        operations,
                        TerminalMachineResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: result_value,
                            scalar_type,
                        }),
                        Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge,
                            value: returned,
                        },
                    )
                }
            };
            Ok(TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: realization.machine,
                attachment: Some(source_type),
                parameters: Vec::new(),
                structural_parameters: vec![parameter.clone()],
                ranked_scc: None,
                result,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling,
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    structural_parameters: Vec::new(),
                    id: block,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    operations,
                    terminator,
                }],
                contract: empty_terminal_contract(realization.machine.get()),
            })
        })
        .collect()
}

fn validate_callable_result(
    checked: &CheckedTrees,
    callable: &CheckedDynamicRealizationCallablePlan,
) -> Result<(), LoweringError> {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == callable.realization_machine)
        .ok_or(LoweringError::Unsupported(
            "dynamic callable machine is absent",
        ))?;
    let state = checked
        .typed
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == callable.realization_state)
        .ok_or(LoweringError::Unsupported(
            "dynamic callable state is absent",
        ))?;
    let mut result_type = state.return_type;
    while let TypeReferenceNode::Constrained { base_type, .. } = checked
        .typed
        .type_reference_table
        .type_reference(result_type)
    {
        result_type = *base_type;
    }
    let matches = match &callable.body {
        CheckedDynamicRealizationBodyPlan::Unit => matches!(
            checked
                .typed
                .type_reference_table
                .type_reference(result_type),
            TypeReferenceNode::Unit
        ),
        CheckedDynamicRealizationBodyPlan::Scalar {
            result_type: primitive,
            ..
        } => checked.typed.primitive_type_reference(result_type) == Some(*primitive),
    };
    if !matches {
        return unsupported("dynamic callable result kind drifted from its checked signature");
    }
    Ok(())
}
