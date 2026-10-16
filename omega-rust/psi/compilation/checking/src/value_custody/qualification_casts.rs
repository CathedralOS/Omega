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
use crate::proof_contracts::contract_entailment::{
    ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use symbols::JoinedPath;
use typed_trees::expression::{ExpressionNode, TableCastExpression};

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
            // Routed membership depends on the operand's live provenance,
            // including a retained atom during explicit weakening. The flow
            // qualification judgment owns that evidence, never this target.
            if crate::field_domains::domain_requires_provenance(program, domain.symbol) {
                return;
            }
            // A predicate proof is not an authorized origin. In particular,
            // computed bounds cannot mint a routed domain whose predicate
            // happens to hold. Existing route transport keeps its own owner.
            if !domain.predicate_body.is_present()
                && domain.alias.is_none()
                && !crate::field_domains::domain_is_vacuous(program, domain.symbol, &mut Vec::new())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`as ... in {name}` cannot establish a non-vacuous domain; every \
                     predicate must be proved and routed provenance must come from an exact \
                     requirement authorized by the domain"
                )));
                return;
            }
            if crate::field_domains::domain_is_vacuous(program, domain.symbol, &mut Vec::new()) {
                return;
            }
            // A structural carrier's predicates read its fields, places and
            // selected cases, which only the flow contexts at the cast's own
            // occurrence know. Checked contracts judge those casts there, with
            // the evidence a qualified initializer or call argument uses.
            if program
                .primitive_type_reference(domain.target_type)
                .is_none()
            {
                return;
            }
            let judgment = operand_mint_judgment(program, cast, source);
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

/// The mint's tri-state judgment for one qualification cast.
enum MintJudgment {
    Discharged,
    FactFalse,
    NotProved,
}

/// Read the operand's actual value, not its asserted result. A literal keeps
/// full-width precision; otherwise the arithmetic walk supplies independent
/// bounds on a private value. The complete theory reader visits aliases and
/// nested membership too, so an alias's empty local fact list is not vacuity.
fn operand_mint_judgment(
    program: &dyn crate::CheckingInput,
    cast: &TableCastExpression,
    source: Option<&ExpressionInterval>,
) -> MintJudgment {
    let value = if let Some(literal) = program.expression_table.integer_node(cast.value)
        && let Some(value) = BigInt::from_decimal_str(literal.text())
    {
        ScopedArithmeticValue::Integer(value)
    } else if let Some(source) = source {
        ScopedArithmeticValue::BoundedInteger {
            value: Box::new(ScopedArithmeticValue::Atom {
                identity: format!("qualification operand {:?}", cast.value),
                unsigned: false,
            }),
            minimum: source.low.map(BigInt::from_i64),
            maximum: source.high.map(BigInt::from_i64),
        }
    } else {
        return MintJudgment::NotProved;
    };
    match crate::proof_contracts::domains::integer_domain_membership(
        program,
        cast.semantic_domain_symbol,
        program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments),
        cast.semantic_domain_id,
        &value,
    ) {
        StrictArithmeticImplicationJudgment::Proven => MintJudgment::Discharged,
        StrictArithmeticImplicationJudgment::Refuted => MintJudgment::FactFalse,
        StrictArithmeticImplicationJudgment::Unknown => MintJudgment::NotProved,
    }
}
