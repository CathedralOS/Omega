//! Qualification consumes the source value judgment already produced by checking.
//!
//! The ordinary branch-sensitive arithmetic walk owns operand evaluation,
//! selected call guarantees, local updates and invalidation. Its retained
//! intervals are keyed by exact expression handles and hulled across clean
//! observations before this query runs. The cast operand, never the asserted
//! result type or receiving declaration, supplies the membership premise.
//! Closed integer literals also retain their full-width value, beyond the
//! compatibility interval's signed endpoint window. A missing judgment is not
//! evidence; no declaration-only or initializer-replay engine fills it in.
//! Predicate discharge and authorized provenance remain independent obligations.

use crate::program_validation::{ExpressionInterval, expression_interval};
use diagnostics::Diagnostic;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};

pub(crate) fn validate(
    program: &dyn crate::CheckingInput,
    intervals: &[ExpressionInterval],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Cast(cast) = node
            && !cast.semantic_domain.is_empty()
        {
            judge_qualification_cast(
                program,
                expression_interval(intervals, cast.value),
                cast,
                diagnostics,
            );
        }
    }
}

/// Judge one qualification cast (`x as T in <DeclaredDomain>`, decision 19).
/// Predicate domains discharge their propositions at this exact site. An
/// empty domain qualifies vacuously. A routed domain cannot be fabricated by
/// `as`, even when it has no predicate propositions.
fn judge_qualification_cast(
    program: &dyn crate::CheckingInput,
    source: Option<&ExpressionInterval>,
    cast: &TableCastExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let members = program
        .expression_table
        .name_path_members(cast.semantic_domain);
    let base_name = members.joined_path();
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
    let declared = program.domain_definition(cast.semantic_domain_symbol);
    match declared {
        Some(domain) => {
            // A predicate proof is not an authorized origin. In particular,
            // computed bounds cannot mint a routed domain whose predicate
            // happens to hold. Existing route transport keeps its own owner.
            if !domain.establishment_routes.is_empty()
                || (!domain.predicate_body.is_present()
                    && !crate::field_domains::domain_is_vacuous(
                        program,
                        domain.symbol,
                        &mut Vec::new(),
                    ))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`as ... in {name}` cannot establish a non-vacuous domain; every \
                     predicate must be proved and routed provenance must come from an exact \
                     requirement authorized by the domain"
                )));
                return;
            }
            if !domain.predicate_body.is_present() {
                return;
            }
            let indices = cast_domain_index_bindings(program, domain, cast);
            let mut judgment = literal_mint_discharges(program, domain, cast.value, &indices);
            if matches!(judgment, MintJudgment::NotProved)
                && let Some(source) = source
                && let Some(judged) =
                    interval_mint_judgment(program, domain, source.low, source.high, &indices)
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
                MintJudgment::NotProved => {
                    diagnostics.push(Diagnostic::error(format!(
                        "`as ... in {name}` cannot prove every domain predicate from the \
                         checked operand value; constrain that value with a guarantee or guard",
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
/// parameter symbol with 256, so facts declared as `subject <= Bound` judge
/// against the instance's argument rather than the parameter name.
fn cast_domain_index_bindings(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    cast: &TableCastExpression,
) -> Vec<(SymbolHandle, i128)> {
    let arguments = program
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments);
    typed_trees::domain::index_parameters(program, domain)
        .iter()
        .zip(arguments)
        .filter_map(|(parameter, argument)| {
            let typed_trees::types::TypeReferenceNode::Named { name, .. } =
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

/// The mint's tri-state judgment for one qualification cast.
enum MintJudgment {
    Discharged,
    FactFalse,
    NotProved,
}

/// Fold every domain fact at the cast's literal value (`subject := literal`).
/// Only integer literals and `subject <op> literal` / `literal <op> subject`
/// comparison facts fold; anything else is conservatively NotProved.
/// Unsupported propositions are missing evidence, never ignorable facts.
fn literal_mint_discharges(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    value: ExpressionHandle,
    indices: &[(SymbolHandle, i128)],
) -> MintJudgment {
    let Some(literal) = program.expression_table.integer_node(value) else {
        return MintJudgment::NotProved;
    };
    let Ok(minted) = literal.text().parse::<i128>() else {
        return MintJudgment::NotProved;
    };
    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let Some(expression) = fact.as_expression() else {
            return MintJudgment::NotProved;
        };
        match program.expression_table.expression(*expression) {
            ExpressionNode::Boolean(true) => continue,
            ExpressionNode::Boolean(false) => return MintJudgment::FactFalse,
            _ => {}
        }
        let Some(binary) = program.expression_table.binary_node(*expression) else {
            return MintJudgment::NotProved;
        };
        if !crate::has_builtin_domain_decomposed_guard_meaning(program, domain, *expression) {
            return MintJudgment::NotProved;
        }
        let side_value = |handle: ExpressionHandle| -> Option<i128> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Name(_)
                    if typed_trees::domain::domain_subject_reference(program, domain, handle) =>
                {
                    Some(minted)
                }
                ExpressionNode::Name(path) => index_literal(indices, path.symbol),
                ExpressionNode::Integer(value) => value.text().parse::<i128>().ok(),
                _ => None,
            }
        };
        let (Some(left), Some(right)) = (side_value(binary.left), side_value(binary.right)) else {
            return MintJudgment::NotProved;
        };
        use typed_trees::expression::BinaryOperator;
        let holds = match binary.operator {
            BinaryOperator::Less => left < right,
            BinaryOperator::LessOrEqual => left <= right,
            BinaryOperator::Greater => left > right,
            BinaryOperator::GreaterOrEqual => left >= right,
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            _ => return MintJudgment::NotProved,
        };
        if !holds {
            return MintJudgment::FactFalse;
        }
    }
    MintJudgment::Discharged
}

/// Every `subject OP literal` domain fact must hold over the whole interval
/// (`subject >= K` iff low >= K; `subject <= K` iff high <= K; strict and equality
/// forms accordingly). An absent endpoint entails nothing on its side.
fn interval_mint_judgment(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    low: Option<i64>,
    high: Option<i64>,
    indices: &[(SymbolHandle, i128)],
) -> Option<MintJudgment> {
    use typed_trees::expression::BinaryOperator;

    for fact in program.proof_facts.span_or_empty(domain.facts) {
        let expression = fact.as_expression()?;
        match program.expression_table.expression(*expression) {
            ExpressionNode::Boolean(true) => continue,
            ExpressionNode::Boolean(false) => return Some(MintJudgment::FactFalse),
            _ => {}
        }
        let Some(binary) = program.expression_table.binary_node(*expression) else {
            return None;
        };
        if !crate::has_builtin_domain_decomposed_guard_meaning(program, domain, *expression) {
            return None;
        }
        let literal_of = |handle: ExpressionHandle| -> Option<i64> {
            match program.expression_table.expression(handle) {
                ExpressionNode::Integer(value) => value.text().parse::<i64>().ok(),
                ExpressionNode::Name(fact_path) => index_literal(indices, fact_path.symbol)
                    .and_then(|value| i64::try_from(value).ok()),
                _ => None,
            }
        };
        let (operator, bound) =
            if typed_trees::domain::domain_subject_reference(program, domain, binary.left) {
                (binary.operator, literal_of(binary.right)?)
            } else if typed_trees::domain::domain_subject_reference(program, domain, binary.right) {
                let flipped = binary.operator.mirrored();
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
            return Some(MintJudgment::NotProved);
        }
    }
    Some(MintJudgment::Discharged)
}
