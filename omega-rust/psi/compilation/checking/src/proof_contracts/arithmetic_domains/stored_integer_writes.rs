//! Concrete stores preserve the semantic integer while fitting its layout width.

use super::integer_ranges::known_u64_value;
use super::{Interval, ValueEnvironment};
use crate::value_custody::places;
use checked_trees::{CheckedStoredIntegerWrite, StoredIntegerWriteSite};
use diagnostics::Diagnostic;
use facts::{FactPlan, PlaceSegment};
use semantic_vocabulary::{IntegerSign, IntegerType};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod recast_history;
mod source_geometry;
pub use source_geometry::stored_integer_source_may_require_fit;

/// Discharge against the pre-store environment. The returned width belongs to
/// this exact destination/value occurrence; it is not a changed semantic type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_stored_integer_write(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    destination: ExpressionHandle,
    value: ExpressionHandle,
    origins: Option<&[crate::LocalWriteOrigin]>,
    interval: Interval,
    primitive: Option<PrimitiveType>,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CheckedStoredIntegerWrite> {
    let destination_primitive =
        places::declared_place_type_raw(program, machine, state, destination)
            .map(|reference| places::assignment_value_type(program, reference))
            .and_then(|reference| program.primitive_type_reference(reference));
    if !destination_primitive.is_some_and(|primitive| primitive.accepts_integer_literal())
        || !program.plan_laid_layouts.iter().any(|plan| {
            plan.geometries(program.typed_program())
                .any(|geometry| !geometry.integer_fields.is_empty())
        })
    {
        return Vec::new();
    }
    let Some(state) = state else {
        return Vec::new();
    };
    let roots = destination_layout_roots(program, state, destination, origins);
    let targets = roots.and_then(|roots| crate::layout_checking_targets(program, roots));
    let Ok(targets) = targets else {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt store in {owner} has no complete exact layout alternative roster"
        )));
        return Vec::new();
    };
    let mut projections = Vec::new();
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        let Ok(selected) =
            destination_projections(&alternative, machine, state, destination, origins)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "IntegerAt store in {owner} has no exact stored field geometry or reference origin"
            )));
            return Vec::new();
        };
        projections.extend(selected);
    }

    let mut writes = Vec::new();
    for (field, stored) in projections {
        let fits =
            stored_integer_value_fits(program, value, interval, primitive, environment, stored);
        if !fits {
            diagnostics.push(Diagnostic::error(format!(
                "IntegerAt store in {owner} requires a value proven to fit {}-bit {} storage",
                stored.bits(),
                match stored.sign() {
                    IntegerSign::Signed => "signed",
                    IntegerSign::Unsigned => "unsigned",
                },
            )));
            return Vec::new();
        }
        let write = CheckedStoredIntegerWrite {
            machine: machine.symbol,
            state: state.symbol,
            site: StoredIntegerWriteSite::Assignment { destination },
            value,
            field,
            stored_type: stored,
        };
        if !writes.contains(&write) {
            writes.push(write);
        }
    }
    writes
}

fn destination_layout_roots(
    program: &dyn crate::CheckingInput,
    state: &State,
    destination: ExpressionHandle,
    origins: Option<&[crate::LocalWriteOrigin]>,
) -> Result<Vec<TypeReferenceHandle>, ()> {
    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(program, destination);
    let place = facts.places.get(place);
    let root = place.root.as_symbol().ok_or(())?;
    let segments = facts.place_segments.span_or_empty(place.segments);
    let mut roots = vec![root_type(program, state, root).ok_or(())?];
    let candidates: Vec<_> = origins
        .unwrap_or(&[])
        .iter()
        .filter(|origin| {
            origin.local_symbol == root && segments.starts_with(&origin.local_segments)
        })
        .collect();
    if let Some(longest) = candidates
        .iter()
        .map(|origin| origin.local_segments.len())
        .max()
    {
        for origin in candidates
            .into_iter()
            .filter(|origin| origin.local_segments.len() == longest)
        {
            if let Some(source) = &origin.source {
                roots.extend(source_layout_roots(program, state, source)?);
            }
        }
    }
    Ok(roots)
}

pub(super) fn source_layout_roots(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<Vec<TypeReferenceHandle>, ()> {
    let mut roots = vec![source_coordinates(program, state, source)?.0];
    for recast in &source.recasts {
        roots.push(recast.target_type);
        roots.push(recast_history::predecessor_coordinates(program, state, recast)?.0);
    }
    Ok(roots)
}

/// Every physical-width demand across exact target guards and retained origins.
/// No decision is published when even one alternative cannot be reconstructed.
pub fn stored_integer_source_projections(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
    suffix: &[PlaceSegment],
) -> Result<Vec<(SymbolHandle, IntegerType)>, ()> {
    let targets =
        crate::layout_checking_targets(program, source_layout_roots(program, state, source)?)?;
    let mut projections = Vec::new();
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        if let Some(projection) =
            stored_integer_source_projection(&alternative, state, source, suffix)?
            && !projections.contains(&projection)
        {
            projections.push(projection);
        }
    }
    Ok(projections)
}

/// The alias inventory belongs to the existing statement-prefix reference
/// traversal. This operation only substitutes its exact coordinates; it does
/// not rediscover initializers, pick one conditional origin, or use path text.
fn destination_projections(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    destination: ExpressionHandle,
    origins: Option<&[crate::LocalWriteOrigin]>,
) -> Result<Vec<(SymbolHandle, IntegerType)>, ()> {
    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(program, destination);
    let place = facts.places.get(place);
    let Some(root) = place.root.as_symbol() else {
        return Err(());
    };
    let segments = facts.place_segments.span_or_empty(place.segments);
    if !crate::place_has_builtin_coordinates(program, machine, Some(state), destination) {
        return Err(());
    }
    let direct_type = root_type(program, state, root).ok_or(())?;
    // An aggregate's own scalar field has the representation selected by its
    // exact nominal declaration. A whole-record loan does not turn Room.value
    // into some other record's IntegerAt merely because its backing root is
    // unresolved (for example after a cyclic reference-returning helper).
    // This establishes encoding only: ordinary write permission and lifetime
    // checking still own whether the loan may be used.
    //
    // A reference-valued field instead writes its referent, whose encoding is
    // not the reference slot's declaration. Likewise typed_projection refuses
    // crossings of stored references. Both continue through prefix origins.
    if matches!(segments.last(), Some(PlaceSegment::Field { .. }))
        && places::declared_place_type_raw(program, machine, Some(state), destination).is_some_and(
            |reference| {
                crate::reference_shell(program, reference).is_none()
                    && program.primitive_type_reference(reference).is_some()
            },
        )
        && let Ok(projection) = typed_projection(program, direct_type, segments)
    {
        return Ok(projection.into_iter().collect());
    }
    let origins = origins.unwrap_or(&[]);
    let candidates = origins
        .iter()
        .filter(|origin| {
            origin.local_symbol == root && segments.starts_with(&origin.local_segments)
        })
        .collect::<Vec<_>>();
    let longest = candidates
        .iter()
        .map(|origin| origin.local_segments.len())
        .max();
    if let Some(longest) = longest {
        let mut projections = Vec::new();
        for origin in candidates
            .into_iter()
            .filter(|origin| origin.local_segments.len() == longest)
        {
            let source = origin.source.as_ref().ok_or(())?;
            if let Some(projection) =
                stored_integer_source_projection(program, state, source, &segments[longest..])?
            {
                if !projections.contains(&projection) {
                    projections.push(projection);
                }
            }
        }
        return Ok(projections);
    }
    // A reference local must have a retained origin. A formal's encoding is
    // an invocation obligation; the separate call consumer substitutes it.
    if program.state_local_declaration(state, root).is_some()
        && crate::reference_shell(program, direct_type).is_some()
    {
        return Err(());
    }
    Ok(typed_projection(program, direct_type, segments)?
        .into_iter()
        .collect())
}

/// Resolve the physical encoding reached through an authenticated storage
/// origin. Assignment and invocation checking share declaration traversal and
/// recast coordinates; neither may infer geometry from the rendered footprint.
/// `Ok(None)` is an authenticated destination with ordinary storage. `Err(())`
/// withholds that conclusion when a root, stored-reference crossing or selector
/// cannot be established. The source may name any alternative retained by the
/// prefix owner; consumers must check every alternative.
pub fn stored_integer_source_projection(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
    suffix: &[PlaceSegment],
) -> Result<Option<(SymbolHandle, IntegerType)>, ()> {
    if source.collection_coarse && !suffix.is_empty() {
        return Err(());
    }
    recast_history::projection(program, state, source, suffix)
}

fn source_coordinates(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<(TypeReferenceHandle, Vec<PlaceSegment>), ()> {
    if !source.root.is_valid() {
        return Err(());
    }
    let declared = root_type(program, state, source.root).ok_or(())?;
    let (carrier, path) = match &source.representation {
        Some((carrier, path)) => (*carrier, path.clone()),
        None => (declared, source.segments.clone()),
    };
    program
        .type_reference_table
        .contains_type_reference(carrier)
        .then_some((carrier, path))
        .ok_or(())
}

fn root_type(
    program: &dyn crate::CheckingInput,
    state: &State,
    root: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameter_by_symbol(state, root)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, root)
                .map(|(_, local)| local.type_reference)
        })
}

/// Walk declaration identities once to the leaf's immediate owner. Geometry
/// normalization then visits only that owner, so an unrelated opaque sibling
/// in an outer record cannot hide a selected IntegerAt field.
fn typed_projection(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Result<Option<(SymbolHandle, IntegerType)>, ()> {
    Ok(typed_storage_projection(program, carrier, path)?.stored)
}

struct StorageProjection {
    reference: TypeReferenceHandle,
    stored: Option<(SymbolHandle, IntegerType)>,
}

fn typed_storage_projection(
    program: &dyn crate::CheckingInput,
    mut carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Result<StorageProjection, ()> {
    let mut selected_case = None;
    for (position, segment) in path.iter().enumerate() {
        if position != 0 && crate::reference_shell(program, carrier).is_some() {
            // Selecting a stored reference's contents requires its own prefix
            // origin row; its declared referee cannot erase an encoded source.
            return Err(());
        }
        carrier = places::unwrapped_type_reference(program, carrier).ok_or(())?;
        match segment {
            PlaceSegment::Field { symbol } => {
                let data = places::data_definition_for_type(program, carrier).ok_or(())?;
                let field = if let Some(case) = selected_case.take() {
                    let variant = program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Variant(variant)
                                if variant.symbol == case =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                        .ok_or(())?;
                    program.data_payload_field_by_symbol(variant, *symbol)
                } else {
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })
                }
                .ok_or(())?;
                if position + 1 == path.len() {
                    return Ok(StorageProjection {
                        reference: field.type_reference,
                        stored: stored_integer_field_type(program, carrier, field.symbol)?
                            .map(|stored| (field.symbol, stored)),
                    });
                }
                carrier = field.type_reference;
            }
            PlaceSegment::FixedIndex { index } => {
                // A literal selector on a slice has the same declared element
                // encoding as a dynamic selector. Slice bounds remain a separate
                // source obligation; this walk establishes only storage geometry.
                carrier = match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    } if index < length => *element_type,
                    TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => return Err(()),
                };
            }
            PlaceSegment::Index { .. } => {
                // The source-origin producer authenticates builtin coordinates.
                // Runtime indexing changes the element, never its declaration's encoding.
                match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => carrier = *element_type,
                    _ => return Err(()),
                }
            }
            PlaceSegment::Case { variant } => {
                if selected_case.replace(*variant).is_some() {
                    return Err(());
                }
            }
            PlaceSegment::OpaqueRange => match program.type_reference_table.type_reference(carrier)
            {
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {}
                _ => return Err(()),
            },
            PlaceSegment::FixedRange { start, end } => {
                if start > end {
                    return Err(());
                }
                match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::FixedArray {
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                        ..
                    } if end <= length => {}
                    TypeReferenceNode::Slice { .. } => {}
                    _ => return Err(()),
                }
            }
        }
    }
    if selected_case.is_some() {
        return Err(());
    }
    Ok(StorageProjection {
        reference: carrier,
        stored: None,
    })
}

/// The same exact declared-field geometry is shared by assignment and value
/// construction. No source expression or scalar alias spelling selects it.
pub(crate) fn stored_integer_field_type(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    field: SymbolHandle,
) -> Result<Option<IntegerType>, ()> {
    let carrier = places::unwrapped_type_reference(program, carrier).ok_or(())?;
    let data = places::data_definition_for_type(program, carrier).ok_or(())?;
    let Some(plan) = program.plan_laid_layout(data.symbol) else {
        return Ok(None);
    };
    let index = plan
        .field_symbols
        .iter()
        .position(|candidate| *candidate == field)
        .ok_or(())?;
    let geometry = plan
        .geometry_for_target(program.typed_program(), program.layout_target())
        .ok_or(())?;
    if !geometry
        .integer_fields
        .iter()
        .any(|integer| integer.field_index == index)
    {
        return Ok(None);
    }
    let projection = crate::recast_scalar_projection(program, carrier, &[], field).ok_or(())?;
    projection.stored_integer.map(Some).ok_or(())
}

pub(crate) fn stored_integer_value_fits(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
    interval: Interval,
    primitive: Option<PrimitiveType>,
    environment: &ValueEnvironment,
    stored: IntegerType,
) -> bool {
    if let Some(value) = known_u64_value(program, environment, value) {
        fits_bounds(stored, i128::from(value), i128::from(value))
    } else {
        interval_fits(stored, interval, primitive)
    }
}

fn interval_fits(
    stored: IntegerType,
    interval: Interval,
    primitive: Option<PrimitiveType>,
) -> bool {
    // A typed value stays within its semantic carrier even when the interval
    // abstraction has an unbounded endpoint (notably the upper half of u64).
    // Physical storage never supplies this missing semantic bound.
    let carrier = primitive
        .filter(|primitive| primitive.accepts_integer_literal())
        .and_then(|primitive| {
            let bits = primitive.scalar_byte_size()?.checked_mul(8)?;
            Some(if primitive.is_signed_integer() {
                (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
            } else {
                (0, (1_i128 << bits) - 1)
            })
        });
    let low = match (interval.low().map(i128::from), carrier) {
        (Some(low), Some((minimum, _))) => Some(low.max(minimum)),
        (low, None) => low,
        (None, Some((minimum, _))) => Some(minimum),
    };
    let high = match (interval.high().map(i128::from), carrier) {
        (Some(high), Some((_, maximum))) => Some(high.min(maximum)),
        (high, None) => high,
        (None, Some((_, maximum))) => Some(maximum),
    };
    low.zip(high)
        .is_some_and(|(low, high)| low <= high && fits_bounds(stored, low, high))
}

fn fits_bounds(stored: IntegerType, low: i128, high: i128) -> bool {
    let bits = stored.bits();
    // IntegerAt currently admits whole-byte widths through 64 bits. Reject an
    // unsupported geometry instead of overflowing the bound calculation.
    if bits == 0 || bits > 64 || bits % 8 != 0 {
        return false;
    }
    let (minimum, maximum) = match stored.sign() {
        IntegerSign::Signed => (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1),
        IntegerSign::Unsigned => (0, (1_i128 << bits) - 1),
    };
    low >= minimum && high <= maximum
}
