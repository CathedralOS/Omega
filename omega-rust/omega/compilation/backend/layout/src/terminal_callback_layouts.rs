//! Complete inline callback paths from canonical declarations and selected bytes.
//! References are carrier leaves. Only an explicitly supplied formal backing
//! root starts a path; an identity-only referent never proves absence of slots.

use calling_conventions::{
    BoundaryCallbackLayout, CallbackDataGeometry, CallbackInlineFieldGeometry,
    CallbackSlotGeometry, LayoutPlanId, NativeParameterId, NativePlace,
    callback_layout_field_slot_id, callback_layout_slot_id, callback_requirement_id,
};
use semantic_vocabulary::{AuthoredFieldStorage, ScalarApplicationId, StructuralTypeId};
use target::TargetProfile;
use terminal_psi::{StructuralTypeShape, TerminalModule};
use terminal_semantics::{ReferenceRecordField, ReferenceStorageGeometry, StorageGeometryError};

/// Exact semantic/native association retained by the checked formal producer.
pub struct CallbackStorageRoot {
    pub record: StructuralTypeId,
    pub parameter: NativeParameterId,
    pub formal_ordinal: u32,
    pub native_ordinal: u32,
}

pub struct TerminalCallbackLayouts<'module, Extents> {
    module: &'module TerminalModule,
    target: TargetProfile,
    extents: Extents,
    closed: Vec<ClosedRecord>,
}

struct ClosedRecord {
    record: StructuralTypeId,
    geometry: CallbackDataGeometry,
    children: Vec<InlineChild>,
    slots: Vec<CallbackSlotGeometry>,
}

#[derive(Clone)]
struct InlineChild {
    record: StructuralTypeId,
    identity: String,
    offset: u64,
    size: u64,
    alignment: u64,
}

impl<'module, Extents> TerminalCallbackLayouts<'module, Extents>
where
    Extents: FnMut(ScalarApplicationId) -> Result<u64, StorageGeometryError>,
{
    pub fn new(module: &'module TerminalModule, target: TargetProfile, extents: Extents) -> Self {
        Self {
            module,
            target,
            extents,
            closed: Vec::new(),
        }
    }

    pub fn for_parameter(
        &mut self,
        root: CallbackStorageRoot,
    ) -> Result<Vec<BoundaryCallbackLayout>, String> {
        let mut output = Vec::new();
        if !Self::reaches_slot(self.module, self.target, root.record, &mut Vec::new())? {
            return Ok(output);
        }
        let root_index = self.close_record(root.record)?;
        let geometry = self.closed[root_index].geometry.clone();
        self.walk(
            &root,
            &geometry,
            root.record,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut output,
        )?;
        output.sort_by(|left, right| left.destination.cmp(&right.destination));
        if output
            .windows(2)
            .any(|pair| pair[0].destination == pair[1].destination)
        {
            return Err("canonical callback storage repeats one exact inline path".into());
        }
        Ok(output)
    }

    fn reaches_slot(
        module: &TerminalModule,
        target: TargetProfile,
        record: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> Result<bool, String> {
        if active.len() >= 128 || active.contains(&record) {
            return Err("canonical callback storage has a recursive inline record path".into());
        }
        let declaration = module
            .structural_type(record)
            .ok_or("callback backing root has no retained storage declaration")?;
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            return Ok(false);
        };
        let native = module
            .native_layouts
            .record(record)
            .ok_or("callback backing record lost its native layout association")?;
        if module
            .native_layouts
            .for_target(native, target.target_name())?
            .is_some_and(|row| !row.report.private_callback_demands.is_empty())
        {
            return Ok(true);
        }
        active.push(record);
        let result = (|| {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                if let Some(child) = field.field_type.as_structural() {
                    let child_declaration = module
                        .structural_type(child)
                        .ok_or("callback inline field has no retained storage declaration")?;
                    if matches!(child_declaration.shape, StructuralTypeShape::Record { .. })
                        && Self::reaches_slot(module, target, child, active)?
                    {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })();
        active.pop();
        result
    }

    fn close_record(&mut self, record: StructuralTypeId) -> Result<usize, String> {
        if let Some(index) = self.closed.iter().position(|row| row.record == record) {
            return Ok(index);
        }
        let native = self
            .module
            .native_layouts
            .record(record)
            .ok_or("callback record lost its native layout association")?;
        let data_identity = self
            .module
            .identities
            .text(native.data_identity)
            .filter(|identity| !identity.is_empty())
            .ok_or("callback record has no complete nominal data identity")?;
        let nominal_fields = self.module.native_layouts.fields(native)?;
        let authored = crate::terminal_layout_identity(self.module, record, self.target)?;
        let mut storage =
            ReferenceStorageGeometry::new(self.module.structural_types.iter(), &mut self.extents)
                .with_target_carriers(Some(runtime_abi::target_carrier_layout(
                    self.target.native_target(),
                )))
                .with_record_storage(crate::TerminalRecordStorage {
                    module: self.module,
                    target: self.target,
                });
        let (size, alignment) = storage.layout(record).map_err(|error| error.to_string())?;
        let fields = storage
            .record_fields(record)
            .map_err(|error| error.to_string())?;
        let mut natural_bytes = Vec::new();
        for quantity in [
            size,
            alignment,
            self.target.native_target().pointer_size as u64,
            self.target.native_target().pointer_alignment as u64,
        ] {
            natural_bytes.extend_from_slice(&quantity.to_le_bytes());
        }
        let mut children = Vec::new();
        let mut occupied = Vec::new();
        for field in fields {
            let nominal = nominal_fields
                .iter()
                .find(|nominal| nominal.field == field.declaration.id)
                .ok_or("callback field lost its exact nominal declaration association")?;
            let identity = self
                .module
                .identities
                .text(nominal.declaration_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("callback field has no complete nominal declaration identity")?;
            if authored.is_none() {
                let offset = field
                    .byte_offset
                    .ok_or("natural callback record has fragmented field storage")?;
                natural_bytes.extend_from_slice(&(identity.len() as u64).to_le_bytes());
                natural_bytes.extend_from_slice(identity.as_bytes());
                for quantity in [offset, field.byte_size, field.byte_alignment] {
                    natural_bytes.extend_from_slice(&quantity.to_le_bytes());
                }
            }
            if let Some(child) = field.declaration.field_type.as_structural()
                && self
                    .module
                    .structural_type(child)
                    .is_some_and(|child| matches!(child.shape, StructuralTypeShape::Record { .. }))
                && Self::reaches_slot(self.module, self.target, child, &mut Vec::new())?
            {
                if field.repeated.is_some()
                    || matches!(field.storage, Some(AuthoredFieldStorage::Fragmented(_)))
                {
                    return Err(
                        "callback inline record does not have one direct field placement".into(),
                    );
                }
                children.push(InlineChild {
                    record: child,
                    identity: identity.to_owned(),
                    offset: field
                        .byte_offset
                        .ok_or("callback inline field has no direct byte offset")?,
                    size: field.byte_size,
                    alignment: field.byte_alignment,
                });
            }
            occupied.extend(occupied_field_bytes(&field, size)?);
        }
        let (layout, policy) = match &authored {
            Some(authored) => (
                authored.data_layout,
                Some(authored.policy_identity.to_owned()),
            ),
            None => (
                LayoutPlanId::new(calling_conventions::callback_nominal_report_fingerprint(
                    b"omega.callback-natural-data-layout.v1",
                    &[data_identity.as_bytes(), &natural_bytes],
                ))
                .ok_or("callback natural layout has an absent identity")?,
                None,
            ),
        };
        let geometry = CallbackDataGeometry {
            identity: data_identity.to_owned(),
            policy,
            layout,
            size,
            alignment,
        };
        let mut slots: Vec<CallbackSlotGeometry> = Vec::new();
        if let Some(authored) = authored {
            let pointer = self.target.native_target();
            for demand in &authored.realization.report.private_callback_demands {
                if demand.layout_subject_identity != authored.policy_identity
                    || demand.slot_application.layout_identity != authored.policy_identity
                    || demand.slot_application.semantic_identity != authored.semantic_identity
                    || demand.callback_requirement_identity
                        != demand.slot_application.requirement_identity
                {
                    return Err("callback slot changed its exact semantic, policy or requirement application".into());
                }
                let end = demand
                    .offset
                    .checked_add(pointer.pointer_size as u64)
                    .ok_or("callback slot extent overflows")?;
                if pointer.pointer_alignment == 0
                    || !pointer.pointer_alignment.is_power_of_two()
                    || alignment < pointer.pointer_alignment as u64
                    || !demand
                        .offset
                        .is_multiple_of(pointer.pointer_alignment as u64)
                    || end > size
                {
                    return Err(
                        "callback slot has invalid target alignment or containing extent".into(),
                    );
                }
                if occupied
                    .iter()
                    .any(|range| range.overlaps(demand.offset, end))
                {
                    return Err("callback slot overlaps semantic field storage".into());
                }
                if slots
                    .iter()
                    .any(|prior| prior.offset < end && demand.offset < prior.offset + prior.size)
                {
                    return Err("canonical callback private slots overlap".into());
                }
                let slot = callback_layout_slot_id(authored.slot_layout, &demand.slot_identity);
                let requirement = callback_requirement_id(&demand.callback_requirement_identity);
                if slots.iter().any(|prior| {
                    prior.slot == slot
                        || (prior.requirement == requirement
                            && prior.requirement_identity != demand.callback_requirement_identity)
                }) {
                    return Err(
                        "canonical callback nominal slot or requirement identity collides".into(),
                    );
                }
                slots.push(CallbackSlotGeometry {
                    identity: demand.slot_identity.clone(),
                    policy: demand.layout_subject_identity.clone(),
                    requirement_identity: demand.callback_requirement_identity.clone(),
                    layout: authored.slot_layout,
                    slot,
                    requirement,
                    conformance_commitment: demand.slot_application.application_commitment,
                    offset: demand.offset,
                    size: pointer.pointer_size as u64,
                    alignment: pointer.pointer_alignment as u64,
                });
            }
        }
        let index = self.closed.len();
        self.closed.push(ClosedRecord {
            record,
            geometry,
            children,
            slots,
        });
        Ok(index)
    }

    fn walk(
        &mut self,
        formal: &CallbackStorageRoot,
        root: &CallbackDataGeometry,
        record: StructuralTypeId,
        path: &mut Vec<CallbackInlineFieldGeometry>,
        active: &mut Vec<StructuralTypeId>,
        output: &mut Vec<BoundaryCallbackLayout>,
    ) -> Result<(), String> {
        if active.contains(&record) || active.len() >= 128 {
            return Err("canonical callback inline path is recursive".into());
        }
        active.push(record);
        let index = self.close_record(record)?;
        for ordinal in 0..self.closed[index].slots.len() {
            let terminal = self.closed[index].slots[ordinal].clone();
            let mut parent = root;
            let mut field_path = Vec::with_capacity(path.len() + 1);
            for field in path.iter() {
                field_path.push(callback_layout_field_slot_id(
                    parent.layout,
                    &field.identity,
                ));
                parent = &field.child;
            }
            field_path.push(terminal.slot);
            let layout = BoundaryCallbackLayout::new(
                formal.formal_ordinal,
                formal.native_ordinal,
                NativePlace::Field {
                    parameter: formal.parameter,
                    layout: root.layout,
                    field_path,
                },
                root.clone(),
                path,
                terminal,
            );
            layout.field_offset(self.target.native_target())?;
            output.push(layout);
        }
        for ordinal in 0..self.closed[index].children.len() {
            let field = self.closed[index].children[ordinal].clone();
            let child = self.close_record(field.record)?;
            let geometry = self.closed[child].geometry.clone();
            if field.size != geometry.size || field.alignment != geometry.alignment {
                return Err("callback inline field differs from its exact child storage".into());
            }
            path.push(CallbackInlineFieldGeometry {
                identity: field.identity,
                offset: field.offset,
                child: geometry,
            });
            self.walk(formal, root, field.record, path, active, output)?;
            path.pop();
        }
        active.pop();
        Ok(())
    }
}

enum OccupiedBytes {
    Contiguous {
        start: u64,
        end: u64,
    },
    Repeated {
        start: u64,
        element_size: u64,
        element_count: u64,
        stride: u64,
    },
}

impl OccupiedBytes {
    fn overlaps(&self, start: u64, end: u64) -> bool {
        match *self {
            Self::Contiguous {
                start: occupied,
                end: occupied_end,
            } => occupied < end && start < occupied_end,
            Self::Repeated {
                start: occupied,
                element_size,
                element_count,
                stride,
            } => {
                if element_size == 0 || element_count == 0 || end <= occupied {
                    return false;
                }
                // The nearest element preceding the queried end is the last
                // possible overlap. Padding never expands into element bytes.
                let last = ((end - 1 - occupied) / stride).min(element_count - 1);
                occupied + last * stride + element_size > start
            }
        }
    }
}

fn occupied_field_bytes(
    field: &ReferenceRecordField<'_>,
    bound: u64,
) -> Result<Vec<OccupiedBytes>, String> {
    let mut output = Vec::new();
    if let Some(AuthoredFieldStorage::Fragmented(fragments)) = field.storage {
        for fragment in fragments.fragments() {
            let end = fragment
                .container_byte_offset
                .checked_add(fragment.container_width_bits.div_ceil(8))
                .ok_or("callback transfer container extent overflows")?;
            if end > bound {
                return Err("callback transfer container exceeds its record".into());
            }
            output.push(OccupiedBytes::Contiguous {
                start: fragment.container_byte_offset,
                end,
            });
        }
    } else {
        let start = field
            .byte_offset
            .ok_or("callback field lost its actual storage offset")?;
        if let Some(repeated) = field.repeated {
            if repeated.element_count != 0 && repeated.element_size != 0 {
                if repeated.element_stride < repeated.element_size {
                    return Err("callback repeated field has overlapping element storage".into());
                }
                let end = (repeated.element_count - 1)
                    .checked_mul(repeated.element_stride)
                    .and_then(|span| start.checked_add(span))
                    .and_then(|last| last.checked_add(repeated.element_size))
                    .ok_or("callback repeated field extent overflows")?;
                if end > bound {
                    return Err("callback repeated field exceeds its record".into());
                }
                output.push(OccupiedBytes::Repeated {
                    start,
                    element_size: repeated.element_size,
                    element_count: repeated.element_count,
                    stride: repeated.element_stride,
                });
            }
        } else if field.byte_size != 0 {
            let end = start
                .checked_add(field.byte_size)
                .ok_or("callback semantic field extent overflows")?;
            if end > bound {
                return Err("callback semantic field exceeds its record".into());
            }
            output.push(OccupiedBytes::Contiguous { start, end });
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::OccupiedBytes;

    #[test]
    fn repeated_storage_preserves_padding_and_half_open_edges() {
        let occupied = OccupiedBytes::Repeated {
            start: 8,
            element_size: 4,
            element_count: 3,
            stride: 16,
        };
        for (start, end, overlaps) in [
            (0, 8, false),
            (8, 12, true),
            (12, 24, false),
            (23, 25, true),
            (28, 40, false),
            (40, 44, true),
            (44, 52, false),
            (0, 52, true),
        ] {
            assert_eq!(occupied.overlaps(start, end), overlaps, "{start}..{end}");
        }
    }

    #[test]
    fn repeated_storage_queries_do_not_expand_the_element_count() {
        let count = u64::MAX / 16;
        let occupied = OccupiedBytes::Repeated {
            start: 0,
            element_size: 8,
            element_count: count,
            stride: 16,
        };
        let last = (count - 1) * 16;
        assert!(occupied.overlaps(last, last + 8));
        assert!(!occupied.overlaps(last + 8, last + 16));
        assert!(!occupied.overlaps(last + 16, u64::MAX));
    }
}
