//! Citation statements, missing-citation suggestions and citation
//! instantiation along edges.
//!
//! A citation selects an exact declaration, proves its entry premises, then
//! transports only unconditional guarantees. Completed-result tag facts use
//! the caller's local binding identity and remain separate from value equations:
//! classifying a result never invents its fields or unfolds the selected body.
//! Static callable contracts need their own substitution; a private selected
//! implementation cannot supply those public guarantees implicitly.

use crate::proof_contracts::contract_entailment::RESULT_BINDER;
use crate::proof_contracts::contract_entailment::law_conformance::{
    diagnostic_shape_match, display_structural_term, term_mentions_variable,
};
use crate::proof_contracts::contract_entailment::refuted_requires::collect_instantiated_conjuncts;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    CaseGuarantee, StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::StatementNode;

#[derive(Clone)]
pub(super) struct CitationTarget {
    symbol: SymbolHandle,
    result_binding: SymbolHandle,
    has_static_selection: bool,
}

#[derive(Clone, Default)]
pub(super) struct CitationFacts {
    pub(super) equations: Vec<(StructuralTerm, StructuralTerm)>,
    cases: Vec<CaseGuarantee>,
}

impl CitationFacts {
    pub(super) fn intake(&self, judge: &mut StructuralJudge<'_>) {
        for (left, right) in &self.equations {
            judge.intake_equation(left.clone(), right.clone(), 0);
        }
        for guarantee in &self.cases {
            judge.intake_case_guarantee(guarantee);
        }
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.equations.extend(other.equations);
        self.cases.extend(other.cases);
    }
}

fn selected_citation_machine(program: &TypedTrees, symbol: SymbolHandle) -> Option<&Machine> {
    if !symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.attached_data.is_none()
            && (machine.symbol == symbol
                || program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == symbol))
    })
}

/// Whether a statement is a CITATION: a free (receiver-less) call whose
/// callee resolves to a free PROOF MACHINE other than the enclosing one
/// (self-calls are recursion, owned by the descent checks and the IH).
pub(crate) fn is_citation_statement(
    program: &TypedTrees,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    call: &typed_trees::statement::TableCall,
) -> bool {
    if !call.receiver.is_empty() {
        return false;
    }
    selected_citation_machine(program, call.target_symbol).is_some_and(|callee| {
        !std::ptr::eq(callee, machine) && classification.is_proof_machine(program, callee)
    })
}

/// Statement-call CITATIONS (ch10 "Citing Proofs"; the settled proof-citation
/// rule, 2026-07-18): `add_zero_right(b);` inside a proof body delivers
/// the callee's proven ensures to this site, instantiated at the call's
/// argument terms -- the equations these return feed the structural judge's
/// hypotheses exactly like requires facts. Fact injection is the explicit
/// default; no global rules, no pattern matching, nothing applies silently.
///
/// Soundness: the callee's ensures is machine-checked in this same
/// validation batch (a false lemma raises its own error, so no compiling
/// program cites an unproven fact), and lemma-citing-lemma cycles are
/// machine CALL cycles -- banned by the call-graph rule, so mutual
/// false-certification is structurally impossible.
///
/// A requires-bearing lemma contributes facts only after its premises are
/// established at this statement, independently of its own conclusions.
pub(crate) fn collect_citation_equations(
    program: &TypedTrees,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
    judge: Option<&StructuralJudge>,
) -> Vec<(StructuralTerm, StructuralTerm)> {
    let mut equations = Vec::new();
    let mut site_judge = judge.cloned();
    // Machine level reads the ENTRY state only: sub-state citations
    // reference sub-state parameters, which have no machine-level frame --
    // they intake PER ARM in `recognize_structural_case_arms`, converted
    // under that arm's environment.
    let Some(entry) = program.machine_states(machine).first() else {
        return equations;
    };
    // Entry-state citations may follow authored or lowering-generated `let`
    // bindings. Termify their operands under the same incremental local
    // environment used by sub-state proof arms. Treating a local as a fresh
    // variable loses the cited equation exactly when a proof names an
    // intermediate term (`let cross = mul(..); sub_self(cross);`).
    let mut environment: Vec<(String, StructuralTerm)> = program
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            let name = parameter.name.as_str().to_owned();
            (name.clone(), StructuralTerm::Variable(name))
        })
        .collect();
    for statement in program.statement_table.statements(entry.statement_nodes) {
        if let StatementNode::LocalData(local) = statement {
            if let Some(judge) = site_judge.as_ref()
                && let Some(term) = judge.callee_term(local.initial_value, &environment, 0)
            {
                environment.push((local.name.as_str().to_owned(), term));
            }
            continue;
        }
        let Some((target, argument_handles)) = citation_call_in_statement(program, statement)
        else {
            continue;
        };
        let mut argument_terms: Vec<StructuralTerm> = Vec::with_capacity(argument_handles.len());
        let mut arguments_termify = true;
        for argument in &argument_handles {
            let term = site_judge
                .as_ref()
                .and_then(|judge| judge.callee_term(*argument, &environment, 0))
                .or_else(|| structural_term(program, *argument));
            let Some(term) = term else {
                arguments_termify = false;
                break;
            };
            argument_terms.push(term);
        }
        if !arguments_termify {
            continue;
        }
        let established = instantiate_citation(
            program,
            classification,
            machine,
            &target,
            &argument_terms,
            diagnostics,
            site_judge.as_ref(),
            false,
        );
        if let Some(judge) = &mut site_judge {
            established.intake(judge);
        }
        equations.extend(established.equations);
    }
    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
        eprintln!("CITE machine={} equations={equations:?}", machine.name);
    }
    equations
}

/// The failure-side HALF of the citation ergonomics (the OWNER_QUESTIONS
/// #14 answer, verbatim design: "when an obligation fails and a known
/// lemma's ensures shape-matches it, the diagnostic NAMES the missing
/// citation... Suggestion at failure, never silent application"). Scans
/// requires-free free proof machines for an ensures `==`-conjunct that
/// first-order matches the fenced goal (lemma parameters as pattern
/// variables, either orientation) and renders the exact citation statement
/// to write. DIAGNOSTIC ONLY -- nothing here feeds the judge.
pub(crate) fn suggest_missing_citation(
    program: &TypedTrees,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    fact: ExpressionHandle,
) -> Option<String> {
    // Walk the fact's `&&`-conjuncts; the first suggestible equation wins.
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            return suggest_missing_citation(program, classification, machine, binary.left)
                .or_else(|| {
                    suggest_missing_citation(program, classification, machine, binary.right)
                });
        }
        BinaryOperator::Equal => {}
        _ => return None,
    }
    let goal_left = structural_term(program, binary.left)?;
    let goal_right = structural_term(program, binary.right)?;

    for lemma in program.machines() {
        if lemma.attached_data.is_some()
            || std::ptr::eq(lemma, machine)
            || !classification.is_proof_machine(program, lemma)
        {
            continue;
        }
        let mut has_requires = false;
        let mut ensures_facts: Vec<ExpressionHandle> = Vec::new();
        for contract in program.machine_contracts(lemma) {
            match &contract.kind {
                SignatureContractKind::Requires => {
                    has_requires |= !program.proof_facts.span_or_empty(contract.facts).is_empty();
                }
                SignatureContractKind::Ensures => {
                    for lemma_fact in program.proof_facts.span_or_empty(contract.facts) {
                        if let ProofFact::Expression(expression) = lemma_fact {
                            ensures_facts.push(*expression);
                        }
                    }
                }
                SignatureContractKind::EnsuresForResultCase { .. }
                | SignatureContractKind::Crashes { .. } => {}
            }
        }
        // Suggestions do not establish a candidate's premises. Keep them
        // requires-free rather than propose an inapplicable citation.
        if has_requires {
            continue;
        }
        let Some(entry) = program.machine_states(lemma).first() else {
            continue;
        };
        let parameters: Vec<String> = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect();
        for lemma_fact in &ensures_facts {
            if let Some(suggestion) = suggest_conjunct_match(
                program,
                lemma,
                &parameters,
                *lemma_fact,
                &goal_left,
                &goal_right,
            ) {
                return Some(suggestion);
            }
        }
    }
    None
}

fn suggest_conjunct_match(
    program: &TypedTrees,
    lemma: &Machine,
    parameters: &[String],
    lemma_fact: ExpressionHandle,
    goal_left: &StructuralTerm,
    goal_right: &StructuralTerm,
) -> Option<String> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(lemma_fact) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            return suggest_conjunct_match(
                program,
                lemma,
                parameters,
                binary.left,
                goal_left,
                goal_right,
            )
            .or_else(|| {
                suggest_conjunct_match(
                    program,
                    lemma,
                    parameters,
                    binary.right,
                    goal_left,
                    goal_right,
                )
            });
        }
        BinaryOperator::Equal => {}
        _ => return None,
    }
    let lemma_left = structural_term(program, binary.left)?;
    let lemma_right = structural_term(program, binary.right)?;
    let result_binder = RESULT_BINDER.to_owned();
    // `result`-shaped conjuncts describe the lemma's application, not a
    // free-standing law; the law conjuncts are the suggestible material.
    if term_mentions_variable(&lemma_left, &result_binder)
        || term_mentions_variable(&lemma_right, &result_binder)
    {
        return None;
    }
    for (first, second) in [(goal_left, goal_right), (goal_right, goal_left)] {
        let mut bindings: Vec<(String, StructuralTerm)> = Vec::new();
        if diagnostic_shape_match(&lemma_left, first, parameters, &mut bindings)
            && diagnostic_shape_match(&lemma_right, second, parameters, &mut bindings)
        {
            let arguments: Vec<String> = parameters
                .iter()
                .map(|parameter| {
                    bindings
                        .iter()
                        .find(|(name, _)| name == parameter)
                        .map(|(_, term)| display_structural_term(term))
                        .unwrap_or_else(|| "..".to_owned())
                })
                .collect();
            return Some(format!(
                "note: `{lemma}` proves this shape -- cite it: `{lemma}({arguments});`",
                lemma = lemma.name.as_str(),
                arguments = arguments.join(", "),
            ));
        }
    }
    None
}

/// Extract a potential citation call from a statement: the exact target and
/// argument expression handles, for either spelling -- the bare statement
/// call (ch10's canonical form) or the let-bound call (a legal spelling in
/// its own right, and what the trailing-return auto-hoist lowers the bare
/// form into). The proof-machine gates apply in `instantiate_citation`.
///
/// NOTE the two spellings' argument spans live in DIFFERENT arenas:
/// statement calls own theirs in the statement table, expression calls in
/// the expression table.
pub(crate) fn citation_call_in_statement<'program>(
    program: &'program TypedTrees,
    statement: &'program StatementNode,
) -> Option<(CitationTarget, Vec<ExpressionHandle>)> {
    match statement {
        StatementNode::Call(call) if call.receiver.is_empty() => Some((
            CitationTarget {
                symbol: call.target_symbol,
                result_binding: SymbolHandle::invalid(),
                has_static_selection: !call.machine_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some(),
            },
            program
                .statement_table
                .expression_handles(call.arguments)
                .to_vec(),
        )),
        StatementNode::LocalData(local_data) => {
            let ExpressionNode::Call(call) = program
                .expression_table
                .expression(local_data.initial_value)
            else {
                return None;
            };
            if call.receiver.is_valid() {
                return None;
            }
            Some((
                CitationTarget {
                    symbol: call.target_symbol,
                    result_binding: local_data.symbol,
                    has_static_selection: !call.machine_arguments.is_empty()
                        || call.static_machine_parameter.is_valid()
                        || !call.evidence_arguments.is_empty()
                        || call.static_requirement_dispatch.is_some(),
                },
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            ))
        }
        _ => None,
    }
}

/// One citation for the edge judge: the callee's requires must judge Proven
/// under the CURRENT hypotheses (else the citation contributes nothing);
/// its ensures intake with `result` mapped to the call term, and a `let`
/// binder aliases the call term.
pub(crate) fn intake_citation_for_edge(
    program: &TypedTrees,
    judge: &mut StructuralJudge<'_>,
    call: &typed_trees::expression::TableCallExpression,
    binder: Option<&str>,
    _call_expression: ExpressionHandle,
) {
    let Some(callee) = program.machines().iter().find(|candidate| {
        candidate.attached_data.is_none()
            && candidate
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(candidate.name.as_str())
                == call.target.as_str()
    }) else {
        return;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let argument_handles = program.expression_table.expression_handles(call.arguments);
    if parameters.len() != argument_handles.len() {
        return;
    }
    let mut argument_terms = Vec::with_capacity(argument_handles.len());
    for argument in argument_handles {
        let Some(term) = structural_term(program, *argument) else {
            return;
        };
        argument_terms.push(term);
    }
    let call_term = StructuralTerm::Application {
        machine: call.target.as_str().to_owned(),
        arguments: argument_terms.clone(),
    };
    let mut map: Vec<(String, StructuralTerm)> = parameters
        .iter()
        .zip(argument_terms)
        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term))
        .collect();
    map.push((RESULT_BINDER.to_owned(), call_term.clone()));

    let facts = |kind: typed_trees::signature::SignatureContractKind| {
        program
            .machine_contracts(callee)
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
            })
            .collect::<Vec<_>>()
    };
    for fact in facts(typed_trees::signature::SignatureContractKind::Requires) {
        if super::structural_terms::is_case_observation(program, fact) {
            return;
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            return;
        };
        if binary.operator != BinaryOperator::Equal {
            return;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            return;
        };
        let left = StructuralJudge::substitute_term(&left, &map);
        let right = StructuralJudge::substitute_term(&right, &map);
        let verdict = judge.judge_equation(judge.resolve(left.clone()), judge.resolve(right), 0);
        if std::env::var_os("OMEGA_EDGE_TRACE").is_some() {
            eprintln!(
                "EDGE citation {} requires resolved {:?} verdict {}",
                call.target.as_str(),
                judge.resolve(left),
                match verdict {
                    StructuralJudgment::Proven => "Proven",
                    StructuralJudgment::Refuted => "Refuted",
                    StructuralJudgment::Unknown => "Unknown",
                }
            );
        }
        if !matches!(verdict, StructuralJudgment::Proven) {
            return;
        }
    }
    for fact in facts(typed_trees::signature::SignatureContractKind::Ensures) {
        if super::structural_terms::is_case_observation(program, fact) {
            continue;
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            continue;
        };
        if binary.operator != BinaryOperator::Equal {
            continue;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            continue;
        };
        judge.intake_equation(
            StructuralJudge::substitute_term(&left, &map),
            StructuralJudge::substitute_term(&right, &map),
            0,
        );
    }
    if let Some(binder) = binder {
        // A `let` binder EXPANDS to its call term (a substitution, not an
        // intake -- intake_equation orients application sides REDUCING,
        // which is exactly backwards for a binder the obligation must see
        // through).
        judge
            .substitutions
            .insert(0, (binder.to_owned(), call_term));
    }
}

/// A bare statement-call citation (no binder).
pub(crate) fn intake_statement_citation_for_edge(
    program: &TypedTrees,
    judge: &mut StructuralJudge<'_>,
    call: &typed_trees::statement::TableCall,
) {
    let Some(callee) = program.machines().iter().find(|candidate| {
        candidate.attached_data.is_none()
            && candidate
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(candidate.name.as_str())
                == call.target.as_str()
    }) else {
        return;
    };
    let Some(entry) = program.machine_states(callee).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let argument_handles = program.statement_table.expression_handles(call.arguments);
    if parameters.len() != argument_handles.len() {
        return;
    }
    let mut argument_terms = Vec::with_capacity(argument_handles.len());
    for argument in argument_handles {
        let Some(term) = structural_term(program, *argument) else {
            return;
        };
        argument_terms.push(term);
    }
    let call_term = StructuralTerm::Application {
        machine: call.target.as_str().to_owned(),
        arguments: argument_terms.clone(),
    };
    let mut map: Vec<(String, StructuralTerm)> = parameters
        .iter()
        .zip(argument_terms)
        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term))
        .collect();
    map.push((RESULT_BINDER.to_owned(), call_term));
    let collect = |kind: typed_trees::signature::SignatureContractKind| {
        program
            .machine_contracts(callee)
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
            })
            .collect::<Vec<_>>()
    };
    for fact in collect(typed_trees::signature::SignatureContractKind::Requires) {
        if super::structural_terms::is_case_observation(program, fact) {
            return;
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            return;
        };
        if binary.operator != BinaryOperator::Equal {
            return;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            return;
        };
        let left = StructuralJudge::substitute_term(&left, &map);
        let right = StructuralJudge::substitute_term(&right, &map);
        if !matches!(
            judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
            StructuralJudgment::Proven
        ) {
            return;
        }
    }
    for fact in collect(typed_trees::signature::SignatureContractKind::Ensures) {
        if super::structural_terms::is_case_observation(program, fact) {
            continue;
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            continue;
        };
        if binary.operator != BinaryOperator::Equal {
            continue;
        }
        let (Some(left), Some(right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            continue;
        };
        judge.intake_equation(
            StructuralJudge::substitute_term(&left, &map),
            StructuralJudge::substitute_term(&right, &map),
            0,
        );
    }
}

/// The machine's requires facts as expressions (empty when none).
pub(crate) fn machine_requires_facts(
    program: &TypedTrees,
    machine: &Machine,
) -> Vec<ExpressionHandle> {
    program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| {
            matches!(
                contract.kind,
                typed_trees::signature::SignatureContractKind::Requires
            )
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
        })
        .collect()
}

pub(crate) fn instantiate_citation(
    program: &TypedTrees,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    target: &CitationTarget,
    argument_terms: &[StructuralTerm],
    diagnostics: &mut Vec<Diagnostic>,
    judge: Option<&StructuralJudge>,
    allow_self_induction: bool,
) -> CitationFacts {
    let mut established = CitationFacts::default();
    let Some(callee) = selected_citation_machine(program, target.symbol) else {
        return established;
    };
    let self_citation = std::ptr::eq(callee, machine);
    let resultless_entry = program
        .machine_states(callee)
        .first()
        .is_some_and(|entry| !entry.return_type.is_valid());
    // A self-citation is an induction hypothesis only in a structurally
    // refined arm and only for a resultless theorem. The recursion validator
    // independently proves strict descent on that exact StatementNode::Call.
    // Value-returning recursive calls continue to contribute their IH through
    // the arm's returned value term; treating their lowering-generated local
    // as a second citation would duplicate and prematurely discharge it.
    if (self_citation && (!allow_self_induction || !resultless_entry))
        || !classification.is_proof_machine(program, callee)
    {
        return established;
    }
    let mut requires_facts: Vec<ExpressionHandle> = Vec::new();
    let mut requires_out_of_language = false;
    let mut ensures_facts: Vec<ExpressionHandle> = Vec::new();
    for contract in program.machine_contracts(callee) {
        match &contract.kind {
            SignatureContractKind::Requires => {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    match fact {
                        ProofFact::Expression(expression) => requires_facts.push(*expression),
                        // Membership requires are outside the structural
                        // judge's language: the site cannot discharge them.
                        ProofFact::Membership(_) | ProofFact::Proposition(_) => {
                            requires_out_of_language = true;
                        }
                    }
                }
            }
            SignatureContractKind::Ensures => {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if let ProofFact::Expression(expression) = fact {
                        ensures_facts.push(*expression);
                    }
                }
            }
            SignatureContractKind::EnsuresForResultCase { .. }
            | SignatureContractKind::Crashes { .. } => {}
        }
    }
    // The ENTRY state carries the signature; further states are the
    // lemma's own sub-proofs (add_comm's per-arm states) and do not affect
    // what a citation delivers.
    let Some(entry) = program.machine_states(callee).first() else {
        return established;
    };
    let parameters = program.state_parameters(entry);
    if parameters.len() != argument_terms.len() {
        return established;
    }
    let mut map: Vec<(String, StructuralTerm)> = Vec::with_capacity(parameters.len() + 1);
    for (parameter, term) in parameters.iter().zip(argument_terms) {
        map.push((parameter.name.as_str().to_owned(), term.clone()));
    }
    // SITE DISCHARGE (math roster N3): a theorem applies only at operands
    // satisfying its REQUIRES, so each requires conjunct instantiates under
    // the citation's argument map and must judge PROVEN against the facts
    // established before this statement (machine requires, case refinements,
    // an available IH, and earlier citations). Sites without a judge keep the
    // blanket refusal.
    if requires_out_of_language || (!requires_facts.is_empty() && judge.is_none()) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` cites `{}`, whose requires contract cannot be \
             established by this citation judgment -- its required evidence \
             must be available before importing the callee's guarantees",
            machine.name, callee.name,
        )));
        return established;
    }
    if let Some(judge) = judge {
        for fact in &requires_facts {
            if !instantiated_fact_established(program, judge, parameters, *fact, &map) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` cites `{}`, but the callee's requires fact \
                     `{}` is not established at this citation site under the \
                     citing machine's hypotheses -- add the matching requires \
                     (or cite at operands that satisfy it)",
                    machine.name,
                    callee.name,
                    program.expression_table.display_name(*fact),
                )));
                return established;
            }
        }
    }
    // `result` in the callee's ensures denotes the application itself at
    // these operands.
    map.push((
        RESULT_BINDER.to_owned(),
        StructuralTerm::Application {
            machine: callee.name.as_str().to_owned(),
            arguments: argument_terms.to_vec(),
        },
    ));
    for fact in &ensures_facts {
        collect_instantiated_conjuncts(program, *fact, &map, &mut established.equations);
    }
    // A completed local result is identified by its binding, not by a
    // name-backed application term. No static selection is guessed here.
    if !target.has_static_selection
        && let Some(judge) = judge
    {
        if target.result_binding.is_valid() {
            map.pop();
            map.push((
                RESULT_BINDER.to_owned(),
                StructuralTerm::BoundValue(target.result_binding),
            ));
        }
        for fact in ensures_facts {
            judge.instantiated_case_guarantees(callee, fact, &map, &mut established.cases);
        }
    }
    established
}

/// Does the callee's requires fact, instantiated at the citation's argument
/// map, judge PROVEN under the citing machine's hypotheses? `&&` recurses;
/// equality conjuncts and exact case predicates are supported. Unsupported
/// predicates are not established; absence of a refutation is not evidence.
pub(crate) fn instantiated_fact_established(
    program: &TypedTrees,
    judge: &StructuralJudge,
    parameters: &[typed_trees::signature::StateParameter],
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
) -> bool {
    if super::structural_terms::is_case_observation(program, fact) {
        return judge.has_exact_case_subjects()
            && matches!(
                judge.instantiated_case_judgment(parameters, fact, map),
                StructuralJudgment::Proven
            );
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::And => {
            instantiated_fact_established(program, judge, parameters, binary.left, map)
                && instantiated_fact_established(program, judge, parameters, binary.right, map)
        }
        BinaryOperator::Equal => {
            let (Some(left), Some(right)) = (
                structural_term(program, binary.left),
                structural_term(program, binary.right),
            ) else {
                return false;
            };
            let left = StructuralJudge::substitute_term(&left, map);
            let right = StructuralJudge::substitute_term(&right, map);
            matches!(
                judge.judge_equation(judge.resolve(left), judge.resolve(right), 0),
                StructuralJudgment::Proven
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{ExpressionNode, StatementNode, citation_call_in_statement};

    #[test]
    fn specialized_machine_binder_keeps_its_contract_boundary() {
        let source = "data Tree { case Empty; case Node(child: Tree); }
            machine make() -> Tree { transition { _ -> Tree::Empty } }
            machine caller() { make(); let known: Tree = make(); }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("symbols");
        let mut program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("types");
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "caller")
            .expect("caller");
        let binder = caller.symbol;
        let statements = program
            .statement_table
            .statements(program.machine_states(caller)[0].statement_nodes)
            .to_vec();
        let mut observed = 0;
        for mut statement in statements {
            let Some((ordinary, _)) = citation_call_in_statement(&program, &statement) else {
                continue;
            };
            assert!(!ordinary.has_static_selection);
            // Specialization preserves this binder even when the selected
            // target is concrete and its explicit selection arrays are empty.
            match &mut statement {
                StatementNode::Call(call) => call.static_machine_parameter = binder,
                StatementNode::LocalData(local) => {
                    let ExpressionNode::Call(call) =
                        program.expression_table.expression_mut(local.initial_value)
                    else {
                        panic!("call initializer");
                    };
                    call.static_machine_parameter = binder;
                }
                _ => panic!("citation statement"),
            }
            let (specialized, _) =
                citation_call_in_statement(&program, &statement).expect("specialized call");
            assert!(
                specialized.has_static_selection,
                "private implementation guarantees must not cross the binder contract"
            );
            observed += 1;
        }
        assert_eq!(observed, 2, "exercise bare and let-bound calls");
    }
}
