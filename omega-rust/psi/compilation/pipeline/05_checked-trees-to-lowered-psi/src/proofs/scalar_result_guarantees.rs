//! Export demanded scalar result relations through ordinary proved guarantees.
//!
//! A call preserves its result carrier, but the carrier alone loses relations
//! to actual parameters. Follow pending proof demands to scalar callees and
//! propose the parameter-only denotation of their emitted return values. The
//! operation semantic tables own arithmetic policy; this pass never evaluates
//! source syntax or specializes a body for one caller. Every normal return must
//! propose the same relation, and ordinary ContractEnsures reconstruction and
//! proof admission must establish it before it becomes a caller fact.

use std::collections::{BTreeMap, BTreeSet};

use lowered_psi::LoweredPsi;
use proof_admission::{PredicateDenotationError, check_value_equality_denotation};
use semantic_vocabulary::{
    BlockId, MachineId, ObligationId, PlaceId, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use terminal_psi::{ContractClause, OperationKind, TerminalMachine, Terminator};
use terminal_verifier::{ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet};

use super::ProofSearchDeclines;
use super::nonzero_divisor_certificate::{
    produce_checked_canonical_integer_proof, produce_relaxed_integer_proof,
};
use crate::direct_calls::{DirectCallRole, direct_call_parts};
use crate::lowering_error::{LoweringError, ProofSearchCapacity};

#[cfg(test)]
mod tests;

pub(super) fn retain_provable(
    lowered: &mut LoweredPsi,
    declines: &mut ProofSearchDeclines,
) -> Result<(), LoweringError> {
    let original = terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut remaining = ProofSearchCapacity::ScalarResultGuaranteeWork.limit() as usize;
    // Parameter-forwarding equations are a machine-level fact: demand
    // discovery and result-relation building share one reconstruction per
    // machine instead of re-deriving the same edge roster.
    let mut forwarded: BTreeMap<MachineId, Vec<Proposition>> = BTreeMap::new();
    let demanded = demanded_callees(lowered, &original, &mut remaining, &mut forwarded);
    let validated = terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let mut candidates = Vec::new();
    for machine in &lowered.semantic_module.machines {
        if remaining == 0 {
            break;
        }
        if !demanded.contains(&machine.id) {
            continue;
        }
        let context = validated
            .value_context(machine)
            .map_err(LoweringError::InvalidTerminalModule)?;
        if let Some(proposition) =
            result_relation(machine, &context, &mut remaining, &mut forwarded)
            && !machine
                .contract
                .ensures
                .iter()
                .any(|clause| clause.proposition == proposition)
        {
            candidates.push((machine.id, proposition));
        }
    }
    if remaining == 0 {
        declines.record(ProofSearchCapacity::ScalarResultGuaranteeWork);
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let mut fresh_obligations = super::FreshObligations::above(&lowered.semantic_module)?;
    let mut added = Vec::new();
    for (owner, proposition) in candidates {
        let Some(fresh) = fresh_obligations.mint(declines) else {
            break;
        };
        let machine = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == owner)
            .ok_or(LoweringError::Unsupported(
                "result guarantee lost its owner",
            ))?;
        machine.contract.ensures.push(ContractClause {
            proposition,
            obligation: fresh,
        });
        added.push(fresh);
    }
    // The batch is optional. Preserve every authored clause and supplied proof
    // if its new contract facts cannot all be admitted together. Finalization
    // reconstructs and emits evidence again against the retained module.
    // A valid module may have consumers that forbid additional guarantees
    // (for example, dynamic realizations). Such a proposal also declines.
    let proved = matches!(
        proves_and_preserves_evidence(lowered, &original, &added, declines),
        Ok(true)
    );
    if !proved {
        for machine in &mut lowered.semantic_module.machines {
            machine
                .contract
                .ensures
                .retain(|clause| !added.contains(&clause.obligation));
        }
    }
    Ok(())
}

/// Value identities are machine-scoped. Each site's directed definition slice
/// stays in its owner; a call result selects a callee rather than crossing into
/// the callee's unrelated value namespace.
fn demanded_callees(
    lowered: &LoweredPsi,
    original: &ReconstructedTerminalObligationSet,
    remaining: &mut usize,
    forwarded: &mut BTreeMap<MachineId, Vec<Proposition>>,
) -> BTreeSet<MachineId> {
    let supplied = lowered
        .proof_bundle
        .evidence
        .iter()
        .map(|proof| proof.obligation)
        .collect::<BTreeSet<_>>();
    let mut demanded = BTreeSet::new();
    for machine in &lowered.semantic_module.machines {
        let mut calls = BTreeMap::new();
        let mut stores = Vec::new();
        for (block, operation) in machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .map(move |operation| (block.id, operation))
        }) {
            if !charge(remaining, 1) {
                return demanded;
            }
            if let Some(call) = direct_call_parts(&operation.kind)
                && call.role == DirectCallRole::Scalar
                && let Some(result) = operation.result.scalar_ref()
            {
                calls.insert(result.id, call.callee);
            }
            if let OperationKind::StructuralScalarFieldStore {
                destination,
                field,
                value,
                ..
            } = &operation.kind
            {
                stores.push((block, *destination, *field, *value));
            }
        }
        if calls.is_empty() {
            continue;
        }
        // A machine's site axiom rosters accumulate along its flow: a deeper
        // obligation's roster extends an earlier one's verbatim. Indexing
        // every roster independently rescans the shared prefix once per
        // site — quadratic on entry-scale machines whose rosters grow to
        // tens of thousands of equations. Cover the rosters by prefix
        // chains instead: one index built over a chain's longest roster
        // serves every shorter member through its own length bound, and a
        // roster that shares no prefix simply forms a singleton chain.
        // Construction walks the reconstruction's already materialized
        // rosters, so it is bookkeeping rather than candidate search and
        // stays uncharged; the pool keeps charging every bounded visit,
        // store match, forwarded build, and denotation check below.
        let sites = original
            .obligations()
            .iter()
            .filter(|site| {
                site.owner.machine() == machine.id && !supplied.contains(&site.obligation.id)
            })
            .collect::<Vec<_>>();
        let mut order = (0..sites.len()).collect::<Vec<_>>();
        order.sort_by_key(|&index| std::cmp::Reverse(sites[index].semantic_axioms.len()));
        let mut chain_heads: Vec<usize> = Vec::new();
        let mut site_chain = vec![0usize; sites.len()];
        for index in order {
            match chain_heads.iter().position(|&head| {
                sites[head]
                    .semantic_axioms
                    .starts_with(&sites[index].semantic_axioms)
            }) {
                Some(chain) => site_chain[index] = chain,
                None => {
                    site_chain[index] = chain_heads.len();
                    chain_heads.push(index);
                }
            }
        }
        let chain_indexes = chain_heads
            .iter()
            .map(|&head| AxiomDefinitions::collect(&sites[head].semantic_axioms))
            .collect::<Vec<_>>();
        // Forwarded edges answer a demanded site, not the whole machine:
        // build them once per machine only when a site actually needs them.
        let mut forwarded_definitions: BTreeMap<ValueId, Vec<&Proposition>> = BTreeMap::new();
        if !sites.is_empty() {
            let Some(forwarded) = forwarded_equations(machine, remaining, forwarded) else {
                return demanded;
            };
            for equation in forwarded {
                if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = equation {
                    forwarded_definitions.entry(*id).or_default().push(equation);
                }
            }
        }
        let mut storage_names: BTreeMap<(BlockId, PlaceId), terminal_semantics::StorageNames> =
            BTreeMap::new();
        for (site_index, site) in sites.iter().enumerate() {
            let mut owning_blocks = None;
            let mut pending = Vec::new();
            if site
                .obligation
                .proposition
                .visit_value_ids_bounded(remaining, |value| pending.push(value))
                .is_none()
            {
                return demanded;
            }
            let definitions = &chain_indexes[site_chain[site_index]];
            let bound = site.semantic_axioms.len();
            let mut visited = BTreeSet::new();
            while let Some(value) = pending.pop() {
                if !charge(remaining, 1) {
                    return demanded;
                }
                if !visited.insert(value) {
                    continue;
                }
                if let Some(callee) = calls.get(&value) {
                    demanded.insert(*callee);
                    continue;
                }
                for equation in definitions.definitions_within(value, bound) {
                    // A surviving exact store bridges a demanded field read
                    // to a call result — reconstruction owns mutation
                    // invalidation; matching the complete field term
                    // preserves root, path and carrier identity, and this
                    // selects a proof candidate, never establishes a field
                    // fact. Without that store, a join discarded the branch
                    // equation: the store table supplies possible
                    // dependencies instead. Follow whole-storage bindings
                    // behind this demand; matching the leaf field still
                    // conservatively includes different intermediate paths.
                    // These are candidate dependencies only: each callee
                    // still proves its proposed result guarantee.
                    if let Proposition::Equal(_, observed) = equation {
                        let mut bridged = definitions
                            .field_definitions_within(observed, bound)
                            .peekable();
                        if bridged.peek().is_some() {
                            for equation in bridged {
                                if equation
                                    .visit_value_ids_bounded(remaining, |dependency| {
                                        if dependency != value {
                                            pending.push(dependency);
                                        }
                                    })
                                    .is_none()
                                {
                                    return demanded;
                                }
                            }
                        } else if let Some((root, path)) = match observed {
                            ScalarTerm::IntegerField { root, path, .. }
                            | ScalarTerm::BooleanField { root, path } => Some((root, path)),
                            _ => None,
                        } && let Some(
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(field),
                        ) = path.last()
                        {
                            if !charge(remaining, stores.len()) {
                                return demanded;
                            }
                            // Scalar-only demands never need graph ownership;
                            // repeated field dependencies share this site's lookup.
                            let demand_blocks = owning_blocks
                                .get_or_insert_with(|| demand_blocks(machine, site.owner));
                            for &block in demand_blocks.iter() {
                                if let std::collections::btree_map::Entry::Vacant(entry) =
                                    storage_names.entry((block, *root))
                                {
                                    if !charge(remaining, machine.blocks.len()) {
                                        return demanded;
                                    }
                                    entry.insert(terminal_semantics::storage_names(
                                        machine, block, *root,
                                    ));
                                }
                            }
                            for (block, destination, stored_field, value) in &stores {
                                if stored_field == field
                                    && demand_blocks.iter().any(|demand| {
                                        storage_names[&(*demand, *root)]
                                            .in_block(*block)
                                            .any(|name| name == *destination)
                                    })
                                {
                                    // Attach the dependency only when this
                                    // observation is demanded, rather than
                                    // selecting every scalar callee.
                                    pending.push(*value);
                                }
                            }
                        }
                    }
                    if equation
                        .visit_value_ids_bounded(remaining, |dependency| {
                            if dependency != value {
                                pending.push(dependency);
                            }
                        })
                        .is_none()
                    {
                        return demanded;
                    }
                }
                for equation in forwarded_definitions.get(&value).into_iter().flatten() {
                    if equation
                        .visit_value_ids_bounded(remaining, |dependency| {
                            if dependency != value {
                                pending.push(dependency);
                            }
                        })
                        .is_none()
                    {
                        return demanded;
                    }
                }
            }
        }
    }
    demanded
}

/// Storage observations in an operation question use that block's names;
/// arrival and cleanup questions are expressed before their exact edge.
/// Contract ensures may be demanded by any normal return.
fn demand_blocks(
    machine: &TerminalMachine,
    owner: ReconstructedTerminalObligationOwner,
) -> Vec<BlockId> {
    machine
        .blocks
        .iter()
        .filter_map(|block| {
            let owns = match owner {
                ReconstructedTerminalObligationOwner::Operation { operation, .. }
                | ReconstructedTerminalObligationOwner::CallRequires { operation, .. } => block
                    .operations
                    .iter()
                    .any(|candidate| candidate.id == operation),
                ReconstructedTerminalObligationOwner::ScalarBlockInvariant { edge, .. }
                | ReconstructedTerminalObligationOwner::NominalCleanupRequires { edge, .. } => {
                    block.terminator.edges().any(|candidate| candidate == edge)
                }
                ReconstructedTerminalObligationOwner::ContractEnsures { .. } => {
                    matches!(
                        block.terminator,
                        Terminator::Return { .. }
                            | Terminator::ReturnUnit { .. }
                            | Terminator::ReturnUnitPartialAffine { .. }
                            | Terminator::ReturnUnitNominalAffine { .. }
                            | Terminator::ReturnStructural { .. }
                    )
                }
            };
            owns.then_some(block.id)
        })
        .collect()
}

/// Position-tagged definition index over one axiom roster. A chain of
/// prefix-consistent site rosters shares one build: a member consults it
/// through its own roster length, so only equations it actually carries
/// are visible.
struct AxiomDefinitions<'a> {
    definitions: BTreeMap<ValueId, Vec<(usize, &'a Proposition)>>,
    field_definitions: BTreeMap<&'a ScalarTerm, Vec<(usize, &'a Proposition)>>,
}

impl<'a> AxiomDefinitions<'a> {
    fn collect(axioms: &'a [Proposition]) -> Self {
        let mut index = Self {
            definitions: BTreeMap::new(),
            field_definitions: BTreeMap::new(),
        };
        for (position, equation) in axioms.iter().enumerate() {
            if let Proposition::Equal(ScalarTerm::Value { id, .. }, _) = equation {
                index
                    .definitions
                    .entry(*id)
                    .or_default()
                    .push((position, equation));
            } else if let Proposition::Equal(
                field @ (ScalarTerm::IntegerField { .. } | ScalarTerm::BooleanField { .. }),
                _,
            ) = equation
            {
                index
                    .field_definitions
                    .entry(field)
                    .or_default()
                    .push((position, equation));
            }
        }
        index
    }

    /// The equations defining `value` inside the first `bound` roster
    /// positions — exactly a member site's own axioms when its roster is a
    /// prefix of the indexed one.
    fn definitions_within(
        &self,
        value: ValueId,
        bound: usize,
    ) -> impl Iterator<Item = &'a Proposition> + '_ {
        self.definitions
            .get(&value)
            .into_iter()
            .flatten()
            .take_while(move |&(position, _)| *position < bound)
            .map(|&(_, equation)| equation)
    }

    fn field_definitions_within(
        &self,
        field: &ScalarTerm,
        bound: usize,
    ) -> impl Iterator<Item = &'a Proposition> + '_ {
        self.field_definitions
            .get(field)
            .into_iter()
            .flatten()
            .take_while(move |&(position, _)| *position < bound)
            .map(|&(_, equation)| equation)
    }
}

fn charge(remaining: &mut usize, count: usize) -> bool {
    match remaining.checked_sub(count) {
        Some(rest) => {
            *remaining = rest;
            true
        }
        None => {
            *remaining = 0;
            false
        }
    }
}

/// Parameter-forwarding equations are one machine-level reconstruction.
/// Share the computed roster through the pass's pool instead of paying its
/// charge again at every consumer.
fn forwarded_equations<'cache>(
    machine: &TerminalMachine,
    remaining: &mut usize,
    forwarded: &'cache mut BTreeMap<MachineId, Vec<Proposition>>,
) -> Option<&'cache Vec<Proposition>> {
    if !forwarded.contains_key(&machine.id) {
        let equations = forwarded_parameter_equations(machine, remaining)?;
        forwarded.insert(machine.id, equations);
    }
    forwarded.get(&machine.id)
}

fn result_relation(
    machine: &TerminalMachine,
    context: &PropositionContext,
    remaining: &mut usize,
    forwarded: &mut BTreeMap<MachineId, Vec<Proposition>>,
) -> Option<Proposition> {
    let result = machine.result.scalar_ref()?;
    if !charge(remaining, machine.parameters.len())
        || !charge(remaining, machine.contract.erased_scalar_formals.len())
    {
        return None;
    }
    for block in &machine.blocks {
        if !charge(remaining, 1)
            || !charge(remaining, block.parameters.len())
            || !charge(remaining, block.erased_scalar_formals.len())
            || !charge(remaining, block.operations.len())
        {
            return None;
        }
    }
    let scope = PropositionContext::from_value_types(
        machine
            .parameters
            .iter()
            .chain(std::iter::once(result))
            .map(|value| (value.id, value.scalar_type)),
    )
    .ok()?;
    let value_types: BTreeMap<ValueId, ScalarType> = machine
        .parameters
        .iter()
        .chain(machine.contract.erased_scalar_formals.iter())
        .chain(std::iter::once(result))
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.erased_scalar_formals),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.scalar_ref()),
        )
        .map(|value| (value.id, value.scalar_type))
        .collect();
    let mut equations = forwarded_equations(machine, remaining, forwarded)?.clone();
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        *remaining = remaining.checked_sub(1)?;
        if let Some(meaning) =
            terminal_semantics::goal_free_scalar_leaf_semantics(operation, &value_types).ok()?
        {
            equations.push(meaning.result_equation().clone());
        } else if let Some(meaning) =
            terminal_semantics::proof_bearing_scalar_leaf_semantics(operation, &value_types).ok()?
        {
            equations.push(meaning.result_equation().clone());
        } else if let Some(equation) =
            terminal_verifier::reconstruct_trapping_integer_normal_return_equation(
                operation,
                &value_types,
            )
            .ok()?
        {
            // A trap never reaches a normal return. Reuse its verifier-owned
            // continuation equation as a candidate, not a caller assumption:
            // every return still owes the ordinary ContractEnsures proof.
            equations.push(equation);
        }
    }
    let mut common = None;
    for block in &machine.blocks {
        let Terminator::Return { value, .. } = block.terminator else {
            continue;
        };
        *remaining = remaining.checked_sub(1)?;
        let relation = Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            ScalarTerm::value(value, *value_types.get(&value)?),
        );
        relation.visit_value_ids_bounded(remaining, |_| {})?;
        for equation in &equations {
            equation.visit_value_ids_bounded(remaining, |_| {})?;
        }
        // Do not add the result-to-return equation to the substitution roster:
        // the declared result is a boundary variable, not a body definition.
        let relation = if equations.is_empty() {
            relation
        } else {
            match check_value_equality_denotation(context, &relation, equations.iter()) {
                Ok(relation) => relation,
                Err(PredicateDenotationError::ResourceLimitExceeded) => {
                    *remaining = 0;
                    return None;
                }
                Err(_) => return None,
            }
        };
        // Expansion has the transport kernel's own finite bound. Charge its
        // complete output against the shared pool before retaining or cloning it.
        relation.visit_value_ids_bounded(remaining, |_| {})?;
        scope.validate(&relation).ok()?;
        let Proposition::Equal(left, right) = relation else {
            return None;
        };
        // Generated clauses obey the same wire ordering as authored contracts.
        let relation = super::contract_predicates::canonical_equality(left, right).ok()?;
        if common
            .as_ref()
            .is_some_and(|previous| previous != &relation)
        {
            return None;
        }
        common = Some(relation);
    }
    common
}

/// Continuation blocks can rename an input without changing its value. Follow
/// a parameter only when every incoming edge supplies the same scalar identity;
/// never pick one branch of a join or treat a case payload as an ordinary actual.
/// These equations only expand a candidate. Independent ContractEnsures replay
/// must still reconstruct the relation at every normal return.
fn forwarded_parameter_equations<'machine>(
    machine: &'machine TerminalMachine,
    remaining: &mut usize,
) -> Option<Vec<Proposition>> {
    let mut incoming: BTreeMap<BlockId, Vec<Option<&[ValueId]>>> = BTreeMap::new();
    for block in &machine.blocks {
        let mut record = |target, arguments: Option<&'machine [ValueId]>| {
            if !charge(remaining, 1 + arguments.map_or(0, <[ValueId]>::len)) {
                return None;
            }
            incoming.entry(target).or_default().push(arguments);
            Some(())
        };
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => record(*target, Some(arguments))?,
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                record(when_true.target, Some(&when_true.arguments))?;
                record(when_false.target, Some(&when_false.arguments))?;
            }
            Terminator::StructuralCase { cases, .. } => {
                for successor in cases {
                    record(successor.target, None)?;
                }
            }
            _ => {}
        }
    }
    let mut equations = Vec::new();
    for block in &machine.blocks {
        if block.id == machine.entry {
            continue;
        }
        let Some(arrivals) = incoming.get(&block.id) else {
            continue;
        };
        let Some(Some(first)) = arrivals.first() else {
            continue;
        };
        for (position, parameter) in block.parameters.iter().enumerate() {
            if !charge(remaining, arrivals.len()) {
                return None;
            }
            let actual = *first.get(position)?;
            if actual != parameter.id
                && arrivals.iter().all(|arguments| {
                    arguments.is_some_and(|arguments| arguments.get(position) == Some(&actual))
                })
            {
                equations.push(Proposition::Equal(
                    ScalarTerm::value(parameter.id, parameter.scalar_type),
                    ScalarTerm::value(actual, parameter.scalar_type),
                ));
            }
        }
    }
    Some(equations)
}

fn proves_and_preserves_evidence(
    lowered: &LoweredPsi,
    original: &ReconstructedTerminalObligationSet,
    added: &[ObligationId],
    declines: &mut ProofSearchDeclines,
) -> Result<bool, LoweringError> {
    let validated = terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let proposed = terminal_verifier::reconstruct_execution_terminal_obligations(validated)
        .map_err(LoweringError::InvalidTerminalModule)?;
    for obligation in added {
        let Some(site) = proposed.obligations().iter().find(|site| {
            site.obligation.id == *obligation
                && matches!(
                    site.owner,
                    ReconstructedTerminalObligationOwner::ContractEnsures { .. }
                )
        }) else {
            return Ok(false);
        };
        let machine = validated
            .machine(site.owner.machine())
            .ok_or(LoweringError::Unsupported(
                "result guarantee lost its proof owner",
            ))?;
        let context = validated
            .value_context(machine)
            .map_err(LoweringError::InvalidTerminalModule)?;
        let parameters = machine.parameters.iter().map(|value| value.id).collect();
        if produce_checked_canonical_integer_proof(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            &parameters,
            declines,
        )
        .or_else(|| {
            produce_relaxed_integer_proof(
                &context,
                &site.obligation.proposition,
                &site.requirements,
                &site.semantic_axioms,
                &parameters,
                declines,
            )
        })
        .is_none()
        {
            return Ok(false);
        }
    }
    super::retained_evidence::replays(validated, original, &proposed, &lowered.proof_bundle)
}
