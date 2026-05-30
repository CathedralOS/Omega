//! Domain-operator dispatch validation (chapter 8 semantic domain operators).
//!
//! A *domain operator* is an `operator` declared inside a `domain`. Its meaning
//! is only selectable when the enclosing domain's proof facts hold in the
//! current context — that is what makes it a *semantic* operator rather than a
//! purely syntactic overload. Resolution keys on the operator's spelling plus
//! its operand types (return type never distinguishes), exactly like the
//! expression-level spelling dispatch in `omega-typed-trees`.
//!
//! This module enforces the chapter-8 selection rule: competing domain meanings
//! for one spelling are a compile error. Two domain operators that
//!   * bind the same spelling,
//!   * share the same normalized operand-type signature, and
//!   * classify the same target type (so their proof contexts could both apply
//!     to one value),
//! cannot be disambiguated by any provable fact, because selecting one over the
//! other would require a fact the context does not carry. Such pairs are
//! rejected; only a meaning whose domain context is uniquely provable may be
//! selected.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::DomainDefinition;
use omega_typed_trees::operator::{OperatorDefinition, operator_operand_signature};

use crate::type_references::type_references_match;

/// A single domain-operator meaning: a spelled operator carried by a domain,
/// together with the domain context that must be provable to select it.
struct DomainOperatorMeaning<'a> {
    domain: &'a DomainDefinition,
    operand_signature: String,
    spelling_symbol: &'static str,
    operator_name: String,
}

pub(super) fn validate_domain_operator_dispatch(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Collect every spelled domain operator as a candidate meaning.
    let mut meanings = Vec::new();
    for domain in program.domain_definitions() {
        for operator in program.domain_operators(domain) {
            let Some(spelling) = operator.spelling else {
                continue;
            };
            meanings.push(DomainOperatorMeaning {
                domain,
                operand_signature: operator_operand_signature(program, operator),
                spelling_symbol: spelling.symbol(),
                operator_name: operator_name(program, operator),
            });
        }
    }

    // Two meanings compete when they share a spelling and operand signature and
    // classify the same target type: no provable fact can select between them.
    for (index, meaning) in meanings.iter().enumerate() {
        let competitor = meanings[..index].iter().find(|previous| {
            previous.spelling_symbol == meaning.spelling_symbol
                && previous.operand_signature == meaning.operand_signature
                && type_references_match(
                    program,
                    previous.domain.target_type,
                    meaning.domain.target_type,
                )
        });

        if let Some(competitor) = competitor {
            diagnostics.push(Diagnostic::error(format!(
                "domain operator spelling `{}` has competing meanings for operand types \
                 `({})`: domain `{}` operator `{}` and domain `{}` operator `{}` both apply to \
                 the same target type, so no provable fact can select between them",
                meaning.spelling_symbol,
                meaning.operand_signature,
                competitor.domain.name,
                competitor.operator_name,
                meaning.domain.name,
                meaning.operator_name,
            )));
        }
    }
}

fn operator_name(program: &TypedTrees, operator: &OperatorDefinition) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
