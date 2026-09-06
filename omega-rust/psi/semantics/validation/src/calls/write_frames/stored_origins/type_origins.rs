//! Type-derived symbolic origins for owned aggregate values. Cases are possible,
//! not selected by a constructor; arrays retain one unknown-element selector.

use super::super::isolation::concrete_nominal_type;
use super::{FramePathPrecision, FramePlaceOrigin, StoredLocalOrigins, StoredWriteOrigin};
use facts::PlaceSegment;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(in crate::calls::write_frames) fn declared_origins(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
) -> Option<StoredLocalOrigins> {
    declared_origins_for_query(program, symbol, name, reference, false)
}

pub(in crate::calls::write_frames) fn declared_origins_for_query(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
    include_shared: bool,
) -> Option<StoredLocalOrigins> {
    let mut origins = StoredLocalOrigins {
        local_symbol: symbol,
        references: Vec::new(),
        cases: Vec::new(),
        moves: Vec::new(),
    };
    let mut pending = vec![(reference, Vec::new(), Vec::new())];
    while let Some((reference, segments, mut visiting)) = pending.pop() {
        if !reference.is_valid() || visiting.contains(&reference) {
            return None;
        }
        visiting.push(reference);
        if program.primitive_type_reference(reference).is_some() {
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => {}
            TypeReferenceNode::Constrained { base_type, .. } => {
                pending.push((*base_type, segments, visiting));
            }
            TypeReferenceNode::Reference {
                access, referee, ..
            } => {
                if !access.is_exclusive() && !include_shared {
                    continue;
                }
                if !super::super::reference_origins::referent_has_only_owned_storage(
                    program, *referee,
                ) {
                    return None;
                }
                let mut path = name.to_owned();
                let mut precision = FramePathPrecision::Exact;
                for segment in &segments {
                    match segment {
                        PlaceSegment::Field { symbol } => {
                            path.push('.');
                            path.push_str(program.symbols.name(*symbol));
                        }
                        PlaceSegment::Case { .. } => {}
                        _ => {
                            precision = FramePathPrecision::CollectionCoarse;
                            break;
                        }
                    }
                }
                origins.references.push(StoredWriteOrigin {
                    local_symbol: symbol,
                    local_path: path.clone(),
                    local_segments: segments.clone(),
                    origin: FramePlaceOrigin {
                        path,
                        precision,
                        source: super::super::FrameSourcePlace {
                            root: symbol,
                            segments,
                            builtin_coordinates: symbol.is_valid(),
                        },
                    },
                });
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                if *length != 0 {
                    let mut segments = segments;
                    // This is only a may-write selector, never an actual index
                    // expression or evidence authorizing an element access.
                    segments.push(PlaceSegment::Index {
                        expression: Default::default(),
                    });
                    pending.push((*element_type, segments, visiting));
                }
            }
            node if concrete_nominal_type(node).is_some() => {
                let (symbol, _) = concrete_nominal_type(node)?;
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == symbol)?;
                if program.symbols.get(symbol).kind != SymbolKind::Data
                    || !definition.type_parameters.is_empty()
                {
                    return None;
                }
                for member in program.data_members(definition) {
                    match member {
                        DataMember::Field(field) => {
                            push_field(
                                program,
                                &mut pending,
                                &segments,
                                &visiting,
                                field.symbol,
                                field.type_reference,
                            )?;
                        }
                        DataMember::Variant(variant) => {
                            if program.symbols.get(variant.symbol).kind != SymbolKind::Variant {
                                return None;
                            }
                            let mut selected = segments.clone();
                            selected.push(PlaceSegment::Case {
                                variant: variant.symbol,
                            });
                            origins.cases.push(selected.clone());
                            for field in program.data_payload_fields(variant) {
                                push_field(
                                    program,
                                    &mut pending,
                                    &selected,
                                    &visiting,
                                    field.symbol,
                                    field.type_reference,
                                )?;
                            }
                        }
                    }
                }
            }
            _ => return None,
        }
    }
    if !origins.cases.is_empty() {
        origins.moves.push(
            super::super::path_instantiation::aggregate_arguments::AggregateMove {
                local_segments: Vec::new(),
                source: super::super::FrameSourcePlace {
                    root: origins.local_symbol,
                    segments: Vec::new(),
                    builtin_coordinates: origins.local_symbol.is_valid(),
                },
                type_reference: reference,
            },
        );
    }
    Some(origins)
}

/// A disjoint owned field has no reference leaves, but an unknown field is
/// not evidence of an empty frame. Validate the demand before filtering leaves.
pub(in crate::calls::write_frames) fn demand_is_declared(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    suffix: &str,
) -> bool {
    let mut pending = vec![(reference, suffix)];
    let mut visited = Vec::new();
    while let Some((reference, suffix)) = pending.pop() {
        if !reference.is_valid() || visited.contains(&(reference, suffix)) {
            continue;
        }
        visited.push((reference, suffix));
        if suffix.is_empty() {
            return true;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => pending.push((*base_type, suffix)),
            TypeReferenceNode::Reference { referee, .. } => pending.push((*referee, suffix)),
            node if concrete_nominal_type(node).is_some() => {
                let Some((symbol, _)) = concrete_nominal_type(node) else {
                    continue;
                };
                let Some(suffix) = suffix.strip_prefix('.') else {
                    continue;
                };
                let (field_name, rest) = super::split_place_root(suffix);
                let Some(definition) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == symbol)
                else {
                    continue;
                };
                for member in program.data_members(definition) {
                    match member {
                        DataMember::Field(field) if field.name.as_str() == field_name => {
                            pending.push((field.type_reference, rest));
                        }
                        DataMember::Variant(variant) => {
                            for field in program.data_payload_fields(variant) {
                                if field.name.as_str() == field_name {
                                    pending.push((field.type_reference, rest));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    false
}

type PendingOrigin = (
    TypeReferenceHandle,
    Vec<PlaceSegment>,
    Vec<TypeReferenceHandle>,
);

fn push_field(
    program: &TypedTrees,
    pending: &mut Vec<PendingOrigin>,
    segments: &[PlaceSegment],
    visiting: &[TypeReferenceHandle],
    symbol: SymbolHandle,
    reference: TypeReferenceHandle,
) -> Option<()> {
    if program.symbols.get(symbol).kind != SymbolKind::Field {
        return None;
    }
    let mut segments = segments.to_vec();
    segments.push(PlaceSegment::Field { symbol });
    pending.push((reference, segments, visiting.to_vec()));
    Some(())
}
