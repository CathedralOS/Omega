//!
//! This file checks law conformance of one machine.
//! `operator_contracts.rs` snapshots and checks operator contracts,
//! `law_guarantees.rs` matches machine law guarantees,
//! `proposition_laws.rs` checks proposition law conformance,
//! `inherited_requirements.rs` instantiates inherited requirement
//! propositions and `slot_bindings.rs` binds carrier slots and shapes
//! diagnostics.

mod inherited_requirements;
mod law_guarantees;
mod operator_contracts;
mod proposition_laws;
mod slot_bindings;

pub use inherited_requirements::{
    InheritedRequirementApplication, inherited_requirement_proposition_application,
    inherited_requirement_proposition_label, inherited_satisfier_parameters,
};
pub use law_guarantees::{MatchedLawGuarantee, matched_machine_law_guarantees};
pub(crate) use operator_contracts::check_operator_contract_conformance;
pub(crate) use proposition_laws::{collect_equality_conjuncts, indexed_law_binder_arguments};
pub(crate) use slot_bindings::{
    diagnostic_shape_match, display_structural_term, term_mentions_variable,
};

use super::{
    Diagnostic, ExpressionHandle, Machine, ProofFact, StateSignature, StructuralTerm,
    structural_term, unfold_constant_applications,
};
use crate::proof_contracts::contract_entailment::law_conformance::proposition_laws::{
    bindings_are_forall_general, check_proposition_law_conformance,
};
use crate::proof_contracts::contract_entailment::law_conformance::slot_bindings::{
    carrier_slot_bindings, rewrite_slot_applications,
};

pub(crate) fn check_law_conformance(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    conformance_alias: Option<&str>,
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
    matches: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
) {
    let trait_definition = application.declaration;
    // The declared law conjuncts (Equal binaries; And-chains split;
    // `result`-mentioning conjuncts are functional specs, not laws -- they
    // stay outside this check, exactly like the suggestion path).
    let mut law_conjuncts: Vec<ExpressionHandle> = Vec::new();
    let mut proposition_laws = Vec::new();
    for fact in crate::proof_contracts::proof_facts::ensures_facts(
        program,
        program.state_signature_contracts(requirement),
    ) {
        match fact {
            ProofFact::Expression(expression) => {
                collect_equality_conjuncts(program, *expression, &mut law_conjuncts);
            }
            ProofFact::Proposition(application) => proposition_laws.push(application),
            ProofFact::Membership(_) => {}
        }
    }
    if law_conjuncts.is_empty() && proposition_laws.is_empty() {
        return; // an OP requirement, not a law
    }

    let requirement_parameters: Vec<String> = program
        .state_signature_parameters(requirement)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();

    let Some(entry_state) = program.machine_states(machine).first() else {
        return; // the signature check already flagged a stateless machine
    };
    let satisfier_parameters: Vec<String> = program
        .state_parameters(entry_state)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();

    let mut proven_propositions = Vec::new();
    let mut proven_expressions = Vec::new();
    for fact in crate::proof_contracts::proof_facts::ensures_facts(
        program,
        program.machine_contracts(machine),
    ) {
        match fact {
            ProofFact::Proposition(application) => proven_propositions.push(application),
            ProofFact::Expression(expression) => proven_expressions.push(*expression),
            ProofFact::Membership(_) => {}
        }
    }
    check_proposition_law_conformance(
        program,
        machine,
        application,
        requirement,
        &proposition_laws,
        &proven_propositions,
        &proven_expressions,
        diagnostics,
    );
    if law_conjuncts.is_empty() {
        return;
    }

    // The CARRIER is the satisfier's first entry parameter type (law
    // requirements are Self-shaped; the signature check already bound Self
    // there), or its return type for parameterless requirements.
    let carrier = program
        .state_parameters(entry_state)
        .first()
        .map(|parameter| parameter.type_reference)
        .unwrap_or(entry_state.return_type);

    // The trait's op-slot names, and the carrier's bound machine for each.
    let slot_names: Vec<String> = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .map(|signature| signature.name.as_str().to_owned())
        .collect();

    // The satisfier's own PROVEN ensures conjuncts (machine-checked by this
    // engine before this point -- compiling means proven).
    let mut proven_conjuncts: Vec<(ExpressionHandle, ExpressionHandle)> = Vec::new();
    for fact in crate::proof_contracts::proof_facts::ensures_facts(
        program,
        program.machine_contracts(machine),
    ) {
        if let Some(expression) = fact.as_expression() {
            let mut conjuncts = Vec::new();
            collect_equality_conjuncts(program, *expression, &mut conjuncts);
            proven_conjuncts.extend(
                conjuncts
                    .into_iter()
                    .map(|conjunct| (conjunct, *expression)),
            );
        }
    }

    for law_conjunct in &law_conjuncts {
        if crate::proof_contracts::contract_results::expression_uses_reserved_result(
            program,
            *law_conjunct,
        ) {
            // Selected content and other functional guarantees may not have
            // first-order terms. Their exact result occurrence still keeps
            // them out of law matching; ordinary exit proof remains required.
            continue;
        }
        let Some(binary) = program.expression_table.binary_node(*law_conjunct) else {
            continue;
        };
        // Contract fields and builtin measures belong to their declaration's
        // exact parameter telescope. Without that scope a projection becomes
        // opaque display text, which cannot establish public correspondence.
        // This reads declarations only; the provider still proves its own
        // ensures independently before the matched law can be consumed.
        let requirement_scope = Some((
            requirement.symbol,
            program.state_signature_parameters(requirement),
        ));
        let (Some(law_left), Some(law_right)) = (
            structural_term(program, None, requirement_scope, binary.left, None),
            structural_term(program, None, requirement_scope, binary.right, None),
        ) else {
            continue; // out-of-language law conjunct: nothing to enforce yet
        };
        // A law selects only operation slots it actually uses. Other members
        // may have several legitimate realizations (including target bodies);
        // their existence cannot make an unrelated equation ambiguous.
        let mut required_slots = Vec::new();
        rewrite_slot_applications(program, &law_left, &slot_names, &[], &mut required_slots);
        rewrite_slot_applications(program, &law_right, &slot_names, &[], &mut required_slots);
        let slot_bindings = carrier_slot_bindings(
            program,
            trait_definition,
            carrier,
            conformance_alias,
            &required_slots,
            diagnostics,
        );

        // Resolve the law's op-slot applications to the carrier's machines.
        let mut missing_slots: Vec<String> = Vec::new();
        let law_left = rewrite_slot_applications(
            program,
            &law_left,
            &slot_names,
            &slot_bindings,
            &mut missing_slots,
        );
        let law_right = rewrite_slot_applications(
            program,
            &law_right,
            &slot_names,
            &slot_bindings,
            &mut missing_slots,
        );
        // N4 identity-law bridging: nullary CONSTANT applications
        // (`zero()`, `one()`) normalize to their constructor bodies, so
        // `add(a, zero())` and the proof's `add(a, Nat::Zero)` are one
        // term.
        let law_left = unfold_constant_applications(program, law_left);
        let law_right = unfold_constant_applications(program, law_right);
        if !missing_slots.is_empty() {
            missing_slots.sort();
            missing_slots.dedup();
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}`, whose law mentions `{}` -- but no machine \
                 satisfies that requirement for this carrier (conform the op first; the law \
                 check resolves op slots through the carrier's own conformances)",
                machine.name,
                trait_definition.name,
                requirement.name,
                missing_slots.join("`, `"),
            )));
            continue;
        }

        let matched = proven_conjuncts.iter().find_map(|(proven, source)| {
            if crate::proof_contracts::contract_results::expression_uses_reserved_result(
                program, *proven,
            ) {
                return None;
            }
            let Some(proven_binary) = program.expression_table.binary_node(*proven) else {
                return None;
            };
            let provider_scope = Some((machine.symbol, program.state_parameters(entry_state)));
            let (Some(proven_left), Some(proven_right)) = (
                structural_term(program, None, provider_scope, proven_binary.left, None),
                structural_term(program, None, provider_scope, proven_binary.right, None),
            ) else {
                return None;
            };
            let proven_left = unfold_constant_applications(program, proven_left);
            let proven_right = unfold_constant_applications(program, proven_right);
            [(&proven_left, &proven_right), (&proven_right, &proven_left)]
                .into_iter()
                .any(|(first, second)| {
                    // Every guarantee belongs to this invocation's exact
                    // formal positions. Readonly inputs and proof-only values
                    // may still have asymmetric entry premises or normal-exit
                    // conditions; a bijection is not a contract substitution.
                    let mut bindings: Vec<(String, StructuralTerm)> = requirement_parameters
                        .iter()
                        .zip(&satisfier_parameters)
                        .map(|(required, actual)| {
                            (required.clone(), StructuralTerm::Variable(actual.clone()))
                        })
                        .collect();
                    diagnostic_shape_match(&law_left, first, &requirement_parameters, &mut bindings)
                        && diagnostic_shape_match(
                            &law_right,
                            second,
                            &requirement_parameters,
                            &mut bindings,
                        )
                        && bindings_are_forall_general(&bindings, &satisfier_parameters)
                })
                .then_some(*source)
        });

        if let Some(source) = matched {
            matches.push((*law_conjunct, source));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but proves no ensures matching the declared \
                 law `{} == {}` -- a law requirement's satisfier must carry that equation as a \
                 machine-checked ensures, general in every law parameter",
                machine.name,
                trait_definition.name,
                requirement.name,
                display_structural_term(&law_left),
                display_structural_term(&law_right),
            )));
        }
    }
}
