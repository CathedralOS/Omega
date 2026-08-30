use super::scalar_representation::{
    mutable_scalar_representation_facts, mutable_scalar_representation_facts_equivalent,
    scalar_representation_facts_imply, MutableScalarRepresentationFacts,
};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use psi_typed_trees::TypedTrees;
use std::collections::HashSet;

type SymbolIdentity = (u32, u32);

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
    let mut representation = mutable_record_type_representation(
        program,
        type_reference,
        &mut HashSet::new(),
        allow_stored_integer_projection,
    )?;
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
    symbol: psi_symbols::SymbolHandle,
    visiting: &mut HashSet<SymbolIdentity>,
    allow_stored_integer_projection: bool,
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

    let mut fields = Vec::new();
    let mut field_types = Vec::new();
    let mut field_symbols = Vec::new();
    for member in program.data_members(data) {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
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
            allow_stored_integer_projection,
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
                allow_stored_integer_projection,
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
        (plan.size, plan.align, plan.offsets.clone())
    } else {
        let mut offsets = Vec::with_capacity(fields.len());
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

    let mut leaves = Vec::new();
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
    allow_stored_integer_projection: bool,
) -> Option<MutableRecordRepresentation> {
    if let Some(primitive) = program.primitive_type_reference(type_reference) {
        let size = primitive.scalar_byte_size()?;
        return Some(MutableRecordRepresentation {
            size,
            align: size,
            leaves: vec![MutableRecordLeaf {
                offset: 0,
                size,
                facts: mutable_scalar_representation_facts(program, type_reference)?,
            }],
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
                allow_stored_integer_projection,
            )?;
            let size = element.size.checked_mul(*length)?;
            let leaf_count = element.leaves.len().checked_mul(*length)?;
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
            allow_stored_integer_projection,
        ),
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
mod tests {
    use super::*;
    use psi_typed_trees::data::{DataDefinition, DataField, DataMember};
    use psi_typed_trees::name::Identifier;

    fn named_type(
        program: &mut TypedTrees,
        symbol: psi_symbols::SymbolHandle,
        name: &str,
    ) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(name),
            })
    }

    fn push_single_field_record(
        program: &mut TypedTrees,
        symbol: psi_symbols::SymbolHandle,
        field_type: TypeReferenceHandle,
    ) {
        let mut definition = DataDefinition {
            symbol,
            name: Identifier::generated("Cell"),
            ..DataDefinition::default()
        };
        program.push_data_member(
            &mut definition,
            DataMember::Field(DataField {
                type_reference: field_type,
                ..DataField::default()
            }),
        );
        program.push_data_definition(definition);
    }

    #[test]
    fn representation_resolves_same_spelling_by_exact_symbol() {
        let mut program = TypedTrees::default();
        let first_symbol = psi_symbols::SymbolHandle::from_arena_index(11);
        let selected_symbol = psi_symbols::SymbolHandle::from_arena_index(12);
        let u64_type = named_type(&mut program, psi_symbols::SymbolHandle::invalid(), "u64");
        let u8_type = named_type(&mut program, psi_symbols::SymbolHandle::invalid(), "u8");
        push_single_field_record(&mut program, first_symbol, u64_type);
        push_single_field_record(&mut program, selected_symbol, u8_type);
        let selected_type = named_type(&mut program, selected_symbol, "Cell");

        let representation = mutable_type_representation(&program, selected_type)
            .expect("the selected record has an exact scalar representation");

        assert_eq!(representation.size, 1);
        assert_eq!(representation.align, 1);
        assert_eq!(representation.leaves.len(), 1);
    }

    #[test]
    fn repeated_leaf_capacity_overflow_fails_closed() {
        let mut program = TypedTrees::default();
        let u8_type = named_type(&mut program, psi_symbols::SymbolHandle::invalid(), "u8");
        let element = mutable_type_representation(&program, u8_type)
            .expect("u8 has a one-byte representation");
        let fixed_array = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: u8_type,
                length: FixedArrayLength::Literal(usize::MAX),
            });

        assert!(mutable_type_representation(&program, fixed_array).is_none());
        assert!(repeat_representation(&element, usize::MAX).is_none());
        assert!(repeat_representation_with_stride(&element, usize::MAX, 1).is_none());
    }
}
