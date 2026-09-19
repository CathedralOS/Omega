//! Standing `data ... where` facts over the root-installed machine receiver
//! become machine-entry runtime requirements.
//!
//! A zero-satisfying data carrier satisfies its authored default-domain facts
//! at its ZII value, and the write net is total: every write reaching a legal
//! observation re-proves the carrier's facts. Publishing those facts as entry
//! `requires` rows is therefore sound exactly where the incoming value is the
//! machine's own root-installed storage -- the unit entry `main`. A callee
//! would instead force each caller to discharge the same row at the call
//! site; callers cannot re-discharge a fact their own storage has since
//! overwritten, so non-entry machines keep the facts as write obligations
//! only.
//!
//! A `zero_gated` definition's ZII value violates its domain, so its own
//! facts are withheld (the access gate owns those reads); descent still
//! continues beneath it because a nested zero-satisfying carrier's facts hold
//! independently. Anything outside the emittable fragment -- operator
//! spellings, arithmetic, case paths, indexed reads, borrowed fields -- is
//! withheld rather than weakening the fact's enforcement elsewhere.

use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedOperatorFacts,
    CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// The `data where` facts holding at machine entry over `self`, lowered into
/// the structural runtime-requirement vocabulary. Facts the bounded terminal
/// requirement namespace cannot carry are skipped one at a time; the package
/// itself never fails closed on them.
pub(crate) fn machine_entry_where_requirements(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &CheckedOperatorFacts,
) -> Vec<CheckedBooleanExpression> {
    // Only the root-installed receiver is guaranteed to arrive as machine
    // storage: the unit entry (`main`, qualified `Owner::main` when attached)
    // is caller-free by construction.
    let name = machine.name.as_str();
    if name != "main" && !name.ends_with("::main") {
        return Vec::new();
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    // A scalar-result machine replays its requirement package positionally
    // against authored scalar sources; keep the package source-shaped there.
    if !matches!(
        program
            .type_reference_table
            .type_reference(entry.return_type),
        TypeReferenceNode::Unit
    ) {
        return Vec::new();
    }
    let parameters = program.state_parameters(entry);
    let mut receivers = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self);
    let Some((position, parameter)) = receivers.next() else {
        return Vec::new();
    };
    if receivers.next().is_some() || parameter.is_const {
        return Vec::new();
    }
    let Ok(parameter_position) = u32::try_from(position) else {
        return Vec::new();
    };
    let mut owners = program
        .data_definitions()
        .iter()
        .filter(|owner| owner.symbol == machine.attached_data_symbol);
    let Some(owner) = owners.next() else {
        return Vec::new();
    };
    if owners.next().is_some()
        || machine.attached_data.as_ref() != Some(&owner.name)
        || program.symbols.get(owner.symbol).kind != symbols::SymbolKind::Data
    {
        return Vec::new();
    }
    // The receiver's own type must name the attached declaration (the same
    // `Self`/owner nominal shapes `exact_self_parameter` accepts); any other
    // carrier keeps its facts out of this contract.
    let mut reference = parameter.type_reference;
    let mut receiver_names_owner = false;
    for _ in 0..64 {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            break;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, name }
                if (*symbol == machine.symbol && name.as_str() == "Self")
                    || (*symbol == owner.symbol && *name == owner.name) =>
            {
                receiver_names_owner = true;
                break;
            }
            _ => break,
        }
    }
    if !receiver_names_owner {
        return Vec::new();
    }
    let mut requirements = Vec::new();
    let mut visited = vec![owner.symbol];
    append_where_requirements(
        program,
        operators,
        parameter_position,
        &mut Vec::new(),
        owner,
        &mut visited,
        &mut requirements,
    );
    requirements
}

/// Emit the definition's own facts at `prefix`, then descend through owned
/// data-typed fields. `visited` is the data-definition stack on the current
/// path: the cycle guard keeps a self-referential nominal carrier from
/// looping.
fn append_where_requirements(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameter_position: u32,
    prefix: &mut Vec<CheckedStructuralPredicatePathSegment>,
    definition: &DataDefinition,
    visited: &mut Vec<SymbolHandle>,
    requirements: &mut Vec<CheckedBooleanExpression>,
) {
    // A gated definition's own facts are withheld: its ZII storage does not
    // satisfy them, so seeding them at entry would assume false premises.
    if !definition.zero_gated {
        for fact in program.proof_facts.span_or_empty(definition.where_facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(requirement) = where_boolean(
                program,
                operators,
                parameter_position,
                prefix,
                definition,
                *expression,
            ) {
                requirements.push(requirement);
            }
        }
    }
    for member in program.data_members(definition) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if !field.symbol.is_valid() || field.relevance.is_erased() {
            continue;
        }
        let Some(nested) = owned_field_data_definition(program, field.type_reference) else {
            continue;
        };
        if visited.contains(&nested.symbol) {
            continue;
        }
        visited.push(nested.symbol);
        prefix.push(CheckedStructuralPredicatePathSegment::Field(
            field_identity(field),
        ));
        append_where_requirements(
            program,
            operators,
            parameter_position,
            prefix,
            nested,
            visited,
            requirements,
        );
        prefix.pop();
        visited.pop();
    }
}

/// The nominal data definition a field OWNS inline: `Constrained` wrappers
/// peel, but a `&T`/`&mut T` field is borrowed storage whose referee's facts
/// do not ride on this machine's storage.
fn owned_field_data_definition<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            owned_field_data_definition(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *symbol),
        _ => None,
    }
}

/// The canonical structural identity a field's path segment carries: its
/// ordinal `#n` when one was assigned, else the authored spelling. This is
/// the same spelling `resolve_structural_parameter_path` matches.
fn field_identity(field: &DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}

/// One data definition's field, selected by its retained symbol or falling
/// back to the authored spelling when no symbol was retained. An erased
/// field has no scalar storage in the Terminal record, so a fact leaf or
/// intermediate segment naming it cannot become a structural requirement
/// path; treat it as unresolvable rather than emitting a requirement the
/// lowering would reject.
fn find_data_field<'program>(
    program: &'program TypedTrees,
    definition: &DataDefinition,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program DataField> {
    program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        if field.relevance.is_erased() {
            return None;
        }
        if symbol.is_valid() {
            (field.symbol == symbol).then_some(field)
        } else {
            (field.name.as_str() == name).then_some(field)
        }
    })
}

/// Resolve a where-fact leaf (`value`, `inner.value`, `a.b`) to its field
/// path below `prefix` plus the leaf's declared type. Only plain record-field
/// chains lower: case payloads, indexed reads, builtin `len`/`capacity`
/// receivers, and borrowed fields have no structural-requirement spelling.
fn where_field_path(
    program: &TypedTrees,
    prefix: &[CheckedStructuralPredicatePathSegment],
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<(
    Vec<CheckedStructuralPredicatePathSegment>,
    TypeReferenceHandle,
)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let members = program.expression_table.name_path_members(name.members);
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(name.member_symbols);
            let mut path = prefix.to_vec();
            let mut owner = definition;
            let mut leaf = None;
            for (index, member) in members.iter().enumerate() {
                let symbol = member_symbols
                    .get(index)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                let field = find_data_field(program, owner, symbol, member.as_str())?;
                path.push(CheckedStructuralPredicatePathSegment::Field(
                    field_identity(field),
                ));
                if index + 1 == members.len() {
                    leaf = Some(field.type_reference);
                } else {
                    owner = owned_field_data_definition(program, field.type_reference)?;
                }
            }
            Some((path, leaf?))
        }
        ExpressionNode::Member(member) => {
            if member.case_variant.is_some() {
                return None;
            }
            let (mut path, receiver_type) =
                where_field_path(program, prefix, definition, member.receiver)?;
            let owner = owned_field_data_definition(program, receiver_type)?;
            let field =
                find_data_field(program, owner, member.member_symbol, member.member.as_str())?;
            path.push(CheckedStructuralPredicatePathSegment::Field(
                field_identity(field),
            ));
            Some((path, field.type_reference))
        }
        ExpressionNode::Borrow(borrow) => {
            where_field_path(program, prefix, definition, borrow.target)
        }
        _ => None,
    }
}

/// An integer operand in the bounded requirement vocabulary: a fixed integer
/// parameter field below the receiver, or a literal. The returned primitive
/// is the operand's resolved carrier; a literal whose landing is still open
/// contributes `None` so the comparison's other side can land it.
fn where_scalar(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameter_position: u32,
    prefix: &[CheckedStructuralPredicatePathSegment],
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<(CheckedScalarExpression, Option<PrimitiveType>)> {
    if operators.expression_use(expression).is_some() {
        return None;
    }
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        let primitive = literal
            .landing()
            .and_then(|landing| match landing.landed_type {
                numerics::literals::LandedIntegerType::I8 => Some(PrimitiveType::I8),
                numerics::literals::LandedIntegerType::I16 => Some(PrimitiveType::I16),
                numerics::literals::LandedIntegerType::I32 => Some(PrimitiveType::I32),
                numerics::literals::LandedIntegerType::I64 => Some(PrimitiveType::I64),
                numerics::literals::LandedIntegerType::U8 => Some(PrimitiveType::U8),
                numerics::literals::LandedIntegerType::U16 => Some(PrimitiveType::U16),
                numerics::literals::LandedIntegerType::U32 => Some(PrimitiveType::U32),
                numerics::literals::LandedIntegerType::U64 => Some(PrimitiveType::U64),
                numerics::literals::LandedIntegerType::Addr => None,
            });
        return Some((
            CheckedScalarExpression::IntegerLiteral {
                literal: literal.clone(),
            },
            primitive,
        ));
    }
    let (path, leaf_type) = where_field_path(program, prefix, definition, expression)?;
    let primitive_type = program.primitive_type_reference(leaf_type)?;
    match primitive_type {
        PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64 => Some((
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            },
            Some(primitive_type),
        )),
        _ => None,
    }
}

/// Land an unlanded literal at the comparison's common carrier, reusing the
/// shared retag so an out-of-range literal refuses rather than truncating.
fn land_literal_operand(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    match &expression {
        CheckedScalarExpression::IntegerLiteral { literal } if literal.landing().is_none() => {
            crate::values::retag_exact_integer_literal(&expression, primitive_type)
        }
        _ => Some(expression),
    }
}

/// One comparison side pair: both operands must share a single integer
/// carrier, with the field side (when present) fixing it.
fn where_integer_comparison(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameter_position: u32,
    prefix: &[CheckedStructuralPredicatePathSegment],
    definition: &DataDefinition,
    kind: CheckedIntegerComparisonKind,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let (left, left_type) = where_scalar(
        program,
        operators,
        parameter_position,
        prefix,
        definition,
        left,
    )?;
    let (right, right_type) = where_scalar(
        program,
        operators,
        parameter_position,
        prefix,
        definition,
        right,
    )?;
    let operand_type = match (left_type, right_type) {
        (Some(left_type), Some(right_type)) if left_type == right_type => left_type,
        (Some(operand_type), None) | (None, Some(operand_type)) => operand_type,
        _ => return None,
    };
    Some(CheckedBooleanExpression::IntegerComparison {
        kind,
        left: Box::new(land_literal_operand(left, operand_type)?),
        right: Box::new(land_literal_operand(right, operand_type)?),
    })
}

/// Lower one authored default-domain fact expression into the bounded
/// structural runtime-requirement vocabulary over the receiver's field
/// namespace. The fact's `Name`/`Member` leaves name fields of `definition`;
/// `prefix` already carries the path from `self` to `definition`'s storage.
fn where_boolean(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameter_position: u32,
    prefix: &[CheckedStructuralPredicatePathSegment],
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    if operators.expression_use(expression).is_some() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            Some(CheckedBooleanExpression::Not(Box::new(where_boolean(
                program,
                operators,
                parameter_position,
                prefix,
                definition,
                unary.operand,
            )?)))
        }
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::And => Some(CheckedBooleanExpression::And {
                left: Box::new(where_boolean(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    binary.left,
                )?),
                right: Box::new(where_boolean(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    binary.right,
                )?),
            }),
            BinaryOperator::Or => Some(CheckedBooleanExpression::Or {
                left: Box::new(where_boolean(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    binary.left,
                )?),
                right: Box::new(where_boolean(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    binary.right,
                )?),
            }),
            BinaryOperator::Less => where_integer_comparison(
                program,
                operators,
                parameter_position,
                prefix,
                definition,
                CheckedIntegerComparisonKind::LessThan,
                binary.left,
                binary.right,
            ),
            BinaryOperator::LessOrEqual => where_integer_comparison(
                program,
                operators,
                parameter_position,
                prefix,
                definition,
                CheckedIntegerComparisonKind::LessOrEqual,
                binary.left,
                binary.right,
            ),
            BinaryOperator::Greater => where_integer_comparison(
                program,
                operators,
                parameter_position,
                prefix,
                definition,
                CheckedIntegerComparisonKind::LessThan,
                binary.right,
                binary.left,
            ),
            BinaryOperator::GreaterOrEqual => where_integer_comparison(
                program,
                operators,
                parameter_position,
                prefix,
                definition,
                CheckedIntegerComparisonKind::LessOrEqual,
                binary.right,
                binary.left,
            ),
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                let negated = binary.operator == BinaryOperator::NotEqual;
                if let Some(comparison) = where_integer_comparison(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    CheckedIntegerComparisonKind::Equal,
                    binary.left,
                    binary.right,
                ) {
                    return Some(if negated {
                        CheckedBooleanExpression::Not(Box::new(comparison))
                    } else {
                        comparison
                    });
                }
                let left = where_boolean(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    binary.left,
                )?;
                let right = where_boolean(
                    program,
                    operators,
                    parameter_position,
                    prefix,
                    definition,
                    binary.right,
                )?;
                let equal = CheckedBooleanExpression::Equal {
                    left: Box::new(left),
                    right: Box::new(right),
                };
                Some(if negated {
                    CheckedBooleanExpression::Not(Box::new(equal))
                } else {
                    equal
                })
            }
            _ => None,
        },
        _ => {
            // A bare Boolean field leaf (`where initialized,`) is a structural
            // Boolean observation at the same path.
            let (path, leaf_type) = where_field_path(program, prefix, definition, expression)?;
            (program.primitive_type_reference(leaf_type) == Some(PrimitiveType::Bool)).then(|| {
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position,
                    path,
                }
            })
        }
    }
}

#[cfg(test)]
mod tests;
