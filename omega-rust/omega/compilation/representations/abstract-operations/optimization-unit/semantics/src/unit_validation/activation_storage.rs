//! Replay activation order from the current optimized graph. This projection
//! retains every operation which can establish, transport or retire a claim or
//! its loans; scalar computations and byte observations do not affect custody.

use crate::OptimizationUnitValidationError;
use abstract_operations::{AbstractCallTarget, AbstractOperation as A, StructuralCopyPath};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use terminal_psi::{Operation, OperationKind as T, OperationResult, Terminator};

pub(super) fn validate(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let active = function.block_nodes().any(|node| {
        matches!(
            node.operation,
            A::EstablishActivationClaim { .. } | A::ReleaseActivationClaim { .. }
        )
    });
    let (suspending, known, machines) = possibly_suspending_operations(unit);
    let project = || -> Result<(), &'static str> {
        let claim_types = unit
            .block_nodes()
            .filter(|node| matches!(node.operation, A::EstablishActivationClaim { .. }))
            .map(|node| {
                let projected = operation(&node.operation, true)?
                    .ok_or("activation establishment lost its replay event")?;
                terminal_semantics::activation_storage::activation_claim_shape_in(
                    &unit.structural_types,
                    &projected,
                )
                .map(|shape| shape.claim_type)
            })
            .collect::<Result<std::collections::BTreeSet<_>, &'static str>>()?;
        let mut blocks = Vec::with_capacity(function.blocks.len());
        for block in &function.blocks {
            let mut operations = Vec::new();
            let mut terminator = None;
            for node in &block.nodes {
                if let Some(end) = termination(&node.operation) {
                    terminator = Some(end);
                    continue;
                }
                if let Some(operation) = operation(&node.operation, active)? {
                    operations.push(operation);
                }
            }
            blocks.push(terminal_psi::Block {
                id: block.id,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: block.structural_parameters.clone(),
                operations,
                terminator: terminator
                    .ok_or("activation replay block has no current terminator")?,
            });
        }
        terminal_semantics::activation_storage::validate_flow(
            &unit.structural_types,
            &function.structural_parameters,
            &function.block_entry_claims,
            function.entry,
            &blocks,
            &claim_types,
            &|id| suspending.contains(&id),
            &|callee| !known.contains(&callee) || machines.contains(&callee),
        )?;
        Ok(())
    };
    project().map_err(
        |reason| OptimizationUnitValidationError::InvalidActivationStorage {
            machine: function.machine,
            reason,
        },
    )
}

fn operation(source: &A, active: bool) -> Result<Option<Operation>, &'static str> {
    let mut result = source
        .structural_result()
        .map(|(_, result)| result.clone())
        .map_or(OperationResult::Unit, OperationResult::Structural);
    let kind = match source {
        A::EstablishActivationClaim {
            bound,
            committed,
            backing_type,
            established_case,
            rejected_case,
            ..
        } => T::EstablishActivationClaim {
            bound: *bound,
            committed: *committed,
            backing_type: *backing_type,
            established_case: *established_case,
            rejected_case: *rejected_case,
        },
        A::ReleaseActivationClaim { source, claim, .. } => T::ReleaseActivationClaim {
            source: source.clone(),
            claim: *claim,
        },
        A::MoveStructuralValue { source, .. } => T::MoveStructuralValue {
            source: source.clone(),
        },
        A::MoveStructuralField {
            source,
            path,
            field,
            ..
        } => T::MoveStructuralField {
            source: source.place(),
            path: path.clone(),
            field: *field,
        },
        A::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
            ..
        } => T::StoreStructuralField {
            disposition: *disposition,
            destination: destination.place(),
            path: path.clone(),
            field: *field,
            value: value.clone(),
        },
        A::EstablishRecord { fields, .. } => T::EstablishRecord {
            fields: fields.clone(),
        },
        A::EstablishCase {
            result_case,
            fields,
            ..
        } => T::EstablishStructuralCase {
            result_case: *result_case,
            fields: fields.clone(),
        },
        A::EstablishStructuralArray { elements, .. } => T::EstablishStructuralArray {
            elements: elements.clone(),
        },
        A::EstablishStructuralLocal { .. } => T::EstablishStructuralLocal {},
        A::StructuralLeafCopy { source, path, .. } => match path {
            StructuralCopyPath::Ordinary(path) => T::StructuralLeafCopy {
                source: *source,
                path: path.clone(),
            },
            StructuralCopyPath::Case(path) => T::StructuralCaseLeafCopy {
                source: *source,
                path: path.clone(),
            },
        },
        A::TransformContent {
            source,
            parents,
            derivations,
            ..
        } => T::TransformContent {
            source: source.clone(),
            parents: parents.clone(),
            derivations: derivations.clone(),
        },
        A::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            obligation,
            ..
        } => T::ByteSequenceSubslice {
            source: source.clone(),
            start: *start,
            end: *end,
            length: *length,
            obligation: *obligation,
        },
        A::ElementViewSubslice {
            source,
            start,
            end,
            length,
            obligation,
            ..
        } => T::ElementViewSubslice {
            source: source.clone(),
            start: *start,
            end: *end,
            length: *length,
            obligation: *obligation,
        },
        A::EstablishReference {
            source, storage, ..
        } => T::EstablishReference {
            source: source.clone(),
            storage: storage.clone(),
        },
        A::EstablishPlacedReference {
            source,
            correspondence,
            ..
        } => T::EstablishPlacedReference {
            source: source.clone(),
            resident: correspondence.resident_source(),
        },
        A::EstablishElementView {
            destination,
            source,
            element,
            region,
            ..
        } => {
            if source.access == terminal_psi::StructuralAccess::MutableBorrow {
                T::EstablishMutableElementView {
                    destination: *destination,
                    source: source.clone(),
                    element: *element,
                    region: *region,
                }
            } else {
                T::EstablishElementView {
                    destination: *destination,
                    source: source.clone(),
                    element: *element,
                    region: *region,
                }
            }
        }
        A::ReleaseReference { source, path, .. } => T::ReleaseReference {
            source: *source,
            path: path.clone(),
        },
        A::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            predicate_obligation,
            ..
        } => T::StructuralByteViewFieldStore {
            destination: *destination,
            path: path.clone(),
            field: *field,
            source: *source,
            predicate_obligation: *predicate_obligation,
        },
        A::StructuralCaseMembership {
            result: scalar,
            source,
            path,
            case,
            ..
        } => {
            result = OperationResult::Scalar(terminal_psi::ValueDeclaration {
                id: scalar.value,
                scalar_type: scalar.scalar_type,
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            });
            T::StructuralCaseMembership {
                source: *source,
                path: path.clone(),
                case: *case,
            }
        }
        A::Call {
            result: value,
            scalar_type,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } => {
            result = OperationResult::Scalar(terminal_psi::ValueDeclaration {
                id: *value,
                scalar_type: *scalar_type,
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            });
            T::Call {
                callee: *callee,
                arguments: arguments.clone(),
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
            }
        }
        A::CallStructural {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } => match callee {
            AbstractCallTarget::Machine(callee) => T::CallUnit {
                callee: *callee,
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers: claim_transfers.clone(),
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
            },
            AbstractCallTarget::Dynamic {
                selector,
                interface,
                slot,
            } => T::CallDynamic {
                selector: *selector,
                interface: *interface,
                slot: *slot,
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers: claim_transfers.clone(),
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
                family_actuals: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
            },
        },
        A::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            ..
        } => T::BoundaryCall {
            boundary: *boundary,
            arguments: arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            completion_receipts: completion_receipts.clone(),
            requirement_obligations: Vec::new(),
        },
        // Every remaining structural result is rejected in a claim-bearing
        // activation. An unimplemented descriptor transport cannot silently
        // erase its origin from this replay. Other validators independently
        // check scalar/byte accesses, effects, and ordinary linear custody.
        _ if active && source.structural_result().is_some() => {
            return Err(
                "activation replay has no origin-preserving projection for this structural operation",
            );
        }
        _ => return Ok(None),
    };
    let id = source
        .psi_operation()
        .ok_or("activation replay operation has no source identity")?;
    Ok(Some(Operation {
        id,
        static_reach_binding: None,
        suspension_crossing: None,
        result,
        kind,
    }))
}

fn successor(edge: &abstract_operations::AbstractSuccessor) -> terminal_psi::SuccessorEdge {
    terminal_psi::SuccessorEdge {
        edge: edge.psi_edge,
        target: edge.target,
        arguments: edge
            .bindings
            .iter()
            .map(|binding| binding.argument)
            .collect(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: edge
            .structural_bindings
            .iter()
            .map(|binding| binding.argument.clone())
            .collect(),
        cleanup_actions: edge.cleanup_actions.clone(),
    }
}

fn termination(operation: &A) -> Option<Terminator> {
    Some(match operation {
        A::Jump {
            psi_edge,
            target,
            bindings,
            structural_bindings,
            cleanup_actions,
        } => Terminator::Jump {
            edge: *psi_edge,
            target: *target,
            arguments: bindings.iter().map(|binding| binding.argument).collect(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: structural_bindings
                .iter()
                .map(|binding| binding.argument.clone())
                .collect(),
            cleanup_actions: cleanup_actions.clone(),
        },
        A::Conditional {
            condition,
            when_true,
            when_false,
        } => Terminator::Conditional {
            condition: *condition,
            when_true: successor(when_true),
            when_false: successor(when_false),
        },
        A::StructuralCase {
            source,
            path,
            cases,
        } => Terminator::StructuralCase {
            source: *source,
            path: path.clone(),
            cases: cases
                .iter()
                .map(|edge| terminal_psi::StructuralCaseSuccessorEdge {
                    edge: edge.psi_edge,
                    target: edge.target,
                    case: edge.case,
                    payload_fields: edge.payloads.iter().map(|payload| payload.field).collect(),
                    cleanup_actions: edge.cleanup_actions.clone(),
                })
                .collect(),
        },
        A::Return {
            psi_edge,
            value,
            cleanup_actions,
            ..
        } => Terminator::Return {
            edge: *psi_edge,
            value: *value,
            cleanup_actions: cleanup_actions.clone(),
        },
        A::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => Terminator::ReturnUnit {
            edge: *psi_edge,
            cleanup_actions: cleanup_actions.clone(),
        },
        A::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            cleanup_actions,
            ..
        } => Terminator::ReturnStructural {
            edge: *psi_edge,
            source: *source,
            returned_claims: returned_claims.clone(),
            cleanup_actions: cleanup_actions.clone(),
        },
        A::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => Terminator::Crash {
            edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        A::InvokeNoNormal {
            psi_edge,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => Terminator::InvokeNoNormal {
            edge: *psi_edge,
            static_reach_binding: None,
            callee: *callee,
            arguments: arguments.clone(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: structural_arguments.clone(),
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
        A::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
        } => Terminator::ExternalCompletion {
            edge: *psi_edge,
            boundary: *boundary,
            kind: *kind,
            arguments: arguments.clone(),
            structural_arguments: structural_arguments.clone(),
            requirement_obligations: requirement_obligations.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        },
        _ => return None,
    })
}

// Recompute from the executable graph: effects/provenance rows do not prove
// preservation of suspension crossings after optimization. Unknown and dynamic
// calls fail closed; a closed internal leaf can still borrow activation bytes.
fn possibly_suspending_operations(
    unit: &PsiOptimizationUnit,
) -> (
    std::collections::BTreeSet<semantic_vocabulary::OperationId>,
    std::collections::BTreeSet<semantic_vocabulary::MachineId>,
    std::collections::BTreeSet<semantic_vocabulary::MachineId>,
) {
    use std::collections::BTreeSet;
    let known: BTreeSet<_> = unit.functions.iter().map(|f| f.machine).collect();
    let mut machines = BTreeSet::new();
    let may = |operation: &A, machines: &BTreeSet<_>| match operation {
        A::Call { callee, .. }
        | A::CallStructural {
            callee: AbstractCallTarget::Machine(callee),
            ..
        } => !known.contains(callee) || machines.contains(callee),
        A::CallStructural {
            callee: AbstractCallTarget::Dynamic { .. },
            ..
        }
        | A::BoundaryCall { .. } => true,
        _ => false,
    };
    loop {
        let additions: Vec<_> = unit
            .functions
            .iter()
            .filter(|f| f.block_nodes().any(|n| {
                may(&n.operation, &machines) || n.operation.cleanup_rosters()
                    .flat_map(|(_, actions)| actions).any(|action| {
                        matches!(action, terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup)
                            if !known.contains(&cleanup.cleanup_machine) || machines.contains(&cleanup.cleanup_machine))
                    })
            }))
            .map(|f| f.machine)
            .collect();
        let previous = machines.len();
        machines.extend(additions);
        if previous == machines.len() {
            break;
        }
    }
    let operations = unit
        .block_nodes()
        .filter(|n| may(&n.operation, &machines))
        .filter_map(|n| n.operation.psi_operation())
        .collect();
    (operations, known, machines)
}
