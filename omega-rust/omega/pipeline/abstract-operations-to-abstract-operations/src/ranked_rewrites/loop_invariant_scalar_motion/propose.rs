//! Optimizer module role: proposal leaf. Component-custody-derived exact relocation candidates.

use super::{
    BlockId, LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate,
    LoopInvariantScalarMotionError, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
    NodeLocation, OperationId, OptimizationNode, PlaceId, PsiProvenance, ValueDefinitionSite,
    ValueId, VerifiedPsiOptimizationSession, apply, candidate_identity,
};
use abstract_operations::AbstractOperation;
pub(super) fn all(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError> {
    let mut candidates = Vec::new();
    for component in session.cycle_components().components() {
        if let Some(candidate) = component_candidate(session, component)? {
            candidates.push(candidate);
        }
    }
    let required = u64::try_from(candidates.len())
        .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
    if required > candidate_limit {
        return Err(LoopInvariantScalarMotionError::CandidateBudgetExhausted {
            required,
            limit: candidate_limit,
        });
    }
    Ok(candidates)
}

/// Plan the exact admissible-node relocation for one component, realize the
/// transformed unit, and bind the observed destinations into the candidate.
fn component_candidate(
    session: &VerifiedPsiOptimizationSession,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Result<Option<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError> {
    let Some(plan) = component_plan(session, component)? else {
        return Ok(None);
    };
    let output = apply::realize(
        session.unit(),
        component,
        &plan.nodes,
        plan.certificate_tail,
    )?;
    let relocations = plan
        .nodes
        .iter()
        .map(|node| {
            Ok(LoopInvariantScalarRelocation {
                destination: apply::operation_location(&output, node.psi_operation)
                    .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?,
                node: node.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
    let identity = candidate_identity(
        session.unit().identity,
        output.identity,
        &component.id,
        &relocations,
    );
    Ok(Some(LoopInvariantScalarMotionCandidate {
        identity,
        input: session.unit().identity,
        output: output.identity,
        component: component.id.clone(),
        relocations,
    }))
}

/// The independently replayable relocation plan for one component: every
/// admissible scalar node still inside a member block — a scalar-constant
/// leaf, an invariant place observation (byte-exact when its root is
/// preheader-visible or run-covered, or with its storage root rebound to the
/// representative its member structural parameter resolves to), a byte read
/// or subslice (root rebound, scalar operands substituted,
/// `length` still coupled to a `ByteSequenceLength` on the rebound root, and
/// for a subslice the structural result preserved inside the moved
/// operation), a byte-sequence-literal establishment (declared place, type,
/// and payload all preserved byte-exact inside the moved operation while
/// consumers keep spelling the same place identity), a primitive-local
/// establishment (declared place, type, and claim-free custody preserved
/// byte-exact while its scalar initializer substitutes like a computation —
/// the run orders it ahead of any `CallStructuralScalar` borrowing its
/// root), a record establishment (declared place, structural type, result
/// custody, declaration order, and range obligations preserved byte-exact
/// while each scalar field value substitutes like a computation and each
/// structural field copy's root lands like a shared-borrow call
/// argument's — the
/// whole-component custody bound proving no member stores to the declared
/// place), an invariant scalar
/// computation (an obligated variant keeps
/// its discharged obligation byte-exact inside the moved operation), an
/// invariant scalar-signature call whose callee's transitive effect summary
/// proves no observable effect, crash, or suspension and whose member roster
/// is unobservable throughout, an invariant unit-result call —
/// `CallUnit` — or scalar-result structural call — `CallStructuralScalar` —
/// whose callee passes the same effect bar, whose member roster
/// is unobservable, whose component performs no member-visible place
/// mutation or custody movement, and whose every shared-borrow structural
/// argument names a root already visible at the preheader, resolved through
/// an invariant member structural parameter, or produced by a node earlier
/// in the same run — while a mutable or write-only borrow argument names
/// only a member-produced root the same run already covers, or a
/// computation whose
/// member-internal operands are all defined by nodes earlier in the same run —
/// plus the number of countdown-certificate constants already occupying the
/// preheader tail (the dedicated countdown boundary owns their role order).
/// `nodes` is in run order: each node's member-internal producers were
/// admitted ahead of it, so the relocated sequence stays def-before-use.
pub(super) struct ComponentPlan {
    pub(super) nodes: Vec<LoopInvariantScalarNode>,
    pub(super) certificate_tail: usize,
}

/// The admission result for one member node: the operand rewrites, the
/// optional observed-root rebind, and the structural-argument root rewrites
/// the relocated node records.
struct MemberAdmission {
    operand_rewrites: Vec<(ValueId, ValueId)>,
    root_rewrite: Option<(PlaceId, PlaceId)>,
    argument_rewrites: Vec<(PlaceId, PlaceId)>,
}

/// The component-fixed evidence a member admission consults: the relocation
/// topology's non-speculative gate, the preheader insertion point the
/// representable check measures against, and the lazily computed transitive
/// effect table call admissions share.
struct PlanEvidence<'a> {
    session: &'a VerifiedPsiOptimizationSession,
    function: &'a optimization_unit::PsiOptimizationFunction,
    component: &'a optimization_unit::OptimizerCycleComponent,
    preheader_source: BlockId,
    insertion: usize,
    guaranteed_entry: bool,
    guaranteed: std::collections::BTreeSet<BlockId>,
    sites: std::collections::BTreeMap<ValueId, ValueDefinitionSite>,
    call_effects: std::cell::RefCell<Option<crate::EffectSummaryAnalysis>>,
}

impl PlanEvidence<'_> {
    /// The transitive per-function effect table call admission consults,
    /// computed once over the session's verified seed unit.
    fn effects(&self) -> std::cell::Ref<'_, crate::EffectSummaryAnalysis> {
        if self.call_effects.borrow().is_none() {
            *self.call_effects.borrow_mut() = Some(crate::validation::unit_effect_summaries(
                self.session.unit(),
            ));
        }
        std::cell::Ref::map(self.call_effects.borrow(), |slot| {
            slot.as_ref().expect("effect table just computed")
        })
    }

    /// Whether every representative `substitution` maps to is already visible
    /// where the relocated run lands: a function parameter, a preheader block
    /// parameter, a preheader node defined ahead of the run, or the result of
    /// a node the same run already covers. Representatives defined by other
    /// dominating blocks would need a dominance query this family does not
    /// run, so they stay inside the loop.
    fn representable(
        &self,
        substitution: &std::collections::BTreeMap<ValueId, ValueId>,
        relocating: &std::collections::BTreeSet<ValueId>,
    ) -> bool {
        substitution.values().all(|representative| {
            relocating.contains(representative)
                || match self.sites.get(representative) {
                    Some(ValueDefinitionSite::FunctionParameter(_)) => true,
                    Some(ValueDefinitionSite::BlockParameter { block, .. })
                        if *block == self.preheader_source =>
                    {
                        true
                    }
                    Some(ValueDefinitionSite::Node {
                        block,
                        node: defined,
                    }) if *block == self.preheader_source => {
                        usize::try_from(*defined).is_ok_and(|defined| defined < self.insertion)
                    }
                    _ => false,
                }
        })
    }
}

/// The complete admission of one member node under the current relocation
/// run — the exact predicate the fixed-point scan and the mutable-borrower
/// coverage oracle share: every non-leaf arm keeps the non-speculative gate,
/// then replays the source-shape admission from `validation`. Returns the
/// operand rewrites, observed-root rebind, and structural-argument rebinds
/// the relocation records; `None` when the node may not move.
fn admit_member_node(
    evidence: &PlanEvidence<'_>,
    member: BlockId,
    node: &OptimizationNode,
    relocating: &std::collections::BTreeSet<ValueId>,
    relocating_roots: &std::collections::BTreeSet<PlaceId>,
) -> Option<MemberAdmission> {
    let function = evidence.function;
    let component = evidence.component;
    let mut root_rewrite = None;
    let mut argument_rewrites = Vec::new();
    let operand_rewrites = if crate::validation::admissible_scalar_leaf_relocation(node) {
        Vec::new()
    } else if let Some(source) = crate::validation::admissible_invariant_place_read(node) {
        // An invariant place observation keeps both halves of the
        // non-speculative gate — observing a root performs work a skipped
        // traversal would not — and additionally needs the component to
        // preserve place custody and its storage root to land where the
        // relocated run can see it: visible at the preheader insertion
        // point, produced by a node the same run already covers, or the
        // representative its member structural parameter resolves to.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let root = crate::validation::invariant_place_observation_admission(
            function,
            component,
            node,
            relocating_roots,
        )?;
        root_rewrite = (root != source).then_some((source, root));
        Vec::new()
    } else if let Some((source, _, _)) = crate::validation::admissible_invariant_byte_read(node) {
        // A byte read keeps the same non-speculative gate as a place
        // observation — the read performs work a bypassed traversal would
        // not — then needs both evidence halves at once: its storage root
        // resolves through the shared observation-root admission while its
        // `index` and `length` operands obey the scalar substitution rule,
        // and `length` must stay paired with a `ByteSequenceLength`
        // measuring the rebound root so the moved operation still validates
        // against its obligation.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let (root, substitution) = crate::validation::invariant_byte_read_admission(
            function,
            component,
            node,
            relocating,
            relocating_roots,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        root_rewrite = (root != source).then_some((source, root));
        substitution.into_iter().collect()
    } else if let Some((source, _, _, _)) = crate::validation::admissible_invariant_subslice(node) {
        // A subslice keeps the byte family's whole evidence surface: the
        // non-speculative gate, the observation-root resolution, the
        // `start`/`end`/`length` substitution, and the `length` coupling to
        // a `ByteSequenceLength` on the rebound root. Its structural result
        // is not a definable operand — the moved operation preserves the
        // fresh view place and the bounds obligation byte-exact.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let (root, substitution) = crate::validation::invariant_subslice_admission(
            function,
            component,
            node,
            relocating,
            relocating_roots,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        root_rewrite = (root != source).then_some((source, root));
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_byte_literal(node) {
        // A byte-sequence-literal establishment is the family's first
        // non-observation structural relocation: it declares a fresh
        // immutable view root over constant bytes, reads no scalar or
        // structural operand, and carries no custody events, so the whole
        // operation — place declaration, type, and payload — moves
        // byte-exact while every consumer keeps spelling the same place
        // identity. Establishing the view still performs work a bypassed
        // traversal would not, so both halves of the non-speculative gate
        // apply and no observation-root or operand evidence exists to
        // re-derive.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        Vec::new()
    } else if crate::validation::admissible_invariant_primitive_local(node).is_some() {
        // A primitive-local establishment is the byte literal's
        // operand-carrying sibling — and the storage prerequisite a
        // `CallStructuralScalar` borrows: the cyclic eligibility fence only
        // lets a structural-scalar borrow argument name a `let mut` local's
        // place, so the call relocates only when the establishment that
        // produced that root leaves in the same run and the place lands in
        // `relocating_roots`. Establishing the cell performs work a
        // bypassed traversal would not, so both halves of the
        // non-speculative gate apply; the whole-component custody bound then
        // proves no member observes the declared place — the one condition
        // under which a cell initialized once still reads `value` on every
        // traversal — and `value` itself obeys the scalar substitution. The
        // declared place, structural type, and result custody move
        // byte-exact inside the moved operation.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let substitution = crate::validation::invariant_primitive_local_admission(
            function, component, node, relocating,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_record(node).is_some() {
        // A record establishment is the primitive local's multi-field
        // sibling: it declares a fresh claim-free unrestricted record place
        // whose declaration-ordered initializers are scalar reads or owned
        // copies of unrestricted roots.
        // Establishing the record performs work a bypassed traversal would
        // not, so both halves of the non-speculative gate apply; the
        // whole-component custody bound then proves no member stores to the
        // declared place — the one condition under which a record
        // established once still reads its initializers on every traversal —
        // each scalar field value obeys the scalar substitution, and each
        // copied root must land somewhere the run can see it: already
        // preheader-visible, resolved through an invariant member structural
        // parameter (rebound on the moved node), or produced by a node
        // earlier in the same run (the argument keeps spelling the
        // preserved place identity). The declared place, structural type,
        // result custody, declaration order, and range obligations move
        // byte-exact inside the moved operation.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let (substitution, rewrites) = crate::validation::invariant_record_admission(
            function,
            component,
            node,
            relocating,
            relocating_roots,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        argument_rewrites = rewrites;
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_scalar_array(node).is_some() {
        // A scalar-array establishment is the record's flat sibling: it
        // declares a fresh claim-free unrestricted array place whose
        // declaration-ordered scalar leaves each obey the use-site
        // substitution. Establishing the payload performs work a bypassed
        // traversal would not, so both halves of the non-speculative gate
        // apply; the whole-component custody bound then proves no member
        // stores to the declared place — the one condition under which a
        // payload established once still reads its elements on every
        // traversal. The declared place, structural type, and result custody
        // move byte-exact inside the moved operation.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let substitution = crate::validation::invariant_scalar_array_admission(
            function, component, node, relocating,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_scalar_case(node).is_some() {
        // A scalar-case establishment is the family's first custody-rewriting
        // relocation: the cyclic eligibility fence already confines its
        // affine sum result to the member block that dispatches or returns
        // it, discarding the fresh place on every dispatch edge. Moving the
        // establishment into the preheader keeps one persistent result live
        // through the whole component, so the realization strips it from
        // member-internal edges and disposes it on every exit edge and
        // member return instead — the admission replays the containment
        // bound proving the result is only ever spelled through member
        // positions that rewrite covers. Establishing the sum performs work
        // a bypassed traversal would not, so both halves of the
        // non-speculative gate apply, and each scalar field value obeys the
        // shared member-parameter substitution. An unrestricted result is
        // copyable custody instead: the shared no-member-stores bound
        // applies and no edge custody changes.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let substitution = crate::validation::invariant_scalar_case_admission(
            function, component, node, relocating,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_scalar_call(node).is_some() {
        // A scalar-signature call keeps the full non-speculative gate — it
        // performs callee work a skipped traversal would not — and then adds
        // its own evidence: the callee's transitive effect summary must
        // prove no observable effect, crash, or suspension, and every member
        // node must be unobservable so hoisting the call's possible
        // divergence cannot reorder member work anyone could observe. Each
        // scalar argument then obeys the shared member-parameter
        // substitution.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let effects = evidence.effects();
        let substitution = crate::validation::invariant_scalar_call_admission(
            function, component, node, relocating, &effects,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_unit_call(node).is_some() {
        // A unit-result call keeps the scalar call's full evidence surface —
        // the non-speculative gate, the pure transitive callee, and the
        // unobservable member roster — then adds the structural halves: the
        // component must perform no member-visible place mutation or custody
        // movement (the callee can observe caller places through its shared
        // borrows, and can write its exclusively borrowed member-produced
        // roots), every structural argument must be a borrow — never an
        // `Owned` transfer — and each argument root must land somewhere the
        // run can see it — already preheader-visible, resolved through an
        // invariant member structural parameter, or produced by a node the
        // same run already covers.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let effects = evidence.effects();
        let (substitution, rewrites) = crate::validation::invariant_unit_call_admission(
            function,
            component,
            node,
            relocating,
            relocating_roots,
            &effects,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        argument_rewrites = rewrites;
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_structural_scalar_call(node).is_some() {
        // A scalar-result structural call keeps the unit call's whole
        // evidence surface — the non-speculative gate, the pure transitive
        // callee, the unobservable member roster, the whole-component
        // place-custody bound, and every borrow argument root landing where
        // the run can see it — and then its preserved scalar result joins
        // `relocating`, so a member node consuming the call's return value
        // relocates behind it in the same run.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let effects = evidence.effects();
        let (substitution, rewrites) =
            crate::validation::invariant_structural_scalar_call_admission(
                function,
                component,
                node,
                relocating,
                relocating_roots,
                &effects,
            )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        argument_rewrites = rewrites;
        substitution.into_iter().collect()
    } else if crate::validation::admissible_invariant_structural_call(node).is_some() {
        // A structural-result call keeps the call family's effect and
        // observability evidence — the non-speculative gate, the pure
        // transitive callee, and the unobservable member roster — and then
        // adds the custody-rewriting half the scalar-case establishment
        // introduced: the cyclic eligibility fence already confines its
        // affine claim-free result to the member block that dispatches or
        // returns it, so hoisting the call keeps the one persistent result
        // live through the whole component while member-internal edges stop
        // discarding it and every exit edge and member return disposes it
        // instead. The admitted shape carries no structural arguments —
        // the callee cannot observe a caller place — and no claim,
        // obligation, crash, or evidence rows, so the moved operation keeps
        // every non-operand field byte-exact while each scalar argument
        // obeys the shared member-parameter substitution. Its declared
        // place joins `relocating_roots`, so a member node anchored on the
        // persistent result relocates behind it in the same run.
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let effects = evidence.effects();
        let substitution = crate::validation::invariant_structural_call_admission(
            function, component, node, relocating, &effects,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        substitution.into_iter().collect()
    } else {
        if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
            return None;
        }
        let substitution = crate::validation::invariant_scalar_operand_substitution(
            function, component, node, relocating,
        )?;
        if !evidence.representable(&substitution, relocating) {
            return None;
        }
        substitution.into_iter().collect()
    };
    Some(MemberAdmission {
        operand_rewrites,
        root_rewrite,
        argument_rewrites,
    })
}

/// Whether every member node that mutably borrows `root` will itself
/// relocate when `root`'s producer does: a relocated establishment leaves
/// its declared cell initialized once, so a mutable borrower that stayed
/// inside would read the cell's accumulated post-write contents where the
/// source traversal re-initialized it each iteration — the producer may only
/// move when every mutable borrower moves with it. The scan covers the whole
/// function, not just this component's roster: a borrower living in another
/// component's member — or in no component at all — cannot relocate under
/// this run, so the producer stays too. Each in-roster borrower is replayed
/// through the shared member admission with `root` already produced; the
/// admission is monotone in the run sets, so a borrower admissible now still
/// admits on the later pass that actually scans it — the producer lands
/// first and the borrower follows in the same run. A write-only borrower is
/// exempt: its callee cannot read the cell, so a staying write-only borrower
/// keeps writing the same deterministic contents nobody observes.
fn mutable_borrowers_relocate(
    evidence: &PlanEvidence<'_>,
    root: PlaceId,
    relocating: &std::collections::BTreeSet<ValueId>,
    relocating_roots: &std::collections::BTreeSet<PlaceId>,
) -> bool {
    let mut extended = relocating_roots.clone();
    extended.insert(root);
    for block in &evidence.function.blocks {
        for node in &block.nodes {
            if !crate::validation::mutable_borrow_roots(&node.operation).contains(&root) {
                continue;
            }
            if !evidence.component.members.contains(&block.id)
                || admit_member_node(evidence, block.id, node, relocating, &extended).is_none()
            {
                return false;
            }
        }
    }
    true
}

pub(super) fn component_plan(
    session: &VerifiedPsiOptimizationSession,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Result<Option<ComponentPlan>, LoopInvariantScalarMotionError> {
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    // The component's entry edges may share one preheader block even when
    // several of them exist — a multi-arm dispatch whose every arm enters the
    // cycle. Entries departing different blocks leave no unique insertion
    // point, so the component declines.
    let Some(preheader_source) = crate::validation::shared_entry_source(component) else {
        return Ok(None);
    };
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)
        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
    let Some(terminator_index) = preheader.nodes.len().checked_sub(1) else {
        return Ok(None);
    };
    // Whether reaching the preheader guarantees entering the component: every
    // successor of its terminator must be a member — each is an entry edge by
    // construction. A relocation past a terminator with a non-member
    // successor would execute on traversals that never enter the loop, so the
    // gate declines non-leaf motion when this does not hold.
    let members: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let guaranteed_entry = !preheader.nodes[terminator_index].successors.is_empty()
        && preheader.nodes[terminator_index]
            .successors
            .iter()
            .all(|edge| members.contains(&edge.target));
    let certificate_operations = certificate_operations(session, component);
    let certificate_tail = preheader.nodes[..terminator_index]
        .iter()
        .rev()
        .take_while(|node| {
            matches!(
                node.provenance.first(),
                Some(PsiProvenance::Operation(operation))
                    if certificate_operations.contains(operation)
            )
        })
        .count();
    let insertion = terminator_index - certificate_tail;
    let sites = crate::validation::value_definition_sites(function);
    // Profitability gate: a computation moves only when reaching the
    // preheader guarantees entering the component, and only out of a member
    // block guaranteed to execute on every traversal that leaves it. A block
    // a bypassing exit can skip keeps its computations inside the loop —
    // moving them would speculate work the source traversal may never
    // perform, so the boundary declines them even though every admitted
    // operation is total. Scalar-constant leaves stay exempt: re-expressing a
    // constant in the preheader performs no work the traversal could have
    // skipped. Both halves of the gate are derived topology over the
    // authenticated component, so validation replays them exactly.
    let guaranteed = crate::validation::guaranteed_executed_member_blocks(component);
    // Invariant discovery is a fixed point: a computation whose
    // member-internal operand is defined by an already-planned relocation is
    // itself invariant — the run preserves the producer's result identity and
    // orders it first — so each pass admits exactly the nodes whose remaining
    // in-loop operands the run already covers. Scanning members in roster
    // order until no pass admits anything keeps the admitted set and the run
    // order canonical for independent replay.
    let mut nodes = Vec::new();
    let mut relocating = std::collections::BTreeSet::new();
    // The place roots the run's already-admitted nodes produce: a relocated
    // establishment keeps its declared place identity byte-exact, so a
    // structural argument spelling a member-produced root stays correct when
    // the producer lands in the same run ahead of the call, and a place
    // observation reading that same member-produced root relocates behind it
    // — the persistent cell holds the same contents on every traversal.
    let mut relocating_roots = std::collections::BTreeSet::new();
    let mut admitted = std::collections::BTreeSet::new();
    let evidence = PlanEvidence {
        session,
        function,
        component,
        preheader_source,
        insertion,
        guaranteed_entry,
        guaranteed,
        sites,
        call_effects: std::cell::RefCell::new(None),
    };
    loop {
        let mut progressed = false;
        for member in &component.members {
            let block = function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
            for (index, node) in block.nodes.iter().enumerate() {
                let psi_operation = match node.provenance.first() {
                    Some(PsiProvenance::Operation(operation)) => *operation,
                    _ => continue,
                };
                if admitted.contains(&psi_operation)
                    || certificate_operations.contains(&psi_operation)
                {
                    continue;
                }
                let Some(MemberAdmission {
                    operand_rewrites,
                    root_rewrite,
                    argument_rewrites,
                }) = admit_member_node(&evidence, *member, node, &relocating, &relocating_roots)
                else {
                    continue;
                };
                // A node producing a member place root relocates only when
                // every member mutable borrower of that root relocates in
                // the same run: the moved establishment initializes its cell
                // once, so a mutable borrower left inside would read
                // accumulated post-write contents where the source traversal
                // re-initialized the cell. The oracle replays each
                // borrower's admission with the root already produced;
                // admission is monotone in the run sets, so the borrower
                // lands on a later pass.
                if let Some(root) = crate::validation::produced_place_root(&node.operation)
                    && !mutable_borrowers_relocate(&evidence, root, &relocating, &relocating_roots)
                {
                    continue;
                }
                let result = match node.definitions.as_slice() {
                    [definition] => {
                        relocating.insert(definition.value);
                        LoopInvariantNodeResult::Scalar {
                            value: definition.value,
                            scalar_type: definition.scalar_type,
                        }
                    }
                    [] => match &node.operation {
                        // Only a shape-gated subslice, byte literal,
                        // primitive local, or unit call reaches relocation
                        // without a scalar definition — any other zero- or
                        // multi-definition node cannot pass an admission
                        // gate, so reaching one here means the plan drifted.
                        AbstractOperation::ByteSequenceSubslice { result, .. }
                            if crate::validation::admissible_invariant_subslice(node).is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        AbstractOperation::EstablishByteSequenceLiteral { place, .. }
                            if crate::validation::admissible_invariant_byte_literal(node) =>
                        {
                            LoopInvariantNodeResult::LiteralPlace(*place)
                        }
                        AbstractOperation::EstablishPrimitiveLocal { result, .. }
                            if crate::validation::admissible_invariant_primitive_local(node)
                                .is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        AbstractOperation::EstablishRecord { result, .. }
                            if crate::validation::admissible_invariant_record(node).is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        AbstractOperation::EstablishScalarArray { result, .. }
                            if crate::validation::admissible_invariant_scalar_array(node)
                                .is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        AbstractOperation::EstablishScalarCase { result, .. }
                            if crate::validation::admissible_invariant_scalar_case(node)
                                .is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        AbstractOperation::CallUnit { .. }
                            if crate::validation::admissible_invariant_unit_call(node)
                                .is_some() =>
                        {
                            LoopInvariantNodeResult::Unit
                        }
                        AbstractOperation::CallStructural { result, .. }
                            if crate::validation::admissible_invariant_structural_call(node)
                                .is_some() =>
                        {
                            LoopInvariantNodeResult::Structural(result.clone())
                        }
                        _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                    },
                    _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                };
                admitted.insert(psi_operation);
                if let Some(root) = crate::validation::produced_place_root(&node.operation) {
                    relocating_roots.insert(root);
                }
                nodes.push(LoopInvariantScalarNode {
                    psi_operation,
                    result,
                    location: NodeLocation {
                        machine,
                        block: *member,
                        node: u32::try_from(index)
                            .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?,
                    },
                    operand_rewrites,
                    root_rewrite,
                    argument_rewrites,
                    provenance: node.provenance.clone(),
                    fuel: node.fuel.clone(),
                });
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    if nodes.is_empty() {
        return Ok(None);
    }
    Ok(Some(ComponentPlan {
        nodes,
        certificate_tail,
    }))
}

/// Operations owned by the authenticated countdown relation for this
/// component. The dedicated countdown boundary retains their role-ordered
/// tail; the general boundary never takes custody of them.
fn certificate_operations(
    session: &VerifiedPsiOptimizationSession,
    component: &optimization_unit::OptimizerCycleComponent,
) -> std::collections::BTreeSet<OperationId> {
    session
        .ranking_certificates()
        .certificates()
        .iter()
        .filter(|certificate| certificate.component == component.id)
        .flat_map(|certificate| {
            [
                certificate.guard.zero_operation,
                certificate.descent.one_operation,
            ]
        })
        .collect()
}
