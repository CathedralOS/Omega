//! One ordinary attached Unit machine lowered from its checked effect plan:
//! provider attachment and local places, byte-sequence literals, the argument
//! schedule that emits every operation in order, then completion, the cleanup
//! roster, crash routes, and the machine's single block.

use super::bodies::UnitBody;
use super::call_closure::unique_unit_boundary;
use super::catalog::lower_provider_candidate_service_ceiling;
use super::composed_control::callable::EmissionCounters;
use super::parameters::lower_declared_service_reach;
use super::parameters::{
    lower_installation_machine_service_ceiling, lower_structural_arguments, validate_transfer_shape,
};
use super::provider_attachments::lower_provider_attachment_places;
use super::provider_attachments::validate_provider_attachment_requirements;
use super::signatures::MachineSignature;
use super::{
    CheckedUnitProviderCandidate, argument_evaluation, argument_schedule, byte_subslices,
    ordinary_calls, parameters, primitive_locals, reference_results, scalar_arrays, signatures,
    structural_calls, structural_values,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::scalar_graph::scalar_call_closure::callee::{CheckedScalarCallee, PreparedScalarCallee};
use crate::unit::{
    Block, CheckedBoundaryMachineResultPlan, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, ClaimTransfer,
    CompletionReceipt, ContractClause, LoweringError, MachineContract, Multiplicity, Operation,
    OperationKind, OperationResult, PlaceId, ScalarType, SemanticDomainId, StructuralDomainId,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, Terminator, ValueDeclaration, allocate_dense, content_conservation,
    contract_id, direct_expression_contains_short_circuit, edge_id, emit_direct_expression,
    lookup_claim_id, lookup_domain_id, lookup_machine_id, lookup_service_id, lookup_type_id,
    lower_checked_crash_route_buckets, lower_checked_scalar_expression,
    lower_structural_crash_route_buckets, obligation_id, place_id, terminal_scalar_type,
    unsupported, validate_direct_parameter_types, value_id,
};
use crate::unit::{
    BoundaryMachineId, MachineId, ServiceId, ServiceReachId, StructuralParameterDeclaration,
    StructuralTypeDeclaration,
};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
use lowered_psi::{
    LoweredSelectedIeeeFloatComparisonOccurrence, LoweredSelectedIeeeFloatFmaOccurrence,
    LoweredSourceCallOccurrence,
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
    pub(super) plans: &'a checked_trees::CheckedUnitEffectPlans,
    pub(super) closure: &'a [symbols::SymbolHandle],
    pub(super) provider_candidate_plans: &'a [CheckedUnitProviderCandidate],
    pub(super) prepared_scalar_machines: &'a [PreparedScalarCallee<'a>],
}

/// The lowered machine and the occurrence rows the closure accumulates.
pub(super) struct EmittedMachine {
    pub(super) machine: TerminalMachine,
    pub(super) source_calls: Vec<LoweredSourceCallOccurrence>,
    pub(super) selected_ieee_float_fmas: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
    pub(super) selected_ieee_float_comparisons: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
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
    let mut next_edge = *edge_counter;
    let mut next_call_obligation = *call_obligation_counter;
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
    let called_boundaries = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { target_machine, .. } => {
                Some(*target_machine)
            }
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
            .structural_types
            .iter()
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
        match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            } => literal_arguments.extend(
                structural_arguments
                    .iter()
                    .filter(|argument| argument.byte_sequence_literal().is_some()),
            ),
            _ => {}
        }
    }
    let mut literal_places = literal_arguments
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| {
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
    for (argument, place) in literal_arguments.iter().zip(&literal_places) {
        let bytes = argument
            .byte_sequence_literal()
            .ok_or(LoweringError::Unsupported(
                "byte-sequence literal payload is absent",
            ))?;
        let id = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            id,
            result: terminal_psi::OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place.id,
                bytes: bytes.to_vec(),
            },
        });
    }
    let mut next_literal_argument = 0usize;
    let call_literal_count = literal_places.len();
    let mut next_value_identity = next_value;
    let mut scalar_result_values = scalar_parameters.clone();
    let mut primitive_local_places = Vec::<primitive_locals::PrimitiveLocal>::new();
    let mut structural_result_places = Vec::<(StructuralPlaceDeclaration, bool)>::new();
    let mut structural_value_temporaries = Vec::<StructuralPlaceDeclaration>::new();
    let mut evaluation = argument_evaluation::Evaluation::new(&mut next_block)?;
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
    let mut staged_arguments = vec![Vec::<usize>::new(); plan.operations.len()];
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
    let mut staged_subslices = vec![Vec::<(usize, PlaceId)>::new(); plan.operations.len()];
    let mut subslice_places = Vec::new();
    let mut retained_scalar_prefix = None;
    let mut staged_scalar_result = None;
    for step in argument_schedule::build(checked, plan)? {
        let mut scalar_calls = CallEmissionContext {
            machine_ids,
            requirement_counts: scalar_requirement_counts,
            next_obligation_identity: next_call_obligation,
            obligation_limit: u64::MAX,
        };
        let (operation_index, staged) = match step {
            argument_schedule::Step::Begin => {
                if retained_scalar_prefix
                    .replace(scalar_result_values.len())
                    .is_some()
                    || staged_scalar_result.is_some()
                {
                    return unsupported("nested argument staging has an unfinished group");
                }
                continue;
            }
            argument_schedule::Step::End => {
                let prefix = retained_scalar_prefix
                    .take()
                    .ok_or(LoweringError::Unsupported(
                        "nested argument staging has no active group",
                    ))?;
                scalar_result_values.truncate(prefix);
                // Argument temporaries are never source-local bindings.
                // Only the successful outer result extends that namespace.
                if let Some(result) = staged_scalar_result.take() {
                    scalar_result_values.push(result);
                }
                continue;
            }
            argument_schedule::Step::Argument { operation, ordinal } => {
                let source_value_count = retained_scalar_prefix.ok_or(
                    LoweringError::Unsupported("staged scalar argument has no source prefix"),
                )?;
                if staged_arguments[operation].len() != ordinal {
                    return unsupported("staged scalar argument ordinals are not dense");
                }
                let value = evaluation.argument_at(
                    checked,
                    plan.machine,
                    plan.state,
                    &plan.operations[operation],
                    ordinal,
                    source_value_count,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut scalar_calls,
                )?;
                next_call_obligation = scalar_calls.next_obligation_identity;
                staged_arguments[operation].push(scalar_result_values.len());
                scalar_result_values.push(value);
                continue;
            }
            argument_schedule::Step::Subslice { operation, ordinal } => {
                let source_value_count = retained_scalar_prefix.ok_or(
                    LoweringError::Unsupported("subslice staging has no source prefix"),
                )?;
                if staged_subslices[operation]
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
                    parameters,
                    &evaluation.structural_parameters,
                    &scalar_result_values[..source_value_count],
                    type_ids,
                    &mut next_place,
                    &mut next_value_identity,
                    &mut operations,
                )?;
                staged_subslices[operation].push((ordinal, place.id));
                subslice_places.push(place);
                continue;
            }
            argument_schedule::Step::Call(index) if retained_scalar_prefix.is_some() => {
                (index, true)
            }
            argument_schedule::Step::Constructor(index) if retained_scalar_prefix.is_some() => {
                (index, true)
            }
            argument_schedule::Step::Ordinary(index) if retained_scalar_prefix.is_none() => {
                (index, false)
            }
            _ => return unsupported("call schedule disagrees with its active argument group"),
        };
        let operation = &plan.operations[operation_index];
        if let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            affine_discards, ..
        } = operation
        {
            let mut discards = Vec::new();
            let mut residuals = Vec::new();
            for discard in affine_discards {
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } = discard.source
                else {
                    return unsupported("call continuation cleanup requires a result binding");
                };
                let (place, discard_on_return) = structural_result_places
                    .get(binding_ordinal as usize)
                    .ok_or(LoweringError::Unsupported(
                        "call continuation result has not been produced",
                    ))?;
                if *discard_on_return {
                    return unsupported("call continuation cleanup has conflicting custody");
                }
                if discard.path.is_empty() {
                    discards.push(place.id);
                } else {
                    residuals.push(terminal_psi::StructuralAffineDiscard {
                        place: place.id,
                        path: lower_structural_path(&discard.path),
                        structural_type: lookup_type_id(type_ids, &discard.type_identity)?,
                    });
                }
            }
            evaluation.cleanup_continuation(
                discards,
                residuals,
                &mut scalar_result_values,
                &mut next_value_identity,
                &mut next_block,
                &mut next_edge,
                &operations,
            )?;
            continue;
        }
        let source_value_count = retained_scalar_prefix.unwrap_or(scalar_result_values.len());
        let evaluated_scalar_arguments = if staged {
            Some(
                staged_arguments[operation_index]
                    .iter()
                    .map(|index| {
                        scalar_result_values
                            .get(*index)
                            .copied()
                            .ok_or(LoweringError::Unsupported(
                                "staged scalar operand lost its retained value",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else {
            evaluation.arguments(
                checked,
                plan.machine,
                plan.state,
                operation,
                &mut scalar_result_values,
                &mut next_value_identity,
                &mut next_block,
                &mut next_edge,
                &mut operations,
                &mut scalar_calls,
            )?
        };
        next_call_obligation = scalar_calls.next_obligation_identity;
        let mut source_call = None;
        let kind = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                discard_result_on_return,
                ..
            } => {
                let mut emit_operand_call =
                    |operand: &CheckedUnitEffectOperationPlan,
                     evaluated: Option<&[ValueDeclaration]>,
                     call_context: &mut CallEmissionContext<'_>,
                     output: &mut OperationBuffer,
                     place_counter: &mut u64| {
                        let CheckedUnitEffectOperationPlan::StructuralCall {
                            target_machine,
                            result,
                            ..
                        } = operand
                        else {
                            return unsupported("record operand is not a structural call");
                        };
                        if result.binding_ordinal as usize != structural_result_places.len() {
                            return unsupported("structural operand result binding is not dense");
                        }
                        let prepared = ordinary_calls::prepare(
                            checked,
                            plans,
                            operand,
                            signatures::find(machine_signatures, *target_machine)?.call_target(),
                            evaluated,
                            parameters,
                            &local_places,
                            &structural_result_places,
                            &primitive_local_places,
                            type_ids,
                            structural_types,
                            &[],
                            call_context,
                        )?;
                        let declaration = ordinary_calls::emit_structural(
                            checked,
                            plan.state,
                            operand,
                            prepared,
                            lookup_machine_id(machine_ids, *target_machine)?,
                            type_ids,
                            domain_ids,
                            claim_bindings,
                            true,
                            place_counter,
                            output,
                        )?;
                        structural_result_places.push((declaration, false));
                        Ok(declaration)
                    };
                let declaration = structural_values::emit(
                    checked,
                    plan.machine,
                    plan.state,
                    operation,
                    structural_types,
                    type_ids,
                    &mut next_place,
                    &mut structural_value_temporaries,
                    &mut emit_operand_call,
                    &mut scalar_calls,
                    &mut evaluation,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?;
                next_call_obligation = scalar_calls.next_obligation_identity;
                if result.binding_ordinal as usize != structural_result_places.len() {
                    return unsupported("structural value result binding is not dense");
                }
                structural_result_places.push((declaration, *discard_result_on_return));
                continue;
            }
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                statement_index,
                symbol,
                type_identity,
                primitive_type,
                ..
            } => {
                if primitive_local_places
                    .iter()
                    .any(|local| local.symbol == *symbol)
                {
                    return unsupported("primitive local is established more than once");
                }
                let value = crate::expression_preparation::bindings::ScalarBindings::new(
                    source_value_count,
                )
                .with_primitive_storage(&evaluation.primitive_storage)
                .expression_at(
                    checked,
                    plan.state,
                    *statement_index,
                    CheckedScalarExpressionRole::StorageInitializer,
                )?;
                let structural_type = lookup_type_id(type_ids, type_identity)?;
                if value.scalar_type() != terminal_scalar_type(*primitive_type)?
                    || !structural_types.iter().any(|declaration| {
                        declaration.id == structural_type
                            && declaration.shape
                                == StructuralTypeShape::PrimitiveScalar(value.scalar_type())
                    })
                {
                    return unsupported("primitive local initializer and referent types disagree");
                }
                let local = primitive_locals::emit(
                    *symbol,
                    structural_type,
                    &value,
                    &scalar_result_values,
                    &mut next_place,
                    &mut next_value_identity,
                    &mut operations,
                )?;
                evaluation.primitive_storage.push((
                    local.symbol,
                    local.declaration.id,
                    local.scalar_type,
                ));
                primitive_local_places.push(local);
                continue;
            }
            CheckedUnitEffectOperationPlan::EstablishReference { result, source } => {
                if result.binding_ordinal as usize != structural_result_places.len() {
                    return unsupported("reference result binding is not dense");
                }
                let arguments = parameters::lower_structural_arguments(
                    std::slice::from_ref(source),
                    parameters,
                    &local_places,
                    &structural_result_places,
                    &[],
                    &primitive_local_places,
                )?;
                let declaration = reference_results::emit(
                    result,
                    arguments[0].clone(),
                    type_ids,
                    &mut next_place,
                    &mut operations,
                )?;
                structural_result_places.push((declaration, false));
                continue;
            }
            CheckedUnitEffectOperationPlan::ReleaseReference {
                binding_ordinal, ..
            } => {
                let source = structural_result_places
                    .get(*binding_ordinal as usize)
                    .ok_or(LoweringError::Unsupported(
                        "released reference result is absent",
                    ))?
                    .0
                    .id;
                OperationKind::ReleaseReference {
                    source: evaluation.current_structural_place(source),
                }
            }
            CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source,
                result,
                elements,
            } => {
                if result.binding_ordinal as usize != structural_result_places.len() {
                    return unsupported("array result binding is not dense");
                }
                // Earlier arguments are private staging slots, not source
                // bindings. Keep them live through every leaf's control
                // joins, then remove only this constructor's leaf tail.
                let leaf_start = scalar_result_values.len();
                let source_value_count = retained_scalar_prefix.unwrap_or(leaf_start);
                for (ordinal, element) in elements.iter().enumerate() {
                    let element_ordinal = u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("array element ordinal exceeds u32")
                    })?;
                    let value = evaluation.source_value(
                        checked,
                        plan.machine,
                        plan.state,
                        result.statement_index,
                        CheckedScalarExpressionRole::ArrayElement {
                            source: *source,
                            element_ordinal,
                        },
                        element,
                        source_value_count,
                        &mut scalar_result_values,
                        &mut next_value_identity,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &mut scalar_calls,
                    )?;
                    scalar_result_values.push(value);
                }
                next_call_obligation = scalar_calls.next_obligation_identity;
                // Private control joins may replace every scalar identity.
                // Read the completed leaves only after the final element.
                let declaration = scalar_arrays::emit(
                    result,
                    &scalar_result_values[leaf_start..],
                    type_ids,
                    &mut next_place,
                    &mut operations,
                )?;
                scalar_result_values.truncate(leaf_start);
                structural_result_places.push((declaration, false));
                if *source == checked_trees::CheckedArrayConstructionSource::Statement {
                    structural_values::bind_local(
                        checked,
                        plan,
                        operation,
                        declaration.id,
                        &mut evaluation.structural_locals,
                    )?;
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal,
                type_identity,
                ..
            } => {
                let local = local_places
                    .get(usize::try_from(*declaration_ordinal).map_err(|_| {
                        LoweringError::Unsupported("Unit local ordinal exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "Unit local ordinal is not dense",
                    ))?;
                if !matches!(
                    local.kind,
                    StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: ordinal,
                        structural_type,
                        ..
                    } if ordinal == *declaration_ordinal
                        && structural_type == lookup_type_id(type_ids, type_identity)?
                ) {
                    return unsupported("Unit local declaration drifted from checked custody");
                }
                OperationKind::EstablishTrivialAffineLocal {
                    destination: local.id,
                }
            }
            CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. }
                if !UnitBody::contains(plans, *target_machine) =>
            {
                let result = structural_calls::emit(
                    checked,
                    plan,
                    operation,
                    parameters,
                    evaluated_scalar_arguments.as_deref(),
                    structural_types,
                    type_ids,
                    machine_ids,
                    &structural_result_places,
                    &mut next_place,
                    &mut operations,
                )?;
                structural_values::bind_local(
                    checked,
                    plan,
                    operation,
                    result.0.id,
                    &mut evaluation.structural_locals,
                )?;
                structural_result_places.push(result);
                continue;
            }
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                ..
            } => {
                let claim_transfers = match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        claim_transfers, ..
                    } => claim_transfers.as_slice(),
                    _ => &[],
                };
                let call_byte_places = byte_subslices::argument_places(
                    structural_arguments,
                    &literal_places,
                    &mut next_literal_argument,
                    &staged_subslices[operation_index],
                )?;
                scalar_calls.next_obligation_identity = next_call_obligation;
                let ordinary_calls::PreparedCall {
                    arguments: terminal_scalar_arguments,
                    structural_arguments: terminal_arguments,
                    requirement_obligations,
                    crash_continuations,
                } = ordinary_calls::prepare(
                    checked,
                    plans,
                    operation,
                    signatures::find(machine_signatures, *target_machine)?.call_target(),
                    evaluated_scalar_arguments.as_deref(),
                    parameters,
                    &local_places,
                    &structural_result_places,
                    &primitive_local_places,
                    type_ids,
                    structural_types,
                    &call_byte_places,
                    &mut scalar_calls,
                )?;
                next_call_obligation = scalar_calls.next_obligation_identity;
                source_call = Some((*coordinate, None, *target_state));
                if let CheckedUnitEffectOperationPlan::StructuralCall {
                    discard_result_on_return,
                    ..
                } = operation
                {
                    let declaration = ordinary_calls::emit_structural(
                        checked,
                        plan.state,
                        operation,
                        ordinary_calls::PreparedCall {
                            arguments: terminal_scalar_arguments,
                            structural_arguments: terminal_arguments,
                            requirement_obligations,
                            crash_continuations,
                        },
                        lookup_machine_id(machine_ids, *target_machine)?,
                        type_ids,
                        domain_ids,
                        claim_bindings,
                        true,
                        &mut next_place,
                        &mut operations,
                    )?;
                    let place = declaration.id;
                    structural_result_places.push((declaration, *discard_result_on_return));
                    structural_values::bind_local(
                        checked,
                        plan,
                        operation,
                        place,
                        &mut evaluation.structural_locals,
                    )?;
                    continue;
                }
                OperationKind::CallUnit {
                    callee: lookup_machine_id(machine_ids, *target_machine)?,
                    arguments: terminal_scalar_arguments,
                    structural_arguments: terminal_arguments,
                    claim_transfers: claim_transfers
                        .iter()
                        .map(|transfer| {
                            Ok(ClaimTransfer {
                                claim: lookup_claim_id(claim_bindings, transfer.claim_identity)?,
                                argument_index: transfer.argument_index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                    requirement_obligations,
                    crash_continuations,
                }
            }
            CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                result,
                target_machine: realization_machine,
                target_state: realization_state,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                coordinate,
                result,
                realization_machine,
                realization_state,
                ..
            } => {
                let source_target = match operation {
                    CheckedUnitEffectOperationPlan::ScalarCall { target_state, .. } => {
                        *target_state
                    }
                    CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                        realization_machine,
                        ..
                    } => *realization_machine,
                    _ => unreachable!("combined Unit scalar call arm"),
                };
                if usize::try_from(result.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                    != Some(source_value_count)
                {
                    return unsupported(
                        "Unit scalar result binding ordinal drifted from source order",
                    );
                }
                let target = match operation {
                    CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                        CheckedScalarCallee::find_for_unit_call(checked, *realization_machine)?
                    }
                    _ => CheckedScalarCallee::find(checked, *realization_machine)?,
                };
                // Both catalogs have been source-validated before emission.
                // An operation-body callee uses its real ordered emitter,
                // not a fabricated PreparedScalarCallee graph.
                let target_result = match prepared_scalar_machines
                    .iter()
                    .find(|prepared| prepared.source_machine() == *realization_machine)
                {
                    Some(prepared) => prepared.result_type(),
                    None if closure.contains(realization_machine)
                        && matches!(target, CheckedScalarCallee::Operations(_)) =>
                    {
                        target.result_type()?
                    }
                    None => {
                        return unsupported(
                            "Unit scalar call target is absent from the prepared closure",
                        );
                    }
                };
                let target_parameter_types = target.parameter_types()?;
                if target.entry_state()? != *realization_state
                    || target_result != terminal_scalar_type(result.primitive_type)?
                {
                    return unsupported(
                        "Unit scalar call disagrees with its prepared target signature",
                    );
                }
                let arguments = if let Some(arguments) = evaluated_scalar_arguments.as_deref() {
                    argument_evaluation::validated_values(
                        Some(arguments),
                        &target_parameter_types
                            .iter()
                            .map(|primitive| terminal_scalar_type(*primitive))
                            .collect::<Result<Vec<_>, _>>()?,
                    )?
                } else {
                    let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                        scalar_arguments,
                        ..
                    } = operation
                    else {
                        return unsupported("Unit scalar call has no evaluated arguments");
                    };
                    let source_types = scalar_result_values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>();
                    if scalar_arguments.len() != target_parameter_types.len() {
                        return unsupported("selected scalar call argument count disagrees");
                    }
                    scalar_arguments.iter().zip(&target_parameter_types).map(|(argument, primitive)| {
                        let argument = lower_checked_scalar_expression(argument)?;
                        let scalar_type = terminal_scalar_type(*primitive)?;
                        if argument.scalar_type() != scalar_type || direct_expression_contains_short_circuit(&argument) {
                            return unsupported("selected scalar call has unsupported argument control or carrier");
                        }
                        validate_direct_parameter_types(&argument, &source_types)?;
                        Ok(ValueDeclaration {
qualifications: Default::default(), id: emit_direct_expression(&argument, &scalar_result_values, &mut next_value_identity, &mut operations), scalar_type })
                    }).collect::<Result<Vec<_>, LoweringError>>()?
                };
                let target_contract = checked
                    .facts
                    .contract_plans
                    .for_machine(*realization_machine)
                    .ok_or(LoweringError::Unsupported(
                        "Unit scalar call target has no checked contract",
                    ))?;
                let crash_continuations = lower_checked_crash_route_buckets(
                    target_contract.crash.published(),
                    &arguments,
                )?;
                let requirement_count = scalar_requirement_counts
                    .iter()
                    .find_map(|(source, count)| (*source == *realization_machine).then_some(*count))
                    .ok_or(LoweringError::Unsupported(
                        "Unit scalar call target has no prepared contract",
                    ))?;
                let requirement_obligations = (0..requirement_count)
                    .map(|_| {
                        let obligation = obligation_id(next_call_obligation);
                        next_call_obligation = next_call_obligation.checked_add(1).ok_or(
                            LoweringError::Unsupported(
                                "Unit scalar call obligation identity space is exhausted",
                            ),
                        )?;
                        Ok(obligation)
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(next_value_identity),
                    scalar_type: target_result,
                };
                next_value_identity =
                    next_value_identity
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar result value identity space is exhausted",
                        ))?;
                let operation_id = operations.allocate();
                operations.record_source_call(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "Unit scalar call statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "Unit scalar call ordinal coordinate exceeds usize",
                            )
                        })?,
                    },
                    None,
                    operation_id,
                    source_target,
                )?;
                let callee = lookup_machine_id(machine_ids, *realization_machine)?;
                let arguments = arguments.iter().map(|argument| argument.id).collect();
                let kind = if let CheckedUnitEffectOperationPlan::ScalarCall {
                    structural_arguments,
                    claim_transfers,
                    ..
                } = operation
                    && (!structural_arguments.is_empty()
                        || !claim_transfers.is_empty()
                        || target.requires_structural_frame())
                {
                    if target.structural_parameters().is_empty() && !structural_arguments.is_empty()
                    {
                        return unsupported(
                            "structural scalar call has no structural checked body",
                        );
                    }
                    validate_transfer_shape(
                        structural_arguments,
                        claim_transfers,
                        parameters,
                        &local_places,
                        &structural_result_places,
                        target.structural_parameters(),
                        type_ids,
                        structural_types,
                        &target
                            .entry_claims()
                            .iter()
                            .map(|claim| claim.parameter_index)
                            .collect::<Vec<_>>(),
                        &primitive_local_places,
                        None,
                    )?;
                    OperationKind::CallStructuralScalar {
                        callee,
                        arguments,
                        structural_arguments: lower_structural_arguments(
                            structural_arguments,
                            parameters,
                            &local_places,
                            &structural_result_places,
                            &[],
                            &primitive_local_places,
                        )?,
                        claim_transfers: claim_transfers
                            .iter()
                            .map(|transfer| {
                                Ok(ClaimTransfer {
                                    claim: lookup_claim_id(
                                        claim_bindings,
                                        transfer.claim_identity,
                                    )?,
                                    argument_index: transfer.argument_index,
                                })
                            })
                            .collect::<Result<Vec<_>, LoweringError>>()?,
                        requirement_obligations,
                        crash_continuations,
                    }
                } else {
                    OperationKind::Call {
                        callee,
                        arguments,
                        requirement_obligations,
                        crash_continuations,
                    }
                };
                operations.push(Operation {
                    static_reach_binding: None,
                    id: operation_id,
                    result: terminal_psi::OperationResult::Scalar(value),
                    kind,
                });
                if staged {
                    if staged_scalar_result.replace(value).is_some() {
                        return unsupported(
                            "nested argument group produces more than one scalar binding",
                        );
                    }
                } else if !crate::emission::call_source_custody::initializers::discards_result(
                    checked,
                    plan.state,
                    *coordinate,
                )? {
                    scalar_result_values.push(value);
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value } => {
                if usize::try_from(result.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                    != Some(scalar_result_values.len())
                {
                    return unsupported(
                        "Unit scalar expression local binding drifted from source order",
                    );
                }
                let role = if plan.scalar_result.as_ref() == Some(result) {
                    CheckedScalarExpressionRole::Return
                } else {
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: result.binding_ordinal,
                    }
                };
                let lowered = evaluation.source_value(
                    checked,
                    plan.machine,
                    plan.state,
                    result.statement_index,
                    role,
                    value,
                    source_value_count,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut scalar_calls,
                )?;
                if lowered.scalar_type != terminal_scalar_type(result.primitive_type)? {
                    return unsupported(
                        "Unit scalar expression local type disagrees with its binding",
                    );
                }
                next_call_obligation = scalar_calls.next_obligation_identity;
                scalar_result_values.push(lowered);
                continue;
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                coordinate,
                result,
                realization_machine,
                realization_state,
                scalar_arguments,
                structural_arguments,
                ..
            } => {
                if usize::try_from(result.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                    != Some(scalar_result_values.len())
                {
                    return unsupported(
                        "selected structural Unit operator result binding ordinal drifted from source order",
                    );
                }
                let realizations = checked
                    .facts
                    .flow
                    .terminal_structural_scalar_returns
                    .machines
                    .iter()
                    .filter(|target| {
                        target.machine == *realization_machine && target.state == *realization_state
                    })
                    .collect::<Vec<_>>();
                let [target] = realizations.as_slice() else {
                    return unsupported(
                        "selected structural Unit operator target is absent or ambiguous",
                    );
                };
                validate_transfer_shape(
                    structural_arguments,
                    &[],
                    parameters,
                    &[],
                    &[],
                    &target.structural_parameters,
                    type_ids,
                    structural_types,
                    &[],
                    &primitive_local_places,
                    None,
                )?;
                if scalar_arguments.len() != target.scalar_parameters.len() {
                    return unsupported(
                        "selected structural Unit operator scalar argument count disagrees with its realization",
                    );
                }
                let source_types = scalar_result_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>();
                let scalar_arguments = scalar_arguments
                    .iter()
                    .zip(&target.scalar_parameters)
                    .map(|(argument, target)| {
                        let argument = lower_checked_scalar_expression(argument)?;
                        if direct_expression_contains_short_circuit(&argument) {
                            return unsupported(
                                "selected structural Unit operator arguments do not yet admit short-circuit control",
                            );
                        }
                        let target_type = terminal_scalar_type(target.primitive_type)?;
                        if argument.scalar_type() != target_type {
                            return unsupported(
                                "selected structural Unit operator scalar argument type disagrees with its realization",
                            );
                        }
                        validate_direct_parameter_types(&argument, &source_types)?;
                        Ok(emit_direct_expression(
                            &argument,
                            &scalar_result_values,
                            &mut next_value_identity,
                            &mut operations,
                        ))
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let arguments = lower_structural_arguments(
                    structural_arguments,
                    parameters,
                    &[],
                    &[],
                    &[],
                    &primitive_local_places,
                )?;
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(next_value_identity),
                    scalar_type: terminal_scalar_type(result.primitive_type)?,
                };
                next_value_identity =
                    next_value_identity
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "selected structural scalar result identity space is exhausted",
                        ))?;
                let operation_id = operations.allocate();
                operations.record_source_call(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "selected structural scalar statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "selected structural scalar call ordinal exceeds usize",
                            )
                        })?,
                    },
                    None,
                    operation_id,
                    *realization_machine,
                )?;
                operations.push(Operation {
                    static_reach_binding: None,
                    id: operation_id,
                    result: OperationResult::Scalar(value),
                    kind: OperationKind::CallStructuralScalar {
                        callee: lookup_machine_id(machine_ids, *realization_machine)?,
                        arguments: scalar_arguments,
                        structural_arguments: arguments,
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                });
                scalar_result_values.push(value);
                continue;
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                coordinate,
                result,
                realization_machine,
                realization_state,
                scalar_arguments,
                structural_arguments,
                discard_result_on_return,
                ..
            } => {
                let realizations = checked
                    .facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines
                    .iter()
                    .filter(|target| {
                        target.machine == *realization_machine && target.state == *realization_state
                    })
                    .collect::<Vec<_>>();
                let [target] = realizations.as_slice() else {
                    return unsupported(
                        "selected structural-result Unit operator target is absent or ambiguous",
                    );
                };
                validate_transfer_shape(
                    structural_arguments,
                    &[],
                    parameters,
                    &[],
                    &[],
                    std::slice::from_ref(&target.structural_parameter),
                    type_ids,
                    structural_types,
                    &[],
                    &primitive_local_places,
                    None,
                )?;
                if scalar_arguments.len() != target.scalar_parameters.len() {
                    return unsupported(
                        "selected structural-result scalar argument count disagrees with its realization",
                    );
                }
                let source_types = scalar_result_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>();
                let scalar_arguments = scalar_arguments
                    .iter()
                    .zip(&target.scalar_parameters)
                    .map(|(argument, target)| {
                        let argument = lower_checked_scalar_expression(argument)?;
                        if direct_expression_contains_short_circuit(&argument) {
                            return unsupported(
                                "selected structural-result arguments do not admit short-circuit control",
                            );
                        }
                        let target_type = terminal_scalar_type(target.primitive_type)?;
                        if argument.scalar_type() != target_type {
                            return unsupported(
                                "selected structural-result scalar argument type disagrees with its realization",
                            );
                        }
                        validate_direct_parameter_types(&argument, &source_types)?;
                        Ok(emit_direct_expression(
                            &argument,
                            &scalar_result_values,
                            &mut next_value_identity,
                            &mut operations,
                        ))
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let arguments = lower_structural_arguments(
                    structural_arguments,
                    parameters,
                    &[],
                    &[],
                    &[],
                    &primitive_local_places,
                )?;
                let operation_id = operations.allocate();
                let result_place = place_id(allocate_dense(&mut next_place)?);
                let result_type = lookup_type_id(type_ids, &result.type_identity)?;
                let result_declaration = StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation_id,
                        structural_type: result_type,
                    },
                };
                operations.record_source_call(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "selected structural-result statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "selected structural-result call ordinal exceeds usize",
                            )
                        })?,
                    },
                    None,
                    operation_id,
                    *realization_machine,
                )?;
                operations.push(Operation {
                    static_reach_binding: None,
                    id: operation_id,
                    result: OperationResult::Structural(StructuralOperationResult {
                        place: result_place,
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::CallStructuralWithScalarArguments {
                        callee: lookup_machine_id(machine_ids, *realization_machine)?,
                        arguments: scalar_arguments,
                        structural_arguments: arguments,
                        claim_transfers: Vec::new(),
                        returned_claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                });
                structural_result_places.push((result_declaration, *discard_result_on_return));
                continue;
            }
            CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                coordinate,
                result,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                format,
                operands,
            } => {
                if usize::try_from(result.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                    != Some(scalar_result_values.len())
                {
                    return unsupported(
                        "selected IEEE FMA result binding ordinal drifted from source order",
                    );
                }
                let result_type = terminal_scalar_type(result.primitive_type)?;
                if result_type != ScalarType::IeeeFloat(*format) {
                    return unsupported(
                        "selected IEEE FMA result type disagrees with its exact format",
                    );
                }
                let [left, right, addend] = operands.as_slice() else {
                    return unsupported("selected IEEE FMA must retain exactly three operands");
                };
                let source_types = scalar_result_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>();
                let mut lower_operand = |operand: &CheckedScalarExpression| {
                    let operand = lower_checked_scalar_expression(operand)?;
                    if direct_expression_contains_short_circuit(&operand) {
                        return unsupported(
                            "selected IEEE FMA operands do not admit short-circuit control",
                        );
                    }
                    if operand.scalar_type() != result_type {
                        return unsupported(
                            "selected IEEE FMA operand type disagrees with its result",
                        );
                    }
                    validate_direct_parameter_types(&operand, &source_types)?;
                    Ok(emit_direct_expression(
                        &operand,
                        &scalar_result_values,
                        &mut next_value_identity,
                        &mut operations,
                    ))
                };
                let left = lower_operand(left)?;
                let right = lower_operand(right)?;
                let addend = lower_operand(addend)?;
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(next_value_identity),
                    scalar_type: result_type,
                };
                next_value_identity =
                    next_value_identity
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "selected IEEE FMA result value identity space is exhausted",
                        ))?;
                let operation = operations.allocate();
                operations.record_selected_ieee_float_fma(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "selected IEEE FMA statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "selected IEEE FMA call ordinal exceeds usize",
                            )
                        })?,
                    },
                    operation,
                    *requirement_operator,
                    *provider_plan_report_fingerprint,
                    *provider_plan_commitment,
                    *format,
                )?;
                operations.push(Operation {
                    static_reach_binding: None,
                    id: operation,
                    result: terminal_psi::OperationResult::Scalar(value),
                    kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                        left,
                        right,
                        addend,
                    },
                });
                scalar_result_values.push(value);
                continue;
            }
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                source_site,
                target_machine,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                ..
            } => {
                source_call = Some((*coordinate, *source_site, *target_machine));
                let target = unique_unit_boundary(plans, *target_machine)?;
                structural_calls::validate_consumer(
                    checked,
                    plan,
                    operation,
                    &target.structural_parameters,
                    &[],
                )?;
                let expected_claim_arguments = structural_arguments
                    .iter()
                    .enumerate()
                    .flat_map(|(argument_index, argument)| {
                        plan.entry_claims
                            .iter()
                            .filter(move |claim| {
                                argument.byte_sequence_literal().is_none()
                                    && Some(claim.parameter_index)
                                        == argument.source_parameter_index()
                                    && (argument.path.is_empty() || claim.path == argument.path)
                            })
                            .map(move |_| {
                                u32::try_from(argument_index).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "boundary Unit argument index exceeds u32",
                                    )
                                })
                            })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                validate_transfer_shape(
                    structural_arguments,
                    completion_receipts,
                    parameters,
                    &[],
                    &structural_result_places,
                    &target.structural_parameters,
                    type_ids,
                    structural_types,
                    &expected_claim_arguments,
                    &primitive_local_places,
                    None,
                )?;
                let (_, boundary, _, target_scalar_parameters) = lowered_boundary_parameters
                    .iter()
                    .find(|(symbol, _, _, _)| *symbol == *target_machine)
                    .ok_or(LoweringError::Unsupported(
                        "boundary Unit call target is absent from the lowered closure",
                    ))?;
                if scalar_arguments.len() != target_scalar_parameters.len() {
                    return unsupported(
                        "boundary Unit scalar argument count disagrees with its declaration",
                    );
                }
                let arguments = argument_evaluation::validated_values(
                    evaluated_scalar_arguments.as_deref(),
                    target_scalar_parameters,
                )?
                .iter()
                .map(|value| value.id)
                .collect();
                let call_byte_places = byte_subslices::argument_places(
                    structural_arguments,
                    &literal_places,
                    &mut next_literal_argument,
                    &staged_subslices[operation_index],
                )?;
                OperationKind::BoundaryCall {
                    boundary: *boundary,
                    arguments,
                    structural_arguments: lower_structural_arguments(
                        structural_arguments,
                        parameters,
                        &[],
                        &structural_result_places,
                        &call_byte_places,
                        &primitive_local_places,
                    )?,
                    completion_receipts: completion_receipts
                        .iter()
                        .map(|settlement| {
                            Ok(CompletionReceipt {
                                claim: lookup_claim_id(claim_bindings, settlement.claim_identity)?,
                                argument_index: settlement.argument_index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                }
            }
            CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                source_site,
                result,
                target_machine,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                ..
            } => {
                if usize::try_from(result.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
                    != Some(source_value_count)
                {
                    return unsupported(
                        "Unit scalar result binding ordinal drifted from source order",
                    );
                }
                let target = unique_unit_boundary(plans, *target_machine)?;
                if target.result.scalar() != Some(result.primitive_type) {
                    return unsupported(
                        "Unit scalar result type drifted from its checked boundary target",
                    );
                }
                structural_calls::validate_consumer(
                    checked,
                    plan,
                    operation,
                    &target.structural_parameters,
                    &[],
                )?;
                let expected_claim_arguments = structural_arguments
                    .iter()
                    .enumerate()
                    .flat_map(|(argument_index, argument)| {
                        plan.entry_claims
                            .iter()
                            .filter(move |claim| {
                                argument.byte_sequence_literal().is_none()
                                    && Some(claim.parameter_index)
                                        == argument.source_parameter_index()
                                    && (argument.path.is_empty() || claim.path == argument.path)
                            })
                            .map(move |_| {
                                u32::try_from(argument_index).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "boundary scalar argument index exceeds u32",
                                    )
                                })
                            })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                validate_transfer_shape(
                    structural_arguments,
                    completion_receipts,
                    parameters,
                    &[],
                    &structural_result_places,
                    &target.structural_parameters,
                    type_ids,
                    structural_types,
                    &expected_claim_arguments,
                    &primitive_local_places,
                    None,
                )?;
                let (_, boundary, _, target_scalar_parameters) = lowered_boundary_parameters
                    .iter()
                    .find(|(symbol, _, _, _)| *symbol == *target_machine)
                    .ok_or(LoweringError::Unsupported(
                        "boundary scalar call target is absent from the lowered closure",
                    ))?;
                if scalar_arguments.len() != target_scalar_parameters.len() {
                    return unsupported(
                        "boundary scalar argument count disagrees with its declaration",
                    );
                }
                let arguments = argument_evaluation::validated_values(
                    evaluated_scalar_arguments.as_deref(),
                    target_scalar_parameters,
                )?
                .iter()
                .map(|value| value.id)
                .collect();
                let call_byte_places = byte_subslices::argument_places(
                    structural_arguments,
                    &literal_places,
                    &mut next_literal_argument,
                    &staged_subslices[operation_index],
                )?;
                let kind = OperationKind::BoundaryCall {
                    boundary: *boundary,
                    arguments,
                    structural_arguments: lower_structural_arguments(
                        structural_arguments,
                        parameters,
                        &[],
                        &structural_result_places,
                        &call_byte_places,
                        &primitive_local_places,
                    )?,
                    completion_receipts: completion_receipts
                        .iter()
                        .map(|settlement| {
                            Ok(CompletionReceipt {
                                claim: lookup_claim_id(claim_bindings, settlement.claim_identity)?,
                                argument_index: settlement.argument_index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                };
                let value = ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(next_value_identity),
                    scalar_type: terminal_scalar_type(result.primitive_type)?,
                };
                next_value_identity =
                    next_value_identity
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit scalar result value identity space is exhausted",
                        ))?;
                let id = operations.allocate();
                operations.record_source_call(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "boundary scalar call statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "boundary scalar call ordinal coordinate exceeds usize",
                            )
                        })?,
                    },
                    *source_site,
                    id,
                    *target_machine,
                )?;
                operations.push(Operation {
                    static_reach_binding: None,
                    id,
                    result: terminal_psi::OperationResult::Scalar(value),
                    kind,
                });
                if staged {
                    if staged_scalar_result.replace(value).is_some() {
                        return unsupported(
                            "nested argument group produces more than one scalar binding",
                        );
                    }
                } else if !crate::emission::call_source_custody::initializers::discards_result(
                    checked,
                    plan.state,
                    *coordinate,
                )? {
                    scalar_result_values.push(value);
                }
                continue;
            }
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                source_site,
                result,
                target_machine,
                scalar_arguments,
                structural_arguments,
                completion_receipts,
                discard_result_on_return,
                ..
            } => {
                if usize::try_from(result.binding_ordinal).ok()
                    != Some(structural_result_places.len())
                {
                    return unsupported(
                        "Unit structural result binding ordinal drifted from source order",
                    );
                }
                let target = unique_unit_boundary(plans, *target_machine)?;
                let CheckedBoundaryMachineResultPlan::Structural {
                    type_identity,
                    multiplicity,
                    qualifications,
                } = &target.result
                else {
                    return unsupported(
                        "Unit structural result target is not a structural boundary",
                    );
                };
                if type_identity != &result.type_identity || multiplicity != &result.multiplicity {
                    return unsupported(
                        "Unit structural result drifted from its checked boundary target",
                    );
                }
                structural_calls::validate_consumer(
                    checked,
                    plan,
                    operation,
                    &target.structural_parameters,
                    &[],
                )?;
                let expected_claim_arguments = structural_arguments
                    .iter()
                    .enumerate()
                    .flat_map(|(argument_index, argument)| {
                        plan.entry_claims
                            .iter()
                            .filter(move |claim| {
                                argument.byte_sequence_literal().is_none()
                                    && Some(claim.parameter_index)
                                        == argument.source_parameter_index()
                                    && (argument.path.is_empty() || claim.path == argument.path)
                            })
                            .map(move |_| {
                                u32::try_from(argument_index).map_err(|_| {
                                    LoweringError::Unsupported(
                                        "boundary structural argument index exceeds u32",
                                    )
                                })
                            })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                validate_transfer_shape(
                    structural_arguments,
                    completion_receipts,
                    parameters,
                    &[],
                    &structural_result_places,
                    &target.structural_parameters,
                    type_ids,
                    structural_types,
                    &expected_claim_arguments,
                    &primitive_local_places,
                    None,
                )?;
                let (_, boundary, _, target_scalar_parameters) = lowered_boundary_parameters
                    .iter()
                    .find(|(symbol, _, _, _)| *symbol == *target_machine)
                    .ok_or(LoweringError::Unsupported(
                        "boundary structural call target is absent from the lowered closure",
                    ))?;
                if scalar_arguments.len() != target_scalar_parameters.len() {
                    return unsupported(
                        "boundary structural argument count disagrees with its declaration",
                    );
                }
                let arguments = argument_evaluation::validated_values(
                    evaluated_scalar_arguments.as_deref(),
                    target_scalar_parameters,
                )?
                .iter()
                .map(|value| value.id)
                .collect();
                let call_byte_places = byte_subslices::argument_places(
                    structural_arguments,
                    &literal_places,
                    &mut next_literal_argument,
                    &staged_subslices[operation_index],
                )?;
                let kind = OperationKind::BoundaryCall {
                    boundary: *boundary,
                    arguments,
                    structural_arguments: lower_structural_arguments(
                        structural_arguments,
                        parameters,
                        &[],
                        &structural_result_places,
                        &call_byte_places,
                        &primitive_local_places,
                    )?,
                    completion_receipts: completion_receipts
                        .iter()
                        .map(|settlement| {
                            Ok(CompletionReceipt {
                                claim: lookup_claim_id(claim_bindings, settlement.claim_identity)?,
                                argument_index: settlement.argument_index,
                            })
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?,
                };
                let id = operations.allocate();
                let structural_type = lookup_type_id(type_ids, type_identity)?;
                let result_place = place_id(allocate_dense(&mut next_place)?);
                let result_declaration = StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: id,
                        structural_type,
                    },
                };
                operations.record_source_call(
                    SourceCallCoordinate {
                        state: plan.state,
                        statement_index: usize::try_from(coordinate.statement_index).map_err(
                            |_| {
                                LoweringError::Unsupported(
                                    "boundary structural call statement coordinate exceeds usize",
                                )
                            },
                        )?,
                        call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                            LoweringError::Unsupported(
                                "boundary structural call ordinal coordinate exceeds usize",
                            )
                        })?,
                    },
                    *source_site,
                    id,
                    *target_machine,
                )?;
                operations.push(Operation {
                    static_reach_binding: None,
                    id,
                    result: OperationResult::Structural(StructuralOperationResult {
                        place: result_place,
                        structural_type,
                        multiplicity: match multiplicity {
                            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                            Multiplicity::Affine => StructuralMultiplicity::Affine,
                            Multiplicity::Linear => StructuralMultiplicity::Linear,
                        },
                        qualifications: qualifications
                            .iter()
                            .map(|domain| lookup_domain_id(domain_ids, *domain))
                            .collect::<Result<Vec<_>, _>>()?,
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind,
                });
                structural_result_places.push((result_declaration, *discard_result_on_return));
                continue;
            }
            CheckedUnitEffectOperationPlan::PortWrite {
                service_reach,
                port,
                value,
                ..
            } => {
                let direct = checked
                    .facts
                    .service_reaches
                    .rows
                    .services(service_reach.direct);
                let [port_service] = direct else {
                    return unsupported(
                        "port output does not carry the unique exact checked PortIo service",
                    );
                };
                if !checked
                    .facts
                    .service_reaches
                    .rows
                    .services(service_reach.transitive)
                    .contains(port_service)
                {
                    return unsupported(
                        "port output does not carry the unique exact checked PortIo service",
                    );
                }
                OperationKind::PortWrite {
                    // `CheckedUnitEffectOperationPlan::PortWrite` is minted only for the
                    // exact checked asm-port-out builtin. Its singleton direct row is
                    // therefore the symbol-backed PortIo authority; no spelling lookup is
                    // repeated here.
                    service: lookup_service_id(service_ids, *port_service)?,
                    port: *port,
                    value: *value,
                }
            }
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index,
                destination,
                path,
                value,
            } => {
                let destination = match destination {
                    checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                        parameter_index,
                    } => {
                        let parameter = parameters.get(*parameter_index as usize).ok_or(
                            LoweringError::Unsupported("primitive store parameter is absent"),
                        )?;
                        crate::emission::primitive_store::parameter_destination(
                            parameter,
                            path,
                            structural_types,
                        )?
                    }
                    checked_trees::CheckedPrimitiveStoreDestination::Local { symbol } => {
                        if !path.is_empty() {
                            return unsupported(
                                "primitive local store has a projected destination",
                            );
                        }
                        let local = primitive_locals::find(&primitive_local_places, *symbol)?;
                        crate::emission::primitive_store::Destination {
                            place: local.declaration.id,
                            path: Vec::new(),
                            scalar_type: local.scalar_type,
                        }
                    }
                };
                let kind = crate::emission::primitive_store::emit_assignment(
                    checked,
                    plan.machine,
                    plan.state,
                    *statement_index,
                    destination,
                    value,
                    &mut evaluation,
                    source_value_count,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut scalar_calls,
                )?;
                next_call_obligation = scalar_calls.next_obligation_identity;
                kind
            }
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                crate::emission::structural_byte_sequence_store::literal_view_type(
                    structural_types,
                )?;
                crate::emission::structural_byte_sequence_store::emit(
                    store,
                    parameters,
                    structural_types,
                    &mut literal_places,
                    &mut next_place,
                    &mut next_value_identity,
                    &mut next_call_obligation,
                    &mut operations,
                )?
            }
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
                    scalar_result_values.len(),
                )
                .with_primitive_storage(&evaluation.primitive_storage)
                .with_structural_parameters(&evaluation.structural_parameters)
                .with_resolved_structural_observations(
                    &evaluation.structural_fields,
                    &evaluation.structural_cases,
                );
                let index = bindings.expression_at(
                    checked,
                    plan.state,
                    store.statement_index,
                    CheckedScalarExpressionRole::AssignmentIndex,
                )?;
                let value = bindings.expression_at(
                    checked,
                    plan.state,
                    store.statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )?;
                crate::emission::structural_byte_sequence_index_store::emit(
                    store,
                    parameters,
                    structural_types,
                    &index,
                    &value,
                    &scalar_result_values,
                    &mut next_value_identity,
                    &mut next_call_obligation,
                    &mut operations,
                )?
            }
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => {
                let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
                    scalar_result_values.len(),
                )
                .with_primitive_storage(&evaluation.primitive_storage)
                .with_structural_parameters(&evaluation.structural_parameters);
                let index = bindings.expression_at(
                    checked,
                    plan.state,
                    write.statement_index,
                    CheckedScalarExpressionRole::AssignmentIndex,
                )?;
                let value = bindings.expression_at(
                    checked,
                    plan.state,
                    write.statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )?;
                crate::emission::byte_sequence_write::emit(
                    write,
                    parameters,
                    structural_types,
                    &index,
                    &value,
                    &scalar_result_values,
                    &mut next_value_identity,
                    &mut next_call_obligation,
                    &mut operations,
                )?
            }
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                let destination = parameters
                    .iter()
                    .find(|parameter| {
                        Some(parameter.position) == store.destination.parameter_position()
                    })
                    .ok_or(LoweringError::Unsupported(
                        "structural scalar store names an unknown parameter",
                    ))?;
                let lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                        store,
                        store.statement_index,
                        destination,
                        structural_types,
                        crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                    )?;
                let value = evaluation.field_assignment_value(
                    checked,
                    plan.machine,
                    plan.state,
                    store,
                    &mut scalar_result_values,
                    &mut next_value_identity,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut scalar_calls,
                )?;
                next_call_obligation = scalar_calls.next_obligation_identity;
                if value.scalar_type != lowered.scalar_type {
                    return unsupported("structural scalar store RHS differs from its field type");
                }
                lowered.into_operation(destination.place, value.id, &mut next_call_obligation)?
            }
            CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => {
                return unsupported("Unit return is not the final checked operation");
            }
        };
        let id = operations.allocate();
        if let Some((coordinate, source_site, target_machine)) = source_call {
            operations.record_source_call(
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
        operations.push(Operation {
            static_reach_binding: None,
            id,
            result: terminal_psi::OperationResult::Unit,
            kind,
        });
    }
    if next_literal_argument != call_literal_count {
        return unsupported("byte-sequence literal argument consumption is incomplete");
    }
    let controlled_result = if plan.scalar_control.is_some() {
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
            plan,
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
    let mut selection_roster = structural_result_places
        .iter()
        .rev()
        .map(|(place, discard)| (place.id, *discard))
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
                                path: lower_structural_path(&reference.path),
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
    let crash_routes = if let Some(contract_plan) =
        checked.facts.contract_plans.for_machine(plan.machine)
    {
        if parameters.is_empty() {
            lower_checked_crash_route_buckets(contract_plan.crash.published(), &scalar_parameters)?
        } else {
            lower_structural_crash_route_buckets(
                contract_plan.crash.published(),
                &scalar_parameters,
                &signature.predicate_parameters,
                structural_types,
                runtime_requirements,
            )?
        }
    } else {
        Vec::new()
    };
    evaluation.remap_transported_call_operands(&mut operations);
    evaluation.blocks.push(Block {
        id: block,
        parameters: evaluation.parameters,
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
        crate::scalar_graph::scalar_contracts::clauses(refined.ensures(), &namespace)?
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
            requires: runtime_requirements.clone(),
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
    })
}
