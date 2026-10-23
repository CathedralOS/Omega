//! Independent case-membership specialization replay mechanics.
//!
//! Validation never trusts the candidate's membership rows: it re-derives the
//! claimed place's proof evidence — an `EstablishCase` producer on an
//! operation-result place at an empty path, the same producer reached
//! through `Field` descents across owned, complete structural children
//! under an `EstablishRecord` or through `Case` then `Field` descents into
//! the payload child an `EstablishCase` stores whole, a chain reached
//! through a uniform block-parameter binding or a `StructuralLeafCopy`
//! whose projection — case-qualified steps directly, ordinary field
//! identities resolved to roster handles — prefixes the path still
//! to resolve, an `EstablishZeroedStructural` producer
//! whose all-zero storage selects the first declared case at every
//! position of a zero-constructible shape, or a declared closed
//! `Sum`/`Mixed` roster of exactly one case at the position each
//! observation resolves to — replays the admissible observation set for
//! that place, requires the claimed rows to equal the replayed rows
//! exactly, rebuilds the output itself, and reconstructs the exact node
//! custody the folded observations carry.

use crate::candidates::provenance_realized_at;
use crate::candidates::state_specialization::cyclic_machines;
use crate::candidates::structural_bindings::bound_place;
use crate::candidates::{accept_region_rewrite, canonicalize_provenance};
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    FoldedCaseMembershipRow, OptimizationNode, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
    declared_structural_type, resolved_sole_case, sole_case_at_path,
};
use semantic_vocabulary::{
    BlockId, MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralTypeId,
};

use terminal_psi::StructuralPlaceDeclaration;

/// The evidence every membership observing `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), the function every proof replays inside, and the root
/// proof — the establishment-or-roster basis empty-path observations may
/// draw on. `None` when `place` is not rostered in `function`.
struct MembershipEvidence<'a> {
    declaration: &'a StructuralPlaceDeclaration,
    root_type: Option<StructuralTypeId>,
    root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
    function: &'a PsiOptimizationFunction,
}

/// The membership evidence for one rostered place, or `None` when the place
/// does not exist in `function`.
fn membership_evidence<'a>(
    unit: &PsiOptimizationUnit,
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<MembershipEvidence<'a>> {
    let declaration = function.structural_place(place)?;
    Some(MembershipEvidence {
        declaration,
        root_type: declared_structural_type(function, declaration),
        root_basis: optimization_unit::proof_basis(unit, function, declaration, &bound_place),
        function,
    })
}

/// The admissibility of one node under `evidence`: a `StructuralCaseMembership`
/// observing the evidence's place into a Boolean result, whose verdict the
/// unit proves — the place's root basis at an empty path, the establishment
/// a non-empty path resolves to through stored-whole children, `EstablishCase`
/// payload children, uniform block-parameter bindings, and leaf copies
/// across canonical or resolved ordinary projections, or the sole-case
/// roster at the resolved nested position. `None`
/// for every other node.
fn admit_membership_node(
    unit: &PsiOptimizationUnit,
    evidence: &MembershipEvidence<'_>,
    machine: MachineId,
    block: BlockId,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FoldedCaseMembershipRow> {
    let O::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = &node.operation
    else {
        return None;
    };
    if *source != evidence.declaration.id || result.scalar_type != ScalarType::Boolean {
        return None;
    }
    // Every path asks the establishment chain first — at an empty path the
    // place's own producer, and at any depth a `Field` descent into a
    // stored-whole child or a `Case` then `Field` descent into an
    // `EstablishCase` payload, each possibly reached through the place's
    // uniform block-parameter binding or a `StructuralLeafCopy` whose
    // projection the walk prefixes. Failing an
    // establishment, the sole-case roster at the resolved end type proves
    // the verdict without custody evidence.
    let (proven_case, producer) = optimization_unit::established_case_at(
        unit,
        evidence.function,
        evidence.declaration,
        path,
        &bound_place,
    )
    .map(|(operation, case)| (case, Some(operation)))
    .or_else(|| resolved_sole_case(unit, evidence.root_type?, path).map(|case| (case, None)))?;
    Some(FoldedCaseMembershipRow {
        site: optimization_unit::NodeLocation {
            machine,
            block,
            node: u32::try_from(node_index).ok()?,
        },
        psi_operation: *psi_operation,
        result: result.value,
        source: evidence.declaration.id,
        producer,
        observed_case: *case,
        proven_case,
        outcome: proven_case == *case,
    })
}

/// The independently derived fold roster for `place`: every
/// `StructuralCaseMembership` observing it whose verdict the per-site basis
/// proves, sorted by site.
fn plan_memberships(
    unit: &PsiOptimizationUnit,
    evidence: &MembershipEvidence<'_>,
    function: &PsiOptimizationFunction,
) -> Vec<FoldedCaseMembershipRow> {
    let mut memberships = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) =
                admit_membership_node(unit, evidence, function.machine, block.id, node_index, node)
            else {
                continue;
            };
            memberships.push(row);
        }
    }
    memberships.sort_by_key(|row| row.site);
    memberships
}

/// The folded node a row admits at its site: a `BooleanConstant` with the
/// membership's own custody identity and result value, checked against the
/// claimed basis — an establishment producer must be the `EstablishCase`
/// the row's path resolves to through stored-whole children, `EstablishCase`
/// payload children, uniform block-parameter bindings, and leaf copies
/// across canonical or resolved ordinary projections, while a roster row
/// must resolve to exactly the claimed case.
fn folded_node(
    row: &FoldedCaseMembershipRow,
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, OptimizationUnitValidationError> {
    let index = usize::try_from(row.site.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = function
        .block(row.site.block)
        .and_then(|block| block.nodes.get(index))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if row.site.machine != function.machine
        || *psi_operation != row.psi_operation
        || result.value != row.result
        || result.scalar_type != ScalarType::Boolean
        || *source != row.source
        || *case != row.observed_case
        || row.outcome != (row.proven_case == row.observed_case)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    match row.producer {
        // The claimed producer must be the `EstablishCase` the
        // establishment chain under `row.source` resolves `path` to — the
        // observed place's own producer at an empty path, the operation a
        // uniform block-parameter binding or leaf copy forwards
        // to, a stored-whole record child's producer at a nested `Field`
        // position, or an `EstablishCase` payload child's producer at a
        // `Case` then `Field` position.
        Some(producer) => {
            let resolved = function
                .structural_place(row.source)
                .and_then(|declaration| {
                    optimization_unit::established_case_at(
                        unit,
                        function,
                        declaration,
                        path,
                        &bound_place,
                    )
                });
            if resolved != Some((producer, row.proven_case)) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
        }
        // With no producer claim, the membership's path position must itself
        // resolve to a closed roster of exactly the claimed case: the place's
        // root type at an empty path, or the nested type its path descends to.
        None => {
            let declaration = function.structural_place(row.source);
            if declaration
                .and_then(|declaration| sole_case_at_path(unit, function, declaration, path))
                != Some(row.proven_case)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
        }
    }
    let operation = O::BooleanConstant {
        psi_operation: *psi_operation,
        result: result.value,
        value: row.outcome,
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

/// The exact node custody the folded observations carry: each folded site
/// retains the membership's own provenance and fuel settlement, realized at
/// the same node.
fn accepted_provenance(
    function: &PsiOptimizationFunction,
    rows: &[FoldedCaseMembershipRow],
) -> Result<Vec<ProvenanceRewrite>, OptimizationUnitValidationError> {
    let mut provenance = Vec::new();
    for row in rows {
        let index = usize::try_from(row.site.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let node = function
            .block(row.site.block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        let site = PsiRealizationSite::Node(row.site);
        provenance.push(provenance_realized_at(
            site,
            site,
            &node.provenance,
            &node.fuel,
        ));
    }
    canonicalize_provenance(&mut provenance);
    Ok(provenance)
}

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::SpecializeCaseMembership(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != patch.memberships.first().map(|row| row.site) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.memberships.is_empty()
        || candidate.predicted_cost_delta()
            != -i64::try_from(patch.memberships.len())
                .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    // A machine holding a cyclic component is frozen byte-exact: none of its
    // nodes may fold, so any candidate naming it is rejected before replay.
    if cyclic_machines(input).contains(&patch.machine) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .function(patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let Some(evidence) = membership_evidence(input, function, patch.place) else {
        return Err(OptimizationUnitValidationError::CandidateLocationMissing);
    };
    if patch.producer != evidence.root_basis.and_then(|(_, producer)| producer) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let replayed = plan_memberships(input, &evidence, function);
    if replayed.is_empty() || patch.memberships != replayed {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let mut expected_blocks = patch
        .memberships
        .iter()
        .map(|row| row.site.block)
        .collect::<Vec<_>>();
    expected_blocks.sort_unstable();
    expected_blocks.dedup();
    let expected_provenance = accepted_provenance(function, &patch.memberships)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let folded = patch
        .memberships
        .iter()
        .map(|row| folded_node(row, input, function).map(|node| (row.site, node)))
        .collect::<Result<Vec<_>, _>>()?;
    accept_region_rewrite(
        input,
        candidate,
        patch.machine,
        &expected_blocks,
        b"omega.validator.case-membership-specialization.v1",
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
            Ok(())
        },
    )
}
