use super::*;
use crate::callback_registrar_assigned_operands::tests::{
    Fixture, build_target, fixture, parameter_fixture, plan as plan_assigned, shared_root_fixture,
    with_formal_operand_kind,
};
use omega_abstract_operations::{InstructionOperandKind, RuntimeStorageRegion};
use omega_backend_plan::replay_callback_private_object_store_requests;
use omega_object_file::{
    FunctionSymbolPlan, ObjectPlan, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    storage_region_symbol_name,
};
use omega_target::NativeTarget;

const ENTRY_MACHINE_NAME: &str = "Main";

fn runtime_fixture(
    fixture: Fixture,
    region: RuntimeStorageRegion,
    byte_offset: usize,
) -> (
    Fixture,
    Arc<[CallbackRegistrarAssignedOperandBinding]>,
    ObjectPlan,
) {
    let (mut fixture, bindings) = with_formal_operand_kind(
        fixture,
        InstructionOperandKind::RuntimeStorageAddress {
            region,
            byte_offset,
        },
    );
    let mut boundary = fixture.placements[0]
        .private_materialization
        .as_ref()
        .unwrap()
        .registrar_boundary_entry_plan
        .clone();
    boundary.call.callback_materializations.clear();
    crate::callback_private_address_stores::insert_callback_private_address_store_operations(
        &mut fixture.abstract_operations,
        &bindings,
        Some(&boundary),
    )
    .unwrap();
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
    let bindings = plan_assigned(&fixture);
    let mut object = ObjectPlan::with_capacities(
        NativeTarget::windows_x64(),
        0,
        bindings.len() + 1,
        bindings.len(),
    );
    object.layout.symbols.insert(SymbolPlan {
        name: storage_region_symbol_name(region, ENTRY_MACHINE_NAME),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 0,
        size: 256,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    for (index, identity) in bindings
        .iter()
        .map(|binding| binding.destination.binding.demand.function_identity)
        .enumerate()
    {
        if object
            .layout
            .function_symbols
            .iter()
            .any(|(_, function)| function.identity == identity)
        {
            continue;
        }
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!("callback_private_{index}"),
            section: SymbolSection::Section(SectionKind::Text),
            offset: index * 8,
            size: 8,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object
            .layout
            .function_symbols
            .insert(FunctionSymbolPlan { identity, symbol });
    }
    (fixture, bindings, object)
}

fn plan(
    fixture: &Fixture,
    bindings: &[CallbackRegistrarAssignedOperandBinding],
    object: &ObjectPlan,
) -> Result<Arc<[CallbackPrivateObjectStoreRequest]>, psi_diagnostics::Diagnostic> {
    plan_callback_private_object_store_requests(
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
        object,
        ENTRY_MACHINE_NAME,
    )
}

fn replay(
    fixture: &Fixture,
    bindings: &[CallbackRegistrarAssignedOperandBinding],
    object: &ObjectPlan,
    requests: &[CallbackPrivateObjectStoreRequest],
) -> Result<(), omega_calling_conventions::PlanDiagnostic> {
    replay_callback_private_object_store_requests(
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
        object,
        ENTRY_MACHINE_NAME,
        requests,
    )
}

#[test]
fn retains_exact_machine_and_runtime_frame_object_relative_requests() {
    for (region, formal_ordinal) in [
        (RuntimeStorageRegion::Machine, 1),
        (RuntimeStorageRegion::RuntimeFrame, 4),
    ] {
        let (fixture, bindings, object) = runtime_fixture(fixture(formal_ordinal), region, 32);
        let assigned_snapshot = bindings[0].clone();
        let requests = plan(&fixture, &bindings, &object).unwrap();

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].storage_region, region);
        assert_eq!(requests[0].storage_base_offset, 32);
        assert_eq!(requests[0].slot_offset, 8);
        assert_eq!(requests[0].destination_offset, 40);
        assert_eq!(requests[0].byte_size, 8);
        assert_eq!(requests[0].alignment, 8);
        assert_eq!(requests[0].storage_symbol_plan.kind, SymbolKind::Object);
        assert_eq!(
            requests[0].storage_symbol_plan.section,
            SymbolSection::Section(SectionKind::Bss)
        );
        assert_eq!(requests[0].function_symbol_plan.kind, SymbolKind::Function);
        assert_eq!(requests[0].assigned_binding, assigned_snapshot);
        assert_eq!(
            requests[0]
                .assigned_binding
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
        replay(&fixture, &bindings, &object, &requests).unwrap();
    }
}

#[test]
fn shared_argument_root_retains_distinct_ordered_slot_requests() {
    let (fixture, bindings, object) =
        runtime_fixture(shared_root_fixture(), RuntimeStorageRegion::Machine, 32);
    let requests = plan(&fixture, &bindings, &object).unwrap();

    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].storage_symbol, requests[1].storage_symbol);
    assert_eq!(requests[0].destination_offset, 40);
    assert_eq!(requests[1].destination_offset, 48);
    assert_ne!(requests[0].function_identity, requests[1].function_identity);
    replay(&fixture, &bindings, &object, &requests).unwrap();
    assert!(
        replay(
            &fixture,
            &bindings,
            &object,
            &[requests[1].clone(), requests[0].clone()]
        )
        .is_err()
    );
}

#[test]
fn rejects_data_address_and_direct_parameter_shapes() {
    let (data_fixture, data_bindings) = with_formal_operand_kind(
        fixture(1),
        InstructionOperandKind::DataAddress {
            data: psi_arena::Handle::invalid(),
        },
    );
    let (_, _, object) = runtime_fixture(fixture(1), RuntimeStorageRegion::Machine, 32);
    assert!(plan(&data_fixture, &data_bindings, &object).is_err());

    let (mut parameter_fixture, parameter_bindings) = with_formal_operand_kind(
        parameter_fixture(),
        InstructionOperandKind::RuntimeStorageAddress {
            region: RuntimeStorageRegion::Machine,
            byte_offset: 32,
        },
    );
    let mut boundary = parameter_fixture.placements[0]
        .private_materialization
        .as_ref()
        .unwrap()
        .registrar_boundary_entry_plan
        .clone();
    boundary.call.callback_materializations.clear();
    assert!(
        crate::callback_private_address_stores::insert_callback_private_address_store_operations(
            &mut parameter_fixture.abstract_operations,
            &parameter_bindings,
            Some(&boundary),
        )
        .is_err()
    );
}

#[test]
fn replay_rejects_cardinality_identity_and_geometry_drift() {
    let (fixture, bindings, object) =
        runtime_fixture(fixture(1), RuntimeStorageRegion::Machine, 32);
    let requests = plan(&fixture, &bindings, &object).unwrap();
    assert!(replay(&fixture, &bindings, &object, &[]).is_err());
    assert!(
        replay(
            &fixture,
            &bindings,
            &object,
            &[requests[0].clone(), requests[0].clone()]
        )
        .is_err()
    );

    let mut assigned = requests[0].clone();
    assigned.assigned_binding_index = 1;
    assert!(replay(&fixture, &bindings, &object, &[assigned]).is_err());
    let mut assigned_snapshot = requests[0].clone();
    assigned_snapshot.assigned_binding.assigned_operand = psi_arena::Handle::invalid();
    assert!(replay(&fixture, &bindings, &object, &[assigned_snapshot]).is_err());
    let mut region = requests[0].clone();
    region.storage_region = RuntimeStorageRegion::RuntimeFrame;
    assert!(replay(&fixture, &bindings, &object, &[region]).is_err());
    let mut base = requests[0].clone();
    base.storage_base_offset += 8;
    assert!(replay(&fixture, &bindings, &object, &[base]).is_err());
    let mut slot = requests[0].clone();
    slot.slot_offset += 8;
    assert!(replay(&fixture, &bindings, &object, &[slot]).is_err());
    let mut geometry = requests[0].clone();
    geometry.destination_offset += 8;
    assert!(replay(&fixture, &bindings, &object, &[geometry]).is_err());
    let mut byte_size = requests[0].clone();
    byte_size.byte_size += 8;
    assert!(replay(&fixture, &bindings, &object, &[byte_size]).is_err());
    let mut alignment = requests[0].clone();
    alignment.alignment = 4;
    assert!(replay(&fixture, &bindings, &object, &[alignment]).is_err());
    let mut storage = requests[0].clone();
    storage.storage_symbol = psi_arena::Handle::invalid();
    assert!(replay(&fixture, &bindings, &object, &[storage]).is_err());
    let mut storage_snapshot = requests[0].clone();
    storage_snapshot.storage_symbol_plan.size += 8;
    assert!(replay(&fixture, &bindings, &object, &[storage_snapshot]).is_err());
    let mut function = requests[0].clone();
    function.function_identity = omega_control_flow::MachineFunctionIdentity::default();
    assert!(replay(&fixture, &bindings, &object, &[function]).is_err());
    let mut function_symbol = requests[0].clone();
    function_symbol.function_symbol = function_symbol.storage_symbol;
    assert!(replay(&fixture, &bindings, &object, &[function_symbol]).is_err());
    let mut function_snapshot = requests[0].clone();
    function_snapshot.function_symbol_plan.size += 8;
    assert!(replay(&fixture, &bindings, &object, &[function_snapshot]).is_err());
}

#[test]
fn replay_rejects_store_source_and_function_boundary_drift() {
    let (mut source_drift, bindings, object) =
        runtime_fixture(fixture(1), RuntimeStorageRegion::Machine, 32);
    let requests = plan(&source_drift, &bindings, &object).unwrap();
    source_drift
        .abstract_operations
        .code
        .instructions
        .get_mut(requests[0].abstract_store_instruction)
        .source_statement += 1;
    assert!(replay(&source_drift, &bindings, &object, &requests).is_err());

    let (mut boundary_drift, bindings, object) =
        runtime_fixture(fixture(1), RuntimeStorageRegion::Machine, 32);
    let requests = plan(&boundary_drift, &bindings, &object).unwrap();
    let function = boundary_drift
        .abstract_operations
        .code
        .functions
        .iter()
        .next()
        .unwrap()
        .0;
    boundary_drift
        .abstract_operations
        .code
        .functions
        .get_mut(function)
        .instructions = psi_arena::HandleSpan::from_parts(bindings[0].abstract_instruction, 1);
    assert!(replay(&boundary_drift, &bindings, &object, &requests).is_err());
}

#[test]
fn planner_rejects_missing_duplicate_or_drifted_object_symbols() {
    let (fixture, bindings, object) =
        runtime_fixture(fixture(1), RuntimeStorageRegion::Machine, 32);

    let mut missing_storage = object.clone();
    missing_storage.layout.symbols = psi_arena::Arena::new();
    assert!(plan(&fixture, &bindings, &missing_storage).is_err());

    let mut duplicate_storage = object.clone();
    duplicate_storage
        .layout
        .symbols
        .insert(object.layout.symbols.iter().next().unwrap().1.clone());
    assert!(plan(&fixture, &bindings, &duplicate_storage).is_err());

    let mut short_storage = object.clone();
    let storage = short_storage.layout.symbols.iter().next().unwrap().0;
    short_storage.layout.symbols.get_mut(storage).size = 39;
    assert!(plan(&fixture, &bindings, &short_storage).is_err());

    let mut wrong_storage_section = object.clone();
    wrong_storage_section
        .layout
        .symbols
        .get_mut(storage)
        .section = SymbolSection::Section(SectionKind::Data);
    assert!(plan(&fixture, &bindings, &wrong_storage_section).is_err());

    let mut wrong_storage_kind = object.clone();
    wrong_storage_kind.layout.symbols.get_mut(storage).kind = SymbolKind::Function;
    assert!(plan(&fixture, &bindings, &wrong_storage_kind).is_err());

    let mut missing_function = object.clone();
    missing_function.layout.function_symbols = psi_arena::Arena::new();
    assert!(plan(&fixture, &bindings, &missing_function).is_err());

    let mut duplicate_function = object.clone();
    let function = duplicate_function
        .layout
        .function_symbols
        .iter()
        .next()
        .unwrap()
        .1
        .clone();
    duplicate_function.layout.function_symbols.insert(function);
    assert!(plan(&fixture, &bindings, &duplicate_function).is_err());
}

#[test]
fn planner_rejects_overflow_and_misaligned_destinations() {
    let (mut overflow_fixture, overflow_bindings) = with_formal_operand_kind(
        fixture(1),
        InstructionOperandKind::RuntimeStorageAddress {
            region: RuntimeStorageRegion::Machine,
            byte_offset: usize::MAX,
        },
    );
    let mut boundary = overflow_fixture.placements[0]
        .private_materialization
        .as_ref()
        .unwrap()
        .registrar_boundary_entry_plan
        .clone();
    boundary.call.callback_materializations.clear();
    assert!(
        crate::callback_private_address_stores::insert_callback_private_address_store_operations(
            &mut overflow_fixture.abstract_operations,
            &overflow_bindings,
            Some(&boundary),
        )
        .is_err()
    );

    let (misaligned_fixture, misaligned_bindings, misaligned_object) =
        runtime_fixture(fixture(1), RuntimeStorageRegion::Machine, 33);
    assert!(
        plan(
            &misaligned_fixture,
            &misaligned_bindings,
            &misaligned_object
        )
        .is_err()
    );

    let ordinary = fixture(1);
    assert_eq!(plan_assigned(&ordinary).len(), 1);
}
