//! Exact constructor operands in authored order, independently of their consumer.
//!
//! This is a source classifier, not establishment evidence. Checking still owes
//! default domains and ownership; lowering must evaluate the returned operands
//! before constructing the selected nominal case. Only the existing closed,
//! fully authored scalar-payload sum route is described here.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub struct ScalarCaseConstructor {
    pub type_reference: TypeReferenceHandle,
    pub case: SymbolHandle,
    pub fields: Vec<(SymbolHandle, ExpressionHandle, PrimitiveType)>,
}

/// A case literal whose payload fields hold independently typed operands:
/// each is a scalar computation or a nested structural value, mirroring record
/// field composition. The selected case still supplies the discriminated tag.
pub struct StructuralCaseConstructor {
    pub type_reference: TypeReferenceHandle,
    pub case: SymbolHandle,
    pub fields: Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle)>,
}

/// Classify value sequencing before permission checking. A named source is not
/// evidence of an available owner; selected transfers require checked receipts.
pub fn is_scalar_case_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> bool {
    is_case_value(
        program,
        expression,
        reference,
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Match(_)
        ),
    )
}

fn is_case_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    allow_existing: bool,
) -> bool {
    // This value route has no numeric field-establishment obligations yet.
    // Keep constrained constructors with the proof-bearing return producer;
    // selecting this plan must not displace a supported bounded-field return.
    if !crate::has_plain_owned_contents(program, reference) {
        return false;
    }
    let expected = program.normalized_type_identity(reference);
    let mut pending = vec![(expression, 0_usize)];
    while let Some((expression, depth)) = pending.pop() {
        if depth >= program.expression_table.expression_count() {
            return false;
        }
        if let Some(constructor) = scalar_case_constructor(program, expression) {
            if program.normalized_type_identity(constructor.type_reference) != expected {
                return false;
            }
            continue;
        }
        if allow_existing && scalar_case_value_source(program, expression, reference).is_some() {
            continue;
        }
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
        else {
            return false;
        };
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() {
            return false;
        }
        pending.extend(arms.iter().map(|arm| (arm.value, depth + 1)));
    }
    true
}

/// Resolve a whole local/parameter occurrence without erasing a borrow or
/// qualification. The caller independently checks its scope and live custody.
pub fn scalar_case_value_source(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let source = crate::plain_owned_value_source(program, expression, reference)?;
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let owner =
        crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
    let members = program.data_members(owner);
    if members.is_empty()
        || members.iter().any(|member| match member {
            DataMember::Field(_) => true,
            DataMember::Variant(case) => program.data_payload_fields(case).iter().any(|field| {
                program
                    .primitive_type_reference(field.type_reference)
                    .is_none()
            }),
        })
    {
        return None;
    }
    Some(source)
}

/// One named case of a closed non-generic sum with every declared payload
/// field authored exactly once. The resolver reports each field's declared
/// type; the payload route — scalar operands or structural field values —
/// decides what each caller may do with it.
/// The `(formal parameter symbol, argument reference)` pairs an instantiated
/// `reference` supplies for `owner`'s type parameters — `None` when the
/// reference does not name `owner` or its argument count disagrees.
pub fn member_type_substitutions(
    program: &TypedTrees,
    owner: &DataDefinition,
    reference: TypeReferenceHandle,
) -> Option<Vec<(SymbolHandle, TypeReferenceHandle)>> {
    let arguments = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } if *symbol == owner.symbol => Vec::new(),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if *base_symbol == owner.symbol => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .to_vec(),
        _ => return None,
    };
    let parameters = program.data_type_parameters(owner);
    (parameters.len() == arguments.len()).then(|| {
        parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .zip(arguments)
            .collect()
    })
}

/// `declared` with the owner's instantiated parameters answered by their
/// argument references. A bare parameter name swaps to its argument handle;
/// arguments nested deeper in the declared type stay in declared form, which
/// the checked route declines downstream.
pub fn substituted_member_reference(
    program: &TypedTrees,
    declared: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(declared)
        && let Some((_, actual)) = substitutions.iter().find(|(formal, _)| formal == symbol)
    {
        return *actual;
    }
    declared
}

/// The constructor parts a case literal carries, in authored order. `expected`
/// is the construction's destination type: when it instantiates the owner's
/// type parameters the returned `type_reference` is that instantiation and
/// every returned `declared` type is answered under its substitutions. Without
/// it the owner must be a closed data definition, matching the previous
/// bare-named lookup.
fn case_constructor_parts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: Option<TypeReferenceHandle>,
    erased_member_owner: bool,
) -> Option<(
    TypeReferenceHandle,
    SymbolHandle,
    Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle, bool)>,
)> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let (owner, selected, fields) = match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => (
            crate::program_scope::data_definitions::data_definition_by_symbol(
                program,
                literal.type_symbol,
            )?,
            literal.case_symbol?,
            program.expression_table.struct_fields(literal.fields),
        ),
        ExpressionNode::Name(name) => (
            crate::exact_case_reference_owner(program, expression)?,
            name.symbol,
            &[][..],
        ),
        _ => return None,
    };
    let (reference, substitutions) = match expected {
        Some(expected) => {
            let reference = crate::unwrapped_type_reference(program, expected)?;
            let substitutions = member_type_substitutions(program, owner, reference)?;
            (reference, substitutions)
        }
        None => {
            if !owner.type_parameters.is_empty() {
                return None;
            }
            (
                program
                    .type_reference_table
                    .find_named_type_reference(owner.symbol)?,
                Vec::new(),
            )
        }
    };
    // A scalar construction needs numeric-owned contents throughout; the
    // structural route may stand a union whose only non-variant members are
    // `[erased]` -- cleanup-owned contents admit them because they never
    // produce runtime storage (storage_contents.rs).
    let contents = if erased_member_owner {
        crate::has_structural_construction_contents(program, reference)
    } else {
        crate::has_plain_owned_contents_with_numeric_constraints(program, reference)
    };
    // The structural route also stands a mixed shape whose runtime common
    // fields are scalar primitives, the only common fields
    // `data::validate_data_shape` admits. Case construction zero-initializes
    // every common field the literal leaves unnamed, so a named one is an
    // operand beside the payload and an unnamed one is the lowering's zero.
    let common_scalar = |field: &typed_trees::data::DataField| {
        erased_member_owner
            && (field.relevance.is_erased()
                || program
                    .primitive_type_reference(field.type_reference)
                    .is_some())
    };
    if !contents
        || program
            .data_members(owner)
            .iter()
            .any(|member| match member {
                DataMember::Field(field) => !common_scalar(field),
                DataMember::Variant(_) => false,
            })
        || program.symbols.get(selected).parent != owner.symbol
    {
        return None;
    }
    let case = program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(case) if case.symbol == selected => Some(case),
            _ => None,
        })?;
    let declarations = program.data_payload_fields(case);
    let mut operands: Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle, bool)> =
        Vec::with_capacity(fields.len());
    for field in fields {
        // An erased owner member authored beside the case is declared-level
        // content: it never produces runtime storage, so it is consumed
        // rather than matched against the case's payload declarations.
        let erased_owner_member = erased_member_owner
            && program.data_members(owner).iter().any(|member| {
                matches!(member, DataMember::Field(owner_field)
                    if owner_field.symbol == field.field_symbol
                        && owner_field.relevance.is_erased())
            });
        // An erased payload member likewise holds no runtime slot: an authored
        // operand is proof material the construction consumes, never a
        // runtime field (proofs/contracts.md, explicit erased bindings).
        let erased_payload_member = erased_member_owner
            && declarations.iter().any(|declaration| {
                declaration.symbol == field.field_symbol && declaration.relevance.is_erased()
            });
        if erased_owner_member || erased_payload_member {
            if !program.expression_table.expression_is_valid(field.value) {
                return None;
            }
            continue;
        }
        // A runtime common field of a mixed owner is named at most once and
        // stays out of the payload count; admission above limited it to the
        // structural route and scalar carriers.
        let common = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(owner_field) if owner_field.symbol == field.field_symbol => {
                    Some(owner_field)
                }
                _ => None,
            });
        let declared = substituted_member_reference(
            program,
            match common {
                Some(common) => common.type_reference,
                None => {
                    declarations
                        .iter()
                        .find(|declaration| declaration.symbol == field.field_symbol)?
                        .type_reference
                }
            },
            &substitutions,
        );
        if !field.field_symbol.is_valid()
            || operands
                .iter()
                .any(|(symbol, ..)| *symbol == field.field_symbol)
            || !program.expression_table.expression_is_valid(field.value)
        {
            return None;
        }
        let erased = match common {
            Some(common) => common.relevance.is_erased(),
            None => declarations.iter().any(|declaration| {
                declaration.symbol == field.field_symbol && declaration.relevance.is_erased()
            }),
        };
        operands.push((field.field_symbol, field.value, declared, erased));
    }
    // The runtime payload declarations keep their authored-exactly-once shape:
    // every one answered one operand. Erased owner and payload members and
    // named common fields never enter the count; an omitted erased payload
    // member is determined by its nullary constructor, which checking proved.
    let payload_operands = operands
        .iter()
        .filter(|(symbol, ..)| {
            declarations
                .iter()
                .any(|declaration| declaration.symbol == *symbol)
        })
        .count();
    let runtime_payload = declarations
        .iter()
        .filter(|declaration| !(erased_member_owner && declaration.relevance.is_erased()))
        .count();
    (payload_operands == runtime_payload).then_some((reference, selected, operands))
}

pub fn scalar_case_constructor(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ScalarCaseConstructor> {
    let (reference, selected, operands) = case_constructor_parts(program, expression, None, false)?;
    let mut fields = Vec::with_capacity(operands.len());
    for (symbol, value, declared, ..) in operands {
        fields.push((symbol, value, program.primitive_type_reference(declared)?));
    }
    Some(ScalarCaseConstructor {
        type_reference: reference,
        case: selected,
        fields,
    })
}

/// The same closed-constructor recognition without the scalar-payload
/// constraint. Erased members keep semantic content but no runtime storage, so
/// a runtime construction cannot establish them; declaration-level `[erased]`
/// members are admitted instead -- constructing a case writes its tag and
/// runtime fields, the erased members never materialize.
pub fn structural_case_constructor(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> Option<StructuralCaseConstructor> {
    let (reference, selected, operands) =
        case_constructor_parts(program, expression, Some(expected), true)?;
    let mut fields = Vec::with_capacity(operands.len());
    for (symbol, value, declared, erased) in operands {
        if erased {
            return None;
        }
        fields.push((symbol, value, declared));
    }
    Some(StructuralCaseConstructor {
        type_reference: reference,
        case: selected,
        fields,
    })
}
