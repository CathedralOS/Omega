//! Type-derived origins for incoming owned aggregate values. Cases are possible,
//! not selected by a constructor; arrays retain one unknown-element selector.

use super::{FramePathPrecision, FramePlaceOrigin, StoredLocalOrigins, StoredWriteOrigin};
use psi_facts::PlaceSegment;
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn parameter_origins(
    program: &TypedTrees,
    parameter: &StateParameter,
) -> Option<StoredLocalOrigins> {
    let mut origins = StoredLocalOrigins {
        local_symbol: parameter.symbol,
        references: Vec::new(),
        cases: Vec::new(),
    };
    let mut pending = vec![(parameter.type_reference, Vec::new(), Vec::new())];
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
                if !access.is_exclusive() {
                    continue;
                }
                if !super::super::reference_origins::referent_has_only_owned_storage(
                    program, *referee,
                ) {
                    return None;
                }
                let mut path = parameter.name.as_str().to_owned();
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
                    local_symbol: parameter.symbol,
                    local_path: path.clone(),
                    local_segments: segments,
                    origin: FramePlaceOrigin { path, precision },
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
            TypeReferenceNode::Named { symbol, .. } => {
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == *symbol)?;
                if program.symbols.get(*symbol).kind != SymbolKind::Data
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
    Some(origins)
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
