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
    PsiOptimizationUnit, ScalarType,
};
use optimization_unit::{argument_rewrites_place, declared_structural_type, resolved_sole_case};
use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use terminal_psi::{StructuralArgument, StructuralPlaceDeclaration};

/// The evidence every membership observing `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), the function every proof replays inside, and the root
/// proof — the establishment-or-roster basis empty-path observations may
/// draw on. `None` when `place` is not rostered in `function`.
pub(super) struct MembershipEvidence<'a> {
    pub(super) declaration: &'a StructuralPlaceDeclaration,
    pub(super) root_type: Option<StructuralTypeId>,
    pub(super) root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
    pub(super) function: &'a PsiOptimizationFunction,
}

/// The membership evidence for one rostered place, or `None` when the place
/// does not exist in `function`.
pub(super) fn membership_evidence<'a>(
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
/// unit proves — the establishment chain under the place at the observed
/// path, or the sole-case roster at the resolved position. `None` for every
/// other node.
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

/// The one place `place` is bound to by every incoming edge when `place` is
/// a structural block parameter, or `None` when it is not a block
/// parameter, when any incoming edge fails to bind it or binds a different
/// place, when any bound argument is pathed or carries write authority, or
/// when the parameter's contents may still be rewritten or mutably re-lent
/// inside `function`. A uniform whole `Owned`/`SharedBorrow` argument means
/// the parameter's contents are exactly the bound place's for the block's
/// lifetime — an owned binding transfers the storage itself, and a shared
/// loan freezes the bound place's contents for the borrow — so the bound
/// place's establishment is the parameter's own.
pub(crate) fn bound_place(function: &PsiOptimizationFunction, place: PlaceId) -> Option<PlaceId> {
    optimization_unit::bound_place(function, place, &operation_rewrites_place)
}

/// Whether `operation` writes or vacates `place`, or re-lends it with
/// authority another party can write through.
fn operation_rewrites_place(operation: &O, place: PlaceId) -> bool {
    if match operation {
        // Bare-place destinations that replace or re-establish contents.
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::StructuralByteSequenceFieldByteStore { destination, .. }
        | O::ByteSequenceStore { destination, .. }
        | O::StructuralByteSequenceFieldStore { destination, .. }
        | O::EstablishElementView { destination, .. } => *destination == place,
        // Parameter-row destinations and vacating move sources.
        O::WriteOnlyPrimitiveStore { destination, .. } => destination.place() == place,
        O::StoreStructuralField { destination, .. } => destination.place() == place,
        O::StructuralScalarFieldStore { destination, .. } => destination.place() == place,
        O::MoveStructuralField { source, .. } => source.place() == place,
        // An atomic resident's contents may change under another event.
        O::AtomicEvent { event, .. } => event.place() == Some(place),
        _ => false,
    } {
        return true;
    }
    structural_argument_operands(operation).any(|argument| argument_rewrites_place(argument, place))
}

/// Every structural-argument operand `operation` carries: record children
/// stored whole, element-view and reference sources, a reseating store's
/// delivered value, and call argument rows.
fn structural_argument_operands(operation: &O) -> impl Iterator<Item = &StructuralArgument> {
    let mut operands: Vec<&StructuralArgument> = Vec::new();
    match operation {
        O::EstablishCase { fields, .. } | O::EstablishRecord { fields, .. } => {
            for initializer in fields {
                if let Some(argument) = initializer.value.as_structural() {
                    operands.push(argument);
                }
            }
        }
        O::EstablishElementView { source, .. } | O::EstablishReference { source, .. } => {
            operands.push(source);
        }
        O::StoreStructuralField { value, .. } => operands.push(value),
        O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => operands.extend(structural_arguments.iter()),
        _ => {}
    }
    operands.into_iter()
}
