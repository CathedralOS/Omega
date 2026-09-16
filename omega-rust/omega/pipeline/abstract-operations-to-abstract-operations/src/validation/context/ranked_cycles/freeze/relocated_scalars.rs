//! Optimizer module role: validation leaf. Exact ranked-body relocation normalization.
//!
//! A transformed cyclic body stays frozen against the reconstructed seed
//! except for independently admitted scalar motion: an admissible
//! source-owned scalar constant leaf, an admissible place observation whose
//! component performs no place mutation or custody movement and whose storage
//! root is visible at the preheader insertion point — directly, or as the
//! representative an invariant member structural parameter resolves to — an
//! admissible byte observation (a `ByteSequenceRead`, or a
//! `ByteSequenceSubslice` whose structural view result and bounds obligation
//! relocate byte-exact inside the moved operation), or
//! an admissible scalar
//! computation (an obligated variant keeps its verifier-discharged
//! obligation byte-exact inside the moved operation) whose uses are all
//! defined outside the component, name
//! provably invariant
//! member parameters, or are defined by another node relocated
//! out of the same component's run, may relocate from one of its component's
//! member blocks into the tail of that component's unique-entry preheader.
//! Every non-leaf relocation also replays the proposal's non-speculative
//! custody against the authenticated topology: the entry edge must be the
//! preheader terminator's only successor, and the source member block must
//! be one every traversal that leaves the component executed — a forged move
//! out of a bypassed member or past a conditional entry charges the node's
//! fuel on traversals the source never paid it on, so this fence rejects it
//! rather than trusting the transformed unit.
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
    PsiOptimizationFunction, PsiProvenance, ScalarType, ValueId,
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
    expected: &PsiOptimizationFunction,
    current: &PsiOptimizationFunction,
    components: &[&OptimizerCycleComponent],
) -> Result<(), OptimizationUnitValidationError> {
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

    // Each relocated node must land in its own component's unique-entry
    // preheader ahead of the terminator that owns the entry edge, and it must
    // retain every source-owned field.
    let mut guaranteed_members: BTreeMap<CycleComponentId, BTreeSet<BlockId>> = BTreeMap::new();
    for relocation in &moved {
        let component = relocation.home;
        let [entry] = component.entries.as_slice() else {
            return Err(mismatch(machine, relocation.expected_block));
        };
        if component.members.contains(&entry.source) || relocation.current_block != entry.source {
            return Err(mismatch(machine, relocation.expected_block));
        }
        let preheader =
            block(current, entry.source).ok_or_else(|| mismatch(machine, entry.source))?;
        let Some(terminator) = preheader.nodes.last() else {
            return Err(mismatch(machine, entry.source));
        };
        if !terminator
            .successors
            .iter()
            .any(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
        {
            return Err(mismatch(machine, entry.source));
        }
        let leaf = crate::validation::admissible_scalar_leaf_relocation(relocation.expected);
        if !leaf {
            // The proposal's non-speculative custody is re-derived here
            // rather than trusted from the transformed unit: a non-leaf
            // node may relocate only when reaching the preheader
            // guarantees entering the component — the entry edge is its
            // terminator's only successor — and only out of a member block
            // every traversal that leaves the component executed. A forged
            // move out of a bypassed member or past a conditional entry
            // relocates work the source traversal could skip, so the moved
            // node's fuel charge lands on traversals that never paid it.
            // Scalar-constant leaves stay exempt: re-expressing a constant
            // performs no work the traversal could have skipped.
            let guaranteed_entry = matches!(
                terminator.successors.as_slice(),
                [entry_edge]
                    if entry_edge.psi_edge == entry.edge && entry_edge.target == entry.target
            );
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
        let (substitution, root) = if leaf {
            (BTreeMap::new(), None)
        } else if crate::validation::admissible_invariant_place_read(relocation.expected).is_some()
        {
            // The whole-component place-custody gate and the root's
            // preheader visibility — direct or through the member
            // parameter's agreed representative — are re-derived here from
            // the seed rather than trusted from the transformed unit.
            match crate::validation::invariant_place_observation_admission(
                expected,
                component,
                relocation.expected,
            ) {
                Some(root) => (BTreeMap::new(), Some(root)),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        } else if crate::validation::admissible_invariant_byte_read(relocation.expected).is_some() {
            // Both the root half and the scalar-operand half re-derive
            // from the seed: the run-internal `length` producer must
            // already appear among this component's relocated results,
            // and its own observation root must resolve to the read's
            // rebound root.
            match crate::validation::invariant_byte_read_admission(
                expected,
                component,
                relocation.expected,
                relocated_results
                    .get(&component.id)
                    .unwrap_or(&no_relocated_results),
            ) {
                Some((root, substitution)) => (substitution, Some(root)),
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
            ) {
                Some((root, substitution)) => (substitution, Some(root)),
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
                Some(substitution) => (substitution, None),
                None => return Err(mismatch(machine, relocation.expected_block)),
            }
        };
        if !same_relocated_node(relocation.expected, relocation.current, &substitution, root) {
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
    for expected_block in &expected.blocks {
        let current_block = block(current, expected_block.id)
            .ok_or_else(|| mismatch(machine, expected_block.id))?;
        if expected_block.parameters != current_block.parameters
            || expected_block.structural_parameters != current_block.structural_parameters
        {
            return Err(mismatch(machine, expected_block.id));
        }
        let expected_nodes = retained_nodes(expected_block, &relocated_operations);
        let current_nodes = retained_nodes(current_block, &relocated_operations);
        if expected_nodes.len() != current_nodes.len()
            || expected_nodes
                .iter()
                .zip(current_nodes)
                .any(|(expected, current)| !same_position_normalized_node(expected, current))
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
