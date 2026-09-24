//! One ordinary attached Unit machine lowered from its checked effect plan:
//! `emit` establishes the provider attachment, local and literal places,
//! walks the argument schedule through `MachineEmission::lower_step`, then
//! seals completion, the cleanup roster, crash routes and the machine's
//! single block.
//!
//! `lower_step` sends stores, scalar locals, borrowed-storage windows and
//! continuation cleanup to `operation_frame::OperationFrame`, the emitter
//! composed-graph states share, and keeps one method per remaining kind in
//! `locals` (structural values and ordinary-only establishments), `calls` and
//! `boundary_calls`.

use super::bodies::{UnitBody, UnitPlans};
use super::catalog::lower_provider_candidate_service_ceiling;
use super::composed_control::callable::EmissionCounters;
use super::operation_frame::{OperationFrame, StructuralResults, StructuralTypeRoster};
use super::parameters::lower_declared_service_reach;
use super::parameters::lower_installation_machine_service_ceiling;
use super::provider_attachments::lower_provider_attachment_places;
use super::provider_attachments::validate_provider_attachment_requirements;
use super::signatures::MachineSignature;
use super::{
    CheckedUnitProviderCandidate, argument_evaluation, argument_schedule, byte_subslices,
    parameters, primitive_locals,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::scalar_graph::scalar_call_closure::callee::PreparedScalarCallee;
use crate::unit::{
    Block, CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, ClaimId,
    ContractClause, LoweringError, MachineContract, Multiplicity, Operation, OperationKind,
    PermissionClaimIdentity, PlaceId, ScalarType, SemanticDomainId, StructuralDomainId,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration, allocate_dense,
    content_conservation, contract_id, dense_identity, edge_id, lookup_domain_id, lookup_type_id,
    lower_checked_crash_route_buckets, lower_structural_crash_route_buckets, obligation_id,
    place_id, terminal_scalar_type, unsupported, value_id,
};
use crate::unit::{
    BoundaryMachineId, MachineId, ServiceId, ServiceReachId, StructuralParameterDeclaration,
    StructuralTypeDeclaration,
};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
use lowered_psi::{
    LoweredSelectedIeeeFloatComparisonOccurrence, LoweredSelectedIeeeFloatFmaOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSourceCallOccurrence,
};

/// What the closure keeps for every ordinary machine emitter: the identity
/// rosters, boundary parameters, signatures and requirement counts, plus the
/// checked plans and closure roster that calls resolve against. The structural
/// type roster travels separately because a body may extend it.
#[derive(Clone, Copy)]
pub(super) struct ClosureCatalog<'a> {
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    pub(super) domain_ids: &'a [(SemanticDomainId, StructuralDomainId)],
    pub(super) service_ids: &'a [(ServiceReachId, ServiceId)],
    pub(super) boundary_parameters: &'a [(
        symbols::SymbolHandle,
        BoundaryMachineId,
        Vec<StructuralParameterDeclaration>,
        Vec<ScalarType>,
    )],
    pub(super) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub(super) signatures: &'a [MachineSignature],
    pub(super) scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    pub(super) plans: UnitPlans<'a>,
    pub(super) closure: &'a [symbols::SymbolHandle],
    pub(super) provider_candidate_plans: &'a [CheckedUnitProviderCandidate],
    pub(super) prepared_scalar_machines: &'a [PreparedScalarCallee<'a>],
}

mod boundary_calls;
mod calls;
mod locals;
mod projected_moves;

/// One ordinary machine's emission in flight: the shared catalog it resolves
/// against, the identity counters it advances, and the places, values and
/// staging it accumulates operation by operation. `lower_step` walks the
/// argument schedule; each operation kind has a method in the family files.
pub(super) struct MachineEmission<'a> {
    checked: &'a CheckedTrees,
    plan: &'a CheckedUnitEffectMachinePlan,
    plans: UnitPlans<'a>,
    parameters: &'a Vec<StructuralParameterDeclaration>,
    scalar_parameter_count: usize,
    /// The caller's claim bindings: the lowered entry claims plus every
    /// caller-local binding a boundary result mints for a claim established
    /// at its own binding statement. The table grows during emission so
    /// later receipts, transfers and result custody resolve the same
    /// semantic identity to the same `ClaimId`.
    claim_bindings: Vec<(PermissionClaimIdentity, ClaimId)>,
    /// Dense tail past the lowered entry claims; each minted boundary result
    /// claim allocates the next value so two claims never share an id.
    next_claim: u64,
    structural_types: &'a mut Vec<StructuralTypeDeclaration>,
    type_ids: &'a [(String, StructuralTypeId)],
    domain_ids: &'a [(SemanticDomainId, StructuralDomainId)],
    service_ids: &'a [(ServiceReachId, ServiceId)],
    lowered_boundary_parameters: &'a [(
        symbols::SymbolHandle,
        BoundaryMachineId,
        Vec<StructuralParameterDeclaration>,
        Vec<ScalarType>,
    )],
    machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    machine_signatures: &'a [MachineSignature],
    scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    closure: &'a [symbols::SymbolHandle],
    prepared_scalar_machines: &'a [PreparedScalarCallee<'a>],
    next_place: u64,
    next_value_identity: u64,
    next_block: u64,
    next_edge: u64,
    next_call_obligation: u64,
    operations: OperationBuffer,
    evaluation: argument_evaluation::Evaluation,
    /// Open borrowed-storage repair windows on the current emission path.
    /// Empty for bodies without move-out/restore statements.
    borrowed_windows: crate::emission::borrowed_window::BorrowedWindowLedger,
    scalar_result_values: Vec<ValueDeclaration>,
    primitive_local_places: Vec<primitive_locals::PrimitiveLocal>,
    structural_result_places: Vec<(StructuralPlaceDeclaration, bool)>,
    structural_value_temporaries: Vec<StructuralPlaceDeclaration>,
    local_places: Vec<StructuralPlaceDeclaration>,
    literal_places: Vec<StructuralPlaceDeclaration>,
    next_literal_argument: usize,
    staged_arguments: Vec<Vec<usize>>,
    staged_subslices: Vec<Vec<(usize, PlaceId)>>,
    subslice_places: Vec<StructuralPlaceDeclaration>,
    retained_scalar_prefix: Option<usize>,
    staged_scalar_result: Option<ValueDeclaration>,
    scalar_calls: CallEmissionContext<'a>,
    source_call: Option<(
        checked_trees::CheckedUnitCallCoordinate,
        Option<checked_trees::NominalMachineUseSite>,
        symbols::SymbolHandle,
    )>,
}

/// What one scheduled operation hands the lowering methods.
pub(super) struct StepInputs {
    pub(super) operation_index: usize,
    pub(super) staged: bool,
    pub(super) source_value_count: usize,
    pub(super) evaluated_scalar_arguments: Option<Vec<ValueDeclaration>>,
}

/// The lowered machine and the occurrence rows the closure accumulates.
pub(super) struct EmittedMachine {
    pub(super) machine: TerminalMachine,
    pub(super) source_calls: Vec<LoweredSourceCallOccurrence>,
    pub(super) selected_ieee_float_fmas: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    pub(super) selected_ieee_float_comparisons: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(super) selected_integer_comparisons: Vec<LoweredSelectedIntegerComparisonOccurrence>,
}

/// Emit one ordinary machine. Counters advance only when emission succeeds;
/// a failed emission fails the whole closure.
pub(super) fn emit(
    checked: &CheckedTrees,
    signature: &MachineSignature,
    plan: &CheckedUnitEffectMachinePlan,
    terminal_machine: MachineId,
    structural_types: &mut Vec<StructuralTypeDeclaration>,
    catalog: ClosureCatalog<'_>,
    counters: EmissionCounters<'_>,
) -> Result<EmittedMachine, LoweringError> {
    let ClosureCatalog {
        type_ids,
        domain_ids,
        service_ids,
        boundary_parameters: lowered_boundary_parameters,
        machine_ids,
        signatures: machine_signatures,
        scalar_requirement_counts,
        plans,
        closure,
        provider_candidate_plans,
        prepared_scalar_machines,
    } = catalog;
    let EmissionCounters {
        place: place_counter,
        value: value_counter,
        block: block_counter,
        operation: operation_counter,
        edge: edge_counter,
        call_obligation: call_obligation_counter,
    } = counters;
    let mut next_place = *place_counter;
    let mut next_value = *value_counter;
    let mut next_block = *block_counter;
    let mut next_operation = *operation_counter;
    let next_edge = *edge_counter;
    let next_call_obligation = *call_obligation_counter;
    let parameters = &signature.parameters;
    let scalar_parameters = signature.scalar_parameters.clone();
    let scalar_parameter_count = scalar_parameters.len();
    let runtime_requirements = &signature.runtime_requirements;
    let entry_claims = &signature.claims.entry_claims;
    let claim_bindings = &signature.claims.source_claims;
    let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
        checked,
        structural_types,
        &plan.structural_parameters,
        parameters,
        &plan.entry_claims,
        claim_bindings,
    )?;
    // Provider attachment requirements cover signature-directed boundary calls
    // only. A direct call to a boundary declaration (a top-level boundary
    // requirement, boundary, or admission-claim machine) settles through that
    // machine's own retained boundary seam: the operation carries
    // `target_machine != target_state`, while a signature call records the
    // signature symbol in both. Machine-directed calls consume no attached
    // provider field and must not count as provider obligations here.
    let called_boundaries = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                target_machine,
                target_state,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                target_machine,
                target_state,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                target_machine,
                target_state,
                ..
            } => (target_machine == target_state).then_some(*target_machine),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_boundaries = lowered_boundary_parameters
        .iter()
        .map(|(symbol, boundary, _, _)| (*symbol, *boundary))
        .collect::<Vec<_>>();
    let attachment = plan
        .attachment_type_identity
        .as_deref()
        .map(|identity| lookup_type_id(type_ids, identity))
        .transpose()?;
    let provider_places = if let Some(attachment) = attachment {
        let checked_attachment = plans
            .structural_types()
            .find(|declaration| {
                Some(declaration.identity.as_str()) == plan.attachment_type_identity.as_deref()
            })
            .ok_or(LoweringError::Unsupported(
                "attached Unit machine is missing its checked attachment shape",
            ))?;
        validate_provider_attachment_requirements(
            checked_attachment,
            &plan.provider_attachment_requirements,
            &called_boundaries,
        )?;
        let attachment_declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == attachment)
            .expect("lowered attachment declaration exists");
        lower_provider_attachment_places(
            attachment,
            attachment_declaration,
            &plan.provider_attachment_requirements,
            &provider_boundaries,
            &mut next_place,
        )?
    } else {
        if !plan.provider_attachment_requirements.is_empty() {
            return unsupported(
                "free Unit machine fabricates provider-backed attachment requirements",
            );
        }
        Vec::new()
    };
    let local_places = plan
        .trivial_affine_locals
        .iter()
        .map(|local| {
            Ok(StructuralPlaceDeclaration {
                id: place_id(allocate_dense(&mut next_place)?),
                kind: StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal: local.declaration_ordinal,
                    structural_type: lookup_type_id(type_ids, &local.type_identity)?,
                    construction: local
                        .construction
                        .as_ref()
                        .map(|element| {
                            Ok(semantic_vocabulary::AffineConstructionElement {
                                root_structural_type: lookup_type_id(
                                    type_ids,
                                    &element.root_type_identity,
                                )?,
                                index: element.index,
                            })
                        })
                        .transpose()?,
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut literal_arguments = Vec::new();
    for operation in &plan.operations {
        let (target_machine, structural_arguments) = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                target_machine,
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                target_machine,
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                target_machine,
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::CallUnit {
                target_machine,
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                target_machine,
                structural_arguments,
                ..
            } => (target_machine, structural_arguments),
            _ => continue,
        };
        literal_arguments.extend(
            structural_arguments
                .iter()
                .enumerate()
                .filter(|(_, argument)| argument.byte_sequence_literal().is_some())
                .map(|(index, argument)| (argument, *target_machine, index)),
        );
    }
    let mut literal_qualifications = Vec::new();
    let literal_places = literal_arguments
        .iter()
        .enumerate()
        .map(|(ordinal, (argument, target_machine, argument_index))| {
            // The literal occurrence replays the domain memberships checking
            // discharged on its bytes: the target parameter's declared
            // qualifications are exactly the admitted set.
            let qualifications = match lowered_boundary_parameters
                .iter()
                .find(|(symbol, ..)| *symbol == *target_machine)
            {
                Some((_, _, parameters, _)) => parameters
                    .get(*argument_index)
                    .ok_or(LoweringError::Unsupported(
                        "literal call boundary target parameter is absent",
                    ))?
                    .qualifications
                    .clone(),
                None => UnitBody::find(plans, *target_machine)?
                    .entry()?
                    .structural_parameters
                    .get(*argument_index)
                    .ok_or(LoweringError::Unsupported(
                        "literal call target parameter is absent",
                    ))?
                    .qualifications
                    .iter()
                    .map(|domain| lookup_domain_id(domain_ids, *domain))
                    .collect::<Result<Vec<_>, _>>()?,
            };
            literal_qualifications.push(qualifications);
            Ok(StructuralPlaceDeclaration {
                id: place_id(allocate_dense(&mut next_place)?),
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("byte-sequence literal count exceeds u32")
                    })?,
                    structural_type: lookup_type_id(type_ids, &argument.type_identity)?,
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let operation_identity_base = next_operation
        .checked_sub(1)
        .expect("terminal operation identity starts at one");
    let mut operations = OperationBuffer::new(operation_identity_base);
    for ((argument, _, _), (place, qualifications)) in literal_arguments
        .iter()
        .zip(literal_places.iter().zip(&literal_qualifications))
    {
        let bytes = argument
            .byte_sequence_literal()
            .ok_or(LoweringError::Unsupported(
                "byte-sequence literal payload is absent",
            ))?;
        let id = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: terminal_psi::OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place.id,
                bytes: bytes.to_vec(),
                qualifications: qualifications.clone(),
            },
        });
    }
    let next_literal_argument = 0usize;
    let call_literal_count = literal_places.len();
    let next_value_identity = next_value;
    let scalar_result_values = scalar_parameters.clone();
    let primitive_local_places = Vec::<primitive_locals::PrimitiveLocal>::new();
    let structural_result_places = Vec::<(StructuralPlaceDeclaration, bool)>::new();
    let structural_value_temporaries = Vec::<StructuralPlaceDeclaration>::new();
    let mut evaluation = argument_evaluation::Evaluation::new(&mut next_block)?;
    evaluation.erased_scalar_formals = signature.erased_scalar_parameters.clone();
    evaluation.record_fields = crate::scalar_graph::scalar_computations::fields::prepare(
        checked,
        plan.machine,
        structural_types,
    )?;
    evaluation.arrays = crate::scalar_graph::scalar_computations::arrays::prepare(
        checked,
        plan.machine,
        structural_types,
        &mut next_place,
    )?;
    evaluation.cases = crate::scalar_graph::scalar_computations::cases::prepare(
        checked,
        plan.machine,
        structural_types,
        &mut next_place,
    )?;
    evaluation.structural_parameters = plan
        .structural_parameters
        .iter()
        .zip(parameters)
        .map(|(source, parameter)| (source.position, parameter.clone()))
        .collect();
    evaluation.primitive_storage =
        crate::expression_preparation::source_custody::primitive_references::bindings(
            checked,
            plan.state,
            &evaluation.structural_parameters,
            structural_types,
        )?;
    let staged_arguments = vec![Vec::<usize>::new(); plan.operations.len()];
    evaluation.structural_fields =
        crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
            &evaluation.structural_parameters,
            structural_types,
        );
    evaluation.structural_cases =
        crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
            &evaluation.structural_parameters,
            structural_types,
        );
    evaluation.element_views = crate::expression_preparation::bindings::element_views(
        &evaluation.structural_parameters,
        structural_types,
    );
    let staged_subslices = vec![Vec::<(usize, PlaceId)>::new(); plan.operations.len()];
    let subslice_places = Vec::new();
    let retained_scalar_prefix = None;
    let staged_scalar_result = None;
    let mut emission = MachineEmission {
        checked,
        plan,
        plans,
        parameters,
        scalar_parameter_count,
        claim_bindings: claim_bindings.clone(),
        next_claim: dense_identity(claim_bindings.len())?,
        borrowed_windows: Default::default(),
        structural_types,
        type_ids,
        domain_ids,
        service_ids,
        lowered_boundary_parameters,
        machine_ids,
        machine_signatures,
        scalar_requirement_counts,
        closure,
        prepared_scalar_machines,
        next_place,
        next_value_identity,
        next_block,
        next_edge,
        next_call_obligation,
        operations,
        evaluation,
        scalar_result_values,
        primitive_local_places,
        structural_result_places,
        structural_value_temporaries,
        local_places,
        literal_places,
        next_literal_argument,
        staged_arguments,
        staged_subslices,
        subslice_places,
        retained_scalar_prefix,
        staged_scalar_result,
        scalar_calls: CallEmissionContext {
            machine_ids,
            requirement_counts: scalar_requirement_counts,
            next_obligation_identity: next_call_obligation,
            obligation_limit: u64::MAX,
        },
        source_call: None,
    };
    for step in argument_schedule::build(checked, plan)? {
        emission.lower_step(step)?;
    }
    let MachineEmission {
        structural_types,
        mut next_place,
        mut next_value_identity,
        mut next_block,
        mut next_edge,
        mut next_call_obligation,
        mut operations,
        mut evaluation,
        mut scalar_result_values,
        primitive_local_places,
        structural_result_places,
        structural_value_temporaries,
        local_places,
        literal_places,
        next_literal_argument,
        subslice_places,
        borrowed_windows,
        ..
    } = emission;
    borrowed_windows.require_closed()?;

    if next_literal_argument != call_literal_count {
        return unsupported("byte-sequence literal argument consumption is incomplete");
    }
    let controlled_result = if let Some(control) = &plan.scalar_control {
        // Prefix values and structural locals are already established. The
        // completion adds only the selected guard/arm evaluation and join.
        let mut scalar_calls = CallEmissionContext {
            machine_ids,
            requirement_counts: scalar_requirement_counts,
            next_obligation_identity: next_call_obligation,
            obligation_limit: u64::MAX,
        };
        let result = evaluation.scalar_control_result(
            checked,
            plan.machine,
            plan.state,
            control,
            &mut scalar_result_values,
            &mut next_value_identity,
            &mut next_block,
            &mut next_edge,
            &mut operations,
            &mut scalar_calls,
        )?;
        next_call_obligation = scalar_calls.next_obligation_identity;
        Some(result)
    } else {
        None
    };
    next_operation = operations.next_identity;
    let CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
        ..
    } = plan.operations.last().expect("Unit sequence was validated")
    else {
        unreachable!()
    };
    let local_discards = trivial_affine_local_discard_ordinals.iter().map(|ordinal| {
        local_places
            .get(usize::try_from(*ordinal).map_err(|_| {
                LoweringError::Unsupported("Unit local cleanup ordinal exceeds usize")
            })?)
            .map(|local| local.id)
            .ok_or(LoweringError::Unsupported(
                "Unit local cleanup ordinal is not dense",
            ))
    });
    // Legacy locals are a leading constructor prefix; operation results
    // therefore die before those locals, and all locals before parameters.
    // Admitting interleaved legacy constructors requires one declaration-
    // ordered cleanup roster instead of concatenating these two groups.
    // Parameter selection sources have no result row: they follow the
    // result rows in descending authored position so the splice replaces
    // each row with its residual join parameter.
    // A projected owned argument can move the complete subtree roster out
    // of a result carrier, in which case the frontier retires the place at
    // the consuming call and the return roster must not discard its root.
    let moved_subtrees = projected_moves::moved_subtrees(&operations.operations);
    let mut selection_roster = structural_result_places
        .iter()
        .rev()
        .map(|(place, discard)| {
            let retired = moved_subtrees
                .get(&evaluation.current_structural_place(place.id))
                .is_some_and(|moved| {
                    projected_moves::retires(structural_types, &operations.operations, place, moved)
                });
            (place.id, *discard && !retired)
        })
        .collect::<Vec<_>>();
    let mut parameter_sources = Vec::new();
    for cleanup in &evaluation.selection_cleanups {
        for source in &cleanup.sources {
            if let Some((position, _)) = evaluation
                .structural_parameters
                .iter()
                .find(|(_, declaration)| declaration.place == *source)
            {
                parameter_sources.push((*position, *source));
            }
        }
    }
    parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    parameter_sources.dedup_by_key(|(_, place)| *place);
    selection_roster.extend(
        parameter_sources
            .into_iter()
            .map(|(_, place)| (place, false)),
    );
    let trivial_affine_discards = evaluation
        .selection_return_discards(selection_roster)?
        .into_iter()
        .map(Ok)
        .chain(local_discards)
        .chain(trivial_affine_discards.iter().map(|parameter_index| {
            parameters
                .get(usize::try_from(*parameter_index).map_err(|_| {
                    LoweringError::Unsupported("Unit affine discard parameter index exceeds usize")
                })?)
                .map(|parameter| parameter.place)
                .ok_or(LoweringError::Unsupported(
                    "Unit affine discard has an invalid parameter index",
                ))
        }))
        .collect::<Result<Vec<_>, _>>()?;
    let block = evaluation.current;
    let mut scalar_return = plan
        .scalar_result
        .as_ref()
        .map(|result| {
            let position = scalar_parameter_count
                .checked_add(result.binding_ordinal as usize)
                .ok_or(LoweringError::Unsupported(
                    "scalar completion binding position overflows",
                ))?;
            let source = scalar_result_values
                .get(position)
                .ok_or(LoweringError::Unsupported(
                    "scalar completion binding is absent",
                ))?;
            let scalar_type = terminal_scalar_type(result.primitive_type)?;
            if source.scalar_type != scalar_type {
                return unsupported("scalar completion binding has a different carrier");
            }
            Ok::<_, LoweringError>((
                source.id,
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(allocate_dense(&mut next_value_identity)?),
                    scalar_type,
                },
            ))
        })
        .transpose()?;
    if let Some(source) = controlled_result {
        scalar_return = Some((
            source.id,
            ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(&mut next_value_identity)?),
                scalar_type: source.scalar_type,
            },
        ));
    }
    // Completion reserves its own scalar result identity after the body.
    // Include it before the next helper borrows the shared value allocator.
    next_value = next_value_identity;
    let structural_return = plan
        .structural_result
        .as_ref()
        .map(|result| {
            let source = match result.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    structural_result_places
                        .get(binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "returned structural binding is absent",
                        ))?
                        .0
                        .id
                }
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                    parameters
                        .get(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "returned structural parameter is absent",
                        ))?
                        .place
                }
                _ => {
                    return unsupported("structural return source is not an owned whole value");
                }
            };
            let source = evaluation.current_structural_place(source);
            let place = place_id(allocate_dense(&mut next_place)?);
            let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
            Ok::<_, LoweringError>((
                source,
                terminal_psi::StructuralResultDeclaration {
                    place,
                    structural_type,
                    multiplicity: match result.multiplicity {
                        Multiplicity::Affine => StructuralMultiplicity::Affine,
                        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                        Multiplicity::Linear => {
                            return unsupported(
                                "structural completion requires retained linear result claims",
                            );
                        }
                    },
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: result
                        .reference_sources
                        .iter()
                        .map(|reference| {
                            let arguments = parameters::lower_structural_arguments(
                                std::slice::from_ref(&reference.source),
                                parameters,
                                &[],
                                &[],
                                &[],
                                &[],
                            )?;
                            Ok(terminal_psi::StructuralReferenceResultSource {
                                path: lower_structural_path(&reference.path)?,
                                source: arguments[0].clone(),
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()
                        .map(|mut sources| {
                            // Wire maps use canonical path order, independently
                            // of authored construction and reverse cleanup order.
                            sources.sort_by(|left, right| left.path.cmp(&right.path));
                            sources
                        })?,
                },
            ))
        })
        .transpose()?;
    let edge = edge_id(allocate_dense(&mut next_edge)?);
    let crash_routes = {
        let effective = crate::unit::effective_crash_routes(checked, plan.machine)?;
        if parameters.is_empty() {
            lower_checked_crash_route_buckets(&effective, &scalar_parameters)?
        } else {
            lower_structural_crash_route_buckets(
                &effective,
                &scalar_parameters,
                &signature.predicate_parameters,
                structural_types,
                runtime_requirements,
            )?
        }
    };
    evaluation.remap_transported_call_operands(&mut operations);
    evaluation.blocks.push(Block {
        id: block,
        parameters: evaluation.parameters,
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: evaluation.block_structural_parameters,
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: if let Some((source, _)) = &scalar_return {
            Terminator::Return {
                edge,
                value: *source,
                cleanup_actions: trivial_affine_discards
                    .into_iter()
                    .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                    .collect(),
            }
        } else if let Some((source, _)) = &structural_return {
            Terminator::ReturnStructural {
                edge,
                source: *source,
                returned_claims: Vec::new(),
                trivial_affine_discards,
            }
        } else {
            Terminator::ReturnUnit {
                edge,
                trivial_affine_discards,
            }
        },
    });
    evaluation.blocks.sort_by_key(|block| block.id);
    let computed_array_places = evaluation
        .blocks
        .iter()
        .flat_map(|block| {
            crate::scalar_graph::scalar_computations::arrays::declarations(&block.operations).chain(
                crate::scalar_graph::scalar_computations::cases::declarations(&block.operations),
            )
        })
        .filter(|place| {
            !structural_result_places
                .iter()
                .any(|(existing, _)| existing.id == place.id)
                && !structural_value_temporaries
                    .iter()
                    .any(|existing| existing.id == place.id)
        })
        .collect::<Vec<_>>();
    let OperationBuffer {
        source_calls,
        selected_ieee_float_fmas,
        selected_ieee_float_comparisons,
        selected_integer_comparisons,
        ..
    } = operations;
    let mut structural_places = parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .chain(provider_places.iter().copied())
        .chain(local_places.iter().copied())
        .chain(primitive_local_places.iter().map(|local| local.declaration))
        .chain(literal_places.iter().copied())
        .chain(subslice_places.iter().copied())
        .chain(structural_result_places.iter().map(|(place, _)| *place))
        .chain(structural_value_temporaries)
        .chain(computed_array_places)
        .collect::<Vec<_>>();
    // Argument-time view producers interleave with reserved call results.
    // Declaration order is canonical identity order, not execution order.
    structural_places.sort_by_key(|place| place.id);
    if let Some((_, result)) = &structural_return {
        structural_places.push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::Result,
        });
    }
    let normal_guarantees = if let Some((_, result)) = scalar_return {
        // Contract result identity is distinct from the body's returned
        // binding. Only exit reconstruction equates them, after normal
        // completion; crashes and incomplete calls establish neither.
        let mut namespace = scalar_parameters.clone();
        namespace.push(result);
        let contract = checked
            .facts
            .contract_plans
            .for_machine(plan.machine)
            .ok_or(LoweringError::Unsupported(
                "scalar completion has no checked contract",
            ))?;
        let refined = crate::scalar_graph::scalar_contracts::with_result_range(
            checked,
            plan.state,
            scalar_parameters.len(),
            &contract.closed_scalar_values,
        )?;
        crate::scalar_graph::scalar_contracts::clauses(
            refined.ensures(),
            &namespace,
            &signature.erased_scalar_parameters,
        )?
        .into_iter()
        .map(|proposition| {
            Ok(ContractClause {
                obligation: obligation_id(allocate_dense(&mut next_call_obligation)?),
                proposition,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?
    } else {
        Vec::new()
    };
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: lower_declared_service_reach(checked, plan.machine, service_ids)?,
        id: terminal_machine,
        attachment,
        parameters: scalar_parameters.clone(),
        structural_parameters: parameters.clone(),
        ranked_scc: None,
        result: if let Some((_, result)) = scalar_return {
            TerminalMachineResult::Scalar(result)
        } else {
            structural_return.map_or(TerminalMachineResult::Unit, |(_, result)| {
                TerminalMachineResult::Structural(result)
            })
        },
        structural_places,
        entry_claims: entry_claims.clone(),
        published_service_ceiling: if let Some(provider) = provider_candidate_plans
            .iter()
            .find(|candidate| candidate.candidate == plan.machine)
        {
            lower_provider_candidate_service_ceiling(checked, plans, provider, plan, service_ids)?
        } else {
            lower_installation_machine_service_ceiling(
                checked,
                plan.machine,
                plan.contract_service_reach,
                plan.service_reach,
                service_ids,
            )?
        },
        content_entry_claims,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: evaluation.entry,
        blocks: evaluation.blocks,
        contract: MachineContract {
            id: contract_id(terminal_machine.get()),
            crash_routes,
            erased_scalar_formals: signature.erased_scalar_parameters.clone(),
            erased_proof_formals:
                crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
                    &signature.erased_proof_parameters,
                ),
            requires: signature.requires.clone(),
            ensures: normal_guarantees,
            outcome_specific_ensures: Vec::new(),
        },
    };
    *place_counter = next_place;
    *value_counter = next_value;
    *block_counter = next_block;
    *operation_counter = next_operation;
    *edge_counter = next_edge;
    *call_obligation_counter = next_call_obligation;
    Ok(EmittedMachine {
        machine,
        source_calls,
        selected_ieee_float_fmas,
        selected_ieee_float_comparisons,
        selected_integer_comparisons,
    })
}

impl MachineEmission<'_> {
    /// Lower one scheduled step: argument staging bookkeeping, a continuation
    /// cleanup, or one operation dispatched to its kind's method and pushed
    /// with its source-call record.
    fn lower_step(&mut self, step: argument_schedule::Step) -> Result<(), LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        self.scalar_calls = CallEmissionContext {
            machine_ids: self.machine_ids,
            requirement_counts: self.scalar_requirement_counts,
            next_obligation_identity: self.next_call_obligation,
            obligation_limit: u64::MAX,
        };
        let (operation_index, staged) = match step {
            argument_schedule::Step::Begin => {
                if self
                    .retained_scalar_prefix
                    .replace(self.scalar_result_values.len())
                    .is_some()
                    || self.staged_scalar_result.is_some()
                {
                    return unsupported("nested argument staging has an unfinished group");
                }
                return Ok(());
            }
            argument_schedule::Step::End => {
                let prefix =
                    self.retained_scalar_prefix
                        .take()
                        .ok_or(LoweringError::Unsupported(
                            "nested argument staging has no active group",
                        ))?;
                self.scalar_result_values.truncate(prefix);
                // Argument temporaries are never source-local bindings.
                // Only the successful outer result extends that namespace.
                if let Some(result) = self.staged_scalar_result.take() {
                    self.scalar_result_values.push(result);
                }
                return Ok(());
            }
            argument_schedule::Step::Argument { operation, ordinal } => {
                let source_value_count =
                    self.retained_scalar_prefix
                        .ok_or(LoweringError::Unsupported(
                            "staged scalar argument has no source prefix",
                        ))?;
                if self.staged_arguments[operation].len() != ordinal {
                    return unsupported("staged scalar argument ordinals are not dense");
                }
                let value = self.evaluation.argument_at(
                    checked,
                    plan.machine,
                    plan.state,
                    &plan.operations[operation],
                    ordinal,
                    source_value_count,
                    &mut self.scalar_result_values,
                    &mut self.next_value_identity,
                    &mut self.next_block,
                    &mut self.next_edge,
                    &mut self.operations,
                    &mut self.scalar_calls,
                )?;
                self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                self.staged_arguments[operation].push(self.scalar_result_values.len());
                self.scalar_result_values.push(value);
                return Ok(());
            }
            argument_schedule::Step::Subslice { operation, ordinal } => {
                let source_value_count =
                    self.retained_scalar_prefix
                        .ok_or(LoweringError::Unsupported(
                            "subslice staging has no source prefix",
                        ))?;
                if self.staged_subslices[operation]
                    .iter()
                    .any(|(prior, _)| *prior == ordinal)
                {
                    return unsupported("subslice argument is evaluated more than once");
                }
                let place = byte_subslices::emit(
                    checked,
                    plan,
                    &plan.operations[operation],
                    ordinal,
                    self.parameters,
                    &self.evaluation.structural_parameters,
                    &self.evaluation.view_locals,
                    &self.scalar_result_values[..source_value_count],
                    self.type_ids,
                    &mut self.next_place,
                    &mut self.next_value_identity,
                    &mut self.operations,
                )?;
                self.staged_subslices[operation].push((ordinal, place.id));
                self.subslice_places.push(place);
                return Ok(());
            }
            argument_schedule::Step::Call(index) if self.retained_scalar_prefix.is_some() => {
                (index, true)
            }
            argument_schedule::Step::Constructor(index)
                if self.retained_scalar_prefix.is_some() =>
            {
                (index, true)
            }
            argument_schedule::Step::Ordinary(index) if self.retained_scalar_prefix.is_none() => {
                (index, false)
            }
            _ => return unsupported("call schedule disagrees with its active argument group"),
        };
        let operation = &plan.operations[operation_index];
        // A continuation cleanup reads no operands; it commits the completed
        // call's dying results before the next operation evaluates any.
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
        ) {
            let source_value_count = self.scalar_result_values.len();
            return self.emit_through_frame(operation, source_value_count);
        }
        let source_value_count = self
            .retained_scalar_prefix
            .unwrap_or(self.scalar_result_values.len());
        let evaluated_scalar_arguments = if staged {
            Some(
                self.staged_arguments[operation_index]
                    .iter()
                    .map(|index| {
                        self.scalar_result_values.get(*index).copied().ok_or(
                            LoweringError::Unsupported(
                                "staged scalar operand lost its retained value",
                            ),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            self.evaluation.arguments(
                checked,
                plan.machine,
                plan.state,
                operation,
                &mut self.scalar_result_values,
                &mut self.next_value_identity,
                &mut self.next_block,
                &mut self.next_edge,
                &mut self.operations,
                &mut self.scalar_calls,
            )?
        };
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        self.source_call = None;
        let step = StepInputs {
            operation_index,
            staged,
            source_value_count,
            evaluated_scalar_arguments,
        };
        let kind = match operation {
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
            | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                return self.emit_through_frame(operation, step.source_value_count);
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                self.establish_structural_value(operation)?
            }
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. } => {
                self.establish_primitive_local(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::EstablishReference { .. } => {
                self.establish_reference(operation)?
            }
            CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {
                self.release_reference(operation)?
            }
            CheckedUnitEffectOperationPlan::EstablishScalarArray { .. } => {
                self.establish_scalar_array(operation)?
            }
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. } => {
                self.establish_trivial_affine_local(operation)?
            }
            CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
                if !UnitBody::contains(self.plans, *target_machine) =>
            {
                self.external_structural_call(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::CallUnit { .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { .. } => {
                self.call_unit(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::ScalarCall { .. }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. } => {
                self.scalar_call(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall { .. } => {
                self.selected_operator_structural_scalar_call(operation)?
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. } => {
                self.selected_operator_structural_call(operation)?
            }
            CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. } => {
                self.selected_ieee_float_fused_multiply_add(operation)?
            }
            CheckedUnitEffectOperationPlan::BoundaryCall { .. } => {
                self.boundary_call(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. } => {
                self.boundary_scalar_call(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. } => {
                self.boundary_structural_call(operation, &step)?
            }
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_) => {
                return unsupported("Unit structural case field store has no lowered operation");
            }
            CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => {
                return unsupported("Unit return is not the final checked operation");
            }
        };
        let Some(kind) = kind else {
            return Ok(());
        };
        let id = self.operations.allocate();
        if let Some((coordinate, source_site, target_machine)) = self.source_call {
            self.operations.record_source_call(
                SourceCallCoordinate {
                    state: plan.state,
                    statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "boundary Unit call statement coordinate exceeds usize",
                        )
                    })?,
                    call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "boundary Unit call ordinal coordinate exceeds usize",
                        )
                    })?,
                },
                source_site,
                id,
                target_machine,
            )?;
        }
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: terminal_psi::OperationResult::Unit,
            kind,
        });
        Ok(())
    }
}

impl<'a> MachineEmission<'a> {
    /// Lower one store, scalar local, borrowed window or continuation cleanup
    /// through the operation frame composed states share.
    fn emit_through_frame(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        source_value_count: usize,
    ) -> Result<(), LoweringError> {
        let plan = self.plan;
        OperationFrame {
            checked: self.checked,
            machine: plan.machine,
            state: plan.state,
            scalar_result: plan.scalar_result.as_ref(),
            scalar_parameter_count: self.scalar_parameter_count,
            source_value_count,
            parameters: self.parameters,
            structural_types: StructuralTypeRoster::Owned(self.structural_types),
            type_ids: self.type_ids,
            service_ids: self.service_ids,
            primitive_locals: &self.primitive_local_places,
            results: StructuralResults::Dense(&mut self.structural_result_places),
            literal_places: &mut self.literal_places,
            windows: &mut self.borrowed_windows,
            evaluation: &mut self.evaluation,
            values: &mut self.scalar_result_values,
            next_place: &mut self.next_place,
            next_value: &mut self.next_value_identity,
            next_block: &mut self.next_block,
            next_edge: &mut self.next_edge,
            calls: &mut self.scalar_calls,
            operations: &mut self.operations,
        }
        .emit(operation)?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        Ok(())
    }
}
