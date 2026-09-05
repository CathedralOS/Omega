//! A callee aggregate footprint can reach several independent caller referents.
//! Walk declared value structure until a reference boundary; owned by-value
//! storage is private, while coarse collection demand visits every element.

use super::{FramePathPrecision, FramePlaceOrigin, append_place_suffix, split_place_root};
use crate::calls::write_frames::isolation::struct_literal_matches_expected_type;
use crate::calls::write_frames::reference_origins::{
    exclusive_reference_origin, referent_has_only_owned_storage,
};
use crate::symbols::TopLevelSymbols;
use psi_facts::PlaceSegment;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn written_paths(
    program: &TypedTrees,
    caller_machine: &Machine,
    argument: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    suffix: &str,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    let leaves = reference_leaves(
        program,
        caller_machine,
        argument,
        expected_type,
        suffix,
        symbols,
        active_states,
    )?;
    let mut written = Vec::new();
    for leaf in leaves.references {
        if !written.contains(&leaf.origin.path) {
            written.push(leaf.origin.path);
        }
    }
    Some(written)
}

/// Temporary structural evidence shared by immediate arguments and stored
/// aggregate declarations. Only the string view coarsens array selectors.
pub(in crate::calls::write_frames) struct ReferenceLeaf {
    pub local_suffix: String,
    pub local_segments: Vec<PlaceSegment>,
    pub origin: FramePlaceOrigin,
}

#[derive(Default)]
pub(in crate::calls::write_frames) struct AggregateOrigins {
    pub references: Vec<ReferenceLeaf>,
    /// Each path ends in the selected Case, including empty payload cases.
    pub cases: Vec<Vec<PlaceSegment>>,
}

pub(in crate::calls::write_frames) fn reference_leaves(
    program: &TypedTrees,
    caller_machine: &Machine,
    argument: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    suffix: &str,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<AggregateOrigins> {
    reference_leaves_with_stored_origins(
        program,
        caller_machine,
        argument,
        expected_type,
        suffix,
        symbols,
        active_states,
        &|_, _| None,
    )
}

/// State transfer supplies prior value evidence; the shared walker only
/// attaches those leaves beneath the current literal's structural prefix.
pub(in crate::calls::write_frames) fn reference_leaves_with_stored_origins(
    program: &TypedTrees,
    caller_machine: &Machine,
    argument: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    suffix: &str,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    stored_origins: &impl Fn(ExpressionHandle, TypeReferenceHandle) -> Option<AggregateOrigins>,
) -> Option<AggregateOrigins> {
    let mut pending = vec![(
        argument,
        expected_type,
        suffix,
        String::new(),
        Vec::new(),
        false,
    )];
    let mut leaves = AggregateOrigins::default();
    while let Some((expression, reference, suffix, local_suffix, local_segments, local_coarse)) =
        pending.pop()
    {
        if !reference.is_valid() {
            return None;
        }
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
        ) && program.primitive_type_reference(reference).is_none()
            && matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::FixedArray { .. }
            )
        {
            // Public argument queries cannot replay a caller prefix inside raw
            // frame inference. Their default resolver refuses this boundary;
            // declaration transfer supplies the already-established evidence.
            if !suffix.is_empty() {
                return None;
            }
            let moved = stored_origins(expression, reference)?;
            for case in moved.cases {
                let mut selection = local_segments.clone();
                selection.extend(case);
                leaves.cases.push(selection);
            }
            for mut leaf in moved.references {
                let mut segments = local_segments.clone();
                segments.extend(leaf.local_segments);
                leaf.local_segments = segments;
                leaf.local_suffix = if local_coarse {
                    local_suffix.clone()
                } else {
                    append_place_suffix(&local_suffix, &leaf.local_suffix)
                };
                leaves.references.push(leaf);
            }
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                pending.push((
                    expression,
                    *base_type,
                    suffix,
                    local_suffix,
                    local_segments,
                    local_coarse,
                ));
            }
            TypeReferenceNode::Reference {
                access, referee, ..
            } => {
                if !access.is_exclusive() {
                    continue;
                }
                if !referent_has_only_owned_storage(program, *referee) {
                    return None;
                }
                let origin = exclusive_reference_origin(
                    program,
                    caller_machine,
                    expression,
                    symbols,
                    active_states,
                )?;
                let path = match origin.precision {
                    FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
                    FramePathPrecision::CollectionCoarse => origin.path,
                };
                leaves.references.push(ReferenceLeaf {
                    local_suffix,
                    local_segments,
                    origin: FramePlaceOrigin {
                        path,
                        precision: origin.precision,
                    },
                });
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let ExpressionNode::ArrayLiteral(elements) =
                    program.expression_table.expression(expression)
                else {
                    return None;
                };
                let elements = program.expression_table.expression_handles(*elements);
                // Indexed frame paths deliberately stop at the collection.
                if !suffix.is_empty() || elements.len() != *length {
                    return None;
                }
                for (index, element) in elements.iter().enumerate().rev() {
                    let mut segments = local_segments.clone();
                    segments.push(PlaceSegment::FixedIndex { index });
                    pending.push((
                        *element,
                        *element_type,
                        "",
                        local_suffix.clone(),
                        segments,
                        true,
                    ));
                }
            }
            TypeReferenceNode::Named { symbol, .. }
                if program.primitive_type_reference(reference).is_none() =>
            {
                let ExpressionNode::StructLiteral(literal) =
                    program.expression_table.expression(expression)
                else {
                    return None;
                };
                if !struct_literal_matches_expected_type(program, literal, reference)
                    || !symbol.is_valid()
                    || (literal.type_symbol.is_valid() && literal.type_symbol != *symbol)
                {
                    return None;
                }
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == *symbol)?;
                let mut fields = Vec::new();
                let mut local_segments = local_segments;
                if let Some(case) = literal.case_name.as_ref() {
                    let mut variants =
                        program
                            .data_members(definition)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Variant(variant) if variant.name == *case => {
                                    Some(variant)
                                }
                                _ => None,
                            });
                    let variant = variants.next()?;
                    if !variant.symbol.is_valid()
                        || variants.next().is_some()
                        || literal
                            .case_symbol
                            .is_some_and(|symbol| symbol.is_valid() && symbol != variant.symbol)
                    {
                        return None;
                    }
                    local_segments.push(PlaceSegment::Case {
                        variant: variant.symbol,
                    });
                    leaves.cases.push(local_segments.clone());
                    for field in program.data_payload_fields(variant) {
                        fields.push((field.name.as_str(), field.symbol, field.type_reference));
                    }
                } else {
                    for member in program.data_members(definition) {
                        let DataMember::Field(field) = member else {
                            return None;
                        };
                        fields.push((field.name.as_str(), field.symbol, field.type_reference));
                    }
                }
                let actuals = program.expression_table.struct_fields(literal.fields);
                for (index, field) in actuals.iter().enumerate() {
                    if actuals[..index]
                        .iter()
                        .any(|previous| previous.name == field.name)
                        || !fields.iter().any(|(name, symbol, _)| {
                            *name == field.name.as_str()
                                && (!field.field_symbol.is_valid() || field.field_symbol == *symbol)
                        })
                    {
                        return None;
                    }
                }
                let selected = if suffix.is_empty() {
                    None
                } else {
                    let (field, rest) = split_place_root(suffix.strip_prefix('.')?);
                    Some((field, rest))
                };
                if selected.is_some_and(|(selected, _)| {
                    !fields.iter().any(|(name, _, _)| *name == selected)
                }) {
                    return None;
                }
                for (name, symbol, field_type) in fields {
                    if !symbol.is_valid() {
                        return None;
                    }
                    if selected.is_some_and(|(selected, _)| selected != name) {
                        continue;
                    }
                    let value = actuals
                        .iter()
                        .find(|field| field.name.as_str() == name)
                        .map(|field| field.value)
                        .unwrap_or_default();
                    // Missing reference initializers cannot create a complete
                    // empty footprint; omitted owned scalar defaults can.
                    let mut segments = local_segments.clone();
                    segments.push(PlaceSegment::Field { symbol });
                    let field_suffix = if local_coarse {
                        local_suffix.clone()
                    } else {
                        format!("{local_suffix}.{name}")
                    };
                    pending.push((
                        value,
                        field_type,
                        selected.map_or("", |(_, rest)| rest),
                        field_suffix,
                        segments,
                        local_coarse,
                    ));
                }
            }
            _ if program.primitive_type_reference(reference).is_some()
                || matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Unit
                ) =>
            {
                if !suffix.is_empty() {
                    return None;
                }
                // This value lives in the callee's by-value aggregate copy.
            }
            _ => return None,
        }
    }
    Some(leaves)
}
