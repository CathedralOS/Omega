//! Checked operator targets and authored operator selection candidates.

use std::collections::HashMap;

use crate::authored_selections::CheckedResolutionTarget;
use crate::authored_selections::call_targets::{declaration_target, exact_named_operator_call};
use crate::authored_selections::contract_resolution;
use crate::authored_selections::member_targets::{
    expression_is_contextual_domain_primitive, expression_is_contextual_statement_primitive,
    expression_is_intrinsic_primitive_without_origin, reachable_expressions,
};
use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionOccurrenceId,
};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

fn intrinsic_operator_operand_is_primitive(
    program: &TypedTrees,
    node: &ExpressionNode,
    origin: checked_trees::CheckedValueOrigin,
) -> bool {
    let operand = match node {
        ExpressionNode::Binary(binary) => binary.left,
        ExpressionNode::Unary(unary) => unary.operand,
        _ => return false,
    };
    crate::operators::expression_type_reference_for_origin(program, operand, origin)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
        || expression_is_intrinsic_primitive_without_origin(program, operand)
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
    pub(crate) fn index(program: &TypedTrees, facts: &CheckFacts) -> Self {
        let mut by_expression: HashMap<_, Vec<checked_trees::CheckedValueOrigin>> = HashMap::new();
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
    program: &TypedTrees,
    facts: &CheckFacts,
    value_origins: &GenericOperatorValueOrigins,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<Option<CheckedResolutionTarget>, Diagnostic> {
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
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && let Some(spelling) = crate::operators::binary_operator_spelling(binary.operator)
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
        let Some(machine) = crate::lookup::machine_by_symbol(program, machine_symbol) else {
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
                crate::operators::expression_type_reference_for_origin(program, operand, origin)
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
        let target = CheckedResolutionTarget::Declaration(requirement.symbol);
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
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> Option<CheckedResolutionTarget> {
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
        return Some(CheckedResolutionTarget::Intrinsic(
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
                .then_some(CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ))
        })
        .or_else(|| {
            contract_resolution::checked_operator_resolution(program, facts, expression, node)
                .and_then(|resolution| match resolution {
                    contract_resolution::CheckedContractOperatorResolution::Declaration(symbol) => {
                        declaration_target(symbol)
                    }
                    contract_resolution::CheckedContractOperatorResolution::Builtin => {
                        Some(CheckedResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        ))
                    }
                })
        })
        .or_else(|| {
            resolve_authored_operator_without_use_fact(program, node)
                .and_then(|operator| declaration_target(operator.symbol))
        })
        .or_else(|| {
            let operand = match node {
                ExpressionNode::Binary(binary) => binary.left,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => return None,
            };
            (contract_resolution::checked_operand_type(program, facts, expression, operand)
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
                .is_some()
                || expression_is_intrinsic_primitive_without_origin(program, operand)
                || checked_operator_expression_is_intrinsic_primitive(facts, operand)
                || expression_is_contextual_domain_primitive(program, expression, operand)
                || expression_is_contextual_statement_primitive(program, expression, operand))
            .then_some(CheckedResolutionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            ))
        })
        .or_else(|| {
            operator_has_no_authored_spelling_candidate(program, node).then_some(
                CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ),
            )
        })
}

pub(crate) fn checked_operator_target_for_occurrence(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
) -> Option<CheckedResolutionTarget> {
    checked_operator_target(program, facts, expression, node).or_else(|| {
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
            .find_map(|(candidate, candidate_node)| {
                checked_operator_target(program, facts, candidate, candidate_node)
            })
    })
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

pub(crate) fn checked_structural_equality_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    if call.target.as_str() != "equals" || !call.target_symbol.is_valid() {
        return false;
    }

    let owners = program
        .machines()
        .iter()
        .filter(|machine| {
            machine.attached_data.is_some()
                && program.machine_states(machine).iter().any(|state| {
                    state.symbol == call.target_symbol && state.name.as_str() == "equals"
                })
        })
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return false;
    };
    let Some(carrier) = owner.attached_data.as_ref() else {
        return false;
    };
    if !program.conformances().iter().any(|conformance| {
        conformance.trait_name.as_str() == "Equatable"
            && conformance
                .carrier_name()
                .is_some_and(|candidate| candidate.as_str() == carrier.as_str())
    }) {
        return false;
    }

    let checked_targets = facts
        .flow
        .control
        .states
        .iter()
        .flat_map(|(_, state)| {
            facts
                .flow
                .control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .map(move |checked_call| (state, checked_call))
        })
        .filter_map(|(state, checked_call)| {
            match crate::semantic::calls::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                checked_call.statement_index,
                checked_call.call_ordinal,
            ) {
                Some(crate::semantic::calls::CallSite::Expression {
                    expression: candidate,
                    ..
                }) if candidate == expression => Some(checked_call.target_symbol),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    !checked_targets.is_empty()
        && checked_targets
            .iter()
            .all(|target| *target == call.target_symbol)
}

/// Declaration contracts and other proof-static expressions are validated
/// without an executable value origin, so they do not always produce a
/// `CheckedOperatorUseFact`. Finalize their authored selection only when the
/// typed operands select one exact declared operator; partial or ambiguous
/// reconstruction remains unresolved and therefore rejects package custody.
fn resolve_authored_operator_without_use_fact<'program>(
    program: &'program TypedTrees,
    node: &ExpressionNode,
) -> Option<&'program typed_trees::operator::OperatorDefinition> {
    use language_core::OperatorSpelling;
    pub(crate) use typed_trees::expression::BinaryOperator;

    let (spelling, operand_types) = match node {
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::CaseMembership => return None,
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
    let candidates =
        typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operand_types);
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    Some(candidate.operator)
}

pub(crate) fn authored_operand_type(
    program: &TypedTrees,
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
        ExpressionNode::Member(member) => type_reference_for_symbol(
            program,
            crate::flow::effective_member_symbol(program, member.receiver, member),
        ),
        ExpressionNode::Call(call) => {
            exact_named_operator_call(program, call).map(|operator| operator.return_type)
        }
        ExpressionNode::Name(path) => type_reference_for_symbol(program, path.symbol)
            .or_else(|| operator_contract_value_type(program, expression, path)),
        _ => None,
    }
}

fn operator_contract_value_type(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    let mut types = program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
        .filter(|operator| {
            program.operator_contracts(operator).iter().any(|contract| {
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        typed_trees::domain::ProofFact::Expression(root) => {
                            crate::authored_selections::member_targets::expression_contains(
                                program, *root, expression,
                            )
                        }
                        typed_trees::domain::ProofFact::Membership(membership) => {
                            crate::authored_selections::member_targets::expression_contains(
                                program,
                                membership.value,
                                expression,
                            )
                        }
                        typed_trees::domain::ProofFact::Proposition(_) => false,
                    })
            })
        })
        .filter_map(|operator| {
            if name == "result" {
                return Some(operator.return_type);
            }
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.name.as_str() == name)
                .map(|parameter| parameter.type_reference)
        });
    let first = types.next()?;
    types
        .all(|type_reference| type_reference == first)
        .then_some(first)
}

fn operator_has_no_authored_spelling_candidate(
    program: &TypedTrees,
    node: &ExpressionNode,
) -> bool {
    use language_core::OperatorSpelling;
    use typed_trees::expression::BinaryOperator;
    let spelling = match node {
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Add => OperatorSpelling::Add,
            BinaryOperator::Subtract => OperatorSpelling::Subtract,
            BinaryOperator::Multiply => OperatorSpelling::Multiply,
            BinaryOperator::Divide => OperatorSpelling::Divide,
            BinaryOperator::Modulo => OperatorSpelling::Modulo,
            BinaryOperator::Equal => OperatorSpelling::Equal,
            BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
            BinaryOperator::Less => OperatorSpelling::Less,
            BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
            BinaryOperator::Greater => OperatorSpelling::Greater,
            BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
            BinaryOperator::And
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Or
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::CaseMembership => return true,
        },
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
    typed_trees::operator::resolve_spelling(program, spelling, None).is_empty()
}

/// Return whether an early typed operator expression cannot select an authored
/// operator declaration. Ordinary checked lowering remains responsible for
/// rejecting a semantically invalid builtin use.
pub(crate) fn typed_operator_has_no_authored_selection(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let node = program.expression_table.expression(expression);
    operator_has_no_authored_spelling_candidate(program, node)
        // A loaded declaration with the same spelling is not necessarily a
        // candidate for these operands. Reuse the checked selection query's
        // primitive/candidate distinction before build-time execution, while
        // leaving compatible domain operators subject to authored admission.
        || (matches!(node, ExpressionNode::Binary(_))
            && expression_is_intrinsic_primitive_without_origin(program, expression))
}

pub(crate) fn typed_operator_authored_selection_candidates(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    use language_core::OperatorSpelling;
    pub(crate) use typed_trees::expression::BinaryOperator;

    let (spelling, operand_types) = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::CaseMembership => return Vec::new(),
            };
            (
                spelling,
                vec![
                    authored_operand_type(program, binary.left),
                    authored_operand_type(program, binary.right),
                ],
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = authored_operand_type(program, indexed.collection);
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
        ExpressionNode::Unary(_) => return Vec::new(),
        _ => return Vec::new(),
    };

    typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operand_types)
        .into_iter()
        .map(|candidate| candidate.operator.symbol)
        .collect()
}

pub(crate) fn type_reference_for_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let declaration = program.symbols.get(symbol);
    if declaration.kind == SymbolKind::Unknown {
        return None;
    }
    let parent = declaration.parent;
    let owner = program.symbols.get(parent);
    if declaration.kind == SymbolKind::Const {
        return program.const_declarations().iter().find_map(|declaration| {
            (declaration.symbol == symbol).then_some(declaration.declared_type)
        });
    }
    // Parent links select the declaration's current owner. generated_from is
    // provenance, not a fallback to a template's potentially different type.
    let data_owner = if owner.kind == SymbolKind::Variant {
        owner.parent
    } else {
        parent
    };
    for data in program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == data_owner)
    {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(field.type_reference);
                }
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == parent => {
                    if let Some(type_reference) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|field| (field.symbol == symbol).then_some(field.type_reference))
                    {
                        return Some(type_reference);
                    }
                }
                _ => {}
            }
        }
    }
    let machine_owner = if owner.kind == SymbolKind::State {
        owner.parent
    } else {
        parent
    };
    if let Some(machine) = crate::lookup::machine_by_symbol(program, machine_owner) {
        if let Some(type_reference) = program
            .machine_owned_data(machine)
            .iter()
            .find_map(|owned| (owned.symbol == symbol).then_some(owned.type_reference))
        {
            return Some(type_reference);
        }
        // A bare attached field names the machine's inherited Field slot,
        // not the original data member. Rejoin that exact owner/slot before
        // inferring the type used to select a nested member declaration.
        if declaration.kind == symbols::SymbolKind::Field
            && declaration.parent == machine.symbol
            && let Some(field) = validation::exact_attached_field(
                program,
                machine,
                symbol,
                program.symbols.name(symbol),
            )
        {
            return Some(field.type_reference);
        }
        for state in program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == parent)
        {
            if let Some(type_reference) =
                program
                    .state_parameters(state)
                    .iter()
                    .find_map(|parameter| {
                        (parameter.symbol == symbol).then_some(parameter.type_reference)
                    })
            {
                return Some(type_reference);
            }
            if let Some(type_reference) = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == symbol =>
                    {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
            {
                return Some(type_reference);
            }
        }
    }
    for definition in program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == owner.parent)
    {
        for signature in program
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == parent)
        {
            if let Some(type_reference) = program
                .state_signature_parameters(signature)
                .iter()
                .find_map(|parameter| {
                    (parameter.symbol == symbol).then_some(parameter.type_reference)
                })
            {
                return Some(type_reference);
            }
        }
    }
    for proposition in program
        .propositions()
        .iter()
        .filter(|proposition| proposition.symbol == parent)
    {
        if let Some(type_reference) = program
            .proposition_parameters(proposition)
            .iter()
            .find_map(|parameter| (parameter.symbol == symbol).then_some(parameter.type_reference))
        {
            return Some(type_reference);
        }
    }
    None
}
