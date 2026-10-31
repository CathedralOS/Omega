//! Same-invocation reuse of one already-judged dependency body.
//!
//! Package review checks a dependency, then the dependent's own program
//! parses those bodies again and would judge their contract entailment a
//! second time. This module keeps the typed body in the dependent program
//! and reuses only that one entailment judgment, when a portable
//! correspondence says the body, its contracts, its resolved calls and the
//! correspondences of those callees are the ones already judged. The authored
//! body hash is not that admission: package identity, dependency scope, and
//! callee context must match as well.
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
/// One dependency body whose declaration, state, and expression
/// correspondence already matched a strict check. Later queries read that
/// frozen body; this row does not transfer entailment ordinals.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct FrozenDependencyBody {
    pub identity: String,
    /// Authored body and contract text. Equal text is not admission.
    pub body_hash: u64,
    /// Package, dependency scope, callee identities, and substitutions.
    /// A later consumer must match this context; the body hash does not.
    pub dependency_context: u64,
}

#[derive(Debug, Clone, Copy)]
struct BodyAdmission {
    body_hash: u64,
    dependency_context: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct DependencyBodyJudgment {
    pub identity: String,
    pub body_hash: u64,
    pub dependency_context: u64,
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
    body_hash: u64,
    dependency_context: u64,
    ensures_len: u32,
    proved_normal_return_ordinals: Vec<u32>,
    proved_entry_ordinals: Vec<u32>,
}

/// Lookup and correspondence tables for one validation pass.
pub(super) struct DependencyBodyIndex {
    active: bool,
    record: bool,
    identities: Vec<Option<String>>,
    admissions: Vec<Option<BodyAdmission>>,
    callees: Vec<Vec<u32>>,
    priors: Vec<PriorJudgment>,
    frozen_priors: Vec<FrozenDependencyBody>,
}

impl DependencyBodyIndex {
    pub(super) fn inactive() -> Self {
        Self {
            active: false,
            record: false,
            identities: Vec::new(),
            admissions: Vec::new(),
            callees: Vec::new(),
            priors: Vec::new(),
            frozen_priors: Vec::new(),
        }
    }

    pub(super) fn prepare(
        program: &dyn crate::CheckingInput,
        priors: &[DependencyBodyJudgment],
        frozen_priors: &[FrozenDependencyBody],
        record: bool,
    ) -> Self {
        if priors.is_empty() && frozen_priors.is_empty() && !record {
            return Self::inactive();
        }
        let machines = program.machines();
        let machine_count = machines.len();
        let mut machine_of_symbol = Vec::new();
        for (machine_index, machine) in machines.iter().enumerate() {
            let machine_index = u32::try_from(machine_index).unwrap_or(u32::MAX);
            for symbol in std::iter::once(machine.symbol).chain(
                program
                    .machine_states(machine)
                    .iter()
                    .map(|state| state.symbol),
            ) {
                let symbol_index = symbol.arena_index() as usize;
                if machine_of_symbol.len() <= symbol_index {
                    machine_of_symbol.resize(symbol_index + 1, u32::MAX);
                }
                machine_of_symbol[symbol_index] = machine_index;
            }
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
        let mut body_hashes = Vec::with_capacity(machine_count);
        let mut callees = Vec::with_capacity(machine_count);
        for machine in machines {
            let (body_hash, machine_callees) =
                local_body_hash(program, machine, sources.as_deref(), &machine_of_symbol);
            body_hashes.push(body_hash);
            callees.push(machine_callees);
        }
        let mut admissions = Vec::with_capacity(machine_count);
        let mut other_symbols = Vec::new();
        for (machine_index, machine) in machines.iter().enumerate() {
            let admission = body_hashes[machine_index].map(|body_hash| BodyAdmission {
                body_hash,
                dependency_context: dependency_context(
                    program,
                    machine,
                    &body_hashes,
                    &callees[machine_index],
                    &identities,
                    &machine_of_symbol,
                    &mut other_symbols,
                ),
            });
            let admission = admission.and_then(|admission| {
                substitution_context(
                    program,
                    machine,
                    admission,
                    &identities,
                    &machine_of_symbol,
                    &mut other_symbols,
                )
            });
            admissions.push(admission);
        }
        Self {
            active: true,
            record,
            identities,
            admissions,
            callees,
            priors: collapse_prior_judgments(priors),
            frozen_priors: frozen_priors.to_vec(),
        }
    }

    /// The body text and its callee correspondences match a retained body.
    pub(super) fn admits_frozen_body(&self, machine_index: usize) -> bool {
        let Some(admission) = self.admissions.get(machine_index).and_then(Option::as_ref) else {
            return false;
        };
        let Some(identity) = self.identities.get(machine_index).and_then(Option::as_ref) else {
            return false;
        };
        self.frozen_priors
            .iter()
            .any(|prior| prior.identity == *identity && same_body_and_context(prior, admission))
            && self.frozen_callees_match(machine_index)
    }

    pub(super) fn frozen_bodies(&self) -> Vec<FrozenDependencyBody> {
        if !self.record {
            return Vec::new();
        }
        self.identities
            .iter()
            .zip(&self.admissions)
            .filter_map(|(identity, admission)| {
                let admission = admission.as_ref()?;
                Some(FrozenDependencyBody {
                    identity: identity.as_ref()?.clone(),
                    body_hash: admission.body_hash,
                    dependency_context: admission.dependency_context,
                })
            })
            .collect()
    }

    fn frozen_callees_match(&self, machine_index: usize) -> bool {
        let Some(callees) = self.callees.get(machine_index) else {
            return false;
        };
        for callee in callees {
            let callee = *callee as usize;
            let Some(admission) = self.admissions.get(callee).and_then(Option::as_ref) else {
                return false;
            };
            let Some(identity) = self.identities.get(callee).and_then(Option::as_ref) else {
                return false;
            };
            if !self
                .frozen_priors
                .iter()
                .any(|prior| prior.identity == *identity && same_body_and_context(prior, admission))
            {
                return false;
            }
        }
        true
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
        let admission = self.admissions.get(machine_index)?.as_ref()?;
        let identity = self.identities.get(machine_index)?.as_ref()?;
        let prior = find_prior(&self.priors, identity)?;
        if prior.body_hash != admission.body_hash
            || prior.dependency_context != admission.dependency_context
            || !self.callees_match(machine_index)
        {
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
        if self.admissions.get(machine_index)?.is_none() {
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
            let Some(admission) = self.admissions.get(machine_index).and_then(Option::as_ref)
            else {
                continue;
            };
            judgments.push(DependencyBodyJudgment {
                identity: identity.clone(),
                body_hash: admission.body_hash,
                dependency_context: admission.dependency_context,
                ensures_len: captured.ensures_len,
                proved_normal_return_ordinals: captured.proved_normal_return_ordinals.clone(),
                proved_entry_ordinals: captured.proved_entry_ordinals.clone(),
            });
        }
        collapse_prior_judgments(&judgments)
            .into_iter()
            .map(|prior| DependencyBodyJudgment {
                identity: prior.identity,
                body_hash: prior.body_hash,
                dependency_context: prior.dependency_context,
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
            let Some(admission) = self.admissions.get(callee).and_then(Option::as_ref) else {
                return false;
            };
            let Some(identity) = self.identities.get(callee).and_then(Option::as_ref) else {
                return false;
            };
            match find_prior(&self.priors, identity) {
                Some(prior)
                    if prior.body_hash == admission.body_hash
                        && prior.dependency_context == admission.dependency_context => {}
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
            judgment.body_hash == first.body_hash
                && judgment.dependency_context == first.dependency_context
                && judgment.ensures_len == first.ensures_len
                && judgment.proved_normal_return_ordinals == first.proved_normal_return_ordinals
                && judgment.proved_entry_ordinals == first.proved_entry_ordinals
        });
        if same {
            collapsed.push(PriorJudgment {
                identity: first.identity.clone(),
                body_hash: first.body_hash,
                dependency_context: first.dependency_context,
                ensures_len: first.ensures_len,
                proved_normal_return_ordinals: first.proved_normal_return_ordinals.clone(),
                proved_entry_ordinals: first.proved_entry_ordinals.clone(),
            });
        }
        index = end;
    }
    collapsed
}

fn same_body_and_context(prior: &FrozenDependencyBody, admission: &BodyAdmission) -> bool {
    prior.body_hash == admission.body_hash
        && prior.dependency_context == admission.dependency_context
}

/// Fold a fresh clone's substitution into the dependency context. The body
/// hash stays the authored text. An in-place rewrite, or more than one
/// specialization of the same symbol, has no portable context.
fn substitution_context(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    admission: BodyAdmission,
    identities: &[Option<String>],
    machine_of_symbol: &[u32],
    other_symbols: &mut Vec<Option<String>>,
) -> Option<BodyAdmission> {
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
        return Some(admission);
    };
    let mut hash = semantic_vocabulary::Fnv1a::resume(admission.dependency_context);
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
    Some(BodyAdmission {
        body_hash: admission.body_hash,
        dependency_context: hash.finish(),
    })
}

fn dependency_context(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    body_hashes: &[Option<u64>],
    callees: &[u32],
    identities: &[Option<String>],
    machine_of_symbol: &[u32],
    other_symbols: &mut Vec<Option<String>>,
) -> u64 {
    let mut context = semantic_vocabulary::Fnv1a::new();
    hash_producer_context(&mut context, program, machine.symbol);
    for callee in callees {
        let callee = *callee as usize;
        context.byte(1);
        if let Some(identity) = identities.get(callee).and_then(Option::as_ref) {
            context.string_terminated(identity);
        } else {
            context.byte(0);
        }
        match body_hashes.get(callee).and_then(Option::as_ref) {
            Some(body_hash) => {
                context.byte(1);
                context.u64(*body_hash);
            }
            None => context.byte(0),
        }
        if let Some(machine) = program.machines().get(callee) {
            hash_producer_context(&mut context, program, machine.symbol);
        }
    }
    // Resolved call targets that are not program machines still name a dependency.
    for (state_index, state) in program.machine_states(machine).iter().enumerate() {
        for (statement_index, _) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            for call in
                crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            {
                let target = call.target_symbol(program);
                if machine_slot(machine_of_symbol, target).is_none() {
                    hash_target(
                        &mut context,
                        program,
                        target,
                        identities,
                        machine_of_symbol,
                        other_symbols,
                    );
                }
            }
            let _ = state_index;
        }
    }
    context.finish()
}

fn local_body_hash(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    sources: Option<&source::SourceMap>,
    machine_of_symbol: &[u32],
) -> (Option<u64>, Vec<u32>) {
    let Some(sources) = sources else {
        return (None, Vec::new());
    };
    let mut body = semantic_vocabulary::Fnv1a::new();
    hash_supply(&mut body, machine.supply_mode);
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
                    body.byte(1);
                    body.string_terminated(sources.text_at(span));
                }
                None => body.byte(0),
            }
            for call in
                crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            {
                let target = call.target_symbol(program);
                if let Some(callee) = machine_slot(machine_of_symbol, target)
                    && let Ok(callee) = u32::try_from(callee)
                {
                    callees.push(callee);
                }
            }
            let _ = state_index;
        }
    }
    if machine.supplies_checked_body() && statement_count > 0 && authored_statements == 0 {
        return (None, callees);
    }
    for contract in program.machine_contracts(machine) {
        body.byte(match contract.kind {
            SignatureContractKind::Requires => 1,
            SignatureContractKind::Ensures => 2,
            SignatureContractKind::EnsuresForResultCase { .. } => 3,
            SignatureContractKind::Crashes { .. } => 4,
        });
        for fact in contract.facts.handles() {
            if !hash_fact(&mut body, program, sources, fact) {
                return (None, callees);
            }
        }
    }
    (Some(body.finish()), callees)
}

fn hash_producer_context(
    hash: &mut semantic_vocabulary::Fnv1a,
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) {
    let Some(span) = program.symbols.symbol_provenance_source_span(symbol) else {
        hash.byte(0);
        return;
    };
    let Some(file) = program.symbols.source_file(span) else {
        hash.byte(0);
        return;
    };
    hash.byte(match file.dependency_scope {
        source::DependencyScope::Product => 1,
        source::DependencyScope::Build => 2,
    });
    match &file.package_identity {
        Some(package) => {
            hash.byte(1);
            hash.bytes(&package.digest());
        }
        None => hash.byte(0),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn index_for(context: u64) -> DependencyBodyIndex {
        let body_hash = 41;
        DependencyBodyIndex {
            active: true,
            record: false,
            identities: vec![Some("package:aa::wrapper".to_owned())],
            admissions: vec![Some(BodyAdmission {
                body_hash,
                dependency_context: context,
            })],
            callees: vec![Vec::new()],
            priors: Vec::new(),
            frozen_priors: vec![FrozenDependencyBody {
                identity: "package:aa::wrapper".to_owned(),
                body_hash,
                dependency_context: 7,
            }],
        }
    }

    #[test]
    fn same_body_hash_with_different_dependency_context_is_refused() {
        assert!(!index_for(9).admits_frozen_body(0));
        assert!(index_for(7).admits_frozen_body(0));
    }
}
