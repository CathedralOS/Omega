//! One checked dynamic call, for either result.
//!
//! The caller borrows its attachment, selects a descriptor from one field
//! (directly, rebound once, or stored through one aggregate field) and calls
//! the requirement once: straight into the selected realization, through the
//! descriptor, or into the first forwarded helper. A scalar call may first
//! store one caller field and binds the call's result. The realizations, the
//! exact conformance applications and the helper chain are emitted beside the
//! caller. A [`DynamicCall`] lane supplies only its result, caller store,
//! helper bodies and source publication.

use super::{
    Block, CheckedStructuralAccess, CheckedTrees, LoweredPsi, LoweringError, Operation,
    OperationKind, OperationResult, ProofBundle, StructuralAccess, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, allocate_dense, block_id, edge_id,
    lookup_type_id, lower_installation_machine_service_ceiling, lower_root_service_reach,
    machine_id, operation_id, place_id, unsupported, value_id,
};
use crate::unit::dynamic_composed_unit::LoweredDynamicDispatch;
use crate::unit::dynamic_composed_unit::applications::{
    exact_machine_service_summary, lower_changed_initial_application, lower_exact_application,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{DynamicCall, DynamicLoweringLane};
use crate::unit::dynamic_composed_unit::forwarded_helpers::{
    dynamic_source_call_occurrences, extend_parameter_forwarding_catalog,
    forwarded_helper_chain_ids, materialize_forwarded_helper_chain,
};
use crate::unit::dynamic_composed_unit::plan_validation::validate_exact_plan;
use crate::unit::dynamic_composed_unit::realizations::{
    collect_dynamic_realizations, materialize_dynamic_realizations, retain_realizations_for_lane,
    selected_realization,
};
use crate::unit::dynamic_composed_unit::source_lowering::{
    lower_dynamic_call_custody, validate_and_lower_dynamic_source,
};
use crate::unit::dynamic_composed_unit::store_operations::{
    empty_terminal_contract, lower_caller_store_operations,
};
use crate::unit::dynamic_composed_unit::structural_types::{
    lower_dynamic_structural_types_for_source, terminal_structural_multiplicity,
};

/// Lower one checked call: validate it under its lane, lower the caller's
/// borrowed `self`, source and any retained store, retain the selected
/// application and the lane's descriptor custody, then emit the caller, the
/// realizations and the forwarded helper chain.
pub(super) fn lower<Call: DynamicCall>(
    checked: &CheckedTrees,
    call: &Call,
    lane: DynamicLoweringLane<'_>,
) -> Result<LoweredDynamicDispatch, LoweringError> {
    validate_exact_plan(checked, call, lane)?;
    let plan = call.view();
    let result_type = call.result_type()?;
    let (structural_types, type_ids) = lower_dynamic_structural_types_for_source(
        checked,
        plan.caller_attachment_type_identity,
        plan.caller_attachment_type_identity,
        plan.source_path,
        plan.source_type_identity,
    )?;
    let caller_attachment = lookup_type_id(&type_ids, plan.caller_attachment_type_identity)?;
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(plan.caller_multiplicity),
        access: match plan.caller_parameter_access {
            CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
            _ => return unsupported("direct dynamic caller requires a borrowed self parameter"),
        },
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let source = validate_and_lower_dynamic_source(
        &caller_self,
        &plan,
        plan.source_path,
        plan.source_type_identity,
        &structural_types,
        &type_ids,
    )?;

    // The caller's operations run in authored order: the retained field
    // store, the stored descriptor's establishment, then the call.
    let caller_machine = machine_id(1);
    let mut next_operation = 1_u64;
    let mut next_value = 1_u64;
    let mut caller_operations = lower_caller_store_operations(
        call.caller_store(),
        &plan,
        &caller_self,
        &structural_types,
        &type_ids,
        &mut next_operation,
        &mut next_value,
    )?;
    let descriptor_store_operation = match lane {
        DynamicLoweringLane::Stored(_) => Some(operation_id(allocate_dense(&mut next_operation)?)),
        _ => None,
    };
    let call_operation = operation_id(allocate_dense(&mut next_operation)?);
    let call_result = result_type
        .map(|scalar_type| {
            Ok::<_, LoweringError>(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type,
            })
        })
        .transpose()?;

    let source_type = lookup_type_id(&type_ids, plan.source_type_identity)?;
    let all_realizations = collect_dynamic_realizations(checked, &plan, 2)?;
    let lowered_realizations = retain_realizations_for_lane(&all_realizations, &plan, lane)?;
    let selected = selected_realization(call, &lowered_realizations)?;
    let (realization_machine, callable_identity) =
        (selected.machine, selected.callable_identity.clone());
    let (application, selected_row) =
        lower_exact_application(checked, &plan, caller_machine, &lowered_realizations)?;
    let initial_application =
        lower_changed_initial_application(checked, &plan, lane, caller_machine)?;

    let mut next_block = 2_u64;
    let mut next_place = 2_u64;
    let mut next_edge = 2_u64;
    let forwarded_helpers = forwarded_helper_chain_ids(
        call,
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let (mut dynamic_dispatch, call_kind) = lower_dynamic_call_custody(
        lane,
        &caller_self,
        &plan,
        result_type.is_some(),
        &structural_types,
        &type_ids,
        caller_machine,
        call_operation,
        descriptor_store_operation,
        source,
        initial_application.as_ref(),
        &application,
        &selected_row,
        callable_identity,
        realization_machine,
        forwarded_helpers.first().copied(),
    )?;
    if forwarded_helpers.len() > 1 {
        extend_parameter_forwarding_catalog(&mut dynamic_dispatch, &forwarded_helpers)?;
    }

    let caller_block = block_id(1);
    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        plan.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(plan.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, plan.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, plan.caller_machine, &[])?;
    if let Some(id) = descriptor_store_operation {
        caller_operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: OperationKind::StoreDynamicDescriptor {
                descriptor_ordinal: 0,
            },
        });
    }
    caller_operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: call_operation,
        result: call_result.map_or(OperationResult::Unit, OperationResult::Scalar),
        kind: call_kind,
    });
    let realization_machines = materialize_dynamic_realizations(
        checked,
        &plan,
        &lowered_realizations,
        source_type,
        &structural_types,
        &mut next_block,
        &mut next_place,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let mut source_call_occurrences =
        dynamic_source_call_occurrences(&[(&plan, call_operation)], &forwarded_helpers)?;
    let forwarded_helper_machines = materialize_forwarded_helper_chain(
        checked,
        call,
        &application,
        &selected_row,
        &forwarded_helpers,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
        &mut source_call_occurrences,
    )?;

    let mut applications = vec![application];
    applications.extend(initial_application);
    applications.sort_by(|left, right| {
        (
            left.owner,
            left.declaration_identity.as_str(),
            left.report_fingerprint,
        )
            .cmp(&(
                right.owner,
                right.declaration_identity.as_str(),
                right.report_fingerprint,
            ))
    });
    let mut machines = vec![TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller_machine,
        attachment: Some(caller_attachment),
        parameters: Vec::new(),
        structural_parameters: vec![caller_self.clone()],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: caller_self.place,
            kind: StructuralPlaceKind::Parameter {
                position: caller_self.position,
                is_self: caller_self.is_self,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: caller_reach,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: caller_block,
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: caller_block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: caller_operations,
            terminator: Terminator::ReturnUnit {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_terminal_contract(caller_machine.get()),
    }];
    machines.extend(realization_machines);
    machines.extend(forwarded_helper_machines);

    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            structural_types,
            root_service_reach,
            closed_conformance_applications: applications,
            dynamic_dispatch,
            machines,
            ..TerminalModule::for_entry(caller_machine)
        },
        proof_bundle: ProofBundle {
            crash_obligations: Vec::new(),
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    };
    call.retain_sources(lowered, &lowered_realizations, &forwarded_helpers)
}
