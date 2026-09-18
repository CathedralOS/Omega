//! Materialization plans and their application: scalar and aggregate
//! layout materialization into bytes, scalar layout decoding, and the
//! crate's diagnostic type.

pub(crate) mod field_identities;
pub(crate) mod field_values;
pub(crate) mod stored_integer_writes;

use crate::layout_reports::{
    IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};
use crate::materialization::field_identities::{
    MaterializationFieldKey, materialization_field_key, stable_identity_suffix,
    validate_materialization_field_identities,
};
use crate::materialization::field_values::{
    AggregateFieldSchema, AggregateFieldShape, AggregateFieldValue, ScalarFieldSchema,
    ScalarFieldValue,
};
use crate::materialization::stored_integer_writes::{
    apply_write, low_mask, read_container, scalar_fragment, validate_fragment, validate_write,
    validate_write_source_value,
};
use crate::placement::{ByteOrder, MaterializationAction, PlacementConstraints};
use crate::post_handoff_writer::{
    PostHandoffWriterPlan, PostHandoffWriterSource, PostHandoffWriterStep,
};
use crate::symbolic_materialization::apply_scalar_entry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMaterializationPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub actions: Vec<MaterializationAction>,
}

impl SymbolicMaterializationPlan {
    pub fn derive_post_handoff_writer(
        &self,
    ) -> Result<PostHandoffWriterPlan, MaterializationDiagnostic> {
        if self.actions.is_empty() {
            return Err(MaterializationDiagnostic(
                "post-handoff writer requires at least one fragment".into(),
            ));
        }
        let mut steps = Vec::with_capacity(self.actions.len());
        for action in &self.actions {
            let step = match action {
                MaterializationAction::ResolvedWrite {
                    write,
                    source_value,
                } => PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolved(*source_value),
                },
                MaterializationAction::RuntimeWriter(write) => PostHandoffWriterStep {
                    write: write.clone(),
                    source: PostHandoffWriterSource::Resolve(write.target),
                },
                MaterializationAction::NativePointerRelocation { .. } => {
                    return Err(MaterializationDiagnostic(
                        "loader-native relocation cannot enter a post-handoff writer program"
                            .into(),
                    ));
                }
            };
            steps.push(step);
        }
        Ok(PostHandoffWriterPlan {
            byte_len: self.byte_len,
            byte_order: self.byte_order,
            placement: self.placement,
            steps,
        })
    }

    /// Applies a fully resolved plan atomically with respect to the destination
    /// slice: unresolved actions reject before any output byte changes.
    pub fn materialize_resolved_into(
        &self,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        if destination.len() < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "materialization needs {} bytes, destination has {}",
                self.byte_len,
                destination.len()
            )));
        }
        if let Some(action) = self
            .actions
            .iter()
            .find(|action| !matches!(action, MaterializationAction::ResolvedWrite { .. }))
        {
            return Err(MaterializationDiagnostic(format!(
                "materialization still contains an unresolved action: {action:?}"
            )));
        }

        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            validate_write(self.byte_len, write)?;
            validate_write_source_value(write, *source_value, "resolved symbolic")?;
        }

        let mut staged = destination[..self.byte_len].to_vec();
        for action in &self.actions {
            let MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } = action
            else {
                unreachable!("unresolved actions were rejected above")
            };
            apply_write(&mut staged, self.byte_order, write, *source_value)?;
        }
        destination[..self.byte_len].copy_from_slice(&staged);
        Ok(())
    }
}

/// Materializes one complete ordinary scalar value through validated layout
/// entries. Output starts from zero so padding and reserved bits stay
/// deterministic. The destination is changed only after every field and
/// fragment has validated.
///
/// This is the numeric sibling of symbolic materialization. It cannot resolve
/// code/data symbols, install hardware state, or mint authority; it only turns
/// named scalar values into bytes according to an already validated plan.
pub fn materialize_scalar_layout_into(
    layout: &LayoutPlanReport,
    values: &[ScalarFieldValue],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "scalar materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    validate_materialization_field_identities(layout)?;

    let mut supplied = std::collections::BTreeMap::new();
    let mut supplied_names = std::collections::BTreeSet::new();
    for value in values {
        if !supplied_names.insert(value.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` is supplied more than once",
                value.field
            )));
        }
        let key = materialization_field_key(&value.field, value.member_identity);
        if supplied.insert(key, value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` repeats stable member identity #{}",
                value.field,
                value
                    .member_identity
                    .expect("only numbered values can collide after name validation")
            )));
        }
    }

    let planned = layout
        .entries
        .iter()
        .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(entry) = layout.entries.iter().find(|entry| {
        !supplied.contains_key(&materialization_field_key(
            &entry.field,
            entry.member_identity,
        ))
    }) {
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no supplied scalar value{suffix}",
            entry.field
        )));
    }
    if let Some(value) = supplied
        .iter()
        .find_map(|(key, value)| (!planned.contains(key)).then_some(value))
    {
        let suffix = stable_identity_suffix(value.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "supplied scalar field `{}` has no entry in the validated layout plan{suffix}",
            value.field
        )));
    }

    let mut staged = vec![0_u8; byte_len];
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        let value = supplied
            .get(&key)
            .expect("complete field set validated above");
        apply_scalar_entry(&mut staged, byte_order, entry, value)?;
    }
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

/// Materializes complete fields through their validated placements:
/// whole-extent `At` copies, compiler-sized element `At` placements at a
/// constant destination stride for one outer fixed array, and scalar
/// `IntegerAt`/`Bits` placements for fields whose encoded extent fits the
/// 64-bit materialization carrier. Plan validation already limits scalar
/// placements to scalar-typed fields, so the carrier bound is what remains
/// visible here. All validation and copying happens against a staged zeroed
/// buffer, so rejection leaves `destination` unchanged. Wider aggregate
/// fields are deliberately not interpreted as scalar fragments or stored
/// integers.
pub fn materialize_aggregate_layout_into(
    layout: &LayoutPlanReport,
    fields: &[AggregateFieldSchema],
    values: &[AggregateFieldValue],
    byte_order: ByteOrder,
    destination: &mut [u8],
) -> Result<(), MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "aggregate materialization requires a fixed-size layout plan".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if destination.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "aggregate materialization needs {byte_len} bytes, destination has {}",
            destination.len()
        )));
    }
    validate_materialization_field_identities(layout)?;

    let mut schemas = std::collections::BTreeMap::new();
    let mut schema_names = std::collections::BTreeSet::new();
    for field in fields {
        if !schema_names.insert(field.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` is declared more than once",
                field.field
            )));
        }
        let key = materialization_field_key(&field.field, field.member_identity);
        if schemas.insert(key, field).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` repeats stable member identity #{}",
                field.field,
                field
                    .member_identity
                    .expect("only numbered fields can collide after name validation")
            )));
        }
    }
    let mut supplied = std::collections::BTreeMap::new();
    for value in values {
        if supplied.insert(value.field.as_str(), value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` is supplied more than once",
                value.field
            )));
        }
    }
    let mut planned =
        std::collections::BTreeMap::<MaterializationFieldKey, Vec<&LayoutFieldEntryReport>>::new();
    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        planned.entry(key).or_default().push(entry);
    }
    if let Some(entries) = planned
        .iter()
        .find_map(|(key, entries)| (!schemas.contains_key(key)).then_some(entries))
    {
        let entry = entries
            .first()
            .expect("planned aggregate key always retains an entry");
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no aggregate schema extent{suffix}",
            entry.field
        )));
    }
    if let Some(field) = schemas
        .iter()
        .find_map(|(key, field)| (!planned.contains_key(key)).then_some(field))
    {
        let suffix = stable_identity_suffix(field.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "aggregate schema field `{}` has no entry in the validated layout plan{suffix}",
            field.field
        )));
    }
    if let Some(field) = fields
        .iter()
        .find(|field| !supplied.contains_key(field.field.as_str()))
    {
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no supplied aggregate value",
            field.field
        )));
    }
    if let Some(field) = supplied
        .keys()
        .find(|field| !schema_names.contains(**field))
    {
        return Err(MaterializationDiagnostic(format!(
            "supplied aggregate field `{field}` has no compiler-derived aggregate schema"
        )));
    }

    let mut staged = vec![0_u8; byte_len];
    let mut occupied = vec![false; byte_len];
    // `Bits` fragments read-modify-write shared containers, so their
    // destination bits are checked only after every whole-byte placement has
    // claimed its extent: `occupied` carries `At` and `IntegerAt` bytes while
    // `fragment_destinations` accumulates the exact destination bits fragments
    // already wrote inside each shared byte.
    let mut deferred_fragments = Vec::new();
    for (field_key, schema) in schemas {
        let field_name = schema.field.as_str();
        let entries = planned
            .get_mut(&field_key)
            .expect("complete aggregate plan set validated above");
        let has_scalar_placement = entries.iter().any(|entry| {
            matches!(
                entry.placement,
                LayoutPlacementReport::IntegerAt { .. } | LayoutPlacementReport::Bits { .. }
            )
        });
        if has_scalar_placement
            && entries
                .iter()
                .any(|entry| matches!(entry.placement, LayoutPlacementReport::At { .. }))
        {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{field_name}` mixes `At` with scalar placements"
            )));
        }
        if has_scalar_placement {
            if !matches!(schema.shape, AggregateFieldShape::Whole) {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` carries scalar placements but is an outer fixed array"
                )));
            }
            if schema.byte_size > 8 {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` extent is {} bytes, past the 64-bit scalar placement carrier",
                    schema.byte_size
                )));
            }
            let value = supplied
                .get(field_name)
                .expect("complete aggregate field set validated above");
            let expected_size = usize::try_from(schema.byte_size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{}` extent cannot be represented on this compiler host",
                    field_name
                ))
            })?;
            if value.bytes.len() != expected_size {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` supplies {} bytes, but its compiler-derived extent is {expected_size}",
                    field_name,
                    value.bytes.len()
                )));
            }
            let scalar = ScalarFieldValue {
                field: value.field.clone(),
                member_identity: schema.member_identity,
                width_bits: u16::try_from(expected_size * 8)
                    .expect("the eight-byte carrier bound keeps the extent inside u16"),
                value: read_container(&value.bytes, byte_order),
            };
            for entry in entries.iter() {
                match entry.placement {
                    LayoutPlacementReport::IntegerAt { .. } => {
                        let fragment = scalar_fragment(entry, scalar.width_bits)?;
                        let start =
                            usize::try_from(fragment.container_byte_offset).map_err(|_| {
                                MaterializationDiagnostic(format!(
                                    "aggregate field `{field_name}` stored-integer offset cannot be represented on this compiler host"
                                ))
                            })?;
                        let end = start
                            .checked_add(usize::from(fragment.container_width_bits / 8))
                            .ok_or_else(|| {
                                MaterializationDiagnostic(format!(
                                    "aggregate field `{field_name}` stored-integer range overflows"
                                ))
                            })?;
                        if end > byte_len {
                            return Err(MaterializationDiagnostic(format!(
                                "aggregate field `{field_name}` writes through byte {end}, past the {byte_len}-byte layout"
                            )));
                        }
                        if occupied[start..end].iter().any(|claimed| *claimed) {
                            return Err(MaterializationDiagnostic(format!(
                                "aggregate field `{field_name}` stored-integer placement overlaps an earlier placement"
                            )));
                        }
                        apply_scalar_entry(&mut staged, byte_order, entry, &scalar)?;
                        occupied[start..end].fill(true);
                    }
                    LayoutPlacementReport::Bits { .. } => {
                        deferred_fragments.push((*entry, scalar.clone()));
                    }
                    LayoutPlacementReport::At { .. } => {
                        unreachable!("mixed `At` and scalar placements rejected above")
                    }
                }
            }
            continue;
        }
        let value = supplied
            .get(field_name)
            .expect("complete aggregate field set validated above");
        let expected_size = usize::try_from(schema.byte_size).map_err(|_| {
            MaterializationDiagnostic(format!(
                "aggregate field `{}` extent cannot be represented on this compiler host",
                field_name
            ))
        })?;
        if value.bytes.len() != expected_size {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{}` supplies {} bytes, but its compiler-derived extent is {expected_size}",
                field_name,
                value.bytes.len()
            )));
        }

        entries.sort_unstable_by_key(|entry| match entry.placement {
            LayoutPlacementReport::At { offset } => offset,
            _ => unreachable!("non-At aggregate entries rejected above"),
        });
        let (source_chunk_size, required_align) = if entries.len() == 1 {
            (expected_size, None)
        } else {
            let AggregateFieldShape::Repeated {
                element_byte_size,
                element_align,
                element_count,
            } = schema.shape
            else {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` has more than one `At` placement but is not an outer fixed array"
                )));
            };
            let actual_count = u64::try_from(entries.len()).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` placement count cannot be represented as u64"
                ))
            })?;
            if actual_count != element_count {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` has {actual_count} element placements, expected {element_count}"
                )));
            }
            let element_byte_size = usize::try_from(element_byte_size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` element extent cannot be represented on this compiler host"
                ))
            })?;
            let offsets = entries
                .iter()
                .map(|entry| match entry.placement {
                    LayoutPlacementReport::At { offset } => offset,
                    _ => unreachable!("non-At aggregate entries rejected above"),
                })
                .collect::<Vec<_>>();
            let stride = offsets[1].checked_sub(offsets[0]).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element offsets are not ordered"
                ))
            })?;
            if stride < element_byte_size as u64
                || offsets.windows(2).any(|pair| pair[1] - pair[0] != stride)
            {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element placements do not have one nonoverlapping constant stride"
                )));
            }
            (element_byte_size, Some(element_align))
        };

        let chunks = value.bytes.chunks_exact(source_chunk_size);
        if !chunks.remainder().is_empty() || chunks.len() != entries.len() {
            return Err(MaterializationDiagnostic(format!(
                "aggregate field `{field_name}` bytes do not tile its compiler-derived source elements exactly"
            )));
        }
        for (entry, source) in entries.iter().zip(chunks) {
            let LayoutPlacementReport::At { offset } = entry.placement else {
                unreachable!("non-At aggregate entries rejected above")
            };
            if required_align.is_some_and(|align| !offset.is_multiple_of(align)) {
                return Err(MaterializationDiagnostic(format!(
                    "repeated aggregate field `{field_name}` element offset {offset} violates its compiler-derived alignment {}",
                    required_align.expect("checked as present")
                )));
            }
            let start = usize::try_from(offset).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` offset cannot be represented on this compiler host"
                ))
            })?;
            let end = start.checked_add(source.len()).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` destination range overflows"
                ))
            })?;
            if end > byte_len {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` writes through byte {end}, past the {byte_len}-byte layout"
                )));
            }
            if occupied[start..end].iter().any(|occupied| *occupied) {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{field_name}` overlaps an earlier aggregate placement"
                )));
            }
            staged[start..end].copy_from_slice(source);
            occupied[start..end].fill(true);
        }
    }
    let mut fragment_destinations = std::collections::BTreeMap::<usize, u8>::new();
    for (entry, scalar) in deferred_fragments {
        let fragment = scalar_fragment(entry, scalar.width_bits)?;
        for bit in fragment.destination_lsb..fragment.destination_lsb + fragment.width {
            let byte = usize::try_from(fragment.container_byte_offset + u64::from(bit) / 8)
                .map_err(|_| {
                    MaterializationDiagnostic(format!(
                        "aggregate field `{}` fragment destination cannot be represented on this compiler host",
                        entry.field
                    ))
                })?;
            let Some(whole_byte_claimed) = occupied.get(byte) else {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` fragment writes byte {byte}, past the {byte_len}-byte layout",
                    entry.field
                )));
            };
            if *whole_byte_claimed {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` fragment destination overlaps a whole-extent placement",
                    entry.field
                )));
            }
            let mask = 1_u8 << (bit % 8);
            let applied = fragment_destinations.entry(byte).or_default();
            if *applied & mask != 0 {
                return Err(MaterializationDiagnostic(format!(
                    "aggregate field `{}` fragment destination overlaps an earlier fragment",
                    entry.field
                )));
            }
            *applied |= mask;
        }
        apply_scalar_entry(&mut staged, byte_order, entry, &scalar)?;
    }
    destination[..byte_len].copy_from_slice(&staged);
    Ok(())
}

/// Decodes one complete fixed scalar layout without establishing any semantic
/// domain or authority fact. Callers receive ordinary named values; a separate
/// validator decides whether those values establish an imported-table claim.
pub fn decode_scalar_layout(
    layout: &LayoutPlanReport,
    fields: &[ScalarFieldSchema],
    byte_order: ByteOrder,
    source: &[u8],
) -> Result<Vec<ScalarFieldValue>, MaterializationDiagnostic> {
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic("scalar decoding requires a fixed-size layout plan".into())
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "fixed layout size {size} cannot be represented on this compiler host"
                ))
            })
        })?;
    if source.len() < byte_len {
        return Err(MaterializationDiagnostic(format!(
            "scalar decoding needs {byte_len} bytes, source has {}",
            source.len()
        )));
    }
    validate_materialization_field_identities(layout)?;

    let mut decoded = std::collections::BTreeMap::new();
    let mut schema_names = std::collections::BTreeSet::new();
    for field in fields {
        if !schema_names.insert(field.field.as_str()) {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` is declared more than once",
                field.field
            )));
        }
        let key = materialization_field_key(&field.field, field.member_identity);
        if decoded.insert(key, (field, 0_u64, 0_u64)).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` repeats stable member identity #{}",
                field.field,
                field
                    .member_identity
                    .expect("only numbered schemas can collide after name validation")
            )));
        }
    }
    let planned = layout
        .entries
        .iter()
        .map(|entry| materialization_field_key(&entry.field, entry.member_identity))
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(entry) = layout.entries.iter().find(|entry| {
        !decoded.contains_key(&materialization_field_key(
            &entry.field,
            entry.member_identity,
        ))
    }) {
        let suffix = stable_identity_suffix(entry.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "layout field `{}` has no scalar decode schema{suffix}",
            entry.field
        )));
    }
    if let Some(field) = decoded
        .iter()
        .find_map(|(key, (field, _, _))| (!planned.contains(key)).then_some(field))
    {
        let suffix = stable_identity_suffix(field.member_identity);
        return Err(MaterializationDiagnostic(format!(
            "scalar decode field `{}` has no entry in the validated layout plan{suffix}",
            field.field
        )));
    }

    for entry in &layout.entries {
        let key = materialization_field_key(&entry.field, entry.member_identity);
        let (field, value, covered) = decoded
            .get_mut(&key)
            .expect("complete field set validated above");
        let width_bits = field.width_bits;
        let fragment = scalar_fragment(entry, width_bits)?;
        validate_fragment(
            byte_len,
            &entry.field,
            fragment.container_byte_offset,
            fragment.container_width_bits,
            fragment.destination_lsb,
            fragment.source_lsb,
            fragment.width,
        )?;
        if let LayoutPlacementReport::IntegerAt {
            stored_width,
            interpretation,
            ..
        } = entry.placement
        {
            let stored_width = u16::try_from(stored_width).map_err(|_| {
                MaterializationDiagnostic(format!(
                    "scalar field `{}` has an invalid stored-integer width",
                    entry.field
                ))
            })?;
            if stored_width == 0
                || stored_width > 64
                || !stored_width.is_multiple_of(8)
                || width_bits < stored_width
            {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` cannot decode {stored_width}-bit stored-integer storage into its {}-bit semantic carrier",
                    entry.field, width_bits
                )));
            }
            if *covered != 0 {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` has more than one stored-integer decode entry",
                    entry.field
                )));
            }
            let container_bytes = usize::from(fragment.container_width_bits / 8);
            let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
                MaterializationDiagnostic(
                    "stored-integer offset cannot be represented on this host".into(),
                )
            })?;
            let end = start.checked_add(container_bytes).ok_or_else(|| {
                MaterializationDiagnostic("stored-integer byte range overflows".into())
            })?;
            let stored = read_container(&source[start..end], byte_order) & low_mask(stored_width);
            *value = match interpretation {
                IntegerInterpretation::Unsigned => stored,
                IntegerInterpretation::Signed if stored & (1_u64 << (stored_width - 1)) != 0 => {
                    stored | (low_mask(width_bits) & !low_mask(stored_width))
                }
                IntegerInterpretation::Signed => stored,
            };
            *covered = low_mask(width_bits);
            continue;
        }
        let source_mask = low_mask(fragment.width) << fragment.source_lsb;
        if *covered & source_mask != 0 {
            return Err(MaterializationDiagnostic(format!(
                "scalar field `{}` decode fragments overlap in the logical source",
                entry.field
            )));
        }
        let container_bytes = usize::from(fragment.container_width_bits / 8);
        let start = usize::try_from(fragment.container_byte_offset).map_err(|_| {
            MaterializationDiagnostic("container offset cannot be represented on this host".into())
        })?;
        let end = start
            .checked_add(container_bytes)
            .ok_or_else(|| MaterializationDiagnostic("container byte range overflows".into()))?;
        let container = read_container(&source[start..end], byte_order);
        let value_fragment = (container >> fragment.destination_lsb) & low_mask(fragment.width);
        *value |= value_fragment << fragment.source_lsb;
        *covered |= source_mask;
    }

    decoded
        .into_iter()
        .map(|(_, (field, value, covered))| {
            let width_bits = field.width_bits;
            if covered != low_mask(width_bits) {
                return Err(MaterializationDiagnostic(format!(
                    "scalar field `{}` decode fragments do not tile its complete {width_bits}-bit source",
                    field.field
                )));
            }
            Ok(ScalarFieldValue {
                field: field.field.clone(),
                member_identity: field.member_identity,
                width_bits,
                value,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationDiagnostic(pub String);

impl std::fmt::Display for MaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MaterializationDiagnostic {}
