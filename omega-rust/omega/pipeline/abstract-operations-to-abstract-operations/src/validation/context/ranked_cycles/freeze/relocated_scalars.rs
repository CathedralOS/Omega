//! Optimizer module role: validation leaf. Exact ranked-body relocation normalization.
//!
//! A transformed cyclic body stays frozen against the reconstructed seed
//! except for independently admitted scalar motion: an admissible
//! source-owned scalar constant leaf, an admissible place observation whose
//! component performs no place mutation or custody movement and whose storage
//! root is visible at the preheader insertion point — directly, produced by
//! a node the same component's run already relocated, or as the
//! representative an invariant member structural parameter resolves to — an
//! admissible byte observation (a `ByteSequenceRead`, or a
//! `ByteSequenceSubslice` whose structural view result and bounds obligation
//! relocate byte-exact inside the moved operation), an admissible
//! byte-sequence-literal establishment (whose declared place, structural
//! type, and payload relocate byte-exact inside the moved operation while
//! consumers keep spelling the same place identity), an admissible
//! primitive-local establishment (whose declared place, structural type, and
//! claim-free result custody relocate byte-exact while its scalar
//! initializer obeys the re-derived substitution — and whose declared root
//! is what a `CallStructuralScalar`'s borrow argument keeps
//! spelling), an admissible
//! record establishment (whose declared place, structural type, result
//! custody, declaration order, and range obligations relocate byte-exact
//! while each scalar field value obeys the re-derived substitution and each
//! structural field copy keeps or rebinds its root under the same
//! preheader-landing rule a shared-borrow call argument obeys, all under
//! the same no-member-stores custody bound — while an affine result,
//! admitted only for the field-free declaration the composed-control
//! lowering emits for a trivial affine local, re-expresses the scalar-case
//! custody frontier: the persistent preheader place stays live across
//! member-internal edges and every exit edge and member return disposes
//! it), an admissible
//! scalar-array establishment (whose declared place, structural type, and
//! claim-free unrestricted result custody relocate byte-exact while each
//! element operand obeys the re-derived substitution under the same
//! custody bound — a bound that also tolerates a member call copying a
//! member-produced unrestricted array into the callee through an `Owned`
//! argument, since the copy observes rather than moves the fresh root),
//! an admissible scalar-case establishment (whose declared place, result
//! case, and claim-free result custody relocate byte-exact while each
//! scalar field value obeys the re-derived substitution — and the family's
//! first custody-rewriting move: an affine result's dispatch edges no
//! longer discard it inside the component, since the one persistent
//! preheader place stays live across traversals, while every exit edge
//! and member return disposes it instead, a frontier the retained-member
//! comparison re-derives through the same custody rewrite rather than
//! trusting the transformed spelling; an unrestricted result instead keeps
//! the shared no-member-stores custody bound and moves byte-exact),
//! an admissible scalar-signature call (its callee's transitive effect
//! summary proves no observable effect, crash, or suspension, and every node
//! inside the member roster is unobservable, so hoisting the call's possible
//! divergence reorders nothing anyone could see), an admissible
//! borrow unit or scalar-result structural call (the same effect and
//! observability bars plus the whole-component place-custody bound and each
//! borrowed root's preheader landing — a shared borrow's root must be
//! preheader-visible, resolved through an invariant member structural
//! parameter, or produced in the same run, while a mutable or write-only
//! borrow may name only a uniquely member-produced root the same run already
//! relocated and no other member observes), an admissible
//! structural-result call — `CallStructural` — whose affine claim-free
//! result the cyclic eligibility fence already confined to the producing
//! member block's dispatch or return (the same effect and observability
//! bars, the same place-custody bound — tolerating the run's confined-result
//! discards, which the relocation re-expresses — the same borrowed-root
//! landing rule, the scalar-case containment bound on the result place, the
//! scalar argument substitution, and the same member-internal discard
//! stripping and exit disposal the establishment's custody rewrite
//! performs), or
//! an admissible scalar
//! computation (an obligated variant keeps its verifier-discharged
//! obligation byte-exact inside the moved operation) whose uses are all
//! defined outside the component, name
//! provably invariant
//! member parameters, or are defined by another node relocated
//! out of the same component's run, may relocate from one of its component's
//! member blocks into the tail of that component's unique preheader — the
//! one block every entry edge departs, which may be several edges when a
//! multi-arm dispatch enters the cycle through all of them.
//! Every non-leaf relocation also replays the proposal's non-speculative
//! custody against the authenticated topology: every successor of the
//! preheader's terminator must be an entry edge into the roster, and the
//! source member block must be one every traversal that leaves the component
//! executed — a forged move out of a bypassed member or past a terminator
//! whose successor leaves charges the node's fuel on traversals the source
//! never paid it on, so this fence rejects it rather than trusting the
//! transformed unit.
//! Moved computations rebind each invariant-parameter operand to the
//! representative every reaching edge agrees on — the substitution is
//! re-derived here from the seed, not trusted from the transformed unit —
//! while a run-internal producer operand stays bound to the result value the
//! relocation preserves, and core use/def validation keeps the run
//! def-before-use. Definition/use sites and the function-wide effect
//! sequence are derived coordinates rebuilt by core validation, so the
//! comparison below retains every source-owned field rather than the
//! refreshed coordinates.

use super::super::super::super::{
    BTreeSet, BlockId, OperationId, OptimizationBlock, OptimizationNode, PlaceId,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ScalarType, ValueId,
};

use super::super::CycleComponentId;
use super::{BTreeMap, MachineId, OptimizationUnitValidationError, OptimizerCycleComponent};
struct Moved<'function> {
    home: &'function OptimizerCycleComponent,
    expected_block: BlockId,
    expected: &'function OptimizationNode,
    current_block: BlockId,
    current_index: usize,
    current: &'function OptimizationNode,
}

pub(super) fn validate(
    machine: MachineId,
    expected_unit: &PsiOptimizationUnit,
    current: &PsiOptimizationFunction,
    components: &[&OptimizerCycleComponent],
) -> Result<(), OptimizationUnitValidationError> {
    let expected = expected_unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
            machine,
        ))?;
    // A source operation left a component's member blocks when its unique
    // current occurrence sits outside that component. Missing or duplicated
    // occurrences are never admitted moves; the frozen comparison below
    // rejects them.
    let mut moved = Vec::new();
    for component in components {
        for member in &component.members {
            let expected_block =
                block(expected, *member).ok_or_else(|| mismatch(machine, *member))?;
            for node in &expected_block.nodes {
                let Some(PsiProvenance::Operation(operation)) = node.provenance.first().copied()
                else {
                    continue;
                };
                let found = occurrences(current, operation);
                let [(current_block, current_index, current_node)] = found.as_slice() else {
                    continue;
                };
                if component.members.contains(current_block) {
                    continue;
                }
                moved.push(Moved {
                    home: component,
                    expected_block: *member,
                    expected: node,
                    current_block: *current_block,
                    current_index: *current_index,
                    current: current_node,
                });
            }
        }
    }

    if moved.is_empty() {
        // Without admitted motion the complete cyclic body, including prefix
        // definitions and exit observations, stays byte-exact frozen.
        for expected_block in &expected.blocks {
            if block(current, expected_block.id) != Some(expected_block) {
                return Err(mismatch(machine, expected_block.id));
            }
        }
        for current_block in &current.blocks {
            if block(expected, current_block.id).is_none() {
                return Err(mismatch(machine, current_block.id));
            }
        }
        return Ok(());
    }

    // Result values every relocated node defined, keyed by its home
    // component: a member-internal operand of one moved node is admissible
    // exactly when its producer relocated out of the same component's member
    // roster in the same run.
    let mut relocated_results: BTreeMap<CycleComponentId, BTreeSet<ValueId>> = BTreeMap::new();
    for relocation in &moved {
        relocated_results
            .entry(relocation.home.id.clone())
            .or_default()
            .extend(
                relocation
                    .expected
                    .definitions
                    .iter()
                    .map(|definition| definition.value),
            );
    }
    let no_relocated_results = BTreeSet::new();
    // Place roots every relocated node produced, keyed by its home
    // component: a structural argument of a moved call may keep spelling a
    // member-produced root exactly when that root's producer relocated out
    // of the same component's roster in the same run — the run preserves the
    // producer's declared place identity, so the argument stays byte-exact.
    let mut relocated_roots: BTreeMap<CycleComponentId, BTreeSet<PlaceId>> = BTreeMap::new();
    for relocation in &moved {
        if let Some(root) = crate::validation::produced_place_root(&relocation.expected.operation) {
            relocated_roots
                .entry(relocation.home.id.clone())
                .or_default()
                .insert(root);
        }
    }
    let no_relocated_roots = BTreeSet::new();
    // Affine scalar-case, empty-record, and structural-call results every
    // relocated
    // establishment or call produced,
    // keyed by its home component: the relocation re-expresses their
    // dispatch custody — member-internal edges keep the one persistent
    // place live while exit edges and member returns dispose it — so the
    // retained-member comparison below normalizes each seed node through
    // the same custody rewrite the realization performs before comparing
    // byte-exact.
    let mut relocated_case_results: BTreeMap<CycleComponentId, BTreeSet<PlaceId>> = BTreeMap::new();
    for relocation in &moved {
        let result = match &relocation.expected.operation {
            abstract_operations::AbstractOperation::EstablishScalarCase { result, .. }
            | abstract_operations::AbstractOperation::EstablishRecord { result, .. }
            | abstract_operations::AbstractOperation::CallStructural { result, .. } => result,
            _ => continue,
        };
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
            relocated_case_results
                .entry(relocation.home.id.clone())
                .or_default()
                .insert(result.place);
        }
    }

    // Each relocated node must land in its own component's unique preheader
    // ahead of the terminator that owns every entry edge, and it must retain
    // every source-owned field.
    let mut guaranteed_members: BTreeMap<CycleComponentId, BTreeSet<BlockId>> = BTreeMap::new();
    // The transitive per-function effect table a scalar-call relocation
    // replays is derived lazily — only a moved node carrying the call shape
    // computes it — and always over the reconstructed seed unit, never over
    // the transformed unit being fenced.
    let mut call_effects = None;
    for relocation in &moved {
        let component = relocation.home;
        let Some(preheader_source) = crate::validation::shared_entry_source(component) else {
            return Err(mismatch(machine, relocation.expected_block));
        };
        if relocation.current_block != preheader_source {
            return Err(mismatch(machine, relocation.expected_block));
        }
        let preheader =
            block(current, preheader_source).ok_or_else(|| mismatch(machine, preheader_source))?;
        let Some(terminator) = preheader.nodes.last() else {
            return Err(mismatch(machine, preheader_source));
        };
        // Every entry edge the component authenticates must depart this one
        // terminator: the shared preheader owns all of them, so a forged
        // destination or a missing entry edge rejects here.
        if !component.entries.iter().all(|entry| {
            terminator
                .successors
                .iter()
                .any(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
        }) {
            return Err(mismatch(machine, preheader_source));
        }
        let leaf = crate::validation::admissible_scalar_leaf_relocation(relocation.expected);
        if !leaf {
            // The proposal's non-speculative custody is re-derived here
            // rather than trusted from the transformed unit: a non-leaf
            // node may relocate only when reaching the preheader
            // guarantees entering the component — every successor of its
            // terminator is an entry edge into the roster — and only out of
            // a member block every traversal that leaves the component
            // executed. A forged move out of a bypassed member or past a
            // terminator with a non-member successor relocates work the
            // source traversal could skip, so the moved node's fuel charge
            // lands on traversals that never paid it. Scalar-constant
            // leaves stay exempt: re-expressing a constant performs no work
            // the traversal could have skipped.
            let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
            let guaranteed_entry = !terminator.successors.is_empty()
                && terminator
                    .successors
                    .iter()
                    .all(|edge| members.contains(&edge.target));
            let guaranteed = guaranteed_members
                .entry(component.id.clone())
                .or_insert_with(|| crate::validation::guaranteed_executed_member_blocks(component));
            if !guaranteed_entry || !guaranteed.contains(&relocation.expected_block) {
                return Err(mismatch(machine, relocation.expected_block));
            }
        }
        // Leaf relocations move byte-exact; invariant computations rebind
        // member parameters to the representatives this validator re-derives
        // from the expected seed, so a forged operand rewrite cannot carry
        // different authority than the loop's own edges prove. A
        // member-internal operand may instead name the result of another node
        // relocated out of the same component's roster; an operand whose
        // producer stayed inside the loop has no substitution and rejects.
        // An admitted place observation carries no operand rewrites but may
        // rebind its storage root: when the expected root is an invariant
        // member structural parameter, the root the seed resolves it to is
        // re-derived here rather than trusted from the transformed unit. A
        // byte read needs both halves at once — the scalar substitution for
        // its `index`/`length` operands and the root its storage source
        // resolves to — plus the `length` coupling that keeps the moved read
        // paired with a `ByteSequenceLength` measuring the same root, so a
        // forged source, operand, or obligation spelling rejects.
        let (substitution, root, argument_roots) = if leaf {
            (BTreeMap::new(), None, BTreeMap::new())
        } else if crate::validation::admissible_invariant_place_read(relocation.expected).is_some()
        {
            // The whole-component place-custody gate and the root's
            // landing — preheader-visible, produced by a node this
            // component's run already relocated, or resolved through the
            // member parameter's agreed representative — are re-derived here
            // from the seed rather than trusted from the transformed unit.
            match crate::validation::invariant_place_observation_admission(
                expected,
                component,
                relocation.expected,
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
            ) {
                Some(root) => (BTreeMap::new(), Some(root), BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_byte_read(relocation.expected).is_some() {
            // Both the root half and the scalar-operand half re-derive
            // from the seed: the run-internal `length` producer must
            // already appear among this component's relocated results,
            // and its own observation root must resolve to the read's
            // rebound root under the same relocated member roots.
            match crate::validation::invariant_byte_read_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
            ) {
                Some((root, substitution)) => (substitution, Some(root), BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_subslice(relocation.expected).is_some() {
            // A subslice replays the same two halves — root resolution
            // and `start`/`end`/`length` substitution with the `length`
            // coupling — while its structural result place, type,
            // multiplicity, and bounds obligation stay byte-exact inside
            // the moved operation. A forged result or obligation
            // spelling rejects in `same_relocated_node`'s operation
            // comparison.
            match crate::validation::invariant_subslice_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
            ) {
                Some((root, substitution)) => (substitution, Some(root), BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_byte_literal(relocation.expected) {
            // A byte-sequence-literal establishment relocates byte-exact:
            // its declared place, structural type, and payload stay inside
            // the moved operation, so a forged declaration or payload
            // spelling rejects in `same_relocated_node`'s operation
            // comparison. There is no observed root or scalar operand to
            // re-derive; the shared non-speculative gate above already
            // replayed because a literal is not a scalar-constant leaf.
            (BTreeMap::new(), None, BTreeMap::new())
        } else if crate::validation::admissible_invariant_scalar_call(relocation.expected).is_some()
        {
            // A scalar call replays its whole admission from the seed: the
            // callee's transitive summary must prove no observable effect,
            // crash, or suspension, and every member node must be
            // unobservable — the divergence-reordering custody a forged
            // member or callee would break. Its scalar arguments then obey
            // the same re-derived substitution a computation obeys.
            let effects = call_effects
                .get_or_insert_with(|| crate::validation::unit_effect_summaries(expected_unit));
            match crate::validation::invariant_scalar_call_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                effects,
            ) {
                Some(substitution) => (substitution, None, BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_unit_call(relocation.expected).is_some() {
            // A unit-result call replays the scalar call's whole admission
            // plus its structural halves from the seed: the pure callee,
            // the unobservable member roster, the whole-component
            // place-custody bound, the non-owned borrow whitelist, and each
            // argument root's landing — already preheader-visible, resolved
            // through an invariant member structural parameter, or produced
            // by a node this component's run already relocated; a mutable or
            // write-only borrow additionally requires that root's unique
            // member producer among the relocated set. A forged
            // move that skipped the root rebind, kept an owned argument,
            // or left the literal's producer behind rejects here or in
            // `same_relocated_node`'s operation comparison.
            let effects = call_effects
                .get_or_insert_with(|| crate::validation::unit_effect_summaries(expected_unit));
            match crate::validation::invariant_unit_call_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
                effects,
            ) {
                Some((substitution, rewrites)) => {
                    (substitution, None, rewrites.into_iter().collect())
                }
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_structural_scalar_call(
            relocation.expected,
        )
        .is_some()
        {
            // A scalar-result structural call replays the unit call's whole
            // admission from the seed — the pure callee, the unobservable
            // member roster, the place-custody bound, the non-owned borrow
            // whitelist, and each argument root's landing — and additionally
            // preserves its scalar result identity, so a forged result or a
            // skipped scalar-argument or borrow rebind rejects here or in
            // `same_relocated_node`'s operation comparison.
            let effects = call_effects
                .get_or_insert_with(|| crate::validation::unit_effect_summaries(expected_unit));
            match crate::validation::invariant_structural_scalar_call_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
                effects,
            ) {
                Some((substitution, rewrites)) => {
                    (substitution, None, rewrites.into_iter().collect())
                }
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_structural_call(relocation.expected)
            .is_some()
        {
            // A structural-result call replays the borrow calls' whole
            // admission from the seed — the pure transitive callee, the
            // unobservable member roster, the place-custody bound run with
            // this component's relocated roots plus the call's own result
            // tolerated, the non-owned borrow whitelist, and each argument
            // root's landing — plus the affine result's containment: the
            // result place must stay inside the member roster spelled only
            // through positions the custody rewrite re-expresses. A forged
            // result, scalar argument, borrowed-root rebind, or claim
            // spelling rejects here or in `same_relocated_node`'s operation
            // comparison, and a kept internal discard or missing exit
            // disposal rejects in the retained-member normalization.
            let effects = call_effects
                .get_or_insert_with(|| crate::validation::unit_effect_summaries(expected_unit));
            match crate::validation::invariant_structural_call_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
                effects,
            ) {
                Some((substitution, rewrites)) => {
                    (substitution, None, rewrites.into_iter().collect())
                }
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_primitive_local(relocation.expected)
            .is_some()
        {
            // A primitive-local establishment replays its whole admission
            // from the seed: the whole-component place-custody bound must
            // prove no member stores to the declared place — the one
            // condition under which a cell initialized once still reads its
            // `value` on every traversal — and the initializing `value`
            // operand obeys the same re-derived scalar substitution a
            // computation obeys. The declared place, structural type, and
            // result custody stay byte-exact inside the moved operation, so
            // a forged declaration or a skipped `value` rebind rejects here
            // or in `same_relocated_node`'s operation comparison.
            match crate::validation::invariant_primitive_local_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
            ) {
                Some(substitution) => (substitution, None, BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_record(relocation.expected).is_some() {
            // A record establishment replays its whole admission from the
            // seed: the whole-component place-custody bound must prove no
            // member stores to the declared place — the one condition under
            // which a record established once still reads its initializers
            // on every traversal — each scalar field value obeys the same
            // re-derived substitution a computation obeys, and each
            // structural field's copied root must land preheader-visible,
            // resolved through an invariant member structural parameter, or
            // produced by a node this component's run already relocated. The
            // declared place, structural type, result custody, declaration
            // order, and range obligations stay byte-exact inside the moved
            // operation, so a forged declaration, a skipped field rebind, or
            // a forged copied-root rewrite rejects here or in
            // `same_relocated_node`'s operation comparison.
            match crate::validation::invariant_record_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
                relocated_roots
                    .get(&component.id)
                    .unwrap_or(&no_relocated_roots),
            ) {
                Some((substitution, rewrites)) => {
                    (substitution, None, rewrites.into_iter().collect())
                }
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_scalar_array(relocation.expected)
            .is_some()
        {
            // A scalar-array establishment replays its whole admission from
            // the seed: the whole-component place-custody bound must prove no
            // member stores to the declared place — the one condition under
            // which a payload established once still reads its elements on
            // every traversal — and each element operand obeys the same
            // re-derived substitution a computation obeys. The declared
            // place, structural type, and claim-free result custody stay
            // byte-exact inside the moved operation, so a forged declaration
            // or a skipped element rebind rejects here or in
            // `same_relocated_node`'s operation comparison.
            match crate::validation::invariant_scalar_array_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
            ) {
                Some(substitution) => (substitution, None, BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_scalar_case(relocation.expected).is_some()
        {
            // A scalar-case establishment replays its whole admission from
            // the seed: an affine result's dispatch custody must stay inside
            // the component spelled only through positions the custody
            // rewrite re-expresses — the retained-member comparison below
            // normalizes the same frontier the realization writes, so a kept
            // member-internal discard or a missing exit disposal rejects —
            // while each scalar field value obeys the re-derived
            // substitution. The declared place, result case, and claim-free
            // result custody stay byte-exact inside the moved operation, so
            // a forged declaration or a skipped field rebind rejects here or
            // in `same_relocated_node`'s operation comparison.
            match crate::validation::invariant_scalar_case_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
            ) {
                Some(substitution) => (substitution, None, BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else {
            match crate::validation::invariant_scalar_operand_substitution(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
            ) {
                Some(substitution) => (substitution, None, BTreeMap::new()),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        };
        if !same_relocated_node(
            relocation.expected,
            relocation.current,
            &substitution,
            root,
            &argument_roots,
        ) {
            return Err(mismatch(machine, relocation.expected_block));
        }
    }

    // Relocated nodes sharing one preheader form the contiguous run
    // immediately ahead of that block's terminator.
    let mut destinations = BTreeMap::<BlockId, Vec<usize>>::new();
    for relocation in &moved {
        destinations
            .entry(relocation.current_block)
            .or_default()
            .push(relocation.current_index);
    }
    for (destination, mut indices) in destinations {
        indices.sort_unstable();
        let terminator = block(current, destination)
            .and_then(|block| block.nodes.len().checked_sub(1))
            .ok_or_else(|| mismatch(machine, destination))?;
        if indices.len() > terminator
            || indices
                .iter()
                .enumerate()
                .any(|(offset, index)| *index != terminator - indices.len() + offset)
        {
            return Err(mismatch(machine, destination));
        }
    }

    let relocated_operations = moved
        .iter()
        .filter_map(|relocation| match relocation.expected.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => Some(*operation),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    // The proposal's move-together coupling, replayed against the seed: a
    // relocated establishment initializes its declared cell once, so a node
    // that mutably borrows a relocated root but stayed behind would read
    // accumulated post-write contents where the source traversal
    // re-initialized the cell. Every mutable borrower of a relocated root —
    // anywhere in the function, not only inside the roster, since a borrower
    // outside the component can never have moved under its run — must itself
    // appear among the relocated operations; a forged move that hoisted the
    // producer while leaving its mutable borrower resident rejects here.
    let all_relocated_roots: BTreeSet<PlaceId> = relocated_roots
        .values()
        .flat_map(|roots| roots.iter().copied())
        .collect();
    if !all_relocated_roots.is_empty() {
        for expected_block in &expected.blocks {
            for node in &expected_block.nodes {
                if !crate::validation::mutable_borrow_roots(&node.operation)
                    .iter()
                    .any(|root| all_relocated_roots.contains(root))
                {
                    continue;
                }
                let moved_borrower = matches!(
                    node.provenance.first(),
                    Some(PsiProvenance::Operation(operation))
                        if relocated_operations.contains(operation)
                );
                if !moved_borrower {
                    return Err(mismatch(machine, expected_block.id));
                }
            }
        }
    }
    for expected_block in &expected.blocks {
        let current_block = block(current, expected_block.id)
            .ok_or_else(|| mismatch(machine, expected_block.id))?;
        if expected_block.parameters != current_block.parameters
            || expected_block.structural_parameters != current_block.structural_parameters
        {
            return Err(mismatch(machine, expected_block.id));
        }
        // A block inside a component whose run relocated affine scalar-case,
        // empty-record, or structural-call results keeps every retained node
        // but spells
        // the persistent result's custody differently — member-internal
        // edges keep it live while exit edges and member returns dispose
        // it. Normalize each
        // seed node through the same custody rewrite the realization
        // performs so a forged spelling — a kept internal discard, a missing
        // exit disposal, a reordered roster — rejects byte-exact.
        let custody = components
            .iter()
            .find(|component| component.members.contains(&expected_block.id))
            .and_then(|component| {
                relocated_case_results
                    .get(&component.id)
                    .map(|case_results| (*component, case_results))
            });
        let expected_nodes = retained_nodes(expected_block, &relocated_operations);
        let current_nodes = retained_nodes(current_block, &relocated_operations);
        if expected_nodes.len() != current_nodes.len()
            || expected_nodes
                .iter()
                .zip(current_nodes)
                .any(|(expected_node, current_node)| {
                    !same_retained_node(expected, expected_node, current_node, custody)
                })
        {
            return Err(mismatch(machine, expected_block.id));
        }
    }
    for current_block in &current.blocks {
        if block(expected, current_block.id).is_none() {
            return Err(mismatch(machine, current_block.id));
        }
    }
    Ok(())
}

fn retained_nodes<'block>(
    block: &'block OptimizationBlock,
    relocated_operations: &BTreeSet<OperationId>,
) -> Vec<&'block OptimizationNode> {
    block
        .nodes
        .iter()
        .filter(|node| {
            !matches!(
                node.provenance.first(),
                Some(PsiProvenance::Operation(operation))
                    if relocated_operations.contains(operation)
            )
        })
        .collect()
}

fn same_relocated_node(
    expected: &OptimizationNode,
    current: &OptimizationNode,
    substitution: &BTreeMap<ValueId, ValueId>,
    root: Option<PlaceId>,
    argument_roots: &BTreeMap<PlaceId, PlaceId>,
) -> bool {
    let mut operation = expected.operation.clone();
    crate::validation::substitute_invariant_scalar_operands(&mut operation, substitution);
    if let Some(root) = root {
        // Admission proved the expected root is either already `root` or the
        // member structural parameter that resolves to it, so rebinding from
        // the expected source cannot admit a different place. Both observation
        // gates name the root the same way, so the expected source is read
        // off whichever one the node's shape admits through.
        let rebound =
            crate::validation::invariant_observation_source(expected).is_some_and(|source| {
                crate::validation::substitute_invariant_place_root(&mut operation, source, root)
            });
        if !rebound {
            return false;
        }
    }
    if !argument_roots.is_empty()
        && !crate::validation::substitute_invariant_call_roots(&mut operation, argument_roots)
    {
        // Admission re-derived the rewrites from the seed, so a rewrite that
        // finds no matching structural argument means the moved node drifted
        // from the plan.
        return false;
    }
    operation == current.operation
        && expected.provenance == current.provenance
        && expected.fuel == current.fuel
        && position_normalized_definitions(expected) == position_normalized_definitions(current)
        && expected
            .uses
            .iter()
            .map(|value_use| {
                substitution
                    .get(&value_use.value)
                    .copied()
                    .unwrap_or(value_use.value)
            })
            .eq(current.uses.iter().map(|value_use| value_use.value))
        && expected.successors == current.successors
        && expected.ownership == current.ownership
}

/// A retained member node whose component relocated affine scalar-case,
/// empty-record, or structural-call results spells their persistent custody
/// differently from
/// the seed:
/// member-internal edges keep the place live where the source's fresh place
/// died at dispatch, and every exit edge and member return disposes it
/// instead. Normalize the seed node through the same custody rewrite the
/// realization performs ([`crate::validation::rewrite_scalar_case_custody`])
/// before comparing byte-exact — a forged transformed spelling cannot pass
/// unless it is exactly the re-derived frontier.
fn same_retained_node(
    function: &PsiOptimizationFunction,
    expected: &OptimizationNode,
    current: &OptimizationNode,
    custody: Option<(&OptimizerCycleComponent, &BTreeSet<PlaceId>)>,
) -> bool {
    let Some((component, case_results)) = custody else {
        return same_position_normalized_node(expected, current);
    };
    let mut normalized = expected.clone();
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    crate::validation::rewrite_scalar_case_custody(
        &function.structural_places,
        &members,
        case_results,
        &mut normalized,
    );
    same_position_normalized_node(&normalized, current)
}

fn same_position_normalized_node(expected: &OptimizationNode, current: &OptimizationNode) -> bool {
    // Relocation rebases definition/use coordinates and the function-wide
    // effect sequence. Core unit validation immediately reconstructs those
    // derived fields; this freeze retains every source-owned field instead.
    expected.operation == current.operation
        && expected.provenance == current.provenance
        && expected.fuel == current.fuel
        && position_normalized_definitions(expected) == position_normalized_definitions(current)
        && expected
            .uses
            .iter()
            .map(|value_use| value_use.value)
            .eq(current.uses.iter().map(|value_use| value_use.value))
        && expected.successors == current.successors
        && expected.ownership == current.ownership
}

fn position_normalized_definitions(node: &OptimizationNode) -> Vec<(ValueId, ScalarType)> {
    node.definitions
        .iter()
        .map(|definition| (definition.value, definition.scalar_type))
        .collect()
}

fn occurrences(
    function: &PsiOptimizationFunction,
    operation: OperationId,
) -> Vec<(BlockId, usize, &OptimizationNode)> {
    function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter(move |(_, node)| {
                    node.provenance.first() == Some(&PsiProvenance::Operation(operation))
                })
                .map(move |(node, value)| (block.id, node, value))
        })
        .collect()
}

fn block(function: &PsiOptimizationFunction, id: BlockId) -> Option<&OptimizationBlock> {
    function.blocks.iter().find(|block| block.id == id)
}

fn mismatch(machine: MachineId, block: BlockId) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch { machine, block }
}
