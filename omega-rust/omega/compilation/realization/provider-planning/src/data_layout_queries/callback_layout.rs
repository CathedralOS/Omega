/*
We represent exact target-closed inline paths to compiler-private callback slots
(`PrivateCallbackLayout` and `PrivateCallbackInlineField`) along with their
geometry validation.

Natural and policy-laid records share this path owner. Each hop retains its
named declaration and native geometry (references never become inline hops),
and the terminal slot retains its selected policy and conformance. Arena's
zero-handle default row carries an empty named identity so it can never
satisfy geometry validation.
*/

use crate::data_layout_queries::sizing::pointer_layout;
use crate::data_layout_queries::{
    TargetClosedDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
};
use arena::{Arena, HandleSpan};
use calling_conventions::{
    NativeParameterId, NativePlace, callback_layout_field_slot_id, callback_layout_slot_id,
    callback_requirement_id,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackLayout {
    root_layout: TargetClosedDataLayoutIdentity,
    fields: Arena<PrivateCallbackInlineField>,
    path: HandleSpan<PrivateCallbackInlineField>,
    terminal_slot: TargetClosedPrivateCallbackDemand,
    composed_placements: calling_conventions::PrivateCallbackPlacements,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCallbackInlineField {
    pub(crate) symbol: SymbolHandle,
    pub(crate) field_layout: crate::data_layout_queries::FieldLayout,
    pub(crate) identity: Arc<str>,
    pub(crate) offset: usize,
    pub(crate) extent: usize,
    pub(crate) alignment: usize,
    pub(crate) child_layout: TargetClosedDataLayoutIdentity,
}

impl Default for PrivateCallbackInlineField {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::default(),
            field_layout: crate::data_layout_queries::FieldLayout::default(),
            identity: Arc::from(""),
            offset: 0,
            extent: 0,
            alignment: 1,
            child_layout: TargetClosedDataLayoutIdentity {
                data_symbol: SymbolHandle::default(),
                data_identity: Arc::from(""),
                layout_subject_identity: None,
                layout: calling_conventions::LayoutPlanId::new(1).expect("nonzero arena dummy"),
                physical: crate::data_layout_queries::TypeLayout::default(),
            },
        }
    }
}

impl PrivateCallbackLayout {
    pub const fn root_layout(&self) -> &TargetClosedDataLayoutIdentity {
        &self.root_layout
    }
    pub fn inline_fields(&self) -> Result<&[PrivateCallbackInlineField], String> {
        self.fields
            .span(self.path)
            .ok_or_else(|| "callback inline path has stale field storage".to_owned())
    }
    pub const fn terminal_slot(&self) -> &TargetClosedPrivateCallbackDemand {
        &self.terminal_slot
    }
    pub fn composed_placements(&self) -> &[layout_plans::LayoutPlacementReport] {
        self.composed_placements.as_slice()
    }

    pub(crate) fn from_fields(
        root_layout: TargetClosedDataLayoutIdentity,
        inline_fields: &[PrivateCallbackInlineField],
        terminal_slot: TargetClosedPrivateCallbackDemand,
    ) -> Result<Self, String> {
        let mut fields = Arena::with_capacity(inline_fields.len());
        let mut path = HandleSpan::empty();
        let mut base = 0u64;
        for field in inline_fields {
            base = base
                .checked_add(field.offset as u64)
                .ok_or_else(|| "callback inline path offset overflowed".to_owned())?;
            fields.append_to_span(&mut path, field.clone());
        }
        let composed_placements = layout_plans::compose_private_callback_placements(
            terminal_slot.placements.as_slice(),
            base,
        )?;
        Ok(Self {
            root_layout,
            fields,
            path,
            terminal_slot,
            composed_placements: calling_conventions::PrivateCallbackPlacements::new(
                &composed_placements,
            ),
        })
    }

    pub fn native_demand(
        &self,
        parameter: NativeParameterId,
    ) -> Result<calling_conventions::NativeCallbackDemand, String> {
        let mut parent = &self.root_layout;
        let mut field_path = Vec::new();
        for field in self.inline_fields()? {
            field_path.push(callback_layout_field_slot_id(
                parent.layout,
                &field.identity,
            ));
            parent = &field.child_layout;
        }
        field_path.push(self.terminal_slot.slot);
        Ok(calling_conventions::NativeCallbackDemand {
            destination: NativePlace::Field {
                parameter,
                layout: self.root_layout.layout,
                field_path,
            },
            requirement: self.terminal_slot.requirement,
        })
    }

    pub fn validate(&self, destination: &NativePlace, target: NativeTarget) -> Result<(), String> {
        let NativePlace::Field { parameter, .. } = destination else {
            return Err("callback layout catalog contains a non-field destination".to_owned());
        };
        let terminal = &self.terminal_slot;
        if terminal.slot_identity.is_empty()
            || terminal.callback_requirement_identity.is_empty()
            || terminal.slot != callback_layout_slot_id(terminal.layout, &terminal.slot_identity)
            || terminal.requirement
                != callback_requirement_id(&terminal.callback_requirement_identity)
            || !terminal.slot_application.has_complete_identity()
            || terminal.slot_application.layout_identity
                != terminal.layout_subject_identity.as_ref()
            || terminal.slot_application.requirement_identity
                != terminal.callback_requirement_identity.as_ref()
        {
            return Err("callback layout catalog lost its named slot application".to_owned());
        }
        if self.native_demand(*parameter)?.destination != *destination {
            return Err("callback layout catalog changed its exact ordered field path".to_owned());
        }
        validate_geometry(self, target)
    }
}

impl PrivateCallbackInlineField {
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub const fn offset(&self) -> usize {
        self.offset
    }
    pub const fn extent(&self) -> usize {
        self.extent
    }
    pub const fn alignment(&self) -> usize {
        self.alignment
    }
    pub const fn child_layout(&self) -> &TargetClosedDataLayoutIdentity {
        &self.child_layout
    }
}

fn validate_geometry(entry: &PrivateCallbackLayout, target: NativeTarget) -> Result<(), String> {
    let root = &entry.root_layout;
    let terminal = &entry.terminal_slot;
    let slot = pointer_layout(target);
    if !valid_layout(root)
        || terminal.byte_size == 0
        || terminal.byte_size != slot.size
        || terminal.alignment != slot.alignment
    {
        return Err(
            "callback layout catalog changed its root or target pointer geometry".to_owned(),
        );
    }
    let mut parent = root;
    let mut offset = 0usize;
    let mut seen = vec![root.data_symbol];
    for field in entry.inline_fields()? {
        let child = &field.child_layout;
        if seen.contains(&child.data_symbol)
            || !field.symbol.is_valid()
            || field.identity.is_empty()
            || !valid_layout(child)
            || field.field_layout.symbol != field.symbol
            || field.field_layout.type_descriptor.inline_data_symbol() != Some(child.data_symbol)
            || field.field_layout.offset != field.offset
            || field.field_layout.layout != child.physical
            || field.extent != child.physical.size
            || field.alignment != child.physical.alignment
            || !contained(parent, field.offset, field.extent, field.alignment)
        {
            return Err(
                "callback layout catalog changed its named inline field geometry".to_owned(),
            );
        }
        offset = offset
            .checked_add(field.offset)
            .ok_or_else(|| "callback inline path offset overflowed".to_owned())?;
        seen.push(child.data_symbol);
        parent = child;
    }
    if parent.data_symbol != terminal.data_symbol
        || parent.layout_subject_identity.as_deref()
            != Some(terminal.layout_subject_identity.as_ref())
    {
        return Err("callback layout catalog changed its terminal owner".to_owned());
    }
    layout_plans::validate_private_callback_placements(
        terminal.placements.as_slice(),
        terminal.byte_size as u64,
        terminal.alignment as u64,
        parent.physical.size as u64,
        parent.physical.alignment as u64,
    )?;
    let placements = layout_plans::compose_private_callback_placements(
        terminal.placements.as_slice(),
        offset as u64,
    )?;
    if placements.as_slice() != entry.composed_placements.as_slice() {
        return Err("callback layout catalog changed its composed placements".to_owned());
    }
    layout_plans::validate_private_callback_placements(
        &placements,
        terminal.byte_size as u64,
        terminal.alignment as u64,
        root.physical.size as u64,
        root.physical.alignment as u64,
    )?;
    Ok(())
}

fn valid_layout(layout: &TargetClosedDataLayoutIdentity) -> bool {
    layout.data_symbol.is_valid()
        && !layout.data_identity.is_empty()
        && layout
            .layout_subject_identity
            .as_ref()
            .is_none_or(|identity| !identity.is_empty())
        && layout.physical.alignment.is_power_of_two()
}

fn contained(
    layout: &TargetClosedDataLayoutIdentity,
    offset: usize,
    extent: usize,
    alignment: usize,
) -> bool {
    alignment.is_power_of_two()
        && layout.physical.alignment >= alignment
        && offset.is_multiple_of(alignment)
        && offset
            .checked_add(extent)
            .is_some_and(|end| end <= layout.physical.size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_layout_queries::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
    use calling_conventions::{LayoutPlanId, callback_layout_slot_id, callback_requirement_id};
    use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
    use layout_plans::LayoutPlacementReport;

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
            slot_application: PrivateCallbackSlotSelection {
                declaration_identity: "fixture::Slot".into(),
                trait_identity: "core::layout::PrivateCallbackSlot".into(),
                semantic_identity: owner.data_identity.to_string(),
                layout_identity: owner.layout_subject_identity.as_ref().unwrap().to_string(),
                requirement_identity: "fixture::Procedure::call".into(),
                application_report_fingerprint: 7,
                application_commitment: [7; 32],
                application_encoding: vec![7].into(),
            },
            slot_identity: "fixture::Slot".into(),
            layout_subject_identity: owner.layout_subject_identity.as_ref().unwrap().clone(),
            callback_requirement_identity: "fixture::Procedure::call".into(),
            layout: owner.layout,
            slot: callback_layout_slot_id(owner.layout, "fixture::Slot"),
            requirement: callback_requirement_id("fixture::Procedure::call"),
            placements: calling_conventions::PrivateCallbackPlacements::new(&[
                LayoutPlacementReport::At { offset: 8 },
            ]),
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
            assert_eq!(
                layout.composed_placements(),
                &[LayoutPlacementReport::At {
                    offset: u64::try_from(8 * (depth + 1)).unwrap(),
                }]
            );
        }
    }

    #[test]
    fn path_mutations_do_not_retarget_a_private_slot() {
        let target = NativeTarget::windows_x64();
        let base = fixture(3);
        let expected = destination(&base);
        let mut changed = base.clone();
        changed.terminal_slot.placements =
            calling_conventions::PrivateCallbackPlacements::new(&[LayoutPlacementReport::At {
                offset: 16,
            }]);
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
        changed.terminal_slot.slot_application.layout_identity = "foreign::Policy".into();
        assert!(changed.validate(&expected, target).is_err());
        let mut changed = base.clone();
        changed.terminal_slot.slot_application.requirement_identity = "foreign::call".into();
        assert!(changed.validate(&expected, target).is_err());
        let mut changed = base.clone();
        changed.terminal_slot.byte_size = 4;
        assert!(changed.validate(&expected, target).is_err());
    }
}
