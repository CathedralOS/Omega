//! Exact constructor operands in authored order, independently of their consumer.
//!
//! This is a source classifier, not establishment evidence. Checking still owes
//! default domains and ownership; lowering must evaluate the returned operands
//! before constructing the selected nominal case. Scalar and structural
//! operands share the same exact case declaration and operand ordering.

use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

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
    /// Missing runtime operands, in declaration order. This roster is not
    /// permission to initialize them: checking proves zero validity and the
    /// structural producer must materialize each declared carrier.
    pub omitted_fields: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

/// Classify value sequencing before permission checking. A named source is not
/// evidence of an available owner; selected transfers require checked receipts.
pub fn is_scalar_case_value(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    allow_existing: bool,
) -> bool {
    // This value route has no numeric field-establishment obligations yet.
    // Keep constrained constructors with the proof-bearing return producer;
    // selecting this plan must not displace a supported bounded-field return.
    if !crate::has_contents(
        program,
        reference,
        &[],
        crate::value_custody::storage_contents::ContentsRequirement::PlainOwned,
    ) {
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
        let Some(dispatch) = program.expression_table.match_node(expression) else {
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let source = crate::plain_owned_value_source(program, expression, reference)?;
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let owner = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
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

/// The constructor parts a case literal carries, in authored order. `expected`
/// selects the already prepared closed declaration, including nested type
/// applications and regions. Authored selectors may still belong to its
/// template; declaration positions and identities translate those selectors,
/// never their types. Checking does not instantiate or substitute members.
fn case_constructor_parts(
    program: &dyn crate::CheckingInput,
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
    let (authored_owner, selected, fields) = match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) => (
            crate::declarations::data_queries::data_definition_by_symbol(
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
    let reference = match expected {
        Some(expected) => crate::unwrapped_type_reference(program, expected)?,
        None => {
            if !authored_owner.type_parameters.is_empty() {
                return None;
            }
            program
                .type_reference_table
                .find_named_type_reference(authored_owner.symbol)?
        }
    };
    let owner = program.closed_data_constructor_definition(reference, authored_owner.symbol)?;
    let (case_ordinal, authored_case) = program
        .data_members(authored_owner)
        .iter()
        .enumerate()
        .find_map(|(ordinal, member)| match member {
            DataMember::Variant(case) if case.symbol == selected => Some((ordinal, case)),
            _ => None,
        })?;
    let DataMember::Variant(case) = program.data_members(owner).get(case_ordinal)? else {
        return None;
    };
    if case.name != authored_case.name || case.identity != authored_case.identity {
        return None;
    }
    // Structural construction composes completed operands, just like a record;
    // it does not require the whole carrier to be copyable or borrow-free.
    // An exclusive descriptor keeps its own formation and transfer obligations.
    // This classifier only pairs exact declarations with authored operands;
    // structural producers establish each field and omitted zero operand.
    // The scalar-only interface still requires numeric-owned payloads and has
    // no vocabulary for common fields or structural children.
    if (!erased_member_owner
        && (!crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        ) || program
            .data_members(owner)
            .iter()
            .any(|member| matches!(member, DataMember::Field(_)))))
        || program.symbols.get(selected).parent != authored_owner.symbol
    {
        return None;
    }
    let declarations = program.data_payload_fields(case);
    let mut operands: Vec<(SymbolHandle, ExpressionHandle, TypeReferenceHandle, bool)> =
        Vec::with_capacity(fields.len());
    for field in fields {
        let declaration = program
            .data_members(authored_owner)
            .iter()
            .enumerate()
            .find_map(|(ordinal, member)| match member {
                DataMember::Field(original) if original.symbol == field.field_symbol => {
                    let DataMember::Field(closed) = program.data_members(owner).get(ordinal)?
                    else {
                        return None;
                    };
                    Some((original, closed))
                }
                _ => None,
            })
            .or_else(|| {
                program
                    .data_payload_fields(authored_case)
                    .iter()
                    .enumerate()
                    .find_map(|(ordinal, original)| {
                        (original.symbol == field.field_symbol)
                            .then(|| declarations.get(ordinal).map(|closed| (original, closed)))
                            .flatten()
                    })
            })?;
        let (original, declared) = declaration;
        if original.name != declared.name
            || original.identity != declared.identity
            || original.relevance != declared.relevance
        {
            return None;
        }
        // Both common and payload erased members carry proof material, not
        // runtime storage. The prepared declaration owns that distinction.
        if erased_member_owner && declared.relevance.is_erased() {
            if !program.expression_table.expression_is_valid(field.value) {
                return None;
            }
            continue;
        }
        if !field.field_symbol.is_valid()
            || operands
                .iter()
                .any(|(symbol, ..)| *symbol == declared.symbol)
            || !program.expression_table.expression_is_valid(field.value)
        {
            return None;
        }
        operands.push((
            declared.symbol,
            field.value,
            declared.type_reference,
            declared.relevance.is_erased(),
        ));
    }
    // The scalar-only interface requires an authored operand for every runtime
    // payload. Structural construction instead carries an explicit omitted
    // roster to its zero producer. Erased members never enter the runtime count.
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
    (erased_member_owner || payload_operands == runtime_payload).then_some((
        reference,
        case.symbol,
        operands,
    ))
}

pub fn scalar_case_constructor(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    let owner = program.closed_data_definition(reference)?;
    let omitted_fields = program
        .data_members(owner)
        .iter()
        .flat_map(|member| match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(case) if case.symbol == selected => {
                program.data_payload_fields(case)
            }
            DataMember::Variant(_) => &[],
        })
        .filter(|field| !field.relevance.is_erased())
        .filter(|field| fields.iter().all(|(symbol, ..)| *symbol != field.symbol))
        .map(|field| (field.symbol, field.type_reference))
        .collect();
    Some(StructuralCaseConstructor {
        type_reference: reference,
        case: selected,
        fields,
        omitted_fields,
    })
}
