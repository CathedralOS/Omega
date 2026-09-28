use super::{PrivateCallbackInlineField, PrivateCallbackLayout};
use crate::{
    FieldLayout, TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
    TargetClosedTwoHopPrivateCallbackPath, TypeLayout,
};
use calling_conventions::{
    LayoutPlanId, NativeParameterId, NativePlace, callback_layout_field_slot_id,
    callback_layout_slot_id, callback_requirement_id,
};
use symbols::SymbolHandle;
use target::NativeTarget;
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
};

fn direct_fixture(target: NativeTarget) -> PrivateCallbackLayout {
    let pointer = target.pointer_size;
    let alignment = target.pointer_alignment;
    let data_symbol = SymbolHandle::from_arena_index(127);
    let layout = LayoutPlanId::new(109).unwrap();
    let slot_identity = "fixture::CallbackSlot".into();
    let requirement_identity = "fixture::Callback::call".into();
    let terminal_slot = TargetClosedPrivateCallbackDemand {
        data_symbol,
        slot_identity,
        slot_application: ClosedConformanceApplication {
            arguments: Box::new([]),
            declaration: SymbolHandle::from_arena_index(131),
            trait_definition: SymbolHandle::from_arena_index(137),
            commitment: ClosedConformanceApplicationCommitment::from_digest([7; 32]),
            ..ClosedConformanceApplication::default()
        },
        layout_subject_identity: "fixture::Layout".into(),
        callback_requirement_identity: requirement_identity,
        layout,
        slot: callback_layout_slot_id(layout, "fixture::CallbackSlot"),
        requirement: callback_requirement_id("fixture::Callback::call"),
        offset: pointer,
        byte_size: pointer,
        alignment,
    };

    PrivateCallbackLayout {
        root_layout: TargetClosedPlanLaidDataLayoutIdentity {
            data_symbol,
            data_identity: "fixture::Record".into(),
            layout_subject_identity: terminal_slot.layout_subject_identity.clone(),
            // Named root and legacy terminal layouts intentionally use different IDs.
            layout: LayoutPlanId::new(139).unwrap(),
            physical: TypeLayout {
                size: 4 * pointer,
                alignment,
            },
        },
        inline_field: None,
        terminal_slot,
        composed_offset: pointer,
    }
}

fn two_hop_fixture(target: NativeTarget) -> PrivateCallbackLayout {
    let pointer = target.pointer_size;
    let alignment = target.pointer_alignment;
    let mut entry = direct_fixture(target);
    let child_symbol = SymbolHandle::from_arena_index(149);
    entry.terminal_slot.data_symbol = child_symbol;
    entry.terminal_slot.layout_subject_identity = "fixture::ChildLayout".into();
    let field_identity = "fixture::Record::child";
    entry.inline_field = Some(PrivateCallbackInlineField {
        symbol: SymbolHandle::from_arena_index(151),
        identity: field_identity.into(),
        offset: pointer,
        extent: 2 * pointer,
        alignment,
        child_layout: TargetClosedPlanLaidDataLayoutIdentity {
            data_symbol: child_symbol,
            data_identity: "fixture::Child".into(),
            layout_subject_identity: entry.terminal_slot.layout_subject_identity.clone(),
            layout: LayoutPlanId::new(157).unwrap(),
            physical: TypeLayout {
                size: 2 * pointer,
                alignment,
            },
        },
    });
    entry.composed_offset = 2 * pointer;
    entry
}

fn destination(layout: &PrivateCallbackLayout) -> NativePlace {
    let parameter = NativeParameterId::new(107).unwrap();
    match &layout.inline_field {
        Some(field) => NativePlace::Field {
            parameter,
            layout: layout.root_layout.layout,
            field_path: vec![
                callback_layout_field_slot_id(layout.root_layout.layout, &field.identity),
                layout.terminal_slot.slot,
            ],
        },
        None => layout.terminal_slot.native_demand(parameter).destination,
    }
}

#[test]
fn direct_and_inline_callback_layouts_replay_for_supported_hosts() {
    for target in [
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
        NativeTarget::macos_x64(),
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
    ] {
        for layout in [direct_fixture(target), two_hop_fixture(target)] {
            layout.validate(&destination(&layout), target).unwrap();
            let mut wrong_pointer = target;
            wrong_pointer.pointer_size *= 2;
            assert!(
                layout
                    .validate(&destination(&layout), wrong_pointer)
                    .is_err()
            );
            let mut wrong_alignment = target;
            wrong_alignment.pointer_alignment *= 2;
            assert!(
                layout
                    .validate(&destination(&layout), wrong_alignment)
                    .is_err()
            );
        }
    }
}

#[test]
fn callback_layout_rejects_named_slot_and_geometry_drift_without_panicking() {
    let mutations: &[fn(&mut PrivateCallbackLayout)] = &[
        |entry| entry.terminal_slot.slot_identity = "fixture::ForeignSlot".into(),
        |entry| entry.terminal_slot.callback_requirement_identity = "fixture::Foreign::call".into(),
        |entry| entry.terminal_slot.requirement = callback_requirement_id("fixture::Foreign::call"),
        |entry| entry.terminal_slot.slot_application.declaration = SymbolHandle::invalid(),
        |entry| entry.terminal_slot.slot_application.trait_definition = SymbolHandle::invalid(),
        |entry| {
            entry.terminal_slot.slot_application.commitment =
                ClosedConformanceApplicationCommitment::default()
        },
        |entry| entry.terminal_slot.layout_subject_identity = "fixture::ForeignLayout".into(),
        |entry| entry.terminal_slot.data_symbol = SymbolHandle::from_arena_index(173),
        |entry| entry.terminal_slot.offset = usize::MAX,
        |entry| entry.terminal_slot.byte_size = usize::MAX,
        |entry| entry.terminal_slot.alignment = 0,
        |entry| entry.root_layout.physical.size = 0,
        |entry| entry.root_layout.physical.alignment = 0,
        |entry| entry.root_layout.data_identity = "".into(),
        |entry| entry.composed_offset = usize::MAX,
    ];
    let target = NativeTarget::host();
    for fixture in [direct_fixture, two_hop_fixture] {
        for (mutation_index, mutate) in mutations.iter().enumerate() {
            let mut layout = fixture(target);
            let destination = destination(&layout);
            mutate(&mut layout);
            assert!(
                layout.validate(&destination, target).is_err(),
                "mutation {mutation_index}"
            );
        }
    }
}

#[test]
fn inline_callback_layout_rejects_child_field_and_composition_drift() {
    let mutations: &[fn(&mut PrivateCallbackInlineField)] = &[
        |field| field.symbol = SymbolHandle::invalid(),
        |field| field.identity = "fixture::Record::foreign".into(),
        |field| field.offset = usize::MAX,
        |field| field.extent = usize::MAX,
        |field| field.alignment = 0,
        |field| field.child_layout.data_symbol = SymbolHandle::invalid(),
        |field| field.child_layout.data_symbol = SymbolHandle::from_arena_index(179),
        |field| field.child_layout.layout_subject_identity = "fixture::ForeignChild".into(),
        |field| field.child_layout.physical.size = 0,
        |field| field.child_layout.physical.alignment = 0,
    ];
    let target = NativeTarget::host();
    for (mutation_index, mutate) in mutations.iter().enumerate() {
        let mut layout = two_hop_fixture(target);
        let destination = destination(&layout);
        mutate(layout.inline_field.as_mut().unwrap());
        assert!(
            layout.validate(&destination, target).is_err(),
            "mutation {mutation_index}"
        );
    }
}

#[test]
fn callback_layout_rejects_changed_native_field_paths() {
    let target = NativeTarget::host();
    for layout in [direct_fixture(target), two_hop_fixture(target)] {
        let mut destination = destination(&layout);
        let NativePlace::Field { field_path, .. } = &mut destination else {
            unreachable!()
        };
        field_path.clear();
        assert!(layout.validate(&destination, target).is_err());
        assert!(
            layout
                .validate(
                    &NativePlace::Parameter(NativeParameterId::new(107).unwrap()),
                    target
                )
                .is_err()
        );
    }
}

#[test]
fn direct_layout_construction_requires_one_exact_named_root() {
    let target = NativeTarget::host();
    let expected = direct_fixture(target);
    let mut layouts = Vec::new();
    let terminal = &expected.terminal_slot;
    assert!(PrivateCallbackLayout::direct(&layouts, terminal).is_err());
    let mut foreign = expected.root_layout.clone();
    foreign.layout_subject_identity = "fixture::ForeignLayout".into();
    layouts.push(foreign);
    assert!(PrivateCallbackLayout::direct(&layouts, terminal).is_err());
    layouts.push(expected.root_layout.clone());
    assert_eq!(
        PrivateCallbackLayout::direct(&layouts, terminal).unwrap(),
        expected
    );
    layouts.push(expected.root_layout.clone());
    assert!(PrivateCallbackLayout::direct(&layouts, terminal).is_err());
}

#[test]
fn inline_layout_construction_retains_exact_path_and_rejects_foreign_child() {
    let target = NativeTarget::host();
    let expected = two_hop_fixture(target);
    let field = expected.inline_field.as_ref().unwrap();
    let mut path = TargetClosedTwoHopPrivateCallbackPath {
        root_layout_index: 0,
        root_layout: expected.root_layout.clone(),
        field_symbol: field.symbol,
        field: arena::Handle::invalid(),
        field_layout: FieldLayout::default(),
        field_identity: field.identity.clone(),
        field_slot: callback_layout_field_slot_id(expected.root_layout.layout, &field.identity),
        field_relative_offset: field.offset,
        field_extent: field.extent,
        field_alignment: field.alignment,
        child_layout_index: 1,
        child_layout: field.child_layout.clone(),
        terminal_demand_index: 0,
        terminal_demand: expected.terminal_slot.clone(),
        composed_offset: expected.composed_offset,
    };
    let retained = PrivateCallbackLayout::two_hop(&path).unwrap();
    assert_eq!(retained, expected);
    retained
        .validate(
            &path
                .native_demand(NativeParameterId::new(107).unwrap())
                .destination,
            target,
        )
        .unwrap();
    path.field_slot =
        callback_layout_field_slot_id(path.root_layout.layout, "fixture::ForeignField");
    assert!(
        retained
            .validate(
                &path
                    .native_demand(NativeParameterId::new(107).unwrap())
                    .destination,
                target
            )
            .is_err()
    );
    path.child_layout.data_symbol = SymbolHandle::invalid();
    assert!(PrivateCallbackLayout::two_hop(&path).is_err());
}
