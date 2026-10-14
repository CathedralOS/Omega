//! A selected implementation accepts every input its requirement permits.
//!
//! Signature matching calls this judgment for exact satisfaction edges and
//! named maps alike. Requirement premises entail implementation premises;
//! implementation guarantees entail requirement guarantees. Law conclusions
//! retain their independent law judgment, and exact edges inherit guarantees
//! which their bodies must prove. Neither exempts implementation premises.
//!
//! Applied signatures own trait substitution. Canonical declaration projection
//! owns exact fact identity, with formal symbols bound by position. The existing
//! scoped integer prover supplies implication beyond exact correspondence;
//! unknown implication refuses, rather than comparing diagnostic spellings.

use super::law_conformance::{indexed_law_binder_arguments, term_mentions_variable};
use super::structural_terms::structural_term;
use super::{
    RESULT_BINDER, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication_for_declaration,
};
use crate::proof_contracts::contract_structure::{
    CorrespondenceFact, callable_binders, correspondence_fact, generic_binders,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::{
    SignatureContract, SignatureContractKind, StateParameter, StateSignature,
};
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn check_contract_refinement(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &typed_trees::state::State,
    member_parameters: &[StateParameter],
    invocation_parameters: &[typed_trees::data::TypeParameter],
    application: &AppliedTraitScope<'_>,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let trait_definition = application.declaration;
    let requirement_name = format!("{}::{}", trait_definition.name, requirement.name);
    let applied = program.applied_callable_signature(application, requirement.symbol);
    let requirement = match applied {
        Some(applied) => applied,
        None if requirement.type_parameters.is_empty()
            && application.substitutions.is_empty()
            && trait_definition.lifetime_parameters.is_empty() =>
        {
            requirement
        }
        None => {
            diagnostics.push(Diagnostic::error(format!(
                "requirement `{requirement_name}` has no prepared contract for its exact trait application"
            )));
            return;
        }
    };
    let required_parameters = program.state_signature_parameters(requirement);
    if required_parameters.len() != member_parameters.len() {
        // Signature correspondence owns the arity diagnostic.
        return;
    }
    // Capture binders remain lexical operands, independent of the member's
    // invocation family. Both sides of trait application share their exact
    // symbols, while equally spelled invocation binders keep their own slots.
    let captured = program
        .machine_type_reference_scope(machine, |left, right| left.symbol == right.symbol)
        .0
        .into_iter()
        .filter(|parameter| {
            !invocation_parameters
                .iter()
                .any(|local| local.symbol == parameter.symbol)
        })
        .enumerate()
        .map(|(position, parameter)| (parameter.symbol, format!("$capture{position}")))
        .collect::<Vec<_>>();
    let required_generic = program.state_signature_type_parameters(requirement);
    if !binder_mapping_is_unique(&generic_binders(required_generic), &captured)
        || !binder_mapping_is_unique(&generic_binders(invocation_parameters), &captured)
    {
        diagnostics.push(Diagnostic::error(
            "contract refinement has duplicate declaration-symbol binder mappings",
        ));
        return;
    }
    let mut required = ContractFacts::collect(
        program,
        requirement.symbol,
        program.state_signature_contracts(requirement).iter(),
        required_parameters,
        requirement.return_type,
        program.state_signature_type_parameters(requirement),
        &captured,
    );
    if let Err(diagnostic) = required.instantiate_family_packs(
        program,
        program.state_signature_type_parameters(requirement),
        required_parameters,
        invocation_parameters,
        member_parameters,
        &captured,
    ) {
        diagnostics.push(diagnostic);
        return;
    }
    let member = ContractFacts::collect(
        program,
        machine.symbol,
        program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(entry)),
        member_parameters,
        entry.return_type,
        invocation_parameters,
        &captured,
    );
    let owes_through_edge = program
        .machine_trait_conformances(machine)
        .iter()
        .any(|edge| {
            edge.requirement_symbol == requirement.symbol
                && AppliedTraitScope::from_machine_conformance(
                    program.typed_program(),
                    machine,
                    edge,
                )
                .is_some_and(|edge| edge.same_application(program.typed_program(), application))
        });
    for conclusion in &required.ensures {
        if owes_through_edge
            || conclusion.law_checked
            || entailed(program, &member.ensures, &member, conclusion, &required)
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` realizes `{requirement_name}` but does not guarantee its `ensures {}`; callers of the requirement assume it",
            machine.name, render(program, &conclusion.fact),
        )));
    }
    for premise in &member.requires {
        if entailed(program, &required.requires, &required, premise, &member) {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` realizes `{requirement_name}` but requires `{}`, which the requirement does not imply; callers of the requirement prove only its own `requires`",
            machine.name, render(program, &premise.fact),
        )));
    }
}

struct ContractFacts {
    owner: SymbolHandle,
    bindings: Vec<ScopedArithmeticBinding>,
    requires: Vec<Conjunct>,
    ensures: Vec<Conjunct>,
}

struct Conjunct {
    fact: ProofFact,
    identity: Option<CorrespondenceFact>,
    law_checked: bool,
}

impl ContractFacts {
    fn collect<'a>(
        program: &dyn crate::CheckingInput,
        owner: SymbolHandle,
        contracts: impl Iterator<Item = &'a SignatureContract>,
        parameters: &[StateParameter],
        result: TypeReferenceHandle,
        generics: &[typed_trees::data::TypeParameter],
        captured: &[(SymbolHandle, String)],
    ) -> Self {
        let mut generic = generic_binders(generics);
        generic.extend_from_slice(captured);
        let binders = callable_binders(&generic, parameters);
        let mut facts = Self {
            owner,
            bindings: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        };
        for (position, parameter) in parameters.iter().enumerate() {
            facts.bind_integer(
                program,
                ScopedArithmeticBinder::Symbol(parameter.symbol),
                parameter.type_reference,
                format!("\0contract-parameter:{position}"),
            );
        }
        facts.bind_integer(
            program,
            ScopedArithmeticBinder::Result,
            result,
            "\0contract-result".to_owned(),
        );
        for contract in contracts {
            let target = match contract.kind {
                SignatureContractKind::Requires => &mut facts.requires,
                SignatureContractKind::Ensures => &mut facts.ensures,
                _ => continue,
            };
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let mut conjuncts = Vec::new();
                match fact {
                    ProofFact::Expression(expression) => {
                        split_conjuncts(program, *expression, &mut conjuncts)
                    }
                    _ => conjuncts.push(fact.clone()),
                }
                for fact in conjuncts {
                    let law_checked = match &fact {
                        ProofFact::Expression(expression) => is_law_conjunct(program, *expression),
                        ProofFact::Proposition(_) => true,
                        ProofFact::Membership(_) => false,
                    };
                    target.push(Conjunct {
                        identity: correspondence_fact(program, &fact, &binders),
                        fact,
                        law_checked,
                    });
                }
            }
        }
        facts
    }

    /// A requirement over a carrier family states its premises without index
    /// packs (`R(left, right)`); the laws quantify those packs independently
    /// and the realizer's binders instantiate them through its parameter
    /// types (quotients.md, "Families, relations, and evidence"). Recover
    /// each omitted pack by the same rule law conformance uses, then compare
    /// the completed premise in the realizer's binder namespace. A pack that
    /// cannot be recovered keeps the premise without identity, which refuses.
    fn instantiate_family_packs(
        &mut self,
        program: &dyn crate::CheckingInput,
        required_generics: &[typed_trees::data::TypeParameter],
        required_parameters: &[StateParameter],
        member_generics: &[typed_trees::data::TypeParameter],
        member_parameters: &[StateParameter],
        captured: &[(SymbolHandle, String)],
    ) -> Result<(), Diagnostic> {
        let mut generics = generic_binders(required_generics);
        generics.extend_from_slice(captured);
        for (symbol, name) in generic_binders(member_generics) {
            if let Some((_, existing)) = generics.iter().find(|(existing, _)| *existing == symbol) {
                if *existing != name {
                    return Err(Diagnostic::error(
                        "contract family refinement has conflicting declaration-symbol binder mappings",
                    ));
                }
            } else {
                generics.push((symbol, name));
            }
        }
        let binders = callable_binders(&generics, required_parameters);
        for conjunct in &mut self.requires {
            let ProofFact::Proposition(application) = &conjunct.fact else {
                continue;
            };
            if conjunct.identity.is_some() || !application.binder_arguments.is_empty() {
                continue;
            }
            let Some(binder_arguments) = indexed_law_binder_arguments(
                program,
                application,
                required_parameters,
                member_parameters,
            ) else {
                continue;
            };
            let completed = typed_trees::proposition::PropositionApplication {
                binder_arguments: binder_arguments.into(),
                ..application.clone()
            };
            conjunct.identity =
                correspondence_fact(program, &ProofFact::Proposition(completed), &binders);
        }
        Ok(())
    }

    fn bind_integer(
        &mut self,
        program: &dyn crate::CheckingInput,
        binder: ScopedArithmeticBinder,
        reference: TypeReferenceHandle,
        identity: String,
    ) {
        let Some(primitive) = program
            .primitive_type_reference(reference)
            .filter(|primitive| primitive.is_fixed_integer())
        else {
            return;
        };
        self.bindings.push(ScopedArithmeticBinding {
            binder,
            value: ScopedArithmeticValue::Atom {
                identity,
                unsigned: matches!(
                    primitive,
                    typed_trees::unsigned_integer_primitive_pattern!()
                ),
            },
        });
    }

    fn expression(&self, expression: ExpressionHandle) -> ScopedArithmeticExpression {
        ScopedArithmeticExpression {
            owner: self.owner,
            expression,
            bindings: self.bindings.clone(),
        }
    }
}

fn entailed(
    program: &dyn crate::CheckingInput,
    premises: &[Conjunct],
    premise_scope: &ContractFacts,
    goal: &Conjunct,
    goal_scope: &ContractFacts,
) -> bool {
    if let Some(identity) = &goal.identity
        && premises
            .iter()
            .any(|premise| premise.identity.as_ref() == Some(identity))
    {
        return true;
    }
    let ProofFact::Expression(expression) = goal.fact else {
        return false;
    };
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Boolean(true)
    ) {
        return true;
    }
    let hypotheses = premises
        .iter()
        .filter_map(|premise| {
            let ProofFact::Expression(expression) = premise.fact else {
                return None;
            };
            Some(ScopedArithmeticHypothesis {
                proposition: premise_scope.expression(expression),
                holds: true,
            })
        })
        .collect::<Vec<_>>();
    matches!(
        scoped_arithmetic_implication_for_declaration(
            program,
            goal_scope.owner,
            &hypotheses,
            &goal_scope.expression(expression)
        ),
        StrictArithmeticImplicationJudgment::Proven
    )
}

fn split_conjuncts(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    output: &mut Vec<ProofFact>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => split_conjuncts(program, atomic.value, output),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            split_conjuncts(program, binary.left, output);
            split_conjuncts(program, binary.right, output);
        }
        _ => output.push(ProofFact::Expression(expression)),
    }
}

fn is_law_conjunct(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let result = RESULT_BINDER.to_owned();
    match (
        structural_term(program, None, None, binary.left, None),
        structural_term(program, None, None, binary.right, None),
    ) {
        (Some(left), Some(right)) => {
            !term_mentions_variable(&left, &result) && !term_mentions_variable(&right, &result)
        }
        _ => false,
    }
}

fn render(program: &dyn crate::CheckingInput, fact: &ProofFact) -> String {
    match fact {
        ProofFact::Expression(expression) => program.expression_table.display_name(*expression),
        ProofFact::Membership(membership) => format!(
            "{} in {}",
            program.expression_table.display_name(membership.value),
            program.symbols.display_path(membership.domain_symbol, "::")
        ),
        ProofFact::Proposition(application) => application.name.as_str().to_owned(),
    }
}

fn binder_mapping_is_unique(
    own: &[(SymbolHandle, String)],
    captures: &[(SymbolHandle, String)],
) -> bool {
    let mut seen = Vec::new();
    own.iter().chain(captures).all(|(symbol, _)| {
        if seen.contains(symbol) {
            false
        } else {
            seen.push(*symbol);
            true
        }
    })
}
