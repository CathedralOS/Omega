use omega_checked_trees::{
    CheckedOperatorCandidateFact, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, CheckedValueFacts, CheckedValueOrigin,
};
use omega_core::arena::Arena;
use omega_core::operator_spelling::OperatorSpelling;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::TypeParameter;
use omega_typed_trees::domain::DomainDefinition;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::operator::OperatorDefinition;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn build_operator_facts(
    program: &TypedTrees,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    let mut uses = Arena::default();
    let mut candidates = Arena::default();
    let mut seen = Vec::new();

    for (_, value) in values.values.iter() {
        collect_expression_operator_use(
            program,
            value.expression,
            value.origin,
            &mut seen,
            &mut uses,
            &mut candidates,
        );
    }

    CheckedOperatorFacts::with_roots(uses, candidates)
}

fn collect_expression_operator_use(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut Vec<(ExpressionHandle, CheckedValueOrigin)>,
    uses: &mut Arena<CheckedOperatorUseFact>,
    candidates: &mut Arena<CheckedOperatorCandidateFact>,
) {
    if !expression.is_valid() || seen.iter().any(|seen| *seen == (expression, origin)) {
        return;
    }
    seen.push((expression, origin));

    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let spelling = indexed_operator_spelling(program, indexed.index);
            let receiver_type =
                expression_type_reference_for_origin(program, indexed.collection, origin);
            uses.append(operator_use_fact(
                program,
                expression,
                origin,
                spelling,
                receiver_type,
                candidates,
            ));
            collect_expression_operator_use(
                program,
                indexed.collection,
                origin,
                seen,
                uses,
                candidates,
            );
            collect_expression_operator_use(program, indexed.index, origin, seen, uses, candidates);
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_operator_use(program, *value, origin, seen, uses, candidates);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_operator_use(program, binary.left, origin, seen, uses, candidates);
            collect_expression_operator_use(program, binary.right, origin, seen, uses, candidates);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_operator_use(program, cast.value, origin, seen, uses, candidates);
        }
        ExpressionNode::Call(call) => {
            collect_expression_operator_use(program, call.receiver, origin, seen, uses, candidates);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_operator_use(program, *argument, origin, seen, uses, candidates);
            }
        }
        ExpressionNode::Member(member) => {
            collect_expression_operator_use(
                program,
                member.receiver,
                origin,
                seen,
                uses,
                candidates,
            );
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression_operator_use(program, *inner, origin, seen, uses, candidates);
        }
        ExpressionNode::Range(range) => {
            collect_expression_operator_use(program, range.start, origin, seen, uses, candidates);
            collect_expression_operator_use(program, range.end, origin, seen, uses, candidates);
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_operator_use(
                    program,
                    field.value,
                    origin,
                    seen,
                    uses,
                    candidates,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn indexed_operator_spelling(program: &TypedTrees, index: ExpressionHandle) -> OperatorSpelling {
    if index.is_valid()
        && matches!(
            program.expression_table.expression(index),
            ExpressionNode::Range(_)
        )
    {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    }
}

fn operator_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    receiver_type: Option<TypeReferenceHandle>,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let candidates = operator_candidates_by_spelling(program, spelling, receiver_type);
    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(
        candidates
            .iter()
            .map(|candidate| candidate.as_checked_candidate()),
    );
    let selected_operator_symbol = match candidates.as_slice() {
        [candidate] => candidate.operator.symbol,
        _ => SymbolHandle::invalid(),
    };
    let status = match candidate_count {
        0 => CheckedOperatorResolutionStatus::Missing,
        1 => CheckedOperatorResolutionStatus::Resolved,
        _ => CheckedOperatorResolutionStatus::Ambiguous,
    };

    CheckedOperatorUseFact {
        expression,
        origin,
        spelling,
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    }
}

struct OperatorCandidate<'program> {
    operator: &'program OperatorDefinition,
    domain: Option<&'program DomainDefinition>,
}

impl OperatorCandidate<'_> {
    fn as_checked_candidate(&self) -> CheckedOperatorCandidateFact {
        if let Some(domain) = self.domain {
            CheckedOperatorCandidateFact::domain(self.operator.symbol, domain.symbol)
        } else {
            CheckedOperatorCandidateFact::root(self.operator.symbol)
        }
    }
}

fn operator_candidates_by_spelling(
    program: &TypedTrees,
    spelling: OperatorSpelling,
    receiver_type: Option<TypeReferenceHandle>,
) -> Vec<OperatorCandidate<'_>> {
    let root_candidates = program
        .operators()
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .map(|operator| OperatorCandidate {
            operator,
            domain: None,
        });
    let domain_candidates = program.domain_definitions().iter().flat_map(|domain| {
        program
            .domain_operators(domain)
            .iter()
            .filter(move |operator| operator.spelling == Some(spelling))
            .map(move |operator| OperatorCandidate {
                operator,
                domain: Some(domain),
            })
    });

    let candidates = root_candidates.chain(domain_candidates).collect::<Vec<_>>();

    if let Some(receiver_type) = receiver_type {
        candidates
            .into_iter()
            .filter(|candidate| {
                candidate_matches_receiver(program, candidate.operator, receiver_type)
            })
            .collect()
    } else {
        candidates
    }
}

fn candidate_matches_receiver(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    receiver_type: TypeReferenceHandle,
) -> bool {
    let Some(receiver_parameter) = program.operator_parameters(operator).first() else {
        return false;
    };
    type_reference_matches(
        program,
        receiver_type,
        receiver_parameter.type_reference,
        program.operator_type_parameters(operator),
        &mut Vec::new(),
    )
}

fn type_reference_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
) -> bool {
    if !actual.is_valid() || !expected.is_valid() {
        return false;
    }
    if let Some(type_parameter) = expected_type_parameter(program, expected, type_parameters) {
        if let Some((_, bound_actual)) = bindings
            .iter()
            .find(|(symbol, _)| *symbol == type_parameter.symbol)
        {
            return type_references_equal(program, actual, *bound_actual);
        }
        bindings.push((type_parameter.symbol, actual));
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(expected),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                is_mutable: actual_mutable,
            },
            TypeReferenceNode::Reference {
                referee: expected_referee,
                is_mutable: expected_mutable,
            },
        ) => {
            actual_mutable == expected_mutable
                && type_reference_matches(
                    program,
                    *actual_referee,
                    *expected_referee,
                    type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            _,
        ) => type_reference_matches(program, *actual_base, expected, type_parameters, bindings),
        (
            _,
            TypeReferenceNode::Constrained {
                base_type: expected_base,
                ..
            },
        ) => type_reference_matches(program, actual, *expected_base, type_parameters, bindings),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: expected_element,
                length: expected_length,
            },
        ) => {
            actual_length == expected_length
                && type_reference_matches(
                    program,
                    *actual_element,
                    *expected_element,
                    type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: expected_element,
            },
        ) => type_reference_matches(
            program,
            *actual_element,
            *expected_element,
            type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                name: actual_name,
            },
            TypeReferenceNode::Named {
                symbol: expected_symbol,
                name: expected_name,
            },
        ) => {
            (actual_symbol.is_valid() && actual_symbol == expected_symbol)
                || actual_name == expected_name
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_symbol,
                base_name: actual_name,
                arguments: actual_arguments,
            },
            TypeReferenceNode::Generic {
                base_symbol: expected_symbol,
                base_name: expected_name,
                arguments: expected_arguments,
            },
        ) => {
            ((actual_symbol.is_valid() && actual_symbol == expected_symbol)
                || actual_name == expected_name)
                && type_reference_spans_match(
                    program,
                    *actual_arguments,
                    *expected_arguments,
                    type_parameters,
                    bindings,
                )
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        _ => false,
    }
}

fn type_reference_spans_match(
    program: &TypedTrees,
    actual: omega_core::arena::HandleSpan<TypeReferenceHandle>,
    expected: omega_core::arena::HandleSpan<TypeReferenceHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
) -> bool {
    let actual = program.type_reference_table.type_reference_handles(actual);
    let expected = program
        .type_reference_table
        .type_reference_handles(expected);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            type_reference_matches(program, *actual, *expected, type_parameters, bindings)
        })
}

fn expected_type_parameter<'a>(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    type_parameters: &'a [TypeParameter],
) -> Option<&'a TypeParameter> {
    match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => type_parameters.iter().find(|parameter| {
            (symbol.is_valid() && parameter.symbol == *symbol) || parameter.name == *name
        }),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn type_references_equal(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    type_reference_matches(program, left, right, &[], &mut Vec::new())
}

fn expression_type_reference_for_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
) -> Option<TypeReferenceHandle> {
    let (state_symbol, statement_index) = match origin {
        CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } => (state_symbol, statement_index),
        CheckedValueOrigin::NestedExpression { .. }
        | CheckedValueOrigin::MachineDecrease { .. }
        | CheckedValueOrigin::MachineOwnedDataInitializer { .. } => return None,
    };
    expression_type_reference_in_state(program, state_symbol, statement_index, expression)
}

fn expression_type_reference_in_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, *inner)
        }
        ExpressionNode::Name(path) => {
            symbol_type_reference_in_state(program, state_symbol, statement_index, path.symbol)
        }
        ExpressionNode::Member(member) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            member.receiver,
        )
        .and_then(|receiver| field_type_reference(program, receiver, member.member_symbol)),
        ExpressionNode::Indexed(indexed) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            indexed.collection,
        )
        .and_then(|collection| indexed_element_type_reference(program, collection)),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => None,
    }
}

fn symbol_type_reference_in_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }

    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| local_type_reference_before_statement(program, state, statement_index, symbol))
}

fn local_type_reference_before_statement(
    program: &TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let omega_typed_trees::statement::StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == symbol).then_some(local.type_reference)
        })
}

fn field_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    field_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        }
        | TypeReferenceNode::Named {
            symbol: base_symbol,
            name: base_name,
        } => program
            .data_definitions()
            .iter()
            .find(|data| {
                (base_symbol.is_valid() && data.symbol == *base_symbol) || data.name == *base_name
            })
            .and_then(|data| data_field_type_reference(program, data, field_symbol)),
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_field_type_reference(
    program: &TypedTrees,
    data: &omega_typed_trees::data::DataDefinition,
    field_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !field_symbol.is_valid() {
        return None;
    }
    program.data_members(data).iter().find_map(|member| {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == field_symbol).then_some(field.type_reference)
    })
}

fn indexed_element_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => indexed_element_type_reference(program, *referee),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => None,
    }
}
