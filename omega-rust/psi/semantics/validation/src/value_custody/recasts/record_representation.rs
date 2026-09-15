use super::scalar_representation::{
    MutableScalarRepresentationFacts, mutable_scalar_representation_facts,
    mutable_scalar_representation_facts_equivalent, scalar_representation_facts_imply,
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
/// representation facts a mutable alias may expose. Record-wide invariants and
/// zero-gated establishment remain fenced: arbitrary field writes cannot prove
/// those relational facts. Leaves may carry scalar domains/ranges (and bool's
/// exact `{0,1}` set), because both alias directions are checked below.
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
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)?;
    if !data.where_facts.is_empty() || data.zero_gated {
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
        let Some(representation) = mutable_record_type_representation(
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
            has_stored_integer_projection = true;
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
    for (field, field_offset) in fields.into_iter().zip(offsets) {
        for mut leaf in field.leaves {
            leaf.offset = leaf.offset.checked_add(field_offset)?;
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
                    leaves.push(MutableRecordLeaf {
                        offset: leaf.offset.checked_add(element_offset)?,
                        size: leaf.size,
                        facts: leaf.facts.clone(),
                    });
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
            leaves.push(MutableRecordLeaf {
                offset: base.checked_add(leaf.offset)?,
                size: leaf.size,
                facts: leaf.facts.clone(),
            });
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
            leaves.push(MutableRecordLeaf {
                offset: base.checked_add(leaf.offset)?,
                size: leaf.size,
                facts: leaf.facts.clone(),
            });
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
