//! Readonly operator-use discovery and operand-directed selection.
//!
//! Preparation and checking share the same authored origins, candidate order,
//! generational handles and arithmetic policies. This owner discovers uses;
//! the transforming stage binds executable calls and publishes proof evidence.

use std::collections::HashSet;

use arena::Arena;
use checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedNamedOperatorUseFact, CheckedOperatorCandidateFact,
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
    CheckedValueFacts, CheckedValueOrigin,
};
use language_core::operator_spelling::OperatorSpelling;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::FloatFormat;
use symbols::SymbolHandle;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
};
use typed_trees::operator::{
    SelectedTraitOperatorMeaning, SpelledOperator, resolve_spelling_for_operands,
    selected_trait_operator_meanings,
};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use super::receiver::expression_type_reference_for_origin;
use typed_trees::operator::binary_operator_spelling;

/// Retain meanings independent of flow-domain admission before specialization
/// copies authored occurrences. Generic implementations select requirements,
/// never fabricated closed maps. Root/domain candidates remain deferred: only
/// checking can decide which domain memberships hold at their use sites.
pub fn collect_pre_specialization_operator_selections(
    program: &dyn crate::CheckingInput,
) -> Result<
    Vec<(
        ExpressionHandle,
        crate::selection_resolution::AuthoredSelectionResolutionTarget,
    )>,
    diagnostics::Diagnostic,
> {
    use crate::selection_resolution::AuthoredSelectionResolutionTarget;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;

    let mut selections = Vec::new();
    for operation in program.expression_table.selected_index_operations() {
        if let Some(target) = crate::selection_resolution::declaration_target(operation.operator) {
            selections.push((operation.expression, target));
        }
    }
    let mut diagnostic = None;
    program.for_each_machine_state_contract_frame(|machine, state, contracts, _| {
        let mut expressions = Vec::new();
        for contract in contracts {
            crate::expression_traversal::collect_contract_facts(program, contract.facts, &mut expressions);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::expression_traversal::collect_statement_expression_trees(program, statement, &mut expressions);
        }
        for expression in expressions {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                continue;
            };
            let Some(spelling) = binary_operator_spelling(binary.operator) else {
                continue;
            };
            let operand_types = [binary.left, binary.right].map(|operand| {
                crate::expression_result_type_reference(program, machine, state, operand)
            });
            if operand_types.iter().all(Option::is_none) {
                continue;
            }
            let bound_meanings = typed_trees::operator::bound_trait_operator_meanings(
                program, machine.symbol, spelling, &operand_types,
            );
            let selected_meanings = selected_trait_operator_meanings(
                program, machine.symbol, spelling, &operand_types,
            );
            let target = match (bound_meanings.as_slice(), selected_meanings.as_slice()) {
                ([meaning], _) => AuthoredSelectionResolutionTarget::Declaration(meaning.requirement.symbol),
                ([], [meaning]) => AuthoredSelectionResolutionTarget::Declaration(meaning.requirement.symbol),
                ([], []) => {
                    // No implementation candidate is not proof that an open
                    // carrier has primitive meaning. Specialization settles
                    // those uses; only known primitive carriers freeze here.
                    if operand_types.iter().flatten().any(|reference| {
                        program.primitive_type_reference(*reference).is_none()
                    }) {
                        continue;
                    }
                    if !resolve_spelling_for_operands(
                        program, spelling, &operand_types,
                        program.expression_table.source_span(expression),
                    ).is_empty() || !typed_trees::closed_numeric::has_builtin_anonymous_operands(program, expression, spelling) {
                        continue;
                    }
                    AuthoredSelectionResolutionTarget::Intrinsic(AuthoredDeclarationSelectionIntrinsic::BuiltinOperator)
                }
                _ => {
                    diagnostic.get_or_insert_with(|| diagnostics::Diagnostic::error(format!(
                        "operator `{}` on machine `{}` has multiple explicitly selected implementation meanings",
                        spelling.symbol(), machine.name,
                    )));
                    continue;
                }
            };
            selections.push((expression, target));
        }
    });
    match diagnostic {
        Some(diagnostic) => Err(diagnostic),
        None => Ok(selections),
    }
}

/// The operator and named uses with their candidates, without the requirement
/// uses and declaration contracts `build_operator_facts` adds.
pub fn build_operator_uses(
    program: &dyn crate::CheckingInput,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    let mut uses = Arena::default();
    let mut named_uses = Arena::default();
    let mut candidates = Arena::default();
    // Membership only: the walk never iterates this set, so its handle and
    // origin keys take the integer hasher instead of SipHash.
    let mut seen = HashSet::with_hasher(symbols::BuildSymbolHasher);

    for (_, value) in values.values.iter() {
        // The recursive walk already visits every child with its enclosing
        // declaration/state context. Nested value rows retain parentage, not
        // an independent operand-typing context; seeding them again would
        // resolve the same occurrence with wildcard operands.
        if matches!(value.origin, CheckedValueOrigin::NestedExpression { .. }) {
            continue;
        }
        collect_expression_operator_use(
            program,
            value.expression,
            value.origin,
            &mut seen,
            &mut uses,
            &mut named_uses,
            &mut candidates,
        );
    }

    CheckedOperatorFacts::with_roots(uses, named_uses, candidates)
}

fn collect_expression_operator_use(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut HashSet<(ExpressionHandle, CheckedValueOrigin), symbols::BuildSymbolHasher>,
    uses: &mut Arena<CheckedOperatorUseFact>,
    named_uses: &mut Arena<CheckedNamedOperatorUseFact>,
    candidates: &mut Arena<CheckedOperatorCandidateFact>,
) {
    if !expression.is_valid() || !seen.insert((expression, origin)) {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_expression_operator_use(
                program,
                dispatch.subject,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            let subject_type =
                expression_type_reference_for_origin(program, dispatch.subject, origin);
            let floating_subject = subject_type
                .and_then(|reference| crate::unwrapped_type_reference(program, reference))
                .and_then(|reference| program.primitive_type_reference(reference))
                .or_else(|| crate::match_subject_primitive_type(program, dispatch))
                .is_some_and(|primitive| {
                    matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                });
            for (ordinal, arm) in program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .enumerate()
            {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    if floating_subject && let Ok(ordinal) = u32::try_from(ordinal) {
                        let source_arm = dispatch.arms.handle_at(ordinal);
                        let mut equality = operator_use_fact(
                            program,
                            expression,
                            origin,
                            OperatorSpelling::Equal,
                            &[
                                subject_type,
                                expression_type_reference_for_origin(program, pattern, origin),
                            ],
                            candidates,
                        );
                        equality.occurrence =
                            checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm };
                        uses.append(equality);
                    }
                }
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    collect_expression_operator_use(
                        program, pattern, origin, seen, uses, named_uses, candidates,
                    );
                }
                collect_expression_operator_use(
                    program, arm.value, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => collect_expression_operator_use(
            program,
            atomic.value,
            origin,
            seen,
            uses,
            named_uses,
            candidates,
        ),
        ExpressionNode::Indexed(indexed) => {
            let spelling = indexed_operator_spelling(program, indexed.index);
            let operand_types = indexed_operand_types(program, indexed, origin);
            uses.append(operator_use_fact(
                program,
                expression,
                origin,
                spelling,
                &operand_types,
                candidates,
            ));
            collect_expression_operator_use(
                program,
                indexed.collection,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program,
                indexed.index,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_operator_use(
                    program, *value, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            // Resolve from the complete recoverable operand tuple. A
            // contextual literal or nested binary may not expose an
            // independent left-hand type, while the companion operand still
            // fixes the exact overload and arithmetic policy.
            if let Some(spelling) = binary_operator_spelling(binary.operator)
                && let operand_types = [
                    expression_type_reference_for_origin(program, binary.left, origin),
                    expression_type_reference_for_origin(program, binary.right, origin),
                ]
                && operand_types.iter().any(Option::is_some)
                && let Some(fact) = binary_operator_use_fact(
                    program,
                    expression,
                    origin,
                    spelling,
                    &operand_types,
                    candidates,
                )
            {
                uses.append(fact);
            }
            collect_expression_operator_use(
                program,
                binary.left,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program,
                binary.right,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_operator_use(
                program, cast.value, origin, seen, uses, named_uses, candidates,
            );
        }
        ExpressionNode::Call(call) => {
            if let Some(named_use) = named_operator_use_fact(program, expression, origin, call) {
                named_uses.append(named_use);
            }
            collect_expression_operator_use(
                program,
                call.receiver,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_operator_use(
                    program, *argument, origin, seen, uses, named_uses, candidates,
                );
            }
        }
        ExpressionNode::Member(member) => {
            collect_expression_operator_use(
                program,
                member.receiver,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Borrow(inner) => {
            collect_expression_operator_use(
                program,
                inner.target,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_operator_use(
                program,
                unary.operand,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
        }
        ExpressionNode::Range(range) => {
            collect_expression_operator_use(
                program,
                range.start,
                origin,
                seen,
                uses,
                named_uses,
                candidates,
            );
            collect_expression_operator_use(
                program, range.end, origin, seen, uses, named_uses, candidates,
            );
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
                    named_uses,
                    candidates,
                );
            }
        }
        ExpressionNode::SavedSubject(_) | typed_trees::literal_expression_pattern!() => {}
    }
}

/// Retain the selected identity of one unambiguously resolved named operator
/// call. Every boundary-operator provider path consumes this common fact;
/// numeric policy adaptation remains specific to the normalized float surface.
/// Policy adaptation is operand-driven for float-returning F32/F64 operations;
/// classification and destination-owned float-to-integer conversions carry no
/// float result adapter.
fn named_operator_use_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedOperatorUseFact> {
    let operator = typed_trees::operator::resolve_named_expression_call(program, call)
        .or_else(|| {
            // Token-machine overloads use the same complete operand matching as
            // their spelled form. Namespace/arity candidates alone cannot choose
            // between, for example, the f32 and f64 requirements.
            let operands: Vec<_> = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|operand| expression_type_reference_for_origin(program, *operand, origin))
                .collect();
            let candidates = typed_trees::operator::named_expression_call_candidates(program, call);
            let mut matching = candidates.into_iter().filter(|candidate| {
                program
                    .machine_token_binding_by_symbol(candidate.symbol)
                    .is_some()
                    && candidate.spelling.is_some_and(|spelling| {
                        typed_trees::operator::resolve_spelling_for_operands(
                            program,
                            spelling,
                            &operands,
                            program.expression_table.source_span(expression),
                        )
                        .iter()
                        .any(|selected| selected.operator.symbol == candidate.symbol)
                    })
            });
            let selected = matching.next()?;
            matching.next().is_none().then_some(selected)
        })
        .filter(|operator| {
            program.symbols.source_reference_may_select_symbol(
                program.expression_table.source_span(expression),
                operator.symbol,
            )
        })?;
    let path = program.operator_path_members(operator.name);
    // Namespace shape classifies a numeric policy adapter; it does not
    // determine whether an ordinary named operator use exists.
    let format = match path {
        [namespace, requirement] => match namespace.as_str() {
            "F32" => Some(FloatFormat::BINARY32),
            "F64" => Some(FloatFormat::BINARY64),
            "Float" => typed_trees::operator::primitive_float_binary_semantics(program, operator)
                .map(|(_, format)| match format {
                    numerics::literals::FloatFormat::F32 => FloatFormat::BINARY32,
                    numerics::literals::FloatFormat::F64 => FloatFormat::BINARY64,
                }),
            "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64"
                if matches!(requirement.as_str(), "from_f32" | "from_f64") =>
            {
                None
            }
            _ => None,
        },
        _ => None,
    };
    let policy_adapter = match format {
        Some(format)
            if matches!(
                (
                    format,
                    program.primitive_type_reference(operator.return_type)
                ),
                (FloatFormat::BINARY32, Some(PrimitiveType::F32))
                    | (FloatFormat::BINARY64, Some(PrimitiveType::F64))
            ) =>
        {
            named_float_policy_adapter(program, call, origin, format)
        }
        _ => CheckedArithmeticPolicyAdapter::None,
    };

    Some(CheckedNamedOperatorUseFact {
        expression,
        origin,
        selected_operator_symbol: operator.symbol,
        policy_adapter,
    })
}

pub(super) fn named_float_policy_adapter(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
    origin: CheckedValueOrigin,
    format: FloatFormat,
) -> CheckedArithmeticPolicyAdapter {
    // Numeric `as` selects a raw conversion beneath its authored cast. Source
    // arithmetic policy governs the already-evaluated operand; the cast's
    // exact formation and result qualification own this conversion's use.
    if call.exact_coercion_target.is_valid() {
        return CheckedArithmeticPolicyAdapter::None;
    }
    let operands: Vec<_> = program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .map(|argument| expression_type_reference_for_origin(program, *argument, origin))
        .collect();
    crate::value_custody::expression_types::operand_arithmetic_domain(program, &operands)
        .map_or(CheckedArithmeticPolicyAdapter::None, |domain| {
            float_policy_adapter(format, domain)
        })
}

/// Records a spelled binary use when spelled candidates match the left
/// operand type. Root-only candidate sets resolve immediately (root spelled
/// operators are the declared surface of the builtin operation). Any
/// domain-owned candidate defers to the binding-site selection pass
/// (`select_pending_domain_operator_meanings`), which reads declarations,
/// mints, and signature `requires` but never flow facts.

/// Candidate count and arena span for a resolved spelling candidate set.
fn checked_candidate_span(
    program: &dyn crate::CheckingInput,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
    candidates: &[SpelledOperator<'_>],
) -> (usize, arena::HandleSpan<CheckedOperatorCandidateFact>) {
    (
        candidates.len(),
        candidate_facts.insert_many(
            candidates
                .iter()
                .map(|candidate| checked_operator_candidate(program, candidate)),
        ),
    )
}

/// One operator use record; both use-fact builders mint the same shape.
/// Domain-pending and unique-candidate statuses outrank every caller's own
/// fallback; `None` defers to that fallback.
fn domain_or_unique_operator_status(
    candidates: &[SpelledOperator<'_>],
) -> Option<(CheckedOperatorResolutionStatus, SymbolHandle)> {
    if candidates
        .iter()
        .any(|candidate| candidate.domain.is_some())
    {
        Some((
            CheckedOperatorResolutionStatus::DomainPending,
            SymbolHandle::invalid(),
        ))
    } else if let [candidate] = candidates {
        Some((
            CheckedOperatorResolutionStatus::Resolved,
            candidate.operator.symbol,
        ))
    } else {
        None
    }
}

fn operator_use_record(
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    selected_operator_symbol: SymbolHandle,
    candidates: arena::HandleSpan<CheckedOperatorCandidateFact>,
    candidate_count: usize,
    status: CheckedOperatorResolutionStatus,
) -> CheckedOperatorUseFact {
    CheckedOperatorUseFact {
        expression,
        origin,
        occurrence: Default::default(),
        spelling,
        policy_adapter: CheckedArithmeticPolicyAdapter::None,
        selected_operator_symbol,
        candidates,
        candidate_count,
        status,
    }
}

fn binary_operator_use_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> Option<CheckedOperatorUseFact> {
    let trait_candidates = origin_machine_symbol(origin).map_or_else(Vec::new, |machine_symbol| {
        selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
    });
    if !trait_candidates.is_empty() {
        return Some(trait_operator_use_fact(
            program,
            expression,
            origin,
            spelling,
            trait_candidates,
            candidate_facts,
        ));
    }
    let candidates = resolve_spelling_for_operands(
        program,
        spelling,
        operand_types,
        program.expression_table.source_span(expression),
    );
    if candidates.is_empty() {
        return None;
    }

    let (candidate_count, candidate_span) =
        checked_candidate_span(program, candidate_facts, &candidates);
    let (status, selected_operator_symbol) = domain_or_unique_operator_status(&candidates)
        .unwrap_or((
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        ));

    Some(CheckedOperatorUseFact {
        expression,
        origin,
        occurrence: Default::default(),
        spelling,
        policy_adapter: arithmetic_policy_adapter(program, spelling, operand_types),
        selected_operator_symbol,
        candidates: candidate_span,
        candidate_count,
        status,
    })
}

fn indexed_operator_spelling(
    program: &dyn crate::CheckingInput,
    index: ExpressionHandle,
) -> OperatorSpelling {
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

/// Read collection and index operand types, including both authored range bounds.
pub fn indexed_operand_types(
    program: &dyn crate::CheckingInput,
    indexed: &TableIndexedExpression,
    origin: CheckedValueOrigin,
) -> Vec<Option<TypeReferenceHandle>> {
    let mut operand_types = vec![expression_type_reference_for_origin(
        program,
        indexed.collection,
        origin,
    )];
    match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(range) => {
            operand_types.push(expression_type_reference_for_origin(
                program,
                range.start,
                origin,
            ));
            operand_types.push(expression_type_reference_for_origin(
                program, range.end, origin,
            ));
        }
        _ => operand_types.push(expression_type_reference_for_origin(
            program,
            indexed.index,
            origin,
        )),
    }
    operand_types
}

/// Records the typed-trees resolution outcome for one use site as checked
/// evidence. Every known operand position participates, including both range
/// bounds for `[..]`.
fn operator_use_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let trait_candidates = origin_machine_symbol(origin).map_or_else(Vec::new, |machine_symbol| {
        selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
    });
    if !trait_candidates.is_empty() {
        return trait_operator_use_fact(
            program,
            expression,
            origin,
            spelling,
            trait_candidates,
            candidate_facts,
        );
    }
    let use_site = program.expression_table.source_span(expression);
    let mut candidates = if matches!(spelling, OperatorSpelling::Index | OperatorSpelling::Range) {
        typed_trees::operator::resolve_indexed_spelling_for_operands(
            program,
            spelling,
            operand_types,
            use_site,
        )
    } else {
        resolve_spelling_for_operands(program, spelling, operand_types, use_site)
    };
    if spelling == OperatorSpelling::Index {
        let collection = operand_types.first().copied().flatten();
        candidates.retain(|candidate| {
            typed_trees::operator::indexed_operator_governs_occurrence(
                program,
                spelling,
                candidate.operator,
                expression,
                collection,
            )
        });
    }
    let (candidate_count, candidate_span) =
        checked_candidate_span(program, candidate_facts, &candidates);
    let (status, selected_operator_symbol) =
        if let Some(status) = domain_or_unique_operator_status(&candidates) {
            status
        } else if candidates.is_empty() {
            let indexed_spelling = matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Indexed(indexed)
                    if indexed_operator_spelling(program, indexed.index) == spelling
            );
            let builtin_geometry = operand_types
                .first()
                .copied()
                .flatten()
                .and_then(|reference| crate::unwrapped_type_reference(program, reference))
                .is_some_and(|reference| {
                    matches!(
                        program.type_reference_table.type_reference(reference),
                        typed_trees::types::TypeReferenceNode::FixedArray { .. }
                            | typed_trees::types::TypeReferenceNode::Slice { .. }
                    )
                });
            let builtin_indexed = indexed_spelling
                && origin_machine_symbol(origin).is_some_and(|machine_symbol| {
                    (builtin_geometry
                        && typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            machine_symbol,
                            expression,
                            spelling,
                            operand_types,
                        ))
                        || typed_trees::operator::declined_copy_index_is_builtin_place(
                            program,
                            machine_symbol,
                            expression,
                            operand_types,
                        )
                });
            (
                if builtin_indexed {
                    CheckedOperatorResolutionStatus::BuiltinFallback
                } else {
                    CheckedOperatorResolutionStatus::Missing
                },
                SymbolHandle::invalid(),
            )
        } else {
            (
                CheckedOperatorResolutionStatus::Ambiguous,
                SymbolHandle::invalid(),
            )
        };

    operator_use_record(
        expression,
        origin,
        spelling,
        selected_operator_symbol,
        candidate_span,
        candidate_count,
        status,
    )
}

fn origin_machine_symbol(origin: CheckedValueOrigin) -> Option<SymbolHandle> {
    match origin {
        CheckedValueOrigin::MachineDecrease { machine_symbol, .. }
        | CheckedValueOrigin::MachineOwnedDataInitializer { machine_symbol, .. }
        | CheckedValueOrigin::StateStatement { machine_symbol, .. } => Some(machine_symbol),
        CheckedValueOrigin::NestedExpression { .. } => None,
    }
}

fn trait_operator_use_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    spelling: OperatorSpelling,
    candidates: Vec<SelectedTraitOperatorMeaning<'_>>,
    candidate_facts: &mut Arena<CheckedOperatorCandidateFact>,
) -> CheckedOperatorUseFact {
    let candidate_count = candidates.len();
    let candidate_span = candidate_facts.insert_many(candidates.iter().map(|candidate| {
        CheckedOperatorCandidateFact::trait_backed(
            candidate.requirement.symbol,
            candidate.application.declaration,
            candidate.row.realization_machine,
            candidate.row.realization_state,
            candidate.application.report_fingerprint,
            candidate.application.commitment,
        )
        .with_signature(
            program
                .state_signature_parameters(candidate.requirement)
                .first()
                .map(|parameter| parameter.type_reference)
                .unwrap_or_else(TypeReferenceHandle::invalid),
            candidate.requirement.return_type,
            candidate.requirement.contracts,
            program
                .trait_type_parameters(candidate.trait_definition)
                .len()
                + program
                    .state_signature_type_parameters(candidate.requirement)
                    .len(),
            program
                .state_signature_parameters(candidate.requirement)
                .len(),
            candidate.trait_definition.is_boundary,
        )
    }));
    let (status, selected_operator_symbol) = if let [candidate] = candidates.as_slice() {
        (
            CheckedOperatorResolutionStatus::Resolved,
            candidate.requirement.symbol,
        )
    } else {
        (
            CheckedOperatorResolutionStatus::Ambiguous,
            SymbolHandle::invalid(),
        )
    };

    operator_use_record(
        expression,
        origin,
        spelling,
        selected_operator_symbol,
        candidate_span,
        candidate_count,
        status,
    )
}

pub(super) fn arithmetic_policy_adapter(
    program: &dyn crate::CheckingInput,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> CheckedArithmeticPolicyAdapter {
    if !matches!(
        spelling,
        OperatorSpelling::Add
            | OperatorSpelling::Subtract
            | OperatorSpelling::Multiply
            | OperatorSpelling::Divide
    ) {
        return CheckedArithmeticPolicyAdapter::None;
    }

    let mut format = None;
    for type_reference in operand_types.iter().flatten().copied() {
        let candidate_format = match program.primitive_type_reference(type_reference) {
            Some(PrimitiveType::F32) => Some(FloatFormat::BINARY32),
            Some(PrimitiveType::F64) => Some(FloatFormat::BINARY64),
            _ => None,
        };
        if let Some(candidate_format) = candidate_format {
            if format.is_some_and(|format| format != candidate_format) {
                return CheckedArithmeticPolicyAdapter::None;
            }
            format = Some(candidate_format);
        }
    }
    let Some(format) = format else {
        return CheckedArithmeticPolicyAdapter::None;
    };
    crate::value_custody::expression_types::operand_arithmetic_domain(program, operand_types)
        .map_or(CheckedArithmeticPolicyAdapter::None, |domain| {
            float_policy_adapter(format, domain)
        })
}

fn float_policy_adapter(
    format: FloatFormat,
    domain: ArithmeticDomain,
) -> CheckedArithmeticPolicyAdapter {
    match domain {
        ArithmeticDomain::Saturating => {
            CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format }
        }
        ArithmeticDomain::Trapping => {
            CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { format }
        }
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {
            CheckedArithmeticPolicyAdapter::None
        }
    }
}

/// Project a resolved spelling candidate's exact declaration and signature.
pub fn checked_operator_candidate(
    program: &dyn crate::CheckingInput,
    candidate: &SpelledOperator<'_>,
) -> CheckedOperatorCandidateFact {
    let fact = if let Some(domain) = candidate.domain {
        CheckedOperatorCandidateFact::domain(candidate.operator.symbol, domain.symbol)
    } else {
        CheckedOperatorCandidateFact::root(candidate.operator.symbol)
    };
    fact.with_signature(
        program
            .operator_parameters(candidate.operator)
            .first()
            .map(|parameter| parameter.type_reference)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        candidate.operator.return_type,
        candidate.operator.contracts,
        program.operator_type_parameters(candidate.operator).len(),
        program.operator_parameters(candidate.operator).len(),
        candidate.operator.is_boundary,
    )
}
