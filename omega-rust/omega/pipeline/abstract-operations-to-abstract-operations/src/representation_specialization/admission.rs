//! Optimizer module role: admission leaf. Proven-case predicates behind the plan.
//!
//! The plan is built from two predicates only: the place's
//! declaration/type/proof evidence and the per-node admissibility that
//! resolves one membership observation's proven case. The independent
//! validator in `optimization-unit-semantics` re-derives the same predicates
//! from the candidate's rows rather than trusting this enumeration — a
//! matcher cannot attest to itself.

use super::{
    FoldedCaseMembershipRow, NodeLocation, O, OperationId, PlaceId, PsiOptimizationFunction,
    PsiOptimizationUnit, ScalarType, StructuralPlaceKind,
};
use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
};

/// The evidence every membership observing `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), and the root proof — the establishment-or-roster basis
/// empty-path observations may draw on. `None` when `place` is not rostered
/// in `function`.
pub(super) struct MembershipEvidence<'a> {
    pub(super) declaration: &'a StructuralPlaceDeclaration,
    pub(super) root_type: Option<StructuralTypeId>,
    pub(super) root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
}

/// The membership evidence for one rostered place, or `None` when the place
/// does not exist in `function`.
pub(super) fn membership_evidence<'a>(
    unit: &PsiOptimizationUnit,
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<MembershipEvidence<'a>> {
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    Some(MembershipEvidence {
        declaration,
        root_type: declared_structural_type(function, declaration),
        root_basis: proof_basis(unit, function, declaration),
    })
}

/// The admissibility of one node under `evidence`: a `StructuralCaseMembership`
/// observing the evidence's place into a Boolean result, whose verdict the
/// unit proves — the place's root basis at an empty path, or the sole-case
/// roster at the resolved nested position. `None` for every other node.
pub(super) fn admit_membership_node(
    unit: &PsiOptimizationUnit,
    evidence: &MembershipEvidence<'_>,
    machine: semantic_vocabulary::MachineId,
    block: semantic_vocabulary::BlockId,
    node_index: usize,
    node: &optimization_unit::OptimizationNode,
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
    // An empty path observes the place's root case and uses its
    // establishment-or-roster basis; a non-empty path observes a nested
    // position only the resolved roster can prove.
    let (proven_case, producer) = if path.is_empty() {
        evidence.root_basis?
    } else {
        resolved_sole_case(unit, evidence.root_type?, path).map(|case| (case, None))?
    };
    Some(FoldedCaseMembershipRow {
        site: NodeLocation {
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

/// The case `place`'s root is proven to hold, plus its establishment
/// producer when the proof is one `EstablishScalarCase` operation result.
fn proof_basis(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<(StructuralCaseId, Option<OperationId>)> {
    if let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind {
        let established = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find_map(|node| match &node.operation {
                O::EstablishScalarCase {
                    psi_operation,
                    result,
                    result_case,
                    ..
                } if *psi_operation == producer && result.place == declaration.id => {
                    Some(*result_case)
                }
                _ => None,
            });
        if let Some(case) = established {
            return Some((case, Some(producer)));
        }
    }
    sole_case(unit, function, declaration).map(|case| (case, None))
}

/// The declared structural type of one rostered place, or `None` when the
/// place kind carries no resolvable type in this function.
pub(crate) fn declared_structural_type(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralTypeId> {
    match declaration.kind {
        StructuralPlaceKind::OperationResult {
            structural_type, ..
        }
        | StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }
        | StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        } => Some(structural_type),
        StructuralPlaceKind::ProviderAttachment { attachment, .. } => Some(attachment),
        StructuralPlaceKind::Parameter { .. } => function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::BlockParameter { .. } => function
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::Result => function
            .result
            .structural()
            .and_then(|result| (result.place == declaration.id).then_some(result.structural_type)),
    }
}

/// The one case of the closed roster the `path` resolves to under the
/// place's declared structural type, or `None` when the path fails to
/// descend — a `Field` name absent from a `Record`/`Mixed` common-field
/// roster, a `FixedIndex` on a non-array, a `Referent` crossing on a
/// non-reference — or when the resolved end type is not a `Sum`/`Mixed`
/// roster of exactly one case.
fn sole_case_at_path(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    resolved_sole_case(unit, declared_structural_type(function, declaration)?, path)
}

/// The one case of a declared closed roster, or `None` when the place's
/// declared type is not a sum shape or names more than one case.
fn sole_case(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralCaseId> {
    sole_case_at_path(unit, function, declaration, &[])
}

/// The sole case of the closed roster at `path`'s end under `root`, or
/// `None` when the path fails to descend or the end type is not a sole-case
/// `Sum`/`Mixed` roster.
fn resolved_sole_case(
    unit: &PsiOptimizationUnit,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    let mut current = root;
    for segment in path {
        current = descended_type(unit, current, segment)?;
    }
    roster_sole_case(unit, current)
}

/// The structural type one path segment descends to under `current`, or
/// `None` when the segment does not apply to `current`'s shape or names a
/// field whose declared type is not structural.
fn descended_type(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    segment: &StructuralPathSegment,
) -> Option<StructuralTypeId> {
    let shape = &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == current)?
        .shape;
    match segment {
        StructuralPathSegment::FixedByteRange { .. } => None,
        StructuralPathSegment::Field(identity) => {
            let fields = match shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            };
            match fields
                .iter()
                .find(|field| field.identity == *identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(next),
                _ => None,
            }
        }
        StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
            match shape {
                StructuralTypeShape::FixedArray { element, .. } => Some(*element),
                _ => None,
            }
        }
        StructuralPathSegment::Referent => match shape {
            StructuralTypeShape::Reference { referent, .. } => Some(*referent),
            _ => None,
        },
    }
}

/// The one case of `type_id`'s closed roster, or `None` when its shape is
/// not a `Sum`/`Mixed` or names more than one case.
fn roster_sole_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = match &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)?
        .shape
    {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return None,
    };
    let [sole] = cases.as_slice() else {
        return None;
    };
    Some(sole.id)
}
