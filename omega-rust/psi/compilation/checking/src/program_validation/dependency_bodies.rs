//! Same-invocation reuse of one already-judged dependency body.
//!
//! Package review checks a dependency, then the dependent's own program
//! parses those bodies again and would judge their contract entailment a
//! second time. This module keeps the typed body in the dependent program
//! and reuses only that one entailment judgment, when a portable
//! correspondence says the body, its contracts, its resolved calls and the
//! correspondences of those callees are the ones already judged.
//!
//! A specialization mixes its substitution identities into the correspondence.
//! The first specialization may rewrite the template symbol in place; the
//! substitution, not the shared symbol, decides whether that judgment applies.
//! A symbol that carries more than one specialization has no single
//! correspondence and is judged again.
//!
//! A judgment records the raw proved `ensures` ordinals, before exit
//! coverage. The dependent installs those ordinals as its own expression
//! handles and runs coverage again. Coverage is a transition-guard walk;
//! the entailment proof is the work that is not repeated.

use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::SignatureContractKind;

use super::ProvenContractExpressions;
use super::statements::statement_source_span;

/// One strict check's entailment judgment for a single non-instance machine.
/// Ordinals index that machine's expression `ensures` list, which is stable
/// for a matching correspondence and is remapped into the consumer's arena.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyBodyJudgment {
    pub identity: String,
    pub correspondence: u64,
    pub ensures_len: u32,
    pub proved_normal_return_ordinals: Vec<u32>,
    pub proved_entry_ordinals: Vec<u32>,
}

/// Raw proved ordinals captured while a machine is judged, before coverage
/// decides whether those proofs describe every exit.
pub(super) struct CapturedEntailment {
    pub(super) ensures_len: u32,
    pub(super) proved_normal_return_ordinals: Vec<u32>,
    pub(super) proved_entry_ordinals: Vec<u32>,
}

struct PriorJudgment {
    identity: String,
    correspondence: u64,
    ensures_len: u32,
    proved_normal_return_ordinals: Vec<u32>,
    proved_entry_ordinals: Vec<u32>,
}

/// Lookup and correspondence tables for one validation pass.
pub(super) struct DependencyBodyIndex {
    active: bool,
    record: bool,
    identities: Vec<Option<String>>,
    correspondences: Vec<Option<u64>>,
    callees: Vec<Vec<u32>>,
    priors: Vec<PriorJudgment>,
}

impl DependencyBodyIndex {
    pub(super) fn inactive() -> Self {
        Self {
            active: false,
            record: false,
            identities: Vec::new(),
            correspondences: Vec::new(),
            callees: Vec::new(),
            priors: Vec::new(),
        }
    }

    pub(super) fn prepare(
        program: &dyn crate::CheckingInput,
        priors: &[DependencyBodyJudgment],
        record: bool,
    ) -> Self {
        if priors.is_empty() && !record {
            return Self::inactive();
        }
        let machines = program.machines();
        let machine_count = machines.len();
        let mut machine_of_symbol = Vec::new();
        for (machine_index, machine) in machines.iter().enumerate() {
            let symbol_index = machine.symbol.arena_index() as usize;
            if machine_of_symbol.len() <= symbol_index {
                machine_of_symbol.resize(symbol_index + 1, u32::MAX);
            }
            machine_of_symbol[symbol_index] = u32::try_from(machine_index).unwrap_or(u32::MAX);
        }
        let mut identities = Vec::with_capacity(machine_count);
        for machine in machines {
            identities.push(
                crate::machine_calls::contract_identities::normalized_machine_identity(
                    program, machine,
                ),
            );
        }
        let sources = program.symbols.source_map();
        let mut correspondences = Vec::with_capacity(machine_count);
        let mut callees = Vec::with_capacity(machine_count);
        let mut other_symbols = Vec::new();
        for machine in machines {
            let (correspondence, machine_callees) = local_correspondence(
                program,
                machine,
                sources.as_deref(),
                &identities,
                &machine_of_symbol,
                &mut other_symbols,
            );
            let correspondence = correspondence.and_then(|correspondence| {
                substitution_correspondence(
                    program,
                    machine,
                    correspondence,
                    &identities,
                    &machine_of_symbol,
                    &mut other_symbols,
                )
            });
            correspondences.push(correspondence);
            callees.push(machine_callees);
        }
        Self {
            active: true,
            record,
            identities,
            correspondences,
            callees,
            priors: collapse_prior_judgments(priors),
        }
    }

    pub(super) fn active_observation(&self) -> bool {
        self.active
    }

    /// Install a prior entailment when this machine and every machine it
    /// calls still have the correspondence that was judged. `None` means
    /// the body is judged again.
    pub(super) fn reuse(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        machine_index: usize,
    ) -> Option<ProvenContractExpressions> {
        if !self.active || self.priors.is_empty() {
            return None;
        }
        let correspondence = self.correspondences.get(machine_index)?.as_ref()?;
        let identity = self.identities.get(machine_index)?.as_ref()?;
        let prior = find_prior(&self.priors, identity)?;
        if prior.correspondence != *correspondence || !self.callees_match(machine_index) {
            return None;
        }
        let ensures = ensures_expressions(program, machine);
        if ensures.len() != prior.ensures_len as usize {
            return None;
        }
        proved_from_ordinals(
            &ensures,
            &prior.proved_entry_ordinals,
            &prior.proved_normal_return_ordinals,
        )
    }

    pub(super) fn capture(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        machine_index: usize,
        proven: &ProvenContractExpressions,
    ) -> Option<CapturedEntailment> {
        if !self.record {
            return None;
        }
        // Ordinal reuse transfers independent verdicts, not dependencies on
        // current-program law subjects and value observations. Rejudge such a
        // body in the consumer arena; dropping this roster would turn a
        // conditional candidate into an incomplete cached judgment.
        if !proven.conditional_content.is_empty() {
            return None;
        }
        if self.correspondences.get(machine_index)?.is_none() {
            return None;
        }
        if self.identities.get(machine_index)?.is_none() {
            return None;
        }
        let ensures = ensures_expressions(program, machine);
        let ensures_len = u32::try_from(ensures.len()).ok()?;
        Some(CapturedEntailment {
            ensures_len,
            proved_entry_ordinals: ordinals(&ensures, &proven.entry)?,
            proved_normal_return_ordinals: ordinals(&ensures, &proven.normal_returns)?,
        })
    }

    pub(super) fn judgments(
        &self,
        captured: &[Option<CapturedEntailment>],
    ) -> Vec<DependencyBodyJudgment> {
        if !self.record {
            return Vec::new();
        }
        let mut judgments = Vec::new();
        for (machine_index, captured) in captured.iter().enumerate() {
            let Some(captured) = captured else {
                continue;
            };
            let Some(identity) = self.identities.get(machine_index).and_then(Option::as_ref) else {
                continue;
            };
            let Some(correspondence) = self
                .correspondences
                .get(machine_index)
                .and_then(Option::as_ref)
            else {
                continue;
            };
            judgments.push(DependencyBodyJudgment {
                identity: identity.clone(),
                correspondence: *correspondence,
                ensures_len: captured.ensures_len,
                proved_normal_return_ordinals: captured.proved_normal_return_ordinals.clone(),
                proved_entry_ordinals: captured.proved_entry_ordinals.clone(),
            });
        }
        collapse_prior_judgments(&judgments)
            .into_iter()
            .map(|prior| DependencyBodyJudgment {
                identity: prior.identity,
                correspondence: prior.correspondence,
                ensures_len: prior.ensures_len,
                proved_normal_return_ordinals: prior.proved_normal_return_ordinals,
                proved_entry_ordinals: prior.proved_entry_ordinals,
            })
            .collect()
    }

    fn callees_match(&self, machine_index: usize) -> bool {
        let Some(callees) = self.callees.get(machine_index) else {
            return false;
        };
        for callee in callees {
            let callee = *callee as usize;
            let Some(correspondence) = self.correspondences.get(callee).and_then(Option::as_ref)
            else {
                return false;
            };
            let Some(identity) = self.identities.get(callee).and_then(Option::as_ref) else {
                return false;
            };
            match find_prior(&self.priors, identity) {
                Some(prior) if prior.correspondence == *correspondence => {}
                _ => return false,
            }
        }
        true
    }
}

fn machine_slot(machine_of_symbol: &[u32], symbol: symbols::SymbolHandle) -> Option<usize> {
    let slot = *machine_of_symbol.get(symbol.arena_index() as usize)?;
    (slot != u32::MAX).then_some(slot as usize)
}

fn find_prior<'a>(priors: &'a [PriorJudgment], identity: &str) -> Option<&'a PriorJudgment> {
    let index = priors
        .binary_search_by(|prior| prior.identity.as_str().cmp(identity))
        .ok()?;
    Some(&priors[index])
}

fn collapse_prior_judgments(judgments: &[DependencyBodyJudgment]) -> Vec<PriorJudgment> {
    let mut ordered = judgments.to_vec();
    ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut collapsed = Vec::new();
    let mut index = 0;
    while index < ordered.len() {
        let mut end = index + 1;
        while end < ordered.len() && ordered[end].identity == ordered[index].identity {
            end += 1;
        }
        let group = &ordered[index..end];
        let first = &group[0];
        let same = group.iter().all(|judgment| {
            judgment.correspondence == first.correspondence
                && judgment.ensures_len == first.ensures_len
                && judgment.proved_normal_return_ordinals == first.proved_normal_return_ordinals
                && judgment.proved_entry_ordinals == first.proved_entry_ordinals
        });
        if same {
            collapsed.push(PriorJudgment {
                identity: first.identity.clone(),
                correspondence: first.correspondence,
                ensures_len: first.ensures_len,
                proved_normal_return_ordinals: first.proved_normal_return_ordinals.clone(),
                proved_entry_ordinals: first.proved_entry_ordinals.clone(),
            });
        }
        index = end;
    }
    collapsed
}

/// Fold a fresh clone's substitution into the body correspondence. An
/// in-place rewrite, or more than one specialization of the same symbol,
/// has no portable correspondence.
fn substitution_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    correspondence: u64,
    identities: &[Option<String>],
    machine_of_symbol: &[u32],
    other_symbols: &mut Vec<Option<String>>,
) -> Option<u64> {
    let mut matched = None;
    for specialization in &program.machine_specializations {
        if specialization.instance != machine.symbol {
            continue;
        }
        if matched.is_some() {
            return None;
        }
        matched = Some(specialization);
    }
    let Some(specialization) = matched else {
        return Some(correspondence);
    };
    let mut hash = semantic_vocabulary::Fnv1a::resume(correspondence);
    hash.byte(9);
    hash.string_terminated(&specialization.normalized_template_identity);
    for text in specialization
        .type_argument_identities
        .iter()
        .chain(specialization.const_argument_identities.iter())
    {
        hash.string_terminated(text);
    }
    for symbol in specialization
        .machine_arguments
        .iter()
        .chain(specialization.conformance_arguments.iter())
    {
        hash_target(
            &mut hash,
            program,
            *symbol,
            identities,
            machine_of_symbol,
            other_symbols,
        );
    }
    for application in &specialization.conformance_applications {
        for text in application
            .lifetime_arguments
            .iter()
            .chain(application.type_arguments.iter())
            .chain(application.trait_lifetime_arguments.iter())
            .chain(application.trait_arguments.iter())
        {
            hash.string_terminated(text);
        }
        for symbol in std::iter::once(application.declaration)
            .chain(std::iter::once(application.trait_definition))
            .chain(application.machine_arguments.iter().copied())
        {
            hash_target(
                &mut hash,
                program,
                symbol,
                identities,
                machine_of_symbol,
                other_symbols,
            );
        }
        for row in &application.rows {
            for symbol in [
                row.declaring_trait,
                row.requirement,
                row.realization_machine,
                row.realization_state,
            ] {
                hash_target(
                    &mut hash,
                    program,
                    symbol,
                    identities,
                    machine_of_symbol,
                    other_symbols,
                );
            }
        }
    }
    Some(hash.finish())
}

fn local_correspondence(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    sources: Option<&source::SourceMap>,
    identities: &[Option<String>],
    machine_of_symbol: &[u32],
    other_symbols: &mut Vec<Option<String>>,
) -> (Option<u64>, Vec<u32>) {
    let Some(sources) = sources else {
        return (None, Vec::new());
    };
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash_supply(&mut hash, machine.supply_mode);
    let mut authored_statements = 0u32;
    let mut statement_count = 0u32;
    let mut callees = Vec::new();
    for (state_index, state) in program.machine_states(machine).iter().enumerate() {
        for (statement_index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            statement_count = statement_count.saturating_add(1);
            match statement_source_span(program, statement) {
                Some(span) => {
                    authored_statements = authored_statements.saturating_add(1);
                    hash.byte(1);
                    hash.string_terminated(sources.text_at(span));
                }
                None => hash.byte(0),
            }
            for call in
                crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            {
                let target = call.target_symbol(program);
                hash_target(
                    &mut hash,
                    program,
                    target,
                    identities,
                    machine_of_symbol,
                    other_symbols,
                );
                if let Some(callee) = machine_slot(machine_of_symbol, target) {
                    if let Ok(callee) = u32::try_from(callee) {
                        callees.push(callee);
                    }
                }
            }
            let _ = state_index;
        }
    }
    if machine.supplies_checked_body() && statement_count > 0 && authored_statements == 0 {
        return (None, callees);
    }
    for contract in program.machine_contracts(machine) {
        hash.byte(match contract.kind {
            SignatureContractKind::Requires => 1,
            SignatureContractKind::Ensures => 2,
            SignatureContractKind::EnsuresForResultCase { .. } => 3,
            SignatureContractKind::Crashes { .. } => 4,
        });
        for fact in contract.facts.handles() {
            if !hash_fact(&mut hash, program, sources, fact) {
                return (None, callees);
            }
        }
    }
    (Some(hash.finish()), callees)
}

fn hash_fact(
    hash: &mut semantic_vocabulary::Fnv1a,
    program: &dyn crate::CheckingInput,
    sources: &source::SourceMap,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
) -> bool {
    let authored = program
        .proof_fact_source_span(fact)
        .filter(|span| *span != source::SourceSpan::default());
    let text = if let Some(span) = authored {
        sources.text_at(span)
    } else if let ProofFact::Expression(expression) = program.proof_facts.get(fact) {
        let span = program.expression_table.source_span(*expression);
        if span == source::SourceSpan::default() {
            return false;
        }
        sources.text_at(span)
    } else {
        return false;
    };
    hash.byte(1);
    hash.string_terminated(text);
    true
}

fn hash_target(
    hash: &mut semantic_vocabulary::Fnv1a,
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    identities: &[Option<String>],
    machine_of_symbol: &[u32],
    other_symbols: &mut Vec<Option<String>>,
) {
    if let Some(machine_index) = machine_slot(machine_of_symbol, symbol) {
        if let Some(identity) = identities.get(machine_index).and_then(Option::as_ref) {
            hash.byte(1);
            hash.string_terminated(identity);
            return;
        }
    }
    let symbol_index = symbol.arena_index() as usize;
    if other_symbols.len() <= symbol_index {
        other_symbols.resize(symbol_index + 1, None);
    }
    if other_symbols[symbol_index].is_none() {
        other_symbols[symbol_index] = Some(
            crate::machine_calls::contract_identities::conformance_symbol_identity(program, symbol),
        );
    }
    hash.byte(2);
    hash.string_terminated(other_symbols[symbol_index].as_deref().unwrap_or(""));
}

fn hash_supply(hash: &mut semantic_vocabulary::Fnv1a, mode: language_semantics::MachineSupplyMode) {
    match mode {
        language_semantics::MachineSupplyMode::CheckedBody => hash.byte(0),
        language_semantics::MachineSupplyMode::Requirement => hash.byte(1),
        language_semantics::MachineSupplyMode::TopLevelRequirement => hash.byte(2),
        language_semantics::MachineSupplyMode::Boundary => hash.byte(3),
        language_semantics::MachineSupplyMode::AdmissionClaim => hash.byte(4),
        language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism } => {
            hash.byte(5);
            hash.u32(binding.map(|binding| binding.0).unwrap_or(0));
            hash.byte(
                mechanism
                    .map(language_semantics::ExternalBindingMechanism::identity_tag)
                    .unwrap_or(0),
            );
        }
    }
}

pub(super) fn ensures_expressions(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<ExpressionHandle> {
    let mut ensures = Vec::new();
    for contract in program.machine_contracts(machine) {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                ensures.push(*expression);
            }
        }
    }
    ensures
}

fn ordinals(ensures: &[ExpressionHandle], proved: &[ExpressionHandle]) -> Option<Vec<u32>> {
    let mut used = vec![false; ensures.len()];
    let mut ordinals = Vec::with_capacity(proved.len());
    for handle in proved {
        let ordinal = ensures
            .iter()
            .enumerate()
            .position(|(index, candidate)| !used[index] && candidate == handle)?;
        used[ordinal] = true;
        ordinals.push(u32::try_from(ordinal).ok()?);
    }
    Some(ordinals)
}

fn proved_from_ordinals(
    ensures: &[ExpressionHandle],
    entry: &[u32],
    normal_returns: &[u32],
) -> Option<ProvenContractExpressions> {
    let mut proven = ProvenContractExpressions::default();
    for ordinal in entry {
        proven
            .entry
            .push(*ensures.get(usize::try_from(*ordinal).ok()?)?);
    }
    for ordinal in normal_returns {
        proven
            .normal_returns
            .push(*ensures.get(usize::try_from(*ordinal).ok()?)?);
    }
    Some(proven)
}
