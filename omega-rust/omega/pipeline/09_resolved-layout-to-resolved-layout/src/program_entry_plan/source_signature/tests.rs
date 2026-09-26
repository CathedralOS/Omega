//! Source signature tests.

use super::{
    ProgramEntrySourceExtentValueLayout, ProgramEntrySourceReceiverSignature,
    ProgramEntrySourceResultSignature, ProgramStorageEntryRootRole,
    SelectedProgramEntrySourceSignature,
};
use arena::Arena;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees_to_checked_trees::checked_trees::name::Identifier;

fn key() -> abstract_operations_to_target_operations::function_identity::StateKey {
    abstract_operations_to_target_operations::function_identity::StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    }
}

fn extent_layout() -> ProgramEntrySourceExtentValueLayout {
    ProgramEntrySourceExtentValueLayout::from_checked_record(
        SymbolHandle::from_arena_index(3),
        SymbolHandle::from_arena_index(4),
        0,
        abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(8, 8),
        SymbolHandle::from_arena_index(5),
        8,
        abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(8, 8),
        abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(16, 8),
    )
    .expect("exact Extent record graph")
}

fn backend_extent_layout(
    base_name: &str,
    base_offset: usize,
    length_type: &str,
    aggregate_size: usize,
) -> crate::layout::LayoutPlan {
    let layout = extent_layout();
    let mut fields = Arena::with_capacity(2);
    let field_span = fields.insert_many([
        crate::layout::FieldLayout {
            symbol: layout.fields()[0].symbol(),
            name: Identifier::from(base_name),
            offset: base_offset,
            type_symbol: SymbolHandle::invalid(),
            type_name: Arc::from("addr"),
            type_descriptor: crate::layout::TypeLayoutDescriptor::default(),
            layout: crate::layout::TypeLayout {
                size: 8,
                alignment: 8,
            },
        },
        crate::layout::FieldLayout {
            symbol: layout.fields()[1].symbol(),
            name: Identifier::from("length"),
            offset: 8,
            type_symbol: SymbolHandle::invalid(),
            type_name: Arc::from(length_type),
            type_descriptor: crate::layout::TypeLayoutDescriptor::default(),
            layout: crate::layout::TypeLayout {
                size: 8,
                alignment: 8,
            },
        },
    ]);
    let mut data_layouts = Arena::with_capacity(1);
    data_layouts.insert(crate::layout::DataLayout {
        symbol: layout.data_symbol(),
        name: Identifier::from("Extent"),
        shape: crate::layout::DataShape::Record { fields: field_span },
        layout: crate::layout::TypeLayout {
            size: aggregate_size,
            alignment: 8,
        },
    });
    crate::layout::LayoutPlan {
        data_layouts,
        fields,
        bit_fields: Vec::new(),
        stored_integers: Vec::new(),
        repeated_fields: Vec::new(),
        machine_layouts: Arena::with_capacity(0),
        variants: Arena::with_capacity(0),
        private_callback_demands: Vec::new(),
        plan_laid_layout_identities: Vec::new(),
        two_hop_private_callback_paths: Vec::new(),
    }
}

fn signature(receiver: ProgramEntrySourceReceiverSignature) -> SelectedProgramEntrySourceSignature {
    SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        target::TargetProfile::UefiX64.program_entry_slot(),
        key().machine,
        key().state,
        "Boot::launch".into(),
        "launch".into(),
        "normalized-callable".into(),
        receiver,
        vec![
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::Image,
                0,
                "Extent in Granted".into(),
                abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                    16, 8,
                ),
                extent_layout(),
                false,
                false,
            ),
            SelectedProgramEntrySourceSignature::visible_parameter(
                ProgramStorageEntryRootRole::InitialStorage,
                1,
                "Extent in Granted".into(),
                abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                    16, 8,
                ),
                extent_layout(),
                false,
                false,
            ),
        ],
    )
    .expect("exact typed source signature")
}

#[test]
fn exact_program_storage_signature_binds_declarations_without_values() {
    let signature = signature(ProgramEntrySourceReceiverSignature::ProvisionedMutable {
        normalized_type_identity: "Boot".into(),
    });
    signature
        .validate_program_storage_binding(
            target::TargetProfile::UefiX64.program_entry_slot(),
            key(),
            Some("Boot"),
            "Extent in Granted",
            "Extent in Granted",
        )
        .expect("exact declaration binding");
    assert_eq!(signature.result(), ProgramEntrySourceResultSignature::Unit);
}

#[test]
fn source_signature_identity_binds_normalized_semantics_not_arena_coordinates() {
    let source = signature(ProgramEntrySourceReceiverSignature::Free);
    let identity = source.identity();
    assert_eq!(identity, source.clone().identity());

    let mut renumbered = source.clone();
    renumbered.machine_symbol = SymbolHandle::from_arena_index(20);
    renumbered.state_symbol = SymbolHandle::from_arena_index(21);
    for (index, parameter) in renumbered.visible_parameters.iter_mut().enumerate() {
        let base = u32::try_from(index).expect("fixture index fits u32") * 3;
        parameter.extent_value_layout.data_symbol = SymbolHandle::from_arena_index(30 + base);
        parameter.extent_value_layout.fields[0].symbol = SymbolHandle::from_arena_index(31 + base);
        parameter.extent_value_layout.fields[1].symbol = SymbolHandle::from_arena_index(32 + base);
    }
    assert_eq!(identity, renumbered.identity());

    let mut callable_drift = source.clone();
    callable_drift.normalized_callable_identity = "other-callable".into();
    assert_ne!(identity, callable_drift.identity());

    let receiver_drift = signature(ProgramEntrySourceReceiverSignature::ProvisionedMutable {
        normalized_type_identity: "Boot".into(),
    });
    assert_ne!(identity, receiver_drift.identity());

    let mut parameter_drift = source.clone();
    parameter_drift.visible_parameters.swap(0, 1);
    assert_ne!(identity, parameter_drift.identity());
}

#[test]
fn continuation_receiver_and_parameter_drift_fail_closed() {
    let signature = signature(ProgramEntrySourceReceiverSignature::ProvisionedMutable {
        normalized_type_identity: "Boot".into(),
    });
    let mut wrong_key = key();
    wrong_key.state = SymbolHandle::from_arena_index(3);
    for (key, receiver, image, storage) in [
        (
            wrong_key,
            Some("Boot"),
            "Extent in Granted",
            "Extent in Granted",
        ),
        (
            key(),
            Some("Other"),
            "Extent in Granted",
            "Extent in Granted",
        ),
        (key(), Some("Boot"), "Other", "Extent in Granted"),
        (key(), Some("Boot"), "Extent in Granted", "Other"),
    ] {
        signature
            .validate_program_storage_binding(
                target::TargetProfile::UefiX64.program_entry_slot(),
                key,
                receiver,
                image,
                storage,
            )
            .expect_err("source signature drift must reject");
    }
}

#[test]
fn role_mode_slot_and_free_receiver_drift_fail_closed() {
    let mut role_drift = signature(ProgramEntrySourceReceiverSignature::Free);
    role_drift.visible_parameters.swap(0, 1);
    role_drift
        .validate_program_storage_binding(
            target::TargetProfile::UefiX64.program_entry_slot(),
            key(),
            None,
            "Extent in Granted",
            "Extent in Granted",
        )
        .expect_err("role order drift must reject");

    let mut mode_drift = signature(ProgramEntrySourceReceiverSignature::Free);
    mode_drift.visible_parameters[0].is_mutable = true;
    mode_drift
        .validate_program_storage_binding(
            target::TargetProfile::UefiX64.program_entry_slot(),
            key(),
            None,
            "Extent in Granted",
            "Extent in Granted",
        )
        .expect_err("parameter mode drift must reject");

    let signature = signature(ProgramEntrySourceReceiverSignature::Free);
    signature
        .validate_program_storage_binding(
            target::TargetProfile::WindowsX64.program_entry_slot(),
            key(),
            None,
            "Extent in Granted",
            "Extent in Granted",
        )
        .expect_err("target slot drift must reject");
    signature
        .validate_program_storage_binding(
            target::TargetProfile::UefiX64.program_entry_slot(),
            key(),
            Some("Boot"),
            "Extent in Granted",
            "Extent in Granted",
        )
        .expect_err("free source signature cannot acquire a receiver");
}

#[test]
fn extent_field_graph_drift_fails_closed() {
    for (base_offset, length_offset, aggregate) in [
        (
            1,
            8,
            abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                16, 8,
            ),
        ),
        (
            0,
            0,
            abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                16, 8,
            ),
        ),
        (
            0,
            8,
            abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                8, 8,
            ),
        ),
    ] {
        ProgramEntrySourceExtentValueLayout::from_checked_record(
            SymbolHandle::from_arena_index(3),
            SymbolHandle::from_arena_index(4),
            base_offset,
            abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                8, 8,
            ),
            SymbolHandle::from_arena_index(5),
            length_offset,
            abstract_operations_to_target_operations::calling_conventions::ValueShape::integer(
                8, 8,
            ),
            aggregate,
        )
        .expect_err("Extent structural drift must reject");
    }
}

#[test]
fn extent_backend_field_name_type_offset_and_size_drift_fail_closed() {
    extent_layout()
        .validate_backend_layout(&backend_extent_layout("base", 0, "u64", 16))
        .expect("exact backend Extent layout should replay");
    for backend in [
        backend_extent_layout("address", 0, "u64", 16),
        backend_extent_layout("base", 1, "u64", 16),
        backend_extent_layout("base", 0, "i64", 16),
        backend_extent_layout("base", 0, "u64", 24),
    ] {
        extent_layout()
            .validate_backend_layout(&backend)
            .expect_err("backend Extent structural drift must reject");
    }
}
