//! Consumer-facing result types, independent of an enclosing destination.
//!
//! Declaration-backed leaves retain their complete type. Computed builtin
//! results require the node's actual selected meaning and signature. Direct
//! operators may expose caller-independent declared results; copying a
//! sibling's type could turn a comparison into an integer or export unproved
//! input refinements. Guard candidate lookup remains separate: its conservative
//! operand shells are selection inputs, not computed-result evidence.

use language_core::OperatorSpelling;
use numerics::arithmetic::ArithmeticDomain;
use symbols::BuiltinTypeAtom;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[cfg(test)]
mod tests;

/// Return an existing result reference, never an expected-type guess. An
/// unresolved result does not establish anonymous numeric meaning. Builtin
/// computed results retain their carrier and policy, not input predicates.
/// Direct operators retain closed builtin carrier/policy result references.
/// Casts retain their normalized semantic-domain result separately from the
/// authored membership target. Operator results needing instantiated semantic
/// domains, predicates or selected traits remain unresolved rather than losing
/// their meaning.
pub fn expression_result_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    result_type(program, machine, state, expression, &mut Vec::new())
}

fn result_type(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> Option<TypeReferenceHandle> {
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return None;
    }
    active.push(expression);
    let result = match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms);
            let references = arms
                .iter()
                .map(|arm| result_type(program, machine, state, arm.value, active))
                .collect::<Vec<_>>();
            references
                .iter()
                .copied()
                .flatten()
                .next()
                .and_then(|first| {
                    // Exact shared identity retains common predicates. Otherwise
                    // a numeric join weakens predicate facts, never policy. A range
                    // on one arm cannot become a promise about an anonymous peer.
                    // Do not compare rendered bounds: equal spellings need not
                    // name the same dependent predicate subject.
                    if references.iter().all(|reference| *reference == Some(first)) {
                        return Some(first);
                    }
                    // Separately authored qualifications have separate type
                    // handles. Their normalized declared-domain identities may
                    // still agree (including transparent aliases and indices).
                    // Raw range predicates remain on the exact-subject path
                    // above: rendered dependent bounds are not interchangeable.
                    if has_domain_result_shell(program, first) {
                        let identity = program.normalized_type_identity(first);
                        if references.iter().all(|reference| {
                            reference.is_some_and(|reference| {
                                has_domain_result_shell(program, reference)
                                    && program.normalized_type_identity(reference) == identity
                            })
                        }) {
                            return Some(first);
                        }
                    }
                    let carrier = arithmetic_carrier(program, first)?;
                    arms.iter()
                        .zip(&references)
                        .all(|(arm, reference)| {
                            reference.map_or_else(
                                || {
                                    crate::literals::has_anonymous_numeric_results(
                                        program, arm.value,
                                    )
                                },
                                |reference| arithmetic_carrier(program, reference) == Some(carrier),
                            )
                        })
                        .then(|| arithmetic_result(program, first))
                        .flatten()
                })
        }
        ExpressionNode::Call(call) => crate::calls::resolved_call_result_type(program, call)
            .or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            }),
        ExpressionNode::StructLiteral(literal) => program
            .type_reference_table
            .find_named_type_reference(literal.type_symbol),
        ExpressionNode::Cast(cast) => {
            // Cast policy and semantic-domain suffixes live outside target_type.
            // Returning that bare target would erase the result qualification
            // before a surrounding operator selects its meaning.
            if program
                .type_reference_table
                .contains_type_reference(cast.result_type)
            {
                Some(cast.result_type)
            } else if !cast.semantic_domain.is_empty() {
                None
            } else if cast.domain == ArithmeticDomain::Exact {
                Some(cast.target_type)
            } else if let TypeReferenceNode::Named { symbol, .. } = program
                .type_reference_table
                .type_reference(cast.target_type)
            {
                program.symbols.builtin_type_atom(*symbol).and_then(|_| {
                    program
                        .type_reference_table
                        .find_arithmetic_result_type_reference(*symbol, cast.domain)
                })
            } else {
                program
                    .type_reference_table
                    .find_policy_qualified_type_reference(cast.target_type, cast.domain)
            }
        }
        ExpressionNode::ZeroValue(reference) => Some(*reference),
        // Place lookup strips a Borrow to its target. A value-type query must
        // not turn that view into a by-value operand and hide reference-typed
        // operator candidates, including through a nested Match result.
        ExpressionNode::Borrow(_) => None,
        ExpressionNode::Integer(_) => {
            crate::operators::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Boolean(_) => builtin_reference(program, BuiltinTypeAtom::Bool),
        ExpressionNode::Float(literal) => literal.landing().and_then(|format| {
            builtin_reference(
                program,
                match format {
                    numerics::literals::FloatFormat::F32 => BuiltinTypeAtom::F32,
                    numerics::literals::FloatFormat::F64 => BuiltinTypeAtom::F64,
                },
            )
        }),
        ExpressionNode::Binary(binary) => {
            let operands = [binary.left, binary.right]
                .map(|operand| result_type(program, machine, state, operand, active));
            binary_result(program, machine, expression, binary, operands)
        }
        ExpressionNode::Unary(unary) => {
            let operand = result_type(program, machine, state, unary.operand, active);
            match unary.operator {
                UnaryOperator::BitwiseNot => operand
                    .and_then(|reference| arithmetic_result(program, reference))
                    .filter(|reference| integer(program, *reference)),
                UnaryOperator::LogicalNot => operand
                    .filter(|reference| primitive(program, *reference) == Some(PrimitiveType::Bool))
                    .and_then(|_| builtin_reference(program, BuiltinTypeAtom::Bool)),
            }
        }
        _ => crate::places::declared_place_type_raw(program, machine, Some(state), expression),
    };
    active.pop();
    result.filter(|reference| {
        program
            .type_reference_table
            .contains_type_reference(*reference)
    })
}

fn has_domain_result_shell(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut seen = Vec::new();
    let mut domain = false;
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !seen.contains(&reference)
    {
        seen.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let Some(constraints) = program.type_reference_table.constraint_span(*constraints)
                else {
                    return false;
                };
                for constraint in constraints {
                    match constraint {
                        TypeConstraintNode::Domain(qualification)
                            if qualification.symbol.is_valid() =>
                        {
                            domain = true
                        }
                        TypeConstraintNode::ArithmeticDomain(_) => {}
                        _ => return false,
                    }
                }
                reference = *base_type;
            }
            TypeReferenceNode::Named { .. } => return domain,
            _ => return false,
        }
    }
    false
}

fn binary_result(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
    operands: [Option<TypeReferenceHandle>; 2],
) -> Option<TypeReferenceHandle> {
    use BinaryOperator::*;
    let spelling = match binary.operator {
        Add => Some(OperatorSpelling::Add),
        Subtract => Some(OperatorSpelling::Subtract),
        Multiply => Some(OperatorSpelling::Multiply),
        Divide => Some(OperatorSpelling::Divide),
        Modulo => Some(OperatorSpelling::Modulo),
        Equal => Some(OperatorSpelling::Equal),
        NotEqual => Some(OperatorSpelling::NotEqual),
        Less => Some(OperatorSpelling::Less),
        LessOrEqual => Some(OperatorSpelling::LessEqual),
        Greater => Some(OperatorSpelling::Greater),
        GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
        And | Or | BitwiseAnd | BitwiseOr | BitwiseXor | ShiftLeft | ShiftRight
        | CaseMembership => None,
    };
    // Unknown operands stay unknown during selection. Substituting the known
    // peer first could hide a heterogeneous or reference-typed declaration.
    if let Some(spelling) = spelling
        && !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &operands,
        )
    {
        let candidates =
            typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operands);
        let [selected] = candidates.as_slice() else {
            return None;
        };
        if !typed_trees::operator::selected_trait_operator_meanings(
            program,
            machine.symbol,
            spelling,
            &operands,
        )
        .is_empty()
        {
            return None;
        }
        // This is the selected declaration's result, never an operand guess.
        // A concrete result needs no substitution even if operands are generic.
        // Dependent predicates and result binders need an instantiated reference;
        // exporting their declaration-local subjects would invent caller facts.
        let result = selected.operator.return_type;
        qualified_builtin_carrier(program, result, false)?;
        return Some(result);
    }
    match binary.operator {
        CaseMembership => crate::bound_expression_meaning::has_exact_case_membership_meaning(
            program, machine, None, expression, binary,
        )
        .then(|| builtin_reference(program, BuiltinTypeAtom::Bool))
        .flatten(),
        And | Or => operands
            .into_iter()
            .all(|reference| {
                reference.and_then(|reference| primitive(program, reference))
                    == Some(PrimitiveType::Bool)
            })
            .then(|| builtin_reference(program, BuiltinTypeAtom::Bool))
            .flatten(),
        Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => {
            let compatible = if let Some(carrier) = operands.into_iter().flatten().next() {
                let primitive = primitive(program, carrier)?;
                let supported = integer(program, carrier)
                    || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    || (matches!(binary.operator, Equal | NotEqual)
                        && primitive == PrimitiveType::Bool);
                supported && compatible_operands(program, binary, operands, carrier)
            } else {
                [binary.left, binary.right]
                    .into_iter()
                    .all(|operand| crate::literals::has_anonymous_numeric_results(program, operand))
            };
            compatible
                .then(|| builtin_reference(program, BuiltinTypeAtom::Bool))
                .flatten()
        }
        ShiftLeft | ShiftRight => {
            let left = operands[0]
                .and_then(|reference| arithmetic_result(program, reference))
                .filter(|reference| integer(program, *reference))?;
            let count = operands[1].map_or_else(
                || crate::literals::has_anonymous_numeric_results(program, binary.right),
                |reference| integer(program, reference),
            );
            count.then_some(left)
        }
        Add | Subtract | Multiply | Divide => {
            let carrier = arithmetic_result(program, operands.into_iter().flatten().next()?)?;
            let numeric = integer(program, carrier)
                || matches!(
                    primitive(program, carrier),
                    Some(PrimitiveType::F32 | PrimitiveType::F64)
                );
            (numeric && compatible_operands(program, binary, operands, carrier)).then_some(carrier)
        }
        Modulo | BitwiseAnd | BitwiseOr | BitwiseXor => {
            let carrier = arithmetic_result(program, operands.into_iter().flatten().next()?)?;
            (integer(program, carrier) && compatible_operands(program, binary, operands, carrier))
                .then_some(carrier)
        }
    }
}

fn compatible_operands(
    program: &TypedTrees,
    binary: &TableBinaryExpression,
    operands: [Option<TypeReferenceHandle>; 2],
    carrier: TypeReferenceHandle,
) -> bool {
    [binary.left, binary.right]
        .into_iter()
        .zip(operands)
        .all(|(expression, reference)| {
            reference.map_or_else(
                || crate::literals::has_anonymous_numeric_results(program, expression),
                |reference| {
                    arithmetic_carrier(program, reference) == arithmetic_carrier(program, carrier)
                },
            )
        })
}

fn primitive(program: &TypedTrees, reference: TypeReferenceHandle) -> Option<PrimitiveType> {
    arithmetic_carrier(program, reference)?;
    program.primitive_type_reference(reference)
}

fn integer(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    matches!(
        primitive(program, reference),
        Some(
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        )
    )
}

// Raw operand references reach overload selection above. Only after builtin
// selection may range predicates be forgotten: an operation on [0..=10] can
// produce 11. Arithmetic policy is different: it governs this result's next
// operation and must survive. Unknown domain and reference shells are not
// predicate-only qualifications and cannot use this projection.
pub(super) fn arithmetic_carrier(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
    qualified_builtin_carrier(program, reference, true)
}

fn qualified_builtin_carrier(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    allow_range_predicates: bool,
) -> Option<(symbols::SymbolHandle, ArithmeticDomain)> {
    let mut policy = None;
    let mut visited = Vec::new();
    while program
        .type_reference_table
        .contains_type_reference(reference)
        && !visited.contains(&reference)
    {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                program.symbols.builtin_type_atom(*symbol)?;
                return Some((*symbol, policy.unwrap_or(ArithmeticDomain::Exact)));
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraint_span(*constraints)? {
                    match constraint {
                        TypeConstraintNode::Range { .. } if allow_range_predicates => {}
                        TypeConstraintNode::ArithmeticDomain(domain) => {
                            if policy.is_some() {
                                return None;
                            }
                            policy = Some(*domain);
                        }
                        TypeConstraintNode::Range { .. }
                        | TypeConstraintNode::Named(_)
                        | TypeConstraintNode::Domain(_) => {
                            return None;
                        }
                    }
                }
                reference = *base_type;
            }
            _ => return None,
        }
    }
    None
}

fn arithmetic_result(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let (carrier, policy) = arithmetic_carrier(program, reference)?;
    program
        .type_reference_table
        .find_arithmetic_result_type_reference(carrier, policy)
}

fn builtin_reference(program: &TypedTrees, atom: BuiltinTypeAtom) -> Option<TypeReferenceHandle> {
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}
