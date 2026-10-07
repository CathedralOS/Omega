//! Closed-row contract refinement: an implementation member honors the
//! complete machine contract of the requirement it realizes
//! (wiki/spec/language/machines.md#substitution).
//!
//! A caller that reaches a member through its requirement, such as a
//! selector call or a call routed through the requirement, proves only the
//! requirement's `requires` and assumes its `ensures`. So the member may
//! demand no premise the requirement lacks, and must guarantee every
//! conclusion the requirement states. Laws, the result-free equalities and
//! proposition applications, keep their own entailment judgment in
//! `law_conformance`; this rule covers every other expression and membership
//! conjunct by exact correspondence. A requirement conclusion must reappear
//! among the member's own `ensures`, which its exits prove, and each member
//! premise must reappear among the requirement's `requires`. Formals compare
//! by position, so a member may rename them.
//!
//! Exact correspondence is deliberately weaker than entailment: a member that
//! proves a stronger conclusion must still state the requirement's spelling.
//! Proposition applications are not unfolded here. A requirement premise such
//! as `Relation(left, right)` may mean the member's `a == b` once the
//! conformance binds `Relation`, so a requirement with any proposition premise
//! leaves the member's premises unchecked. A law requirement, one whose every
//! conclusion is a law, likewise leaves its members' premises to the law
//! judgment. Both constrain proof evidence rather than the runtime operands a
//! selector call passes.

use super::law_conformance::term_mentions_variable;
use super::structural_terms::structural_term;
use super::{RESULT_BINDER, StateSignature, TraitDefinition, TypedTrees};
use diagnostics::Diagnostic;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::proposition::ProofSubstitutions;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};

/// Validate that `machine`, realizing `requirement` through a closed row,
/// refines the requirement's contract.
pub(crate) fn check_contract_refinement(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let required_parameters = program.state_signature_parameters(requirement);
    let member_parameters = program.state_parameters(entry);
    // The signature check reports an arity mismatch; no position map exists.
    if required_parameters.len() != member_parameters.len() {
        return;
    }
    let required_contracts = program.state_signature_contracts(requirement);
    let member_contracts = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry));
    let required = Conjuncts::collect(program, required_contracts.iter(), required_parameters);
    let member = Conjuncts::collect(program, member_contracts, member_parameters);
    let requirement_name = format!("{}::{}", trait_definition.name, requirement.name);

    for conclusion in &required.ensures {
        if conclusion.law_checked
            || member
                .ensures
                .iter()
                .any(|stated| stated.key == conclusion.key)
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` realizes `{requirement_name}` but does not guarantee its `ensures {}`; callers of the requirement assume it, so state it as the member's own `ensures`",
            machine.name,
            render(program, conclusion.expression, required_parameters, member_parameters),
        )));
    }
    if required.proposition_premises || required.laws_only {
        return;
    }
    for premise in &member.requires {
        if required
            .requires
            .iter()
            .any(|stated| stated.key == premise.key)
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` realizes `{requirement_name}` but requires `{}`, which the requirement does not require; callers of the requirement prove only its own `requires`",
            machine.name,
            render(program, premise.expression, member_parameters, member_parameters),
        )));
    }
}

/// One contract's conjuncts, keyed by their spelling with formals replaced by
/// positional placeholders.
struct Conjuncts {
    requires: Vec<Conjunct>,
    ensures: Vec<Conjunct>,
    proposition_premises: bool,
    /// Every conclusion is a law-checked equality or a proposition
    /// application, and there is at least one.
    laws_only: bool,
}

struct Conjunct {
    expression: ExpressionHandle,
    key: String,
    /// A result-free equality the law judgment already enforces.
    law_checked: bool,
}

impl Conjuncts {
    fn collect<'a>(
        program: &TypedTrees,
        contracts: impl Iterator<Item = &'a SignatureContract>,
        parameters: &[StateParameter],
    ) -> Self {
        let placeholders = parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                (
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    format!("#{position}"),
                )
            })
            .collect::<Vec<_>>();
        let mut conjuncts = Self {
            requires: Vec::new(),
            ensures: Vec::new(),
            proposition_premises: false,
            laws_only: false,
        };
        let mut proposition_conclusions = 0usize;
        for contract in contracts {
            let premise = match contract.kind {
                SignatureContractKind::Requires => true,
                SignatureContractKind::Ensures => false,
                _ => continue,
            };
            if premise
                && program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| matches!(fact, ProofFact::Proposition(_)))
            {
                conjuncts.proposition_premises = true;
            }
            let target = if premise {
                &mut conjuncts.requires
            } else {
                &mut conjuncts.ensures
            };
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        let mut split = Vec::new();
                        split_conjuncts(program, *expression, &mut split);
                        target.extend(split.into_iter().map(|expression| Conjunct {
                            expression,
                            key: program.render_proof_expression(
                                expression,
                                ProofSubstitutions::ByParameter(&placeholders),
                            ),
                            law_checked: is_law_conjunct(program, expression),
                        }));
                    }
                    ProofFact::Membership(membership) => target.push(Conjunct {
                        expression: membership.value,
                        key: format!(
                            "{} in {:?}",
                            program.render_proof_expression(
                                membership.value,
                                ProofSubstitutions::ByParameter(&placeholders),
                            ),
                            membership.semantic_domain,
                        ),
                        law_checked: false,
                    }),
                    ProofFact::Proposition(_) => {
                        if !premise {
                            proposition_conclusions += 1;
                        }
                    }
                }
            }
        }
        conjuncts.laws_only = conjuncts.ensures.len() + proposition_conclusions > 0
            && conjuncts
                .ensures
                .iter()
                .all(|conclusion| conclusion.law_checked);
        conjuncts
    }
}

fn split_conjuncts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => split_conjuncts(program, atomic.value, out),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            split_conjuncts(program, binary.left, out);
            split_conjuncts(program, binary.right, out);
        }
        _ => out.push(expression),
    }
}

/// The law judgment enforces an equality whose sides both read as structural
/// terms and neither names the result.
fn is_law_conjunct(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let result = RESULT_BINDER.to_owned();
    match (
        structural_term(program, binary.left),
        structural_term(program, binary.right),
    ) {
        (Some(left), Some(right)) => {
            !term_mentions_variable(&left, &result) && !term_mentions_variable(&right, &result)
        }
        _ => false,
    }
}

/// Spell a conjunct in the member's formal names.
fn render(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authored: &[StateParameter],
    member: &[StateParameter],
) -> String {
    let names = authored
        .iter()
        .zip(member)
        .map(|(authored, member)| {
            (
                authored.symbol,
                authored.name.as_str().to_owned(),
                member.name.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    program.render_proof_expression(expression, ProofSubstitutions::ByParameter(&names))
}
