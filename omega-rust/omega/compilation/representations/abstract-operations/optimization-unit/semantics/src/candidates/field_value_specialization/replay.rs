//! Independent field-value specialization replay mechanics.
//!
//! Validation never trusts the candidate's field rows: it re-derives the
//! claimed place's proof evidence — an `EstablishRecord` producer on an
//! operation-result place at an empty path, an `EstablishCase`
//! producer whose `result_case` matches at a lone `Case` path, the same two
//! reached through `Field` descents across owned, complete structural
//! children (record fields, or `Case` then `Field` into the payload child an
//! `EstablishCase` stores whole for exactly its own case), the same chain
//! entered from a `StructuralLeafCopy` whose projection — case-qualified
//! steps directly, ordinary field identities resolved to roster handles —
//! prefixes the path beneath its source, an
//! `EstablishZeroedStructural` producer whose all-zero storage
//! fixes the scalar leaf of a zero-constructible shape at zero, or a
//! declared `BoundedInteger` singleton bound at the position
//! each observation resolves to — replays the admissible observation set
//! for that place,
//! requires the claimed rows to equal the replayed rows exactly, rebuilds
//! the output itself, and reconstructs the exact node custody the resolved
//! observations carry. A `Constant` row folds its read in place; a `Forward`
//! row rebinds every covered scalar-operand use of the read's result to the
//! proven nonconstant initializer and retires the observation node, so the
//! replay also recomputes the substitution set, the exact use-site roster,
//! initializer dominance, and the shifted-custody provenance ledger.

use crate::candidates::provenance_realized_at;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::state_specialization::cyclic_machines;
use crate::candidates::structural_bindings::bound_place;
use crate::candidates::{
    accept_region_rewrite, candidate_function, canonicalize_provenance, rebuild_node_metadata,
    receiver_custody_distinct, retirable_scalar_node, retire_node_custody, set_node_effect_link,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    FieldEvidence, FieldValueResolution, FieldValueRow, FoldedFieldValue, ForwardedFieldValue,
    FunctionAnalysis, OptimizationNode, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
    ScalarSubstitution, ValueDefinitionSite, field_evidence, function_analysis,
};
use semantic_vocabulary::{BlockId, ScalarType, ValueId};
use std::collections::{BTreeMap, BTreeSet};

/// The admissibility of one node under `evidence`: a Boolean or integer
/// `StructuralScalarFieldRead` observing the evidence's place whose stored
/// value the unit proves. `None` for every other node.
fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &optimization_unit::OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FieldValueRow> {
    optimization_unit::admit_field_node(
        unit,
        evidence,
        function,
        analysis,
        block,
        node_index,
        node,
        &bound_place,
        &forwarded_resolution,
    )
}

/// The `Forward` resolution for one nonconstant-initialized read, or `None`
/// when the substitution cannot retire the observation exactly: the
/// initializer must resolve to a same-function definition of the read's own
/// scalar type and dominate every use site, every scalar-operand use of the
/// read's result must sit in an operation the substitution lane rewrites,
/// and the observation node itself must be cleanly removable — exactly its
/// result definition, no successors, no ownership events, a following node
/// to absorb its custody, and no shared provenance with that receiver. Field
/// observations emit no optimization facts, so no fact custody is owed.
fn forwarded_resolution(
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &optimization_unit::OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
    initializer: ValueId,
) -> Option<ForwardedFieldValue> {
    if initializer == result {
        return None;
    }
    let definition = analysis.definitions.get(&initializer)?;
    if definition.scalar_type != scalar_type {
        return None;
    }
    if !retirable_scalar_node(block, node_index, node, result, scalar_type) {
        return None;
    }
    if !receiver_custody_distinct(block, node_index, node) {
        return None;
    }
    // Every site that references the read's result: the tracked scalar-operand
    // uses, which include each runtime-selected path element's selector. A
    // use inside a byte-sequence operation, or as a runtime selector whose
    // accepted certificate names that exact value, is outside the lane, so
    // the row is inadmissible rather than partially substituted.
    let mut uses = BTreeSet::new();
    for use_block in &function.blocks {
        for (use_index, use_node) in use_block.nodes.iter().enumerate() {
            let referenced = use_node
                .uses
                .iter()
                .any(|use_site| use_site.value == result);
            if !referenced {
                continue;
            }
            if use_node.operation.operates_on_byte_sequence_storage()
                || use_node
                    .operation
                    .runtime_indices()
                    .iter()
                    .any(|(index, _)| *index == result)
            {
                return None;
            }
            let site = optimization_unit::NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(use_index).ok()?,
            };
            let dominates = match definition.site {
                ValueDefinitionSite::FunctionParameter(_) => true,
                ValueDefinitionSite::BlockParameter {
                    block: defining, ..
                } => analysis
                    .dominators
                    .get(&use_block.id)
                    .is_some_and(|set| set.contains(&defining)),
                ValueDefinitionSite::Node {
                    block: defining,
                    node: defined_at,
                } => {
                    if defining == use_block.id {
                        defined_at < site.node
                    } else {
                        analysis
                            .dominators
                            .get(&use_block.id)
                            .is_some_and(|set| set.contains(&defining))
                    }
                }
            };
            if !dominates {
                return None;
            }
            uses.insert(site);
        }
    }
    Some(ForwardedFieldValue {
        initializer,
        scalar_type,
        uses: uses.into_iter().collect(),
    })
}

/// The independently derived resolution roster for `place`: every
/// Boolean or integer `StructuralScalarFieldRead` observing it whose
/// stored value the per-site basis proves, sorted by site.
fn plan_reads(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
) -> Vec<FieldValueRow> {
    let analysis = function_analysis(function);
    let mut reads = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) =
                admit_field_node(unit, evidence, function, &analysis, block, node_index, node)
            else {
                continue;
            };
            reads.push(row);
        }
    }
    reads.sort_by_key(|row| row.site);
    reads
}

/// The folded node a `Constant` row admits at its site: a
/// `BooleanConstant`/`IntegerConstant` with the read's own custody identity
/// and result value. The roster equality check has already proven the
/// claimed basis — this construction only re-verifies the site's shape.
/// `Forward` rows retire their node instead of folding it, so this
/// construction never applies to them.
fn folded_node(
    row: &FieldValueRow,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, OptimizationUnitValidationError> {
    let index = usize::try_from(row.site.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = function
        .block(row.site.block)
        .and_then(|block| block.nodes.get(index))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::StructuralScalarFieldRead {
        psi_operation,
        result: read_result,
        source,
        path,
        field,
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let result = read_result.value;
    if row.site.machine != function.machine
        || *psi_operation != row.psi_operation
        || result != row.result
        || *source != row.source
        || path.as_slice() != row.path.as_slice()
        || *field != row.field
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let operation = match &row.resolution {
        FieldValueResolution::Constant(FoldedFieldValue::Boolean(value)) => O::BooleanConstant {
            psi_operation: *psi_operation,
            result,
            value: *value,
        },
        FieldValueResolution::Constant(FoldedFieldValue::Integer(value)) => O::IntegerConstant {
            psi_operation: *psi_operation,
            result,
            // An integer constant replaces only a non-Boolean read.
            scalar_type: match read_result.scalar_type {
                ScalarType::Boolean => {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                scalar_type => scalar_type,
            },
            value: *value,
        },
        FieldValueResolution::Forward(_) => {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    };
    Ok(OptimizationNode {
        operation,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
        definitions: node.definitions.clone(),
        uses: node.uses.clone(),
        successors: node.successors.clone(),
        ownership: node.ownership.clone(),
    })
}

/// The substitution set a roster of rows carries: exactly one
/// `result` → `initializer` pair per `Forward` row, in canonical order.
fn expected_substitutions(rows: &[FieldValueRow]) -> Vec<ScalarSubstitution> {
    let mut substitutions = rows
        .iter()
        .filter_map(|row| match &row.resolution {
            FieldValueResolution::Forward(forwarded) => Some(ScalarSubstitution {
                from: row.result,
                to: forwarded.initializer,
                scalar_type: forwarded.scalar_type,
            }),
            FieldValueResolution::Constant(_) => None,
        })
        .collect::<Vec<_>>();
    substitutions.sort();
    substitutions
}

/// The exact block region and node custody a row roster carries. A `Constant`
/// row folds its observation in place: the read's own provenance and fuel
/// stay realized at the same node. A `Forward` row retires its observation
/// node: the read's custody lands at the node that inherits the vacated
/// index — the next surviving input node — and every later node in the block
/// shifts down one coordinate per earlier retirement. Because effects are a
/// function-wide sequence, every block at or after the earliest retired-node
/// block is inside the region, and every provenance-bearing node in a
/// region block whose custody moved — or whose own operation changed — gets
/// one ledger row.
fn expected_accounting(
    function: &PsiOptimizationFunction,
    rows: &[FieldValueRow],
) -> Result<(Vec<BlockId>, Vec<ProvenanceRewrite>), OptimizationUnitValidationError> {
    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
    let mut fold_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    let mut use_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    for row in rows {
        match &row.resolution {
            FieldValueResolution::Constant(_) => {
                fold_sites.insert((row.site.block, row.site.node));
            }
            FieldValueResolution::Forward(forwarded) => {
                removals
                    .entry(row.site.block)
                    .or_default()
                    .insert(row.site.node);
                for site in &forwarded.uses {
                    use_sites.insert((site.block, site.node));
                }
            }
        }
    }
    let earliest_removal = function
        .blocks
        .iter()
        .position(|block| removals.contains_key(&block.id));
    let mut affected = BTreeSet::new();
    let mut provenance = Vec::new();
    for (block_position, block) in function.blocks.iter().enumerate() {
        let removal = removals.get(&block.id);
        let first_removed = removal.and_then(|set| set.iter().next().copied());
        let in_suffix = earliest_removal.is_some_and(|position| block_position > position);
        let mut block_affected = removal.is_some() || in_suffix;
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let coordinate = (block.id, node_index);
            if fold_sites.contains(&coordinate) || use_sites.contains(&coordinate) {
                block_affected = true;
            }
            let retired = removal.is_some_and(|set| set.contains(&node_index));
            let shifted = first_removed.is_some_and(|first| node_index > first);
            if !retired
                && !shifted
                && !in_suffix
                && !fold_sites.contains(&coordinate)
                && !use_sites.contains(&coordinate)
            {
                continue;
            }
            if !retired && node.provenance.is_empty() {
                continue;
            }
            let input_site = PsiRealizationSite::Node(optimization_unit::NodeLocation {
                machine: function.machine,
                block: block.id,
                node: node_index,
            });
            let output_index = node_index
                .checked_sub(
                    u32::try_from(
                        removal
                            .map(|set| set.iter().filter(|removed| **removed < node_index).count())
                            .unwrap_or(0),
                    )
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
                )
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let output_site = PsiRealizationSite::Node(optimization_unit::NodeLocation {
                machine: function.machine,
                block: block.id,
                node: output_index,
            });
            provenance.push(provenance_realized_at(
                input_site,
                output_site,
                &node.provenance,
                &node.fuel,
            ));
        }
        if block_affected {
            affected.insert(block.id);
        }
    }
    canonicalize_provenance(&mut provenance);
    Ok((affected.into_iter().collect(), provenance))
}

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::SpecializeFieldValue(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != patch.reads.first().map(|row| row.site) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.reads.is_empty()
        || candidate.predicted_cost_delta()
            != -i64::try_from(patch.reads.len())
                .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    // A machine holding a cyclic component is frozen byte-exact: none of its
    // nodes may fold, so any candidate naming it is rejected before replay.
    if cyclic_machines(input).contains(&patch.machine) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = candidate_function(input, patch.machine)?;
    let Some(evidence) = field_evidence(function, patch.place) else {
        return Err(OptimizationUnitValidationError::CandidateLocationMissing);
    };
    if patch.producer != evidence.root_producer.operation() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let replayed = plan_reads(input, &evidence, function);
    if replayed.is_empty() || patch.reads != replayed {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if candidate.substitutions() != expected_substitutions(&patch.reads) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let (expected_blocks, expected_provenance) = expected_accounting(function, &patch.reads)?;

    let folded = patch
        .reads
        .iter()
        .filter(|row| matches!(row.resolution, FieldValueResolution::Constant(_)))
        .map(|row| folded_node(row, function).map(|node| (row.site, node)))
        .collect::<Result<Vec<_>, _>>()?;
    accept_region_rewrite(
        input,
        candidate,
        patch.machine,
        &expected_blocks,
        b"omega.validator.field-value-specialization.v1",
        expected_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.machine)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            for (location, node) in folded {
                let index = usize::try_from(location.node)
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                let Some(slot) = output_function
                    .block_mut(location.block)
                    .and_then(|block| block.nodes.get_mut(index))
                else {
                    return Err(OptimizationUnitValidationError::CandidateLocationMissing);
                };
                *slot = node;
            }
            // Rebind every covered use of each forwarded result to its proven
            // initializer before retiring the observation nodes.
            for row in &patch.reads {
                let FieldValueResolution::Forward(forwarded) = &row.resolution else {
                    continue;
                };
                for node in output_function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.nodes)
                {
                    rewrite_scalar_value_uses(
                        &mut node.operation,
                        row.result,
                        forwarded.initializer,
                    );
                }
            }
            // Retire each forwarded observation in descending node order
            // inside its block so earlier removals never shift a pending
            // coordinate, and fuse the retired custody into the node
            // inheriting the vacated index.
            let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
            for row in &patch.reads {
                if matches!(row.resolution, FieldValueResolution::Forward(_)) {
                    removals
                        .entry(row.site.block)
                        .or_default()
                        .insert(row.site.node);
                }
            }
            for (block_id, sites) in removals {
                let block = output_function
                    .block_mut(block_id)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                for site in sites.iter().rev() {
                    let index = usize::try_from(*site)
                        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                    retire_node_custody(block, index)?;
                }
            }
            let mut effect = 0u64;
            for block in &mut output_function.blocks {
                for (node_index, node) in block.nodes.iter_mut().enumerate() {
                    rebuild_node_metadata(node, block.id, node_index)?;
                    effect = set_node_effect_link(node, effect)?;
                }
            }
            Ok(())
        },
    )
}
