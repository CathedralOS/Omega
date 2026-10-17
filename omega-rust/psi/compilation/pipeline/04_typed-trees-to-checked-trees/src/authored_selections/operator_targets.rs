//! Checked operator targets and authored operator selection candidates.

/// Handle-keyed maps take the integer hasher; none is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;

use crate::authored_selections::contract_resolution;
use crate::authored_selections::member_targets::{
    expression_is_contextual_statement_primitive, expression_is_intrinsic_primitive_without_origin,
};
use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionOccurrenceId,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use validation::declaration_type_reference;
use validation::expression_owners::reachable_expressions;
use validation::selection_resolution::AuthoredSelectionResolutionTarget;
use validation::selection_resolution::{declaration_target, exact_named_operator_call};

fn intrinsic_operator_operand_is_primitive(
    program: &dyn ::validation::CheckingInput,
    node: &ExpressionNode,
    origin: checked_trees::CheckedValueOrigin,
) -> bool {
    let operand = match node {
        ExpressionNode::Binary(binary) => binary.left,
        ExpressionNode::Unary(unary) => unary.operand,
        _ => return false,
    };
    validation::expression_type_reference_for_origin(program, operand, origin)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
        || expression_is_intrinsic_primitive_without_origin(program, operand, None)
}

/// The checked value facts whose expression contains each expression.
///
/// The open-generic fallback below asks one containment question of every
/// checked value fact, and selection finalization asks it once per authored
/// operator occurrence. Neither the expression table nor the fact roster
/// moves while selections finalize, so one walk per value fact answers every
/// occurrence. Origins keep the fact order the repeated walks produced, and
/// a fact naming an origin already recorded for that expression adds
/// nothing, exactly as the receiving roster's own membership test did.
pub(crate) struct GenericOperatorValueOrigins {
    by_expression:
        HashMap<typed_trees::expression::ExpressionHandle, Vec<checked_trees::CheckedValueOrigin>>,
}

impl GenericOperatorValueOrigins {
    pub(crate) fn index(program: &dyn ::validation::CheckingInput, facts: &CheckFacts) -> Self {
        let mut by_expression: HashMap<_, Vec<checked_trees::CheckedValueOrigin>> =
            HashMap::default();
        for (_, value) in facts.values.values.iter() {
            for reached in reachable_expressions(program, value.expression) {
                let origins = by_expression.entry(reached).or_default();
                if !origins.contains(&value.origin) {
                    origins.push(value.origin);
                }
            }
        }
        Self { by_expression }
    }

    fn origins(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> &[checked_trees::CheckedValueOrigin] {
        self.by_expression
            .get(&expression)
            .map_or(&[][..], Vec::as_slice)
    }
}

pub(crate) fn checked_generic_operator_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    value_origins: &GenericOperatorValueOrigins,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<Option<AuthoredSelectionResolutionTarget>, Diagnostic> {
    let mut selected = None;
    let mut uses = facts
        .operators
        .uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression)
                .then_some((operator_use.spelling, operator_use.origin))
        })
        .collect::<Vec<_>>();
    // An open generic requirement has no selected concrete provider, so the
    // executable operator fact producer may not emit a row for this expression.
    if let Some(binary) = program.expression_table.binary_node(expression)
        && let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
    {
        for origin in value_origins.origins(expression) {
            let selected_use = (spelling, *origin);
            if !uses.contains(&selected_use) {
                uses.push(selected_use);
            }
        }
    }
    for (spelling, origin) in uses {
        let machine_symbol = match origin {
            checked_trees::CheckedValueOrigin::MachineDecrease { machine_symbol, .. }
            | checked_trees::CheckedValueOrigin::MachineOwnedDataInitializer {
                machine_symbol,
                ..
            }
            | checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } => {
                machine_symbol
            }
            checked_trees::CheckedValueOrigin::NestedExpression { .. } => continue,
        };
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            continue;
        };
        if machine.conformance_bounds.is_empty() {
            continue;
        }
        let operands = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
            ExpressionNode::Unary(unary) => vec![unary.operand],
            ExpressionNode::Indexed(indexed) => vec![indexed.collection, indexed.index],
            _ => continue,
        };
        let operand_types = operands
            .into_iter()
            .map(|operand| {
                validation::expression_type_reference_for_origin(program, operand, origin)
            })
            .collect::<Vec<_>>();
        let requirement = validation::generic_bound_operator_requirement(
            program,
            machine,
            spelling,
            &operand_types,
        )
        .map_err(Diagnostic::error)?;
        let Some(requirement) = requirement else {
            continue;
        };
        let target = AuthoredSelectionResolutionTarget::Declaration(requirement.symbol);
        if selected.is_some_and(|previous| previous != target) {
            return Err(Diagnostic::error(
                "one authored operator occurrence has conflicting declared generic owners",
            ));
        }
        selected = Some(target);
    }
    Ok(selected)
}

fn checked_operator_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> Option<AuthoredSelectionResolutionTarget> {
    // These operators have no authored declaration/spelling surface. Once
    // ordinary checking accepts their operand types, their exact meaning is
    // necessarily compiler intrinsic; nested-expression origins need not
    // recover a synthetic type reference merely to finalize custody.
    if matches!(
        node,
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::And
                    | typed_trees::expression::BinaryOperator::BitwiseAnd
                    | typed_trees::expression::BinaryOperator::BitwiseOr
                    | typed_trees::expression::BinaryOperator::BitwiseXor
                    | typed_trees::expression::BinaryOperator::Or
                    | typed_trees::expression::BinaryOperator::ShiftLeft
                    | typed_trees::expression::BinaryOperator::ShiftRight
            )
    ) {
        return Some(AuthoredSelectionResolutionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ));
    }

    let uses = facts
        .operators
        .uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression
                && operator_use.occurrence == checked_trees::CheckedOperatorOccurrence::Expression)
                .then_some(operator_use)
        })
        .collect::<Vec<_>>();

    uses.iter()
        .find_map(|operator_use| {
            (operator_use.status == CheckedOperatorResolutionStatus::Resolved)
                .then(|| declaration_target(operator_use.selected_operator_symbol))
                .flatten()
        })
        .or_else(|| {
            uses.iter()
                .any(|operator_use| {
                    operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
                        || (operator_use.status == CheckedOperatorResolutionStatus::Missing
                            && intrinsic_operator_operand_is_primitive(
                                program,
                                node,
                                operator_use.origin,
                            ))
                })
                .then_some(AuthoredSelectionResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ))
        })
        .or_else(|| {
            contract_resolution::checked_operator_resolution(
                program,
                facts,
                expression,
                node,
                owner_index,
            )
            .and_then(|resolution| match resolution {
                contract_resolution::CheckedContractOperatorResolution::Declaration(symbol) => {
                    declaration_target(symbol)
                }
                contract_resolution::CheckedContractOperatorResolution::Builtin => {
                    Some(AuthoredSelectionResolutionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                    ))
                }
            })
        })
        .or_else(|| {
            resolve_authored_operator_without_use_fact(program, expression, node)
                .and_then(|operator| declaration_target(operator.symbol))
        })
        .or_else(|| {
            let operand = match node {
                ExpressionNode::Binary(binary) => binary.left,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => return None,
            };
            (contract_resolution::checked_operand_type(
                program,
                facts,
                expression,
                operand,
                owner_index,
            )
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
            .is_some()
                || expression_is_intrinsic_primitive_without_origin(
                    program,
                    operand,
                    owner_index.map(|owners| owners.program_only(program)),
                )
                || checked_operator_expression_is_intrinsic_primitive(facts, operand)
                || expression_is_contextual_statement_primitive(
                    program,
                    facts,
                    expression,
                    operand,
                    owner_index,
                ))
            .then_some(AuthoredSelectionResolutionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            ))
        })
        .or_else(|| {
            operator_has_no_authored_spelling_candidate(program, expression, node).then_some(
                AuthoredSelectionResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ),
            )
        })
}

pub(crate) fn checked_operator_target_for_occurrence(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
    owner_index: Option<&validation::expression_owners::OwnerEnvironmentIndex>,
) -> Option<AuthoredSelectionResolutionTarget> {
    let is_rooted = |candidate| {
        owner_index.is_none_or(|owners| owners.contains_expression(candidate))
            || facts
                .operators
                .uses
                .iter()
                .any(|(_, use_fact)| use_fact.expression == candidate)
    };
    let copied_target = || {
        program
            .expression_table
            .iter_expressions()
            .filter(|(candidate, _)| *candidate != expression)
            .filter(|(candidate, _)| {
                program
                    .expression_table
                    .authored_selection_occurrences(*candidate)
                    .any(|retained| retained == occurrence)
            })
            .filter(|(candidate, _)| is_rooted(*candidate))
            .find_map(|(candidate, candidate_node)| {
                checked_operator_target(program, facts, candidate, candidate_node, owner_index)
            })
    };
    // Specialization leaves detached contract copies in the arena. Their
    // concrete operand symbols do not grant a new meaning to the authored
    // occurrence. Carry its rooted selection instead; rooted copies still
    // resolve independently and the ledger checks that they agree.
    if !is_rooted(expression) {
        copied_target()
            .or_else(|| checked_operator_target(program, facts, expression, node, owner_index))
    } else {
        checked_operator_target(program, facts, expression, node, owner_index)
            .or_else(copied_target)
    }
}

fn checked_operator_expression_is_intrinsic_primitive(
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    facts.operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
            && operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
    })
}

/// Whether a `==`/`!=` occurrence typed lowering rewrote into a call selects
/// the `equals` row of an `Equatable` implementation, and checking resolved
/// that call to exactly the same row. The operator occurrence then settles as
/// the builtin operator: the implementation's authored row supplies its meaning.
pub(crate) fn checked_structural_equality_call(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    if !call.target_symbol.is_valid() {
        return false;
    }
    let selects_equality_row = program.conformances().iter().any(|conformance| {
        program.symbols.display_path(conformance.trait_symbol, "::") == "Equatable"
            && conformance.rows.iter().any(|row| {
                row.realization_state == call.target_symbol
                    && row.requirement_name.as_str() == "equals"
            })
    });
    if !selects_equality_row {
        return false;
    }

    // Ordinary checking has already admitted the call, including proof-term
    // formation when it belongs to a contract. Finalize through the common
    // call-selection owner: requiring a runtime flow row here incorrectly
    // excludes proof-static calls with the same exact selected declaration.
    super::call_targets::checked_call_target(
        program,
        facts,
        expression,
        call.target_symbol,
        program.expression_table.source_span(expression),
    ) == call.target_symbol
}

/// Declaration contracts and other proof-static expressions are validated
/// without an executable value origin, so they do not always produce a
/// `CheckedOperatorUseFact`. Finalize their authored selection only when the
/// typed operands select one exact declared operator; partial or ambiguous
/// reconstruction remains unresolved and therefore rejects package custody.
fn resolve_authored_operator_without_use_fact<'program>(
    program: &'program dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> Option<&'program typed_trees::operator::OperatorDefinition> {
    use language_core::OperatorSpelling;

    let (spelling, operand_types) = match node {
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            else {
                return None;
            };
            (
                spelling,
                vec![
                    Some(authored_operand_type(program, binary.left)?),
                    Some(authored_operand_type(program, binary.right)?),
                ],
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = Some(authored_operand_type(program, indexed.collection)?);
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => (
                    OperatorSpelling::Range,
                    vec![
                        collection,
                        authored_operand_type(program, range.start),
                        authored_operand_type(program, range.end),
                    ],
                ),
                _ => (
                    OperatorSpelling::Index,
                    vec![collection, authored_operand_type(program, indexed.index)],
                ),
            }
        }
        _ => return None,
    };
    let candidates = typed_trees::operator::resolve_spelling_for_operands(
        program,
        spelling,
        &operand_types,
        program.expression_table.source_span(expression),
    );
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    Some(candidate.operator)
}

/// The operand type operator selection must read here: the retained type
/// reference when the operand names one, and otherwise the carrier the
/// language fixes for a value that names no type at all.
///
/// A comparison result and a boolean literal are `bool`, but neither points at
/// a written `bool`. Reported as unknown they match every declared parameter
/// as a wildcard, so `(nan32 == nan32) == false` kept a declared `Float::equal`
/// as a candidate and could not be recognized as the builtin it is.
pub(crate) fn authored_operand_descriptor(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> typed_trees::operator::OperandType {
    use typed_trees::operator::OperandType;
    if let Some(type_reference) = authored_operand_type(program, expression) {
        return OperandType::Reference(type_reference);
    }
    authored_operand_carrier(program, expression)
        .map_or(OperandType::Unknown, OperandType::Primitive)
}

/// The primitive carrier an operand's VALUE has, for an operand that names no
/// type reference. An operation's own result is such a value: a comparison
/// produces `bool`, and an arithmetic or bitwise operation produces the
/// carrier its operands share.
///
/// Each answer holds only where no authored operator supplies a different
/// meaning: a declared `less` may return a mask, a declared `add` may return
/// another carrier entirely. Candidates that ALL return the same carrier
/// settle it whichever one is selected, and no candidate at all leaves the
/// builtin operation.
fn authored_operand_carrier(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    use typed_trees::operator::OperandType;
    use typed_trees::types::PrimitiveType;

    /// The carrier a descriptor names, reducing a retained reference through
    /// its shells.
    fn operand_carrier(
        program: &dyn ::validation::CheckingInput,
        operand: OperandType,
    ) -> Option<typed_trees::types::PrimitiveType> {
        match operand {
            OperandType::Reference(reference) => program.primitive_type_reference(reference),
            OperandType::Primitive(primitive) => Some(primitive),
            OperandType::Unknown => None,
        }
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
        ExpressionNode::Binary(binary) => {
            // `&&`, `||` and `is` bind no authored spelling at all, so their
            // result is the builtin one whatever the operands are.
            if matches!(
                binary.operator,
                BinaryOperator::And | BinaryOperator::Or | BinaryOperator::CaseMembership
            ) {
                return Some(PrimitiveType::Bool);
            }
            let left = authored_operand_descriptor(program, binary.left);
            let right = authored_operand_descriptor(program, binary.right);
            let carrier = match binary.operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual => PrimitiveType::Bool,
                // The operands' shared carrier. One unknown side is answered
                // by the other; two KNOWN sides that disagree are a mixed
                // operation this reader does not settle.
                _ => match (
                    operand_carrier(program, left),
                    operand_carrier(program, right),
                ) {
                    (Some(left), Some(right)) if left == right => left,
                    (Some(carrier), None) | (None, Some(carrier)) => carrier,
                    _ => return None,
                },
            };
            candidate_operators(program, expression, &[left, right])
                .iter()
                .all(|candidate| {
                    program.primitive_type_reference(candidate.operator.return_type)
                        == Some(carrier)
                })
                .then_some(carrier)
        }
        ExpressionNode::Unary(unary) => match unary.operator {
            UnaryOperator::LogicalNot => Some(PrimitiveType::Bool),
            UnaryOperator::BitwiseNot => {
                operand_carrier(program, authored_operand_descriptor(program, unary.operand))
            }
        },
        _ => None,
    }
}

pub(crate) fn authored_operand_type(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        // A suffix supplies an exact carrier even in a declaration endpoint
        // with no executable use fact. Reuse numeric landing validation;
        // anonymous literals must remain unknown, not inherit a peer's type.
        ExpressionNode::Integer(_) => {
            program
                .closed_integer_value_in(expression, SymbolHandle::invalid())?
                .type_reference
        }
        ExpressionNode::Atomic(atomic) => authored_operand_type(program, atomic.value),
        ExpressionNode::Borrow(borrow) => authored_operand_type(program, borrow.target),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => declaration_type_reference(
            program,
            validation::effective_member_symbol(program, member.receiver, member),
        ),
        ExpressionNode::Call(call) => {
            exact_named_operator_call(program, call).map(|operator| operator.return_type)
        }
        ExpressionNode::Name(path) => declaration_type_reference(program, path.symbol)
            .or_else(|| operator_contract_value_type(program, expression, path)),
        _ => None,
    }
}

fn operator_contract_value_type(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    let mut types = program.all_operators().filter_map(|operator| {
        // The name match is a field comparison; the containment walk
        // visits every contract expression, so it runs only for an
        // operator whose signature could type the name at all.
        let type_reference = if language_core::is_reserved_result(name) {
            operator.return_type
        } else {
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.name.as_str() == name)?
                .type_reference
        };
        program
            .operator_contracts(operator)
            .iter()
            .any(|contract| {
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        typed_trees::domain::ProofFact::Expression(root) => {
                            validation::expression_owners::expression_contains(
                                program, *root, expression,
                            )
                        }
                        typed_trees::domain::ProofFact::Membership(membership) => {
                            validation::expression_owners::expression_contains(
                                program,
                                membership.value,
                                expression,
                            )
                        }
                        typed_trees::domain::ProofFact::Proposition(_) => false,
                    })
            })
            .then_some(type_reference)
    });
    let first = types.next()?;
    types
        .all(|type_reference| type_reference == first)
        .then_some(first)
}

fn operator_has_no_authored_spelling_candidate(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> bool {
    use language_core::OperatorSpelling;
    let spelling = match node {
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            else {
                return true;
            };
            spelling
        }
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                OperatorSpelling::Range
            } else {
                OperatorSpelling::Index
            }
        }
        // Unary operators have no authored declaration dispatch surface.
        ExpressionNode::Unary(_) => return true,
        _ => return false,
    };
    typed_trees::operator::resolve_spelling(
        program,
        spelling,
        None,
        program.expression_table.source_span(expression),
    )
    .is_empty()
}

/// Return whether an early typed operator expression cannot select an authored
/// operator declaration. Ordinary checked lowering remains responsible for
/// rejecting a semantically invalid builtin use.
pub(crate) fn typed_operator_has_no_authored_selection(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let node = program.expression_table.expression(expression);
    operator_has_no_authored_spelling_candidate(program, expression, node)
        // A loaded declaration with the same spelling is not necessarily a
        // candidate for these operands. Reuse the checked selection query's
        // primitive/candidate distinction before build-time execution, while
        // leaving compatible domain operators subject to authored admission.
        || (matches!(node, ExpressionNode::Binary(_))
            && expression_is_intrinsic_primitive_without_origin(program, expression, None))
}

pub(crate) fn typed_operator_authored_selection_candidates(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    typed_operator_authored_selection_candidate_operators(program, expression)
        .into_iter()
        .map(|candidate| candidate.operator.symbol)
        .collect()
}

/// [`typed_operator_authored_selection_candidates`] with the declarations
/// themselves, for a caller that needs what a selected candidate would return.
fn typed_operator_authored_selection_candidate_operators<'program>(
    program: &'program dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::operator::SpelledOperator<'program>> {
    candidate_operators(program, expression, &[])
}

/// [`typed_operator_authored_selection_candidate_operators`] for a caller that
/// already holds a BINARY expression's two operand descriptors. Deriving them
/// again would walk each operand once per enclosing operation, which turns a
/// nested arithmetic expression's carrier into an exponential walk.
fn candidate_operators<'program>(
    program: &'program dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    binary_operands: &[typed_trees::operator::OperandType],
) -> Vec<typed_trees::operator::SpelledOperator<'program>> {
    use language_core::OperatorSpelling;

    let (spelling, operand_types) = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
            else {
                return Vec::new();
            };
            (
                spelling,
                match binary_operands {
                    [left, right] => vec![*left, *right],
                    _ => vec![
                        authored_operand_descriptor(program, binary.left),
                        authored_operand_descriptor(program, binary.right),
                    ],
                },
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = authored_operand_descriptor(program, indexed.collection);
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => (
                    OperatorSpelling::Range,
                    vec![
                        collection,
                        authored_operand_descriptor(program, range.start),
                        authored_operand_descriptor(program, range.end),
                    ],
                ),
                _ => (
                    OperatorSpelling::Index,
                    vec![
                        collection,
                        authored_operand_descriptor(program, indexed.index),
                    ],
                ),
            }
        }
        ExpressionNode::Unary(_) => return Vec::new(),
        _ => return Vec::new(),
    };

    typed_trees::operator::resolve_spelling_for_operand_types(
        program,
        spelling,
        &operand_types,
        program.expression_table.source_span(expression),
    )
}
