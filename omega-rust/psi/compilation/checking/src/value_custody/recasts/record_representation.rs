use super::scalar_representation::{
    MutableScalarRepresentationFacts, mutable_scalar_representation_facts,
    mutable_scalar_representation_facts_equivalent, narrow_by_field_literal_bounds,
    scalar_representation_facts_imply,
};
use std::collections::HashSet;
use typed_trees::TypedTrees;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

type SymbolIdentity = (u32, u32);

const MAX_RECAST_REPRESENTATION_DEPTH: usize = 256;
const MAX_RECAST_REPRESENTATION_WORK: usize = 16384;

struct RepresentationBudget {
    depth: usize,
    work: usize,
    lifetime_shell_depth: usize,
}

impl RepresentationBudget {
    fn enter(&mut self) -> Option<()> {
        self.depth = self.depth.checked_add(1)?;
        (self.depth <= MAX_RECAST_REPRESENTATION_DEPTH).then_some(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn consume(&mut self, amount: usize) -> Option<()> {
        self.work = self.work.checked_add(amount)?;
        (self.work <= MAX_RECAST_REPRESENTATION_WORK).then_some(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct MutableRecordRepresentation {
    pub(super) size: usize,
    align: usize,
    leaves: Vec<MutableRecordLeaf>,
    pub(super) has_stored_integer_projection: bool,
}

#[derive(Debug, Clone)]
struct MutableRecordLeaf {
    offset: usize,
    size: usize,
    facts: MutableScalarRepresentationFacts,
    path: Vec<RepresentationSegment>,
    primitive: typed_trees::types::PrimitiveType,
    stored_integer: Option<(u16, layout_plans::IntegerInterpretation)>,
    byte_projection: bool,
    arrays: Vec<NormalizedArrayStep>,
}

/// Depth from the scalar leaf, so prepending enclosing fields preserves the
/// coordinate. Stride comes from normalization, including explicit layout plans.
#[derive(Debug, Clone)]
struct NormalizedArrayStep {
    suffix_length: usize,
    stride: usize,
    extent: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepresentationSegment {
    Field(symbols::SymbolHandle),
    Element(usize),
}

/// A scalar observation's physical geometry, derived by the same normalization
/// that checks recast compatibility. This describes storage, not loan authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecastScalarProjection {
    pub offset: usize,
    pub footprint: usize,
    pub primitive: typed_trees::types::PrimitiveType,
    pub stored_integer: Option<semantic_vocabulary::IntegerType>,
}

/// One retained runtime selector's physical geometry. The caller must resolve
/// the exact input path occurrence and independently establish its array bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecastRuntimeIndexGeometry {
    pub path_index: usize,
    pub stride: usize,
    pub extent: usize,
}

/// A scalar projection with runtime coordinates left symbolic. `offset` in
/// `projection` includes fixed coordinates only; each runtime occurrence adds
/// its value times its stride. Geometry provides neither a value nor a proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecastScalarGeometry {
    pub projection: RecastScalarProjection,
    pub indices: Vec<RecastRuntimeIndexGeometry>,
}

/// One referee's immutable normalized scalar geometry, shared across captured
/// loans and evaluation namespaces without retaining source trees. These are
/// type-owned leaves, not source-expression or selector specializations.
#[derive(Debug, Clone)]
pub struct RecastScalarLayout {
    leaves: std::sync::Arc<[RecastScalarLayoutLeaf]>,
}

#[derive(Debug)]
struct RecastScalarLayoutLeaf {
    path: Box<[checked_trees::CheckedUnitStructuralPathSegment]>,
    projection: RecastScalarProjection,
    arrays: Box<[NormalizedArrayStep]>,
}

impl RecastScalarLayout {
    /// Select an exact scalar leaf. Missing fields, aggregate endpoints,
    /// out-of-bounds fixed indices and runtime selectors have no matching
    /// normalized leaf and remain rejected.
    pub fn scalar_path_projection(
        &self,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<RecastScalarProjection> {
        let mut matching = self.leaves.iter().filter(|leaf| leaf.path.as_ref() == path);
        let selected = matching.next()?;
        matching.next().is_none().then_some(selected.projection)
    }

    /// Resolve fields and fixed coordinates exactly, retaining every runtime
    /// array occurrence separately. Selecting its zero element supplies the
    /// normalized constant offset; the array node supplies stride and extent.
    pub fn scalar_path_geometry(
        &self,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<RecastScalarGeometry> {
        use checked_trees::CheckedUnitStructuralPathSegment as Segment;
        if path.len() > MAX_RECAST_REPRESENTATION_DEPTH {
            return None;
        }
        let mut matching = self.leaves.iter().filter(|leaf| {
            leaf.path.len() == path.len()
                && leaf
                    .path
                    .iter()
                    .zip(path)
                    .all(|(fixed, requested)| match requested {
                        Segment::RuntimeIndex(_) => matches!(fixed, Segment::FixedIndex(0)),
                        _ => fixed == requested,
                    })
        });
        let leaf = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let mut indices = Vec::new();
        for (path_index, segment) in path.iter().enumerate() {
            if !matches!(segment, Segment::RuntimeIndex(_)) {
                continue;
            }
            let suffix_length = path.len().checked_sub(path_index.checked_add(1)?)?;
            let mut arrays = leaf
                .arrays
                .iter()
                .filter(|array| array.suffix_length == suffix_length);
            let array = arrays.next()?;
            if arrays.next().is_some() || array.extent == 0 {
                return None;
            }
            indices.try_reserve(1).ok()?;
            indices.push(RecastRuntimeIndexGeometry {
                path_index,
                stride: array.stride,
                extent: array.extent,
            });
        }
        Some(RecastScalarGeometry {
            projection: leaf.projection,
            indices,
        })
    }
}

/// Prepare the existing recast normalization once. Field path identities are
/// derived from each normalized leaf's exact owning declaration and symbol;
/// this publishes geometry only, never backing custody or write permission.
pub fn recast_scalar_layout(
    program: &TypedTrees,
    root: TypeReferenceHandle,
) -> Option<RecastScalarLayout> {
    let representation = shared_projection_type_representation(program, root)?;
    let mut leaves = Vec::new();
    leaves.try_reserve_exact(representation.leaves.len()).ok()?;
    for leaf in &representation.leaves {
        if !leaf.byte_projection {
            continue;
        }
        let path = checked_projection_path(program, root, &leaf.path)?;
        leaves.push(RecastScalarLayoutLeaf {
            path: path.into_boxed_slice(),
            projection: scalar_leaf_projection(representation.size, leaf)?,
            arrays: leaf.arrays.clone().into_boxed_slice(),
        });
    }
    Some(RecastScalarLayout {
        leaves: leaves.into(),
    })
}

fn checked_projection_path(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    path: &[RepresentationSegment],
) -> Option<Vec<checked_trees::CheckedUnitStructuralPathSegment>> {
    use checked_trees::CheckedUnitStructuralPathSegment as CheckedSegment;
    let mut carrier = root;
    let mut checked_path = Vec::new();
    checked_path.try_reserve_exact(path.len()).ok()?;
    for segment in path {
        match segment {
            RepresentationSegment::Field(symbol) => {
                let owner = crate::program_scope::data_definitions::data_definition_by_symbol(
                    program,
                    program.type_reference_symbol(carrier),
                )?;
                let mut fields =
                    program
                        .data_members(owner)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol && !field.relevance.is_erased() =>
                            {
                                Some(field)
                            }
                            _ => None,
                        });
                let selected = fields.next()?;
                if fields.next().is_some() {
                    return None;
                }
                checked_path.push(CheckedSegment::Field(selected.path_identity()));
                carrier = selected.type_reference;
            }
            RepresentationSegment::Element(index) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                if index >= length {
                    return None;
                }
                checked_path.push(CheckedSegment::FixedIndex(u64::try_from(*index).ok()?));
                carrier = *element_type;
            }
        }
    }
    // Replay the public path through the same owner-qualified resolver used
    // by one-shot queries. Same-spelled or ambiguous members cannot replace
    // the exact symbols retained by normalization.
    (recast_projection_path(program, root, &checked_path)? == path).then_some(checked_path)
}

/// Resolve an already checked record path under its exact field owners, then
/// select its normalized leaf. Keeping field identities on normalization leaves
/// avoids another layout calculator in source-to-Terminal projection lowering.
pub fn recast_scalar_projection(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    carrier_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    field: symbols::SymbolHandle,
) -> Option<RecastScalarProjection> {
    let mut path = recast_projection_path(program, root, carrier_path)?;
    // The field-ending API retains the exact authored symbol, not merely a
    // same-spelled member of the selected carrier.
    path.push(RepresentationSegment::Field(field));
    normalized_scalar_projection(program, root, &path)
}

/// Resolve a complete scalar-leaf path, including fixed-array endpoints, using
/// the normalized recast geometry. Runtime selectors require separately
/// retained values and are not geometry supplied by this query.
pub fn recast_scalar_path_projection(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    scalar_path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<RecastScalarProjection> {
    let path = recast_projection_path(program, root, scalar_path)?;
    normalized_scalar_projection(program, root, &path)
}

/// One-shot counterpart of the prepared layout query. Retained selectors remain
/// caller-owned; this reconstructs only the exact declaration's geometry.
pub fn recast_scalar_path_geometry(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    scalar_path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<RecastScalarGeometry> {
    recast_scalar_layout(program, root)?.scalar_path_geometry(scalar_path)
}

fn recast_projection_path(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    carrier_path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RepresentationSegment>> {
    let mut carrier = root;
    let mut path = Vec::new();
    for segment in carrier_path {
        let identity = match segment {
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => identity,
            // A literal element of a fixed array selects that element's
            // normalized leaves, which carry the same element coordinate.
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                let index = usize::try_from(*index).ok()?;
                if index >= *length {
                    return None;
                }
                path.push(RepresentationSegment::Element(index));
                carrier = *element_type;
                continue;
            }
            _ => return None,
        };
        let owner = crate::program_scope::data_definitions::data_definition_by_symbol(
            program,
            program.type_reference_symbol(carrier),
        )?;
        let mut fields = program
            .data_members(owner)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(candidate)
                    if !candidate.relevance.is_erased()
                        && candidate.path_identity() == *identity =>
                {
                    Some(candidate)
                }
                _ => None,
            });
        let selected = fields.next()?;
        if fields.next().is_some() {
            return None;
        }
        path.push(RepresentationSegment::Field(selected.symbol));
        carrier = selected.type_reference;
    }
    Some(path)
}

fn normalized_scalar_projection(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    path: &[RepresentationSegment],
) -> Option<RecastScalarProjection> {
    let representation = shared_projection_type_representation(program, root)?;
    let leaf = representation
        .leaves
        .iter()
        .find(|leaf| leaf.path == path)?;
    scalar_leaf_projection(representation.size, leaf)
}

fn scalar_leaf_projection(
    footprint: usize,
    leaf: &MutableRecordLeaf,
) -> Option<RecastScalarProjection> {
    if !leaf.byte_projection {
        return None;
    }
    Some(RecastScalarProjection {
        offset: leaf.offset,
        footprint,
        primitive: leaf.primitive,
        stored_integer: match leaf.stored_integer {
            Some((width, interpretation)) => {
                let sign = match interpretation {
                    layout_plans::IntegerInterpretation::Signed => {
                        semantic_vocabulary::IntegerSign::Signed
                    }
                    layout_plans::IntegerInterpretation::Unsigned => {
                        semantic_vocabulary::IntegerSign::Unsigned
                    }
                };
                Some(semantic_vocabulary::IntegerType::new(sign, width).ok()?)
            }
            None => None,
        },
    })
}

/// One step from a record root to a normalized leaf: a field by its exact
/// symbol, or a literal element of a fixed array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecastLeafSegment {
    Field(symbols::SymbolHandle),
    Element(usize),
}

/// The source leaf a typed record recast view's direct field observes, and
/// the carriers on both sides: an admitted view keeps the leaf's width but
/// may name an integer of the other sign.
pub struct RecastRecordSourceLeaf {
    pub path: Vec<RecastLeafSegment>,
    pub primitive: typed_trees::types::PrimitiveType,
    pub view_primitive: typed_trees::types::PrimitiveType,
}

/// Rejoin a typed record view's direct scalar field to the source leaf it
/// re-views. An admitted record recast has identical geometry on both sides
/// (the judgment above compares leaves at equal offsets and widths), so the
/// one source leaf at the target leaf's offset and width is the storage the
/// view's field names. `None` when either side has no normalized
/// representation, the field is not a direct scalar leaf of the target, or no
/// single source leaf matches; the caller then keeps its refusal.
pub fn recast_record_source_leaf(
    program: &TypedTrees,
    source: TypeReferenceHandle,
    target: TypeReferenceHandle,
    target_field: symbols::SymbolHandle,
) -> Option<RecastRecordSourceLeaf> {
    let source_representation = shared_projection_type_representation(program, source)?;
    let target_representation = shared_projection_type_representation(program, target)?;
    if source_representation.size != target_representation.size
        || source_representation.leaves.len() != target_representation.leaves.len()
    {
        return None;
    }
    let target_leaf = target_representation
        .leaves
        .iter()
        .find(|leaf| leaf.path == [RepresentationSegment::Field(target_field)])?;
    let mut matching = source_representation
        .leaves
        .iter()
        .filter(|leaf| leaf.offset == target_leaf.offset && leaf.size == target_leaf.size);
    let source_leaf = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(RecastRecordSourceLeaf {
        path: source_leaf
            .path
            .iter()
            .map(|segment| match segment {
                RepresentationSegment::Field(symbol) => RecastLeafSegment::Field(*symbol),
                RepresentationSegment::Element(index) => RecastLeafSegment::Element(*index),
            })
            .collect(),
        primitive: source_leaf.primitive,
        view_primitive: target_leaf.primitive,
    })
}

pub(super) fn mutable_type_representation(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<MutableRecordRepresentation> {
    type_representation(program, type_reference, false)
}

pub(super) fn shared_projection_type_representation(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<MutableRecordRepresentation> {
    type_representation(program, type_reference, true)
}

fn type_representation(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    allow_stored_integer_projection: bool,
) -> Option<MutableRecordRepresentation> {
    let mut visiting = HashSet::new();
    visiting.try_reserve(256).ok()?;
    let mut budget = RepresentationBudget {
        depth: 0,
        work: 0,
        lifetime_shell_depth: 0,
    };
    let phantom_lifetime_shell =
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            program,
            type_reference,
        );
    let mut representation = if let Some(symbol) = phantom_lifetime_shell {
        budget.lifetime_shell_depth = 1;
        mutable_record_representation_inner(
            program,
            symbol,
            &mut visiting,
            &mut budget,
            allow_stored_integer_projection,
            true,
        )?
    } else {
        mutable_record_type_representation(
            program,
            type_reference,
            &mut visiting,
            &mut budget,
            allow_stored_integer_projection,
            false,
        )?
    };
    if phantom_lifetime_shell.is_some() && representation.size == 0 {
        return None;
    }
    representation
        .leaves
        .sort_by_key(|leaf| (leaf.offset, leaf.size));
    Some(representation)
}

/// Normalize one established record into the byte geometry and scalar
/// representation facts a mutable alias may expose. Leaves may carry scalar
/// domains/ranges (and bool's exact `{0,1}` set), because both alias
/// directions are checked below.
///
/// A `where` conjunction whose every conjunct bounds one direct scalar field
/// by a literal constrains each field independently, so the record's valid
/// values are exactly the product of its narrowed leaves and the leaf-wise
/// comparison decides the whole judgment. Writes through either view still
/// re-prove that view's own `where` facts, which the comparison has shown
/// equivalent (mutable) or implied (shared). Relational facts, bounds on a
/// nested aggregate and zero gating from any other fact remain fenced: a
/// leaf-wise comparison cannot preserve them.
fn mutable_record_representation_inner(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    visiting: &mut HashSet<SymbolIdentity>,
    budget: &mut RepresentationBudget,
    allow_stored_integer_projection: bool,
    allow_lifetime_shell: bool,
) -> Option<MutableRecordRepresentation> {
    budget.enter()?;
    let representation = mutable_record_representation_inner_body(
        program,
        symbol,
        visiting,
        budget,
        allow_stored_integer_projection,
        allow_lifetime_shell,
    );
    budget.leave();
    representation
}

fn mutable_record_representation_inner_body(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
    visiting: &mut HashSet<SymbolIdentity>,
    budget: &mut RepresentationBudget,
    allow_stored_integer_projection: bool,
    allow_lifetime_shell: bool,
) -> Option<MutableRecordRepresentation> {
    if !symbol.is_valid() {
        return None;
    }
    let symbol_identity = (symbol.arena_index(), symbol.generation());
    if !visiting.insert(symbol_identity) {
        return None;
    }
    let data = crate::program_scope::data_definitions::data_definition_by_symbol(program, symbol)?;
    let Some(where_bounds) =
        crate::proof_contracts::default_domains::data_where_field_literal_bounds(program, data)
    else {
        visiting.remove(&symbol_identity);
        return None;
    };
    // Zero gating records that zero may violate the facts; it says nothing a
    // view of already established storage needs once interval-only bounds
    // narrow every leaf they mention. Without such bounds the gate's source is
    // not represented here, so it stays fenced.
    if data.zero_gated && where_bounds.is_empty() {
        visiting.remove(&symbol_identity);
        return None;
    }

    let members = program.data_members(data);
    let mut fields = Vec::new();
    let mut field_types = Vec::new();
    let mut field_symbols = Vec::new();
    fields.try_reserve_exact(members.len()).ok()?;
    field_types.try_reserve_exact(members.len()).ok()?;
    field_symbols.try_reserve_exact(members.len()).ok()?;
    for member in members {
        let typed_trees::data::DataMember::Field(field) = member else {
            visiting.remove(&symbol_identity);
            return None;
        };
        if field.relevance.is_erased() {
            continue;
        }
        let Some(mut representation) = mutable_record_type_representation(
            program,
            field.type_reference,
            visiting,
            budget,
            allow_stored_integer_projection,
            allow_lifetime_shell,
        ) else {
            visiting.remove(&symbol_identity);
            return None;
        };
        let field_bounds = where_bounds
            .iter()
            .filter(|bound| bound.field == field.symbol)
            .copied()
            .collect::<Vec<_>>();
        if !field_bounds.is_empty() {
            let narrowed = match representation.leaves.as_mut_slice() {
                [leaf] if leaf.path.is_empty() => narrow_by_field_literal_bounds(
                    program,
                    data,
                    &mut leaf.facts,
                    leaf.primitive,
                    &field_bounds,
                ),
                _ => None,
            };
            if narrowed.is_none() {
                visiting.remove(&symbol_identity);
                return None;
            }
        }
        fields.push(representation);
        field_types.push(field.type_reference);
        field_symbols.push(field.symbol);
    }

    let mut has_stored_integer_projection = fields
        .iter()
        .any(|field| field.has_stored_integer_projection);
    let (size, align, offsets) = if let Some(plan) = program
        .plan_laid_layouts
        .iter()
        .find(|plan| plan.data_symbol == data.symbol)
    {
        if plan.field_symbols != field_symbols
            || (!allow_stored_integer_projection && !plan.integer_fields.is_empty())
            || plan.offsets.len() != fields.len()
        {
            visiting.remove(&symbol_identity);
            return None;
        }
        for integer_field in &plan.integer_fields {
            let field = fields.get_mut(integer_field.field_index)?;
            if field.leaves.len() != 1
                || integer_field.stored_width_bits == 0
                || integer_field.stored_width_bits % 8 != 0
            {
                visiting.remove(&symbol_identity);
                return None;
            }
            let stored_size = usize::from(integer_field.stored_width_bits / 8);
            field.size = stored_size;
            field.align = field.align.min(stored_size.max(1));
            field.leaves[0].size = stored_size;
            field.leaves[0].stored_integer = Some((
                integer_field.stored_width_bits,
                integer_field.interpretation,
            ));
            has_stored_integer_projection = true;
        }
        // An offset and width cannot replace a fragmented bit recipe.
        for bit_field in &plan.bit_fields {
            for leaf in &mut fields.get_mut(bit_field.field_index)?.leaves {
                leaf.byte_projection = false;
            }
        }
        for repeated_field in &plan.repeated_fields {
            let field_type = *field_types.get(repeated_field.field_index)?;
            let TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(element_count),
            } = program.type_reference_table.type_reference(field_type)
            else {
                visiting.remove(&symbol_identity);
                return None;
            };
            let element = mutable_record_type_representation(
                program,
                *element_type,
                visiting,
                budget,
                allow_stored_integer_projection,
                false,
            )?;
            let repeated = repeat_representation_with_stride(
                &element,
                *element_count,
                repeated_field.element_stride,
            )?;
            *fields.get_mut(repeated_field.field_index)? = repeated;
        }
        if fields.iter().zip(&plan.offsets).any(|(field, offset)| {
            offset
                .checked_add(field.size)
                .is_none_or(|end| end > plan.size)
        }) {
            visiting.remove(&symbol_identity);
            return None;
        }
        let mut offsets = Vec::new();
        offsets.try_reserve_exact(plan.offsets.len()).ok()?;
        offsets.extend_from_slice(&plan.offsets);
        (plan.size, plan.align, offsets)
    } else {
        let mut offsets = Vec::new();
        offsets.try_reserve_exact(fields.len()).ok()?;
        let mut offset = 0usize;
        let mut max_align = 1usize;
        for field in &fields {
            offset = checked_align_up(offset, field.align)?;
            offsets.push(offset);
            offset = offset.checked_add(field.size)?;
            max_align = max_align.max(field.align);
        }
        (checked_align_up(offset, max_align)?, max_align, offsets)
    };

    let leaf_count = fields
        .iter()
        .try_fold(0usize, |count, field| count.checked_add(field.leaves.len()))?;
    budget.consume(leaf_count)?;
    let mut leaves = Vec::new();
    leaves.try_reserve_exact(leaf_count).ok()?;
    for ((field, field_offset), symbol) in fields.into_iter().zip(offsets).zip(field_symbols) {
        for mut leaf in field.leaves {
            leaf.offset = leaf.offset.checked_add(field_offset)?;
            leaf.path.insert(0, RepresentationSegment::Field(symbol));
            leaves.push(leaf);
        }
    }
    visiting.remove(&symbol_identity);
    Some(MutableRecordRepresentation {
        size,
        align,
        leaves,
        has_stored_integer_projection,
    })
}

fn mutable_record_type_representation(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<SymbolIdentity>,
    budget: &mut RepresentationBudget,
    allow_stored_integer_projection: bool,
    allow_lifetime_shell: bool,
) -> Option<MutableRecordRepresentation> {
    budget.enter()?;
    let representation = mutable_record_type_representation_body(
        program,
        type_reference,
        visiting,
        budget,
        allow_stored_integer_projection,
        allow_lifetime_shell,
    );
    budget.leave();
    representation
}

fn mutable_record_type_representation_body(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<SymbolIdentity>,
    budget: &mut RepresentationBudget,
    allow_stored_integer_projection: bool,
    allow_lifetime_shell: bool,
) -> Option<MutableRecordRepresentation> {
    if let Some(primitive) = super::exact_scalar_representation_type(program, type_reference) {
        let size = primitive.scalar_byte_size()?;
        budget.consume(1)?;
        let mut leaves = Vec::new();
        leaves.try_reserve_exact(1).ok()?;
        leaves.push(MutableRecordLeaf {
            offset: 0,
            size,
            facts: mutable_scalar_representation_facts(program, type_reference)?,
            path: Vec::new(),
            primitive,
            stored_integer: None,
            byte_projection: true,
            arrays: Vec::new(),
        });
        return Some(MutableRecordRepresentation {
            size,
            align: size,
            leaves,
            has_stored_integer_projection: false,
        });
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let element = mutable_record_type_representation(
                program,
                *element_type,
                visiting,
                budget,
                allow_stored_integer_projection,
                false,
            )?;
            let size = element.size.checked_mul(*length)?;
            let leaf_count = element.leaves.len().checked_mul(*length)?;
            budget.consume(leaf_count)?;
            let mut leaves = Vec::new();
            leaves.try_reserve_exact(leaf_count).ok()?;
            for index in 0..*length {
                let element_offset = element.size.checked_mul(index)?;
                for leaf in &element.leaves {
                    let mut projected = leaf.clone();
                    projected.arrays.push(NormalizedArrayStep {
                        suffix_length: projected.path.len(),
                        stride: element.size,
                        extent: *length,
                    });
                    projected.path.insert(0, RepresentationSegment::Element(index));
                    projected.offset = leaf.offset.checked_add(element_offset)?;
                    leaves.push(projected);
                }
            }
            Some(MutableRecordRepresentation {
                size,
                align: element.align,
                leaves,
                has_stored_integer_projection: element.has_stored_integer_projection,
            })
        }
        TypeReferenceNode::Named { symbol, .. } => mutable_record_representation_inner(
            program,
            *symbol,
            visiting,
            budget,
            allow_stored_integer_projection,
            allow_lifetime_shell,
        ),
        TypeReferenceNode::Generic { .. }
            if allow_lifetime_shell
                && budget.lifetime_shell_depth > 0
                && budget.lifetime_shell_depth < crate::value_custody::recasts::record_eligibility::MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH =>
        {
            let symbol = crate::value_custody::recasts::record_eligibility::phantom_lifetime_record_symbol_shape(program, type_reference)?;
            budget.lifetime_shell_depth += 1;
            let representation = mutable_record_representation_inner(
                program,
                symbol,
                visiting,
                budget,
                allow_stored_integer_projection,
                allow_lifetime_shell,
            );
            budget.lifetime_shell_depth -= 1;
            representation.filter(|representation| representation.size > 0)
        }
        // A non-scalar constraint is a fact over the aggregate rather than a
        // leaf representation fact. It cannot be preserved by this rung.
        TypeReferenceNode::Constrained { .. } | TypeReferenceNode::Reference { .. } => None,
        _ => None,
    }
}

fn checked_align_up(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 {
        return None;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment.checked_sub(remainder)?)
    }
}

pub(super) fn mutable_record_representations_equivalent(
    program: &TypedTrees,
    source: &MutableRecordRepresentation,
    target: &MutableRecordRepresentation,
) -> bool {
    source.size == target.size
        && source.align == target.align
        && source.leaves.len() == target.leaves.len()
        && source
            .leaves
            .iter()
            .zip(&target.leaves)
            .all(|(source, target)| {
                source.offset == target.offset
                    && source.size == target.size
                    && mutable_scalar_representation_facts_equivalent(
                        program,
                        &source.facts,
                        &target.facts,
                    )
            })
}

pub(super) fn repeat_representation(
    element: &MutableRecordRepresentation,
    count: usize,
) -> Option<MutableRecordRepresentation> {
    let size = element.size.checked_mul(count)?;
    let leaf_count = element.leaves.len().checked_mul(count)?;
    let mut leaves = Vec::new();
    leaves.try_reserve_exact(leaf_count).ok()?;
    for index in 0..count {
        let base = element.size.checked_mul(index)?;
        for leaf in &element.leaves {
            let mut projected = leaf.clone();
            projected.arrays.push(NormalizedArrayStep {
                suffix_length: projected.path.len(),
                stride: element.size,
                extent: count,
            });
            projected
                .path
                .insert(0, RepresentationSegment::Element(index));
            projected.offset = base.checked_add(leaf.offset)?;
            leaves.push(projected);
        }
    }
    Some(MutableRecordRepresentation {
        size,
        align: element.align,
        leaves,
        has_stored_integer_projection: element.has_stored_integer_projection,
    })
}

fn repeat_representation_with_stride(
    element: &MutableRecordRepresentation,
    count: usize,
    stride: usize,
) -> Option<MutableRecordRepresentation> {
    if count > 1 && stride < element.size {
        return None;
    }
    let size = if count == 0 {
        0
    } else {
        stride
            .checked_mul(count.checked_sub(1)?)?
            .checked_add(element.size)?
    };
    let leaf_count = element.leaves.len().checked_mul(count)?;
    let mut leaves = Vec::new();
    leaves.try_reserve_exact(leaf_count).ok()?;
    for index in 0..count {
        let base = stride.checked_mul(index)?;
        for leaf in &element.leaves {
            let mut projected = leaf.clone();
            projected.arrays.push(NormalizedArrayStep {
                suffix_length: projected.path.len(),
                stride,
                extent: count,
            });
            projected
                .path
                .insert(0, RepresentationSegment::Element(index));
            projected.offset = base.checked_add(leaf.offset)?;
            leaves.push(projected);
        }
    }
    Some(MutableRecordRepresentation {
        size,
        align: element.align,
        leaves,
        has_stored_integer_projection: element.has_stored_integer_projection,
    })
}

pub(super) fn representation_is_exactly_tiled(
    representation: &MutableRecordRepresentation,
) -> bool {
    let mut cursor = 0usize;
    for leaf in &representation.leaves {
        if leaf.offset != cursor || leaf.size == 0 {
            return false;
        }
        let Some(next) = cursor.checked_add(leaf.size) else {
            return false;
        };
        cursor = next;
    }
    cursor == representation.size
}

pub(super) fn record_representation_implies(
    program: &TypedTrees,
    source: &MutableRecordRepresentation,
    target: &MutableRecordRepresentation,
) -> bool {
    source.size == target.size
        && source.align == target.align
        && source.leaves.len() == target.leaves.len()
        && source
            .leaves
            .iter()
            .zip(&target.leaves)
            .all(|(source, target)| {
                source.offset == target.offset
                    && source.size == target.size
                    && scalar_representation_facts_imply(program, &source.facts, &target.facts)
            })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod projection_tests;
