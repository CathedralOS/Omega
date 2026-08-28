use super::*;
use crate::callback_private_relocations::{
    plan_callback_private_relocations, tests::fixture_with_destination,
};
use crate::callback_registrar_arguments::{
    plan_callback_registrar_arguments,
    tests::{exact_catalog, exact_surface},
};
use crate::callback_registrar_destinations::{
    plan_callback_registrar_physical_destinations,
    tests::{closed_row, field_destination, layouts},
};
use crate::callback_thunks::plan_callback_thunks;
use omega_abstract_operations::{
    AbstractBoundaryEdge, AbstractFunctionPlan, AbstractHostFormalOperandBinding,
    AbstractHostOperationProvenance, AbstractOperation, AbstractOperationKind,
    AbstractOperationPlan, InstructionOperand, InstructionOperandKind,
};
use omega_backend_plan::replay_callback_registrar_assigned_operand_bindings;
use omega_calling_conventions::{
    HostOperationKey, NativePlace, build_host_abi_plan, callback_native_parameter_id,
};
use omega_platform_interface::LoweredHostOperation;
use omega_target::NativeTarget;
use psi_arena::{Arena, Handle, HandleSpan};
use std::sync::Arc;

pub(crate) struct Fixture {
    pub(crate) placements: Vec<BoundNominalCallbackPlacement>,
    pub(crate) thunks: Arc<[CallbackThunkPlan]>,
    pub(crate) demands: Arc<[CallbackPrivateRelocationDemand]>,
    pub(crate) host_calls: omega_platform_interface::HostCallPlan,
    pub(crate) argument_bindings: Arc<[CallbackRegistrarArgumentBinding]>,
    pub(crate) layouts: omega_layout::LayoutPlan,
    pub(crate) destinations: Arc<[CallbackRegistrarPhysicalDestination]>,
    pub(crate) abstract_operations: AbstractOperationPlan,
    pub(crate) target_operations: omega_target_operations::TargetOperationPlan,
    pub(crate) assigned_operations: omega_assigned_target_operations::AssignedTargetOperationPlan,
}

pub(crate) fn fixture(formal_ordinal: u32) -> Fixture {
    let (placements, thunks, demands, host_calls, boundaries, argument_bindings) =
        exact_catalog(field_destination(formal_ordinal, &[43]));
    build_fixture(
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        argument_bindings,
        layouts(vec![closed_row(43, 8)]),
    )
}

fn build_fixture(
    placements: Vec<BoundNominalCallbackPlacement>,
    thunks: Arc<[CallbackThunkPlan]>,
    demands: Arc<[CallbackPrivateRelocationDemand]>,
    mut host_calls: omega_platform_interface::HostCallPlan,
    mut boundaries: omega_abstract_operations::AbstractBoundarySummary,
    argument_bindings: Arc<[CallbackRegistrarArgumentBinding]>,
    layouts: omega_layout::LayoutPlan,
) -> Fixture {
    let target = NativeTarget::windows_x64();
    let (source_call, call_snapshot) = host_calls
        .calls
        .iter()
        .next()
        .map(|(handle, call)| (handle, call.clone()))
        .unwrap();
    let operation_key = HostOperationKey::default();
    let operation = host_calls.operations.insert(LoweredHostOperation {
        operation_key,
        fixed_leading_immediate: None,
    });
    host_calls.calls.get_mut(source_call).operations = HandleSpan::from_parts(operation, 1);

    let (occurrence, occurrence_snapshot) = boundaries
        .host_calls
        .iter()
        .next()
        .map(|(handle, occurrence)| (handle, occurrence.clone()))
        .unwrap();
    boundaries.edges.insert(AbstractBoundaryEdge {
        host_call: occurrence,
        source_key: call_snapshot.source_key,
        statement_index: call_snapshot.statement_index,
        call_ordinal: call_snapshot.call_ordinal,
        operation_ordinal: 0,
        operation_key,
    });

    let formal_count = usize::try_from(occurrence_snapshot.arguments.count()).unwrap();
    let mut abstract_operations = AbstractOperationPlan::with_capacity(1, 1, formal_count + 1, 0);
    abstract_operations.semantics.boundaries = boundaries;
    let mut operands = HandleSpan::empty();
    for value in 0..=formal_count {
        abstract_operations.code.operands.append_to_span(
            &mut operands,
            InstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(i64::try_from(value).unwrap()),
            },
        );
    }
    let native_arguments = abstract_operations
        .semantics
        .boundaries
        .host_call_arguments
        .span(occurrence_snapshot.arguments)
        .unwrap();
    let formal_operands = (0..formal_count)
        .map(|ordinal| AbstractHostFormalOperandBinding {
            formal_ordinal: u32::try_from(ordinal).unwrap(),
            native_parameter: native_arguments[ordinal].native_parameter.unwrap(),
            operand: Handle::from_parts(
                operands.start().arena_index() + u32::try_from(ordinal + 1).unwrap(),
                operands.start().generation(),
            ),
        })
        .collect::<Vec<_>>();
    let instruction = abstract_operations
        .code
        .instructions
        .insert(AbstractOperation {
            kind: AbstractOperationKind::HostOperation {
                operation_ordinal: 0,
                operands,
                provenance: Some(AbstractHostOperationProvenance {
                    source_call_index: source_call.arena_index(),
                    source_call_generation: source_call.generation(),
                    call_ordinal: call_snapshot.call_ordinal,
                    operation_ordinal: 0,
                    formal_operands: formal_operands.into(),
                }),
            },
            source_key: call_snapshot.source_key,
            source_statement: call_snapshot.statement_index,
        });
    abstract_operations
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: Arc::from("registrar"),
            identity: omega_control_flow::MachineFunctionIdentity::source(call_snapshot.source_key),
            instructions: HandleSpan::from_parts(instruction, 1),
        });

    let destinations = plan_callback_registrar_physical_destinations(
        target,
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &abstract_operations.semantics.boundaries,
        &argument_bindings,
        &layouts,
    )
    .unwrap();
    let target_operations =
        omega_abstract_operations_to_target_operations::build_target_operation_plan(
            target,
            &build_host_abi_plan(target),
            &host_calls,
            &abstract_operations,
        )
        .unwrap();
    let assigned_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            &target_operations,
        );
    Fixture {
        placements,
        thunks,
        demands,
        host_calls,
        argument_bindings,
        layouts,
        destinations,
        abstract_operations,
        target_operations,
        assigned_operations,
    }
}

pub(crate) fn shared_root_fixture() -> Fixture {
    let (control_flow, first) = fixture_with_destination(field_destination(1, &[43]));
    let (_, mut second) = fixture_with_destination(field_destination(1, &[47]));
    second.static_machine_ordinal = 1;
    let placements = vec![first, second];
    let thunks = plan_callback_thunks(&control_flow, &placements).unwrap();
    let demands = plan_callback_private_relocations(&placements, &thunks).unwrap();
    let (host_calls, boundaries) = exact_surface(&placements[0]);
    let argument_bindings =
        plan_callback_registrar_arguments(&placements, &thunks, &demands, &host_calls, &boundaries)
            .unwrap();
    build_fixture(
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        argument_bindings,
        layouts(vec![closed_row(43, 8), closed_row(47, 16)]),
    )
}

pub(crate) fn parameter_fixture() -> Fixture {
    let (placements, thunks, demands, host_calls, boundaries, argument_bindings) =
        exact_catalog(NativePlace::Parameter(callback_native_parameter_id(
            "package::Registrar::register#exact",
            1,
        )));
    build_fixture(
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        argument_bindings,
        layouts(Vec::new()),
    )
}

pub(crate) fn plan(fixture: &Fixture) -> Arc<[CallbackRegistrarAssignedOperandBinding]> {
    plan_callback_registrar_assigned_operand_bindings(
        NativeTarget::windows_x64(),
        &fixture.placements,
        &fixture.thunks,
        &fixture.demands,
        &fixture.host_calls,
        &fixture.abstract_operations.semantics.boundaries,
        &fixture.argument_bindings,
        &fixture.layouts,
        &fixture.destinations,
        &fixture.abstract_operations,
        &fixture.target_operations,
        &fixture.assigned_operations,
    )
    .unwrap()
}

pub(crate) fn with_formal_operand_kind(
    mut fixture: Fixture,
    kind: InstructionOperandKind,
) -> (Fixture, Arc<[CallbackRegistrarAssignedOperandBinding]>) {
    let binding = plan(&fixture)[0].clone();
    let abstract_operand = Handle::from_parts(
        binding.formal_operand.operand.arena_index(),
        binding.formal_operand.operand.generation(),
    );
    fixture
        .abstract_operations
        .code
        .operands
        .get_mut(abstract_operand)
        .kind = kind;
    fixture.target_operations = build_target(
        &fixture,
        NativeTarget::windows_x64(),
        &fixture.abstract_operations,
    )
    .unwrap();
    fixture.assigned_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            &fixture.target_operations,
        );
    let bindings = plan(&fixture);
    (fixture, bindings)
}

fn replay(
    fixture: &Fixture,
    bindings: &[CallbackRegistrarAssignedOperandBinding],
) -> Result<(), omega_calling_conventions::PlanDiagnostic> {
    replay_callback_registrar_assigned_operand_bindings(
        NativeTarget::windows_x64(),
        &fixture.placements,
        &fixture.thunks,
        &fixture.demands,
        &fixture.host_calls,
        &fixture.abstract_operations.semantics.boundaries,
        &fixture.argument_bindings,
        &fixture.layouts,
        &fixture.destinations,
        &fixture.abstract_operations,
        &fixture.target_operations,
        &fixture.assigned_operations,
        bindings,
    )
}

#[test]
fn retains_exact_register_and_stack_formal_operand_identity() {
    for formal_ordinal in [1, 4] {
        let fixture = fixture(formal_ordinal);
        let bindings = plan(&fixture);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].formal_operand.formal_ordinal, formal_ordinal);
        assert_eq!(
            bindings[0].abstract_instruction.arena_index(),
            bindings[0].target_instruction.arena_index()
        );
        assert_eq!(
            bindings[0].target_instruction,
            bindings[0].assigned_instruction
        );
        assert_eq!(
            bindings[0].formal_operand.operand,
            bindings[0].assigned_operand
        );
        assert_eq!(
            bindings[0]
                .destination
                .parameter_placement
                .locations
                .iter()
                .any(|location| matches!(
                    location,
                    omega_calling_conventions::ValueLocation::Stack { .. }
                )),
            formal_ordinal == 4
        );
    }
}

#[test]
fn distinct_private_slots_preserve_one_shared_assigned_argument_root() {
    let fixture = shared_root_fixture();
    let bindings = plan(&fixture);

    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].formal_operand, bindings[1].formal_operand);
    assert_eq!(bindings[0].target_operand, bindings[1].target_operand);
    assert_eq!(bindings[0].assigned_operand, bindings[1].assigned_operand);
    assert_ne!(bindings[0].destination.kind, bindings[1].destination.kind);
    replay(&fixture, &bindings).unwrap();
}

#[test]
fn excludes_result_pseudo_argument_and_rejects_source_identity_drift() {
    let fixture = fixture(1);
    let bindings = plan(&fixture);
    assert_eq!(bindings[0].provenance.formal_operands.len(), 5);
    assert_eq!(bindings[0].formal_operand.formal_ordinal, 1);

    let mut wrong_occurrence = fixture.abstract_operations.clone();
    let instruction_handle = wrong_occurrence.code.instructions.iter().next().unwrap().0;
    let instruction = wrong_occurrence
        .code
        .instructions
        .get_mut(instruction_handle);
    let AbstractOperationKind::HostOperation {
        provenance: Some(provenance),
        ..
    } = &mut instruction.kind
    else {
        unreachable!()
    };
    provenance.source_call_index += 1;
    assert!(
        omega_abstract_operations_to_target_operations::build_target_operation_plan(
            NativeTarget::windows_x64(),
            &build_host_abi_plan(NativeTarget::windows_x64()),
            &fixture.host_calls,
            &wrong_occurrence,
        )
        .is_err()
    );
}

#[test]
fn exact_source_handle_disambiguates_same_coordinate_host_calls() {
    let mut fixture = fixture(1);
    let (_, source_call) = fixture.host_calls.calls.iter().next().unwrap();
    let source_call = source_call.clone();
    let mut decoy = source_call.clone();
    decoy.call_ordinal += 1;
    fixture.host_calls.calls = Arena::new();
    let decoy_handle = fixture.host_calls.calls.insert(decoy.clone());
    let source_handle = fixture.host_calls.calls.insert(source_call.clone());

    let occurrence_handle = fixture
        .abstract_operations
        .semantics
        .boundaries
        .host_calls
        .iter()
        .next()
        .unwrap()
        .0;
    let occurrence = fixture
        .abstract_operations
        .semantics
        .boundaries
        .host_calls
        .get_mut(occurrence_handle);
    occurrence.source_call_index = source_handle.arena_index();
    occurrence.source_call_generation = source_handle.generation();
    let instruction_handle = fixture
        .abstract_operations
        .code
        .instructions
        .iter()
        .next()
        .unwrap()
        .0;
    let instruction = fixture
        .abstract_operations
        .code
        .instructions
        .get_mut(instruction_handle);
    let AbstractOperationKind::HostOperation {
        provenance: Some(provenance),
        ..
    } = &mut instruction.kind
    else {
        unreachable!()
    };
    provenance.source_call_index = source_handle.arena_index();
    provenance.source_call_generation = source_handle.generation();

    fixture.target_operations = build_target(
        &fixture,
        NativeTarget::windows_x64(),
        &fixture.abstract_operations,
    )
    .unwrap();
    fixture.assigned_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            &fixture.target_operations,
        );
    assert_eq!(plan(&fixture).len(), 1);

    let mut wrong = fixture.abstract_operations.clone();
    let instruction = wrong.code.instructions.get_mut(instruction_handle);
    let AbstractOperationKind::HostOperation {
        provenance: Some(provenance),
        ..
    } = &mut instruction.kind
    else {
        unreachable!()
    };
    provenance.source_call_index = decoy_handle.arena_index();
    provenance.source_call_generation = decoy_handle.generation();
    provenance.call_ordinal = decoy.call_ordinal;
    assert!(
        build_target(&fixture, NativeTarget::windows_x64(), &wrong).is_err(),
        "the same-coordinate decoy must not replace the exact source-call handle"
    );
}

#[test]
fn target_lowering_rejects_duplicate_edge_call_ordinal_and_operand_handle_drift() {
    let fixture = fixture(1);
    let target = NativeTarget::windows_x64();

    let mut duplicate_edge = fixture.abstract_operations.clone();
    let edge = duplicate_edge
        .semantics
        .boundaries
        .edges
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    duplicate_edge.semantics.boundaries.edges.insert(edge);
    assert!(build_target(&fixture, target, &duplicate_edge).is_err());

    let mut ordinal = fixture.abstract_operations.clone();
    let instruction_handle = ordinal.code.instructions.iter().next().unwrap().0;
    let instruction = ordinal.code.instructions.get_mut(instruction_handle);
    let AbstractOperationKind::HostOperation {
        provenance: Some(provenance),
        ..
    } = &mut instruction.kind
    else {
        unreachable!()
    };
    provenance.call_ordinal += 1;
    assert!(build_target(&fixture, target, &ordinal).is_err());

    let mut operand = fixture.abstract_operations.clone();
    let instruction_handle = operand.code.instructions.iter().next().unwrap().0;
    let instruction = operand.code.instructions.get_mut(instruction_handle);
    let AbstractOperationKind::HostOperation {
        provenance: Some(provenance),
        ..
    } = &mut instruction.kind
    else {
        unreachable!()
    };
    Arc::make_mut(&mut provenance.formal_operands)[0].operand = Handle::invalid();
    assert!(build_target(&fixture, target, &operand).is_err());
}

pub(crate) fn build_target(
    fixture: &Fixture,
    target: NativeTarget,
    abstract_operations: &AbstractOperationPlan,
) -> Result<omega_target_operations::TargetOperationPlan, psi_diagnostics::Diagnostic> {
    omega_abstract_operations_to_target_operations::build_target_operation_plan(
        target,
        &build_host_abi_plan(target),
        &fixture.host_calls,
        abstract_operations,
    )
}

#[test]
fn replay_rejects_cardinality_instruction_formal_and_shape_drift() {
    let fixture = fixture(1);
    let bindings = plan(&fixture);
    assert!(replay(&fixture, &[]).is_err());
    assert!(replay(&fixture, &[bindings[0].clone(), bindings[0].clone()]).is_err());

    let mut instruction = bindings[0].clone();
    instruction.target_instruction = Handle::invalid();
    assert!(replay(&fixture, &[instruction]).is_err());
    let mut formal = bindings[0].clone();
    formal.formal_operand.formal_ordinal = 4;
    assert!(replay(&fixture, &[formal]).is_err());
    let mut operand = bindings[0].clone();
    operand.assigned_operand = Handle::invalid();
    assert!(replay(&fixture, &[operand]).is_err());
    let mut shape = bindings[0].clone();
    shape.target_operand.kind =
        omega_target_operations::TargetInstructionOperandKind::ByteLength(7);
    assert!(replay(&fixture, &[shape]).is_err());
    let mut abstract_source = bindings[0].clone();
    abstract_source.abstract_provenance.source_call_index += 1;
    assert!(replay(&fixture, &[abstract_source]).is_err());
}

#[test]
fn planner_rejects_duplicate_selected_operation_for_one_formal() {
    let mut fixture = fixture(1);
    let duplicate = fixture
        .target_operations
        .code
        .instructions
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    fixture
        .target_operations
        .code
        .instructions
        .insert(duplicate);
    fixture.assigned_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            &fixture.target_operations,
        );
    assert!(
        plan_callback_registrar_assigned_operand_bindings(
            NativeTarget::windows_x64(),
            &fixture.placements,
            &fixture.thunks,
            &fixture.demands,
            &fixture.host_calls,
            &fixture.abstract_operations.semantics.boundaries,
            &fixture.argument_bindings,
            &fixture.layouts,
            &fixture.destinations,
            &fixture.abstract_operations,
            &fixture.target_operations,
            &fixture.assigned_operations,
        )
        .is_err()
    );
}
