//! Independent field-value specialization replay mechanics.
//!
//! Validation never trusts the candidate's field rows: it re-derives the
//! claimed place's proof evidence with `optimization_unit::field_value_reads`
//! — the same roster the producer publishes, derived from the function rather
//! than read from the candidate — requires the claimed rows to equal the
//! replayed rows exactly, rebuilds the output itself, and reconstructs the
//! exact node custody the resolved observations carry. A `Constant` row folds
//! its read in place; a `Forward` row rebinds every covered scalar-operand use
//! of the read's result to the proven nonconstant initializer and retires the
//! observation node, so the replay also recomputes the substitution set, the
//! exact use-site roster, initializer dominance, and the shifted-custody
//! provenance ledger.

use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::state_specialization::cyclic_machines;
use crate::candidates::{
    accept_region_rewrite, candidate_block, candidate_function, candidate_node, land_folded_nodes,
    retire_node_custody, sweep_output_nodes,
};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    FieldValueResolution, FieldValueRow, FoldedFieldValue, OptimizationNode,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
    ScalarSubstitution, field_evidence, field_value_reads,
};
use semantic_vocabulary::{BlockId, ScalarType};
use std::collections::{BTreeMap, BTreeSet};

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
    let node = candidate_node(candidate_block(function, row.site.block)?, row.site.node)?;
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

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    candidate_patch!(patch, SpecializeFieldValue, candidate);
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
    let replayed = field_value_reads(input, &evidence, function);
    if replayed.is_empty() || patch.reads != replayed {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if candidate.substitutions() != expected_substitutions(&patch.reads) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let (expected_blocks, expected_provenance) =
        optimization_unit::field_value_accounting(function, &patch.reads)
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;

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
            land_folded_nodes(output, patch.machine, folded)?;
            let output_function = output
                .function_mut(patch.machine)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
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
            sweep_output_nodes(output_function, |_| Ok(()))?;
            Ok(())
        },
    )
}
