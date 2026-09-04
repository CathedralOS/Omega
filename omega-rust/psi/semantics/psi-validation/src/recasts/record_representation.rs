use super::scalar_representation::{
    MutableScalarRepresentationFacts, mutable_scalar_representation_facts,
    mutable_scalar_representation_facts_equivalent, scalar_representation_facts_imply,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use std::collections::HashSet;

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
        super::direct_phantom_lifetime_record_symbol(program, type_reference);
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
    symbol: psi_symbols::SymbolHandle,
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
    symbol: psi_symbols::SymbolHandle,
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
                && budget.lifetime_shell_depth < super::MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH =>
        {
            let symbol = super::phantom_lifetime_record_symbol_shape(program, type_reference)?;
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
mod tests {
    use super::*;
    use psi_symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols};
    use psi_typed_trees::data::{
        DataDefinition, DataField, DataMember, TypeParameter, TypeParameterKind,
    };
    use psi_typed_trees::name::Identifier;

    fn program_with_builtins() -> TypedTrees {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        builder.insert_children(root, builtin_type_symbols());
        TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        }
    }

    fn generated_data_symbol(program: &mut TypedTrees, name: &str) -> psi_symbols::SymbolHandle {
        program
            .symbols
            .insert_generated_root_from(program.symbols.root(), SymbolKind::Data, name)
    }

    struct PhantomFixture {
        program: TypedTrees,
        shell: TypeReferenceHandle,
        origin: TypeReferenceHandle,
        base_symbol: psi_symbols::SymbolHandle,
        instance_symbol: psi_symbols::SymbolHandle,
        u8_type: TypeReferenceHandle,
    }

    fn phantom_fixture() -> PhantomFixture {
        let mut program = program_with_builtins();
        let u8_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u8")
            .expect("u8 builtin");
        let u32_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u32")
            .expect("u32 builtin");
        let u8_type = named_type(&mut program, u8_symbol, "u8");
        let u32_type = named_type(&mut program, u32_symbol, "u32");
        let base_symbol = generated_data_symbol(&mut program, "Phantom");
        let instance_symbol = generated_data_symbol(&mut program, "PhantomU32");
        let lifetime_parameters = vec![Identifier::generated("region")];

        let mut base = DataDefinition {
            symbol: base_symbol,
            name: Identifier::generated("Phantom"),
            lifetime_parameters: lifetime_parameters.clone(),
            ..DataDefinition::default()
        };
        program.push_data_type_parameter(
            &mut base,
            TypeParameter {
                name: Identifier::generated("T"),
                kind: TypeParameterKind::Type,
                ..TypeParameter::default()
            },
        );
        program.push_data_member(
            &mut base,
            DataMember::Field(DataField {
                type_reference: u8_type,
                ..DataField::default()
            }),
        );
        program.push_data_definition(base);

        let origin_arguments = program
            .type_reference_table
            .insert_type_reference_handles([u32_type]);
        let origin = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol,
                base_name: Identifier::generated("Phantom"),
                lifetime_arguments: Vec::new(),
                arguments: origin_arguments,
            });
        let mut instance = DataDefinition {
            symbol: instance_symbol,
            name: Identifier::generated("Phantom<u32>"),
            lifetime_parameters,
            generic_instance: Some(origin),
            ..DataDefinition::default()
        };
        program.push_data_member(
            &mut instance,
            DataMember::Field(DataField {
                type_reference: u8_type,
                ..DataField::default()
            }),
        );
        program.push_data_definition(instance);

        let shell = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: instance_symbol,
                base_name: Identifier::generated("Phantom<u32>"),
                lifetime_arguments: vec![Identifier::generated("call")],
                arguments: Default::default(),
            });
        PhantomFixture {
            program,
            shell,
            origin,
            base_symbol,
            instance_symbol,
            u8_type,
        }
    }

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
        let mut program = program_with_builtins();
        let first_symbol = generated_data_symbol(&mut program, "FirstCell");
        let selected_symbol = generated_data_symbol(&mut program, "SelectedCell");
        let u64_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u64")
            .expect("u64 builtin");
        let u8_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u8")
            .expect("u8 builtin");
        let u64_type = named_type(&mut program, u64_symbol, "u64");
        let u8_type = named_type(&mut program, u8_symbol, "u8");
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
        let mut program = program_with_builtins();
        let u8_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u8")
            .expect("u8 builtin");
        let u8_type = named_type(&mut program, u8_symbol, "u8");
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

    #[test]
    fn primitive_spelling_cannot_replace_exact_builtin_identity() {
        let mut program = program_with_builtins();
        let record_symbol = generated_data_symbol(&mut program, "Pretender");
        let u64_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u64")
            .expect("u64 builtin");
        let u64_type = named_type(&mut program, u64_symbol, "u64");
        push_single_field_record(&mut program, record_symbol, u64_type);
        let forged_display = named_type(&mut program, record_symbol, "u8");

        let representation = mutable_type_representation(&program, forged_display)
            .expect("the exact Data symbol resolves as its record, not a displayed primitive");
        assert_eq!(representation.size, 8);
        assert_eq!(representation.align, 8);
    }

    #[test]
    fn aggregate_representation_fails_closed_before_deep_host_recursion() {
        let mut program = program_with_builtins();
        let u8_symbol = program
            .symbols
            .find_child_by_name(program.symbols.root(), "u8")
            .expect("u8 builtin");
        let mut field_type = named_type(&mut program, u8_symbol, "u8");
        let mut root_type = field_type;
        for depth in 0..=MAX_RECAST_REPRESENTATION_DEPTH {
            let symbol = generated_data_symbol(&mut program, &format!("Deep{depth}"));
            push_single_field_record(&mut program, symbol, field_type);
            root_type = named_type(&mut program, symbol, "Deep");
            field_type = root_type;
        }

        assert!(mutable_type_representation(&program, root_type).is_none());
    }

    #[test]
    fn aggregate_representation_rejects_exact_symbol_cycles() {
        let mut program = program_with_builtins();
        let left = generated_data_symbol(&mut program, "Left");
        let right = generated_data_symbol(&mut program, "Right");
        let left_type = named_type(&mut program, left, "Left");
        let right_type = named_type(&mut program, right, "Right");
        push_single_field_record(&mut program, left, right_type);
        push_single_field_record(&mut program, right, left_type);

        assert!(mutable_type_representation(&program, left_type).is_none());
    }

    #[test]
    fn phantom_lifetime_shell_requires_exact_arity_and_runtime_free_origin() {
        let fixture = phantom_fixture();
        assert_eq!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,),
            Some(fixture.instance_symbol)
        );
        assert_eq!(
            super::super::literal_indexed_recast_target_size(&fixture.program, fixture.shell,),
            Some(1)
        );

        for lifetimes in [
            Vec::new(),
            vec![
                Identifier::generated("call"),
                Identifier::generated("other"),
            ],
        ] {
            let mut fixture = phantom_fixture();
            fixture.program.type_reference_table.substitute_node(
                fixture.shell,
                TypeReferenceNode::Generic {
                    base_symbol: fixture.instance_symbol,
                    base_name: Identifier::generated("Phantom<u32>"),
                    lifetime_arguments: lifetimes,
                    arguments: Default::default(),
                },
            );
            assert!(
                super::super::direct_phantom_lifetime_record_symbol(
                    &fixture.program,
                    fixture.shell,
                )
                .is_none()
            );
        }

        let mut fixture = phantom_fixture();
        let runtime_arguments = fixture
            .program
            .type_reference_table
            .insert_type_reference_handles([fixture.u8_type]);
        fixture.program.type_reference_table.substitute_node(
            fixture.shell,
            TypeReferenceNode::Generic {
                base_symbol: fixture.instance_symbol,
                base_name: Identifier::generated("Phantom<u32>"),
                lifetime_arguments: vec![Identifier::generated("call")],
                arguments: runtime_arguments,
            },
        );
        assert!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,)
                .is_none()
        );
    }

    #[test]
    fn array_fence_survives_an_ordinary_named_wrapper() {
        let mut fixture = phantom_fixture();
        let wrapper_symbol = generated_data_symbol(&mut fixture.program, "Wrapper");
        push_single_field_record(&mut fixture.program, wrapper_symbol, fixture.shell);
        let wrapper_type = named_type(&mut fixture.program, wrapper_symbol, "Wrapper");
        let array_type =
            fixture
                .program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: wrapper_type,
                    length: FixedArrayLength::Literal(1),
                });
        let mut visiting = HashSet::new();
        let mut budget = RepresentationBudget {
            depth: 0,
            work: 0,
            lifetime_shell_depth: 1,
        };

        assert!(
            mutable_record_type_representation(
                &fixture.program,
                array_type,
                &mut visiting,
                &mut budget,
                true,
                true,
            )
            .is_none(),
            "array descent must not let a named wrapper re-enable lifetime shells"
        );
    }

    #[test]
    fn phantom_lifetime_shell_uses_symbols_not_display_names() {
        let mut fixture = phantom_fixture();
        fixture.program.type_reference_table.substitute_node(
            fixture.shell,
            TypeReferenceNode::Generic {
                base_symbol: fixture.instance_symbol,
                base_name: Identifier::generated("Decoy"),
                lifetime_arguments: vec![Identifier::generated("call")],
                arguments: Default::default(),
            },
        );
        assert_eq!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,),
            Some(fixture.instance_symbol)
        );

        let decoy = generated_data_symbol(&mut fixture.program, "Decoy");
        fixture.program.push_data_definition(DataDefinition {
            symbol: decoy,
            name: Identifier::generated("Phantom<u32>"),
            ..DataDefinition::default()
        });
        fixture.program.type_reference_table.substitute_node(
            fixture.shell,
            TypeReferenceNode::Generic {
                base_symbol: decoy,
                base_name: Identifier::generated("Phantom<u32>"),
                lifetime_arguments: vec![Identifier::generated("call")],
                arguments: Default::default(),
            },
        );
        assert!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,)
                .is_none()
        );
    }

    #[test]
    fn phantom_lifetime_shell_rejects_malformed_origin_cycle_and_zero_size() {
        let mut fixture = phantom_fixture();
        fixture.program.type_reference_table.substitute_node(
            fixture.origin,
            TypeReferenceNode::Named {
                symbol: fixture.base_symbol,
                name: Identifier::generated("Phantom"),
            },
        );
        assert!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,)
                .is_none()
        );

        let mut fixture = phantom_fixture();
        let instance_handle = fixture
            .program
            .tables
            .data_definitions
            .iter()
            .find_map(|(handle, data)| (data.symbol == fixture.instance_symbol).then_some(handle))
            .expect("instance definition");
        let members = fixture
            .program
            .tables
            .data_definitions
            .get(instance_handle)
            .members;
        let instance_type = named_type(
            &mut fixture.program,
            fixture.instance_symbol,
            "Phantom<u32>",
        );
        let DataMember::Field(field) = &mut fixture
            .program
            .tables
            .data_members
            .span_mut_or_empty(members)[0]
        else {
            panic!("instance field")
        };
        field.type_reference = instance_type;
        assert!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,)
                .is_none()
        );

        let mut fixture = phantom_fixture();
        let zero_array =
            fixture
                .program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: fixture.u8_type,
                    length: FixedArrayLength::Literal(0),
                });
        let instance_handle = fixture
            .program
            .tables
            .data_definitions
            .iter()
            .find_map(|(handle, data)| (data.symbol == fixture.instance_symbol).then_some(handle))
            .expect("instance definition");
        let members = fixture
            .program
            .tables
            .data_definitions
            .get(instance_handle)
            .members;
        let DataMember::Field(field) = &mut fixture
            .program
            .tables
            .data_members
            .span_mut_or_empty(members)[0]
        else {
            panic!("instance field")
        };
        field.type_reference = zero_array;
        assert_eq!(
            super::super::direct_phantom_lifetime_record_symbol(&fixture.program, fixture.shell,),
            Some(fixture.instance_symbol)
        );
        assert!(shared_projection_type_representation(&fixture.program, fixture.shell).is_none());
        assert!(
            super::super::literal_indexed_recast_target_size(&fixture.program, fixture.shell,)
                .is_none()
        );
    }
}
