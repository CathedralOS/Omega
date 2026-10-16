//! Data-only path/geometry controls; real private-field invocation is a corpus oracle.
use super::*;
use crate::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use calling_conventions::{LayoutPlanId, callback_layout_slot_id, callback_requirement_id};
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
};
fn identity(index: u32, size: usize, policy: bool) -> TargetClosedDataLayoutIdentity {
    TargetClosedDataLayoutIdentity {
        data_symbol: SymbolHandle::from_arena_index(index),
        data_identity: format!("fixture::Data{index}").into(),
        layout_subject_identity: policy.then(|| format!("fixture::Layout{index}").into()),
        layout: LayoutPlanId::new(u64::from(index) + 100).unwrap(),
        physical: TypeLayout { size, alignment: 8 },
    }
}
fn fixture(depth: usize) -> PrivateCallbackLayout {
    let owner = identity(17, 16, true);
    let terminal = TargetClosedPrivateCallbackDemand {
        data_symbol: owner.data_symbol,
        slot_application: ClosedConformanceApplication {
            declaration: SymbolHandle::from_arena_index(23),
            trait_definition: SymbolHandle::from_arena_index(29),
            arguments: Box::new([]),
            commitment: ClosedConformanceApplicationCommitment::from_digest([7; 32]),
            ..ClosedConformanceApplication::default()
        },
        slot_identity: "fixture::Slot".into(),
        layout_subject_identity: owner.layout_subject_identity.as_ref().unwrap().clone(),
        callback_requirement_identity: "fixture::Procedure::call".into(),
        layout: owner.layout,
        slot: callback_layout_slot_id(owner.layout, "fixture::Slot"),
        requirement: callback_requirement_id("fixture::Procedure::call"),
        offset: 8,
        byte_size: 8,
        alignment: 8,
    };
    let mut root = owner.clone();
    let mut fields = Vec::new();
    for level in (0..depth).rev() {
        let parent = identity(
            31 + u32::try_from(level).unwrap(),
            root.physical.size + 8,
            false,
        );
        let symbol = SymbolHandle::from_arena_index(53 + u32::try_from(level).unwrap());
        fields.insert(
            0,
            PrivateCallbackInlineField {
                symbol,
                identity: format!("fixture::field{level}").into(),
                offset: 8,
                extent: root.physical.size,
                alignment: 8,
                field_layout: FieldLayout {
                    symbol,
                    offset: 8,
                    type_symbol: root.data_symbol,
                    type_descriptor: TypeLayoutDescriptor::Named {
                        symbol: root.data_symbol,
                        name: Default::default(),
                    },
                    layout: root.physical,
                    ..FieldLayout::default()
                },
                child_layout: root.clone(),
            },
        );
        root = parent;
    }
    PrivateCallbackLayout::from_fields(root, &fields, terminal).unwrap()
}
fn destination(layout: &PrivateCallbackLayout) -> NativePlace {
    layout
        .native_demand(NativeParameterId::new(1).unwrap())
        .unwrap()
        .destination
}
#[test]
fn direct_natural_and_deep_inline_paths_preserve_exact_geometry() {
    for depth in [0, 1, 3, 5] {
        let layout = fixture(depth);
        layout
            .validate(&destination(&layout), NativeTarget::windows_x64())
            .unwrap();
        assert_eq!(layout.inline_fields().unwrap().len(), depth);
        assert_eq!(layout.composed_offset(), 8 * (depth + 1));
    }
}
#[test]
fn path_mutations_do_not_retarget_a_private_slot() {
    let target = NativeTarget::windows_x64();
    let base = fixture(3);
    let expected = destination(&base);
    let mut changed = base.clone();
    changed.terminal_slot.offset += 8;
    assert!(changed.validate(&expected, target).is_err());
    let mut changed = base.clone();
    changed.terminal_slot.slot = calling_conventions::LayoutSlotId::new(991).unwrap();
    assert!(changed.validate(&expected, target).is_err());
    let mut swapped = expected.clone();
    if let NativePlace::Field { field_path, .. } = &mut swapped {
        field_path.swap(0, 1);
    }
    assert!(base.validate(&swapped, target).is_err());
    let mut changed = base.clone();
    let handle = changed.path.start();
    changed.fields.get_mut(handle).offset += 8;
    assert!(changed.validate(&expected, target).is_err());
    let mut changed = base.clone();
    let field = changed.fields.get_mut(changed.path.start());
    field.field_layout.type_descriptor = TypeLayoutDescriptor::Reference {
        referee: Box::new(field.field_layout.type_descriptor.clone()),
        is_mutable: false,
    };
    assert!(changed.validate(&expected, target).is_err());
    let mut changed = base.clone();
    changed.terminal_slot.layout_subject_identity = "foreign::Policy".into();
    assert!(changed.validate(&expected, target).is_err());
    let mut changed = base.clone();
    changed.terminal_slot.byte_size = 4;
    assert!(changed.validate(&expected, target).is_err());
}
