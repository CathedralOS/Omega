//! Plan-laid semantic ranges and the canonical plan-laid data identity.

use crate::FieldLayout;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;

pub(crate) fn canonical_plan_laid_data_identity(
    program: &TypedTrees,
    plan: &typed_trees::typed_trees::PlanLaidLayout,
    definition: &DataDefinition,
    canonical_layout_subject: &str,
) -> Option<String> {
    program
        .normalized_hermetic_symbol_identity(definition.symbol)
        .ok()
        .or_else(|| {
            definition.generic_instance.map(|origin| {
                format!(
                    "closed-data-instance::{}",
                    program.package_qualified_type_identity_text(origin)
                )
            })
        })
        .or_else(|| {
            program
                .normalized_hermetic_symbol_identity(plan.schema_symbol)
                .ok()
                .map(|schema_identity| {
                    format!("closed-plan-laid-data::{canonical_layout_subject}::{schema_identity}")
                })
        })
}

pub(crate) fn plan_laid_semantic_ranges(
    plan: &typed_trees::typed_trees::PlanLaidLayout,
    geometry: &typed_trees::PlanLaidGeometry,
    fields: &[FieldLayout],
) -> Result<Vec<(usize, usize)>, Diagnostic> {
    let mut ranges = SemanticRanges {
        plan,
        geometry,
        ranges: Vec::new(),
    };
    for (index, field) in fields.iter().enumerate() {
        if let Some(bit_field) = geometry
            .bit_fields
            .iter()
            .find(|candidate| candidate.field_index == index)
        {
            for fragment in &bit_field.fragments {
                let byte_size = usize::from(fragment.container_width_bits).div_ceil(8);
                ranges.push(
                    field.name.as_str(),
                    fragment.container_byte_offset,
                    byte_size,
                )?;
            }
            continue;
        }
        if let Some(integer_field) = geometry
            .integer_fields
            .iter()
            .find(|candidate| candidate.field_index == index)
        {
            let byte_size = usize::from(integer_field.stored_width_bits).div_ceil(8);
            ranges.push(field.name.as_str(), field.offset, byte_size)?;
            continue;
        }
        if let Some(repeated_field) = geometry
            .repeated_fields
            .iter()
            .find(|candidate| candidate.field_index == index)
        {
            let Some((_, length)) = field.type_descriptor.fixed_array() else {
                return Err(Diagnostic::error(format!(
                    "plan-laid data `{}` repeated field `{}` lost its fixed-array shape during private callback closure",
                    plan.data_name, field.name
                )));
            };
            if length == 0 || !field.layout.size.is_multiple_of(length) {
                return Err(Diagnostic::error(format!(
                    "plan-laid data `{}` repeated field `{}` has invalid target element geometry",
                    plan.data_name, field.name
                )));
            }
            let element_size = field.layout.size / length;
            // Repeated placement owns each compiler-derived element extent,
            // not the whole stride. The already-validated inter-element
            // padding is intentionally available to a private demand; an
            // ordinary one-entry aggregate `At` row instead reaches the
            // whole-field arm below and occupies its complete layout size.
            for element in 0..length {
                let start = field
                    .offset
                    .checked_add(
                        element
                            .checked_mul(repeated_field.element_stride)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "repeated field callback-closure offset overflows",
                                )
                            })?,
                    )
                    .ok_or_else(|| {
                        Diagnostic::error("repeated field callback-closure offset overflows")
                    })?;
                ranges.push(field.name.as_str(), start, element_size)?;
            }
            continue;
        }
        ranges.push(field.name.as_str(), field.offset, field.layout.size)?;
    }
    Ok(ranges.ranges)
}

/// The semantic-field byte extents of one plan-laid value, appended in field
/// order with the range checks every extent must pass before a private callback
/// slot may claim adjacent storage.
struct SemanticRanges<'a> {
    plan: &'a typed_trees::typed_trees::PlanLaidLayout,
    geometry: &'a typed_trees::PlanLaidGeometry,
    ranges: Vec<(usize, usize)>,
}

impl SemanticRanges<'_> {
    fn push(&mut self, field: &str, start: usize, byte_size: usize) -> Result<(), Diagnostic> {
        if byte_size == 0 {
            return Ok(());
        }
        let end = start.checked_add(byte_size).ok_or_else(|| {
            Diagnostic::error(format!(
                "plan-laid data `{}` semantic field `{field}` extent overflows during private callback closure",
                self.plan.data_name
            ))
        })?;
        if end > self.geometry.size {
            return Err(Diagnostic::error(format!(
                "plan-laid data `{}` semantic field `{field}` range {start}..{end} lies outside its {}-byte layout during private callback closure",
                self.plan.data_name, self.geometry.size
            )));
        }
        self.ranges.push((start, end));
        Ok(())
    }
}

pub(crate) fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < right_end && right_start < left_end
}
