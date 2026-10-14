//! Type-derived symbolic origins for owned aggregate values. Cases are possible,
//! not selected by a constructor. Fixed arrays retain distinct element slots;
//! slices retain an unknown-element selector. Expand fixed coordinates only
//! when a reference, opaque region or case is found: a large scalar array
//! must not manufacture a walk node per element.
//! Opaque members retain an unknown region, not an empty reference inventory.
//! A disjoint member write can then keep its footprint without granting any
//! knowledge of the opaque member's contents or their caller-visible effects.
//! Prepared applications supply their exact substituted field roster, including
//! generic carriers. Walking the open head instead loses that roster and turns
//! an owned home with shared descriptors into unknown write authority. This
//! query reads closed storage; it never specializes a template or grants a loan.

use super::super::type_instantiation::DeclarationScopes;
use super::{FramePlaceOrigin, StoredLocalOrigins, StoredWriteOrigin};
use facts::PlaceSegment;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(in crate::machine_calls::calls::write_frames) fn declared_origins(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
) -> Option<StoredLocalOrigins> {
    declared_origins_for_query(program, symbol, name, reference, false)
}

pub(in crate::machine_calls::calls::write_frames) fn declared_origins_for_query(
    program: &dyn crate::CheckingInput,
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

/// Whether the type's owned borrow-carrying structure contains a sum case,
/// under the same admission rules as `declared_origins`. It walks the same
/// structure but builds no leaf paths.
pub(in crate::machine_calls::calls::write_frames) fn declares_cases(
    program: &dyn crate::CheckingInput,
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
    scope: usize,
    fixed_extent: Option<usize>,
}

#[derive(Default)]
struct TypeWalk {
    steps: Vec<WalkStep>,
    scopes: DeclarationScopes,
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
            scope: parent.map_or(0, |parent| self.steps[parent as usize].scope),
            fixed_extent: None,
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

    fn paths(&self, step: u32) -> Vec<Vec<PlaceSegment>> {
        let mut paths = vec![self.segments(step)];
        let mut current = Some(step);
        while let Some(step) = current {
            let step = self.steps[step as usize];
            if let Some(extent) = step.fixed_extent {
                paths = paths
                    .into_iter()
                    .flat_map(|path| {
                        (0..extent).map(move |index| {
                            let mut selected = path.clone();
                            selected[step.depth as usize - 1] = PlaceSegment::FixedIndex { index };
                            selected
                        })
                    })
                    .collect();
            }
            current = step.parent;
        }
        paths
    }
}

fn walk_declared_origins(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
    name: &str,
    reference: TypeReferenceHandle,
    include_shared: bool,
    detail: OriginDetail,
) -> Option<StoredLocalOrigins> {
    let root_reference = reference;
    let mut origins = StoredLocalOrigins {
        local_symbol: symbol,
        references: Vec::new(),
        cases: Vec::new(),
        moves: Vec::new(),
        symbolic: true,
    };
    // This inventory transports aliases, not ordinary case values. A complete
    // reference-free shape contributes no loan identity, even when its arrays
    // contain many scalar cases. Reuse the capability proof (which excludes
    // opaque and unresolved storage) before materializing any slot paths.
    if super::super::type_capabilities::type_is_reference_free_value(program, reference, &[]) {
        return Some(origins);
    }
    let mut walk = TypeWalk::default();
    let mut pending = vec![walk.push(None, reference, None)?];
    while let Some(step) = pending.pop() {
        let WalkStep {
            parent,
            reference,
            scope,
            ..
        } = walk.steps[step as usize];
        if !reference.is_valid() || walk.is_visiting(parent, reference) {
            return None;
        }
        let (reference, scope) = walk.scopes.head(program, reference, scope);
        walk.steps[step as usize].scope = scope;
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
                if detail == OriginDetail::ShapeOnly {
                    continue;
                }
                for segments in walk.paths(step) {
                    let (path, precision) = super::super::place_paths::coordinate_storage_path(
                        program, name, &segments,
                    );
                    origins.references.push(StoredWriteOrigin {
                        kind: super::StoredOriginKind::Reference,
                        local_symbol: symbol,
                        local_path: path.clone(),
                        local_segments: segments.clone(),
                        may_select: false,
                        // An incoming descriptor has symbolic identity even
                        // when its referee contains more pointers. This names
                        // the incoming value, not the current pointee contents.
                        // Unestablished local frontiers retain the old unknown.
                        origin: (super::super::reference_origins::referent_has_only_owned_storage(
                            program, *referee,
                        ) || (include_shared
                            && symbol.is_valid()
                            && program.symbols.get(symbol).kind == SymbolKind::Parameter))
                            .then_some(FramePlaceOrigin {
                                path,
                                precision,
                                source: super::super::FrameSourcePlace {
                                    recasts: Vec::new(),
                                    representation: None,
                                    kind: Default::default(),
                                    root: symbol,
                                    segments,
                                    builtin_coordinates: symbol.is_valid(),
                                },
                            }),
                    });
                }
            }
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(0),
                ..
            } => {}
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let element = walk.push(
                    Some(step),
                    *element_type,
                    Some(PlaceSegment::FixedIndex { index: 0 }),
                )?;
                walk.steps[element as usize].fixed_extent = Some(*length);
                pending.push(element);
            }
            TypeReferenceNode::Slice { element_type } => {
                // This is a storage inventory, not evidence that any element
                // exists or that the caller may access one.
                let selector = PlaceSegment::Index {
                    expression: Default::default(),
                };
                pending.push(walk.push(Some(step), *element_type, Some(selector))?);
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let (definition, scope) = walk.scopes.definition(program, reference, scope)?;
                walk.steps[step as usize].scope = scope;
                if program.symbols.get(definition.symbol).kind != SymbolKind::Data {
                    return None;
                }
                if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
                    // The boundary declaration exposes no internal field roster.
                    // Keep its entire region unknown, so overlapping writes and
                    // exact-origin queries still fail closed. Rejecting the whole
                    // inventory instead loses independently declared siblings.
                    if detail == OriginDetail::Paths {
                        for segments in walk.paths(step) {
                            let (path, _) = super::super::place_paths::coordinate_storage_path(
                                program, name, &segments,
                            );
                            origins.references.push(StoredWriteOrigin {
                                kind: super::StoredOriginKind::OpaqueRegion,
                                local_symbol: origins.local_symbol,
                                local_path: path,
                                local_segments: segments,
                                may_select: false,
                                origin: None,
                            });
                        }
                    }
                    continue;
                }
                // Only the case-bearing value needs whole-subtree identity.
                // A record enclosing it has independently replaceable siblings;
                // claiming that whole record as one move prevents their transfer.
                if detail == OriginDetail::Paths
                    && program
                        .data_members(definition)
                        .iter()
                        .any(|member| matches!(member, DataMember::Variant(_)))
                {
                    for segments in walk.paths(step) {
                        if origins
                            .moves
                            .iter()
                            .any(|moved| segments.starts_with(&moved.local_segments))
                        {
                            continue;
                        }
                        let type_reference = super::projections::projected_storage_type(
                            program,
                            root_reference,
                            &segments,
                        )?;
                        origins.moves.push(
                            super::super::path_instantiation::aggregate_arguments::AggregateMove {
                                local_segments: segments.clone(),
                                source: super::super::FrameSourcePlace {
                                    root: symbol,
                                    segments,
                                    builtin_coordinates: symbol.is_valid(),
                                    ..Default::default()
                                },
                                type_reference,
                            },
                        );
                    }
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
                            match detail {
                                OriginDetail::Paths => origins.cases.extend(walk.paths(case)),
                                OriginDetail::ShapeOnly => origins.cases.push(Vec::new()),
                            }
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
    Some(origins)
}

/// A disjoint owned field has no reference leaves, but an unknown field is
/// not evidence of an empty frame. Validate the demand before filtering leaves.
pub(in crate::machine_calls::calls::write_frames) fn demand_is_declared(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    suffix: &str,
) -> bool {
    let mut pending = vec![(reference, suffix, 0)];
    let mut scopes = DeclarationScopes::default();
    let mut visited = Vec::new();
    while let Some((reference, suffix, scope)) = pending.pop() {
        if !reference.is_valid() || visited.contains(&(reference, suffix)) {
            continue;
        }
        visited.push((reference, suffix));
        if suffix.is_empty() {
            return true;
        }
        let (reference, scope) = scopes.head(program, reference, scope);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                pending.push((*base_type, suffix, scope))
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push((*referee, suffix, scope)),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let Some(suffix) = suffix.strip_prefix('.') else {
                    continue;
                };
                let (field_name, rest) = super::split_place_root(suffix);
                let Some((definition, scope)) = scopes.definition(program, reference, scope) else {
                    continue;
                };
                for member in program.data_members(definition) {
                    match member {
                        DataMember::Field(field) if field.name.as_str() == field_name => {
                            pending.push((field.type_reference, rest, scope));
                        }
                        DataMember::Variant(variant) => {
                            for field in program.data_payload_fields(variant) {
                                if field.name.as_str() == field_name {
                                    pending.push((field.type_reference, rest, scope));
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

fn push_field(
    program: &dyn crate::CheckingInput,
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
