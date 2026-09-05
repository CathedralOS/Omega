//! Path-conditioned crash-route coverage.
//!
//! A state reached only through a retained incoming guard has that guard in
//! every execution path reaching its body. The same is true of the negated
//! guard on a fallthrough/continuation edge. When that exact normalized
//! predicate is one alternative in a same-cause published bucket, the site's
//! derived condition implies the bucket guard. This is checked implementation
//! evidence and never enters the public contract fingerprint.

use checked_trees::{CheckFacts, CrashRouteGuard};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(crate) fn infer_path_conditioned_guard_coverage(
    program: &TypedTrees,
    facts: &mut CheckFacts,
    incoming_guards: &super::ranges::incoming_guards::IncomingGuardIndex,
) {
    let content_conservation = validation::build_content_conservation_plans(program);
    for machine in program.machines() {
        let incoming = incoming_guards.for_machine(machine.symbol);
        let parameter_names = program
            .machine_states(machine)
            .first()
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(contract_index) = facts
            .contract_plans
            .machines
            .iter()
            .position(|contract| contract.machine == machine.symbol)
        else {
            continue;
        };
        let crash_plan = &facts.contract_plans.machines[contract_index].crash;
        if crash_plan.checked_sites().is_empty() && crash_plan.checked_calls().is_empty() {
            continue;
        }

        let checked_sites = crash_plan
            .checked_sites()
            .iter()
            .map(|site| {
                let mut covering = site.guard_covering_buckets().to_vec();
                let applicable_guards = incoming
                    .iter()
                    .filter(|guard| guard.applies_at(site.location().state()))
                    .collect::<Vec<_>>();
                let path_guard_conjuncts = applicable_guards
                    .iter()
                    .map(|guard| {
                        crate::facts::canonical_crash_path_predicate(
                            program,
                            guard.guard(),
                            guard.is_negated(),
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut path_predicates = Vec::new();
                let mut order_relations = Vec::new();
                let mut integer_disequalities = Vec::new();
                for guard in applicable_guards {
                    collect_structural_guard_consequences(
                        program,
                        guard.guard(),
                        guard.is_negated(),
                        &parameter_names,
                        &content_conservation,
                        &mut path_predicates,
                    );
                    collect_integer_order_relations(
                        program,
                        guard.guard(),
                        guard.is_negated(),
                        &parameter_names,
                        &content_conservation,
                        &mut order_relations,
                        &mut integer_disequalities,
                    );
                }
                push_transitive_integer_order_consequences(
                    program,
                    &mut order_relations,
                    &integer_disequalities,
                    &parameter_names,
                    &content_conservation,
                    &mut path_predicates,
                );
                path_predicates.sort();
                path_predicates.dedup();

                for (bucket_id, bucket) in crash_plan.published_with_ids() {
                    if bucket.cause() != site.cause()
                        || covering.contains(&bucket_id)
                        || !bucket.alternative_guards().iter().any(|route| {
                            matches!(route, CrashRouteGuard::Predicate(predicate)
                                if path_predicates.contains(predicate))
                        })
                    {
                        continue;
                    }
                    covering.push(bucket_id);
                }

                site.clone()
                    .with_path_guard_conjuncts(path_guard_conjuncts)
                    .with_path_guard_consequences(path_predicates)
                    .with_guard_covering_buckets(covering)
            })
            .collect();
        let checked_calls = crash_plan
            .checked_calls()
            .iter()
            .map(|call| {
                let applicable_guards = incoming
                    .iter()
                    .filter(|guard| guard.applies_at(call.location().state()))
                    .collect::<Vec<_>>();
                let path_guard_conjuncts = applicable_guards
                    .iter()
                    .map(|guard| {
                        crate::facts::canonical_crash_path_predicate(
                            program,
                            guard.guard(),
                            guard.is_negated(),
                            &parameter_names,
                            &content_conservation,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut path_guard_consequences = Vec::new();
                let mut order_relations = Vec::new();
                let mut integer_disequalities = Vec::new();
                for guard in applicable_guards {
                    collect_structural_guard_consequences(
                        program,
                        guard.guard(),
                        guard.is_negated(),
                        &parameter_names,
                        &content_conservation,
                        &mut path_guard_consequences,
                    );
                    collect_integer_order_relations(
                        program,
                        guard.guard(),
                        guard.is_negated(),
                        &parameter_names,
                        &content_conservation,
                        &mut order_relations,
                        &mut integer_disequalities,
                    );
                }
                push_transitive_integer_order_consequences(
                    program,
                    &mut order_relations,
                    &integer_disequalities,
                    &parameter_names,
                    &content_conservation,
                    &mut path_guard_consequences,
                );
                call.clone()
                    .with_path_guard_conjuncts(path_guard_conjuncts)
                    .with_path_guard_consequences(path_guard_consequences)
            })
            .collect();
        facts.contract_plans.machines[contract_index].crash = crash_plan
            .clone()
            .with_checked_sites(checked_sites)
            .expect("path-conditioned coverage retains valid checked-site identity")
            .with_checked_calls(checked_calls)
            .expect("path-conditioned coverage retains valid checked-call identity");
    }
}

pub(crate) fn check_published_ceiling_coverage(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for caller in
        facts.contract_plans.machines.iter().filter(|plan| {
            plan.crash.interface() == checked_trees::CrashInterface::PublishedCeiling
        })
    {
        let caller_machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == caller.machine);
        let caller_name = caller_machine
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        for site in caller
            .crash
            .checked_sites()
            .iter()
            .filter(|site| site.guard_covering_buckets().is_empty())
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{caller_name}` has an uncovered {:?} crash at statement {}; publish a same-cause `crashes` route whose guard covers this site",
                site.cause(),
                site.location().statement_ordinal(),
            )));
        }
        for call in caller.crash.checked_calls() {
            for surviving in call.surviving_buckets() {
                let covered = surviving.alternative_guards().iter().all(|route| {
                    caller.crash.published().iter().any(|published| {
                        published.cause() == surviving.cause()
                            && published.alternative_guards().iter().any(|cover| {
                                call_route_guard_covers(
                                    cover,
                                    route,
                                    call.path_guard_consequences(),
                                )
                            })
                    })
                });
                if covered {
                    continue;
                }
                let target_name = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == call.target_machine())
                    .map(|machine| machine.name.as_str())
                    .unwrap_or("<unknown>");
                diagnostics.push(Diagnostic::error(format!(
                    "call from `{caller_name}` to `{target_name}` at statement {} call {} has an uncovered {:?} crash route; publish a same-cause route whose guard covers this invocation",
                    call.location().statement_ordinal(),
                    call.location().call_ordinal(),
                    surviving.cause(),
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn call_route_guard_covers(
    published: &CrashRouteGuard,
    surviving: &CrashRouteGuard,
    path_guard_consequences: &[checked_trees::CrashPredicateIdentity],
) -> bool {
    match published {
        CrashRouteGuard::Truth => true,
        CrashRouteGuard::Predicate(published) => {
            matches!(surviving, CrashRouteGuard::Predicate(surviving) if surviving == published)
                || path_guard_consequences.contains(published)
        }
    }
}

/// Retain only propositional consequences that follow structurally from one
/// incoming predicate. A positive conjunction entails each conjunct; a
/// negated disjunction entails each negated disjunct; logical negation flips
/// polarity; equality/inequality normalize under negation; and comparisons
/// retain operand-reversed equivalents. Everything else remains an opaque
/// canonical atom. This is deliberately incomplete but sound, and does not
/// rewrite public routes.
fn collect_structural_guard_consequences(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<checked_trees::CrashPredicateIdentity>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

    output.push(crate::facts::canonical_crash_path_predicate(
        program,
        expression,
        negated,
        parameter_names,
        content_conservation,
    ));
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            collect_structural_guard_consequences(
                program,
                unary.operand,
                !negated,
                parameter_names,
                content_conservation,
                output,
            );
        }
        ExpressionNode::Binary(binary)
            if (!negated && binary.operator == BinaryOperator::And)
                || (negated && binary.operator == BinaryOperator::Or) =>
        {
            collect_structural_guard_consequences(
                program,
                binary.left,
                negated,
                parameter_names,
                content_conservation,
                output,
            );
            collect_structural_guard_consequences(
                program,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                output,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let operand_and_literal = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                _ => None,
            };
            if let Some((operand, literal)) = operand_and_literal {
                // Normalize `x == true`, `x == false`, `x != true`, and
                // `x != false`, including a negated/fallthrough relation, to
                // the exact polarity of x that the edge establishes.
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                collect_structural_guard_consequences(
                    program,
                    operand,
                    equality_is_negated == literal,
                    parameter_names,
                    content_conservation,
                    output,
                );
            }
            let operands_are_integers =
                comparison_operands_are_integers(program, binary.left, binary.right);
            let normalized = normalized_comparison(binary.operator, negated, operands_are_integers)
                .expect("equality operators are comparisons");
            push_comparison_consequences(
                program,
                normalized,
                binary.left,
                binary.right,
                operands_are_integers,
                parameter_names,
                content_conservation,
                output,
            );
        }
        ExpressionNode::Binary(binary) => {
            let normalized = normalized_comparison(
                binary.operator,
                negated,
                comparison_operands_are_integers(program, binary.left, binary.right),
            );
            if let Some(normalized) = normalized {
                push_comparison_consequences(
                    program,
                    normalized,
                    binary.left,
                    binary.right,
                    comparison_operands_are_integers(program, binary.left, binary.right),
                    parameter_names,
                    content_conservation,
                    output,
                );
            }
        }
        _ => {}
    }
}

#[derive(Clone, PartialEq, Eq)]
struct IntegerOrderRelation {
    left_identity: checked_trees::CrashPredicateIdentity,
    right_identity: checked_trees::CrashPredicateIdentity,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    strict: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct IntegerDisequality {
    left_identity: checked_trees::CrashPredicateIdentity,
    right_identity: checked_trees::CrashPredicateIdentity,
}

fn collect_integer_order_relations(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<IntegerOrderRelation>,
    disequalities: &mut Vec<IntegerDisequality>,
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};

    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            collect_integer_order_relations(
                program,
                unary.operand,
                !negated,
                parameter_names,
                content_conservation,
                output,
                disequalities,
            );
        }
        ExpressionNode::Binary(binary)
            if (!negated && binary.operator == BinaryOperator::And)
                || (negated && binary.operator == BinaryOperator::Or) =>
        {
            collect_integer_order_relations(
                program,
                binary.left,
                negated,
                parameter_names,
                content_conservation,
                output,
                disequalities,
            );
            collect_integer_order_relations(
                program,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                output,
                disequalities,
            );
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let operand_and_literal = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                _ => None,
            };
            if let Some((operand, literal)) = operand_and_literal {
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                collect_integer_order_relations(
                    program,
                    operand,
                    equality_is_negated == literal,
                    parameter_names,
                    content_conservation,
                    output,
                    disequalities,
                );
            }
            collect_normalized_integer_order_relation(
                program,
                binary.operator,
                binary.left,
                binary.right,
                negated,
                parameter_names,
                content_conservation,
                output,
                disequalities,
            );
        }
        ExpressionNode::Binary(binary) => collect_normalized_integer_order_relation(
            program,
            binary.operator,
            binary.left,
            binary.right,
            negated,
            parameter_names,
            content_conservation,
            output,
            disequalities,
        ),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_normalized_integer_order_relation(
    program: &TypedTrees,
    operator: typed_trees::expression::BinaryOperator,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<IntegerOrderRelation>,
    disequalities: &mut Vec<IntegerDisequality>,
) {
    use typed_trees::expression::BinaryOperator;

    if !comparison_operands_are_integers(program, left, right) {
        return;
    }
    let Some(normalized) = normalized_comparison(operator, negated, true) else {
        return;
    };
    let mut push = |left, right, strict| {
        let relation = IntegerOrderRelation {
            left_identity: crate::facts::canonical_crash_operand_identity(
                program,
                left,
                parameter_names,
                content_conservation,
            ),
            right_identity: crate::facts::canonical_crash_operand_identity(
                program,
                right,
                parameter_names,
                content_conservation,
            ),
            left,
            right,
            strict,
        };
        if !output.contains(&relation) {
            output.push(relation);
        }
    };
    match normalized {
        BinaryOperator::Less => push(left, right, true),
        BinaryOperator::LessOrEqual => push(left, right, false),
        BinaryOperator::Greater => push(right, left, true),
        BinaryOperator::GreaterOrEqual => push(right, left, false),
        BinaryOperator::Equal => {
            push(left, right, false);
            push(right, left, false);
        }
        BinaryOperator::NotEqual => {
            let disequality = IntegerDisequality {
                left_identity: crate::facts::canonical_crash_operand_identity(
                    program,
                    left,
                    parameter_names,
                    content_conservation,
                ),
                right_identity: crate::facts::canonical_crash_operand_identity(
                    program,
                    right,
                    parameter_names,
                    content_conservation,
                ),
            };
            if !disequalities.contains(&disequality) {
                disequalities.push(disequality);
            }
        }
        _ => unreachable!("normalized comparisons use only comparison operators"),
    }
}

fn push_transitive_integer_order_consequences(
    program: &TypedTrees,
    relations: &mut Vec<IntegerOrderRelation>,
    disequalities: &[IntegerDisequality],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<checked_trees::CrashPredicateIdentity>,
) {
    let strict_refinements = relations
        .iter()
        .filter(|relation| {
            !relation.strict
                && disequalities.iter().any(|disequality| {
                    (disequality.left_identity == relation.left_identity
                        && disequality.right_identity == relation.right_identity)
                        || (disequality.left_identity == relation.right_identity
                            && disequality.right_identity == relation.left_identity)
                })
        })
        .map(|relation| IntegerOrderRelation {
            strict: true,
            ..relation.clone()
        })
        .collect::<Vec<_>>();
    for refinement in strict_refinements {
        if !relations.contains(&refinement) {
            relations.push(refinement);
        }
    }

    loop {
        let existing = relations.clone();
        let mut added = Vec::new();
        for left in &existing {
            for right in &existing {
                if left.right_identity != right.left_identity {
                    continue;
                }
                let relation = IntegerOrderRelation {
                    left_identity: left.left_identity.clone(),
                    right_identity: right.right_identity.clone(),
                    left: left.left,
                    right: right.right,
                    strict: left.strict || right.strict,
                };
                if !relations.contains(&relation) && !added.contains(&relation) {
                    added.push(relation);
                }
            }
        }
        if added.is_empty() {
            break;
        }
        relations.extend(added);
    }

    let nonstrict_equalities = relations
        .iter()
        .filter(|relation| {
            !relation.strict
                && relations.iter().any(|reverse| {
                    !reverse.strict
                        && reverse.left_identity == relation.right_identity
                        && reverse.right_identity == relation.left_identity
                })
        })
        .map(|relation| (relation.left, relation.right))
        .collect::<Vec<_>>();
    for (left, right) in nonstrict_equalities {
        push_comparison_consequences(
            program,
            typed_trees::expression::BinaryOperator::Equal,
            left,
            right,
            true,
            parameter_names,
            content_conservation,
            output,
        );
    }

    for relation in relations {
        push_comparison_consequences(
            program,
            if relation.strict {
                typed_trees::expression::BinaryOperator::Less
            } else {
                typed_trees::expression::BinaryOperator::LessOrEqual
            },
            relation.left,
            relation.right,
            true,
            parameter_names,
            content_conservation,
            output,
        );
    }
}

fn push_comparison_consequences(
    program: &TypedTrees,
    normalized: typed_trees::expression::BinaryOperator,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    operands_are_integers: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    output: &mut Vec<checked_trees::CrashPredicateIdentity>,
) {
    use typed_trees::expression::BinaryOperator;

    let mut consequences = vec![normalized];
    if operands_are_integers {
        match normalized {
            BinaryOperator::Less => {
                consequences.extend([BinaryOperator::LessOrEqual, BinaryOperator::NotEqual]);
            }
            BinaryOperator::Greater => {
                consequences.extend([BinaryOperator::GreaterOrEqual, BinaryOperator::NotEqual]);
            }
            BinaryOperator::Equal => {
                consequences.extend([BinaryOperator::LessOrEqual, BinaryOperator::GreaterOrEqual]);
            }
            _ => {}
        }
    }
    for operator in consequences {
        output.push(crate::facts::canonical_crash_binary_path_predicate(
            program,
            operator,
            left,
            right,
            parameter_names,
            content_conservation,
        ));
        output.push(crate::facts::canonical_crash_binary_path_predicate(
            program,
            reversed_comparison(operator),
            right,
            left,
            parameter_names,
            content_conservation,
        ));
    }
}

fn normalized_comparison(
    operator: typed_trees::expression::BinaryOperator,
    negated: bool,
    operands_have_total_order: bool,
) -> Option<typed_trees::expression::BinaryOperator> {
    use typed_trees::expression::BinaryOperator;

    Some(match (operator, negated) {
        (BinaryOperator::Equal, false) => BinaryOperator::Equal,
        (BinaryOperator::Equal, true) => BinaryOperator::NotEqual,
        (BinaryOperator::NotEqual, false) => BinaryOperator::NotEqual,
        (BinaryOperator::NotEqual, true) => BinaryOperator::Equal,
        (BinaryOperator::Less, false) => BinaryOperator::Less,
        // Ordered negation is not portable to unordered float values. The
        // caller enables the complement only for checked integer operands.
        (BinaryOperator::Less, true) if operands_have_total_order => BinaryOperator::GreaterOrEqual,
        (BinaryOperator::LessOrEqual, false) => BinaryOperator::LessOrEqual,
        (BinaryOperator::LessOrEqual, true) if operands_have_total_order => BinaryOperator::Greater,
        (BinaryOperator::Greater, false) => BinaryOperator::Greater,
        (BinaryOperator::Greater, true) if operands_have_total_order => BinaryOperator::LessOrEqual,
        (BinaryOperator::GreaterOrEqual, false) => BinaryOperator::GreaterOrEqual,
        (BinaryOperator::GreaterOrEqual, true) if operands_have_total_order => BinaryOperator::Less,
        _ => return None,
    })
}

fn comparison_operands_are_integers(
    program: &TypedTrees,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
) -> bool {
    expression_is_integer_typed(program, left) && expression_is_integer_typed(program, right)
}

fn expression_is_integer_typed(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    use typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => true,
        ExpressionNode::Atomic(atomic) => expression_is_integer_typed(program, atomic.value),
        ExpressionNode::Borrow(inner) => expression_is_integer_typed(program, inner.target),
        ExpressionNode::Cast(cast) => type_reference_is_integer(program, cast.target_type),
        ExpressionNode::Name(path) => {
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
                .is_some_and(|symbol| symbol_is_integer_typed(program, symbol))
        }
        ExpressionNode::Member(member) => {
            let symbol = crate::flow::effective_member_symbol(program, member.receiver, member);
            symbol_is_integer_typed(program, symbol)
        }
        _ => false,
    }
}

fn type_reference_is_integer(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some_and(typed_trees::types::PrimitiveType::accepts_integer_literal)
}

fn symbol_is_integer_typed(program: &TypedTrees, symbol: symbols::SymbolHandle) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
            {
                return type_reference_is_integer(program, parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::LocalData(local) = statement
                    && local.symbol == symbol
                {
                    return type_reference_is_integer(program, local.type_reference);
                }
            }
        }
        if let Some(owned) = program
            .machine_owned_data(machine)
            .iter()
            .find(|owned| owned.symbol == symbol)
        {
            return type_reference_is_integer(program, owned.type_reference);
        }
    }
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return type_reference_is_integer(program, field.type_reference);
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == symbol)
                    {
                        return type_reference_is_integer(program, field.type_reference);
                    }
                }
                _ => {}
            }
        }
    }
    false
}

fn reversed_comparison(
    operator: typed_trees::expression::BinaryOperator,
) -> typed_trees::expression::BinaryOperator {
    use typed_trees::expression::BinaryOperator;

    match operator {
        BinaryOperator::Equal => BinaryOperator::Equal,
        BinaryOperator::NotEqual => BinaryOperator::NotEqual,
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        _ => unreachable!("only normalized comparisons reach operand reversal"),
    }
}
