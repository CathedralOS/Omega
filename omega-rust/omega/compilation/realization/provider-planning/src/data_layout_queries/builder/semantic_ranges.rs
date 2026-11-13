// We derive bit-level semantic field ranges (`plan_laid_semantic_ranges`) for
// plan-laid records so private callback closure can verify that callback slots
// never overlap semantic storage. Repeated fixed-array placements claim each
// element's exact extent rather than the whole stride, leaving validated
// inter-element padding available to private demands.

use crate::data_layout_queries::FieldLayout;
use diagnostics::Diagnostic;

pub(crate) fn plan_laid_semantic_ranges(
    plan: &typed_trees::typed_trees::PlanLaidLayout,
    geometry: &typed_trees::PlanLaidGeometry,
    fields: &[FieldLayout],
) -> Result<Vec<(u64, u64)>, Diagnostic> {
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
                ranges.validate_extent(
                    field.name.as_str(),
                    fragment.container_byte_offset,
                    byte_size,
                )?;
                let start = (fragment.container_byte_offset as u64)
                    .checked_mul(8)
                    .and_then(|base| base.checked_add(u64::from(fragment.destination_lsb)))
                    .ok_or_else(|| {
                        Diagnostic::error("callback semantic fragment offset overflows")
                    })?;
                let end = start
                    .checked_add(u64::from(fragment.width))
                    .ok_or_else(|| {
                        Diagnostic::error("callback semantic fragment extent overflows")
                    })?;
                ranges.ranges.push((start, end));
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

struct SemanticRanges<'a> {
    plan: &'a typed_trees::typed_trees::PlanLaidLayout,
    geometry: &'a typed_trees::PlanLaidGeometry,
    ranges: Vec<(u64, u64)>,
}

impl SemanticRanges<'_> {
    fn push(&mut self, field: &str, start: usize, byte_size: usize) -> Result<(), Diagnostic> {
        if byte_size == 0 {
            return Ok(());
        }
        let end = self.validate_extent(field, start, byte_size)?;
        self.ranges.push((
            (start as u64)
                .checked_mul(8)
                .ok_or_else(|| Diagnostic::error("semantic bit offset overflows"))?,
            (end as u64)
                .checked_mul(8)
                .ok_or_else(|| Diagnostic::error("semantic bit extent overflows"))?,
        ));
        Ok(())
    }

    fn validate_extent(
        &self,
        field: &str,
        start: usize,
        byte_size: usize,
    ) -> Result<usize, Diagnostic> {
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
        Ok(end)
    }
}

pub(crate) fn ranges_overlap(
    left_start: u64,
    left_end: u64,
    right_start: u64,
    right_end: u64,
) -> bool {
    left_start < right_end && right_start < left_end
}
