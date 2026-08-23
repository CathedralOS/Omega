use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use psi_typed_trees::statement::StatementNode;

/// Judge one qualification cast (`x as T in <DeclaredDomain>`, decision 19).
/// Predicate domains discharge their propositions at this exact site. An
/// empty domain qualifies vacuously. A routed domain cannot be fabricated by
/// `as`, even when it has no predicate propositions.
pub(super) fn judge_qualification_cast(
    program: &TypedTrees,
    context: Option<(
        &psi_typed_trees::machine::Machine,
        &psi_typed_trees::state::State,
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
            let mut judgment = literal_mint_discharges(program, domain, cast.value);
            if matches!(judgment, MintJudgment::NotLiteral)
                && let Some((machine, state)) = context
                && let Some(judged) =
                    range_mint_discharges(program, machine, state, domain, cast.value)
            {
                judgment = judged;
            }
            // The GUARD chain's landing: the machine's own REQUIRES facts
            // about the cast value bound it one-sidedly; callers prove the
            // requires at their call sites (incoming guards already serve
            // there), so `transition raw >= 0 { true -> use(raw) }` +
            // `machine use(..) requires raw >= 0` mints inside `use`.
            if matches!(judgment, MintJudgment::NotLiteral)
                && let Some((machine, _)) = context
                && let Some(judged) = requires_mint_discharges(program, machine, domain, cast.value)
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
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    domain: &psi_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> Option<MintJudgment> {
    // The RAW declared type keeps the Constrained shell the range lives in
    // (declared_place_type strips it -- the R2 slice-9 gotcha).
    let declared = crate::places::declared_place_type_raw(program, machine, Some(state), value)?;
    let interval = crate::arithmetic_domains::range_constraint_interval(program, declared)?;
    let (low, high) = (interval.low?, interval.high?);
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            continue;
        };
        // Normalize to `self OP literal`.
        let is_self = |handle: ExpressionHandle| {
            matches!(
                program.expression_table.expression(handle),
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == "self"
                    )
            )
        };
        let literal_of = |handle: ExpressionHandle| -> Option<i64> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                _ => None,
            }
        };
        use psi_typed_trees::expression::BinaryOperator;
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
            BinaryOperator::GreaterOrEqual => low >= bound,
            BinaryOperator::Greater => low > bound,
            BinaryOperator::LessOrEqual => high <= bound,
            BinaryOperator::Less => high < bound,
            BinaryOperator::Equal => low == bound && high == bound,
            BinaryOperator::NotEqual => high < bound || low > bound,
            _ => return None,
        };
        if !holds {
            // The interval does not ENTAIL the fact -- it may still hold at
            // runtime, so this is the undischarged (not FALSE) class.
            return Some(MintJudgment::NotLiteral);
        }
    }
    Some(MintJudgment::Discharged)
}

/// Walk one statement's expressions for qualification casts and judge each
/// WITH machine/state context: the literal fold first, then the value's
/// DECLARED RANGE (flow-integration v1 -- a Name whose declared type
/// carries `[lo..=hi]` discharges facts the whole interval satisfies).
pub(super) fn judge_statement_qualification_casts(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement: &StatementNode,
    judged: &mut Vec<ExpressionHandle>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut roots: Vec<ExpressionHandle> = Vec::new();
    match statement {
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
            if let psi_typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard
            {
                roots.push(*guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    psi_typed_trees::statement::TransitionTargetNode::Value(value) => {
                        roots.push(*value);
                    }
                    psi_typed_trees::statement::TransitionTargetNode::Named {
                        arguments, ..
                    } => {
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
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
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
        ExpressionNode::Mutable(inner) => {
            judge_expression_qualification_casts(
                program,
                machine,
                state,
                *inner,
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
    domain: &psi_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> MintJudgment {
    let ExpressionNode::Integer(literal) = program.expression_table.expression(value) else {
        return MintJudgment::NotLiteral;
    };
    let Ok(minted) = literal.text().parse::<i128>() else {
        return MintJudgment::NotLiteral;
    };
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
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
                        [only] if only.as_str() == "self"
                    ) =>
                {
                    Some(minted)
                }
                ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
                _ => None,
            }
        };
        let (Some(left), Some(right)) = (side_value(binary.left), side_value(binary.right)) else {
            return MintJudgment::NotLiteral;
        };
        use psi_typed_trees::expression::BinaryOperator;
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

/// Requires-route discharge: the machine's REQUIRES facts about the cast
/// value's NAME accumulate one-sided bounds (`raw >= 0` -> low = 0); the
/// domain facts must be entailed by those bounds. `None` when the value is
/// not a bare name or no requires fact speaks about it.
fn requires_mint_discharges(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    domain: &psi_typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
) -> Option<MintJudgment> {
    use psi_typed_trees::expression::BinaryOperator;
    use psi_typed_trees::signature::SignatureContractKind;

    let ExpressionNode::Name(path) = program.expression_table.expression(value) else {
        return None;
    };
    let [value_name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };

    let mut low: Option<i64> = None;
    let mut high: Option<i64> = None;
    let mut spoke = false;
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
            else {
                continue;
            };
            let names_value = |handle: ExpressionHandle| {
                matches!(
                    program.expression_table.expression(handle),
                    ExpressionNode::Name(fact_path)
                        if matches!(
                            program.expression_table.name_path_members(fact_path.members),
                            [only] if only.as_str() == value_name.as_str()
                        )
                )
            };
            let literal_of = |handle: ExpressionHandle| -> Option<i64> {
                match program.expression_table.expression(handle) {
                    ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                    _ => None,
                }
            };
            let (operator, bound) = if names_value(binary.left) {
                let Some(bound) = literal_of(binary.right) else {
                    continue;
                };
                (binary.operator, bound)
            } else if names_value(binary.right) {
                let Some(bound) = literal_of(binary.left) else {
                    continue;
                };
                let flipped = match binary.operator {
                    BinaryOperator::Less => BinaryOperator::Greater,
                    BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                    BinaryOperator::Greater => BinaryOperator::Less,
                    BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                    other => other,
                };
                (flipped, bound)
            } else {
                continue;
            };
            spoke = true;
            match operator {
                BinaryOperator::GreaterOrEqual => low = Some(low.map_or(bound, |l| l.max(bound))),
                BinaryOperator::Greater => {
                    let floor = bound.saturating_add(1);
                    low = Some(low.map_or(floor, |l| l.max(floor)));
                }
                BinaryOperator::LessOrEqual => high = Some(high.map_or(bound, |h| h.min(bound))),
                BinaryOperator::Less => {
                    let ceiling = bound.saturating_sub(1);
                    high = Some(high.map_or(ceiling, |h| h.min(ceiling)));
                }
                BinaryOperator::Equal => {
                    low = Some(low.map_or(bound, |l| l.max(bound)));
                    high = Some(high.map_or(bound, |h| h.min(bound)));
                }
                _ => {}
            }
        }
    }
    if !spoke {
        return None;
    }

    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
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
                        [only] if only.as_str() == "self"
                    )
            )
        };
        let literal_of = |handle: ExpressionHandle| -> Option<i64> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
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
