//! Type-derived symbolic origins for owned aggregate values. Cases are possible,
//! not selected by a constructor; arrays retain one unknown-element selector.

use super::super::isolation::concrete_nominal_type;
use super::{FramePathPrecision, FramePlaceOrigin, StoredLocalOrigins, StoredWriteOrigin};
use crate::fact_plan::PlaceSegment;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    FixedArrayLength, TypeReferenceHandle, TypeReferenceNode,
};
use symbols::{SymbolHandle, SymbolKind};

pub(in crate::validation::machine_calls::calls::write_frames) fn declared_origins(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
) -> Option<StoredLocalOrigins> {
    declared_origins_for_query(program, symbol, name, reference, false)
}

pub(in crate::validation::machine_calls::calls::write_frames) fn declared_origins_for_query(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
    include_shared: bool,
) -> Option<StoredLocalOrigins> {
    walk_declared_origins(
        program,
        symbol,
        name,
        reference,
        include_shared,
        OriginDetail::Paths,
    )
}

/// Whether the type's owned structure contains a sum case, under the same
/// admission rules as `declared_origins`. It walks the same structure but
/// builds no leaf paths.
pub(in crate::validation::machine_calls::calls::write_frames) fn declares_cases(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    walk_declared_origins(
        program,
        SymbolHandle::invalid(),
        "",
        reference,
        false,
        OriginDetail::ShapeOnly,
    )
    .is_some_and(|origins| !origins.cases.is_empty())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OriginDetail {
    Paths,
    /// Record one empty entry per case and no reference leaves.
    ShapeOnly,
}

/// One step of a type walk. Steps link to their parent, so a pending type
/// shares its path prefix instead of copying the segment and visiting lists
/// for each field it descends into.
#[derive(Clone, Copy)]
struct WalkStep {
    parent: Option<u32>,
    /// The type walked at this step. A case step walks no type and holds the
    /// invalid handle; it only contributes its segment.
    reference: TypeReferenceHandle,
    segment: Option<PlaceSegment>,
    /// How many segments the path through this step has, so `segments`
    /// allocates its result once.
    depth: u32,
}

#[derive(Default)]
struct TypeWalk {
    steps: Vec<WalkStep>,
}

impl TypeWalk {
    fn push(
        &mut self,
        parent: Option<u32>,
        reference: TypeReferenceHandle,
        segment: Option<PlaceSegment>,
    ) -> Option<u32> {
        let step = u32::try_from(self.steps.len()).ok()?;
        let depth = parent.map_or(0, |parent| self.steps[parent as usize].depth)
            + u32::from(segment.is_some());
        self.steps.push(WalkStep {
            parent,
            reference,
            segment,
            depth,
        });
        Some(step)
    }

    /// Whether `reference` is already being walked on the path above a step
    /// whose parent is `parent`: the type is recursive through owned storage.
    fn is_visiting(&self, mut parent: Option<u32>, reference: TypeReferenceHandle) -> bool {
        while let Some(step) = parent {
            let step = self.steps[step as usize];
            if step.reference == reference {
                return true;
            }
            parent = step.parent;
        }
        false
    }

    fn segments(&self, step: u32) -> Vec<PlaceSegment> {
        let WalkStep { depth, .. } = self.steps[step as usize];
        let mut segments = vec![
            PlaceSegment::Case {
                variant: SymbolHandle::invalid()
            };
            depth as usize
        ];
        let mut remaining = segments.len();
        let mut current = Some(step);
        while let Some(step) = current {
            let step = self.steps[step as usize];
            if let Some(segment) = step.segment {
                remaining -= 1;
                segments[remaining] = segment;
            }
            current = step.parent;
        }
        segments
    }
}

fn walk_declared_origins(
    program: &TypedTrees,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
    include_shared: bool,
    detail: OriginDetail,
) -> Option<StoredLocalOrigins> {
    let mut origins = StoredLocalOrigins {
        local_symbol: symbol,
        references: Vec::new(),
        cases: Vec::new(),
        moves: Vec::new(),
        symbolic: true,
    };
    let mut walk = TypeWalk::default();
    let mut pending = vec![walk.push(None, reference, None)?];
    while let Some(step) = pending.pop() {
        let WalkStep {
            parent, reference, ..
        } = walk.steps[step as usize];
        if !reference.is_valid() || walk.is_visiting(parent, reference) {
            return None;
        }
        if program.primitive_type_reference(reference).is_some() {
            continue;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Unit => {}
            TypeReferenceNode::Constrained { base_type, .. } => {
                pending.push(walk.push(Some(step), *base_type, None)?);
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
                if detail == OriginDetail::ShapeOnly {
                    continue;
                }
                let segments = walk.segments(step);
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
                    // This is only a may-write selector, never an actual index
                    // expression or evidence authorizing an element access.
                    let selector = PlaceSegment::Index {
                        expression: Default::default(),
                    };
                    pending.push(walk.push(Some(step), *element_type, Some(selector))?);
                }
            }
            node if concrete_nominal_type(node).is_some() => {
                let (symbol, _) = concrete_nominal_type(node)?;
                let definition = data_definition_by_symbol(program, symbol)?;
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
                                &mut walk,
                                &mut pending,
                                step,
                                field.symbol,
                                field.type_reference,
                            )?;
                        }
                        DataMember::Variant(variant) => {
                            if program.symbols.get(variant.symbol).kind != SymbolKind::Variant {
                                return None;
                            }
                            let case = walk.push(
                                Some(step),
                                TypeReferenceHandle::invalid(),
                                Some(PlaceSegment::Case {
                                    variant: variant.symbol,
                                }),
                            )?;
                            origins.cases.push(match detail {
                                OriginDetail::Paths => walk.segments(case),
                                OriginDetail::ShapeOnly => Vec::new(),
                            });
                            for field in program.data_payload_fields(variant) {
                                push_field(
                                    program,
                                    &mut walk,
                                    &mut pending,
                                    case,
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
pub(in crate::validation::machine_calls::calls::write_frames) fn demand_is_declared(
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
                let Some(definition) = data_definition_by_symbol(program, symbol) else {
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

/// A data definition by symbol, resolved through the build-scope memo when
/// one is open — the type walk otherwise re-scans the declaration table per
/// nominal node it descends into.
fn data_definition_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition> {
    crate::validation::machine_calls::effect_inference::plan_scope::data_definition_by_symbol(
        program, symbol,
    )
}

fn push_field(
    program: &TypedTrees,
    walk: &mut TypeWalk,
    pending: &mut Vec<u32>,
    parent: u32,
    symbol: SymbolHandle,
    reference: TypeReferenceHandle,
) -> Option<()> {
    if program.symbols.get(symbol).kind != SymbolKind::Field {
        return None;
    }
    pending.push(walk.push(
        Some(parent),
        reference,
        Some(PlaceSegment::Field { symbol }),
    )?);
    Some(())
}
