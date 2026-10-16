//! One ordinary attached Unit machine lowered from its checked effect plan:
//! `emit` establishes the provider attachment, local and literal places,
//! walks the argument schedule through `MachineEmission::lower_step`, then
//! seals completion, the cleanup roster, crash routes and the machine's
//! single block.
//!
//! `lower_step` evaluates each operation's operands on the argument schedule,
//! then sends stores, scalar locals, borrowed-storage windows, continuation
//! cleanup, structural-value construction and every internal, scalar and
//! boundary call to `operation_frame::OperationFrame`, the emitter
//! composed-graph states share. It keeps one method per remaining
//! ordinary-only kind in `locals` (primitive, reference, array and trivial
//! affine establishments).
//!
//! Checked completion rows carry contextual normal-edge dispositions for named
//! storage, including nominal hooks and linear claims. They use the same action
//! lowering as state graphs; anonymous results and trivial homes keep their
//! independently recorded production/disposal contracts.
//! A named binding alone does not replace production cleanup: only recorded
//! contextual actions at its current home do. A borrowed referent can leave an
//! affine descriptor to retire without any owned-storage StateExit action.
//! The emitted ownership frontier chooses actual edges after scalar control exists.
//! A selected operand may transfer a parameter on only one branch, so a static
//! final-return discard cannot also own that parameter's cleanup.

use super::bodies::{UnitBody, UnitPlans};
use super::catalog::lower_provider_candidate_service_ceiling;
use super::composed_control::callable::EmissionCounters;
use super::operation_frame::{
    CallInputs, Callees, CallerCustody, OperationFrame, StructuralResults, StructuralTypeRoster,
};
use super::parameters::lower_declared_service_reach;
use super::parameters::lower_installation_machine_service_ceiling;
use super::provider_attachments::lower_provider_attachment_places;
use super::signatures::MachineSignature;
use super::{
    CheckedUnitProviderCandidate, argument_evaluation, argument_schedule, byte_subslices,
    parameters, primitive_locals,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use crate::unit::{
    Block, CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    LoweringError, MachineContract, Operation, PlaceId, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, TerminalMachine, TerminalMachineResult, Terminator,
    ValueDeclaration, allocate_dense, content_conservation, contract_id, dense_identity, edge_id,
    lookup_type_id, lower_checked_crash_route_buckets, lower_structural_crash_route_buckets,
    place_id, terminal_scalar_type, unsupported, value_id,
};
use crate::unit::{
    MachineId, ServiceId, ServiceReachId, StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
use lowered_psi::{
    LoweredSelectedIeeeFloatComparisonOccurrence, LoweredSelectedIntegerComparisonOccurrence,
    LoweredSourceCallOccurrence,
};

/// What the closure keeps for every ordinary machine emitter: the identity
/// rosters, boundary parameters, signatures and requirement counts, plus the
/// checked plans and closure roster that calls resolve against. The structural
/// type roster travels separately because a body may extend it.
#[derive(Clone, Copy)]
pub(super) struct ClosureCatalog<'a> {
    pub(super) type_ids: &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    pub(super) domain_ids: &'a [StructuralDomainBinding],
    pub(super) service_ids: &'a [(ServiceReachId, ServiceId)],
    pub(super) boundary_parameters: &'a [super::operation_frame::BoundaryParameters],
    pub(super) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub(super) signatures: &'a [MachineSignature],
    pub(super) scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    pub(super) plans: UnitPlans<'a>,
    pub(super) closure: &'a [symbols::SymbolHandle],
    pub(super) provider_candidate_plans: &'a [CheckedUnitProviderCandidate],
    /// The closure's scalar qualification namespace; a call's emitted
    /// result republishes the callee's qualified result through it.
    pub(super) prepared_scalar_qualifications:
        &'a crate::expression_preparation::qualifications::PreparedScalarQualifications,
}

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
    /// This machine's allocated formals: its erased lanes are the namespace
    /// its calls' erased actuals lower in.
    signature: &'a MachineSignature,
    parameters: &'a Vec<StructuralParameterDeclaration>,
    scalar_parameter_count: usize,
    structural_types: &'a mut Vec<StructuralTypeDeclaration>,
    type_ids: &'a [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    domain_ids: &'a [StructuralDomainBinding],
    service_ids: &'a [(ServiceReachId, ServiceId)],
    lowered_boundary_parameters: &'a [super::operation_frame::BoundaryParameters],
    machine_signatures: &'a [MachineSignature],
    closure: &'a [symbols::SymbolHandle],
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
    structural_result_places: Vec<(Option<StructuralPlaceDeclaration>, bool)>,
    local_places: Vec<StructuralPlaceDeclaration>,
    private_places: Vec<StructuralPlaceDeclaration>,
    next_literal_argument: usize,
    staged_arguments: Vec<Vec<usize>>,
    staged_subslices: Vec<Vec<(usize, PlaceId)>>,
    staged_selectors: Vec<Vec<usize>>,
    subslice_places: Vec<StructuralPlaceDeclaration>,
    retained_scalar_prefix: Option<usize>,
    scalar_calls: CallEmissionContext<'a>,
}

/// What one scheduled operation hands the lowering methods.
pub(super) struct StepInputs {
    pub(super) operation_index: usize,
    pub(super) staged: bool,
    pub(super) finishes_argument_group: bool,
    pub(super) source_value_count: usize,
    pub(super) evaluated_scalar_arguments: Option<Vec<ValueDeclaration>>,
    pub(super) evaluated_selectors: Option<Vec<ValueDeclaration>>,
}

/// The lowered machine and the occurrence rows the closure accumulates.
pub(super) struct EmittedMachine {
    pub(super) scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
    /// Qualification coercions the machine's call-argument expansions minted,
    /// merged into the module's shared qualification catalog at publication.
    pub(super) qualification_coercions: Vec<terminal_psi::ScalarQualificationCoercion>,
    pub(super) machine: TerminalMachine,
    pub(super) source_calls: Vec<LoweredSourceCallOccurrence>,
    pub(super) quotient_adaptations:
        Vec<crate::emission::operation_emission::buffer::SourceQuotientAdaptation>,
    pub(super) quotient_calls: Vec<crate::emission::operation_emission::buffer::SourceQuotientCall>,
    pub(super) selected_ieee_float_comparisons: Vec<LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(super) selected_integer_comparisons: Vec<LoweredSelectedIntegerComparisonOccurrence>,
    pub(super) source_checked_obligations: Vec<semantic_vocabulary::ObligationId>,
}

/// Emit one ordinary machine. Counters advance only when emission succeeds;
/// a failed emission fails the whole closure.
pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        prepared_scalar_qualifications,
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
    let mut next_call_obligation = *call_obligation_counter;
    let parameters = &signature.parameters;
    let scalar_parameters = signature.scalar_parameters.clone();
    let scalar_parameter_count = scalar_parameters.len();
    let runtime_requirements = &signature.runtime_requirements;
    let entry_claims = &signature.claims.entry_claims;
    let claim_bindings = &signature.claims.source_claims;
    let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
        identities,
        checked,
        structural_types,
        &plan.structural_parameters,
        parameters,
        &plan.entry_claims,
        claim_bindings,
    )?;
    let provider_boundaries = lowered_boundary_parameters
        .iter()
        .map(|boundary| (boundary.source, boundary.id))
        .collect::<Vec<_>>();
    let attachment = plan
        .attachment_type_identity
        .map(|identity| lookup_type_id(type_ids, &identity))
        .transpose()?;
    let provider_places = if let Some(attachment) = attachment {
        let attachment_declaration =
            terminal_psi::find_structural_type(structural_types, attachment)
                .expect("lowered attachment declaration exists");
        lower_provider_attachment_places(
            identities,
            checked,
            attachment,
            attachment_declaration,
            structural_types,
            &plan.provider_attachment_requirements,
            &provider_boundaries,
            &mut next_place,
        )?
    } else {
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
    let (private_places, call_literal_count, mut operations) = establish_call_literals(
        identities,
        checked,
        plan,
        plans,
        lowered_boundary_parameters,
        type_ids,
        domain_ids,
        structural_types,
        &mut next_place,
        &mut next_value,
        &mut next_call_obligation,
        next_operation,
    )?;
    let next_literal_argument = 0usize;
    let next_value_identity = next_value;
    let scalar_result_values = scalar_parameters.clone();
    let parameter_storage = primitive_locals::establish_parameter_storage(
        &plan.parameter_storage,
        &plan.scalar_parameters,
        &scalar_parameters,
        type_ids,
        structural_types,
        &mut next_place,
        &mut next_call_obligation,
        &mut operations,
    )?;
    let primitive_local_places = parameter_storage
        .iter()
        .map(|storage| primitive_locals::PrimitiveLocal {
            symbol: storage.local.symbol,
            declaration: storage.local.declaration,
        })
        .collect::<Vec<_>>();
    let structural_result_places = Vec::<(Option<StructuralPlaceDeclaration>, bool)>::new();
    let mut evaluation = argument_evaluation::Evaluation::new(&mut next_block)?;
    evaluation.shared_qualifications = prepared_scalar_qualifications.clone();
    evaluation.machine = Some(terminal_machine);
    evaluation.erased_scalar_formals = signature.erased_scalar_parameters.clone();
    evaluation.place_observations =
        crate::scalar_lowering::computations::fields::prepare_place_observations(
            identities,
            checked,
            plan.machine,
            plan.state,
            structural_types,
        );
    evaluation.record_fields = crate::scalar_lowering::computations::fields::prepare(
        identities,
        checked,
        plan.machine,
        &[plan.machine],
        structural_types,
        &mut next_place,
    )?;
    evaluation.literals = crate::scalar_lowering::computations::literals::prepare(
        identities,
        checked,
        plan.machine,
        structural_types,
        &mut next_place,
    )?;
    evaluation.copies = crate::scalar_lowering::computations::copies::prepare(
        identities,
        checked,
        plan.machine,
        structural_types,
        &mut next_place,
    )?;
    evaluation.produced_values = crate::scalar_lowering::computations::produced_values::prepare(
        identities,
        checked,
        &[plan.machine],
        structural_types,
        &mut next_place,
    )?;
    evaluation.subslices = crate::scalar_lowering::computations::subslices::prepare(
        identities,
        checked,
        &[plan.machine],
        structural_types,
        &mut next_place,
    )?;
    evaluation.cases = crate::scalar_lowering::computations::cases::prepare(
        identities,
        checked,
        &[plan.machine],
        structural_types,
        &mut next_place,
    )?;
    evaluation.bind_structural_parameters(
        plan.structural_parameters
            .iter()
            .zip(parameters)
            .map(|(source, parameter)| (source.position, parameter.clone()))
            .collect(),
        structural_types,
    );
    evaluation.primitive_storage =
        crate::unit::attached_unit::parameters::primitive_reference_storage(
            checked,
            plan.state,
            &plan.structural_parameters,
            &evaluation.structural_parameters,
        )?;
    // An ordinary body is one state: its namespace starts, like a composed
    // state's prefix, from its scalar formals before the first statement.
    // Authored locals then stay separate from anonymous operation results,
    // captured observations and staged argument cells, none of which takes a
    // source ordinal.
    evaluation.scalar_bindings = crate::expression_preparation::bindings::ScalarBindings::entry(
        scalar_parameters.iter().map(|value| value.scalar_type),
        &evaluation.primitive_storage,
        &evaluation.structural_parameters,
        structural_types,
        &evaluation.place_observations,
    );
    primitive_locals::register_mut_formal_storage_homes(&mut evaluation, &parameter_storage)?;
    let staged_arguments = vec![Vec::<usize>::new(); plan.operations.len()];
    let staged_subslices = vec![Vec::<(usize, PlaceId)>::new(); plan.operations.len()];
    let subslice_places = Vec::new();
    let retained_scalar_prefix = None;
    let cleanup_receivers = machine_signatures
        .iter()
        .filter_map(|signature| {
            signature
                .cleanup_receiver
                .map(|receiver| (signature.source, receiver))
        })
        .collect::<Vec<_>>();
    let call_structural_types = structural_types.clone();
    let mut emission = MachineEmission {
        checked,
        plan,
        plans,
        signature,
        parameters,
        scalar_parameter_count,
        borrowed_windows: Default::default(),
        structural_types,
        type_ids,
        domain_ids,
        service_ids,
        lowered_boundary_parameters,
        machine_signatures,
        closure,
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
        local_places,
        private_places,
        next_literal_argument,
        staged_arguments,
        staged_subslices,
        staged_selectors: vec![Vec::new(); plan.operations.len()],
        subslice_places,
        retained_scalar_prefix,
        scalar_calls: CallEmissionContext {
            terminal_machine,
            domain_ids,
            claims: crate::emission::claim_bindings::ClaimBindings::new(
                claim_bindings.clone(),
                dense_identity(claim_bindings.len())?,
            ),
            cleanup_receivers: &cleanup_receivers,
            machine_ids,
            signatures: machine_signatures,
            structural_types: &call_structural_types,
            requirement_counts: scalar_requirement_counts,
            boundaries: lowered_boundary_parameters,
            next_obligation_identity: next_call_obligation,
            obligation_limit: u64::MAX,
        },
    };
    let mut schedule = argument_schedule::build(checked, plan)?
        .into_iter()
        .peekable();
    while let Some(step) = schedule.next() {
        let finishes_argument_group = matches!(schedule.peek(), Some(argument_schedule::Step::End));
        emission.lower_step(identities, step, finishes_argument_group)?;
        if emission.operations.external_completion.is_some() {
            break;
        }
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
        local_places,
        mut private_places,
        next_literal_argument,
        subslice_places,
        borrowed_windows,
        mut scalar_calls,
        ..
    } = emission;
    // Open borrowed holes can affect surviving owners. External completion
    // grants no crossing-safety admission, so keep that missing case fail-closed
    // instead of inventing restoration or cleanup on an abandoned path.
    borrowed_windows.require_closed()?;

    if next_literal_argument != call_literal_count {
        return unsupported("byte-sequence literal argument consumption is incomplete");
    }
    let crash_routes = {
        let effective = crate::unit::effective_crash_routes(checked, plan.machine)?;
        if parameters.is_empty() {
            lower_checked_crash_route_buckets(identities, &effective, &scalar_parameters)?
        } else {
            lower_structural_crash_route_buckets(
                identities,
                &effective,
                &scalar_parameters,
                &signature.predicate_parameters,
                structural_types,
                runtime_requirements,
            )?
        }
    };
    evaluation.remap_transported_call_operands(&mut operations);
    let mut external_result = None;
    let mut parameter_cleanup = Vec::new();
    let (scalar_return, structural_return) = if let Some(terminator) =
        operations.take_external_completion(&mut next_edge)?
    {
        external_result = Some(if let Some(result) = &plan.structural_result {
            TerminalMachineResult::Structural(lower_structural_result_declaration(
                identities,
                checked,
                plan,
                result,
                parameters,
                structural_types,
                type_ids,
                domain_ids,
                &mut next_place,
            )?)
        } else if let Some(primitive_type) = plan
            .scalar_result
            .as_ref()
            .map(|result| result.primitive_type)
            .or_else(|| {
                plan.scalar_control
                    .as_ref()
                    .map(|control| control.primitive_type)
            })
        {
            TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: super::parameters::unit_scalar_result_type(
                    checked,
                    plan.state,
                    terminal_scalar_type(primitive_type)?,
                    prepared_scalar_qualifications,
                )?
                .qualifications,
                id: value_id(allocate_dense(&mut next_value_identity)?),
                scalar_type: terminal_scalar_type(primitive_type)?,
            })
        } else {
            TerminalMachineResult::Unit
        });
        // Result declarations describe the machine's normal-return contract,
        // not a runtime producer on this abandoned path.
        // Abandonment seals this path without evaluating a later result,
        // manufacturing normal receipts, or scheduling implicit cleanup.
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator,
        });
        next_value = next_value_identity;
        (None, None)
    } else if let Some(control) = &plan.scalar_control {
        // Each selected return owns its checked exit transaction. No cleanup
        // is reconstructed from entry multiplicity or repeated at the join.
        scalar_calls.next_obligation_identity = next_call_obligation;
        let Some(CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_local_discard_ordinals,
            ..
        }) = plan.operations.last()
        else {
            return unsupported("scalar control sequence has no checked completion plan");
        };
        let trivial_affine_discards = trivial_affine_local_discard_ordinals
            .iter()
            .map(|ordinal| {
                local_places
                    .get(*ordinal as usize)
                    .map(|local| local.id)
                    .ok_or(LoweringError::Unsupported(
                        "scalar control local cleanup ordinal is not dense",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let claim_bindings = scalar_calls.claims.bindings().to_vec();
        let mut cleanup_catalog = super::completion_cleanup::CompletionCleanupCatalog {
            control: &checked.facts.flow.terminal_unit_effects.cleanup_control,
            types: structural_types,
            type_ids,
            calls: &mut scalar_calls,
        };
        let cleanups = super::completion_cleanup::scalar_returns(
            identities,
            &claim_bindings,
            checked,
            super::completion_cleanup::NormalExitSource {
                cleanups: checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .cleanup_control
                    .normal_exits(plan.normal_exit_cleanups),
                trivial_affine_discards: &trivial_affine_discards,
                operations: &plan.operations,
                parameters: &plan.structural_parameters,
            },
            control,
            &mut cleanup_catalog,
            parameters,
            &evaluation,
            &operations,
        )?;
        let source = evaluation.scalar_control_result(
            identities,
            checked,
            plan.machine,
            plan.state,
            control,
            &cleanups,
            &mut scalar_result_values,
            &mut next_value_identity,
            &mut next_block,
            &mut next_edge,
            &mut operations,
            &mut scalar_calls,
        )?;
        next_call_obligation = scalar_calls.next_obligation_identity;
        let declaration = ValueDeclaration {
            qualifications: super::parameters::unit_scalar_result_type(
                checked,
                plan.state,
                source.scalar_type,
                prepared_scalar_qualifications,
            )?
            .qualifications,
            id: value_id(allocate_dense(&mut next_value_identity)?),
            scalar_type: source.scalar_type,
        };
        next_value = next_value_identity;
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: Terminator::Return {
                edge: edge_id(allocate_dense(&mut next_edge)?),
                value: source.id,
                cleanup_actions: Vec::new(),
            },
        });
        (Some((source.id, declaration)), None)
    } else {
        let Some(CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
            ..
        }) = plan.operations.last()
        else {
            return Err(LoweringError::Unsupported(
                "returning Unit sequence has no checked completion plan",
            ));
        };
        let cleanup_control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
        let [normal_exit] = cleanup_control.normal_exits(plan.normal_exit_cleanups) else {
            return unsupported("ordinary sequence return has no unique source cleanup coordinate");
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
        // Preserve the checker's disposal authority, but delegate its scheduling
        // to the complete operation graph. Entry order is declaration order;
        // affine_cleanup reverses it for cleanup and reconciles selected transfers.
        let current_parameters = evaluation.current_parameters(parameters);
        parameter_cleanup = trivial_affine_discards
            .iter()
            .map(|parameter_index| {
                current_parameters
                    .get(usize::try_from(*parameter_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "Unit affine discard parameter index exceeds usize",
                        )
                    })?)
                    .cloned()
                    .ok_or(LoweringError::Unsupported(
                        "Unit affine discard has an invalid parameter index",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        parameter_cleanup.sort_by_key(|parameter| parameter.position);
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
        // Resolve contextual disposal against the current storage before
        // constructing the production roster. A name is not disposal
        // authority: reference descriptors can owe affine retirement without
        // an owned-storage StateExit row. Only an actual contextual action
        // replaces their independently recorded production cleanup.
        let claim_bindings = scalar_calls.claims.bindings().to_vec();
        let mut cleanup_catalog = super::completion_cleanup::CompletionCleanupCatalog {
            control: cleanup_control,
            types: structural_types,
            type_ids,
            calls: &mut scalar_calls,
        };
        let normal_regions = super::completion_cleanup::for_exit_regions(
            identities,
            &claim_bindings,
            checked,
            super::completion_cleanup::NormalExitSource {
                cleanups: cleanup_control.normal_exits(plan.normal_exit_cleanups),
                // Existing local eligibility keeps its establishment order
                // in the ordinary completion roster below.
                trivial_affine_discards: &[],
                operations: &plan.operations,
                parameters: &plan.structural_parameters,
            },
            normal_exit.exit,
            &mut cleanup_catalog,
            parameters,
            &evaluation,
            &operations,
        )?;
        let mut selection_roster = structural_result_places
            .iter()
            .rev()
            .filter_map(|(place, discard)| Some((place.as_ref()?, discard)))
            .map(|(place, discard)| {
                let retired = moved_subtrees
                    .get(&evaluation.current_structural_place(place.id))
                    .is_some_and(|moved| {
                        projected_moves::retires(
                            structural_types,
                            &operations.operations,
                            place,
                            moved,
                        )
                    });
                // Contextual actions own the current home's exact partition.
                // Keep selection source rows, but do not give that storage a
                // second automatic death from its original production row.
                let contextual_disposal = normal_regions.iter().any(|region| {
                    region.action.place() == evaluation.current_structural_place(place.id)
                });
                (place.id, *discard && !retired && !contextual_disposal)
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
            .collect::<Result<Vec<_>, _>>()?;
        let mut scalar_return = plan
            .scalar_result
            .as_ref()
            .map(|result| {
                let position = scalar_parameter_count
                    .checked_add(result.binding_ordinal as usize)
                    .ok_or(LoweringError::Unsupported(
                        "scalar completion binding position overflows",
                    ))?;
                let position = evaluation.scalar_bindings.immutable_position(position)?;
                let source =
                    scalar_result_values
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
                        qualifications: super::parameters::unit_scalar_result_type(
                            checked,
                            plan.state,
                            scalar_type,
                            prepared_scalar_qualifications,
                        )?
                        .qualifications,
                        id: value_id(allocate_dense(&mut next_value_identity)?),
                        scalar_type,
                    },
                ))
            })
            .transpose()?;
        // Completion reserves its own scalar result identity after the body.
        // Include it before the next helper borrows the shared value allocator.
        next_value = next_value_identity;
        let structural_return = plan
            .structural_result
            .as_ref()
            .map(|result| {
                let source = match result.source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        structural_result_places
                            .get(binding_ordinal as usize)
                            .ok_or(LoweringError::Unsupported(
                                "returned structural binding is absent",
                            ))?
                            .0
                            .as_ref()
                            // Returning a projected owned alias would forward the
                            // root place and silently widen the returned subtree
                            // to the whole root.
                            .ok_or(LoweringError::Unsupported(
                                "returned structural binding is a projected alias",
                            ))?
                            .id
                    }
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                        evaluation
                            .structural_parameters
                            .get(parameter_index as usize)
                            .map(|(_, parameter)| parameter)
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
                let returned_claims = super::claims::lower_return_claims(
                    identities,
                    checked,
                    plan.machine,
                    plan.state,
                    result,
                    &operations,
                    scalar_calls.claims.bindings(),
                    lookup_type_id(type_ids, &result.type_identity)?,
                    structural_types,
                )?;
                Ok::<_, LoweringError>((
                    source,
                    lower_structural_result_declaration(
                        identities,
                        checked,
                        plan,
                        result,
                        parameters,
                        structural_types,
                        type_ids,
                        domain_ids,
                        &mut next_place,
                    )?,
                    returned_claims,
                ))
            })
            .transpose()?;
        let edge = edge_id(allocate_dense(&mut next_edge)?);
        let mut cleanup_regions = Vec::new();
        for place in trivial_affine_discards {
            cleanup_regions.extend(evaluation.normal_discard_regions(place, structural_types)?);
        }
        cleanup_regions.extend(normal_regions);
        let cleanup_actions = if cleanup_regions
            .iter()
            .any(|region| !region.conditions.is_empty() || region.selected_region.is_some())
        {
            // Scalar return operands are already evaluated. Carry their actual
            // SSA values through cleanup rather than observing disposed storage
            // again. Structural completion accepts whole owned sources only;
            // projected aliases were rejected above, and the checked partition
            // excludes the returned owner from disposal.
            let return_position = scalar_return
                .as_ref()
                .map(|(source, _)| {
                    scalar_result_values
                        .iter()
                        .position(|value| value.id == *source)
                        .ok_or(LoweringError::Unsupported(
                            "cleanup lost its captured scalar return",
                        ))
                })
                .transpose()?;
            evaluation.continue_cleanup_regions(
                identities,
                checked,
                cleanup_regions,
                &mut scalar_result_values,
                &mut next_value,
                &mut next_block,
                &mut next_edge,
                &mut operations,
            )?;
            if let Some(position) = return_position {
                scalar_return
                    .as_mut()
                    .ok_or(LoweringError::Unsupported(
                        "cleanup lost its return contract",
                    ))?
                    .0 = scalar_result_values[position].id;
            }
            Vec::new()
        } else {
            cleanup_regions
                .into_iter()
                .map(|region| region.action)
                .collect()
        };
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: if let Some((source, _)) = &scalar_return {
                Terminator::Return {
                    edge,
                    value: *source,
                    cleanup_actions,
                }
            } else if let Some((source, _, returned_claims)) = &structural_return {
                Terminator::ReturnStructural {
                    edge,
                    source: *source,
                    returned_claims: returned_claims.clone(),
                    cleanup_actions,
                }
            } else {
                Terminator::ReturnUnit {
                    edge,
                    cleanup_actions,
                }
            },
        });
        evaluation.blocks.sort_by_key(|block| block.id);
        (scalar_return, structural_return)
    };
    // Operand evaluation can reserve continuation blocks before their parents
    // are sealed. Both normal return and external completion publish identity
    // order, not emission order; abandonment must not skip this common seal.
    evaluation.blocks.sort_by_key(|block| block.id);
    next_operation = operations.next_identity;
    let completes_normally = external_result.is_none();
    let declared_result = external_result.unwrap_or_else(|| {
        if let Some((_, result)) = scalar_return {
            TerminalMachineResult::Scalar(result)
        } else {
            structural_return
                .as_ref()
                .map_or(TerminalMachineResult::Unit, |(_, result, _)| {
                    TerminalMachineResult::Structural(result.clone())
                })
        }
    });
    crate::emission::structural_byte_sequence_store::retain_literal_declarations(
        &operations,
        &mut private_places,
    )?;
    let computed_array_places = evaluation
        .blocks
        .iter()
        .flat_map(|block| {
            crate::scalar_lowering::computations::arrays::declarations(&block.operations).chain(
                crate::scalar_lowering::computations::cases::declarations(&block.operations),
            )
        })
        .filter(|place| {
            !structural_result_places
                .iter()
                .any(|(existing, _)| existing.is_some_and(|existing| existing.id == place.id))
                && !private_places
                    .iter()
                    .any(|existing| existing.id == place.id)
        })
        .collect::<Vec<_>>();
    let OperationBuffer {
        mut claim_introductions,
        selected_cleanup_groups,
        mut scalar_block_invariants,
        mut source_calls,
        quotient_adaptations,
        quotient_calls,
        loan_completions,
        mut selected_ieee_float_comparisons,
        mut selected_integer_comparisons,
        mut source_checked_obligations,
        ..
    } = operations;
    let mut structural_places = parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .chain(provider_places.iter().copied())
        .chain(local_places.iter().copied())
        .chain(primitive_local_places.iter().map(|local| local.declaration))
        .chain(private_places.iter().copied())
        .chain(subslice_places.iter().copied())
        .chain(
            structural_result_places
                .iter()
                .filter_map(|(place, _)| place.as_ref())
                .filter(|place| {
                    !matches!(place.kind, StructuralPlaceKind::ByteSequenceLiteral { .. })
                })
                .copied(),
        )
        .chain(computed_array_places)
        .collect::<Vec<_>>();
    crate::emission::operation_emission::place_declarations::retain_block_parameters(
        evaluation.blocks.iter(),
        &mut structural_places,
    )?;
    crate::emission::operation_emission::place_declarations::retain_operation_results(
        evaluation.blocks.iter().flat_map(|block| &block.operations),
        &mut structural_places,
    )?;
    // Argument-time view producers interleave with reserved call results.
    // Declaration order is canonical identity order, not execution order.
    // The computation scan also encounters subslices whose ordinary call
    // or edge already owns a declaration. Reuse only that exact declaration;
    // a reused identity with different provenance remains an error.
    crate::scalar_lowering::computations::subslices::merge_declarations(
        evaluation.blocks.iter(),
        &mut structural_places,
    )?;
    structural_places.sort_by_key(|place| place.id);
    if let TerminalMachineResult::Structural(result) = &declared_result {
        structural_places.push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::Result,
        });
    }
    let normal_guarantees = super::normal_guarantees::lower(
        identities,
        checked,
        plan.state,
        &scalar_parameters,
        &declared_result,
        &signature.erased_scalar_parameters,
        &crate::scalar_lowering::contracts::ContractViewNamespace {
            parameters: &evaluation.structural_parameters,
            result: None,
            structural_types,
            element_views: &evaluation.element_views,
        },
        &mut next_call_obligation,
    )?;
    let nominal_cleanup_subject =
        crate::machine_lowering::nominal_cleanup_subject(checked, plan.machine, type_ids)?;
    let attachment = attachment.or(nominal_cleanup_subject);
    let mut machine = TerminalMachine {
        declaration: None,
        closed_reach_application: None,
        declared_service_reach: lower_declared_service_reach(checked, plan.machine, service_ids)?,
        id: terminal_machine,
        attachment,
        nominal_cleanup_subject,
        parameters: scalar_parameters.clone(),
        structural_parameters: parameters.clone(),
        ranked_scc: None,
        result: declared_result,
        structural_places,
        entry_claims: entry_claims.clone(),
        block_entry_claims: Vec::new(),
        loan_completions,
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
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    identities,
                    checked,
                    &signature.erased_proof_parameters,
                ),
            requires: signature.requires.clone(),
            source_checked_requires: signature.source_checked_requires.clone(),
            ensures: normal_guarantees,
            outcome_specific_ensures: Vec::new(),
        },
    };
    crate::emission::continuation_specialization::complete(
        &mut machine,
        &selected_cleanup_groups,
        structural_types,
        crate::emission::continuation_specialization::CompanionRows {
            continuation_value_uses: &[],
            claim_introductions: &mut claim_introductions,
            source_calls: &mut source_calls,
            scalar_block_invariants: &mut scalar_block_invariants,
            qualification_coercions: &mut evaluation.qualification_coercions,
            selected_ieee_float_comparisons: &mut selected_ieee_float_comparisons,
            selected_integer_comparisons: &mut selected_integer_comparisons,
            source_checked_obligations: &mut source_checked_obligations,
        },
        crate::emission::continuation_specialization::Counters {
            next_place: &mut next_place,
            next_value: &mut next_value,
            next_block: &mut next_block,
            next_edge: &mut next_edge,
            next_operation: &mut next_operation,
            next_obligation: &mut next_call_obligation,
        },
    )?;
    // Nominal continuation specialization precedes the independent no-code
    // carrier reconciliation. Both consume the same complete declaration roster.
    if completes_normally {
        crate::emission::affine_cleanup::complete(
            &parameter_cleanup,
            &machine.structural_parameters,
            &crate::emission::affine_cleanup::no_code_type_ids(checked, type_ids),
            &machine.structural_places,
            machine.entry,
            &mut machine.blocks,
            structural_types,
        )?;
    }
    *place_counter = next_place;
    *value_counter = next_value;
    *block_counter = next_block;
    *operation_counter = next_operation;
    *edge_counter = next_edge;
    *call_obligation_counter = next_call_obligation;
    Ok(EmittedMachine {
        scalar_block_invariants,
        qualification_coercions: evaluation.qualification_coercions,
        machine,
        source_calls,
        quotient_adaptations,
        quotient_calls,
        selected_ieee_float_comparisons,
        selected_integer_comparisons,
        source_checked_obligations,
    })
}

/// Publish the checked normal-result carrier without evaluating its source.
/// A declaration remains necessary when every emitted path abandons the domain.
#[allow(clippy::too_many_arguments)]
fn lower_structural_result_declaration(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    result: &checked_trees::CheckedUnitStructuralReturnPlan,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    domain_ids: &[StructuralDomainBinding],
    next_place: &mut u64,
) -> Result<terminal_psi::StructuralResultDeclaration, LoweringError> {
    let place = place_id(allocate_dense(next_place)?);
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    let state = typed_trees::state::state_by_symbol(
        checked
            .typed
            .machine_states(checked.typed.machine(plan.machine).ok_or(
                LoweringError::Unsupported("structural result has no exact machine"),
            )?),
        plan.state,
    )
    .ok_or(LoweringError::Unsupported(
        "structural result has no exact state",
    ))?;
    // Completion already captured the complete normal-result origin relation.
    // Lowering publishes that plan, never reconstructs it from old initializers.
    let return_type = state.return_type;
    // A byte view result's inside-borrow domains ride the same peeled
    // carrier identity the result spells, so the machine publishes them
    // as its own qualifications. An authored `ensures result in <domain>`
    // clause transfers the same membership to the caller — a boundary
    // signature already publishes it as the constrained result carrier's
    // qualification roster, so the produced result declaration spells it
    // identically rather than erasing the clause for a non-scalar fact.
    let mut qualifications =
        validation::view_result_carrier_qualifications(&checked.typed, return_type)
            .map_err(LoweringError::Unsupported)?
            .iter()
            .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, structural_type))
            .collect::<Result<Vec<_>, _>>()?;
    qualifications.extend(
        parameters::authored_result_qualification_domains(checked, plan.machine)
            .iter()
            .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, structural_type))
            .collect::<Result<Vec<_>, _>>()?,
    );
    qualifications.sort();
    qualifications.dedup();
    Ok(terminal_psi::StructuralResultDeclaration {
        place,
        structural_type,
        multiplicity: crate::terminal_identities::structural_multiplicity(result.multiplicity),
        qualifications,
        projected_qualifications: parameters::lower_projected_qualifications(
            identities,
            &validation::structural_result_projected_qualifications(&checked.typed, return_type)
                .map_err(LoweringError::Unsupported)?,
            domain_ids,
            structural_type,
            structural_types,
        )?,
        reference_sources: parameters::lower_reference_result_sources(
            identities,
            checked,
            &result.reference_sources,
            structural_type,
            structural_types,
            parameters,
        )?,
    })
}

/// Establish every byte-sequence literal the body passes as a structural
/// call argument: one private place per literal, qualified with the domain
/// memberships the target parameter declares, and one establishment
/// materialization ahead of the body. Argument destinations remain a dense
/// prefix, followed by supporting literal places for fresh owned backing.
/// Returns the declarations, prefix length and establishment operations.
fn establish_call_literals(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    plans: UnitPlans<'_>,
    lowered_boundary_parameters: &[super::operation_frame::BoundaryParameters],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    domain_ids: &[StructuralDomainBinding],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    next_operation: u64,
) -> Result<(Vec<StructuralPlaceDeclaration>, usize, OperationBuffer), LoweringError> {
    let mut literal_arguments = Vec::new();
    for operation in &plan.operations {
        let Some(call) = operation.call() else {
            continue;
        };
        let target_machine = call.target.machine;
        literal_arguments.extend(
            call.structural_arguments
                .iter()
                .enumerate()
                .filter(|(_, argument)| argument.byte_sequence_literal().is_some())
                .map(|(index, argument)| (argument, target_machine, index)),
        );
        // Only the emitted straight-line prefix owns literal establishments.
        // A terminal invocation has no operand schedule or effects after it.
        if matches!(
            call.completion(),
            semantic_vocabulary::BoundaryCompletion::External(_)
        ) {
            break;
        }
    }
    let mut operand_places = Vec::new();
    let operation_identity_base =
        next_operation
            .checked_sub(1)
            .ok_or(LoweringError::Unsupported(
                "terminal operation identity starts at one",
            ))?;
    let mut operations = OperationBuffer::new(
        operation_identity_base,
        std::sync::Arc::new(crate::unit::dynamic_dispatch::DynamicDispatchIndex::new(
            checked,
        )),
    );
    let mut private_places = literal_arguments
        .iter()
        .map(|(argument, target_machine, argument_index)| {
            // The literal occurrence replays the domain memberships checking
            // discharged on its bytes: the target parameter's declared
            // qualifications are exactly the admitted set.
            let qualifications = match lowered_boundary_parameters
                .iter()
                .find(|boundary| boundary.source == *target_machine)
            {
                Some(boundary) => boundary
                    .structural
                    .get(*argument_index)
                    .ok_or(LoweringError::Unsupported(
                        "literal call boundary target parameter is absent",
                    ))?
                    .qualifications
                    .clone(),
                // A Unit body or a scalar callee: the same entry roster
                // `CheckedScalarCallee` resolves for every scalar target.
                None if UnitBody::contains(plans, *target_machine) => {
                    UnitBody::find(plans, *target_machine)?
                        .entry()?
                        .structural_parameters
                        .get(*argument_index)
                        .ok_or(LoweringError::Unsupported(
                            "literal call target parameter is absent",
                        ))?
                        .qualifications
                        .iter()
                        .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, lookup_type_id(type_ids, &argument.type_identity)?))
                        .collect::<Result<Vec<_>, _>>()?
                }
                None => {
                    crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                        checked,
                        *target_machine,
                    )?
                    .structural_parameters()
                    .get(*argument_index)
                    .ok_or(LoweringError::Unsupported(
                        "literal call target parameter is absent",
                    ))?
                    .qualifications
                    .iter()
                    .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, lookup_type_id(type_ids, &argument.type_identity)?))
                    .collect::<Result<Vec<_>, _>>()?
                }
            };
            crate::emission::structural_byte_sequence_store::emit_call_literal(
                identities,
                argument.byte_sequence_literal().ok_or(LoweringError::Unsupported("literal call payload is absent"))?,
                lookup_type_id(type_ids, &argument.type_identity)?,
                qualifications,
                structural_types,
                &mut operand_places,
                next_place,
                next_value,
                next_obligation,
                &mut operations,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let argument_count = private_places.len();
    operand_places.retain(|place| {
        !private_places
            .iter()
            .any(|argument| argument.id == place.id)
    });
    private_places.extend(operand_places);
    Ok((private_places, argument_count, operations))
}

impl MachineEmission<'_> {
    /// Lower one scheduled step: argument staging bookkeeping, a continuation
    /// cleanup, or one operation dispatched to its kind's method and pushed
    /// with its source-call record.
    fn lower_step(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        step: argument_schedule::Step,
        finishes_argument_group: bool,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        self.scalar_calls.next_obligation_identity = self.next_call_obligation;
        let (operation_index, staged) = match step {
            argument_schedule::Step::CaptureBefore(statement) => {
                self.evaluation.prepare_storage_before(
                    identities,
                    self.checked,
                    self.plan.state,
                    statement,
                    self.structural_types,
                    &self.scalar_result_values,
                    &mut self.next_place,
                    &mut self.next_value_identity,
                    &mut self.operations,
                    &mut self.private_places,
                )?;
                return Ok(());
            }
            argument_schedule::Step::Begin => {
                if self
                    .retained_scalar_prefix
                    .replace(self.scalar_result_values.len())
                    .is_some()
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
                // Argument temporaries are never source-local bindings; a
                // completed call already published its result past them.
                self.scalar_result_values.truncate(prefix);
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
                    identities,
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
            argument_schedule::Step::Selector {
                operation,
                argument,
                segment,
            } => {
                let source_value_count =
                    self.retained_scalar_prefix
                        .ok_or(LoweringError::Unsupported(
                            "selector staging has no source prefix",
                        ))?;
                let value = self.evaluation.call_selector(
                    identities,
                    checked,
                    plan.machine,
                    plan.state,
                    &plan.operations[operation],
                    argument,
                    segment,
                    source_value_count,
                    &mut self.scalar_result_values,
                    &mut self.next_value_identity,
                    &mut self.next_block,
                    &mut self.next_edge,
                    &mut self.operations,
                    &mut self.scalar_calls,
                )?;
                self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                self.staged_selectors[operation].push(self.scalar_result_values.len());
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
                let bindings = self
                    .evaluation
                    .scalar_bindings
                    .clone()
                    .with_structural_parameters(&self.evaluation.structural_parameters)
                    .with_structural_observations(self.structural_types)
                    .with_primitive_storage(&self.evaluation.primitive_storage)
                    .with_structural_locals(&self.evaluation.structural_locals)
                    .with_view_locals(&self.evaluation.view_locals)
                    .with_element_views(&self.evaluation.element_views);
                let context = byte_subslices::StagingContext {
                    state: plan.state,
                    structural_types: self.structural_types,
                    type_ids: self.type_ids,
                    bindings: &bindings,
                };
                let place = byte_subslices::emit(
                    identities,
                    checked,
                    &context,
                    &plan.operations[operation],
                    ordinal,
                    &mut self.evaluation,
                    source_value_count,
                    &mut self.scalar_result_values,
                    &mut self.subslice_places,
                    &mut self.next_place,
                    &mut self.next_value_identity,
                    &mut self.next_block,
                    &mut self.next_edge,
                    &mut self.operations,
                    &mut self.scalar_calls,
                )?;
                self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                self.staged_subslices[operation].push((ordinal, place));
                return Ok(());
            }
            argument_schedule::Step::Call(index) if self.retained_scalar_prefix.is_some() => {
                (index, true)
            }
            argument_schedule::Step::Produce(index) if self.retained_scalar_prefix.is_some() => {
                (index, true)
            }
            argument_schedule::Step::Ordinary(index) if self.retained_scalar_prefix.is_none() => {
                (index, false)
            }
            _ => return unsupported("call schedule disagrees with its active argument group"),
        };
        let operation = &plan.operations[operation_index];
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::ReleaseReference {
                defer_to_exit: true,
                ..
            }
        ) {
            return Ok(());
        }
        // A continuation cleanup reads no operands; it commits the completed
        // call's dying results before the next operation evaluates any.
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
        ) {
            let source_value_count = self.scalar_result_values.len();
            return self.emit_through_frame(identities, operation, source_value_count);
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
                identities,
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
        let step = StepInputs {
            operation_index,
            staged,
            finishes_argument_group,
            source_value_count,
            evaluated_scalar_arguments,
            evaluated_selectors: staged
                .then(|| {
                    self.staged_selectors[operation_index]
                        .iter()
                        .map(|slot| {
                            self.scalar_result_values.get(*slot).copied().ok_or(
                                LoweringError::Unsupported(
                                    "staged selector lost its retained value",
                                ),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
        };
        let kind = match operation {
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::RegisterAccess { .. }
            | CheckedUnitEffectOperationPlan::Store(_)
            | CheckedUnitEffectOperationPlan::AtomicFence { .. }
            | CheckedUnitEffectOperationPlan::AtomicAccess(_)
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
            )
            | checked_trees::structural_field_write_pattern!()
            | checked_trees::byte_sequence_establishment_pattern!()
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
            )
            | checked_trees::reference_hold_pattern!() => {
                return self.emit_through_frame(identities, operation, step.source_value_count);
            }
            CheckedUnitEffectOperationPlan::Call(_) => {
                return self.emit_call_through_frame(identities, operation, step);
            }
            CheckedUnitEffectOperationPlan::EstablishLocal(binding) => match binding {
                checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. } => {
                    let local = self
                        .frame(step.source_value_count)
                        .establish_primitive_local(identities, operation)?;
                    self.next_call_obligation = self.scalar_calls.next_obligation_identity;
                    self.primitive_local_places.push(local);
                    return Ok(());
                }
                checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. } => {
                    self.establish_trivial_affine_local(operation)?
                }
                _ => unreachable!("not an ordinary-machine local binding"),
            },
            CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => {
                return unsupported("Unit return is not the final checked operation");
            }
        };
        let Some(kind) = kind else {
            return Ok(());
        };
        let id = self.operations.allocate();
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
    /// Lower one store, scalar local, borrowed window, continuation cleanup or
    /// structural-value construction through the operation frame composed
    /// states share.
    fn emit_through_frame(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        source_value_count: usize,
    ) -> Result<(), LoweringError> {
        self.frame(source_value_count).emit(identities, operation)?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        Ok(())
    }

    /// Lower one call through the frame with its scheduled operands. A staged
    /// call's scalar result is a private temporary of its argument group,
    /// retained until the group ends; every other result the frame publishes.
    fn emit_call_through_frame(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
        step: StepInputs,
    ) -> Result<(), LoweringError> {
        // The literal roster preallocates the byte-sequence literals of every
        // call's structural arguments in call order.
        let byte_places = byte_subslices::argument_places(
            operation.call_structural_arguments(),
            &self.private_places,
            &mut self.next_literal_argument,
            &self.staged_subslices[step.operation_index],
        )?;
        // Only the final consumer retires the group's scratch. A nested
        // producer must preserve earlier operands staged for its enclosing
        // call, including their rebound identities after selective evaluation.
        // The final result then takes its actual source slot, as in a state.
        if step.staged && step.finishes_argument_group {
            self.scalar_result_values.truncate(step.source_value_count);
        }
        self.frame(step.source_value_count).emit_call(
            identities,
            operation,
            CallInputs {
                evaluated_scalar_arguments: step.evaluated_scalar_arguments.as_deref(),
                byte_places: &byte_places,
                evaluated_selectors: step.evaluated_selectors.as_deref(),
            },
        )?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        if step.staged && step.finishes_argument_group {
            self.retained_scalar_prefix = Some(self.scalar_result_values.len());
        }
        Ok(())
    }

    /// The frame over this machine's environment: its dense result roster,
    /// split literal and construction rosters, growable claim table, and the
    /// closure's call targets.
    fn frame(&mut self, source_value_count: usize) -> OperationFrame<'_, 'a> {
        let plan = self.plan;
        OperationFrame {
            checked: self.checked,
            machine: plan.machine,
            state: plan.state,
            scalar_result: plan.scalar_result.as_ref(),
            scalar_parameter_count: self.scalar_parameter_count,
            source_value_count,
            parameters: self.evaluation.current_parameters(self.parameters),
            structural_types: StructuralTypeRoster::Owned(self.structural_types),
            type_ids: self.type_ids,
            service_ids: self.service_ids,
            primitive_locals: &self.primitive_local_places,
            results: StructuralResults::Dense(&mut self.structural_result_places),
            private_places: &mut self.private_places,
            windows: &mut self.borrowed_windows,
            evaluation: &mut self.evaluation,
            values: &mut self.scalar_result_values,
            next_place: &mut self.next_place,
            next_value: &mut self.next_value_identity,
            next_block: &mut self.next_block,
            next_edge: &mut self.next_edge,
            calls: &mut self.scalar_calls,
            operations: &mut self.operations,
            callees: Callees {
                plans: self.plans,
                signatures: self.machine_signatures,
                boundaries: self.lowered_boundary_parameters,
                domain_ids: self.domain_ids,
                closure: self.closure,
            },
            caller: CallerCustody {
                erased_scalar_parameters: &self.signature.erased_scalar_parameters,
                erased_proof_parameters: &self.signature.erased_proof_parameters,
                local_places: &self.local_places,
            },
        }
    }
}
