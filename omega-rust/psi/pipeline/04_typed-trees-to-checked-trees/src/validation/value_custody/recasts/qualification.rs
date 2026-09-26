use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCastExpression,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;
use symbols::SymbolHandle;

/// Judge one qualification cast (`x as T in <DeclaredDomain>`, decision 19).
/// Predicate domains discharge their propositions at this exact site. An
/// empty domain qualifies vacuously. A routed domain cannot be fabricated by
/// `as`, even when it has no predicate propositions.
pub(super) fn judge_qualification_cast(
    program: &TypedTrees,
    context: Option<(
        &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
        &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    )>,
    cast: &TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let members = program
        .expression_table
        .name_path_members(cast.semantic_domain);
    let base_name = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let mut name = base_name.clone();
    if !cast.semantic_domain_arguments.is_empty() {
        let arguments = program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ");
        name = format!("{name}<{arguments}>");
    }
    let declared = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == cast.semantic_domain_symbol);
    match declared {
        Some(domain) => {
            if !domain.predicate_body.is_present() {
                if !domain_is_vacuous(program, domain.symbol, &mut Vec::new()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` cannot establish a non-vacuous domain; every \
                         predicate must be proved and routed provenance must come from an exact \
                         requirement authorized by the domain"
                    )));
                }
                return;
            }
            let indices = cast_domain_index_bindings(program, domain, cast);
            let mut judgment = literal_mint_discharges(program, domain, cast.value, &indices);
            if matches!(judgment, MintJudgment::NotLiteral)
                && let Some((machine, state)) = context
                && let Some(judged) =
                    range_mint_discharges(program, machine, state, domain, cast.value, &indices)
            {
                judgment = judged;
            }
            // The GUARD chain's landing: an immutable parameter's own
            // `requires` facts bound it; callers prove the requires at their
            // call sites (incoming guards already serve there), so
            // `transition raw >= 0 { true -> use(raw) }` +
            // `machine use(..) requires raw >= 0` mints inside `use`.
            if matches!(judgment, MintJudgment::NotLiteral)
                && let Some((machine, state)) = context
                && let Some(judged) =
                    bounds_mint_discharges(program, machine, state, domain, cast.value, &indices)
            {
                judgment = judged;
            }
            match judgment {
                MintJudgment::Discharged => {}
                MintJudgment::FactFalse => {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` cannot mint: a `{name}` domain fact is \
                         FALSE at this value -- the predicate obligation is owed \
                         (decision 19's \"predicate obligation not discharged\" \
                         class)",
                    )));
                }
                MintJudgment::NotLiteral => {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` mints LITERAL values, or names whose \
                         DECLARED RANGE entails the domain facts, in this rung; \
                         other values route through a validating call or guard \
                         until full flow integration lands",
                    )));
                }
            }
        }
        None => {
            let matching_names = program
                .domain_definitions()
                .iter()
                .filter(|domain| {
                    domain.name.as_str() == base_name
                        || domain.name.as_str().ends_with(&format!("::{base_name}"))
                })
                .count();
            diagnostics.push(Diagnostic::error(format!(
                "{} cast domain `{name}` for target `{}`",
                if matching_names > 1 {
                    "ambiguous"
                } else {
                    "unknown"
                },
                program.display_type_reference(cast.target_type),
            )));
        }
    }
}

/// The cast instance's closed index bindings: `domain<const Bound: u64>
/// u64::AtMost<Bound>` minted as `... in AtMost<256>` pairs `Bound`'s
/// parameter symbol with 256, so facts declared as `self <= Bound` judge
/// against the instance's argument rather than the parameter name.
fn cast_domain_index_bindings(
    program: &TypedTrees,
    domain: &symbol_resolved_trees_to_typed_trees::typed_trees::domain::DomainDefinition,
    cast: &TableCastExpression,
) -> Vec<(SymbolHandle, i128)> {
    let arguments = program
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments);
    symbol_resolved_trees_to_typed_trees::typed_trees::domain::index_parameters(program, domain)
        .iter()
        .zip(arguments)
        .filter_map(|(parameter, argument)| {
            let symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode::Named { name, .. } =
                program.type_reference_table.type_reference(*argument)
            else {
                return None;
            };
            let value = if let Some(canonical) =
                language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
            {
                match canonical.decode_encoding() {
                    Some(
                        language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                            value,
                            ..
                        },
                    ) => value,
                    _ => return None,
                }
            } else {
                name.as_str().parse::<i128>().ok()?
            };
            Some((parameter.symbol, value))
        })
        .collect()
}

/// A fact leaf naming one of the minted instance's index parameters reads as
/// the closed argument's value.
fn index_literal(indices: &[(SymbolHandle, i128)], symbol: SymbolHandle) -> Option<i128> {
    indices
        .iter()
        .find(|(candidate, _)| *candidate == symbol)
        .map(|(_, value)| *value)
}

fn domain_is_vacuous(
    program: &TypedTrees,
    domain_symbol: SymbolHandle,
    stack: &mut Vec<SymbolHandle>,
) -> bool {
    if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|candidate| candidate.symbol == domain_symbol)
    else {
        return false;
    };
    if let Some(alias) = domain.alias.as_ref() {
        if alias.constituents.is_empty() {
            return false;
        }
        stack.push(domain_symbol);
        let vacuous = alias
            .constituents
            .iter()
            .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
        stack.pop();
        return vacuous;
    }
    !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
}

/// Flow-integration v1: when the cast VALUE is a Name whose declared type
/// carries a range constraint, every domain fact must hold over the WHOLE
/// interval (`self >= K` iff low >= K; `self <= K` iff high <= K; strict/
/// equality forms accordingly). `None` when the value has no usable range
/// (the caller falls back to the staged refusal).
fn range_mint_discharges(
    program: &TypedTrees,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    domain: &symbol_resolved_trees_to_typed_trees::typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
    indices: &[(SymbolHandle, i128)],
) -> Option<MintJudgment> {
    // The RAW declared type keeps the Constrained shell the range lives in
    // (declared_place_type strips it -- the R2 slice-9 gotcha).
    let declared = crate::validation::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        value,
    )?;
    let interval =
        crate::validation::proof_contracts::arithmetic_domains::range_constraint_interval(
            program, declared,
        )?;
    let (low, high) = (interval.low?, interval.high?);
    interval_mint_judgment(program, domain, Some(low), Some(high), indices)
}

/// Walk one statement's expressions for qualification casts and judge each
/// WITH machine/state context: the literal fold first, then the value's
/// DECLARED RANGE (flow-integration v1 -- a Name whose declared type
/// carries `[lo..=hi]` discharges facts the whole interval satisfies).
pub(super) fn judge_statement_qualification_casts(
    program: &TypedTrees,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    statement: &StatementNode,
    judged: &mut Vec<ExpressionHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut roots: Vec<ExpressionHandle> = Vec::new();
    match statement {
        StatementNode::RootBinding(binding) => {
            roots.extend(
                [binding.receiver, binding.implementation_operand]
                    .into_iter()
                    .filter(|expression| expression.is_valid()),
            );
        }
        StatementNode::AssemblyFact(fact) => roots.push(fact.expression),
        StatementNode::Assignment(assignment) => {
            roots.push(assignment.target);
            roots.push(assignment.value);
        }
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => roots.push(local.initial_value),
        StatementNode::Call(call) => roots.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Transition(transition) => {
            if let symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
                roots.push(*guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Value(value) => {
                        roots.push(*value);
                    }
                    symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        roots.extend(
                            program
                                .statement_table
                                .expression_handles(*arguments)
                                .iter()
                                .copied(),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    for root in roots {
        judge_expression_qualification_casts(program, machine, state, root, judged, diagnostics);
    }
}

fn judge_expression_qualification_casts(
    program: &TypedTrees,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    expression: ExpressionHandle,
    judged: &mut Vec<ExpressionHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => {
            if cast.semantic_domain.count() > 0 {
                judged.push(expression);
                judge_qualification_cast(program, Some((machine, state)), cast, diagnostics);
            }
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                cast.value,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Binary(binary) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                binary.left,
                judged,
                diagnostics,
            );
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                binary.right,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                unary.operand,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Borrow(inner) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                inner.target,
                judged,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                judge_expression_qualification_casts(
                    program,
                    machine,
                    state,
                    *argument,
                    judged,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// The mint's tri-state judgment for one qualification cast.
enum MintJudgment {
    Discharged,
    FactFalse,
    NotLiteral,
}

/// Fold every domain fact at the cast's literal value (`self := literal`).
/// Only integer literals and `self <op> literal` / `literal <op> self`
/// comparison facts fold; anything else is conservatively NotLiteral.
/// `introduction`-clause pseudo-facts (non-Binary) are skipped -- they are
/// policy, not predicate.
fn literal_mint_discharges(
    program: &TypedTrees,
    domain: &symbol_resolved_trees_to_typed_trees::typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
    indices: &[(SymbolHandle, i128)],
) -> MintJudgment {
    let ExpressionNode::Integer(literal) = program.expression_table.expression(value) else {
        return MintJudgment::NotLiteral;
    };
    let Ok(minted) = literal.text().parse::<i128>() else {
        return MintJudgment::NotLiteral;
    };
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Expression(
            expression,
        ) = fact
        else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let side_value = |handle: ExpressionHandle| -> Option<i128> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.is_self_receiver()
                    ) =>
                {
                    Some(minted)
                }
                ExpressionNode::Name(path) => index_literal(indices, path.symbol),
                ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
                _ => None,
            }
        };
        let (Some(left), Some(right)) = (side_value(binary.left), side_value(binary.right)) else {
            return MintJudgment::NotLiteral;
        };
        use symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator;
        let holds = match binary.operator {
            BinaryOperator::Less => left < right,
            BinaryOperator::LessOrEqual => left <= right,
            BinaryOperator::Greater => left > right,
            BinaryOperator::GreaterOrEqual => left >= right,
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            _ => return MintJudgment::NotLiteral,
        };
        if !holds {
            return MintJudgment::FactFalse;
        }
    }
    MintJudgment::Discharged
}

/// Bounds-route discharge: the immutable integer bounds of the cast value,
/// including a parameter's own `requires` comparisons. `None` when the value
/// has no builtin integer bounds.
fn bounds_mint_discharges(
    program: &TypedTrees,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    domain: &symbol_resolved_trees_to_typed_trees::typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
    indices: &[(SymbolHandle, i128)],
) -> Option<MintJudgment> {
    let interval =
        crate::validation::proof_contracts::arithmetic_domains::immutable_integer_expression_interval(
            program, machine, state, value,
        )?;
    interval_mint_judgment(program, domain, interval.low, interval.high, indices)
}

/// Every `self OP literal` domain fact must hold over the whole interval
/// (`self >= K` iff low >= K; `self <= K` iff high <= K; strict and equality
/// forms accordingly). An absent endpoint entails nothing on its side.
fn interval_mint_judgment(
    program: &TypedTrees,
    domain: &symbol_resolved_trees_to_typed_trees::typed_trees::domain::DomainDefinition,
    low: Option<i64>,
    high: Option<i64>,
    indices: &[(SymbolHandle, i128)],
) -> Option<MintJudgment> {
    use symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator;

    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Expression(
            expression,
        ) = fact
        else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        let is_self = |handle: ExpressionHandle| {
            matches!(
                program.expression_table.expression(handle),
                ExpressionNode::Name(fact_path)
                    if matches!(
                        program.expression_table.name_path_members(fact_path.members),
                        [only] if only.is_self_receiver()
                    )
            )
        };
        let literal_of = |handle: ExpressionHandle| -> Option<i64> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                ExpressionNode::Name(fact_path) => index_literal(indices, fact_path.symbol)
                    .and_then(|value| i64::try_from(value).ok()),
                _ => None,
            }
        };
        let (operator, bound) = if is_self(binary.left) {
            (binary.operator, literal_of(binary.right)?)
        } else if is_self(binary.right) {
            let flipped = match binary.operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other => other,
            };
            (flipped, literal_of(binary.left)?)
        } else {
            return None;
        };
        let holds = match operator {
            BinaryOperator::GreaterOrEqual => low.is_some_and(|l| l >= bound),
            BinaryOperator::Greater => low.is_some_and(|l| l > bound),
            BinaryOperator::LessOrEqual => high.is_some_and(|h| h <= bound),
            BinaryOperator::Less => high.is_some_and(|h| h < bound),
            BinaryOperator::Equal => {
                low.is_some_and(|l| l == bound) && high.is_some_and(|h| h == bound)
            }
            BinaryOperator::NotEqual => {
                high.is_some_and(|h| h < bound) || low.is_some_and(|l| l > bound)
            }
            _ => return None,
        };
        if !holds {
            return Some(MintJudgment::NotLiteral);
        }
    }
    Some(MintJudgment::Discharged)
}
