/*
We own whole-unit validation for `PsiOptimizationUnit`. Proceeding in strict
phase order, we verify canonical unit and terminal identities, accepted
obligation and proof-question indexes, machine rosters, structural type and
domain catalogs, service hierarchies and root reach, per-function CFG dominance
and derived node metadata, per-operation scalar and structural contracts,
no-normal-call invariants, runtime evidence, and retained affine cleanup
authority.
*/

use crate::OptimizationUnitValidationError;
use optimization_unit::{
    PsiOptimizationFunction, PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::MachineId;

mod activation_storage {
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
                storage: *storage,
            },
            A::EstablishAccessReference {
                source,
                correspondence,
                ..
            } => T::EstablishAccessReference {
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
            required_contract: None,
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
                        structural_arguments: edge
                            .structural_bindings
                            .iter()
                            .map(|binding| binding.argument.clone())
                            .collect(),
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
            required_contract: None,
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
}
pub(crate) mod affine_authority {
    use crate::{OperationSite, OptimizationUnitValidationError};
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{
        OwnershipFrontierOwnedPlace, OwnershipFrontierSite, OwnershipFrontierSnapshot,
        PsiOptimizationUnit, PsiProvenance,
    };
    use semantic_vocabulary::PlaceId;
    use std::collections::BTreeMap;

    mod block_bindings {
        use abstract_operations::AbstractStructuralBinding;
        use optimization_unit::{
            OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace,
            OwnershipFrontierPartialCustody, OwnershipFrontierSnapshot, PsiOptimizationFunction,
        };
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{
            StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralTypeDeclaration, TerminalAffineCleanupAction,
        };

        pub(super) fn valid_transition(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            unrestricted: &[PlaceId],
            entry: &OwnershipFrontierSnapshot,
            exit: &OwnershipFrontierSnapshot,
            actions: &[TerminalAffineCleanupAction],
            bindings: &[AbstractStructuralBinding],
        ) -> bool {
            let Some(consumed) = consume_sources(function, types, entry, bindings) else {
                return false;
            };
            let Some(mut expected) =
                retire_cleanup_roots(function, types, unrestricted, consumed.frontier, actions)
            else {
                return false;
            };
            let mut owned = expected
                .owned_places
                .iter()
                .map(|owned| (owned.place, owned.multiplicity))
                .collect::<BTreeMap<_, _>>();
            for parameter in consumed.arrivals {
                if owned
                    .insert(parameter.place, parameter.multiplicity)
                    .is_some()
                    || expected
                        .claims
                        .iter()
                        .any(|claim| claim.input == Some(parameter.place))
                {
                    return false;
                }
            }
            for claim in consumed.claims {
                if expected
                    .claims
                    .iter()
                    .any(|current| current.claim == claim.claim)
                {
                    return false;
                }
                expected.claims.push(claim);
            }
            expected.claims.sort_by_key(|claim| claim.claim);
            expected.owned_places = owned
                .into_iter()
                .map(|(place, multiplicity)| OwnershipFrontierOwnedPlace {
                    place,
                    multiplicity,
                })
                .collect();
            expected == *exit
        }

        fn retire_cleanup_roots(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            unrestricted: &[PlaceId],
            mut frontier: OwnershipFrontierSnapshot,
            actions: &[TerminalAffineCleanupAction],
        ) -> Option<OwnershipFrontierSnapshot> {
            let mut retired = Vec::<PlaceId>::new();
            let mut remaining = actions;
            while let Some(action) = remaining.first() {
                let place = action.place();
                if retired.contains(&place) {
                    return None;
                }
                let count = remaining
                    .iter()
                    .take_while(|action| action.place() == place)
                    .count();
                let (group, rest) = remaining.split_at(count);
                if unrestricted.binary_search(&place).is_ok() {
                    if !matches!(group,
                        [TerminalAffineCleanupAction::RetireHome(root) | TerminalAffineCleanupAction::DiscardRoot(root)] if *root == place)
                        || frontier
                            .owned_places
                            .iter()
                            .any(|owned| owned.place == place)
                        || frontier
                            .partial_custody
                            .iter()
                            .any(|custody| custody.place == place)
                        || frontier
                            .claims
                            .iter()
                            .any(|claim| claim.input == Some(place))
                    {
                        return None;
                    }
                    retired.push(place);
                    remaining = rest;
                    continue;
                }
                if group
                    .iter()
                    .all(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_)))
                {
                    consume_residual_group(function, types, &mut frontier, place, group)?;
                    retired.push(place);
                    remaining = rest;
                    continue;
                }
                let partition = match group.split_last()? {
                    (TerminalAffineCleanupAction::RetireHome(retired), partition)
                        if *retired == place =>
                    {
                        partition
                    }
                    _ if matches!(group, [TerminalAffineCleanupAction::DiscardRoot(_)])
                        || matches!(group, [TerminalAffineCleanupAction::InvokeNominal(cleanup)] if cleanup.path.is_empty()) =>
                    {
                        group
                    }
                    _ => return None,
                };
                for action in partition {
                    if matches!(action, TerminalAffineCleanupAction::RetireHome(_)) {
                        return None;
                    }
                    if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                        for consumed in &cleanup.consumed_claims {
                            let position = frontier.claims.iter().position(|claim| {
                                claim.claim == *consumed && claim.input == Some(place)
                            })?;
                            frontier.claims.remove(position);
                        }
                    }
                }
                if frontier
                    .claims
                    .iter()
                    .any(|claim| claim.input == Some(place))
                {
                    return None;
                }
                if let Some(position) = frontier
                    .owned_places
                    .iter()
                    .position(|owned| owned.place == place)
                {
                    frontier.owned_places.remove(position);
                } else {
                    let custody = frontier
                        .partial_custody
                        .iter()
                        .find(|custody| custody.place == place)?;
                    let moved = custody.moved_paths.iter().cloned().collect::<BTreeSet<_>>();
                    if !matches!(group, [TerminalAffineCleanupAction::RetireHome(_)])
                        || !crate::current_ownership::projected_paths_are_fully_consumed(
                            function, types, place, &moved,
                        )
                    {
                        return None;
                    }
                }
                frontier
                    .partial_custody
                    .retain(|custody| custody.place != place);
                retired.push(place);
                remaining = rest;
            }
            Some(frontier)
        }

        /// An equalizing edge discards only its live remainder and retains
        /// the allocated owner. Record exactly those absent paths instead of
        /// treating every cleanup group as whole-home retirement.
        fn consume_residual_group(
            function: &PsiOptimizationFunction,
            types: &(impl crate::StructuralTypes + ?Sized),
            frontier: &mut OwnershipFrontierSnapshot,
            place: PlaceId,
            actions: &[TerminalAffineCleanupAction],
        ) -> Option<()> {
            if !frontier.owned_places.iter().any(|owned| {
                owned.place == place && owned.multiplicity == StructuralMultiplicity::Affine
            }) {
                return None;
            }
            let source =
                super::super::operation_contracts::structural_source_contract(function, place)?;
            if source.access != StructuralAccess::Owned
                || source.multiplicity != StructuralMultiplicity::Affine
            {
                return None;
            }
            let mut moved = frontier
                .partial_custody
                .iter()
                .find(|custody| custody.place == place)
                .map(|custody| custody.moved_paths.iter().cloned().collect::<BTreeSet<_>>())
                .unwrap_or_default();
            for action in actions {
                let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                    return None;
                };
                if discard.place != place
                    || discard.path.is_empty()
                    || super::super::structural_catalog::resolve_structural_path(
                        types,
                        source.structural_type,
                        &discard.path,
                    ) != Some(discard.structural_type)
                    || types
                        .declaration(discard.structural_type)
                        .is_none_or(|declaration| !declaration.no_code_affine_cleanup)
                    || frontier.claims.iter().any(|claim| {
                        claim.input == Some(place)
                            && terminal_semantics::structural_paths_may_overlap(
                                &claim.path,
                                &discard.path,
                            )
                    })
                    || moved.iter().any(|existing| {
                        existing.starts_with(&discard.path) || discard.path.starts_with(existing)
                    })
                {
                    return None;
                }
                moved.insert(discard.path.clone());
            }
            if crate::current_ownership::partial_affine_root_is_drained(
                function, types, place, &moved,
            ) {
                frontier.owned_places.retain(|owned| owned.place != place);
            }
            let missing = OwnershipFrontierPartialCustody {
                place,
                moved_paths: moved.into_iter().collect(),
            };
            match frontier
                .partial_custody
                .binary_search_by_key(&place, |custody| custody.place)
            {
                Ok(position) => frontier.partial_custody[position] = missing,
                Err(position) => frontier.partial_custody.insert(position, missing),
            }
            Some(())
        }

        struct ConsumedSources<'function> {
            frontier: OwnershipFrontierSnapshot,
            arrivals: Vec<&'function StructuralParameterDeclaration>,
            claims: Vec<OwnershipFrontierLiveClaim>,
        }

        fn consume_sources<'function>(
            function: &'function PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            entry: &OwnershipFrontierSnapshot,
            bindings: &[AbstractStructuralBinding],
        ) -> Option<ConsumedSources<'function>> {
            let mut frontier = entry.clone();
            let mut arrivals = Vec::new();
            let mut claims = Vec::new();
            let mut removed_claims = BTreeSet::new();
            for binding in bindings {
                let parameter = function
                    .block_structural_parameters()
                    .find(|parameter| parameter.place == binding.parameter)?;
                if parameter.access != StructuralAccess::Owned {
                    continue;
                }
                let source = binding.argument.place;
                if binding.argument.access != StructuralAccess::Owned {
                    return None;
                }
                let source_claims = entry
                    .claims
                    .iter()
                    .filter(|claim| claim.input == Some(source))
                    .collect::<Vec<_>>();
                if !binding.argument.path.is_empty() {
                    if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                        let source_contract =
                            super::super::operation_contracts::structural_source_contract(
                                function, source,
                            )?;
                        if source_contract.access != StructuralAccess::Owned
                            || source_contract.multiplicity != StructuralMultiplicity::Unrestricted
                            || !source_claims.is_empty()
                            || frontier
                                .owned_places
                                .iter()
                                .any(|owned| owned.place == source)
                            || frontier
                                .partial_custody
                                .iter()
                                .any(|custody| custody.place == source)
                        {
                            return None;
                        }
                        continue;
                    }
                    if parameter.multiplicity != StructuralMultiplicity::Affine
                        || !source_claims.is_empty()
                        || !move_projected_child(function, types, &mut frontier, source, binding)
                    {
                        return None;
                    }
                    arrivals.push(parameter);
                    continue;
                }
                if frontier
                    .partial_custody
                    .iter()
                    .any(|custody| custody.place == source)
                    || (parameter.multiplicity == StructuralMultiplicity::Linear
                        && source_claims.is_empty())
                    || (parameter.multiplicity != StructuralMultiplicity::Linear
                        && !source_claims.is_empty())
                {
                    return None;
                }
                if parameter.multiplicity != StructuralMultiplicity::Unrestricted {
                    let position = frontier.owned_places.iter().position(|owned| {
                        owned.place == source && owned.multiplicity == parameter.multiplicity
                    })?;
                    frontier.owned_places.remove(position);
                    arrivals.push(parameter);
                }
                for claim in source_claims {
                    if !removed_claims.insert(claim.claim) {
                        return None;
                    }
                    let mut moved = claim.clone();
                    moved.input = Some(parameter.place);
                    claims.push(moved);
                }
            }
            frontier
                .claims
                .retain(|claim| !removed_claims.contains(&claim.claim));
            Some(ConsumedSources {
                frontier,
                arrivals,
                claims,
            })
        }

        fn move_projected_child(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            frontier: &mut OwnershipFrontierSnapshot,
            source: PlaceId,
            binding: &AbstractStructuralBinding,
        ) -> bool {
            let path = &binding.argument.path;
            if !frontier.owned_places.iter().any(|owned| {
                owned.place == source && owned.multiplicity == StructuralMultiplicity::Affine
            }) {
                return false;
            }
            let position = match frontier
                .partial_custody
                .binary_search_by_key(&source, |custody| custody.place)
            {
                Ok(position) => position,
                Err(position) => {
                    frontier.partial_custody.insert(
                        position,
                        OwnershipFrontierPartialCustody {
                            place: source,
                            moved_paths: Vec::new(),
                        },
                    );
                    position
                }
            };
            let moved = &mut frontier.partial_custody[position].moved_paths;
            if moved
                .iter()
                .any(|existing| existing.starts_with(path) || path.starts_with(existing))
            {
                return false;
            }
            let insertion = moved
                .binary_search(path)
                .unwrap_or_else(|position| position);
            moved.insert(insertion, path.clone());
            let moved = moved.iter().cloned().collect::<BTreeSet<_>>();
            if crate::current_ownership::partial_affine_root_is_drained(
                function, types, source, &moved,
            ) {
                frontier.owned_places.retain(|owned| owned.place != source);
            }
            true
        }
        #[cfg(test)]
        mod tests {
            use super::*;
            use semantic_vocabulary::PsiSemanticId;
            use terminal_psi::StructuralPathSegment as Segment;
            use terminal_psi::{
                ArrayExtent, StructuralAffineDiscard, StructuralPlaceDeclaration,
                StructuralRecordStorage, StructuralTypeShape, TerminalIdentityIndex,
            };

            fn id<T: PsiSemanticId>(raw: u64) -> T {
                T::new(raw).unwrap()
            }

            #[test]
            fn a_residual_only_edge_retains_its_actual_parent_home() {
                let declarations = (1..=3)
                    .map(|raw| StructuralTypeDeclaration {
                        id: id(raw),
                        identity: TerminalIdentityIndex(raw as u32),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        copyable: false,
                        no_code_affine_cleanup: true,
                        descriptor_cleanup: true,
                        scalar_restriction: None,
                        requires: Vec::new(),
                        shape: if raw == 1 {
                            StructuralTypeShape::Record {
                                fields: Vec::new(),
                                storage: StructuralRecordStorage::Packed,
                            }
                        } else {
                            StructuralTypeShape::FixedArray {
                                element: id(raw - 1),
                                length: ArrayExtent::Literal(2),
                            }
                        },
                    })
                    .collect::<Vec<_>>();
                let types = declarations
                    .iter()
                    .map(|declaration| (declaration.id, declaration))
                    .collect();
                let function = PsiOptimizationFunction {
                    machine: id(1),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    entry: id(1),
                    parameters: Vec::new(),
                    structural_parameters: vec![StructuralParameterDeclaration {
                        place: id(1),
                        position: 0,
                        structural_type: id(3),
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    }],
                    structural_places: vec![StructuralPlaceDeclaration {
                        id: id(1),
                        kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 0 },
                    }],
                    result: abstract_operations::AbstractFunctionResult::Unit,
                    declared_places: BTreeSet::from([id(1)]),
                    entry_claim_declarations: Vec::new(),
                    block_entry_claims: Vec::new(),
                    content_entry_claims: Vec::new(),
                    verified_contract: None,
                    evidence_contract_lanes: Vec::new(),
                    entry_claims: BTreeSet::new(),
                    published_service_ceiling: Vec::new(),
                    declared_service_reach: Vec::new(),
                    facts: Vec::new(),
                    blocks: Vec::new(),
                };
                let first = vec![Segment::FixedIndex(0), Segment::FixedIndex(0)];
                let second = vec![Segment::FixedIndex(0), Segment::FixedIndex(1)];
                let entry = OwnershipFrontierSnapshot {
                    claims: Vec::new(),
                    owned_places: vec![OwnershipFrontierOwnedPlace {
                        place: id(1),
                        multiplicity: StructuralMultiplicity::Affine,
                    }],
                    partial_custody: vec![OwnershipFrontierPartialCustody {
                        place: id(1),
                        moved_paths: vec![first.clone()],
                    }],
                };
                let mut exit = entry.clone();
                exit.partial_custody[0].moved_paths.push(second.clone());
                let action =
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place: id(1),
                        path: second,
                        structural_type: id(1),
                    });
                assert!(valid_transition(
                    &function,
                    &types,
                    &[],
                    &entry,
                    &exit,
                    std::slice::from_ref(&action),
                    &[]
                ));
                let last_sibling =
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place: id(1),
                        path: vec![Segment::FixedIndex(1)],
                        structural_type: id(2),
                    });
                let mut vacant = exit.clone();
                vacant.owned_places.clear();
                vacant.partial_custody[0]
                    .moved_paths
                    .push(vec![Segment::FixedIndex(1)]);
                let mut all = vec![last_sibling, action.clone()];
                assert!(valid_transition(
                    &function,
                    &types,
                    &[],
                    &entry,
                    &vacant,
                    &all,
                    &[]
                ));
                let mut ended = vacant.clone();
                ended.partial_custody.clear();
                assert!(
                    !valid_transition(&function, &types, &[], &entry, &ended, &all, &[]),
                    "an empty obligation frontier retains the vacant home until explicit retirement"
                );
                all.push(TerminalAffineCleanupAction::RetireHome(id(1)));
                assert!(valid_transition(
                    &function,
                    &types,
                    &[],
                    &entry,
                    &ended,
                    &all,
                    &[]
                ));
                let mut wrong_type = action.clone();
                let TerminalAffineCleanupAction::DiscardResidual(discard) = &mut wrong_type else {
                    unreachable!()
                };
                discard.structural_type = id(2);
                assert!(!valid_transition(
                    &function,
                    &types,
                    &[],
                    &entry,
                    &exit,
                    &[wrong_type],
                    &[]
                ));
                assert!(!valid_transition(
                    &function,
                    &types,
                    &[],
                    &entry,
                    &exit,
                    &[action.clone(), action.clone()],
                    &[]
                ));
                let mut claimed = entry.clone();
                claimed
                    .claims
                    .push(optimization_unit::OwnershipFrontierLiveClaim {
                        claim: id(1),
                        input: Some(id(1)),
                        path: vec![Segment::FixedIndex(0), Segment::FixedIndex(1)],
                        multiplicity: Some(StructuralMultiplicity::Linear),
                    });
                assert!(!valid_transition(
                    &function,
                    &types,
                    &[],
                    &claimed,
                    &exit,
                    std::slice::from_ref(&action),
                    &[]
                ));
                assert!(
                    !valid_transition(&function, &types, &[], &entry, &entry, &[action], &[]),
                    "the retained frontier must record the actual discarded subtree"
                );
            }
        }
    }
    pub(super) fn validate_retained_ownership_authority(
        unit: &PsiOptimizationUnit,
    ) -> Result<(), OptimizationUnitValidationError> {
        if unit.ownership_frontier_facts.is_empty() {
            return Ok(());
        }
        let frontiers = unit
            .ownership_frontier_facts
            .iter()
            .map(|fact| ((fact.machine, fact.site), &fact.snapshot))
            .collect::<BTreeMap<_, _>>();
        let structural_types = unit
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>();

        for function in &unit.functions {
            let unrestricted = crate::current_ownership::unrestricted_owned_cleanup_roots(
                function,
                &structural_types,
            );
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                    for edge in &node.successors {
                        for (source_index, source) in edge.provenance.iter().enumerate() {
                            let PsiProvenance::Edge(source) = source else {
                                return Err(
                                OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                },
                            );
                            };
                            let Some(entry) = frontiers.get(&(
                                function.machine,
                                OwnershipFrontierSite::EdgeEntry(*source),
                            )) else {
                                return Err(
                                OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                                    machine: function.machine,
                                    edge: *source,
                                },
                            );
                            };
                            let Some(exit) = frontiers
                                .get(&(function.machine, OwnershipFrontierSite::EdgeExit(*source)))
                            else {
                                return Err(
                                OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                                    machine: function.machine,
                                    edge: *source,
                                },
                            );
                            };
                            let (actions, bindings) = if source_index == 0 {
                                (
                                    edge.cleanup_actions.as_slice(),
                                    edge.structural_bindings.as_slice(),
                                )
                            } else {
                                (&[][..], &[][..])
                            };
                            if !block_bindings::valid_transition(
                                function,
                                &structural_types,
                                &unrestricted,
                                entry,
                                exit,
                                actions,
                                bindings,
                            ) {
                                return Err(
                                OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                },
                            );
                            }
                        }
                    }

                    let O::ReturnStructural {
                        trivial_affine_locals,
                        ..
                    } = &node.operation
                    else {
                        continue;
                    };
                    for (operation, place, _) in trivial_affine_locals {
                        let mismatch = || {
                            OptimizationUnitValidationError::StructuralReturnHiddenLocalCustodyMismatch(
                            OperationSite::new(function.machine, block.id, node_index),
                            *operation,
                        )
                        };
                        let entry = frontiers
                            .get(&(
                                function.machine,
                                OwnershipFrontierSite::OperationEntry(*operation),
                            ))
                            .ok_or_else(mismatch)?;
                        let exit = frontiers
                            .get(&(
                                function.machine,
                                OwnershipFrontierSite::OperationExit(*operation),
                            ))
                            .ok_or_else(mismatch)?;
                        if !valid_hidden_affine_establishment(entry, exit, place.id) {
                            return Err(mismatch());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn valid_hidden_affine_establishment(
        entry: &OwnershipFrontierSnapshot,
        exit: &OwnershipFrontierSnapshot,
        place: PlaceId,
    ) -> bool {
        let mut expected_owned = entry.owned_places.clone();
        if expected_owned.iter().any(|owned| owned.place == place) {
            return false;
        }
        expected_owned.push(OwnershipFrontierOwnedPlace {
            place,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        });
        expected_owned.sort_by_key(|owned| owned.place);
        entry.claims == exit.claims
            && entry.partial_custody == exit.partial_custody
            && expected_owned == exit.owned_places
    }
}
pub(crate) mod arrival_storage {
    use optimization_unit::OptimizationEdge;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{BlockId, PlaceId};
    use std::collections::{BTreeMap, BTreeSet};

    pub(crate) struct StorageNames {
        behind: BTreeMap<BlockId, BTreeSet<PlaceId>>,
    }

    impl StorageNames {
        pub(crate) fn in_block(&self, block: BlockId) -> impl Iterator<Item = PlaceId> + '_ {
            self.behind.get(&block).into_iter().flatten().copied()
        }
    }

    pub(crate) fn storage_names(
        function: &PsiOptimizationFunction,
        block: BlockId,
        root: PlaceId,
    ) -> StorageNames {
        let mut behind = BTreeMap::<BlockId, BTreeSet<PlaceId>>::new();
        let mut pending = vec![(block, root)];
        while let Some((block, root)) = pending.pop() {
            if !behind.entry(block).or_default().insert(root) {
                continue;
            }
            for predecessor in &function.blocks {
                for edge in predecessor.successors() {
                    if edge.target == block
                        && let Some(before) = root_before_edge(edge, root)
                    {
                        pending.push((predecessor.id, before));
                    }
                }
            }
        }
        StorageNames { behind }
    }

    pub(crate) fn root_before_edge(edge: &OptimizationEdge, place: PlaceId) -> Option<PlaceId> {
        let (root, path) = projection_before_edge(edge, place);
        path.is_empty().then_some(root)
    }

    pub(crate) fn projection_before_edge(
        edge: &OptimizationEdge,
        place: PlaceId,
    ) -> (PlaceId, &[terminal_psi::StructuralPathSegment]) {
        match edge.structural_binding(place) {
            Some(binding) => (binding.argument.place, &binding.argument.path),
            None => (place, &[]),
        }
    }
}
pub(crate) fn catalog_mismatch(
    function: &PsiOptimizationFunction,
) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    }
}
mod catalogs {
    use super::operation_contracts::structural_source_contract;
    use crate::OptimizationUnitValidationError;
    use crate::unit_validation::function_structure;
    use crate::unit_validation::function_structure::validate_function;
    use crate::unit_validation::services::{
        boundary_structural_signature_matches, index_service_catalog, valid_service_ceiling,
        validate_provider_service_refinements, validate_root_service_reach,
    };
    use crate::unit_validation::structural_catalog::index_structural_catalogs;
    use crate::unit_validation::structural_catalog::resolve_structural_path;
    use optimization_unit::SourceCheckedScalarReceiving;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use semantic_vocabulary::ObligationId;
    use semantic_vocabulary::{
        BoundaryMachineId, MachineId, OperationId, ScalarType, ServiceId, StructuralTypeId, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_codec::canonical_proposition_order_key;
    use terminal_psi::StructuralFieldType;
    use terminal_psi::StructuralTypeDeclaration;
    use terminal_psi::find_structural_case;
    use terminal_semantics::indexed_byte_store_goal;
    use terminal_semantics::projected_field_declaration;
    use terminal_semantics::runtime_structural_path_tip;

    pub(super) struct UnitIndexes<'a> {
        pub(super) machines: BTreeMap<MachineId, &'a PsiOptimizationFunction>,
        pub(super) boundary_machines:
            BTreeMap<BoundaryMachineId, &'a terminal_psi::BoundaryMachineDeclaration>,
        pub(super) services: BTreeMap<ServiceId, &'a terminal_psi::ServiceDeclaration>,
        pub(super) pruned: BTreeSet<MachineId>,
        pub(super) ownership_frontiers: Vec<optimization_unit::OwnershipFrontierFact>,
    }

    pub(super) fn index_and_validate_unit_catalogs<'unit>(
        unit: &'unit PsiOptimizationUnit,
        cycle_policy: &function_structure::ControlCyclePolicy,
    ) -> Result<UnitIndexes<'unit>, OptimizationUnitValidationError> {
        if let Some(pair) = unit
            .functions
            .windows(2)
            .find(|pair| pair[0].machine >= pair[1].machine)
        {
            return Err(OptimizationUnitValidationError::UnorderedMachine(
                pair[1].machine,
            ));
        }
        let machines = unit
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect::<BTreeMap<_, _>>();
        if unit
            .pruned_machines
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
        }
        let pruned = unit
            .pruned_machines
            .iter()
            .map(|custody| custody.machine)
            .collect::<BTreeSet<_>>();
        if pruned.len() != unit.pruned_machines.len() {
            return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
        }
        if let Some(machine) = machines
            .keys()
            .find(|machine| pruned.contains(machine))
            .copied()
        {
            return Err(OptimizationUnitValidationError::ActivePrunedMachineOverlap(
                machine,
            ));
        }
        if pruned.contains(&unit.entry) {
            return Err(OptimizationUnitValidationError::PrunedEntryMachine(
                unit.entry,
            ));
        }
        if let Some(machine) = unit
            .provider_candidates
            .iter()
            .map(|candidate| candidate.candidate)
            .find(|machine| pruned.contains(machine))
        {
            return Err(OptimizationUnitValidationError::PrunedProviderMachine(
                machine,
            ));
        }
        if unit
            .accepted_obligation_facts
            .iter()
            .any(|fact| !machines.contains_key(&fact.machine) && !pruned.contains(&fact.machine))
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
        }
        let mut boundary_machines = BTreeMap::new();
        for boundary in &unit.boundary_machines {
            if boundary_machines.insert(boundary.id, boundary).is_some() {
                return Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(
                    boundary.id,
                ));
            }
        }
        let services = index_service_catalog(unit)?;
        let (structural_types, structural_domains) = index_structural_catalogs(unit)?;
        validate_dynamic_catalog(unit, &machines, &structural_types)?;
        for boundary in &unit.boundary_machines {
            if !valid_service_ceiling(&boundary.published_service_ceiling, &services) {
                return Err(
                    OptimizationUnitValidationError::InvalidBoundaryServiceCeiling(boundary.id),
                );
            }
            if !boundary_structural_signature_matches(
                boundary,
                &structural_types,
                &structural_domains,
            ) {
                return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                    machine: None,
                });
            }
        }
        validate_provider_service_refinements(unit, &machines, &boundary_machines)?;
        let mut call_accesses = crate::current_ownership::CallAccesses::new(
            &machines,
            boundary_machines.values().copied(),
            &unit.dynamic_dispatch,
            &structural_types,
        );
        let reference_graph = unit
            .functions
            .iter()
            .any(|function| {
                function.block_nodes().any(|node| {
                    matches!(
                        &node.operation,
                        abstract_operations::AbstractOperation::EstablishReference { .. }
                    )
                })
            })
            .then(|| crate::descriptor_results::optimized_graph(unit))
            .flatten();
        let mut reference_memberships = reference_graph.as_ref().map(|graph| {
            function_structure::ReferenceMembershipValidation::new(&unit.structural_types, graph)
        });
        let mut descriptor_requirements = BTreeMap::new();
        let mut ownership_frontiers = Vec::new();
        for function in &unit.functions {
            validate_function(
                unit,
                function,
                &machines,
                &boundary_machines,
                &services,
                &structural_types,
                &structural_domains,
                cycle_policy,
                &mut call_accesses,
                descriptor_requirements.entry(function.machine).or_default(),
                &mut ownership_frontiers,
                reference_memberships.as_mut(),
            )?;
            super::activation_storage::validate(unit, function)?;
            validate_structural_field_range_authority(unit, function, &structural_types)?;
        }
        crate::current_ownership::validate_descriptor_compatibility(
            unit,
            &descriptor_requirements,
            call_accesses.view_graph(),
        )?;
        Ok(UnitIndexes {
            machines,
            boundary_machines,
            services,
            pruned,
            ownership_frontiers,
        })
    }

    fn validate_dynamic_catalog(
        unit: &PsiOptimizationUnit,
        machines: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let invalid = || OptimizationUnitValidationError::DynamicDispatchCatalogMismatch;
        for (ordinal, interface) in unit.dynamic_dispatch.interfaces.iter().enumerate() {
            if usize::try_from(interface.id).ok() != Some(ordinal)
                || interface.identity == terminal_psi::TerminalIdentityIndex::ABSENT
            {
                return Err(invalid());
            }
            for (ordinal, slot) in interface.slots.iter().enumerate() {
                terminal_semantics::validate_dynamic_reference_regions(
                    types.values().copied(),
                    slot,
                )
                .map_err(|reason| {
                    OptimizationUnitValidationError::DynamicReferenceContract {
                        interface: interface.id,
                        slot: slot.slot,
                        reason,
                    }
                })?;
                if usize::try_from(slot.slot).ok() != Some(ordinal)
                    || slot.declaring_trait_identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || slot.requirement_identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || slot
                        .structural_parameters
                        .iter()
                        .any(|formal| !types.contains_key(&formal.structural_type))
                {
                    return Err(invalid());
                }
                if slot
                    .result_claim_paths
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                {
                    return Err(invalid());
                }
                match &slot.result {
                    terminal_psi::BoundaryMachineResult::Unit
                    | terminal_psi::BoundaryMachineResult::Scalar(_) => {
                        if !slot.result_claim_paths.is_empty() {
                            return Err(invalid());
                        }
                    }
                    terminal_psi::BoundaryMachineResult::Structural(result) => {
                        if !types.contains_key(&result.structural_type)
                            || match result.multiplicity {
                                terminal_psi::StructuralMultiplicity::Unrestricted => {
                                    !slot.result_claim_paths.is_empty()
                                }
                                terminal_psi::StructuralMultiplicity::Linear => {
                                    !matches!(slot.result_claim_paths.as_slice(), [path] if path.is_empty())
                                }
                                terminal_psi::StructuralMultiplicity::Affine => {
                                    slot.result_claim_paths.iter().any(Vec::is_empty)
                                }
                            }
                        {
                            return Err(invalid());
                        }
                        for (position, path) in slot.result_claim_paths.iter().enumerate() {
                            if path.iter().any(|segment| {
                                !matches!(
                                    segment,
                                    terminal_psi::StructuralPathSegment::Field(_)
                                        | terminal_psi::StructuralPathSegment::Case(_)
                                        | terminal_psi::StructuralPathSegment::FixedIndex(_)
                                )
                            }) || resolve_structural_path(types, result.structural_type, path)
                                .is_none()
                                || slot.result_claim_paths[..position].iter().any(|previous| {
                                    path.starts_with(previous) || previous.starts_with(path)
                                })
                            {
                                return Err(invalid());
                            }
                        }
                    }
                }
                if slot
                    .entry_claim_paths
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                {
                    return Err(invalid());
                }
                for (claim_position, claim) in slot.entry_claim_paths.iter().enumerate() {
                    let Some(formal) = slot
                        .structural_parameters
                        .get(claim.argument_index as usize)
                    else {
                        return Err(invalid());
                    };
                    if formal.access != terminal_psi::StructuralAccess::Owned
                        || formal.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                        || (claim.path.is_empty()
                            && formal.multiplicity != terminal_psi::StructuralMultiplicity::Linear)
                        || claim.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                terminal_psi::StructuralPathSegment::Field(_)
                                    | terminal_psi::StructuralPathSegment::Case(_)
                                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
                            )
                        })
                        || resolve_structural_path(types, formal.structural_type, &claim.path)
                            .is_none()
                        || slot.entry_claim_paths[..claim_position]
                            .iter()
                            .any(|previous| {
                                previous.argument_index == claim.argument_index
                                    && (previous.path.starts_with(&claim.path)
                                        || claim.path.starts_with(&previous.path))
                            })
                    {
                        return Err(invalid());
                    }
                }
                if slot
                    .structural_parameters
                    .iter()
                    .enumerate()
                    .any(|(argument_index, formal)| {
                        formal.access == terminal_psi::StructuralAccess::Owned
                            && formal.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                            && !slot
                                .entry_claim_paths
                                .iter()
                                .any(|claim| claim.argument_index as usize == argument_index)
                    })
                {
                    return Err(invalid());
                }
            }
        }
        for (ordinal, table) in unit.dynamic_dispatch.tables.iter().enumerate() {
            let interface = unit
                .dynamic_dispatch
                .interface(table.interface)
                .ok_or_else(invalid)?;
            if usize::try_from(table.id).ok() != Some(ordinal)
                || table.implementation_identity == terminal_psi::TerminalIdentityIndex::ABSENT
                || table.rows.len() != interface.slots.len()
            {
                return Err(invalid());
            }
            for (row, slot) in table.rows.iter().zip(&interface.slots) {
                let Some(machine) = row else {
                    continue;
                };
                let function = machines.get(machine).ok_or_else(invalid)?;
                let result_matches = match &function.result {
                    abstract_operations::AbstractFunctionResult::Unit => slot.result.is_unit(),
                    abstract_operations::AbstractFunctionResult::Scalar(result) => {
                        slot.result.scalar() == Some(result.scalar_type)
                    }
                    abstract_operations::AbstractFunctionResult::Structural(result) => {
                        terminal_semantics::reference_result_refines_regions(
                            types.values().copied(),
                            &function.structural_parameters,
                            result,
                            &slot.result_reference_regions,
                        ) && slot.result.matches_machine_result(
                            &terminal_psi::TerminalMachineResult::Structural(result.clone()),
                        )
                    }
                } && terminal_semantics::result_claim_paths_match_returns(
                    &slot.result_claim_paths,
                    function
                        .block_nodes()
                        .filter_map(|node| match &node.operation {
                            abstract_operations::AbstractOperation::ReturnStructural {
                                returned_claims,
                                ..
                            } => Some(returned_claims.as_slice()),
                            _ => None,
                        }),
                );
                if !result_matches
                    || crate::unit_validation::operation_contracts::function_entry_claim_paths(
                        function,
                    )
                    .as_ref()
                        != Some(&slot.entry_claim_paths)
                    || function.parameters.len() != slot.scalar_parameters.len()
                    || !function
                        .parameters
                        .iter()
                        .zip(&slot.scalar_parameters)
                        .all(|(actual, formal)| actual.scalar_type == *formal)
                    || function.structural_parameters.len() != slot.structural_parameters.len()
                    || !function
                        .structural_parameters
                        .iter()
                        .zip(&slot.structural_parameters)
                        .all(|(actual, formal)| {
                            let actual = abstract_operations::StructuralCallFormal::from(actual);
                            let formal = abstract_operations::StructuralCallFormal::from(formal);
                            actual.position == formal.position
                                && actual.structural_type == formal.structural_type
                                && actual.multiplicity == formal.multiplicity
                                && actual.access == formal.access
                                && actual.qualifications == formal.qualifications
                                && actual.projected_qualifications
                                    == formal.projected_qualifications
                        })
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    fn validate_structural_field_range_authority(
        unit: &PsiOptimizationUnit,
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Result<(), OptimizationUnitValidationError> {
        use abstract_operations::AbstractOperation as O;
        use terminal_psi::{RecordFieldValue, StructuralTypeShape};
        macro_rules! require_operation {
            ($operation:expr, $obligation:expr, $goal:expr) => {
                require_accepted_goal_fact(
                    unit,
                    function.machine,
                    optimization_unit::PsiProvenance::Operation(*$operation),
                    Some(*$obligation),
                    Some($goal),
                )?
            };
        }

        for node in function.block_nodes() {
            let paths = node
                .successors
                .iter()
                .flat_map(|edge| {
                    edge.structural_bindings
                        .iter()
                        .map(move |binding| (edge.psi_edge, binding.argument.path.as_slice()))
                })
                .chain(
                    node.operation
                        .cleanup_rosters()
                        .flat_map(|(edge, actions)| {
                            actions.iter().map(move |action| (edge, action.path()))
                        }),
                );
            for (edge, path) in paths {
                for (_, obligation) in path
                    .iter()
                    .filter_map(terminal_psi::StructuralPathSegment::runtime_index)
                {
                    require_accepted_goal_fact(
                        unit,
                        function.machine,
                        optimization_unit::PsiProvenance::Edge(edge),
                        Some(obligation),
                        None,
                    )?;
                }
            }
            match &node.operation {
                O::EstablishElementView {
                    psi_operation,
                    source,
                    element,
                    region:
                        region @ terminal_psi::ElementViewRegion::WholeRepresentation {
                            source_length,
                            obligation,
                        },
                    ..
                } => {
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    let root = super::operation_contracts::structural_source_contract(
                        function,
                        source.place,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    let collection = terminal_semantics::runtime_structural_path_tip(
                        types.values().copied(),
                        root.structural_type,
                        &source.path,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    let Some(terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                        source: source_layout,
                        target: target_layout,
                        ..
                    }) = terminal_semantics::element_view_shape_region_geometry(
                        types.values().copied(),
                        &collection.shape,
                        *element,
                        *region,
                        source.access,
                    )
                    else {
                        return Err(invalid);
                    };
                    let factor = source_layout
                        .size
                        .checked_div(target_layout.size)
                        .ok_or_else(|| invalid.clone())?;
                    let goal =
                        terminal_semantics::whole_representation_count_goal(*source_length, factor)
                            .ok_or_else(|| invalid.clone())?;
                    require_operation!(psi_operation, obligation, &goal);
                    let expected = terminal_codec::canonical_proposition_order_key(&goal)
                        .map_err(|_| invalid.clone())?;
                    if unit
                        .accepted_obligation_fact(
                            function.machine,
                            optimization_unit::PsiProvenance::Operation(*psi_operation),
                            Some(*obligation),
                        )
                        .map(|fact| fact.proposition.as_slice())
                        != Some(expected.as_slice())
                    {
                        return Err(invalid);
                    }
                }
                O::EstablishElementView {
                    psi_operation,
                    source,
                    element,
                    region:
                        terminal_psi::ElementViewRegion::RepresentationBytes {
                            start,
                            end,
                            obligation,
                        },
                    ..
                } => {
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    let root = structural_source_contract(function, source.place)
                        .ok_or_else(|| invalid.clone())?;
                    let collection = runtime_structural_path_tip(
                        types.values().copied(),
                        root.structural_type,
                        &source.path,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    let extent = terminal_semantics::representation_byte_extent(
                        types.values().copied(),
                        collection.id,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    let layout = terminal_semantics::representation_element_layout(
                        types.values().copied(),
                        *element,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    let goal = terminal_semantics::representation_region_goal(
                        *start,
                        *end,
                        extent,
                        layout.size,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    require_operation!(psi_operation, obligation, &goal);
                }
                O::ElementViewWrite {
                    psi_operation,
                    destination,
                    path,
                    index,
                    value,
                    length,
                    obligation,
                } => {
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    let root = structural_source_contract(function, destination.place)
                        .ok_or_else(|| invalid.clone())?;
                    let selected = runtime_structural_path_tip(
                        types.values().copied(),
                        root.structural_type,
                        &destination.path,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    let Some(StructuralTypeShape::ElementView { element, .. }) =
                        Some(&selected.shape)
                    else {
                        return Err(invalid);
                    };
                    if destination.access != terminal_psi::StructuralAccess::MutableBorrow
                        || !terminal_semantics::descriptor_source_projection_accessible(
                            types.values().copied(),
                            root.structural_type,
                            root.multiplicity,
                            root.backing_access(types.values().copied())
                                .ok_or_else(|| invalid.clone())?,
                            &destination.path,
                            terminal_psi::StructuralAccess::MutableBorrow,
                        )
                    {
                        return Err(invalid);
                    }
                    let mut written = vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation: *obligation,
                    }];
                    written.extend(path.iter().cloned());
                    if !root.memberships_disjoint_from_write(&written) {
                        return Err(invalid);
                    }
                    let goal = terminal_semantics::element_view_write_goal(
                        &unit.structural_types,
                        *element,
                        path,
                        *index,
                        *length,
                        *value,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    require_operation!(psi_operation, obligation, &goal);
                }
                abstract_operations::byte_sequence_write_pair_pattern!(
                    psi_operation,
                    index,
                    length,
                    obligation,
                    ..
                )
                | O::StructuralByteSequenceFieldRead {
                    psi_operation,
                    index,
                    length,
                    obligation,
                    ..
                } => {
                    use semantic_vocabulary::{IntegerSign, IntegerType, Proposition, ScalarTerm};
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    let integer =
                        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
                    let mut goal = Proposition::LessThan(
                        ScalarTerm::value(*index, ScalarType::Integer(integer)),
                        ScalarTerm::value(*length, ScalarType::Integer(integer)),
                    );
                    if let O::StructuralByteSequenceFieldByteStore {
                        destination,
                        path,
                        field,
                        value,
                        ..
                    } = &node.operation
                    {
                        let predicates = function_structure::byte_field_predicates(
                            unit,
                            function,
                            *destination,
                            path,
                            *field,
                        )
                        .ok_or(invalid.clone())?;
                        goal = indexed_byte_store_goal(goal, *value, predicates)
                            .map_err(|_| invalid.clone())?;
                    }
                    if let O::ByteSequenceWrite {
                        destination, value, ..
                    } = &node.operation
                    {
                        let predicates =
                            function_structure::byte_write_predicates(unit, function, *destination)
                                .ok_or(invalid.clone())?;
                        goal = indexed_byte_store_goal(goal, *value, predicates)
                            .map_err(|_| invalid.clone())?;
                    }
                    require_operation!(psi_operation, obligation, &goal);
                }
                O::EstablishByteSequenceConcatenation {
                    psi_operation,
                    result,
                    left,
                    right,
                    capacity_obligation,
                } => {
                    use semantic_vocabulary::{
                        IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
                    };
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    if !function_structure::concatenation_preserves_predicates(
                        unit,
                        function,
                        &node.operation,
                    ) {
                        return Err(invalid);
                    }
                    let Some(StructuralTypeDeclaration {
                        shape:
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                            ),
                        ..
                    }) = types.get(&result.structural_type).copied()
                    else {
                        return Err(invalid);
                    };
                    let source_type =
                        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
                    let goal = Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::Add(
                            Box::new(IntegerMathTerm::MathValue {
                                source_type,
                                value: left.length(),
                            }),
                            Box::new(IntegerMathTerm::MathValue {
                                source_type,
                                value: right.length(),
                            }),
                        ),
                        IntegerMathTerm::literal(IntegerValue::Unsigned(u128::from(*capacity))),
                    );
                    require_operation!(psi_operation, capacity_obligation, &goal);
                }
                abstract_operations::byte_sequence_store_pair_pattern!(
                    psi_operation,
                    length,
                    obligation,
                    ..
                ) => {
                    use semantic_vocabulary::{
                        IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm,
                    };
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    let capacity = function_structure::byte_field_store_capacity(
                        function,
                        &node.operation,
                        types,
                    )
                    .ok_or(invalid.clone())?;
                    if !function_structure::replacement_preserves_predicates(
                        unit,
                        function,
                        &node.operation,
                    ) {
                        return Err(invalid);
                    }
                    let integer =
                        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid.clone())?;
                    let goal = Proposition::LessOrEqual(
                        ScalarTerm::value(*length, ScalarType::Integer(integer)),
                        ScalarTerm::Integer {
                            scalar_type: integer,
                            value: IntegerValue::Unsigned(u128::from(capacity)),
                        },
                    );
                    require_operation!(psi_operation, obligation, &goal);
                }
                O::StructuralByteViewFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    source,
                    predicate_obligation,
                } => {
                    let invalid =
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
                    let required = match field {
                        Some(field) => function_structure::byte_field_predicates(
                            unit,
                            function,
                            *destination,
                            path,
                            *field,
                        ),
                        None => {
                            function_structure::declared_descriptor_home(function, *destination)
                                .and_then(|home| {
                                    terminal_semantics::byte_qualification_predicates_in(
                                        unit.structural_domains.iter(),
                                        home.structural_type,
                                        &home.qualifications,
                                    )
                                })
                        }
                    }
                    .ok_or_else(|| invalid.clone())?;
                    let established = function_structure::whole_source_predicates(
                        unit.structural_types.iter(),
                        unit.structural_domains.iter(),
                        function,
                        *source,
                    )
                    .ok_or_else(|| invalid.clone())?;
                    if let Some(obligation) = predicate_obligation {
                        if required.is_empty() {
                            return Err(invalid);
                        }
                        let goal = terminal_semantics::byte_predicate_requirement(
                            required,
                            established,
                            *source,
                            &[],
                        );
                        require_operation!(psi_operation, obligation, &goal);
                    } else if !established.implies(required) {
                        return Err(invalid);
                    }
                }
                O::EstablishPrimitiveLocal {
                    psi_operation,
                    result,
                    value,
                    range_obligation,
                } => {
                    let declaration = types.get(&result.structural_type).ok_or(
                        OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                    )?;
                    require_scalar_receiving_fact(
                        unit,
                        function.machine,
                        *psi_operation,
                        value.value,
                        declaration.scalar_restriction.as_ref(),
                        *range_obligation,
                    )?;
                }
                O::WriteOnlyPrimitiveStore {
                    psi_operation,
                    destination,
                    path,
                    value,
                    range_obligation,
                } => {
                    let (whole, projected) = match destination {
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
                    | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { parameter, .. } => (&parameter.qualifications, &parameter.projected_qualifications),
                    abstract_operations::StructuralScalarStoreDestination::OwnedResult { result, .. } => (&result.qualifications, &result.projected_qualifications),
                };
                    let restriction = terminal_semantics::primitive_store_restriction(
                        types.values().copied(),
                        unit.structural_domains.iter(),
                        destination.structural_type(),
                        whole,
                        projected,
                        destination.additional_input_qualifications(),
                        path,
                    )
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                    require_scalar_receiving_fact(
                        unit,
                        function.machine,
                        *psi_operation,
                        value.value,
                        restriction.as_ref(),
                        *range_obligation,
                    )?;
                }
                O::AtomicEvent {
                    psi_operation,
                    event,
                    resident_obligation,
                    ..
                } => {
                    let Some(location) = event.location() else {
                        if resident_obligation.is_some() {
                            return Err(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        );
                        }
                        continue;
                    };
                    let root = optimization_unit::field_evidence(function, location.root)
                        .and_then(|row| row.root_type)
                        .ok_or(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        )?;
                    let cell = terminal_semantics::atomic_cell_type(
                        types.values().copied(),
                        root,
                        &location.path,
                    )
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                    let bounded = cell.bounds().is_some();
                    let requires = bounded && event.joins_modification_order();
                    if requires != resident_obligation.is_some() {
                        return Err(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        );
                    }
                    if requires {
                        let value = event
                        .replacement()
                        .or(match event {
                            abstract_operations::AbstractAtomicEvent::ReadModifyWrite {
                                operand,
                                ..
                            } => Some(*operand),
                            _ => None,
                        })
                        .ok_or(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        )?;
                        let requirement = terminal_semantics::atomic_resident_requirement(
                            &cell.into(),
                            value,
                        )
                        .ok_or(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        )?;
                        canonical_proposition_order_key(&requirement).map_err(|_| {
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch
                        })?;
                        let receiving = SourceCheckedScalarReceiving {
                            value,
                            restriction: cell.into(),
                        };
                        if unit
                            .accepted_obligation_facts
                            .iter()
                            .filter(|fact| {
                                fact.machine == function.machine
                                    && fact.owner
                                        == optimization_unit::PsiProvenance::Operation(
                                            *psi_operation,
                                        )
                                    && Some(fact.obligation) == *resident_obligation
                                    && fact.psi == unit.psi
                                    && fact.has_canonical_identity()
                                    && fact.scalar_receiving.as_ref() == Some(&receiving)
                            })
                            .count()
                            != 1
                        {
                            return Err(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        );
                        }
                    }
                }
                O::StructuralScalarFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    value,
                    range_obligation,
                } => {
                    let (storage_type, storage_path) =
                        if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
                            super::references::record_field_destination(
                        function,
                        types,
                        destination.place(),
                        path,
                        terminal_psi::StructuralAccess::WriteOnlyBorrow,
                    )
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?
                        } else {
                            (destination.structural_type(), path.as_slice())
                        };
                    let declaration = projected_field_declaration(
                        types.values().copied(),
                        storage_type,
                        storage_path,
                        *field,
                    )
                    .ok_or(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)?;
                    if terminal_semantics::scalar_field_requires_evidence(&declaration.field_type)
                        != range_obligation.is_some()
                    {
                        return Err(
                            OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch,
                        );
                    }
                    validate_field_range_authority(
                        unit,
                        function.machine,
                        *psi_operation,
                        declaration,
                        value.value,
                        *range_obligation,
                    )?;
                }
                O::EstablishCase {
                    psi_operation,
                    result,
                    result_case,
                    fields,
                } => {
                    let Some(StructuralTypeDeclaration {
                        shape: StructuralTypeShape::Sum { cases },
                        ..
                    }) = types.get(&result.structural_type).copied()
                    else {
                        continue;
                    };
                    let Some(case) = find_structural_case(cases, *result_case) else {
                        continue;
                    };
                    for (declaration, binding) in case.fields.iter().zip(fields) {
                        if let RecordFieldValue::Scalar {
                            value,
                            range_obligation,
                        } = &binding.value
                        {
                            validate_field_range_authority(
                                unit,
                                function.machine,
                                *psi_operation,
                                declaration,
                                *value,
                                *range_obligation,
                            )?;
                        }
                    }
                }
                O::EstablishRecord {
                    psi_operation,
                    result,
                    fields,
                } => {
                    let Some(StructuralTypeDeclaration {
                        shape:
                            StructuralTypeShape::Record {
                                fields: declarations,
                                ..
                            },
                        ..
                    }) = types.get(&result.structural_type).copied()
                    else {
                        continue;
                    };
                    for (declaration, initializer) in declarations.iter().zip(fields) {
                        if let RecordFieldValue::Scalar {
                            value,
                            range_obligation,
                        } = &initializer.value
                        {
                            validate_field_range_authority(
                                unit,
                                function.machine,
                                *psi_operation,
                                declaration,
                                *value,
                                *range_obligation,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_field_range_authority(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        declaration: &terminal_psi::StructuralFieldDeclaration,
        value: ValueId,
        obligation: Option<ObligationId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        validate_initialization_range_authority(
            unit,
            machine,
            operation,
            &declaration.field_type,
            value,
            obligation,
        )
    }

    fn validate_scalar_range_authority(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        restriction: &StructuralFieldType,
        value: ValueId,
        obligation: Option<ObligationId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        validate_range_fact(
            unit,
            machine,
            operation,
            terminal_semantics::scalar_field_requirement(restriction, value),
            obligation,
        )
    }

    fn validate_initialization_range_authority(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        restriction: &StructuralFieldType,
        value: ValueId,
        obligation: Option<ObligationId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        validate_range_fact(
            unit,
            machine,
            operation,
            terminal_semantics::scalar_field_initialization_requirement(restriction, value),
            obligation,
        )
    }

    fn require_scalar_receiving_fact(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        value: ValueId,
        restriction: Option<&StructuralFieldType>,
        obligation: Option<ObligationId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let invalid = || OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
        if restriction.is_some() != obligation.is_some() {
            return Err(invalid());
        }
        let Some(restriction) = restriction else {
            return Ok(());
        };
        let receiving = SourceCheckedScalarReceiving {
            value,
            restriction: restriction.clone(),
        };
        if unit
            .accepted_obligation_fact(
                machine,
                optimization_unit::PsiProvenance::Operation(operation),
                obligation,
            )
            .and_then(|fact| fact.scalar_receiving.as_ref())
            != Some(&receiving)
        {
            return Err(invalid());
        }
        validate_scalar_range_authority(unit, machine, operation, restriction, value, obligation)
    }

    fn require_accepted_goal_fact(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        provenance: optimization_unit::PsiProvenance,
        obligation: Option<ObligationId>,
        goal: Option<&semantic_vocabulary::Proposition>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let invalid = || OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch;
        if let Some(goal) = goal {
            canonical_proposition_order_key(goal).map_err(|_| invalid())?;
        }
        if unit
            .accepted_obligation_fact(machine, provenance, obligation)
            .is_none()
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn validate_range_fact(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        requirement: Option<semantic_vocabulary::Proposition>,
        obligation: Option<ObligationId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let Some(requirement) = requirement else {
            return Ok(());
        };
        require_accepted_goal_fact(
            unit,
            machine,
            optimization_unit::PsiProvenance::Operation(operation),
            obligation,
            Some(&requirement),
        )
    }

    pub(super) fn validate_final_authorities(
        unit: &PsiOptimizationUnit,
        indexes: &UnitIndexes<'_>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let UnitIndexes {
            machines,
            boundary_machines,
            services,
            pruned,
            ..
        } = indexes;
        for fact in &unit.ownership_frontier_facts {
            if unit.function(fact.machine).is_none() && !pruned.contains(&fact.machine) {
                return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
            }
        }
        if !machines.contains_key(&unit.entry) {
            return Err(OptimizationUnitValidationError::MissingEntryMachine(
                unit.entry,
            ));
        }
        validate_root_service_reach(unit, machines, boundary_machines, services)?;
        Ok(())
    }
}
pub(crate) mod derived_metadata {
    use crate::OptimizationUnitValidationError;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::ClaimId;

    mod control_flow {
        pub(crate) fn is_terminator(operation: &abstract_operations::AbstractOperation) -> bool {
            use abstract_operations::AbstractOperation as O;
            matches!(
                operation,
                abstract_operations::jump_or_conditional_pattern!()
                    | O::StructuralCase { .. }
                    | abstract_operations::return_or_return_unit_pattern!()
                    | abstract_operations::return_structural_or_crash_pattern!()
                    | O::InvokeNoNormal { .. }
                    | O::ExternalCompletion { .. }
            )
        }
    }
    mod edges {
        use abstract_operations::AbstractOperation;
        use optimization_unit::{OptimizationEdge, PsiProvenance};

        pub(crate) fn successors_match_operation(
            operation: &AbstractOperation,
            actual: &[OptimizationEdge],
        ) -> bool {
            let expected = expected_edges(operation);
            actual.len() == expected.len()
                && actual.iter().zip(expected).all(|(actual, expected)| {
                    actual.psi_edge == expected.psi_edge
                        && actual.target == expected.target
                        && actual.bindings == expected.bindings
                        && actual.structural_bindings == expected.structural_bindings
                        && actual.cleanup_actions == expected.cleanup_actions
                        && actual.provenance.first() == Some(&PsiProvenance::Edge(actual.psi_edge))
                        && actual
                            .provenance
                            .iter()
                            .all(|source| matches!(source, PsiProvenance::Edge(_)))
                })
        }

        pub(crate) fn expected_edges(operation: &AbstractOperation) -> Vec<OptimizationEdge> {
            use abstract_operations::AbstractOperation as O;
            match operation {
                O::Jump {
                    psi_edge,
                    target,
                    bindings,
                    structural_bindings,
                    cleanup_actions,
                } => vec![OptimizationEdge {
                    structural_bindings: structural_bindings.clone(),
                    psi_edge: *psi_edge,
                    target: *target,
                    bindings: bindings.clone(),
                    cleanup_actions: cleanup_actions.clone(),
                    provenance: vec![PsiProvenance::Edge(*psi_edge)],
                    work: vec![optimization_unit::WorkSettlement {
                        site: PsiProvenance::Edge(*psi_edge),
                        units: 1,
                    }],
                }],
                O::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [when_true, when_false]
                    .into_iter()
                    .map(|edge| OptimizationEdge {
                        structural_bindings: edge.structural_bindings.clone(),
                        psi_edge: edge.psi_edge,
                        target: edge.target,
                        bindings: edge.bindings.clone(),
                        cleanup_actions: edge.cleanup_actions.clone(),
                        provenance: vec![PsiProvenance::Edge(edge.psi_edge)],
                        work: vec![optimization_unit::WorkSettlement {
                            site: PsiProvenance::Edge(edge.psi_edge),
                            units: 1,
                        }],
                    })
                    .collect(),
                O::StructuralCase { cases, .. } => cases
                    .iter()
                    .map(|case| OptimizationEdge {
                        psi_edge: case.psi_edge,
                        target: case.target,
                        bindings: Vec::new(),
                        structural_bindings: case.structural_bindings.clone(),
                        cleanup_actions: case.cleanup_actions.clone(),
                        provenance: vec![PsiProvenance::Edge(case.psi_edge)],
                        work: vec![optimization_unit::WorkSettlement {
                            site: PsiProvenance::Edge(case.psi_edge),
                            units: 1,
                        }],
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
    }
    mod places {
        use crate::OptimizationUnitValidationError;
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractStructuralCallResult;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{ClaimId, MachineId, PlaceId};
        use std::collections::BTreeSet;

        pub(crate) fn function_has_claim(
            function: &PsiOptimizationFunction,
            claim: ClaimId,
        ) -> bool {
            function.entry_claims.contains(&claim)
                || function
                    .block_entry_claims
                    .iter()
                    .any(|binding| binding.claim == claim)
                || function
                    .content_entry_claims
                    .iter()
                    .any(|candidate| candidate.claim == claim)
                || function
                    .block_nodes()
                    .filter_map(|node| node.operation.structural_result())
                    .any(|(_, result)| result.claims.iter().any(|binding| binding.claim == claim))
        }

        pub(crate) fn reconstruct_declared_places(
            function: &PsiOptimizationFunction,
        ) -> Result<BTreeSet<PlaceId>, OptimizationUnitValidationError> {
            let mut known_places = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .map(|parameter| parameter.place)
                .chain(
                    function
                        .entry_claim_declarations
                        .iter()
                        .map(|claim| claim.input),
                )
                .chain(function.result.structural().map(|result| result.place))
                .collect::<BTreeSet<_>>();
            for node in function.block_nodes() {
                match &node.operation {
                    abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
                        known_places.insert(place.id);
                    }
                    abstract_operations::establish_or_subslice_set_pattern!(result, ..)
                    | abstract_operations::establish_case_or_record_result_pattern!(result)
                    | O::EstablishNativeCodeLayout { result, .. }
                    | O::EstablishActivationClaim { result, .. }
                    | O::EstablishZeroedStructural { result, .. }
                    | O::EstablishStructuralLocal { result, .. }
                    | O::EstablishReference { result, .. }
                    | O::EstablishAccessReference { result, .. }
                    | O::MoveStructuralField { result, .. }
                    | O::StructuralLeafCopy { result, .. }
                    | O::MoveStructuralValue { result, .. }
                    | O::TransformContent { result, .. }
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { result, .. },
                        ..
                    }
                    | O::BoundaryCall {
                        result: abstract_operations::AbstractBoundaryResult::Structural(result),
                        ..
                    } => {
                        known_places.insert(result.place);
                    }
                    O::AtomicEvent { event, .. } => {
                        if let Some(place) = event.place() {
                            known_places.insert(place);
                        }
                        if let abstract_operations::AbstractAtomicEvent::CompareExchangeOnce {
                            outcome,
                            ..
                        } = event
                        {
                            known_places.insert(outcome.place);
                        }
                    }
                    _ => {}
                }
            }
            for node in function.block_nodes() {
                validate_operation_places(function.machine, &node.operation, &known_places)?;
            }
            Ok(known_places)
        }

        pub(crate) fn validate_operation_places(
            machine: MachineId,
            operation: &abstract_operations::AbstractOperation,
            known: &BTreeSet<PlaceId>,
        ) -> Result<(), OptimizationUnitValidationError> {
            use abstract_operations::AbstractOperation as O;
            let require = |place: PlaceId, known: &BTreeSet<PlaceId>| {
                if known.contains(&place) {
                    Ok(())
                } else {
                    Err(OptimizationUnitValidationError::UnknownPlace { machine, place })
                }
            };
            for (_, actions) in operation.cleanup_rosters() {
                for action in actions {
                    require(action.place(), known)?;
                }
            }
            match operation {
                O::EstablishByteSequenceConcatenation { left, right, .. } => {
                    require(left.source(), known)?;
                    require(right.source(), known)?;
                }
                O::EstablishStructuralArray { elements, .. } => {
                    for argument in elements {
                        require(argument.place, known)?;
                    }
                }
                abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
                    for initializer in fields {
                        if let terminal_psi::RecordFieldValue::Structural(argument) =
                            &initializer.value
                        {
                            require(argument.place, known)?;
                        }
                    }
                }
                O::StructuralCase { source, cases, .. } => {
                    require(*source, known)?;
                    for binding in cases.iter().flat_map(|case| &case.structural_bindings) {
                        require(binding.parameter, known)?;
                        require(binding.argument.place, known)?;
                    }
                }
                O::Jump {
                    structural_bindings,
                    ..
                } => {
                    for binding in structural_bindings {
                        require(binding.parameter, known)?;
                        require(binding.argument.place, known)?;
                    }
                }
                O::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for binding in when_true
                        .structural_bindings
                        .iter()
                        .chain(&when_false.structural_bindings)
                    {
                        require(binding.parameter, known)?;
                        require(binding.argument.place, known)?;
                    }
                }
                abstract_operations::byte_literal_establish_pair_pattern!(..) => {}
                O::PrimitiveLocalStore { destination, .. } => require(*destination, known)?,
                O::EstablishReference { source, .. }
                | O::EstablishAccessReference { source, .. } => require(source.place, known)?,
                O::MoveStructuralValue { source, .. } => require(source.place, known)?,
                O::TransformContent {
                    source, parents, ..
                } => {
                    for argument in std::iter::once(source).chain(parents) {
                        require(argument.place, known)?;
                    }
                }
                O::ReleaseActivationClaim { source, .. } => require(source.place, known)?,
                O::ReleaseReference { source, .. } => require(*source, known)?,
                O::WriteOnlyPrimitiveStore { destination, .. } => {
                    require(destination.place(), known)?;
                }
                O::StructuralScalarFieldStore { destination, .. } => {
                    require(destination.place(), known)?;
                }
                O::MoveStructuralField { source, .. } => {
                    require(source.place(), known)?;
                }
                O::StructuralLeafCopy { source, .. } => {
                    require(*source, known)?;
                }
                O::StoreStructuralField {
                    destination, value, ..
                } => {
                    require(destination.place(), known)?;
                    require(value.place, known)?;
                }
                abstract_operations::boundary_or_structural_call_pair_pattern!(
                    structural_arguments,
                    ..
                ) => {
                    for argument in structural_arguments {
                        require(argument.place, known)?;
                    }
                }
                abstract_operations::byte_sequence_store_pair_pattern!(destination, source, ..) => {
                    require(*destination, known)?;
                    require(*source, known)?;
                }
                abstract_operations::primitive_storage_observation_pattern!(source, ..)
                | O::ByteSequenceRead { source, .. }
                | abstract_operations::byte_sequence_write_pair_pattern!(destination: source, ..)
                | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
                | O::StructuralScalarFieldRead { source, .. } => {
                    require(*source, known)?;
                }
                O::ByteSequenceLength { source, .. }
                | O::ElementViewLength { source, .. }
                | O::ByteSequenceSubslice { source, .. }
                | O::ElementViewSubslice { source, .. } => require(source.place, known)?,
                O::ReturnStructural { source, .. } => require(*source, known)?,
                _ => {}
            }
            Ok(())
        }
    }
    mod provenance {
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractOperation;
        use abstract_operations::jump_or_conditional_pattern;
        use optimization_unit::PsiProvenance;

        pub(crate) fn expected_provenance(operation: &AbstractOperation) -> Vec<PsiProvenance> {
            match operation {
                jump_or_conditional_pattern!() | O::StructuralCase { .. } => Vec::new(),
                abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
                | O::InvokeNoNormal { psi_edge, .. }
                | O::ExternalCompletion { psi_edge, .. }
                | O::Crash { psi_edge, .. } => {
                    vec![PsiProvenance::Edge(*psi_edge)]
                }
                O::ReturnStructural {
                    psi_edge,
                    trivial_affine_locals,
                    ..
                } => std::iter::once(PsiProvenance::Edge(*psi_edge))
                    .chain(
                        trivial_affine_locals
                            .iter()
                            .map(|(operation, _, _)| PsiProvenance::Operation(*operation)),
                    )
                    .collect(),
                _ => {
                    vec![PsiProvenance::Operation(operation.psi_operation().expect(
                        "non-control operation names its authored psi operation",
                    ))]
                }
            }
        }

        pub(crate) fn provenance_matches_operation(
            operation: &AbstractOperation,
            provenance: &[PsiProvenance],
        ) -> bool {
            let expected = expected_provenance(operation);
            if expected.is_empty() {
                match operation {
                    jump_or_conditional_pattern!() | O::StructuralCase { .. } => true,
                    _ => provenance.is_empty(),
                }
            } else {
                provenance.starts_with(&expected)
            }
        }
    }
    pub(crate) use crate::operation_metadata::{
        expected_definitions, expected_ownership, expected_uses,
    };
    pub(crate) use control_flow::is_terminator;
    pub(crate) use edges::{expected_edges, successors_match_operation};
    pub(crate) use optimization_unit::dominators;
    pub(crate) use places::{function_has_claim, reconstruct_declared_places};
    pub(crate) use provenance::{expected_provenance, provenance_matches_operation};

    pub(crate) fn validate_places_and_claims(
        function: &PsiOptimizationFunction,
    ) -> Result<(), OptimizationUnitValidationError> {
        let known_places = reconstruct_declared_places(function)?;
        for parameter in &function.structural_parameters {
            if !function.declared_places.contains(&parameter.place) {
                return Err(OptimizationUnitValidationError::UnknownPlace {
                    machine: function.machine,
                    place: parameter.place,
                });
            }
        }
        for node in function.block_nodes() {
            for event in &node.ownership {
                let claims: &[ClaimId] = match event {
                    optimization_unit::OwnershipEvent::ClaimTransfer(claims)
                    | optimization_unit::OwnershipEvent::StructuralReturn(claims)
                    | optimization_unit::OwnershipEvent::CrashFrontier(claims) => claims,
                    optimization_unit::OwnershipEvent::ExternalCompletionFrontier(claims) => claims,
                    optimization_unit::OwnershipEvent::Cleanup(_) => continue,
                    optimization_unit::OwnershipEvent::ClaimCompletion(receipts) => {
                        for receipt in receipts {
                            if !function_has_claim(function, receipt.claim) {
                                return Err(OptimizationUnitValidationError::UnknownClaim {
                                    machine: function.machine,
                                    claim: receipt.claim,
                                });
                            }
                        }
                        continue;
                    }
                };
                for claim in claims {
                    if !function_has_claim(function, *claim) {
                        return Err(OptimizationUnitValidationError::UnknownClaim {
                            machine: function.machine,
                            claim: *claim,
                        });
                    }
                }
            }
        }
        if known_places != function.declared_places {
            let place = known_places
                .symmetric_difference(&function.declared_places)
                .next()
                .copied()
                .expect("different place sets have a difference");
            return Err(OptimizationUnitValidationError::UnknownPlace {
                machine: function.machine,
                place,
            });
        }
        Ok(())
    }
}
pub(crate) mod function_structure {
    use crate::unit_validation::catalog_mismatch;
    use crate::unit_validation::derived_metadata::validate_places_and_claims;
    use crate::unit_validation::operation_contracts::validate_values_and_bindings;
    use crate::unit_validation::services::valid_service_ceiling;
    use crate::unit_validation::structural_catalog::{
        validate_byte_sequence_literal_witnesses, validate_function_structural_catalog,
        validate_provider_attachment_specialization, validate_trivial_affine_local_witnesses,
    };
    use crate::{OptimizationUnitValidationError, current_ownership};
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use semantic_vocabulary::{
        BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
    };
    use std::collections::BTreeMap;

    mod byte_concatenation {
        use super::byte_field_freshness::field_path;
        use crate::OptimizationUnitValidationError;
        use crate::unit_validation::catalog_mismatch;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
        use semantic_vocabulary::{BlockId, StructuralDomainId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::control_flow::ByteSequenceOperand;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralDomainDeclaration,
            StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
        };

        pub(super) fn validate(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let invalid = || catalog_mismatch(function);
            for (block_position, block) in function.blocks.iter().enumerate() {
                for (node_position, node) in block.nodes.iter().enumerate() {
                    let O::EstablishByteSequenceConcatenation {
                        psi_operation,
                        result,
                        left,
                        right,
                        ..
                    } = &node.operation
                    else {
                        continue;
                    };
                    if result.multiplicity != StructuralMultiplicity::Unrestricted
                        || !result.projected_qualifications.is_empty()
                        || !result.qualification_establishments.is_empty()
                        || !result.claims.is_empty()
                        || !types
                            .get(&result.structural_type)
                            .is_some_and(|declaration| {
                                declaration.shape.is_bounded_owned_byte_sequence()
                            })
                        || !operation_result_placement(function, result, *psi_operation)
                    {
                        return Err(invalid());
                    }
                    for operand in [left, right] {
                        if operand.source() == result.place
                            || function.entry_claim_declared(operand.source())
                            || function.content_entry_claim_declared(operand.source())
                        {
                            return Err(invalid());
                        }
                        match operand {
                            ByteSequenceOperand::Whole { source, length } => {
                                if !super::descriptor_sources::valid_whole(
                                    function, types, domains, source, *length,
                                ) {
                                    return Err(invalid());
                                }
                            }
                            ByteSequenceOperand::Field {
                                source,
                                field,
                                length,
                            } => {
                                let (structural_type, access) =
                                    super::byte_field_freshness::byte_field_root(
                                        function,
                                        source.place,
                                    )
                                    .ok_or_else(invalid)?;
                                if source.access != StructuralAccess::SharedBorrow
                                    || access == StructuralAccess::WriteOnlyBorrow
                                    || !terminal_psi::is_static_structural_scalar_store_path(
                                        &source.path,
                                    )
                                {
                                    return Err(invalid());
                                }
                                let parent =
                                    super::super::structural_catalog::resolve_structural_path(
                                        types,
                                        structural_type,
                                        &source.path,
                                    )
                                    .ok_or_else(invalid)?;
                                let StructuralTypeShape::Record { fields, .. } =
                                    &types.get(&parent).ok_or_else(invalid)?.shape
                                else {
                                    return Err(invalid());
                                };
                                if !fields.iter().any(|declaration| {
                                    declaration.id == *field
                                        && !declaration.relevance.is_erased()
                                        && matches!(
                                            declaration.field_type.byte_sequence_carrier(),
                                            Some(ByteSequenceCarrier::BoundedOwned { .. })
                                        )
                                }) {
                                    return Err(invalid());
                                }
                                let exact =
                                    field_path(function, types, source.place, &source.path, *field)
                                        .ok_or_else(invalid)?;
                                let names = crate::unit_validation::arrival_storage::storage_names(
                                    function,
                                    block.id,
                                    source.place,
                                );
                                let (producer, producer_root) = function.blocks.iter().enumerate().find_map(|(owner, block)| names.in_block(block.id).find_map(|name| block.nodes.iter().position(|candidate| matches!(&candidate.operation,
                            O::StructuralByteSequenceFieldLength { source: measured, path, field: measured_field, result, .. }
                            if *measured == name && *path == source.path && measured_field == field && result.value == *length)).map(|position| ((owner, position), name)))).ok_or_else(invalid)?;
                                if !super::structural_freshness::unchanged_since(
                                    function,
                                    predecessors,
                                    producer,
                                    producer_root,
                                    (block_position, node_position),
                                    source.place,
                                    |candidate, root| {
                                        super::structural_freshness::structural_metadata_may_change(
                                            function, types, root, &exact, candidate,
                                        ) || matches!(candidate, O::StructuralByteSequenceFieldByteStore { destination, path, field, .. }
                                    if *destination == root && field_path(function, types, *destination, path, *field).is_none_or(|written| exact.starts_with(&written) || written.starts_with(&exact)))
                                    },
                                ) {
                                    return Err(invalid());
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    }
    mod byte_field_freshness {
        use super::super::structural_catalog::resolve_structural_path;
        use super::structural_freshness::{structural_metadata_may_change, unchanged_since};
        use crate::unit_validation::catalog_mismatch;
        use crate::{OperationSite, OptimizationUnitValidationError};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{
            StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
            StructuralTypeShape, live_field_by_id,
        };

        pub(super) fn validate(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        ) -> Result<(), OptimizationUnitValidationError> {
            for (block_position, block) in function.blocks.iter().enumerate() {
                for (node_position, node) in block.nodes.iter().enumerate() {
                    let (destination, path, field, length, reads) = match &node.operation {
                        O::StructuralByteSequenceFieldByteStore {
                            destination,
                            path,
                            field,
                            length,
                            ..
                        } => (destination, path, field, length, false),
                        O::StructuralByteSequenceFieldRead {
                            source,
                            path,
                            field,
                            length,
                            ..
                        } => (source, path, field, length, true),
                        _ => continue,
                    };
                    let node =
                        u32::try_from(node_position).map_err(|_| catalog_mismatch(function))?;
                    let invalid = if reads {
                        OptimizationUnitValidationError::InvalidByteSequenceRead(
                            OperationSite::new(function.machine, block.id, node),
                        )
                    } else {
                        OptimizationUnitValidationError::InvalidByteSequenceWrite(
                            OperationSite::new(function.machine, block.id, node),
                        )
                    };
                    let exact_path = field_path(function, types, *destination, path, *field)
                        .ok_or_else(|| invalid.clone())?;
                    let names = crate::unit_validation::arrival_storage::storage_names(
                        function,
                        block.id,
                        *destination,
                    );
                    let (producer, producer_root) = function.blocks.iter().enumerate().find_map(|(owner, block)| {
                names.in_block(block.id).find_map(|name| {
                    block.nodes.iter().position(|candidate| matches!(&candidate.operation,
                        O::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
                        if *source == name && terminal_semantics::structural_paths_select_same_location(measured_path, path) && measured_field == field && result.value == *length
                    )).map(|position| ((owner, position), name))
                })
            }).ok_or_else(|| invalid.clone())?;
                    if !unchanged_since(
                        function,
                        predecessors,
                        producer,
                        producer_root,
                        (block_position, node_position),
                        *destination,
                        |operation, root| {
                            structural_metadata_may_change(
                                function,
                                types,
                                root,
                                &exact_path,
                                operation,
                            )
                        },
                    ) {
                        return Err(invalid);
                    }
                }
            }
            Ok(())
        }

        pub(super) fn field_carrier(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            for (position, segment) in path.iter().enumerate() {
                if matches!(segment, StructuralPathSegment::FixedIndex(_)) {
                    let parent = resolve_structural_path(types, root, &path[..position])?;
                    if matches!(
                        types.get(&parent)?.shape,
                        StructuralTypeShape::ElementView { .. }
                    ) {
                        return None;
                    }
                }
            }
            resolve_structural_path(types, root, path)
        }

        pub(super) fn field_path(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: PlaceId,
            path: &[StructuralPathSegment],
            field: semantic_vocabulary::StructuralFieldId,
        ) -> Option<Vec<StructuralPathSegment>> {
            let (structural_type, _) = byte_field_root(function, root)?;
            let (storage_type, storage_path) =
                if matches!(path.first(), Some(StructuralPathSegment::Referent)) {
                    super::super::references::record_field_destination(
                        function,
                        types,
                        root,
                        path,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    )?
                } else {
                    (structural_type, path)
                };
            let carrier = field_carrier(types, storage_type, storage_path)?;
            let StructuralTypeShape::Record { fields, .. } = &types.get(&carrier)?.shape else {
                return None;
            };
            let selected = live_field_by_id(fields, field)?;
            let mut exact = path.to_vec();
            exact.push(StructuralPathSegment::Field(selected.identity));
            Some(exact)
        }

        pub fn byte_field_root(
            function: &PsiOptimizationFunction,
            root: PlaceId,
        ) -> Option<(StructuralTypeId, StructuralAccess)> {
            use semantic_vocabulary::StructuralPlaceKind;

            let place = function.structural_place(root)?;
            if !matches!(
                place.kind,
                StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::BlockParameter { .. }
                    | StructuralPlaceKind::OperationResult { .. }
            ) || function.entry_claim_declared(root)
                || function.content_entry_claim_declared(root)
            {
                return None;
            }
            let source = crate::unit_validation::operation_contracts::structural_source_contract(
                function, root,
            )?;
            (matches!(
                source.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ))
            .then_some((source.structural_type, source.access))
        }

        pub(super) fn overlaps(
            left: &[StructuralPathSegment],
            right: &[StructuralPathSegment],
        ) -> bool {
            terminal_semantics::structural_paths_may_overlap(left, right)
        }
    }
    mod case_arguments {
        use crate::unit_validation::catalog_mismatch;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, PlaceId, StructuralCaseId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

        use crate::OptimizationUnitValidationError;

        pub(super) fn validate(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        ) -> Result<(), OptimizationUnitValidationError> {
            for (block_position, block) in function.blocks.iter().enumerate() {
                for (node_position, node) in block.nodes.iter().enumerate() {
                    let validate_projection = |root, path: &[StructuralPathSegment]| {
                        for (position, segment) in path.iter().enumerate() {
                            let StructuralPathSegment::Case(case) = segment else {
                                continue;
                            };
                            if !active_on_every_path(
                                function,
                                types,
                                predecessors,
                                (block_position, node_position),
                                root,
                                &path[..position],
                                *case,
                            ) {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        Ok(())
                    };
                    match &node.operation {
                        O::CallStructural {
                            structural_arguments,
                            ..
                        } => {
                            for argument in structural_arguments {
                                validate_projection(argument.place, &argument.path)?;
                            }
                        }
                        O::StructuralCase { source, path, .. } => {
                            validate_projection(*source, path)?;
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        }

        fn active_on_every_path(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            use_site: (usize, usize),
            root: PlaceId,
            path: &[StructuralPathSegment],
            case: StructuralCaseId,
        ) -> bool {
            if membership_on_every_path(
                function,
                types,
                predecessors,
                use_site,
                root,
                path,
                case,
                true,
            ) {
                return true;
            }
            let Some(source) =
                crate::unit_validation::operation_contracts::structural_source_contract(
                    function, root,
                )
            else {
                return false;
            };
            let Some(declaration) = terminal_semantics::runtime_structural_path_tip(
                types.values().copied(),
                source.structural_type,
                path,
            ) else {
                return false;
            };
            let (terminal_psi::StructuralTypeShape::Sum { cases }
            | terminal_psi::StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
            else {
                return false;
            };
            cases.iter().any(|candidate| candidate.id == case)
                && cases
                    .iter()
                    .filter(|candidate| candidate.id != case)
                    .all(|candidate| {
                        membership_on_every_path(
                            function,
                            types,
                            predecessors,
                            use_site,
                            root,
                            path,
                            candidate.id,
                            false,
                        )
                    })
        }

        pub(crate) fn membership_on_every_path(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            use_site: (usize, usize),
            root: PlaceId,
            path: &[StructuralPathSegment],
            case: StructuralCaseId,
            expected: bool,
        ) -> bool {
            let invalidates = |operation: &O, root| {
                super::structural_freshness::structural_metadata_may_change(
                    function, types, root, path, operation,
                )
            };
            super::structural_freshness::established_on_every_path_with_entry(
                function,
                predecessors,
                use_site,
                root,
                |_, operation, root| {
                    matches!(operation, O::EstablishCase { result, result_case, .. }
            if result.place == root && path.is_empty() && (*result_case == case) == expected)
                },
                |site, operation, edge, before| match operation {
                    O::StructuralCase {
                        source,
                        path: selected_path,
                        cases,
                    } => {
                        *source == before
                            && path == selected_path
                            && cases.iter().any(|candidate| {
                                candidate.psi_edge == edge.psi_edge
                                    && (candidate.case == case) == expected
                            })
                    }
                    O::Conditional {
                        condition,
                        when_true,
                        when_false,
                    } => {
                        let truth = if when_true.psi_edge == edge.psi_edge {
                            true
                        } else if when_false.psi_edge == edge.psi_edge {
                            false
                        } else {
                            return false;
                        };
                        function
                            .blocks
                            .iter()
                            .enumerate()
                            .any(|(observed_block, block)| {
                                block
                                    .nodes
                                    .iter()
                                    .enumerate()
                                    .any(|(observed_node, observed)| {
                                        let O::StructuralCaseMembership {
                                            result,
                                            source,
                                            path: observed_path,
                                            case: observed_case,
                                            ..
                                        } = &observed.operation
                                        else {
                                            return false;
                                        };
                                        if result.value != *condition
                                            || *source != before
                                            || observed_path != path
                                        {
                                            return false;
                                        }
                                        let matches_case = if truth {
                                            (*observed_case == case) == expected
                                        } else {
                                            *observed_case == case && !expected
                                        };
                                        matches_case
                                    && super::structural_freshness::established_on_every_path(
                                        function,
                                        predecessors,
                                        site,
                                        before,
                                        |position, _, current_root| {
                                            position == (observed_block, observed_node)
                                                && current_root == before
                                        },
                                        invalidates,
                                    )
                                    })
                            })
                    }
                    _ => false,
                },
                |root| entry_membership(function, types, root, path, case, expected),
                invalidates,
            )
        }

        fn entry_membership(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: PlaceId,
            path: &[StructuralPathSegment],
            case: StructuralCaseId,
            expected: bool,
        ) -> bool {
            use semantic_vocabulary::Proposition;
            let Some(contract) = &function.verified_contract else {
                return false;
            };
            let Some(parameter) = function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == root)
            else {
                return false;
            };
            let matches_subject = |subject: &semantic_vocabulary::StructuralCaseSubject| {
                subject.root() == root
                    && terminal_semantics::canonical_structural_storage_path(
                        types.values().copied(),
                        parameter.structural_type,
                        subject.path(),
                    )
                    .as_deref()
                        == Some(path)
            };
            fn entails(
                proposition: &Proposition,
                matches_subject: &impl Fn(&semantic_vocabulary::StructuralCaseSubject) -> bool,
                case: StructuralCaseId,
                expected: bool,
            ) -> bool {
                match proposition {
                    Proposition::StructuralCaseMembership {
                        subject,
                        case: selected,
                    } => matches_subject(subject) && (*selected == case) == expected,
                    Proposition::Conjunction(children) => children
                        .iter()
                        .any(|child| entails(child, matches_subject, case, expected)),
                    Proposition::Disjunction(children) => {
                        !children.is_empty()
                            && children
                                .iter()
                                .all(|child| entails(child, matches_subject, case, expected))
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } if matches!(conclusion.as_ref(), Proposition::Falsehood) => {
                        !expected
                            && matches!(premise.as_ref(),
                Proposition::StructuralCaseMembership { subject, case: excluded }
                    if *excluded == case && matches_subject(subject))
                    }
                    _ => false,
                }
            }
            contract
                .requires
                .iter()
                .any(|proposition| entails(proposition, &matches_subject, case, expected))
        }
    }
    pub(crate) use case_arguments::membership_on_every_path;
    mod structural_freshness {
        use super::byte_field_freshness::{field_path, overlaps};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

        fn call_argument_changes(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: PlaceId,
            observed: &[StructuralPathSegment],
            argument: &terminal_psi::StructuralArgument,
        ) -> bool {
            use semantic_vocabulary::StructuralPlaceKind;

            let may_alias = |identity, region: &[StructuralPathSegment]| {
                terminal_semantics::runtime_structural_path_tip(
                    types.values().copied(),
                    identity,
                    region,
                )
                .is_none_or(|selected| {
                    !crate::current_ownership::storage_cannot_retain_loan(
                        types,
                        selected.id,
                        &mut BTreeMap::new(),
                    )
                })
            };
            let mut place = argument.place;
            let mut region = argument.path.clone();
            let mut visited = BTreeSet::new();
            loop {
                if region
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                {
                    return true;
                }
                if place == root {
                    return terminal_semantics::structural_paths_may_overlap(observed, &region);
                }
                if !visited.insert(place)
                    || function
                        .block_structural_parameters()
                        .any(|parameter| parameter.place == place || parameter.place == root)
                {
                    return true;
                }
                let Some(declaration) = function.structural_place(place) else {
                    return true;
                };
                let Some(contract) =
                    crate::unit_validation::operation_contracts::structural_source_contract(
                        function, place,
                    )
                else {
                    return true;
                };
                let StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = declaration.kind
                else {
                    return may_alias(contract.structural_type, &region);
                };
                if contract.structural_type != structural_type {
                    return true;
                }
                let Some(operation) = function
                    .block_nodes()
                    .map(|node| &node.operation)
                    .find(|operation| operation.psi_operation() == Some(producer))
                else {
                    return true;
                };
                match operation {
                    O::EstablishElementView {
                        source,
                        result,
                        destination,
                        ..
                    } if result.place == place
                        && *destination == place
                        && result.structural_type == contract.structural_type =>
                    {
                        let mut backing = source.path.clone();
                        if let Some(range) = backing.iter().position(|segment| {
                            matches!(segment, StructuralPathSegment::FixedRange { .. })
                        }) {
                            backing.truncate(range);
                        } else {
                            backing.extend(region);
                        }
                        region = backing;
                        place = source.place;
                    }
                    abstract_operations::element_view_or_byte_subslice_pattern!(
                        source,
                        result,
                        ..
                    ) if result.place == place
                        && result.structural_type == contract.structural_type =>
                    {
                        region = source.path.clone();
                        place = source.place;
                    }
                    _ => {
                        return may_alias(structural_type, &region);
                    }
                }
            }
        }

        pub(super) fn structural_metadata_may_change(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: PlaceId,
            path: &[StructuralPathSegment],
            operation: &O,
        ) -> bool {
            match operation {
                O::ByteSequenceStore { destination, .. } => *destination == root,
                O::StructuralByteSequenceFieldStore {
                    destination,
                    path: written,
                    field,
                    ..
                } => {
                    *destination == root
                        && field_path(function, types, *destination, written, *field)
                            .is_none_or(|written| overlaps(path, &written))
                }
                O::WriteOnlyPrimitiveStore {
                    destination,
                    path: written,
                    ..
                } => {
                    destination.place() == root
                        && terminal_semantics::structural_paths_may_overlap(path, written)
                }
                O::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments
                    .iter()
                    .any(|argument| call_argument_changes(function, types, root, path, argument)),
                O::CallStructural {
                    structural_arguments,
                    ..
                } => structural_arguments.iter().any(|argument| {
                    matches!(
                        argument.access,
                        StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                            | StructuralAccess::Owned
                    ) && call_argument_changes(function, types, root, path, argument)
                }),
                O::StructuralByteSequenceFieldByteStore { .. }
                | abstract_operations::byte_len_or_field_read_pattern!(..)
                | O::StructuralScalarFieldStore { .. }
                | O::ByteSequenceWrite { .. }
                | abstract_operations::byte_read_subslice_triple_pattern!(..)
                | abstract_operations::establish_byte_literal_triple_pattern!(..)
                | O::EstablishPrimitiveLocal { .. }
                | O::PrimitiveLocalStore { .. }
                | O::PrimitiveScalarRead { .. }
                | O::EstablishScalarArray { .. }
                | O::EstablishStructuralArray { .. }
                | abstract_operations::establish_record_or_case_pattern!(..)
                | O::StructuralCaseMembership { .. }
                | O::StructuralScalarFieldRead { .. }
                | O::IntegerConstant { .. }
                | O::IeeeFloatConstant { .. }
                | O::NearestIeeeFloatFusedMultiplyAdd { .. }
                | O::IeeeFloatCompare { .. }
                | O::FloatResultPolicy { .. }
                | O::BooleanConstant { .. }
                | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
                | abstract_operations::integer_equal_less_pair_pattern!(..)
                | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
                | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
                | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
                | abstract_operations::bitwise_or_xor_pair_pattern!(..)
                | abstract_operations::integer_dispatch_triple_pattern!(..)
                | O::Call { .. }
                | abstract_operations::jump_or_conditional_pattern!()
                | O::StructuralCase { .. }
                | abstract_operations::return_or_return_unit_pattern!()
                | abstract_operations::return_structural_or_crash_pattern!() => false,
                _ => true,
            }
        }

        pub(super) fn unchanged_since(
            function: &PsiOptimizationFunction,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            producer: (usize, usize),
            producer_root: PlaceId,
            use_site: (usize, usize),
            root: PlaceId,
            invalidates: impl Fn(&O, PlaceId) -> bool,
        ) -> bool {
            established_on_every_path(
                function,
                predecessors,
                use_site,
                root,
                |position, _, root| position == producer && root == producer_root,
                invalidates,
            )
        }

        pub(super) fn established_on_every_path(
            function: &PsiOptimizationFunction,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            use_site: (usize, usize),
            root: PlaceId,
            establishes: impl Fn((usize, usize), &O, PlaceId) -> bool,
            invalidates: impl Fn(&O, PlaceId) -> bool,
        ) -> bool {
            established_on_every_path_with_edges(
                function,
                predecessors,
                use_site,
                root,
                establishes,
                |_, _, _, _| false,
                invalidates,
            )
        }

        pub(super) fn established_on_every_path_with_edges(
            function: &PsiOptimizationFunction,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            use_site: (usize, usize),
            root: PlaceId,
            establishes: impl Fn((usize, usize), &O, PlaceId) -> bool,
            establishes_edge: impl Fn(
                (usize, usize),
                &O,
                &optimization_unit::OptimizationEdge,
                PlaceId,
            ) -> bool,
            invalidates: impl Fn(&O, PlaceId) -> bool,
        ) -> bool {
            established_on_every_path_with_entry(
                function,
                predecessors,
                use_site,
                root,
                establishes,
                establishes_edge,
                |_| false,
                invalidates,
            )
        }

        pub(super) fn established_on_every_path_with_entry(
            function: &PsiOptimizationFunction,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            use_site: (usize, usize),
            root: PlaceId,
            establishes: impl Fn((usize, usize), &O, PlaceId) -> bool,
            establishes_edge: impl Fn(
                (usize, usize),
                &O,
                &optimization_unit::OptimizationEdge,
                PlaceId,
            ) -> bool,
            establishes_entry: impl Fn(PlaceId) -> bool,
            invalidates: impl Fn(&O, PlaceId) -> bool,
        ) -> bool {
            let mut pending = vec![(use_site.0, use_site.1, root)];
            let mut visited = BTreeSet::new();
            let mut established = false;
            while let Some((block_position, position, root)) = pending.pop() {
                if !visited.insert((block_position, position, root)) {
                    continue;
                }
                let block = &function.blocks[block_position];
                if position != 0 {
                    let previous = (block_position, position - 1);
                    let operation = &block.nodes[position - 1].operation;
                    if establishes(previous, operation, root) {
                        established = true;
                    } else {
                        if invalidates(operation, root) {
                            return false;
                        }
                        pending.push((previous.0, previous.1, root));
                    }
                } else {
                    if block.id == function.entry {
                        if !establishes_entry(root) {
                            return false;
                        }
                        established = true;
                        continue;
                    }
                    let Some(arrivals) = predecessors
                        .get(&block.id)
                        .filter(|arrivals| !arrivals.is_empty())
                    else {
                        return false;
                    };
                    for arrival in arrivals {
                        let Some((owner, predecessor)) = function
                            .blocks
                            .iter()
                            .enumerate()
                            .find(|(_, candidate)| candidate.id == *arrival)
                        else {
                            return false;
                        };
                        let mut found_edge = false;
                        for (position, node) in predecessor.nodes.iter().enumerate() {
                            for edge in node
                                .successors
                                .iter()
                                .filter(|edge| edge.target == block.id)
                            {
                                found_edge = true;
                                let Some(before) =
                                    crate::unit_validation::arrival_storage::root_before_edge(
                                        edge, root,
                                    )
                                else {
                                    return false;
                                };
                                if establishes_edge(
                                    (owner, position),
                                    &node.operation,
                                    edge,
                                    before,
                                ) {
                                    established = true;
                                } else {
                                    pending.push((owner, position + 1, before));
                                }
                            }
                        }
                        if !found_edge {
                            return false;
                        }
                    }
                }
            }
            established
        }
    }
    pub use byte_field_freshness::byte_field_root;
    mod byte_predicates {
        use super::byte_field_freshness::byte_field_root;
        use super::byte_field_freshness::field_path;
        use super::structural_freshness::established_on_every_path;
        use super::structural_freshness::structural_metadata_may_change;
        use crate::{OperationSite, OptimizationUnitValidationError};
        use abstract_operations::AbstractOperation as Operation;
        use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
        use semantic_vocabulary::StructuralFieldId;
        use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralDomainDeclaration;
        use terminal_psi::StructuralPathSegment;
        use terminal_psi::control_flow::ByteSequenceOperand;
        use terminal_psi::{
            ByteSequencePredicate, ByteSequencePredicateSet, StructuralAccess,
            StructuralTypeDeclaration, StructuralTypeShape, live_field_by_id,
        };
        use terminal_semantics::byte_parameter_predicates_in;
        use terminal_semantics::byte_qualification_predicates_in;
        use terminal_semantics::runtime_structural_path_tip;
        use terminal_semantics::shared_byte_field_copy_predicates;

        pub(crate) fn byte_write_predicates(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<ByteSequencePredicateSet> {
            if let Some(parameter) = function.arrival_parameter(place) {
                byte_parameter_predicates_in(unit.structural_domains.iter(), parameter)
            } else {
                owned_byte_predicates(unit, function, place)
            }
        }

        pub(super) fn validate_indexed_byte_stores(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let mut ascii = ByteSequencePredicateSet::EMPTY;
            ascii.insert(ByteSequencePredicate::AsciiOnly);
            for (block_position, block) in function.blocks.iter().enumerate() {
                for (node_position, node) in block.nodes.iter().enumerate() {
                    if let Operation::WriteOnlyPrimitiveStore {
                        destination, path, ..
                    } = &node.operation
                    {
                        if super::fixed_byte_freshness::requires_ascii(unit, destination, path)
                            == Some(true)
                            && !super::fixed_byte_freshness::holds(
                                unit,
                                function,
                                types,
                                predecessors,
                                (block_position, node_position),
                                destination.place(),
                                &path[..path.len() - 1],
                                function.structural_places.len() + 1,
                            )
                        {
                            return Err(OptimizationUnitValidationError::InvalidByteSequenceWrite(
                                OperationSite::new(
                                    function.machine,
                                    block.id,
                                    u32::try_from(node_position).unwrap_or(u32::MAX),
                                ),
                            ));
                        }
                        continue;
                    }
                    if let Operation::ByteSequenceWrite { destination, .. } = &node.operation {
                        if let Some(predicates) =
                            byte_write_predicates(unit, function, *destination)
                        {
                            let splits_scalars = predicates
                                .predicates()
                                .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
                                && !predicates.implies(ascii);
                            if splits_scalars
                                && !established_on_every_path(
                                    function,
                                    predecessors,
                                    (block_position, node_position),
                                    *destination,
                                    |_, operation, root| {
                                        matches!(operation,
                            Operation::EstablishByteSequenceConcatenation { result, .. }
                                if result.place == root
                                    && concatenation_predicates(unit, function, operation)
                                        .is_some_and(|initial| initial.implies(ascii)))
                                    },
                                    |operation, root| {
                                        structural_metadata_may_change(
                                            function,
                                            types,
                                            root,
                                            &[],
                                            operation,
                                        )
                                    },
                                )
                            {
                                return Err(
                                    OptimizationUnitValidationError::InvalidByteSequenceWrite(
                                        OperationSite::new(
                                            function.machine,
                                            block.id,
                                            u32::try_from(node_position).unwrap_or(u32::MAX),
                                        ),
                                    ),
                                );
                            }
                        }
                        continue;
                    }
                    let Operation::StructuralByteSequenceFieldByteStore {
                        destination,
                        path,
                        field,
                        ..
                    } = &node.operation
                    else {
                        continue;
                    };
                    let invalid = || {
                        OptimizationUnitValidationError::InvalidByteSequenceWrite(
                            OperationSite::new(
                                function.machine,
                                block.id,
                                u32::try_from(node_position).unwrap_or(u32::MAX),
                            ),
                        )
                    };
                    let predicates =
                        byte_field_predicates(unit, function, *destination, path, *field)
                            .ok_or_else(invalid)?;
                    let splits_scalars = predicates
                        .predicates()
                        .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
                        && !predicates.implies(ascii);
                    if !splits_scalars {
                        continue;
                    }
                    let exact_path = field_path(function, types, *destination, path, *field)
                        .ok_or_else(invalid)?;
                    let establishes = |_, operation: &Operation, root: PlaceId| {
                        matches!(operation,
                    Operation::StructuralByteSequenceFieldStore {
                        destination: written_root, path: written, field: written_field, source, ..
                    } if *written_root == root && written == path && written_field == field
                        && whole_source_predicates(
                            unit.structural_types.iter(),
                            unit.structural_domains.iter(),
                            function,
                            *source,
                        )
                        .is_some_and(|available| available.implies(ascii)))
                    };
                    let invalidates = |operation: &Operation, root: PlaceId| match operation {
                        Operation::StructuralByteSequenceFieldByteStore {
                            destination: written_root,
                            path: written,
                            field: written_field,
                            ..
                        } if *written_root == root => {
                            !(written == path && written_field == field)
                                && field_path(function, types, root, written, *written_field)
                                    .is_none_or(|written| {
                                        super::byte_field_freshness::overlaps(&exact_path, &written)
                                    })
                        }
                        _ => structural_metadata_may_change(
                            function,
                            types,
                            root,
                            &exact_path,
                            operation,
                        ),
                    };
                    if !established_on_every_path(
                        function,
                        predecessors,
                        (block_position, node_position),
                        *destination,
                        establishes,
                        invalidates,
                    ) {
                        return Err(invalid());
                    }
                }
            }
            Ok(())
        }

        pub(crate) fn owned_byte_predicates(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<ByteSequencePredicateSet> {
            let declaration = function.structural_place(place)?;
            let (producer, structural_type) = declaration.kind.as_operation_result()?;
            let result = function
                .block_nodes()
                .find_map(|node| match &node.operation {
                    Operation::EstablishByteSequenceConcatenation {
                        psi_operation,
                        result,
                        ..
                    } if *psi_operation == producer
                        && result.place == place
                        && result.structural_type == structural_type =>
                    {
                        Some(result)
                    }
                    _ => None,
                })?;
            if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !result.claims.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !unit.structural_types.iter().any(|declaration| {
                    declaration.id == structural_type
                        && declaration.is_bounded_owned_byte_sequence()
                })
            {
                return None;
            }
            byte_qualification_predicates_in(
                unit.structural_domains.iter(),
                structural_type,
                &result.qualifications,
            )
        }

        fn concatenation_predicates(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            operation: &Operation,
        ) -> Option<ByteSequencePredicateSet> {
            let Operation::EstablishByteSequenceConcatenation { left, right, .. } = operation
            else {
                return None;
            };
            let operand = |operand: &ByteSequenceOperand| match operand {
                ByteSequenceOperand::Whole { source, .. } => descriptor_source_predicates(
                    &Catalogs {
                        types: unit.structural_types.iter(),
                        domains: unit.structural_domains.iter(),
                    },
                    function,
                    source,
                    function.structural_places.len().saturating_add(1),
                    &mut Vec::new(),
                ),
                ByteSequenceOperand::Field { source, field, .. }
                    if source.access == StructuralAccess::SharedBorrow =>
                {
                    byte_field_predicates(unit, function, source.place, &source.path, *field)
                }
                _ => None,
            };
            Some(operand(left)?.concatenation(operand(right)?))
        }

        pub(crate) fn concatenation_preserves_predicates(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            operation: &Operation,
        ) -> bool {
            let Operation::EstablishByteSequenceConcatenation { result, .. } = operation else {
                return false;
            };
            owned_byte_predicates(unit, function, result.place).is_some_and(|required| {
                required.is_empty()
                    || concatenation_predicates(unit, function, operation)
                        .is_some_and(|initial| initial.implies(required))
            })
        }

        pub(crate) fn replacement_preserves_predicates(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            operation: &Operation,
        ) -> bool {
            let (source, required) = match operation {
                Operation::ByteSequenceStore {
                    destination,
                    source,
                    ..
                } => {
                    let Some(parameter) = function.arrival_parameter(*destination) else {
                        return false;
                    };
                    (
                        *source,
                        terminal_semantics::byte_parameter_predicates_from_domains(
                            &unit.structural_domains,
                            parameter,
                        ),
                    )
                }
                Operation::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => (
                    *source,
                    byte_field_predicates(unit, function, *destination, path, *field),
                ),
                _ => return false,
            };
            let Some(required) = required else {
                return false;
            };
            required.is_empty()
                || whole_source_predicates(
                    unit.structural_types.iter(),
                    unit.structural_domains.iter(),
                    function,
                    source,
                )
                .is_some_and(|available| available.implies(required))
        }

        pub(crate) fn whole_source_predicates<'t, 'd>(
            types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            domains: impl Iterator<Item = &'d StructuralDomainDeclaration> + Clone,
            function: &PsiOptimizationFunction,
            source: PlaceId,
        ) -> Option<ByteSequencePredicateSet> {
            source_predicates(
                &Catalogs { types, domains },
                function,
                source,
                function.structural_places.len().saturating_add(1),
                &mut Vec::new(),
            )
        }

        pub(crate) fn copied_field_predicates_at_transfer<'t>(
            types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            function: &PsiOptimizationFunction,
            source: PlaceId,
            consumer: semantic_vocabulary::OperationId,
        ) -> Option<ByteSequencePredicateSet> {
            copied_field_predicates_at_invocation(types, function, source, consumer.into())
        }

        pub(crate) fn copied_field_predicates_at_invocation<'t>(
            types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            function: &PsiOptimizationFunction,
            source: PlaceId,
            consumer: semantic_vocabulary::InvocationSite,
        ) -> Option<ByteSequencePredicateSet> {
            let declaration = function.structural_place(source)?;
            let (producer, structural_type) = declaration.kind.as_operation_result()?;
            let node = function
                .block_nodes()
                .find(|node| node.operation.psi_operation() == Some(producer))?;
            let Operation::StructuralLeafCopy {
                source: backing,
                path: abstract_operations::StructuralCopyPath::Ordinary(path),
                result,
                ..
            } = &node.operation
            else {
                return None;
            };
            if result.place != source
                || result.structural_type != structural_type
                || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
            {
                return None;
            }
            let carrier = types
                .clone()
                .find_map(|declaration| match declaration.shape {
                    StructuralTypeShape::ByteSequence(
                        carrier @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) if declaration.id == structural_type => Some(carrier),
                    _ => None,
                })?;
            let parameter = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .find(|parameter| parameter.place == *backing)?;
            if !matches!(
                parameter.access,
                StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
            ) {
                return None;
            }
            let (StructuralPathSegment::Field(identity), parent_path) = path.split_last()? else {
                return None;
            };
            if parent_path.iter().any(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            }) {
                return None;
            }
            let parent =
                runtime_structural_path_tip(types, parameter.structural_type, parent_path)?;
            let fields = parent.shape.record_fields()?;
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
            if field.field_type.byte_sequence_carrier()? != carrier {
                return None;
            }
            let consuming = function.block_nodes().find(|node| {
                node.operation
                    .module_invocation()
                    .map(|call| call.site)
                    .or_else(|| node.operation.psi_operation().map(Into::into))
                    == Some(consumer)
            })?;
            let call_arguments = match &consuming.operation {
                Operation::CallStructural {
                    structural_arguments,
                    ..
                }
                | Operation::InvokeNoNormal {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                _ => &[],
            };
            let mut occurrences = call_arguments
                .iter()
                .chain(crate::current_ownership::constructor_arguments(
                    &consuming.operation,
                ))
                .filter(|argument| argument.place == source)
                .peekable();
            if occurrences.peek().is_none()
                || occurrences.any(|argument| {
                    !argument.path.is_empty() || argument.access != StructuralAccess::Owned
                })
            {
                return None;
            }
            let carries = |bindings: &[abstract_operations::AbstractStructuralBinding]| {
                bindings
                    .iter()
                    .any(|binding| binding.argument.place == source)
            };
            if function.block_nodes().any(|node| {
        let operation = &node.operation;
        let escapes = match operation {
            Operation::Jump { structural_bindings, .. } => carries(structural_bindings),
            Operation::Conditional { when_true, when_false, .. } => carries(&when_true.structural_bindings) || carries(&when_false.structural_bindings),
            Operation::ReturnStructural { source: returned, .. } => *returned == source,
            _ => false
        };
        escapes || (operation.module_invocation().map(|call| call.site).or_else(|| operation.psi_operation().map(Into::into)) != Some(consumer)
            && (may_change_owned_source(operation, source)
                || matches!(operation, Operation::EstablishReference { source: argument, .. } | Operation::EstablishElementView { source: argument, .. } if argument.place == source)
                || matches!(operation, Operation::ByteSequenceSubslice { source: viewed, .. } if viewed.place == source)))
    }) { return None; }
            Some(field.field_type.byte_predicates())
        }

        struct Catalogs<Types, Domains> {
            types: Types,
            domains: Domains,
        }

        pub(crate) fn byte_field_predicates(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            root: PlaceId,
            path: &[StructuralPathSegment],
            field: StructuralFieldId,
        ) -> Option<ByteSequencePredicateSet> {
            byte_field_predicates_in(unit.structural_types.iter(), function, root, path, field)
        }

        fn byte_field_predicates_in<'t>(
            types: impl Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            function: &PsiOptimizationFunction,
            root: PlaceId,
            path: &[StructuralPathSegment],
            field: StructuralFieldId,
        ) -> Option<ByteSequencePredicateSet> {
            let (structural_type, _) = byte_field_root(function, root)?;
            let (structural_type, path) =
                if matches!(path.first(), Some(StructuralPathSegment::Referent)) {
                    let catalog: std::collections::BTreeMap<_, _> = types
                        .clone()
                        .map(|declaration| (declaration.id, declaration))
                        .collect();
                    super::super::references::record_field_destination(
                        function,
                        &catalog,
                        root,
                        path,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    )?
                } else {
                    (structural_type, path)
                };
            let declaration = runtime_structural_path_tip(types, structural_type, path)?;
            let fields = declaration.shape.record_fields()?;
            let field = live_field_by_id(fields, field)?;
            field.field_type.byte_sequence_carrier()?;
            Some(field.field_type.byte_predicates())
        }

        fn source_predicates<'t, 'd, Types, Domains>(
            catalogs: &Catalogs<Types, Domains>,
            function: &PsiOptimizationFunction,
            source: PlaceId,
            remaining: usize,
            memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
        ) -> Option<ByteSequencePredicateSet>
        where
            Types: Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            Domains: Iterator<Item = &'d StructuralDomainDeclaration> + Clone,
        {
            if let Some((_, predicates)) = memo.iter().find(|(place, _)| *place == source) {
                return *predicates;
            }
            let remaining = remaining.checked_sub(1)?;
            let index = memo.len();
            memo.push((source, None));
            let result = derive_source_predicates(catalogs, function, source, remaining, memo);
            memo[index].1 = result;
            result
        }

        fn descriptor_source_predicates<'t, 'd, Types, Domains>(
            catalogs: &Catalogs<Types, Domains>,
            function: &PsiOptimizationFunction,
            source: &terminal_psi::StructuralArgument,
            remaining: usize,
            memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
        ) -> Option<ByteSequencePredicateSet>
        where
            Types: Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            Domains: Iterator<Item = &'d StructuralDomainDeclaration> + Clone,
        {
            if source.path.is_empty() {
                return source_predicates(catalogs, function, source.place, remaining, memo);
            }
            let (root, access) = byte_field_root(function, source.place)?;
            if access == StructuralAccess::WriteOnlyBorrow {
                return None;
            }
            let selected = runtime_structural_path_tip(catalogs.types.clone(), root, &source.path)?;
            if !matches!(
                selected.shape,
                terminal_psi::StructuralTypeShape::ByteSequence(_)
            ) {
                return None;
            }
            Some(
                terminal_semantics::runtime_structural_field(
                    catalogs.types.clone(),
                    root,
                    &source.path,
                )
                .map_or(ByteSequencePredicateSet::EMPTY, |field| {
                    field.field_type.byte_predicates()
                }),
            )
        }

        fn derive_source_predicates<'t, 'd, Types, Domains>(
            catalogs: &Catalogs<Types, Domains>,
            function: &PsiOptimizationFunction,
            source: PlaceId,
            remaining: usize,
            memo: &mut Vec<(PlaceId, Option<ByteSequencePredicateSet>)>,
        ) -> Option<ByteSequencePredicateSet>
        where
            Types: Iterator<Item = &'t StructuralTypeDeclaration> + Clone,
            Domains: Iterator<Item = &'d StructuralDomainDeclaration> + Clone,
        {
            let declaration = function.structural_place(source)?;
            match declaration.kind {
                StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::BlockParameter { .. } => {
                    let parameter = function.arrival_parameter(source)?;
                    if !matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            ) && !abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
                parameter,
                |identity| {
                    catalogs
                        .types
                        .clone()
                        .find(|declaration| declaration.id == identity)
                },
            ) {
                return None;
            }
                    byte_parameter_predicates_in(catalogs.domains.clone(), parameter)
                }
                StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                    let mut producers =
                        function
                            .block_nodes()
                            .filter_map(|node| match &node.operation {
                                Operation::EstablishByteSequenceLiteral {
                                    place, bytes, ..
                                } if place.id == source => Some(bytes),
                                _ => None,
                            });
                    let bytes = producers.next()?;
                    if producers.next().is_some() {
                        return None;
                    }
                    Some(ByteSequencePredicateSet::holding(bytes))
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => {
                    let node = function
                        .block_nodes()
                        .find(|node| node.operation.psi_operation() == Some(producer))?;
                    if let Operation::ByteSequenceSubslice {
                        source: backing,
                        result,
                        ..
                    } = &node.operation
                    {
                        if result.place != source
                            || result.structural_type != structural_type
                            || result.multiplicity
                                != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !result.qualifications.is_empty()
                            || !result.projected_qualifications.is_empty()
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                            || !catalogs.types.clone().any(|declaration| {
                                declaration.id == structural_type
                                    && declaration.shape.is_shared_readable_byte_view()
                            })
                        {
                            return None;
                        }
                        let captured = if backing.path.is_empty() {
                            source_predicates(catalogs, function, backing.place, remaining, memo)?
                        } else {
                            let (root, access) = byte_field_root(function, backing.place)?;
                            if access == StructuralAccess::WriteOnlyBorrow {
                                return None;
                            }
                            shared_byte_field_copy_predicates(
                                catalogs.types.clone(),
                                root,
                                &backing.path,
                            )?
                        };
                        return Some(captured.subslice());
                    }
                    let (_, result) = node.operation.structural_result()?;
                    if result.place != source
                        || result.structural_type != structural_type
                        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !result.projected_qualifications.is_empty()
                        || !result.qualification_establishments.is_empty()
                        || !result.claims.is_empty()
                        || !catalogs.types.clone().any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape.is_owned_or_shared_byte_carrier()
                        })
                    {
                        return None;
                    }
                    let declared = byte_qualification_predicates_in(
                        catalogs.domains.clone(),
                        structural_type,
                        &result.qualifications,
                    )?;
                    if let Operation::StructuralLeafCopy {
                        source: backing,
                        path: abstract_operations::StructuralCopyPath::Ordinary(path),
                        ..
                    } = &node.operation
                        && catalogs.types.clone().any(|declaration| {
                            declaration.id == structural_type
                                && declaration.shape.is_borrowed_byte_view()
                        })
                    {
                        let captured = if path.is_empty() {
                            source_predicates(catalogs, function, *backing, remaining, memo)?
                        } else {
                            let (root, access) = byte_field_root(function, *backing)?;
                            if access == StructuralAccess::WriteOnlyBorrow {
                                return None;
                            }
                            shared_byte_field_copy_predicates(catalogs.types.clone(), root, path)?
                        };
                        let mut predicates = declared;
                        for predicate in captured.predicates() {
                            predicates.insert(predicate);
                        }
                        return Some(predicates);
                    }
                    let operands = match &node.operation {
                        Operation::EstablishByteSequenceConcatenation { left, right, .. } => {
                            Some((left, right))
                        }
                        _ => None,
                    };
                    if function
                        .block_nodes()
                        .any(|node| may_change_owned_source(&node.operation, source))
                    {
                        return Some(declared);
                    }
                    let Some((left, right)) = operands else {
                        return Some(declared);
                    };
                    let mut operand = |operand: &ByteSequenceOperand| match operand {
                        ByteSequenceOperand::Whole { source, .. } => descriptor_source_predicates(
                            catalogs, function, source, remaining, memo,
                        ),
                        ByteSequenceOperand::Field { source, field, .. } => {
                            if source.access != StructuralAccess::SharedBorrow {
                                return None;
                            }
                            byte_field_predicates_in(
                                catalogs.types.clone(),
                                function,
                                source.place,
                                &source.path,
                                *field,
                            )
                        }
                    };
                    Some(operand(left)?.concatenation(operand(right)?))
                }
                _ => None,
            }
        }

        fn may_change_owned_source(operation: &Operation, source: PlaceId) -> bool {
            match operation {
                Operation::ByteSequenceWrite { destination, .. }
                | Operation::ByteSequenceStore { destination, .. }
                | Operation::StructuralByteSequenceFieldStore { destination, .. }
                | Operation::StructuralByteSequenceFieldByteStore { destination, .. } => {
                    *destination == source
                }
                Operation::WriteOnlyPrimitiveStore { destination, .. } => {
                    destination.place() == source
                }
                Operation::StructuralScalarFieldStore { destination, .. } => {
                    destination.place() == source
                }
                Operation::StoreStructuralField { destination, .. } => {
                    destination.place() == source
                }
                Operation::MoveStructuralField { source: moved, .. } => moved.place() == source,
                Operation::CallStructural {
                    structural_arguments,
                    ..
                }
                | Operation::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.iter().any(|argument| {
                    argument.place == source && argument.access != StructuralAccess::SharedBorrow
                }),
                _ => false,
            }
        }
    }
    mod byte_views {
        use std::collections::BTreeMap;

        use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
        use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
        use semantic_vocabulary::{
            BlockId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
        };
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralDomainDeclaration,
            StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
            live_field_by_id,
        };

        use crate::{OperationSite, OptimizationUnitValidationError};
        use abstract_operations::byte_sequence_store_pair_pattern;
        use semantic_vocabulary::PlaceId;
        use terminal_psi::exclusive_borrow_pair_pattern;
        use terminal_psi::mutating_access_pair_pattern;
        use terminal_psi::reading_borrow_pair_pattern;
        use terminal_semantics::descriptor_results::descriptor_value_access;

        pub(crate) fn byte_field_store_capacity(
            function: &PsiOptimizationFunction,
            operation: &AbstractOperation,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> Option<u64> {
            if let AbstractOperation::ByteSequenceStore {
                destination,
                source,
                ..
            } = operation
            {
                let parameter = function.arrival_parameter(*destination)?;
                if destination == source
                    || !matches!(parameter.access, exclusive_borrow_pair_pattern!())
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                    || function.entry_claim_declared(*destination)
                    || function.content_entry_claim_declared(*destination)
                {
                    return None;
                }
                return match structural_types.get(&parameter.structural_type)?.shape {
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        capacity,
                    }) => Some(capacity),
                    _ => None,
                };
            }
            let (destination, path, field) = match operation {
                AbstractOperation::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } if destination != source => (destination, path, field),
                AbstractOperation::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    ..
                } => (destination, path, field),
                _ => return None,
            };
            match writable_byte_field_carrier(
                function,
                *destination,
                path,
                *field,
                structural_types,
            )? {
                ByteSequenceCarrier::BoundedOwned { capacity } => Some(capacity),
                _ => None,
            }
        }

        pub(crate) fn byte_view_field_store_is_valid(
            function: &PsiOptimizationFunction,
            operation: &AbstractOperation,
            source_kind: Option<&StructuralPlaceKind>,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> bool {
            let AbstractOperation::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                source,
                ..
            } = operation
            else {
                return false;
            };
            if source_kind.is_none() || destination == source {
                return false;
            }
            let Some(contract) =
                crate::unit_validation::operation_contracts::structural_source_contract(
                    function, *source,
                )
            else {
                return false;
            };
            let shared_view = ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            };
            let slot_carrier = match field {
                Some(field) => writable_byte_field_carrier(
                    function,
                    *destination,
                    path,
                    *field,
                    structural_types,
                ),
                None if path.is_empty() => declared_descriptor_home(function, *destination)
                    .and_then(|home| {
                        structural_types
                            .get(&home.structural_type)?
                            .shape
                            .is_shared_readable_byte_view()
                            .then_some(shared_view)
                    }),
                None => None,
            };
            matches!(
                contract.access,
                StructuralAccess::SharedBorrow | StructuralAccess::Owned
            ) && contract.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && structural_types
                    .get(&contract.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
                && slot_carrier == Some(shared_view)
        }

        pub fn declared_descriptor_home(
            function: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<&terminal_psi::StructuralOperationResult> {
            function
                .block_nodes()
                .find_map(|node| match &node.operation {
                    AbstractOperation::EstablishStructuralLocal { result, .. }
                        if result.place == place
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty() =>
                    {
                        Some(result)
                    }
                    _ => None,
                })
        }

        fn writable_byte_field_carrier(
            function: &PsiOptimizationFunction,
            destination: PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
            field: semantic_vocabulary::StructuralFieldId,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> Option<ByteSequenceCarrier> {
            let (root_type, storage_path) = if matches!(
                path.first(),
                Some(terminal_psi::StructuralPathSegment::Referent)
            ) {
                super::super::references::record_field_destination(
                    function,
                    structural_types,
                    destination,
                    path,
                    terminal_psi::StructuralAccess::MutableBorrow,
                )?
            } else {
                let (root_type, access) =
                    super::byte_field_freshness::byte_field_root(function, destination)?;
                let writable = match access {
                    exclusive_borrow_pair_pattern!() => true,
                    StructuralAccess::Owned => matches!(
                        structural_types.get(&root_type)?.shape,
                        StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Mixed { .. }
                            | StructuralTypeShape::FixedArray { .. }
                    ),
                    StructuralAccess::SharedBorrow => false,
                };
                if !writable {
                    return None;
                }
                (root_type, path)
            };
            if !terminal_psi::is_structural_scalar_store_path(storage_path) {
                return None;
            }
            let parent = super::byte_field_freshness::field_carrier(
                structural_types,
                root_type,
                storage_path,
            )?;
            let StructuralTypeShape::Record { fields, .. } = &structural_types.get(&parent)?.shape
            else {
                return None;
            };
            let member = live_field_by_id(fields, field)?;
            let contract = crate::unit_validation::operation_contracts::structural_source_contract(
                function,
                destination,
            )?;
            let mut written = path.to_vec();
            written.push(terminal_psi::StructuralPathSegment::Field(member.identity));
            if !contract.memberships_disjoint_from_write(&written) {
                return None;
            }
            member.field_type.byte_sequence_carrier()
        }

        pub(super) fn validate_byte_view_source(
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            operation: &AbstractOperation,
            source_kind: Option<&StructuralPlaceKind>,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            structural_domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let (source, length) = match operation {
                AbstractOperation::ByteSequenceWrite {
                    destination,
                    length,
                    ..
                } => (*destination, Some(*length)),
                byte_sequence_store_pair_pattern!(source, length, ..)
                | AbstractOperation::ByteSequenceRead { source, length, .. } => {
                    (*source, Some(*length))
                }
                _ => return Ok(()),
            };
            validate_source(
                function,
                block,
                node,
                operation,
                source_kind,
                structural_types,
                structural_domains,
                source,
                length,
            )
        }

        pub(super) fn validate_source(
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            operation: &AbstractOperation,
            source_kind: Option<&StructuralPlaceKind>,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            structural_domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            source: PlaceId,
            length: Option<semantic_vocabulary::ValueId>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let permitted_access = |access| match operation {
                AbstractOperation::ByteSequenceWrite { .. } => {
                    access == StructuralAccess::MutableBorrow
                }
                byte_sequence_store_pair_pattern!(..) => access == StructuralAccess::SharedBorrow,
                _ => matches!(
                    access,
                    StructuralAccess::SharedBorrow | mutating_access_pair_pattern!()
                ),
            };
            let arrival = function.arrival_parameter(source);
            let structural_type = match source_kind {
                Some(
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::BlockParameter { .. },
                ) => arrival.and_then(|parameter| {
                    let StructuralTypeShape::ByteSequence(carrier) =
                        structural_types.get(&parameter.structural_type)?.shape
                    else {
                        return None;
                    };
                    let owned = matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. });
                    let permitted = match operation {
                        AbstractOperation::ByteSequenceWrite { .. } => {
                            parameter.access == StructuralAccess::MutableBorrow
                                || owned && parameter.access == StructuralAccess::Owned
                        }
                        AbstractOperation::ByteSequenceLength { .. } => {
                            matches!(parameter.access, reading_borrow_pair_pattern!())
                                || owned
                                    && matches!(
                                        parameter.access,
                                        StructuralAccess::Owned | StructuralAccess::WriteOnlyBorrow
                                    )
                        }
                        byte_sequence_store_pair_pattern!(..) => {
                            parameter.access == StructuralAccess::SharedBorrow
                                || owned && parameter.access == StructuralAccess::Owned
                        }
                        _ => {
                            matches!(parameter.access, reading_borrow_pair_pattern!())
                                || owned && parameter.access == StructuralAccess::Owned
                        }
                    };
                    let qualifications_permitted =
                        terminal_semantics::byte_parameter_predicates_in(
                            structural_domains.values().copied(),
                            parameter,
                        )
                        .is_some();
                    (permitted
                        && qualifications_permitted
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && parameter.projected_qualifications.is_empty())
                    .then_some(parameter.structural_type)
                }),
                Some(StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                }) => Some(*structural_type),
                Some(StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                }) => owned_byte_result(function, source_kind, source, |identity| {
                    structural_types.get(&identity).is_some_and(|declaration| {
                        declaration.shape.is_bounded_owned_byte_sequence()
                    })
                })
                .or_else(|| {
                    let contract = super::super::operation_contracts::structural_source_contract(
                        function, source,
                    );
                    let contract = contract?;
                    let result = contract.operation_result()?;
                    (result.place == source
                        && result.structural_type == *structural_type
                        && permitted_access(contract.access)
                        && result.projected_qualifications.is_empty()
                        && terminal_semantics::byte_qualification_predicates_in(
                            structural_domains.values().copied(),
                            result.structural_type,
                            &result.qualifications,
                        )
                        .is_some()
                        && !contract.has_result_evidence()
                        && function.block_nodes().any(|node| {
                            node.operation.structural_result() == Some((*producer, result))
                        }))
                    .then_some(result.structural_type)
                }),
                _ => None,
            };
            let valid = structural_type
                .and_then(|identity| structural_types.get(&identity))
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::byte_sequence_view_carrier_pattern!()
                        )
                    ) || (declaration.shape.is_bounded_owned_byte_sequence()
                        && (owned_byte_result(function, source_kind, source, |identity| {
                            structural_types.get(&identity).is_some_and(|declaration| {
                                declaration.shape.is_bounded_owned_byte_sequence()
                            })
                        })
                        .is_some()
                            || arrival.is_some()))
                })
                && !function.entry_claim_declared(source)
                && !function.content_entry_claim_declared(source);
            let exact_length = length.is_none_or(|length| {
                function.block_nodes().any(|node| {
                    matches!(&node.operation, AbstractOperation::ByteSequenceLength {
                source: measured, result, ..
            } if measured.place == source && measured.path.is_empty() && result.value == length)
                })
            });
            let valid_result =
                match operation {
                    AbstractOperation::ByteSequenceSubslice {
                        psi_operation,
                        result,
                        ..
                    } => result.place != source
                        && structural_types.get(&result.structural_type).is_some_and(
                            |declaration| {
                                matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ByteSequence(
                                        ByteSequenceCarrier::BorrowedView { .. }
                                    )
                                )
                            },
                        )
                        && structural_type.is_some_and(|source_type| {
                            super::super::operation_contracts::structural_source_contract(
                                function, source,
                            )
                            .is_some_and(|contract| {
                                let Some(result_access) = descriptor_value_access(
                                    structural_types.values().copied(),
                                    result.structural_type,
                                    result.multiplicity,
                                ) else {
                                    return false;
                                };
                                terminal_semantics::descriptor_reborrow_matches(
                                    structural_types.values().copied(),
                                    source_type,
                                    contract.access,
                                    if contract.access != StructuralAccess::Owned {
                                        Some(contract.access)
                                    } else {
                                        descriptor_value_access(
                                            structural_types.values().copied(),
                                            source_type,
                                            contract.multiplicity,
                                        )
                                    },
                                    result.structural_type,
                                    result_access,
                                )
                            })
                        })
                        && super::super::operation_contracts::structural_operation_result_contract(
                            function, operation,
                        )
                        .is_some_and(|contract| contract.operation_result() == Some(result))
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                        && operation_result_placement(function, result, *psi_operation)
                        && !function.entry_claim_declared(result.place)
                        && !function.content_entry_claim_declared(result.place),
                    _ => true,
                };
            let writable = !matches!(operation, AbstractOperation::ByteSequenceWrite { .. })
                || super::super::operation_contracts::structural_source_contract(function, source)
                    .is_some_and(|contract| {
                        matches!(contract.access, mutating_access_pair_pattern!())
                            || contract.access == StructuralAccess::Owned
                                && structural_type
                                    .and_then(|identity| structural_types.get(&identity))
                                    .is_some_and(|declaration| {
                                        declaration.shape.is_bounded_owned_byte_sequence()
                                    })
                    });
            if !valid || !exact_length || !valid_result || !writable {
                return Err(
                    if matches!(operation, AbstractOperation::ByteSequenceSubslice { .. }) {
                        OptimizationUnitValidationError::InvalidByteSequenceSubslice(
                            OperationSite::new(function.machine, block, node),
                        )
                    } else if matches!(operation, AbstractOperation::ByteSequenceWrite { .. }) {
                        OptimizationUnitValidationError::InvalidByteSequenceWrite(
                            OperationSite::new(function.machine, block, node),
                        )
                    } else if length.is_some() {
                        OptimizationUnitValidationError::InvalidByteSequenceRead(
                            OperationSite::new(function.machine, block, node),
                        )
                    } else {
                        OptimizationUnitValidationError::InvalidByteSequenceLength(
                            OperationSite::new(function.machine, block, node),
                        )
                    },
                );
            }
            Ok(())
        }

        pub(super) fn owned_byte_result(
            function: &PsiOptimizationFunction,
            source_kind: Option<&StructuralPlaceKind>,
            source: PlaceId,
            mut bounded_owned_type: impl FnMut(StructuralTypeId) -> bool,
        ) -> Option<StructuralTypeId> {
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } = source_kind?
            else {
                return None;
            };
            let mut matching = function.block_nodes().filter_map(|node| {
                let (operation, result) = match &node.operation {
                    AbstractOperation::EstablishByteSequenceConcatenation {
                        psi_operation,
                        result,
                        ..
                    }
                    | AbstractOperation::CallStructural {
                        psi_operation,
                        result: AbstractStructuralCallResult::Structural { result, .. },
                        ..
                    } => (psi_operation, result),
                    _ => return None,
                };
                (*operation == *producer
                    && result.place == source
                    && result.structural_type == *structural_type
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.projected_qualifications.is_empty()
                    && result.qualification_establishments.is_empty()
                    && result.claims.is_empty()
                    && bounded_owned_type(*structural_type))
                .then_some(*structural_type)
            });
            let structural_type = matching.next()?;
            matching.next().is_none().then_some(structural_type)
        }
    }
    mod descriptor_sources {
        use crate::unit_validation::structural_catalog::{
            StructuralStorageProjection, structural_borrowed_storage_projection,
        };
        use abstract_operations::AbstractOperation as Operation;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::BlockId;
        use semantic_vocabulary::StructuralDomainId;
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeMap;
        use terminal_psi::StructuralArgument;
        use terminal_psi::StructuralDomainDeclaration;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity,
            StructuralTypeDeclaration, StructuralTypeShape,
        };
        use terminal_semantics::same_descriptor_coordinate;

        pub(super) fn valid(
            function: &PsiOptimizationFunction,
            operation: &Operation,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let (source, bytes, cut) = match operation {
                Operation::ByteSequenceLength { source, .. } => (source, true, None),
                Operation::ElementViewLength { source, .. } => (source, false, None),
                Operation::ByteSequenceSubslice {
                    source,
                    result,
                    length,
                    ..
                } => (source, true, Some((result, *length))),
                Operation::ElementViewSubslice {
                    source,
                    result,
                    length,
                    ..
                } => (source, false, Some((result, *length))),
                _ => return false,
            };
            let requested = match cut {
                Some((result, _)) if result.multiplicity == StructuralMultiplicity::Affine => {
                    StructuralAccess::MutableBorrow
                }
                Some((result, _))
                    if result.multiplicity != StructuralMultiplicity::Unrestricted =>
                {
                    return false;
                }
                _ => StructuralAccess::SharedBorrow,
            };
            let Some(shape) = source_shape(function, types, domains, source, requested) else {
                return false;
            };
            if !(bytes && cut.is_none() && fixed_bytes(&shape, types))
                && !matches!(
                    (&shape, bytes),
                    (StructuralTypeShape::ByteSequence(_), true)
                        | (StructuralTypeShape::ElementView { .. }, false)
                )
            {
                return false;
            }
            if let Some((result, length)) = cut {
                if result.place == source.place
                    || !result.qualifications.is_empty()
                    || result
                        .projected_qualifications
                        .iter()
                        .any(|row| bytes || !row.is_element_membership())
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return false;
                }
                let result_shape = types
                    .get(&result.structural_type)
                    .map(|declaration| &declaration.shape);
                let carrier_matches = match (&shape, result_shape) {
                    (
                        StructuralTypeShape::ElementView { element, .. },
                        Some(StructuralTypeShape::ElementView {
                            element: result_element,
                            ..
                        }),
                    ) => element == result_element,
                    (
                        StructuralTypeShape::ByteSequence(_),
                        Some(StructuralTypeShape::ByteSequence(
                            ByteSequenceCarrier::BorrowedView { .. },
                        )),
                    ) => true,
                    _ => false,
                };
                if !carrier_matches {
                    return false;
                }
                function.block_nodes().any(|node| match &node.operation {
                    Operation::ByteSequenceLength {
                        result: measured,
                        source: observed,
                        ..
                    } if bytes => {
                        measured.value == length && same_descriptor_coordinate(observed, source)
                    }
                    Operation::ElementViewLength {
                        result: measured,
                        source: observed,
                        ..
                    } if !bytes => {
                        measured.value == length && same_descriptor_coordinate(observed, source)
                    }
                    _ => false,
                })
            } else {
                true
            }
        }
        fn fixed_bytes(
            shape: &StructuralTypeShape,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> bool {
            let StructuralTypeShape::FixedArray { element, .. } = shape else {
                return false;
            };
            types.get(element).is_some_and(|row| matches!(row.shape,
        StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
        if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address()))
        }
        use terminal_psi::structural_access_covers as covers;

        pub(super) fn valid_whole(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            source: &StructuralArgument,
            length: semantic_vocabulary::ValueId,
        ) -> bool {
            source.access == StructuralAccess::SharedBorrow
                && source_shape(
                    function,
                    types,
                    domains,
                    source,
                    StructuralAccess::SharedBorrow,
                )
                .is_some_and(|shape| {
                    matches!(shape, StructuralTypeShape::ByteSequence(_))
                        || fixed_bytes(&shape, types)
                })
                && function.block_nodes().any(|node| {
                    matches!(&node.operation,
            Operation::ByteSequenceLength { source: measured, result, .. }
            if result.value == length && same_descriptor_coordinate(measured, source))
                })
        }

        pub(super) fn valid_representation_count(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<
                semantic_vocabulary::StructuralDomainId,
                &terminal_psi::StructuralDomainDeclaration,
            >,
            source: &terminal_psi::StructuralArgument,
            source_length: semantic_vocabulary::ValueId,
        ) -> bool {
            if source.access != StructuralAccess::SharedBorrow {
                return false;
            }
            let Some(shape) = source_shape(
                function,
                types,
                domains,
                source,
                StructuralAccess::SharedBorrow,
            ) else {
                return false;
            };
            match shape {
                StructuralTypeShape::ElementView { .. } => function.block_nodes().any(|node| {
                    matches!(&node.operation,
                Operation::ElementViewLength { source: observed, result, .. }
                    if result.value == source_length
                        && terminal_semantics::same_descriptor_coordinate(observed, source))
                }),
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(count),
                    ..
                } => function.block_nodes().any(|node| {
                    matches!(&node.operation,
                Operation::IntegerConstant {
                    result,
                    scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                } if *result == source_length
                    && Ok(*integer) == semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned, 64)
                    && *value == u128::from(count))
                }),
                _ => false,
            }
        }

        fn source_shape(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            source: &StructuralArgument,
            requested: StructuralAccess,
        ) -> Option<StructuralTypeShape> {
            if !covers(source.access, requested) {
                return None;
            }
            let mut borrow = source.clone();
            borrow.access = requested;
            let signature =
                crate::unit_validation::operation_contracts::structural_source_contract(
                    function,
                    source.place,
                )?;
            if !terminal_semantics::descriptor_source_projection_accessible(
                types.values().copied(),
                signature.structural_type,
                signature.multiplicity,
                signature.backing_access(types.values().copied())?,
                &source.path,
                requested,
            ) {
                return None;
            }
            let byte_qualified = types
                .get(&signature.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
                        || fixed_bytes(&declaration.shape, types)
                })
                && signature.declared_byte_predicates(domains).is_some();
            if signature.multiplicity == StructuralMultiplicity::Linear
                || signature.has_result_evidence()
                || !(signature.is_unqualified()
                    || byte_qualified
                    || signature.has_only_element_memberships()
                        && types.get(&signature.structural_type).is_some_and(|row| {
                            matches!(row.shape, StructuralTypeShape::ElementView { .. })
                        }))
            {
                return None;
            }
            let (root, prefix) = crate::unit_validation::references::project_referent_tail(
                types,
                signature.structural_type,
                signature.backing_access(types.values().copied())?,
                &borrow,
            )?;
            let selected = structural_borrowed_storage_projection(
                types.values().copied(),
                root,
                &source.path[prefix..],
                requested,
            )?;
            match selected {
                StructuralStorageProjection::Endpoint(identity) => types
                    .get(&identity)
                    .map(|declaration| declaration.shape.clone()),
                StructuralStorageProjection::ByteField { carrier, .. } => {
                    if matches!(carrier, ByteSequenceCarrier::BorrowedView { access: Some(access) } | ByteSequenceCarrier::BoundedView { access: Some(access), .. } if !covers(access, requested))
                    {
                        return None;
                    }
                    Some(StructuralTypeShape::ByteSequence(carrier))
                }
                StructuralStorageProjection::CollectionWindow { .. } => None,
            }
        }

        pub(super) fn validate_whole_freshness(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, std::collections::BTreeSet<BlockId>>,
        ) -> Result<(), crate::OptimizationUnitValidationError> {
            for (block_position, block) in function.blocks.iter().enumerate() {
                for (node_position, node) in block.nodes.iter().enumerate() {
                    let (byte_operands, representation_count) = match &node.operation {
                        Operation::EstablishByteSequenceConcatenation { left, right, .. }
                        | Operation::ByteSequenceEqual { left, right, .. } => {
                            ([Some(left), Some(right)], None)
                        }
                        Operation::ByteSequencePredicate { operand, .. } => {
                            ([Some(operand), None], None)
                        }
                        Operation::EstablishElementView {
                            source,
                            region:
                                terminal_psi::ElementViewRegion::WholeRepresentation {
                                    source_length,
                                    ..
                                },
                            ..
                        } => ([None, None], Some((source, *source_length))),
                        _ => continue,
                    };
                    let operands = byte_operands
                        .into_iter()
                        .flatten()
                        .filter_map(|operand| match operand {
                            terminal_psi::control_flow::ByteSequenceOperand::Whole {
                                source,
                                length,
                            } => Some((source, *length, false)),
                            _ => None,
                        })
                        .chain(representation_count.map(|(source, length)| (source, length, true)));
                    for (source, length, element_count) in operands {
                        if element_count && function.block_nodes().any(|candidate| {
                            matches!(candidate.operation, Operation::IntegerConstant { result, .. }
                        if result == length)
                        }) {
                            continue;
                        }
                        let invalid = || crate::unit_validation::catalog_mismatch(function);
                        let names = crate::unit_validation::arrival_storage::storage_names(
                            function,
                            block.id,
                            source.place,
                        );
                        let (producer, producer_root) = function.blocks.iter().enumerate().find_map(|(owner, block)| {
                    names.in_block(block.id).find_map(|name| {
                        block.nodes.iter().position(|candidate| {
                            let (observed, result) = match &candidate.operation {
                                Operation::ElementViewLength { source, result, .. } if element_count => (source, result),
                                Operation::ByteSequenceLength { source, result, .. } if !element_count => (source, result),
                                _ => return false,
                            };
                            observed.place == name
                                && terminal_semantics::structural_paths_select_same_location(&observed.path, &source.path)
                                && result.value == length
                        }).map(|position| ((owner, position), name))
                    })
                }).ok_or_else(invalid)?;
                        if !super::structural_freshness::unchanged_since(
                            function,
                            predecessors,
                            producer,
                            producer_root,
                            (block_position, node_position),
                            source.place,
                            |operation, root| {
                                source
                                    .path
                                    .contains(&terminal_psi::StructuralPathSegment::Referent)
                                    && matches!(
                                        operation,
                                        Operation::ByteSequenceStore { .. }
                                            | Operation::StructuralByteSequenceFieldStore { .. }
                                    )
                                    || super::structural_freshness::structural_metadata_may_change(
                                        function,
                                        types,
                                        root,
                                        &source.path,
                                        operation,
                                    )
                            },
                        ) {
                            return Err(invalid());
                        }
                    }
                }
            }
            Ok(())
        }
    }
    mod fixed_byte_freshness {
        use super::{byte_field_freshness, structural_freshness};
        use abstract_operations::{
            AbstractOperation as O, StructuralScalarStoreDestination as Destination,
        };
        use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
        use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{
            ByteSequencePredicate as Predicate, StructuralPathSegment as Segment,
            StructuralTypeDeclaration,
        };
        use terminal_semantics::structural_paths_may_overlap;

        pub(super) fn requires_ascii(
            unit: &PsiOptimizationUnit,
            destination: &Destination,
            path: &[Segment],
        ) -> Option<bool> {
            let (whole, projected) = match destination {
                Destination::Parameter(parameter)
                | Destination::OwnedBlockParameter { parameter, .. } => (
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                ),
                Destination::OwnedResult { result, .. } => {
                    (&result.qualifications, &result.projected_qualifications)
                }
            };
            let predicates = terminal_semantics::primitive_store_byte_predicates(
                unit.structural_types.iter(),
                unit.structural_domains.iter(),
                destination.structural_type(),
                whole,
                projected,
                destination.additional_input_qualifications(),
                path,
            )?;
            Some(
                predicates.predicates().any(|p| p == Predicate::ValidUtf8)
                    && !predicates.predicates().any(|p| p == Predicate::AsciiOnly),
            )
        }

        pub(super) fn holds(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            use_site: (usize, usize),
            root: PlaceId,
            path: &[Segment],
            budget: usize,
        ) -> bool {
            if budget == 0 {
                return false;
            }
            structural_freshness::established_on_every_path_with_entry(
                function,
                predecessors,
                use_site,
                root,
                |position, operation, root| {
                    match operation {
            O::EstablishZeroedStructural { result, .. } if result.place == root => true,
            O::EstablishScalarArray { result, elements, .. } if result.place == root && path.is_empty() =>
                elements.iter().all(|element| function.block_nodes().any(|node| matches!(node.operation,
                    O::IntegerConstant { result, value: semantic_vocabulary::IntegerValue::Unsigned(value), .. }
                    if result == *element && value <= 127))),
            O::StoreStructuralField { destination, path: written, field, value, .. }
                if destination.place() == root => {
                    let written = full_path(function, types, root, written, *field);
                    written.as_deref() == Some(path)
                        && holds(unit, function, types, predecessors, position, value.place, &value.path, budget - 1)
                }
            O::StructuralLeafCopy { source, path: abstract_operations::StructuralCopyPath::Ordinary(copied), result, .. }
                if result.place == root => {
                    let mut selected = copied.clone();
                    selected.extend_from_slice(path);
                    holds(unit, function, types, predecessors, position, *source, &selected, budget - 1)
                }
            operation => operation.structural_result().is_some_and(|(_, result)|
                result.place == root && declared_ascii(unit, result.structural_type,
                    &result.qualifications, &result.projected_qualifications, path)),
        }
                },
                |_, _, _, _| false,
                |root| {
                    function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == root)
                        .is_some_and(|parameter| {
                            declared_ascii(
                                unit,
                                parameter.structural_type,
                                &parameter.qualifications,
                                &parameter.projected_qualifications,
                                path,
                            )
                        })
                },
                |operation, root| {
                    match operation {
                        O::WriteOnlyPrimitiveStore {
                            destination,
                            path: written,
                            value,
                            range_obligation,
                            ..
                        } if destination.place() == root => {
                            if !structural_paths_may_overlap(path, written) {
                                return false;
                            }
                            let one_element = written.strip_prefix(path).is_some_and(|suffix| {
                                matches!(
                                    suffix,
                                    [Segment::FixedIndex(_) | Segment::RuntimeIndex { .. }]
                                )
                            });
                            let literal_ascii = function.block_nodes().any(|node| matches!(node.operation,
                            O::IntegerConstant { result, value: semantic_vocabulary::IntegerValue::Unsigned(byte), .. }
                            if result == value.value && byte <= 127));
                            let receiving_ascii = range_obligation.is_some()
                                && requires_ascii(unit, destination, written) == Some(true);
                            return !(one_element && (literal_ascii || receiving_ascii));
                        }
                        O::StoreStructuralField {
                            destination,
                            path: written,
                            field,
                            ..
                        } if destination.place() == root => {
                            return full_path(function, types, root, written, *field).is_none_or(
                                |written| structural_paths_may_overlap(path, &written),
                            );
                        }
                        O::StructuralScalarFieldStore {
                            destination,
                            path: written,
                            field,
                            ..
                        } if destination.place() == root => {
                            return full_path(function, types, root, written, Some(*field))
                                .is_none_or(|written| {
                                    structural_paths_may_overlap(path, &written)
                                });
                        }
                        O::WriteOnlyPrimitiveStore { .. }
                        | O::StoreStructuralField { .. }
                        | O::StructuralScalarFieldStore { .. }
                        | O::StructuralByteSequenceFieldByteStore { .. }
                        | O::StructuralByteSequenceFieldStore { .. }
                        | O::ByteSequenceStore { .. }
                        | O::ByteSequenceWrite { .. }
                        | O::ElementViewWrite { .. } => return true,
                        _ => {}
                    }
                    structural_freshness::structural_metadata_may_change(
                        function, types, root, path, operation,
                    )
                },
            )
        }

        fn full_path(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: PlaceId,
            path: &[Segment],
            field: Option<semantic_vocabulary::StructuralFieldId>,
        ) -> Option<Vec<Segment>> {
            field.map_or_else(
                || Some(path.to_vec()),
                |field| byte_field_freshness::field_path(function, types, root, path, field),
            )
        }

        fn declared_ascii(
            unit: &PsiOptimizationUnit,
            root: StructuralTypeId,
            whole: &[semantic_vocabulary::StructuralDomainId],
            projected: &[terminal_psi::StructuralPathQualification],
            path: &[Segment],
        ) -> bool {
            let Some(carrier) = terminal_semantics::runtime_structural_path_tip(
                unit.structural_types.iter(),
                root,
                path,
            ) else {
                return false;
            };
            whole
                .iter()
                .filter(|_| path.is_empty())
                .copied()
                .chain(
                    projected
                        .iter()
                        .filter(|membership| membership.path == path)
                        .map(|membership| membership.domain),
                )
                .any(|domain| {
                    terminal_semantics::byte_qualification_predicates_in(
                        unit.structural_domains.iter(),
                        carrier.id,
                        &[domain],
                    )
                    .is_some_and(|predicates| {
                        predicates
                            .predicates()
                            .any(|predicate| predicate == Predicate::AsciiOnly)
                    })
                })
        }
    }
    pub(crate) use byte_predicates::{
        byte_field_predicates, byte_write_predicates, concatenation_preserves_predicates,
        copied_field_predicates_at_invocation, copied_field_predicates_at_transfer,
        replacement_preserves_predicates, whole_source_predicates,
    };
    pub(crate) use byte_views::byte_field_store_capacity;
    pub use byte_views::declared_descriptor_home;
    mod control_flow {
        use crate::unit_validation::derived_metadata::{
            is_terminator, provenance_matches_operation, successors_match_operation,
        };
        use crate::{OperationSite, OptimizationUnitValidationError};
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, MachineId};
        use std::collections::{BTreeMap, BTreeSet};

        pub(super) struct FunctionControlFlow<'a> {
            pub(super) blocks: BTreeMap<BlockId, &'a optimization_unit::OptimizationBlock>,
            pub(super) predecessors: BTreeMap<BlockId, BTreeSet<BlockId>>,
            pub(super) successors: BTreeMap<BlockId, Vec<BlockId>>,
        }

        #[derive(Debug, Default)]
        pub(crate) struct ControlCyclePolicy {
            admitted_machines: BTreeSet<MachineId>,
        }

        impl ControlCyclePolicy {
            pub(crate) fn admit(&mut self, machine: MachineId) {
                self.admitted_machines.insert(machine);
            }

            pub(crate) fn admits(&self, machine: MachineId) -> bool {
                self.admitted_machines.contains(&machine)
            }
        }

        pub(super) fn index_blocks(
            function: &PsiOptimizationFunction,
        ) -> Result<
            BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
            OptimizationUnitValidationError,
        > {
            if let Some(pair) = function
                .blocks
                .windows(2)
                .find(|pair| pair[0].id >= pair[1].id)
            {
                return Err(OptimizationUnitValidationError::UnorderedBlock {
                    machine: function.machine,
                    block: pair[1].id,
                });
            }
            let blocks = function
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect::<BTreeMap<_, _>>();
            if !blocks.contains_key(&function.entry) {
                return Err(OptimizationUnitValidationError::MissingEntryBlock {
                    machine: function.machine,
                    block: function.entry,
                });
            }
            if !blocks[&function.entry].parameters.is_empty() {
                return Err(OptimizationUnitValidationError::EntryBlockHasParameters {
                    machine: function.machine,
                    block: function.entry,
                });
            }
            Ok(blocks)
        }

        pub(super) fn validate_nodes_and_edges<'a>(
            function: &'a PsiOptimizationFunction,
            blocks: BTreeMap<BlockId, &'a optimization_unit::OptimizationBlock>,
        ) -> Result<FunctionControlFlow<'a>, OptimizationUnitValidationError> {
            let mut edge_ids = BTreeSet::new();
            let mut predecessor = function
                .blocks
                .iter()
                .map(|block| (block.id, BTreeSet::new()))
                .collect::<BTreeMap<_, _>>();
            let mut successors = function
                .blocks
                .iter()
                .map(|block| (block.id, Vec::new()))
                .collect::<BTreeMap<_, _>>();
            for block in &function.blocks {
                if block.nodes.is_empty() {
                    return Err(OptimizationUnitValidationError::EmptyBlock {
                        machine: function.machine,
                        block: block.id,
                    });
                }
                for (index, node) in block.nodes.iter().enumerate() {
                    let node_index =
                        u32::try_from(index).expect("unit node index was built as u32");
                    if !provenance_matches_operation(&node.operation, &node.provenance)
                        || !crate::operation_metadata::metadata_matches(node, block.id, node_index)
                        || !successors_match_operation(&node.operation, &node.successors)
                    {
                        return Err(OptimizationUnitValidationError::OperationMetadataMismatch(
                            OperationSite::new(function.machine, block.id, node_index),
                        ));
                    }
                    let terminal = is_terminator(&node.operation);
                    if terminal && index + 1 != block.nodes.len() {
                        return Err(OptimizationUnitValidationError::TerminatorNotLast {
                            machine: function.machine,
                            block: block.id,
                        });
                    }
                    for edge in &node.successors {
                        if !blocks.contains_key(&edge.target) {
                            return Err(OptimizationUnitValidationError::UnknownSuccessor {
                                machine: function.machine,
                                block: block.id,
                                target: edge.target,
                            });
                        }
                        if !edge_ids.insert(edge.psi_edge) {
                            return Err(OptimizationUnitValidationError::DuplicateEdge(
                                edge.psi_edge,
                            ));
                        }
                        predecessor
                            .get_mut(&edge.target)
                            .expect("known target")
                            .insert(block.id);
                        successors
                            .get_mut(&block.id)
                            .expect("every block has a successor row")
                            .push(edge.target);
                    }
                }
                if !is_terminator(&block.nodes.last().expect("nonempty").operation) {
                    return Err(OptimizationUnitValidationError::MissingTerminator {
                        machine: function.machine,
                        block: block.id,
                    });
                }
            }

            Ok(FunctionControlFlow {
                blocks,
                predecessors: predecessor,
                successors,
            })
        }

        pub(crate) fn validate_total_cfg(
            function: &PsiOptimizationFunction,
            blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
            successors: &BTreeMap<BlockId, Vec<BlockId>>,
            admit_cycles: bool,
        ) -> Result<(), OptimizationUnitValidationError> {
            let mut reachable = BTreeSet::new();
            let mut pending = vec![function.entry];
            while let Some(block) = pending.pop() {
                if reachable.insert(block) {
                    pending.extend(successors[&block].iter().copied());
                }
            }
            if reachable.len() != blocks.len() {
                let block = blocks
                    .keys()
                    .find(|block| !reachable.contains(block))
                    .copied()
                    .expect("different block counts have an unreachable block");
                return Err(OptimizationUnitValidationError::UnreachableBlock {
                    machine: function.machine,
                    block,
                });
            }

            let mut indegree = blocks
                .keys()
                .copied()
                .map(|block| (block, 0usize))
                .collect::<BTreeMap<_, _>>();
            for target in successors.values().flatten() {
                *indegree.get_mut(target).expect("successor was validated") += 1;
            }
            let mut ready = indegree
                .iter()
                .filter_map(|(block, count)| (*count == 0).then_some(*block))
                .collect::<BTreeSet<_>>();
            let mut visited = 0usize;
            while let Some(block) = ready.pop_first() {
                visited += 1;
                for target in &successors[&block] {
                    let count = indegree.get_mut(target).expect("successor was validated");
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(*target);
                    }
                }
            }
            if visited != blocks.len() && !admit_cycles {
                let block = indegree
                    .iter()
                    .find_map(|(block, count)| (*count != 0).then_some(*block))
                    .expect("a cyclic graph leaves positive indegree");
                return Err(OptimizationUnitValidationError::ControlCycle {
                    machine: function.machine,
                    block,
                });
            }
            Ok(())
        }

        pub(crate) fn block_reaches(
            successors: &BTreeMap<BlockId, Vec<BlockId>>,
            start: BlockId,
            target: BlockId,
        ) -> bool {
            let mut visited = BTreeSet::new();
            let mut pending = vec![start];
            while let Some(block) = pending.pop() {
                if block == target {
                    return true;
                }
                if visited.insert(block) {
                    pending.extend(successors.get(&block).into_iter().flatten().copied());
                }
            }
            false
        }
    }
    mod fact_index {
        use crate::OptimizationUnitValidationError;
        use optimization_unit::{OptimizationFact, PsiOptimizationFunction};

        pub(crate) fn validate_fact_index(
            function: &PsiOptimizationFunction,
        ) -> Result<(), OptimizationUnitValidationError> {
            let expected = reconstruct_fact_index(function);
            if expected != function.facts {
                return Err(OptimizationUnitValidationError::FactIndexMismatch(
                    function.machine,
                ));
            }
            Ok(())
        }

        pub(crate) fn reconstruct_fact_index(
            function: &PsiOptimizationFunction,
        ) -> Vec<OptimizationFact> {
            use abstract_operations::{AbstractOperation as O, IntegerShiftPolicy};

            let mut expected = Vec::new();
            for operation in function.block_nodes().map(|node| &node.operation) {
                match operation {
                    O::StructuralScalarFieldStore {
                        psi_operation,
                        range_obligation: Some(obligation),
                        ..
                    }
                    | O::StructuralByteViewFieldStore {
                        psi_operation,
                        predicate_obligation: Some(obligation),
                        ..
                    } => expected.push(OptimizationFact::OperationObligationReference {
                        obligation: *obligation,
                        support: *psi_operation,
                    }),
                    abstract_operations::establish_record_or_case_pattern!(
                        psi_operation,
                        fields,
                        ..
                    ) => {
                        expected.extend(fields.iter().filter_map(|initializer| {
                            let terminal_psi::RecordFieldValue::Scalar {
                                range_obligation, ..
                            } = &initializer.value
                            else {
                                return None;
                            };
                            range_obligation.map(|obligation| {
                                OptimizationFact::OperationObligationReference {
                                    obligation,
                                    support: *psi_operation,
                                }
                            })
                        }));
                    }

                    O::EstablishByteSequenceConcatenation {
                        psi_operation,
                        capacity_obligation: obligation,
                        ..
                    }
                    | O::ByteSequenceSubslice {
                        psi_operation,
                        obligation,
                        ..
                    }
                    | O::ByteSequenceWrite {
                        psi_operation,
                        obligation,
                        ..
                    }
                    | abstract_operations::byte_sequence_store_pair_pattern!(
                        psi_operation,
                        obligation,
                        ..
                    )
                    | O::StructuralByteSequenceFieldByteStore {
                        psi_operation,
                        obligation,
                        ..
                    }
                    | abstract_operations::byte_read_or_field_read_pattern!(
                        psi_operation,
                        obligation,
                        ..
                    )
                    | O::ElementViewSubslice {
                        psi_operation,
                        obligation,
                        ..
                    }
                    | O::EstablishElementView {
                        psi_operation,
                        region:
                            terminal_psi::ElementViewRegion::RepresentationBytes { obligation, .. }
                            | terminal_psi::ElementViewRegion::WholeRepresentation { obligation, .. },
                        ..
                    }
                    | abstract_operations::element_view_read_write_pair_pattern!(
                        psi_operation,
                        obligation,
                        ..
                    )
                    | O::IntegerExactCast {
                        psi_operation,
                        obligation,
                        ..
                    } => expected.push(OptimizationFact::OperationObligationReference {
                        obligation: *obligation,
                        support: *psi_operation,
                    }),
                    O::IntegerShift {
                        psi_operation,
                        policy: IntegerShiftPolicy::Exact { obligation },
                        ..
                    } => expected.push(OptimizationFact::OperationObligationReference {
                        obligation: *obligation,
                        support: *psi_operation,
                    }),
                    O::IntegerArithmetic {
                        psi_operation,
                        arithmetic,
                        ..
                    } => {
                        if let Some(obligation) = arithmetic.obligation() {
                            expected.push(OptimizationFact::OperationObligationReference {
                                obligation,
                                support: *psi_operation,
                            });
                        }
                    }
                    _ => {}
                }
                if let Some(support) = operation.psi_operation() {
                    expected.extend(operation.runtime_indices().into_iter().map(
                        |(_, obligation)| OptimizationFact::OperationObligationReference {
                            obligation,
                            support,
                        },
                    ));
                }
                match operation {
                    O::BooleanConstant {
                        psi_operation,
                        result,
                        value,
                    } => expected.push(OptimizationFact::BooleanConstant {
                        value: *result,
                        constant: *value,
                        support: *psi_operation,
                    }),
                    O::IntegerConstant {
                        psi_operation,
                        result,
                        value,
                        ..
                    } => expected.push(OptimizationFact::IntegerConstant {
                        value: *result,
                        constant: *value,
                        support: *psi_operation,
                    }),
                    _ => {}
                }
            }
            expected
        }
    }
    mod mutable_views {
        use crate::unit_validation::structural_catalog::shared_view_declaration;
        use crate::{OperationSite, OptimizationUnitValidationError};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralAccess;

        fn exclusive_access(access: StructuralAccess) -> bool {
            matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())
        }

        pub(crate) struct CurrentMutableBorrows {
            mutable: BTreeSet<PlaceId>,
            arrivals: BTreeMap<BlockId, BTreeSet<PlaceId>>,
            possible_arrivals: BTreeMap<BlockId, BTreeSet<PlaceId>>,
        }

        impl CurrentMutableBorrows {
            pub(crate) fn may_be_live(&self, block: BlockId, place: PlaceId) -> bool {
                self.possible_arrivals
                    .get(&block)
                    .is_some_and(|places| places.contains(&place))
            }
        }

        struct MutableBorrowTransfer {
            source: BlockId,
            target: BlockId,
            retired: BTreeSet<PlaceId>,
            generated: BTreeSet<PlaceId>,
        }

        fn prepare_transfers(
            function: &PsiOptimizationFunction,
            mutable: &BTreeSet<PlaceId>,
            descriptor_views: &BTreeSet<PlaceId>,
            produced: &BTreeMap<BlockId, BTreeSet<PlaceId>>,
        ) -> Vec<MutableBorrowTransfer> {
            let blocks = function
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect::<BTreeMap<_, _>>();
            let block_names = function
                .block_structural_parameters()
                .map(|parameter| parameter.place)
                .collect::<BTreeSet<_>>();
            let mut transfers = Vec::new();
            for source in &function.blocks {
                for edge in source.successors() {
                    let Some(target) = blocks.get(&edge.target) else {
                        continue;
                    };
                    let mut retired = target
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.place)
                        .collect::<BTreeSet<_>>();
                    for binding in &edge.structural_bindings {
                        let argument = &binding.argument;
                        let attenuated_block_name = argument.access
                            == StructuralAccess::SharedBorrow
                            && block_names.contains(&argument.place)
                            && target.structural_parameters.iter().any(|parameter| {
                                parameter.place == binding.parameter
                                    && parameter.access == StructuralAccess::SharedBorrow
                            });
                        if mutable.contains(&argument.place)
                            && argument.path.is_empty()
                            && (descriptor_views.contains(&argument.place)
                                || attenuated_block_name
                                || exclusive_access(argument.access))
                        {
                            retired.insert(argument.place);
                        }
                    }
                    let mut generated = produced[&source.id]
                        .difference(&retired)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    generated.extend(
                        edge.structural_bindings
                            .iter()
                            .map(|binding| binding.parameter)
                            .filter(|place| mutable.contains(place)),
                    );
                    transfers.push(MutableBorrowTransfer {
                        source: source.id,
                        target: edge.target,
                        retired,
                        generated,
                    });
                }
            }
            transfers
        }

        pub(crate) fn current_mutable_borrows(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> CurrentMutableBorrows {
            let produced = function
                .blocks
                .iter()
                .map(|block| {
                    let views = block
                        .nodes
                        .iter()
                        .filter_map(|node| {
                            let source =
                        super::super::operation_contracts::structural_operation_result_contract(
                            function,
                            &node.operation,
                        )?;
                            let result = source.operation_result()?;
                            (exclusive_access(source.access)
                                && types
                                    .get(&source.structural_type)
                                    .is_some_and(|row| shared_view_declaration(row)))
                            .then_some(result.place)
                        })
                        .collect::<BTreeSet<_>>();
                    (block.id, views)
                })
                .collect::<BTreeMap<_, _>>();
            let mutable = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .filter(|parameter| {
                    exclusive_access(parameter.access)
                        && types.get(&parameter.structural_type).is_some_and(|row| {
                            shared_view_declaration(row) || {
                                abstract_operations::control_flow::address_joins::is_address_join(
                                    parameter,
                                    |identity| types.get(&identity).copied(),
                                )
                            }
                        })
                })
                .map(|parameter| parameter.place)
                .chain(produced.values().flatten().copied())
                .collect::<BTreeSet<_>>();
            if mutable.is_empty() {
                return CurrentMutableBorrows {
                    mutable,
                    arrivals: BTreeMap::new(),
                    possible_arrivals: BTreeMap::new(),
                };
            }
            let descriptor_views = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .filter(|parameter| {
                    mutable.contains(&parameter.place)
                        && types
                            .get(&parameter.structural_type)
                            .is_some_and(|row| shared_view_declaration(row))
                })
                .map(|parameter| parameter.place)
                .chain(produced.values().flatten().copied())
                .collect::<BTreeSet<_>>();
            let entry = function
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .filter(|place| mutable.contains(place))
                .collect::<BTreeSet<_>>();
            let transfers = prepare_transfers(function, &mutable, &descriptor_views, &produced);
            let reached = transfers
                .iter()
                .map(|transfer| transfer.target)
                .chain(std::iter::once(function.entry))
                .collect::<BTreeSet<_>>();
            let mut arrivals = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        if block.id == function.entry {
                            entry.clone()
                        } else {
                            mutable.clone()
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let mut possible_arrivals = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        if block.id == function.entry {
                            entry.clone()
                        } else {
                            BTreeSet::new()
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            loop {
                let mut next_arrivals = function
                    .blocks
                    .iter()
                    .map(|block| {
                        let incoming = if block.id == function.entry {
                            entry.clone()
                        } else if reached.contains(&block.id) {
                            mutable.clone()
                        } else {
                            BTreeSet::new()
                        };
                        (block.id, incoming)
                    })
                    .collect::<BTreeMap<_, _>>();
                let mut next_possible = function
                    .blocks
                    .iter()
                    .map(|block| {
                        (
                            block.id,
                            if block.id == function.entry {
                                entry.clone()
                            } else {
                                BTreeSet::new()
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                for transfer in &transfers {
                    if let Some(incoming) = next_arrivals.get_mut(&transfer.target) {
                        incoming.retain(|place| {
                            transfer.generated.contains(place)
                                || (!transfer.retired.contains(place)
                                    && arrivals[&transfer.source].contains(place))
                        });
                    }
                    if let Some(incoming) = next_possible.get_mut(&transfer.target) {
                        incoming.extend(
                            possible_arrivals[&transfer.source]
                                .iter()
                                .copied()
                                .filter(|place| !transfer.retired.contains(place))
                                .chain(transfer.generated.iter().copied()),
                        );
                    }
                }
                if next_arrivals == arrivals && next_possible == possible_arrivals {
                    break;
                }
                arrivals = next_arrivals;
                possible_arrivals = next_possible;
            }
            CurrentMutableBorrows {
                mutable,
                arrivals,
                possible_arrivals,
            }
        }

        pub(super) fn validate(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let CurrentMutableBorrows {
                mutable, arrivals, ..
            } = current_mutable_borrows(function, types);
            if mutable.is_empty() {
                return Ok(());
            }
            for block in &function.blocks {
                let mut available = arrivals[&block.id].clone();
                for (position, node) in block.nodes.iter().enumerate() {
                    let invalid = |place| {
                        OptimizationUnitValidationError::StructuralPlaceNotAvailable(
                            OperationSite::new(function.machine, block.id, position as u32),
                            place,
                        )
                    };
                    for place in super::structural_roots::operation_place_accesses(&node.operation)
                    {
                        if mutable.contains(&place) && !available.contains(&place) {
                            return Err(invalid(place));
                        }
                    }
                    for edge in &node.successors {
                        for (position, binding) in edge.structural_bindings.iter().enumerate() {
                            let place = binding.argument.place;
                            if !mutable.contains(&place) {
                                continue;
                            }
                            for prior in &edge.structural_bindings[..position] {
                                if prior.argument.place != place {
                                    continue;
                                }
                                let shared =
                                    prior.argument.access == StructuralAccess::SharedBorrow;
                                let other_shared =
                                    binding.argument.access == StructuralAccess::SharedBorrow;
                                if (!shared && !other_shared)
                                    || (shared != other_shared
                                        && terminal_semantics::structural_paths_may_overlap(
                                            &prior.argument.path,
                                            &binding.argument.path,
                                        ))
                                {
                                    return Err(invalid(place));
                                }
                            }
                        }
                    }
                    if let Some((_, result)) = node.operation.structural_result()
                        && mutable.contains(&result.place)
                    {
                        available.insert(result.place);
                    }
                    if let O::ByteSequenceWrite {
                        destination,
                        length,
                        ..
                    } = node.operation
                    {
                        let observed = block.nodes[..position].iter().rposition(|prior| matches!(&prior.operation,
                    O::ByteSequenceLength { source, result, .. } if source.place == destination && source.path.is_empty() && result.value == length));
                        if observed.is_none_or(|observed| {
                            block.nodes[observed + 1..position].iter().any(|prior| {
                                !super::structural_roots::operation_place_inputs(&prior.operation)
                                    .is_empty()
                                    && !matches!(
                                        prior.operation,
                                        O::ByteSequenceLength { .. } | O::ByteSequenceWrite { .. }
                                    )
                            })
                        }) {
                            return Err(invalid(destination));
                        }
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) use mutable_views::current_mutable_borrows;
    mod parameters {
        use crate::OptimizationUnitValidationError;
        use optimization_unit::{PsiOptimizationFunction, ValueDefinitionSite};
        use std::collections::BTreeSet;

        pub(super) fn validate_entry_claim_index(
            function: &PsiOptimizationFunction,
        ) -> Result<(), OptimizationUnitValidationError> {
            let indexed_entry_claims = function
                .entry_claim_declarations
                .iter()
                .map(|claim| claim.claim)
                .collect::<BTreeSet<_>>();
            if indexed_entry_claims.len() != function.entry_claim_declarations.len()
                || indexed_entry_claims != function.entry_claims
            {
                return Err(OptimizationUnitValidationError::EntryClaimIndexMismatch(
                    function.machine,
                ));
            }
            Ok(())
        }

        pub(crate) fn validate_parameter_metadata(
            function: &PsiOptimizationFunction,
        ) -> Result<(), OptimizationUnitValidationError> {
            for (position, parameter) in function.parameters.iter().enumerate() {
                let Ok(position) = u32::try_from(position) else {
                    return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                        machine: function.machine,
                        block: None,
                    });
                };
                if parameter.site != ValueDefinitionSite::FunctionParameter(position) {
                    return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                        machine: function.machine,
                        block: None,
                    });
                }
            }
            for block in &function.blocks {
                for (position, parameter) in block.parameters.iter().enumerate() {
                    let Ok(position) = u32::try_from(position) else {
                        return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                            machine: function.machine,
                            block: Some(block.id),
                        });
                    };
                    if parameter.site
                        != (ValueDefinitionSite::BlockParameter {
                            block: block.id,
                            position,
                        })
                    {
                        return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                            machine: function.machine,
                            block: Some(block.id),
                        });
                    }
                }
            }
            Ok(())
        }
    }
    mod provenance {
        use crate::{OperationSite, OptimizationUnitValidationError};
        use optimization_unit::{PsiOptimizationFunction, PsiProvenance};
        use semantic_vocabulary::{BlockId, EdgeId};
        use std::collections::{BTreeMap, BTreeSet};

        use super::control_flow::block_reaches;

        pub(crate) fn validate_provenance_work_effects(
            function: &PsiOptimizationFunction,
        ) -> Result<(), OptimizationUnitValidationError> {
            let mut node_provenance = BTreeMap::<PsiProvenance, Vec<(BlockId, bool)>>::new();
            let mut edge_provenance = BTreeMap::<PsiProvenance, BTreeSet<EdgeId>>::new();
            let mut edge_shapes = BTreeMap::<EdgeId, (BlockId, BlockId)>::new();
            let mut expected_effect = 0u64;
            for block in &function.blocks {
                for (index, node) in block.nodes.iter().enumerate() {
                    let index = u32::try_from(index).expect("unit node index was built as u32");
                    if node.provenance.is_empty() && node.successors.is_empty() {
                        return Err(OptimizationUnitValidationError::IncompleteProvenance(
                            OperationSite::new(function.machine, block.id, index),
                        ));
                    }
                    let unique_node_sources =
                        node.provenance.iter().copied().collect::<BTreeSet<_>>();
                    if unique_node_sources.len() != node.provenance.len() {
                        return Err(OptimizationUnitValidationError::DuplicateProvenance(
                            *node
                                .provenance
                                .first()
                                .expect("duplicated provenance is nonempty"),
                        ));
                    }
                    let is_exact_terminal = node.successors.is_empty()
                        && matches!(
                            node.operation,
                            abstract_operations::return_or_return_unit_pattern!()
                                | abstract_operations::return_structural_or_crash_pattern!()
                        );
                    for site in &node.provenance {
                        if edge_provenance.contains_key(site) {
                            return Err(OptimizationUnitValidationError::DuplicateProvenance(
                                *site,
                            ));
                        }
                        node_provenance
                            .entry(*site)
                            .or_default()
                            .push((block.id, is_exact_terminal));
                    }
                    let source_sites = node.provenance.iter().copied().collect::<BTreeSet<_>>();
                    let settled_sites = node
                        .work
                        .iter()
                        .map(|settlement| settlement.site)
                        .collect::<BTreeSet<_>>();
                    if source_sites != settled_sites
                        || node.work.len() != node.provenance.len()
                        || node
                            .work
                            .iter()
                            .zip(&node.provenance)
                            .any(|(settlement, source)| {
                                settlement.site != *source || settlement.units != 1
                            })
                    {
                        return Err(OptimizationUnitValidationError::WorkDoesNotMatchProvenance(
                            OperationSite::new(function.machine, block.id, index),
                        ));
                    }
                    for settlement in &node.work {
                        let _ = settlement;
                    }
                    for edge in &node.successors {
                        edge_shapes.insert(edge.psi_edge, (block.id, edge.target));
                        if edge.provenance.is_empty()
                            || edge.provenance.first() != Some(&PsiProvenance::Edge(edge.psi_edge))
                            || edge
                                .provenance
                                .iter()
                                .any(|site| !matches!(site, PsiProvenance::Edge(_)))
                        {
                            return Err(OptimizationUnitValidationError::IncompleteProvenance(
                                OperationSite::new(function.machine, block.id, index),
                            ));
                        }
                        let source_sites = edge.provenance.iter().copied().collect::<BTreeSet<_>>();
                        if source_sites.len() != edge.provenance.len()
                            || node_provenance
                                .keys()
                                .any(|site| source_sites.contains(site))
                        {
                            return Err(OptimizationUnitValidationError::DuplicateProvenance(
                                *edge
                                    .provenance
                                    .first()
                                    .expect("edge provenance is nonempty"),
                            ));
                        }
                        if edge.work.len() != edge.provenance.len()
                            || edge
                                .work
                                .iter()
                                .zip(&edge.provenance)
                                .any(|(settlement, source)| {
                                    settlement.site != *source || settlement.units != 1
                                })
                        {
                            return Err(
                                OptimizationUnitValidationError::WorkDoesNotMatchProvenance(
                                    OperationSite::new(function.machine, block.id, index),
                                ),
                            );
                        }
                        for source in &edge.provenance {
                            edge_provenance
                                .entry(*source)
                                .or_default()
                                .insert(edge.psi_edge);
                        }
                    }
                    if node.effect.input != expected_effect
                        || node.effect.output != expected_effect + 1
                    {
                        return Err(OptimizationUnitValidationError::BrokenEffectChain {
                            machine: function.machine,
                            expected: expected_effect,
                            actual: node.effect.input,
                        });
                    }
                    expected_effect += 1;
                }
            }
            let successors = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        block
                            .successors()
                            .map(|edge| edge.target)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for (source, occurrences) in node_provenance {
                if occurrences.len() < 2 {
                    continue;
                }
                if !matches!(source, PsiProvenance::Edge(_))
                    || occurrences.iter().any(|(_, terminal)| !terminal)
                {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(source));
                }
                for (index, (left, _)) in occurrences.iter().enumerate() {
                    for (right, _) in &occurrences[index + 1..] {
                        if left == right
                            || block_reaches(&successors, *left, *right)
                            || block_reaches(&successors, *right, *left)
                        {
                            return Err(
                                OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(
                                    source,
                                ),
                            );
                        }
                    }
                }
            }
            for (source, occurrences) in edge_provenance {
                let occurrences = occurrences.into_iter().collect::<Vec<_>>();
                for (index, left) in occurrences.iter().enumerate() {
                    let (_, left_target) = edge_shapes[left];
                    for right in &occurrences[index + 1..] {
                        let (right_owner, right_target) = edge_shapes[right];
                        let (left_owner, _) = edge_shapes[left];
                        if block_reaches(&successors, left_target, right_owner)
                            || block_reaches(&successors, right_target, left_owner)
                        {
                            return Err(
                                OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(
                                    source,
                                ),
                            );
                        }
                    }
                }
            }
            Ok(())
        }
    }
    mod results {
        use crate::OptimizationUnitValidationError;
        use optimization_unit::PsiOptimizationFunction;

        pub(super) fn validate_function_results(
            function: &PsiOptimizationFunction,
        ) -> Result<(), OptimizationUnitValidationError> {
            for operation in function.block_nodes().map(|node| &node.operation) {
                let matches = match (operation, &function.result) {
                    (
                        abstract_operations::AbstractOperation::Return {
                            result,
                            scalar_type,
                            ..
                        },
                        abstract_operations::AbstractFunctionResult::Scalar(signature),
                    ) => *result == signature.value && *scalar_type == signature.scalar_type,
                    (
                        abstract_operations::AbstractOperation::ReturnUnit { .. },
                        abstract_operations::AbstractFunctionResult::Unit,
                    )
                    | (
                        abstract_operations::AbstractOperation::ReturnStructural { .. },
                        abstract_operations::AbstractFunctionResult::Structural(_),
                    ) => true,
                    (
                        abstract_operations::AbstractOperation::Return { .. }
                        | abstract_operations::AbstractOperation::ReturnUnit { .. }
                        | abstract_operations::AbstractOperation::ReturnStructural { .. },
                        _,
                    ) => false,
                    _ => continue,
                };
                if !matches {
                    return Err(OptimizationUnitValidationError::FunctionResultMismatch(
                        function.machine,
                    ));
                }
            }
            Ok(())
        }
    }
    mod structural_roots {
        use super::super::catalog_mismatch;
        use super::byte_field_freshness::byte_field_root;
        use super::byte_views::byte_field_store_capacity;
        use super::byte_views::validate_byte_view_source;
        use super::descriptor_sources::valid_whole;
        use crate::unit_validation::operation_contracts::structural_source_contract;
        use crate::unit_validation::references::leaf_access_within;
        use crate::unit_validation::references::reference_source_type;
        use crate::unit_validation::structural_catalog::structural_root_key;
        use crate::{OperationSite, OptimizationUnitValidationError};
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractStructuralCallResult;
        use abstract_operations::StructuralWindowRoot;
        use optimization_unit::PsiOptimizationFunction;
        use optimization_unit::PsiOptimizationUnit;
        use semantic_vocabulary::BlockId;
        use semantic_vocabulary::StructuralFieldId;
        use semantic_vocabulary::{
            PlaceId, ScalarType, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
        };
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralAccess;
        use terminal_psi::StructuralOperationResult;
        use terminal_psi::StructuralPathSegment;
        use terminal_psi::StructuralTypeDeclaration;
        use terminal_psi::is_structural_scalar_store_path;
        use terminal_semantics::descriptor_results::DescriptorMachine;
        use terminal_semantics::descriptor_results::current_selected_payload_contract;
        use terminal_semantics::reference_source_storage_path;
        use terminal_semantics::structural_window_projection;

        mod availability {
            use std::collections::{BTreeMap, BTreeSet};

            use abstract_operations::AbstractOperation as O;
            use optimization_unit::PsiOptimizationFunction;
            use semantic_vocabulary::{BlockId, PlaceId};

            use crate::unit_validation::derived_metadata::dominators;
            use crate::{OperationSite, OptimizationUnitValidationError};

            pub(crate) fn validate_structural_place_availability(
                function: &PsiOptimizationFunction,
                blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
                predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            ) -> Result<(), OptimizationUnitValidationError> {
                let mut producers = BTreeMap::<PlaceId, (BlockId, Option<u32>)>::new();
                for block in &function.blocks {
                    for parameter in &block.structural_parameters {
                        producers.insert(parameter.place, (block.id, None));
                    }
                    for (node_index, node) in block.nodes.iter().enumerate() {
                        let place =
                crate::unit_validation::operation_contracts::structural_operation_result_contract(
                    function,
                    &node.operation,
                )
                .and_then(|contract| contract.operation_result().map(|result| result.place))
                .or(match &node.operation {
                    abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
                        Some(place.id)
                    }
                    _ => None,
                });
                        if let Some(place) = place {
                            producers.insert(
                                place,
                                (
                                    block.id,
                                    Some(
                                        u32::try_from(node_index)
                                            .expect("unit node index fits u32"),
                                    ),
                                ),
                            );
                        }
                    }
                }
                let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
                for block in &function.blocks {
                    for (node_index, node) in block.nodes.iter().enumerate() {
                        let node_index =
                            u32::try_from(node_index).expect("unit node index fits u32");
                        for place in operation_place_inputs(&node.operation) {
                            let Some((producer_block, producer_node)) = producers.get(&place)
                            else {
                                continue;
                            };
                            let available = (*producer_block == block.id
                                && producer_node.is_none_or(|producer| producer < node_index))
                                || (*producer_block != block.id
                                    && dominators
                                        .get(&block.id)
                                        .is_some_and(|set| set.contains(producer_block)));
                            if !available {
                                return Err(
                                    OptimizationUnitValidationError::StructuralPlaceNotAvailable(
                                        OperationSite::new(function.machine, block.id, node_index),
                                        place,
                                    ),
                                );
                            }
                        }
                    }
                }
                Ok(())
            }

            pub(crate) fn operation_place_accesses(operation: &O) -> Vec<PlaceId> {
                match operation {
                    O::ActivateTaskRegister {
                        gdt,
                        task_state,
                        scratch,
                        ..
                    } => vec![gdt.place, task_state.place, scratch.place],
                    O::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                        vec![gdt.place, scratch.place]
                    }
                    O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => vec![],
                    O::GlobalDescriptorTable { source, .. } => vec![source.place],
                    O::AtomicEvent { event, .. } => event.place().into_iter().collect(),
                    O::EstablishByteSequenceConcatenation { left, right, .. } => {
                        vec![left.source(), right.source()]
                    }
                    O::EstablishStructuralArray { elements, .. } => {
                        elements.iter().map(|argument| argument.place).collect()
                    }
                    abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields
                        .iter()
                        .filter_map(|initializer| match &initializer.value {
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                Some(argument.place)
                            }
                            terminal_psi::RecordFieldValue::Scalar { .. } => None,
                        })
                        .collect(),
                    O::Jump {
                        structural_bindings,
                        ..
                    } => structural_bindings
                        .iter()
                        .map(|binding| binding.argument.place)
                        .collect(),
                    O::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => when_true
                        .structural_bindings
                        .iter()
                        .chain(&when_false.structural_bindings)
                        .map(|binding| binding.argument.place)
                        .collect(),
                    O::PrimitiveLocalStore { destination, .. } => vec![*destination],
                    O::WriteOnlyPrimitiveStore { destination, .. } => vec![destination.place()],
                    O::StructuralScalarFieldStore { destination, .. } => vec![destination.place()],
                    abstract_operations::boundary_or_structural_call_pair_pattern!(
                        structural_arguments,
                        ..
                    ) => structural_arguments
                        .iter()
                        .map(|argument| argument.place)
                        .collect(),
                    abstract_operations::byte_sequence_store_pair_pattern!(
                        destination,
                        source,
                        ..
                    )
                    | O::StructuralByteViewFieldStore {
                        destination,
                        source,
                        ..
                    } => vec![*destination, *source],
                    O::MoveStructuralField { source, .. } => vec![source.place()],
                    O::StoreStructuralField {
                        destination, value, ..
                    } => vec![destination.place(), value.place],
                    O::StructuralLeafCopy { source, .. } => vec![*source],
                    O::ReleaseActivationClaim { source, .. }
                    | O::SplitActivationClaim { source, .. }
                    | O::MoveStructuralValue { source, .. } => {
                        vec![source.place]
                    }
                    O::MergeActivationClaim { lower, upper, .. } => vec![lower.place, upper.place],
                    O::ByteSequencePredicate { operand, .. } => vec![operand.source()],
                    O::ByteSequenceEqual { left, right, .. } => vec![left.source(), right.source()],
                    O::InvokeNoNormal {
                        structural_arguments,
                        ..
                    }
                    | O::ExternalCompletion {
                        structural_arguments,
                        ..
                    } => structural_arguments
                        .iter()
                        .map(|argument| argument.place)
                        .collect(),
                    O::TransformContent {
                        source, parents, ..
                    } => std::iter::once(source.place)
                        .chain(parents.iter().map(|parent| parent.place))
                        .collect(),
                    O::EstablishReference { source, .. }
                    | O::EstablishAccessReference { source, .. }
                    | O::ByteSequenceSubslice { source, .. }
                    | O::ByteSequenceLength { source, .. }
                    | O::EstablishElementView { source, .. }
                    | O::ElementViewLength { source, .. }
                    | O::ElementViewSubslice { source, .. } => {
                        vec![source.place]
                    }
                    O::ElementViewWrite { destination, .. } => vec![destination.place],
                    O::ReleaseReference { source, .. } => vec![*source],
                    abstract_operations::primitive_storage_observation_pattern!(source, ..)
                    | O::ByteSequenceRead { source, .. }
                    | O::ByteSequenceWrite {
                        destination: source,
                        ..
                    }
                    | O::StructuralByteSequenceFieldByteStore {
                        destination: source,
                        ..
                    }
                    | O::ElementViewRead { source, .. }
                    | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
                    | O::StructuralScalarFieldRead { source, .. }
                    | O::ReturnStructural { source, .. } => {
                        vec![*source]
                    }
                    O::StructuralCase { source, cases, .. } => std::iter::once(*source)
                        .chain(
                            cases
                                .iter()
                                .flat_map(|case| &case.structural_bindings)
                                .map(|binding| binding.argument.place),
                        )
                        .collect(),
                    O::EstablishActivationClaim { .. }
                    | O::EstablishNativeCodeLayout { .. }
                    | O::EstablishScalarArray { .. }
                    | O::EstablishPrimitiveLocal { .. }
                    | O::EstablishByteSequenceLiteral { .. }
                    | O::EstablishTrivialAffineLocal { .. }
                    | O::EstablishZeroedStructural { .. }
                    | O::EstablishStructuralLocal { .. }
                    | O::SelectImplementation { .. }
                    | O::PortWrite { .. }
                    | O::RegisterAccess { .. }
                    | O::Call { .. }
                    | O::IntegerConstant { .. }
                    | O::IeeeFloatConstant { .. }
                    | O::FloatResultPolicy { .. }
                    | O::NearestIeeeFloatFusedMultiplyAdd { .. }
                    | O::IeeeFloatCompare { .. }
                    | O::BooleanConstant { .. }
                    | O::BooleanNot { .. }
                    | O::BooleanEqual { .. }
                    | O::IntegerEqual { .. }
                    | O::IntegerLessThan { .. }
                    | O::IntegerLessOrEqual { .. }
                    | O::IntegerBitwiseNot { .. }
                    | O::IntegerWiden { .. }
                    | O::IntegerExactCast { .. }
                    | O::ScalarRepresentationView { .. }
                    | O::IntegerBitwiseAnd { .. }
                    | O::IntegerBitwiseOr { .. }
                    | O::IntegerBitwiseXor { .. }
                    | O::IntegerShift { .. }
                    | O::IntegerArithmetic { .. }
                    | O::TrappingInteger { .. }
                    | O::Return { .. }
                    | O::ReturnUnit { .. }
                    | O::Crash { .. } => Vec::new(),
                }
            }

            pub(crate) fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
                let mut inputs = operation_place_accesses(operation);
                for (_, actions) in operation.cleanup_rosters() {
                    inputs.extend(actions.iter().map(|action| action.place()));
                }
                inputs
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use abstract_operations::AbstractResult;
                use semantic_vocabulary::{OperationId, ScalarType, ValueId};
                use terminal_psi::{StructuralAccess, StructuralArgument};

                fn argument(number: u64) -> StructuralArgument {
                    StructuralArgument {
                        place: PlaceId::new(number).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }
                }

                #[test]
                fn operation_inputs_retain_all_byte_observations() {
                    let result = AbstractResult {
                        value: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    };
                    let left = terminal_psi::control_flow::ByteSequenceOperand::Whole {
                        source: argument(2),
                        length: ValueId::new(3).unwrap(),
                    };
                    let right = terminal_psi::control_flow::ByteSequenceOperand::Whole {
                        source: argument(4),
                        length: ValueId::new(5).unwrap(),
                    };
                    let predicate = O::ByteSequencePredicate {
                        psi_operation: OperationId::new(1).unwrap(),
                        result,
                        operand: left.clone(),
                        predicate: terminal_psi::ByteSequencePredicate::AsciiOnly,
                    };
                    assert_eq!(operation_place_inputs(&predicate), vec![argument(2).place]);
                    let equal = O::ByteSequenceEqual {
                        psi_operation: OperationId::new(2).unwrap(),
                        result,
                        left,
                        right,
                    };
                    assert_eq!(
                        operation_place_inputs(&equal),
                        vec![argument(2).place, argument(4).place]
                    );
                }

                #[test]
                fn operation_inputs_keep_content_parents_even_when_descriptor_query_selects_source()
                {
                    let result = terminal_psi::StructuralOperationResult {
                        place: PlaceId::new(6).unwrap(),
                        structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    };
                    let transform = O::TransformContent {
                        psi_operation: OperationId::new(1).unwrap(),
                        result,
                        source: argument(2),
                        parents: vec![argument(3), argument(4)],
                        derivations: Vec::new(),
                    };
                    assert_eq!(
                        operation_place_inputs(&transform),
                        vec![argument(2).place, argument(3).place, argument(4).place]
                    );
                }
            }
        }
        mod primitive_locals {
            use std::collections::BTreeMap;

            use abstract_operations::AbstractOperation as O;
            use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
            use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
            use terminal_psi::{
                StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
            };

            use crate::{OperationSite, OptimizationUnitValidationError};

            pub(super) fn validate(
                function: &PsiOptimizationFunction,
                block: BlockId,
                node: u32,
                operation: &O,
                types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            ) -> Result<(), OptimizationUnitValidationError> {
                let claim_free = |place| {
                    !function.entry_claim_declared(place)
                        && !function.content_entry_claim_declared(place)
                };
                let primitive = |structural_type, scalar_type| {
                    types.get(&structural_type).is_some_and(|declaration| {
            matches!(declaration.shape, terminal_psi::StructuralTypeShape::PrimitiveScalar(actual)
                if actual == scalar_type)
        })
                };
                let local_matches = |result: &StructuralOperationResult, scalar_type| {
                    result.multiplicity == StructuralMultiplicity::Unrestricted
                        && result.projected_qualifications.is_empty()
                        && result.qualification_establishments.is_empty()
                        && result.claims.is_empty()
                        && claim_free(result.place)
                        && primitive(result.structural_type, scalar_type)
                };
                let local = |place| {
                    function
                        .block_nodes()
                        .find_map(|node| match &node.operation {
                            O::EstablishPrimitiveLocal { result, .. } if result.place == place => {
                                Some(result)
                            }
                            _ => None,
                        })
                };
                let valid = match operation {
                    O::EstablishPrimitiveLocal {
                        psi_operation,
                        result,
                        value,
                        ..
                    } => {
                        local_matches(result, value.scalar_type)
                            && operation_result_placement(function, result, *psi_operation)
                    }
                    O::PrimitiveLocalStore {
                        destination, value, ..
                    } => local(*destination).is_some_and(|result| {
                        local_matches(result, value.scalar_type)
                            && result.qualifications.is_empty()
                            && types
                                .get(&result.structural_type)
                                .is_some_and(|declaration| declaration.scalar_restriction.is_none())
                    }),
                    O::PrimitiveScalarRead {
                        source,
                        path,
                        result,
                        ..
                    } => {
                        (path.is_empty()
                            && local(*source)
                                .is_some_and(|local| local_matches(local, result.scalar_type)))
                            || (!path.is_empty()
                                && claim_free(*source)
                                && projected_local(function, *source).is_some_and(|local| {
                                    terminal_semantics::primitive_read_projection_type(
                                        types.values().copied(),
                                        local.structural_type,
                                        path,
                                    ) == Some(result.scalar_type)
                                }))
                            || function
                                .structural_parameters
                                .iter()
                                .chain(function.block_structural_parameters())
                                .any(|parameter| {
                                    parameter.place == *source
                                        && (matches!(
                                            parameter.access,
                                            terminal_psi::reading_borrow_pair_pattern!()
                                        ) || (parameter.access == StructuralAccess::Owned
                                            && !path.is_empty()))
                                        && parameter.admits_scalar_read_at(path.is_empty())
                                        && claim_free(*source)
                                        && terminal_semantics::primitive_read_projection_type(
                                            types.values().copied(),
                                            parameter.structural_type,
                                            path,
                                        ) == Some(result.scalar_type)
                                })
                    }
                    _ => true,
                };
                if valid {
                    Ok(())
                } else {
                    Err(OptimizationUnitValidationError::InvalidPrimitiveLocal(
                        OperationSite::new(function.machine, block, node),
                    ))
                }
            }

            fn projected_local(
                function: &PsiOptimizationFunction,
                source: PlaceId,
            ) -> Option<&StructuralOperationResult> {
                let declaration = function.structural_place(source)?;
                let (producer, structural_type) = declaration.kind.as_operation_result()?;
                function.block_nodes().find_map(|node| {
                    let (operation, result) = node.operation.structural_result()?;
                    (operation == producer
                        && result.place == source
                        && result.structural_type == structural_type
                        && matches!(
                            result.multiplicity,
                            terminal_psi::nonlinear_multiplicity_pair_pattern!()
                        )
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty())
                    .then_some(result)
                })
            }
        }
        pub(crate) use availability::validate_structural_place_availability;
        pub(crate) use availability::{operation_place_accesses, operation_place_inputs};

        pub(crate) fn place_kinds_operation_result(
            place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
            result: &StructuralOperationResult,
            producer: semantic_vocabulary::OperationId,
        ) -> bool {
            matches!(
                place_kinds.get(&result.place),
                Some(StructuralPlaceKind::OperationResult {
                    producer: recorded,
                    structural_type,
                }) if *recorded == producer && *structural_type == result.structural_type
            )
        }

        pub(crate) struct ReferenceMembershipValidation<'a> {
            graph: &'a [DescriptorMachine],
            analysis: terminal_semantics::descriptor_results::DescriptorAnalysis<'a>,
        }

        impl<'a> ReferenceMembershipValidation<'a> {
            pub(crate) fn new(
                types: &'a [StructuralTypeDeclaration],
                graph: &'a [DescriptorMachine],
            ) -> Self {
                Self {
                    graph,
                    analysis: terminal_semantics::descriptor_results::DescriptorAnalysis::new(
                        types, graph,
                    ),
                }
            }
        }

        pub(crate) fn validate_structural_root_uniqueness(
            function: &PsiOptimizationFunction,
        ) -> Result<(), OptimizationUnitValidationError> {
            let mut roots = BTreeSet::new();
            for place in &function.structural_places {
                if !roots.insert(structural_root_key(place.kind)) {
                    return Err(
                        OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                            machine: function.machine,
                            kind: place.kind,
                        },
                    );
                }
            }
            Ok(())
        }

        pub(crate) fn validate_structural_root_operations(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            structural_domains: &BTreeMap<
                StructuralDomainId,
                &terminal_psi::StructuralDomainDeclaration,
            >,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            mut reference_memberships: Option<&mut ReferenceMembershipValidation<'_>>,
            view_graph: Option<&[DescriptorMachine]>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let shared_descriptor_result = function.result.structural().is_some_and(|result| {
                result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && terminal_semantics::descriptor_results::is_shared_descriptor(
                        structural_types.values().copied(),
                        result.structural_type,
                    )
            });
            let aggregate_descriptor_slots = function.result.structural().map(|result| {
                terminal_semantics::descriptor_results::aggregate_descriptor_slots(
                    &unit.structural_types,
                    result.place,
                    result.structural_type,
                )
            });
            let aggregate_origins_valid =
                match aggregate_descriptor_slots {
                    None => true,
                    Some(None) => false,
                    Some(Some(slots)) => slots.is_empty()
                        || crate::descriptor_results::owned_aggregate_descriptor_returns_with_graph(
                            function,
                            view_graph,
                            &unit.structural_types,
                        ),
                };
            let descriptor_origins_valid = aggregate_origins_valid
                && (!shared_descriptor_result
                    || view_graph.is_some_and(|graph| {
                        terminal_semantics::descriptor_results::reconstruct_result_origins(
                            &unit.structural_types,
                            graph,
                            function.machine,
                        )
                        .is_some()
                    }));
            let place_kinds = function
                .structural_places
                .iter()
                .map(|place| (place.id, place.kind))
                .collect::<BTreeMap<_, _>>();
            let exact_store_destination =
                |destination: &terminal_psi::StructuralParameterDeclaration| {
                    function.arrival_parameter(destination.place) == Some(destination)
                        && (!matches!(
                            place_kinds.get(&destination.place),
                            Some(StructuralPlaceKind::BlockParameter { .. })
                        ) || matches!(
                            destination.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                        ))
                };
            let exact_scalar_store_destination =
                |destination: &abstract_operations::StructuralScalarStoreDestination| {
                    match destination {
            abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                exact_store_destination(parameter)
                    && matches!(
                        parameter.access,
                        StructuralAccess::Owned
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    )
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                producer,
                result,
            } => {
                place_kinds_operation_result(&place_kinds, result, *producer)
                    && function.block_nodes().any(|node| {
                        node.operation
                            .structural_result()
                            .is_some_and(|(operation, actual)| {
                                operation == *producer && actual == result
                            })
                    })
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                block,
                parameter,
            } => {
                place_kinds.get(&parameter.place)
                    == Some(&StructuralPlaceKind::BlockParameter {
                        block: *block,
                        position: parameter.position,
                    })
                    && *block != function.entry
                    && parameter.access == StructuralAccess::Owned
                    && function.block(*block).is_some_and(|candidate| {
                        candidate
                            .structural_parameters
                            .get(parameter.position as usize)
                            == Some(parameter)
                    })
            }
        }
                };
            for (block_position, block) in function.blocks.iter().enumerate() {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                    if !view_memberships_match(
                        unit,
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        view_graph,
                    ) || !reference_result_memberships_match(
                        unit,
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                    ) {
                        return Err(catalog_mismatch(function));
                    }
                    primitive_locals::validate(
                        function,
                        block.id,
                        node_index,
                        &node.operation,
                        structural_types,
                    )?;
                    match &node.operation {
                        O::EstablishActivationClaim {
                            psi_operation,
                            result,
                            bound,
                            committed,
                            backing_type,
                            established_case,
                            rejected_case,
                        } => {
                            let operation = terminal_psi::Operation {
                                id: *psi_operation,
                                static_reach_binding: None,
            required_contract: None,
                                suspension_crossing: None,
                                result: terminal_psi::OperationResult::Structural(result.clone()),
                                kind: terminal_psi::OperationKind::EstablishActivationClaim {
                                    bound: *bound,
                                    committed: *committed,
                                    backing_type: *backing_type,
                                    established_case: *established_case,
                                    rejected_case: *rejected_case,
                                },
                            };
                            terminal_semantics::activation_storage::activation_claim_shape_in(
                                &unit.structural_types,
                                &operation,
                            )
                            .map_err(|_| catalog_mismatch(function))?;
                            if !place_kinds_operation_result(&place_kinds, result, *psi_operation) {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::StructuralByteSequenceFieldByteStore { .. } => {
                            if byte_field_store_capacity(
                                function,
                                &node.operation,
                                structural_types,
                            )
                            .is_none()
                            {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        abstract_operations::byte_sequence_store_pair_pattern!(source, ..) => {
                            validate_byte_view_source(
                                function,
                                block.id,
                                node_index,
                                &node.operation,
                                place_kinds.get(source),
                                structural_types,
                                structural_domains,
                            )?;
                            if byte_field_store_capacity(
                                function,
                                &node.operation,
                                structural_types,
                            )
                            .is_none()
                            {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::StructuralByteViewFieldStore { source, .. } => {
                            if !super::byte_views::byte_view_field_store_is_valid(
                                function,
                                &node.operation,
                                place_kinds.get(source),
                                structural_types,
                            ) {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        abstract_operations::byte_len_or_field_read_pattern!(
                            source,
                            path,
                            field,
                            ..
                        ) => {
                            let geometry =
                                if matches!(path.first(), Some(StructuralPathSegment::Referent)) {
                                    super::super::references::record_field_destination(
                                        function,
                                        structural_types,
                                        *source,
                                        path,
                                        StructuralAccess::MutableBorrow,
                                    )
                                    .filter(
                                        |(_, storage_path)| {
                                            terminal_psi::is_static_structural_scalar_store_path(
                                                storage_path,
                                            )
                                        },
                                    )
                                } else {
                                    byte_field_root(function, *source)
                                        .map(|(root, _)| (root, path.as_slice()))
                                };
                            let valid = byte_field_root(function, *source)
                        .is_some_and(|(_, access)| {
                            let Some((structural_type, storage_path)) = geometry else { return false; };
                            (access != StructuralAccess::WriteOnlyBorrow
                                || matches!(node.operation, O::StructuralByteSequenceFieldLength { .. }))
                            && is_structural_scalar_store_path(storage_path)
                            && super::byte_field_freshness::field_carrier(
                                structural_types, structural_type, storage_path,
                            ).and_then(|parent| structural_types.get(&parent))
                            .is_some_and(|declaration| match &declaration.shape {
                                terminal_psi::StructuralTypeShape::Record { fields, .. } => fields.iter().any(|candidate|
                                    candidate.id == *field && !candidate.relevance.is_erased()
                                    && terminal_semantics::sequence_field_allows_observation(
                                        structural_types.values().copied(),
                                        &candidate.field_type,
                                        if matches!(node.operation, O::StructuralByteSequenceFieldLength { .. }) {
                                            terminal_semantics::SequenceFieldObservation::Length
                                        } else {
                                            terminal_semantics::SequenceFieldObservation::Contents
                                        },
                                    )),
                                _ => false,
                            })
                        });
                            if !valid {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::StructuralCaseMembership {
                            source,
                            path,
                            case,
                            result,
                            ..
                        } => {
                            let signature = structural_source_contract(function, *source);
                            let valid = result.scalar_type == ScalarType::Boolean
                                && signature.is_some_and(|signature| {
                                    signature.access != StructuralAccess::WriteOnlyBorrow
                                && super::super::structural_catalog::resolve_structural_path(
                                    structural_types,
                                    signature.structural_type,
                                    path,
                                )
                                .and_then(|endpoint| structural_types.get(&endpoint))
                                .is_some_and(|declaration| match &declaration.shape {
                                    terminal_psi::StructuralTypeShape::Sum { cases }
                                    | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => {
                                        cases.iter().any(|candidate| candidate.id == *case)
                                    }
                                    _ => false,
                                })
                                });
                            if !valid {
                                return Err(
                            OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
                                machine: function.machine,
                                source: *source,
                            },
                        );
                            }
                        }
                        O::EstablishElementView {
                            source,
                            region:
                                terminal_psi::ElementViewRegion::WholeRepresentation {
                                    source_length,
                                    ..
                                },
                            ..
                        } => {
                            if !super::descriptor_sources::valid_representation_count(
                                function,
                                structural_types,
                                structural_domains,
                                source,
                                *source_length,
                            ) {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::ByteSequenceSubslice { .. }
                        | O::ByteSequenceLength { .. }
                        | O::ElementViewSubslice { .. }
                        | O::ElementViewLength { .. } => {
                            if !super::descriptor_sources::valid(
                                function,
                                &node.operation,
                                structural_types,
                                structural_domains,
                            ) {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::ByteSequenceEqual { left, right, .. } => {
                            for operand in [left, right] {
                                if let terminal_psi::control_flow::ByteSequenceOperand::Whole {
                                    source,
                                    length,
                                } = operand
                                    && !valid_whole(
                                        function,
                                        structural_types,
                                        structural_domains,
                                        source,
                                        *length,
                                    )
                                {
                                    return Err(catalog_mismatch(function));
                                }
                            }
                        }
                        O::ByteSequencePredicate { operand, .. } => {
                            if let terminal_psi::control_flow::ByteSequenceOperand::Whole {
                                source,
                                length,
                            } = operand
                                && !valid_whole(
                                    function,
                                    structural_types,
                                    structural_domains,
                                    source,
                                    *length,
                                )
                            {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::ByteSequenceRead { source, .. } => {
                            validate_byte_view_source(
                                function,
                                block.id,
                                node_index,
                                &node.operation,
                                place_kinds.get(source),
                                structural_types,
                                structural_domains,
                            )?;
                        }
                        O::ByteSequenceWrite { destination, .. } => {
                            validate_byte_view_source(
                                function,
                                block.id,
                                node_index,
                                &node.operation,
                                place_kinds.get(destination),
                                structural_types,
                                structural_domains,
                            )?;
                        }
                        O::WriteOnlyPrimitiveStore {
                            destination,
                            path,
                            value,
                            ..
                        } => {
                            let valid = exact_scalar_store_destination(destination)
                                && terminal_semantics::primitive_store_restriction(
                                    structural_types.values().copied(),
                                    structural_domains.values().copied(),
                                    destination.structural_type(),
                                    destination.qualifications(),
                                    destination.projected_qualifications(),
                                    destination.additional_input_qualifications(),
                                    path,
                                )
                                .is_some()
                                && (destination.multiplicity()
                                    == terminal_psi::StructuralMultiplicity::Unrestricted
                                    || (!path.is_empty()
                                        && destination.multiplicity()
                                            == terminal_psi::StructuralMultiplicity::Affine))
                                && !function.entry_claim_declared(destination.place())
                                && !function.content_entry_claim_declared(destination.place())
                                && terminal_semantics::primitive_projection_type(
                                    structural_types.values().copied(),
                                    destination.structural_type(),
                                    path,
                                ) == Some(value.scalar_type);
                            if !valid {
                                return Err(
                                    OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore(
                                        OperationSite::new(function.machine, block.id, node_index),
                                    ),
                                );
                            }
                        }
                        O::StructuralScalarFieldStore {
                            destination,
                            path,
                            field,
                            value,
                            range_obligation,
                            ..
                        } => {
                            let geometry = if path.contains(&StructuralPathSegment::Referent) {
                                super::super::references::record_field_destination(
                                    function,
                                    structural_types,
                                    destination.place(),
                                    path,
                                    StructuralAccess::WriteOnlyBorrow,
                                )
                            } else {
                                Some((destination.structural_type(), path.as_slice()))
                            };
                            let field_declaration = geometry.and_then(|(root, storage_path)| {
                                is_structural_scalar_store_path(storage_path).then_some(())?;
                                terminal_semantics::projected_field_declaration(
                                    structural_types.values().copied(),
                                    root,
                                    storage_path,
                                    *field,
                                )
                            });
                            let preserved =
                                terminal_semantics::scalar_field_store_preserves_domain_content(
                                    structural_types.values().copied(),
                                    structural_domains.values().copied(),
                                    &unit.identities,
                                    destination.structural_type(),
                                    destination.qualifications(),
                                    destination.projected_qualifications(),
                                    path,
                                    *field,
                                );
                            let conserved =
                                terminal_semantics::scalar_field_store_preserves_content(
                                    structural_types.values().copied(),
                                    structural_domains.values().copied(),
                                    &unit.identities,
                                    destination.structural_type(),
                                    destination.place(),
                                    function.content_entry_claims.iter(),
                                    path,
                                    *field,
                                );
                            let exact_destination = exact_scalar_store_destination(destination);
                            let valid = exact_destination
                                && preserved
                                && conserved
                                && field_declaration.is_some_and(|declaration| {
                                    declaration.field_type.scalar_type() == Some(value.scalar_type)
                                        && terminal_semantics::scalar_field_requires_evidence(
                                            &declaration.field_type,
                                        ) == range_obligation.is_some()
                                });
                            if !valid {
                                return Err(
                            OptimizationUnitValidationError::InvalidStructuralScalarFieldStore(
                                OperationSite::new(function.machine, block.id, node_index),
                            ),
                        );
                            }
                        }
                        O::StructuralScalarFieldRead {
                            result,
                            source,
                            path,
                            field,
                            ..
                        } => {
                            let valid = matches!(
                                result.scalar_type,
                                ScalarType::Boolean | ScalarType::Integer(_)
                            ) && readable_field_type(
                                function,
                                structural_types,
                                *source,
                            )
                            .is_some_and(|(root, access)| {
                                let (observed_root, access) = match structural_types
                                    .iter()
                                    .find(|(id, _)| **id == root)
                                    .map(|(_, declaration)| &declaration.shape)
                                {
                                    Some(terminal_psi::StructuralTypeShape::Reference {
                                        referent,
                                        access,
                                    }) => (*referent, *access),
                                    Some(terminal_psi::StructuralTypeShape::ElementView {
                                        ..
                                    }) => (root, StructuralAccess::SharedBorrow),
                                    _ => (root, access),
                                };
                                observed_scalar_field(
                                    structural_types,
                                    observed_root,
                                    path,
                                    *field,
                                    access,
                                ) == Some(result.scalar_type)
                            });
                            if !valid {
                                return Err(
                            OptimizationUnitValidationError::InvalidStructuralScalarFieldRead(
                                OperationSite::new(function.machine, block.id, node_index),
                            ),
                        );
                            }
                        }
                        O::EstablishAccessReference {
                            psi_operation,
                            result,
                            source,
                            correspondence,
                        } => {
                            if matches!(
                                correspondence,
                                abstract_operations::AccessReferenceBacking::BorrowedExtent(_)
                            ) {
                                return Err(catalog_mismatch(function));
                            }
                            let actual = reference_source_type(function, structural_types, source);
                            let mut plans = unit
                                .structural_types
                                .access_views()
                                .iter()
                                .filter(|plan| plan.carrier == correspondence.carrier());
                            let (Some(plan), None) = (plans.next(), plans.next()) else {
                                return Err(catalog_mismatch(function));
                            };
                            let exact = crate::access_references::validate_backing(
                                unit.structural_types.as_slice(),
                                unit.structural_types.access_views(),
                                plan,
                                correspondence.source_type(),
                                correspondence,
                            )
                            .is_ok();
                            if !exact
                        || !result.is_affine_plain()
                        || !place_kinds_operation_result(&place_kinds, result, *psi_operation)
                        || !matches!(
                            source.access,
                            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                        )
                        || !actual.is_some_and(|actual| {
                            crate::unit_validation::references::access_reference_source_compatible(
                                structural_types,
                                actual,
                                result.structural_type,
                                correspondence,
                            )
                        })
                        || !leaf_access_within(
                            structural_types,
                            result.structural_type,
                            &[],
                            source.access,
                        )
                    {
                        return Err(catalog_mismatch(function));
                    }
                        }
                        O::EstablishReference {
                            psi_operation,
                            result,
                            source,
                            storage,
                        } => {
                            let ancestors_preserved =
                                structural_source_contract(function, source.place).is_some_and(
                                    |contract| {
                                        let path = reference_source_storage_path(
                                            structural_types.values().copied(),
                                            contract.structural_type,
                                            &source.path,
                                        );
                                        contract.reference_ancestor_qualifications_preserved(
                                            structural_types.values().copied(),
                                            structural_domains.values().copied(),
                                            &unit.identities,
                                            path,
                                            source.access,
                                        ) && (source.access == StructuralAccess::SharedBorrow
                                            || terminal_semantics::store_preserves_content(
                                                structural_types.values().copied(),
                                                structural_domains.values().copied(),
                                                &unit.identities,
                                                contract.structural_type,
                                                source.place,
                                                function.content_entry_claims.iter().filter(
                                                    |claim| claim.input.segments.len() < path.len(),
                                                ),
                                                path,
                                            ))
                                    },
                                );
                            let field_compatible = storage.is_none()
                                && structural_source_contract(function, source.place).is_some_and(
                                    |contract| {
                                        let path = reference_source_storage_path(
                                            structural_types.values().copied(),
                                            contract.structural_type,
                                            &source.path,
                                        );
                                        terminal_semantics::reference_byte_field_compatible(
                                            structural_types.values().copied(),
                                            structural_domains.values().copied(),
                                            contract.structural_type,
                                            path,
                                            result.structural_type,
                                            &result.projected_qualifications,
                                        )
                                    },
                                );
                            let expected =
                                crate::unit_validation::references::referent(
                                    structural_types,
                                    result.structural_type,
                                )
                                .or_else(|| match structural_types
                                    .get(&result.structural_type)
                                    .map(|declaration| &declaration.shape)
                                {
                                    Some(terminal_psi::StructuralTypeShape::Reference {
                                        referent,
                                        ..
                                    }) => Some(*referent),
                                    _ => None,
                                });
                            let valid = expected.is_some()
                        && ancestors_preserved
                        && crate::unit_validation::references::reference_source_accessible(function, structural_types, source)
                        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && result.qualifications.is_empty()
                        && (field_compatible || structural_source_contract(
                            function,
                            source.place,
                        )
                        .is_some_and(|contract| {
                            let path = reference_source_storage_path(
                                structural_types.values().copied(),
                                contract.structural_type,
                                &source.path,
                            );
                            let Ok(mut captured) = contract.reference_memberships(
                                structural_types.values().copied(),
                                structural_domains.values().copied(),
                                &unit.identities,
                                path,
                                source.access,
                                result.structural_type,
                                storage.as_ref(),
                            ) else {
                                return false;
                            };
                            if let Some(context) = reference_memberships.as_deref_mut()
                                && let Ok(backing) = terminal_semantics::descriptor_results::reference_backing_memberships(
                                    &unit.structural_types, &unit.structural_domains, &unit.identities,
                                    &mut context.analysis, function.machine, (block.id, node_index), source, storage.as_ref(), result,
                                    |place| structural_source_contract(function, place)
                                        .map(|contract| contract.payload_contract()),
                                )
                            {
                                captured.extend(backing);
                            }
                            if source.access == StructuralAccess::SharedBorrow
                                && result.projected_qualifications.iter().any(|row| !captured.contains(row))
                            {
                                let Some(context) = reference_memberships.as_deref_mut() else { return false; };
                                let Ok(current) = current_selected_payload_contract(
                                    &unit.structural_types, &unit.structural_domains, context.graph,
                                    function.machine, (block.id, node_index), source.place, path,
                                    &|place| structural_source_contract(function, place)
                                        .map(|contract| contract.payload_contract()),
                                ) else { return false; };
                                terminal_psi::StructuralPathQualification::append_under(
                                    &current.qualifications, &current.projected_qualifications,
                                    &[StructuralPathSegment::Referent], &mut captured,
                                );
                            }
                            result
                                .projected_qualifications
                                .iter()
                                .all(|row| captured.contains(row))
                                && (source.access == StructuralAccess::SharedBorrow
                                    || captured
                                        .iter()
                                        .all(|row| result.projected_qualifications.contains(row)))
                        }))
                        && result.claims.is_empty()
                        && place_kinds_operation_result(&place_kinds, result, *psi_operation)
                        && matches!(
                            source.access,
                            StructuralAccess::MutableBorrow
                                | StructuralAccess::SharedBorrow
                        )
                        && (field_compatible || reference_source_type(
                            function,
                            structural_types,
                            source,
                        )
                        .is_some_and(|actual| match storage {
                            Some(storage) => {
                                terminal_semantics::reference_storage_source_compatible(
                                    structural_types.values().copied(),
                                    actual,
                                    result.structural_type,
                                    storage.stored_scalar,
                                )
                            }
                            None => terminal_semantics::reference_source_compatible(
                                structural_types.values().copied(),
                                actual,
                                result.structural_type,
                            ),
                        }))
                        && leaf_access_within(
                            structural_types,
                            result.structural_type,
                            &[],
                            source.access,
                        );
                            if !valid {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::ReleaseReference { source, path, .. } => {
                            let valid = structural_source_contract(function, *source).is_some_and(
                                |signature| {
                                    (path.is_empty()
                                || (signature.access == StructuralAccess::Owned
                                    && signature.multiplicity
                                        == terminal_psi::StructuralMultiplicity::Affine
                                    && signature.is_unqualified()
                                    && !signature.has_result_evidence()))
                                && path.iter().all(|segment| {
                                    matches!(
                                        segment,
                                        StructuralPathSegment::Field(_)
                                            | StructuralPathSegment::FixedIndex(_)
                                    )
                                })
                                && crate::unit_validation::references::leaf_type(
                                    structural_types,
                                    signature.structural_type,
                                    path,
                                )
                                .and_then(|selected| structural_types.get(&selected))
                                .is_some_and(|declaration| {
                                    matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::Reference { .. }
                                    )
                                })
                                },
                            );
                            if !valid {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::MoveStructuralField {
                            psi_operation,
                            result,
                            source,
                            path,
                            field,
                        } => {
                            let valid = borrowed_window_root(
                                function,
                                source,
                                &place_kinds,
                                StructuralAccess::MutableBorrow,
                            ) && window_field_type(
                                function,
                                structural_types,
                                source,
                                path,
                                *field,
                                StructuralAccess::MutableBorrow,
                            ) == Some(result.structural_type)
                                && root_window_loan(source, path, *field)
                                && window_memberships(
                                    function,
                                    structural_types,
                                    source,
                                    path,
                                    *field,
                                )
                                .is_some_and(
                                    |(whole, projected)| {
                                        result.qualifications == whole
                                            && result.projected_qualifications == projected
                                            && result.qualification_establishments.is_empty()
                                    },
                                )
                                && place_kinds_operation_result(
                                    &place_kinds,
                                    result,
                                    *psi_operation,
                                );
                            if !valid {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::StoreStructuralField {
                            disposition,
                            destination,
                            path,
                            field,
                            value,
                            ..
                        } => {
                            let whole_owned_store = path.is_empty()
                                && field.is_none()
                                && destination.parameter().is_none()
                                && *disposition == terminal_psi::StructuralStoreDisposition::Vacant;
                            let valid = borrowed_window_root(
                                function,
                                destination,
                                &place_kinds,
                                disposition.required_access(),
                            ) && (whole_owned_store
                                || root_window_loan(destination, path, *field))
                                && value.access == StructuralAccess::Owned
                                && value.path.is_empty()
                                && window_field_type(
                                    function,
                                    structural_types,
                                    destination,
                                    path,
                                    *field,
                                    disposition.required_access(),
                                )
                                .is_some_and(|field_type| {
                                    structural_source_contract(function, value.place).is_some_and(
                                        |signature| {
                                            signature.structural_type == field_type
                                        && (*disposition
                                            == terminal_psi::StructuralStoreDisposition::Vacant
                                            || terminal_semantics::discardable_store_value(
                                                structural_types.values().copied(),
                                                field_type,
                                                signature.multiplicity,
                                                function
                                                    .entry_claim_declarations
                                                    .iter()
                                                    .chain(&function.block_entry_claims)
                                                    .any(|claim| claim.input == value.place)
                                                    || function
                                                        .content_entry_claim_declared(value.place)
                                                    || signature.operation_result().is_some_and(
                                                        |result| !result.claims.is_empty(),
                                                    ),
                                            ))
                                        && window_memberships(
                                            function,
                                            structural_types,
                                            destination,
                                            path,
                                            *field,
                                        )
                                        .is_some_and(
                                            |(whole, projected)| {
                                                signature.supplies_qualifications(
                                                    &value.path,
                                                    &whole,
                                                    &projected,
                                                )
                                            },
                                        )
                                        },
                                    )
                                });
                            if !valid {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::MoveStructuralValue {
                            psi_operation,
                            result,
                            source,
                        }
                        | O::TransformContent {
                            psi_operation,
                            result,
                            source,
                            ..
                        } => {
                            let invalid = |reason| {
                                OptimizationUnitValidationError::InvalidOwnedValueTransfer {
                                    machine: function.machine,
                                    operation: *psi_operation,
                                    reason,
                                }
                            };
                            let signature = structural_source_contract(function, source.place)
                                .ok_or_else(|| invalid("source contract is absent"))?;
                            let selected = terminal_semantics::canonical_static_projection(
                                structural_types.values().copied(),
                                signature.structural_type,
                                &source.path,
                            );
                            if source.access != StructuralAccess::Owned
                                || signature.access != StructuralAccess::Owned
                                || source.place == result.place
                                || selected.is_none_or(|(_, declaration)| {
                                    declaration.id != result.structural_type
                                })
                                || !result.qualification_establishments.is_empty()
                                || (source.path.is_empty()
                                    && signature.multiplicity != result.multiplicity)
                                || result.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Unrestricted
                                || signature.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Unrestricted
                                || (!source.path.is_empty()
                                    && result.multiplicity
                                        != terminal_psi::StructuralMultiplicity::Linear
                                    && result.claims.iter().any(|claim| claim.path.is_empty()))
                                || !place_kinds_operation_result(
                                    &place_kinds,
                                    result,
                                    *psi_operation,
                                )
                            {
                                return Err(invalid(
                                    "transfer requires the same owned carrier, multiplicity and fresh result without issuance",
                                ));
                            }
                            let graph = view_graph
                                .ok_or_else(|| invalid("descriptor flow graph is malformed"))?;
                            let contract = current_selected_payload_contract(
                                &unit.structural_types,
                                &unit.structural_domains,
                                graph,
                                function.machine,
                                (block.id, node_index),
                                source.place,
                                &source.path,
                                &|place| {
                                    structural_source_contract(function, place)
                                        .map(|contract| contract.payload_contract())
                                },
                            )
                            .map_err(invalid)?;
                            if contract.carrier != result.structural_type
                                || contract
                                    .qualifications
                                    .iter()
                                    .any(|domain| !result.qualifications.contains(domain))
                                || contract
                                    .projected_qualifications
                                    .iter()
                                    .any(|row| !result.projected_qualifications.contains(row))
                            {
                                return Err(invalid(
                                    "transfer loses the selected source memberships",
                                ));
                            }
                            if let O::TransformContent {
                                parents,
                                derivations,
                                ..
                            } = &node.operation
                            {
                                terminal_semantics::content_transformations::validate_contract(
                                    &unit.structural_types,
                                    &unit.structural_domains,
                                    &unit.identities,
                                    &|place| {
                                        structural_source_contract(function, place)
                                            .map(|contract| contract.structural_type)
                                    },
                                    source,
                                    parents,
                                    derivations,
                                    result,
                                )
                                .map_err(invalid)?;
                                terminal_semantics::content_transformations::validate_memberships(
                                    &unit.structural_types,
                                    &unit.structural_domains,
                                    &unit.identities,
                                    result,
                                    derivations,
                                    &contract.qualifications,
                                    &contract.projected_qualifications,
                                )
                                .map_err(invalid)?;
                                for parent in parents {
                                    let root = structural_source_contract(function, parent.place)
                                        .ok_or_else(|| {
                                        invalid("content parent source contract is absent")
                                    })?;
                                    let parent_contract = current_selected_payload_contract(
                                        &unit.structural_types,
                                        &unit.structural_domains,
                                        graph,
                                        function.machine,
                                        (block.id, node_index),
                                        parent.place,
                                        &parent.path,
                                        &|place| {
                                            structural_source_contract(function, place)
                                                .map(|contract| contract.payload_contract())
                                        },
                                    )
                                    .map_err(invalid)?;
                                    terminal_semantics::content_transformations::validate_parent_memberships(
                                &unit.structural_types, &unit.structural_domains, &unit.identities,
                                root.structural_type, parent, derivations,
                                &parent_contract.qualifications, &parent_contract.projected_qualifications,
                                &|relative| {
                                    let mut path = parent.path.clone();
                                    for segment in relative {
                                        if let StructuralPathSegment::Case(case) = segment
                                            && super::membership_on_every_path(
                                                function, structural_types, predecessors,
                                                (block_position, node_index as usize), parent.place,
                                                &path, *case, false,
                                            )
                                        {
                                            return true;
                                        }
                                        path.push(segment.clone());
                                    }
                                    false
                                },
                            ).map_err(invalid)?;
                                }
                                continue;
                            }
                            let additions = result
                                .qualifications
                                .iter()
                                .map(|domain| (&[][..], domain))
                                .chain(
                                    result
                                        .projected_qualifications
                                        .iter()
                                        .map(|row| (row.path.as_slice(), &row.domain)),
                                );
                            for (path, domain) in additions {
                                let (supplied, _) =
                                    terminal_psi::StructuralPathQualification::select(
                                        &contract.qualifications,
                                        &contract.projected_qualifications,
                                        path,
                                    );
                                if structural_domains.get(domain).is_none_or(|declaration| {
                                    declaration
                                        .missing_membership_requirements(
                                            &unit.structural_domains,
                                            &supplied,
                                        )
                                        .is_none_or(|missing| {
                                            missing
                                                .iter()
                                                .any(|node| node.establishment_routes.is_some())
                                        })
                                }) {
                                    return Err(invalid(
                                        "transfer adds a membership with unknown or unsupplied issuer requirements",
                                    ));
                                }
                            }
                        }
                        O::StructuralLeafCopy {
                            psi_operation,
                            result,
                            source,
                            path,
                        } => {
                            let owned_case_copy =
                                matches!(path, abstract_operations::StructuralCopyPath::Case(_))
                                    && result.multiplicity
                                        == terminal_psi::StructuralMultiplicity::Affine
                                    && structural_types.get(&result.structural_type).is_some_and(
                                        |declaration| {
                                            !matches!(
                                                declaration.shape,
                                                terminal_psi::StructuralTypeShape::Reference { .. }
                                            )
                                        },
                                    );
                            let shared_descriptor_copy = result.multiplicity
                                == terminal_psi::StructuralMultiplicity::Affine
                                && terminal_semantics::is_copyable_carrier(
                                    structural_types.values().copied(),
                                    result.structural_type,
                                );
                            let valid =
                        structural_source_contract(
                            function, *source,
)
                        .is_some_and(|signature| {
                            signature.access != StructuralAccess::WriteOnlyBorrow
                            && result.qualification_establishments.is_empty()
                            && match path {
                                abstract_operations::StructuralCopyPath::Ordinary(path)
                                    if path.is_empty() && signature.structural_type == result.structural_type =>
                                {
                                    signature.multiplicity == result.multiplicity
                                        && !signature.has_result_evidence()
                                }
                                abstract_operations::StructuralCopyPath::Ordinary(_) => true,
                                abstract_operations::StructuralCopyPath::Case(_) => {
                                    result.qualifications.is_empty()
                                }
                            }
                            && match path {
                                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                                    let expected = result.structural_type;
                                    let selected = crate::unit_validation::structural_catalog::resolve_leaf_copy_path(structural_types, signature.structural_type, path, expected);
                                    selected == Some(expected)
                                        || selected.is_some_and(|selected| terminal_semantics::shared_descriptor_copy_matches(
                                            structural_types.values().copied(), selected,
                                            path.is_empty().then_some(signature.access), expected,
                                            result.multiplicity,
                                        ))
                                        || (path.is_empty() && abstract_operations::quotients::licensed_copy(
                                            &unit.structural_types, function.machine, *psi_operation, *source,
                                            signature.structural_type, result,
                                            function.block_nodes().map(|node| &node.operation)))
                                        || crate::unit_validation::structural_catalog::byte_carrier_leaf_window(structural_types, signature.structural_type, path, expected)
                                },
                                abstract_operations::StructuralCopyPath::Case(path) => {
                                    let expected = result.structural_type;
                                    terminal_semantics::canonical_case_leaf_type(structural_types.values().copied(), signature.structural_type, path) == Some(expected)
                                }
                            }
                        }) && (result.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                            || owned_case_copy
                            || shared_descriptor_copy)
                            && copy_memberships_match(
                                unit,
                                function,
                                block.id,
                                node_index,
                                *source,
                                path,
                                result,
                                view_graph,
                            )
                            && result.claims.is_empty()
                            && place_kinds_operation_result(&place_kinds, result, *psi_operation);
                            if !valid {
                                return Err(catalog_mismatch(function));
                            }
                        }
                        O::ReturnStructural { source, .. } => {
                            let Some(signature) = function.result.structural() else {
                                return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch(
                                OperationSite::new(function.machine, block.id, node_index),
                            ),
                        );
                            };
                            let source_contract = function
                                .structural_parameters
                                .iter()
                                .find(|parameter| {
                                    parameter.place == *source
                                        && matches!(
                                            place_kinds.get(source),
                                            Some(StructuralPlaceKind::Parameter { position })
                                                if *position == parameter.position
                                        )
                                })
                                .or_else(|| {
                                    let Some(StructuralPlaceKind::BlockParameter {
                                        block,
                                        position,
                                    }) = place_kinds.get(source).copied()
                                    else {
                                        return None;
                                    };
                                    function
                                        .block(block)
                                        .and_then(|block| {
                                            block.structural_parameters.get(position as usize)
                                        })
                                        .filter(|parameter| {
                                            parameter.place == *source
                                                && parameter.position == position
                                                && (parameter.access == StructuralAccess::Owned
                                                    || shared_descriptor_result)
                                        })
                                })
                                .map(|parameter| {
                                    (
                                        parameter.structural_type,
                                        parameter.multiplicity,
                                        parameter.qualifications.as_slice(),
                                        parameter.projected_qualifications.as_slice(),
                                        BTreeSet::new(),
                                    )
                                })
                                .or_else(|| {
                                    let Some(StructuralPlaceKind::ByteSequenceLiteral {
                                        structural_type,
                                        ..
                                    }) = place_kinds.get(source).copied()
                                    else {
                                        return None;
                                    };
                                    Some((
                                        structural_type,
                                        terminal_psi::StructuralMultiplicity::Unrestricted,
                                        &[][..],
                                        &[][..],
                                        BTreeSet::new(),
                                    ))
                                })
                                .or_else(|| {
                                    let Some(StructuralPlaceKind::OperationResult {
                                        producer,
                                        structural_type,
                                    }) = place_kinds.get(source).copied()
                                    else {
                                        return None;
                                    };
                                    function.block_nodes().find_map(|node| {
                                        let (actual_producer, result) =
                                            node.operation.structural_result()?;
                                        if actual_producer != producer
                                            || result.place != *source
                                            || result.structural_type != structural_type
                                        {
                                            return None;
                                        }
                                        if !shared_descriptor_result
                                            && structural_source_contract(function, *source)
                                                .is_none_or(|source| {
                                                    source.access != StructuralAccess::Owned
                                                })
                                        {
                                            return None;
                                        }
                                        Some((
                                            result.structural_type,
                                            result.multiplicity,
                                            result.qualifications.as_slice(),
                                            result.projected_qualifications.as_slice(),
                                            result
                                                .qualification_establishments
                                                .iter()
                                                .map(|binding| binding.domain)
                                                .collect::<BTreeSet<_>>(),
                                        ))
                                    })
                                });
                            let exact_bounded_owned_bytes = structural_types
                                .get(&signature.structural_type)
                                .is_some_and(|declaration| {
                                    declaration.shape.is_bounded_owned_byte_sequence()
                                });
                            if !descriptor_origins_valid
                        || source_contract.is_none_or(
                            |(structural_type, multiplicity, qualifications, projected, minted)| {
                                structural_type != signature.structural_type
                                    || multiplicity != signature.multiplicity
                                    || if exact_bounded_owned_bytes {
                                        !minted.is_empty()
                                            || qualifications != signature.qualifications
                                            || projected != signature.projected_qualifications
                                    } else {
                                        !qualifications
                                            .iter()
                                            .filter(|domain| !minted.contains(domain))
                                            .all(|domain| signature.qualifications.contains(domain))
                                            || !terminal_semantics::reference_result_memberships_weaken(
                                                unit.structural_types.iter(), signature.structural_type,
                                                &projected.iter()
                                                    .filter(|projection| !minted.contains(&projection.domain))
                                                    .cloned().collect::<Vec<_>>(),
                                                &signature.projected_qualifications,
                                            )
                                    }
                            },
                        )
                    {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnSourceContractMismatch(
                                OperationSite::new(function.machine, block.id, node_index),
                            ),
                        );
                    }
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        }

        fn observed_scalar_field(
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
            path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
            field: StructuralFieldId,
            access: StructuralAccess,
        ) -> Option<ScalarType> {
            let access = terminal_semantics::observed_field_backing_access(
                structural_types.values().copied(),
                root,
                path,
                access,
            )?;
            let declaration = terminal_semantics::observed_field_declaration(
                structural_types.values().copied(),
                root,
                path,
                field,
            )?;
            if matches!(
                declaration.field_type,
                terminal_psi::StructuralFieldType::Atomic(_)
                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
            ) && !matches!(
                access,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            ) {
                return None;
            }
            relevant_scalar_field(std::slice::from_ref(declaration), field, true)
        }

        fn relevant_scalar_field(
            fields: &[terminal_psi::StructuralFieldDeclaration],
            field: StructuralFieldId,
            allow_refined_scalar: bool,
        ) -> Option<ScalarType> {
            fields.iter().find_map(|candidate| {
                (candidate.id == field && !candidate.relevance.is_erased())
                    .then_some(&candidate.field_type)
                    .and_then(|field_type| match field_type {
                        terminal_psi::StructuralFieldType::Scalar(scalar_type) => {
                            Some(*scalar_type)
                        }
                        terminal_psi::StructuralFieldType::Atomic(_)
                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_) => {
                            field_type.atomic_scalar_type()
                        }
                        terminal_psi::StructuralFieldType::IeeeFloat(format) => {
                            Some(ScalarType::IeeeFloat(*format))
                        }
                        terminal_psi::StructuralFieldType::BoundedInteger(bounds)
                            if allow_refined_scalar =>
                        {
                            Some(ScalarType::Integer(bounds.integer_type()))
                        }
                        terminal_psi::StructuralFieldType::RefinedScalar(refinement)
                            if allow_refined_scalar =>
                        {
                            Some(refinement.carrier())
                        }
                        _ => None,
                    })
            })
        }

        fn readable_field_type(
            function: &PsiOptimizationFunction,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            place: PlaceId,
        ) -> Option<(StructuralTypeId, StructuralAccess)> {
            let signature = structural_source_contract(function, place)?;
            if signature.access == StructuralAccess::WriteOnlyBorrow
                || (!signature.is_unqualified()
                    && !structural_types
                        .get(&signature.structural_type)
                        .is_some_and(|declaration| declaration.shape.is_composite_shape()))
                || function.content_entry_claim_declared(place)
            {
                return None;
            }
            if signature.access == StructuralAccess::Owned
                && structural_types
                    .get(&signature.structural_type)
                    .is_some_and(|declaration| (declaration.shape).is_composite_shape())
            {
                return Some((signature.structural_type, signature.access));
            }
            if signature.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || function.entry_claim_declared(place)
                || function.block_nodes().any(|node| {
                    let result = match &node.operation {
                        O::EstablishRecord { result, .. }
                        | O::EstablishZeroedStructural { result, .. }
                        | O::EstablishScalarArray { result, .. }
                        | abstract_operations::structural_array_or_case_pattern!(result, ..)
                        | O::EstablishReference { result, .. }
                        | O::MoveStructuralField { result, .. }
                        | O::CallStructural {
                            result: AbstractStructuralCallResult::Structural { result, .. },
                            ..
                        }
                        | O::BoundaryCall {
                            result: abstract_operations::AbstractBoundaryResult::Structural(result),
                            ..
                        } => result,
                        _ => return false,
                    };
                    result.place == place && !result.claims.is_empty()
                })
            {
                return None;
            }
            Some((signature.structural_type, signature.access))
        }

        fn borrowed_window_root(
            function: &PsiOptimizationFunction,
            root: &StructuralWindowRoot,
            place_kinds: &BTreeMap<PlaceId, StructuralPlaceKind>,
            required_access: StructuralAccess,
        ) -> bool {
            match root {
                StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
                    parameter.access == StructuralAccess::Owned
                        && place_kinds.get(&parameter.place)
                            == Some(&StructuralPlaceKind::BlockParameter {
                                block: *block,
                                position: parameter.position,
                            })
                        && function.blocks.iter().any(|candidate| {
                            candidate.id == *block
                                && candidate.structural_parameters.contains(parameter)
                        })
                }
                StructuralWindowRoot::Parameter(parameter) => {
                    function
                        .structural_parameters
                        .iter()
                        .chain(function.block_structural_parameters())
                        .find(|candidate| candidate.place == parameter.place)
                        == Some(parameter)
                        && ((parameter.access == StructuralAccess::Owned
                            && function.structural_parameters.contains(parameter))
                            || required_access.admits_referent(parameter.access))
                        && matches!(
                            parameter.multiplicity,
                            terminal_psi::StructuralMultiplicity::Unrestricted
                                | terminal_psi::StructuralMultiplicity::Affine
                        )
                        && !function.entry_claim_declared(parameter.place)
                        && !function.content_entry_claim_declared(parameter.place)
                        && match place_kinds.get(&parameter.place) {
                            Some(StructuralPlaceKind::Parameter { position }) => {
                                *position == parameter.position
                                    && function.structural_parameters.contains(parameter)
                            }
                            Some(StructuralPlaceKind::BlockParameter { block, position }) => {
                                *position == parameter.position
                                    && function.blocks.iter().any(|candidate| {
                                        candidate.id == *block
                                            && candidate.structural_parameters.contains(parameter)
                                    })
                            }
                            _ => false,
                        }
                }
                StructuralWindowRoot::OwnedResult { producer, result } => {
                    let producer_admits =
                function.block_nodes()
                    .any(|node| {
                        node.operation.psi_operation() == Some(*producer)
                            && crate::unit_validation::operation_contracts::structural_operation_result_contract(
                                function, &node.operation,
                            )
                            .is_some_and(|source| {
                                source.access == StructuralAccess::Owned
                                    && source.operation_result() == Some(result)
                            })
                    });
                    producer_admits && place_kinds_operation_result(place_kinds, result, *producer)
                }
            }
        }

        fn root_window_loan(
            root: &StructuralWindowRoot,
            path: &[StructuralPathSegment],
            field: Option<StructuralFieldId>,
        ) -> bool {
            terminal_semantics::structural_window_selects_child(path, field)
                || path.contains(&StructuralPathSegment::Referent)
                || (path.is_empty() && root.parameter().is_some())
        }

        fn window_field_type(
            function: &PsiOptimizationFunction,
            structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: &StructuralWindowRoot,
            path: &[StructuralPathSegment],
            field: Option<StructuralFieldId>,
            required_access: StructuralAccess,
        ) -> Option<StructuralTypeId> {
            let (root, path) = if path.contains(&StructuralPathSegment::Referent) {
                crate::unit_validation::references::record_field_destination(
                    function,
                    structural_types,
                    root.place(),
                    path,
                    required_access,
                )?
            } else {
                (root.structural_type(), path)
            };
            structural_window_projection(structural_types.values().copied(), root, path, field)
                .map(|projection| projection.structural_type)
        }

        fn window_memberships(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: &StructuralWindowRoot,
            path: &[StructuralPathSegment],
            field: Option<StructuralFieldId>,
        ) -> Option<(
            Vec<StructuralDomainId>,
            Vec<terminal_psi::StructuralPathQualification>,
        )> {
            let source = structural_source_contract(function, root.place())?;
            if source.structural_type != root.structural_type() {
                return None;
            }
            let projection = structural_window_projection(
                types.values().copied(),
                source.structural_type,
                path,
                field,
            )?;
            source.restorable_qualifications(&projection.path)
        }

        fn reference_result_memberships_match(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            operation: &O,
        ) -> bool {
            let O::CallStructural {
                callee,
                structural_arguments,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } = operation
            else {
                return true;
            };
            let Some(callee) = callee.machine().and_then(|machine| unit.function(machine)) else {
                return true;
            };
            let Some(signature) = callee.result.structural() else {
                return false;
            };
            if result
                .projected_qualifications
                .iter()
                .all(|row| signature.projected_qualifications.contains(row))
            {
                return true;
            }
            let preserved = (|| {
                let graph = crate::descriptor_results::optimized_borrowed_storage_catalog_graph(
                    unit.functions.iter(),
                    &unit.boundary_machines,
                    &unit.structural_types,
                    &unit.dynamic_dispatch,
                )?;
                let mappings = terminal_semantics::bind_reference_call_results(
                    unit.structural_types.iter(),
                    terminal_semantics::ReferenceCallContract::Machine {
                        parameters: &callee.structural_parameters,
                        sources: &signature.reference_sources,
                    },
                    structural_arguments,
                )
                .ok()?;
                terminal_semantics::reference_call_preserved_memberships(
                    &unit.structural_types,
                    &unit.structural_domains,
                    &graph,
                    function.machine,
                    (block, node),
                    result,
                    signature,
                    &mappings,
                    &|place| {
                        structural_source_contract(function, place)
                            .map(|contract| contract.payload_contract())
                    },
                )
                .ok()
            })();
            preserved.is_some_and(|preserved| {
                result.projected_qualifications.iter().all(|row| {
                    signature.projected_qualifications.contains(row) || preserved.contains(row)
                })
            })
        }

        fn view_memberships_match(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            operation: &O,
            view_graph: Option<&[DescriptorMachine]>,
        ) -> bool {
            let (source, result) = match operation {
                O::EstablishElementView { source, result, .. }
                | O::ElementViewSubslice { source, result, .. } => (source, result),
                _ => return true,
            };
            if result.projected_qualifications.is_empty() {
                return true;
            }
            let evidence = (|| {
                let terminal_psi::StructuralTypeShape::ElementView { element, .. } = unit
                    .structural_types
                    .iter()
                    .find(|row| row.id == result.structural_type)?
                    .shape
                else {
                    return None;
                };
                let graph = view_graph?;
                let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
                    &unit.structural_types,
                    graph,
                );
                terminal_semantics::descriptor_results::element_payload_memberships_in(
                    &unit.structural_types,
                    &unit.structural_domains,
                    function.machine,
                    &mut analysis,
                    (block, node),
                    source,
                    element,
                    |place| {
                        structural_source_contract(function, place)
                            .map(|contract| contract.payload_contract())
                    },
                )
            })();
            match evidence {
                None => false,
                Some(terminal_semantics::descriptor_results::ElementPayloadMemberships::Empty) => {
                    true
                }
                Some(
                    terminal_semantics::descriptor_results::ElementPayloadMemberships::Present(
                        contract,
                    ),
                ) => {
                    if result.projected_qualifications.iter().any(|row| {
                        !matches!(row.path.first(), Some(StructuralPathSegment::AllElements))
                    }) {
                        return false;
                    }
                    let (whole, projected) = terminal_psi::StructuralPathQualification::select(
                        &[],
                        &result.projected_qualifications,
                        &[StructuralPathSegment::AllElements],
                    );
                    whole
                        .iter()
                        .all(|domain| contract.qualifications.contains(domain))
                        && projected
                            .iter()
                            .all(|row| contract.projected_qualifications.contains(row))
                }
            }
        }

        fn copy_memberships_match(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            block: BlockId,
            node: u32,
            source: PlaceId,
            path: &abstract_operations::StructuralCopyPath,
            result: &StructuralOperationResult,
            view_graph: Option<&[DescriptorMachine]>,
        ) -> bool {
            if result.qualifications.is_empty() && result.projected_qualifications.is_empty() {
                return true;
            }
            let abstract_operations::StructuralCopyPath::Ordinary(path) = path else {
                return false;
            };
            let memberships = (|| {
                let graph = view_graph?;
                let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
                    &unit.structural_types,
                    graph,
                );
                let root = structural_source_contract(function, source)?;
                let query = terminal_semantics::descriptor_results::descriptor_contents_query(
                    &unit.structural_types,
                    source,
                    root.structural_type,
                    path,
                    None,
                )?;
                let position = (block, node);
                let targets = analysis.payload_targets(function.machine, position, &query)?;
                terminal_semantics::descriptor_results::payload_memberships(
                    &unit.structural_types,
                    &unit.structural_domains,
                    targets,
                    |place| {
                        terminal_semantics::descriptor_results::current_payload_contract(
                            &unit.structural_types,
                            &unit.structural_domains,
                            graph,
                            function.machine,
                            position,
                            place,
                            &|source| {
                                structural_source_contract(function, source)
                                    .map(|contract| contract.payload_contract())
                            },
                        )
                        .ok()
                    },
                )
            })();
            memberships.is_some_and(|contract| {
                contract.carrier == result.structural_type
                    && result
                        .qualifications
                        .iter()
                        .all(|domain| contract.qualifications.contains(domain))
                    && result
                        .projected_qualifications
                        .iter()
                        .all(|row| contract.projected_qualifications.contains(row))
            })
        }
    }
    pub(crate) use structural_roots::{
        ReferenceMembershipValidation, operation_place_accesses, operation_place_inputs,
    };
    mod structural_transfers {
        use super::super::operation_contracts::structural_source_contract;
        use crate::OptimizationUnitValidationError;
        use crate::unit_validation::structural_catalog::shared_view_declaration;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{
            PlaceId, StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
        };
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{
            StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralTypeShape,
        };
        use terminal_semantics::view_descriptor_parameter;

        pub(super) fn validate(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let invalid =
                || OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch;
            let mut places = function
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect::<BTreeSet<_>>();
            for block in &function.blocks {
                if block.id == function.entry && !block.structural_parameters.is_empty() {
                    return Err(invalid());
                }
                for (position, parameter) in block.structural_parameters.iter().enumerate() {
                    if parameter.position as usize != position
                        || !places.insert(parameter.place)
                        || (!plain(parameter)
                            && !qualified_owned_arrival(parameter, types, domains)
                            && !address_join(parameter, types)
                            && !(parameter.qualifications.is_empty()
                                && view_descriptor_parameter(parameter, types.values().copied()))
                            && !owner_loan(parameter, types)
                            && !qualified_owned_bytes(parameter, types)
                            && !qualified_shared_view(parameter, types)
                            && !shared_view_carrier(parameter, types))
                        || !types
                            .get(&parameter.structural_type)
                            .is_some_and(|declaration| {
                                parameter.access == StructuralAccess::Owned
                                    || address_join(parameter, types)
                                    || shared_view_carrier(parameter, types)
                                    || declaration.shape.is_borrowed_byte_carrier()
                                    || matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ElementView { .. }
                                    )
                            })
                        || !function.structural_places.iter().any(|place| {
                            place.id == parameter.place
                                && place.kind
                                    == (StructuralPlaceKind::BlockParameter {
                                        block: block.id,
                                        position: parameter.position,
                                    })
                        })
                        || function.entry_claim_declared(parameter.place)
                        || function.content_entry_claim_declared(parameter.place)
                    {
                        return Err(invalid());
                    }
                }
            }
            for block in &function.blocks {
                for edge in block.successors() {
                    let destination = function.block(edge.target).ok_or(
                        OptimizationUnitValidationError::UnknownSuccessor {
                            machine: function.machine,
                            block: block.id,
                            target: edge.target,
                        },
                    )?;
                    if edge.structural_bindings.len() != destination.structural_parameters.len() {
                        return Err(invalid());
                    }
                    for (binding, parameter) in edge
                        .structural_bindings
                        .iter()
                        .zip(&destination.structural_parameters)
                    {
                        let source = source_contract(function, binding.argument.place, types);
                        if parameter.access != StructuralAccess::Owned
                            && types
                                .get(&parameter.structural_type)
                                .is_some_and(|declaration| {
                                    terminal_semantics::descriptor_results::is_descriptor(
                                        types.values().copied(),
                                        declaration.id,
                                    )
                                })
                            && !source.is_some_and(|(root, access, multiplicity)| {
                                terminal_semantics::descriptor_source_projection_accessible(
                                    types.values().copied(),
                                    root,
                                    multiplicity,
                                    access,
                                    &binding.argument.path,
                                    parameter.access,
                                )
                            })
                        {
                            return Err(invalid());
                        }
                        let borrowed_address = address_join(parameter, types);
                        let exact_source = if qualified_owned_arrival(parameter, types, domains) {
                            structural_source_contract(
                        function, binding.argument.place,
                    ).is_some_and(|source| {
                        source.access == StructuralAccess::Owned
                            && binding.argument.access == StructuralAccess::Owned
                            && if binding.argument.path.is_empty() {
                                source.structural_type == parameter.structural_type
                                    && source.multiplicity == parameter.multiplicity
                            } else {
                                source.multiplicity == StructuralMultiplicity::Affine
                                    && matches!(parameter.multiplicity,
                                        StructuralMultiplicity::Affine | StructuralMultiplicity::Linear)
                                    && crate::unit_validation::structural_catalog::resolve_structural_path(
                                        types, source.structural_type, &binding.argument.path,
                                    ) == Some(parameter.structural_type)
                            }
                            && source.supplies_qualifications(
                                &binding.argument.path,
                                &parameter.qualifications,
                                &parameter.projected_qualifications,
                            )
                    })
                        } else if qualified_shared_view(parameter, types) {
                            qualified_shared_view_source(function, &binding.argument, parameter)
                        } else if owner_loan(parameter, types)
                            || qualified_owned_bytes(parameter, types)
                        {
                            let forwarded = binding.argument.path.is_empty()
                                && function
                                    .structural_parameters
                                    .iter()
                                    .chain(function.block_structural_parameters())
                                    .any(|source| {
                                        source.place == binding.argument.place
                                            && source.structural_type == parameter.structural_type
                                            && source.access == parameter.access
                                            && source.multiplicity == parameter.multiplicity
                                            && source.qualifications == parameter.qualifications
                                            && source.projected_qualifications.is_empty()
                                    });
                            let copied = binding.argument.path.is_empty()
                                && binding.argument.access == StructuralAccess::Owned
                                && function.block_nodes().any(|node| match &node.operation {
                                    O::StructuralLeafCopy {
                                        source: backing,
                                        path:
                                            abstract_operations::StructuralCopyPath::Ordinary(path),
                                        result,
                                        ..
                                    } => {
                                        result.place == binding.argument.place
                                            && result.structural_type == parameter.structural_type
                                            && result.multiplicity
                                                == StructuralMultiplicity::Unrestricted
                                            && result.qualifications.is_empty()
                                            && result.projected_qualifications.is_empty()
                                            && result.claims.is_empty()
                                            && function.arrival_parameter(*backing).is_some_and(
                                                |root| {
                                                    byte_leaf_predicates(
                                                        types,
                                                        domains,
                                                        root.structural_type,
                                                        path,
                                                        parameter,
                                                    )
                                                },
                                            )
                                    }
                                    _ => false,
                                });
                            let projected = !binding.argument.path.is_empty()
                                && function
                                    .arrival_parameter(binding.argument.place)
                                    .is_some_and(|root| {
                                        byte_leaf_predicates(
                                            types,
                                            domains,
                                            root.structural_type,
                                            &binding.argument.path,
                                            parameter,
                                        )
                                    });
                            let owned_result =
                                structural_source_contract(function, binding.argument.place)
                                    .is_some_and(|source| {
                                        super::super::operation_contracts::owned_bounded_owner_loan(
                                            function,
                                            &binding.argument,
                                            parameter,
                                            &source,
                                            types,
                                            domains,
                                        )
                                    });
                            forwarded || copied || projected || owned_result
                        } else if borrowed_address {
                            abstract_operations::control_flow::address_joins::is_address_projection(
                        &binding.argument.path,
                    ) && structural_source_contract(
                        function, binding.argument.place,
                    ).is_some_and(|source| {
                        let Some(access) = source.backing_access(types.values().copied()) else { return false; };
                        (if matches!(
                            parameter.access,terminal_psi::exclusive_borrow_pair_pattern!()) {
                            super::super::operation_contracts::structural_access_can_supply(
                                access,
                                parameter.access,
                            ) && access != StructuralAccess::Owned
                                || access == StructuralAccess::Owned
                                    && function.structural_places.iter().any(|place| {
                                        place.id == binding.argument.place
                                            && matches!(
                                                place.kind,
                                                StructuralPlaceKind::OperationResult { .. }
                                            )
                                    })
                        } else {
                            access != StructuralAccess::WriteOnlyBorrow
                        }) && terminal_semantics::reference_source_projection_accessible(
                                types.values().copied(), source.structural_type, source.multiplicity,
                                access, &binding.argument.path, parameter.access,
                            )
                            && source.borrowed_qualifications_match(&binding.argument.path, parameter)
                            && abstract_operations::control_flow::address_joins::borrowed_referent_type(
                                types.values().copied(),
                                source.structural_type,
                                &binding.argument.path,
                                parameter.access,
                            ) == Some(parameter.structural_type)
                    }) && structural_source_contract(function, binding.argument.place)
                        .is_some_and(|source| source.supplies_qualifications(
                            &binding.argument.path,
                            &parameter.qualifications,
                            &parameter.projected_qualifications,
                        ))
                        } else if shared_view_carrier(parameter, types) {
                            binding.argument.path.is_empty()
                                && source.is_some_and(|(root_type, _, multiplicity)| {
                                    root_type == parameter.structural_type
                                        && multiplicity != StructuralMultiplicity::Linear
                                })
                        } else if view_descriptor_parameter(parameter, types.values().copied())
                            && parameter.qualifications.is_empty()
                            && !binding.argument.path.is_empty()
                        {
                            source.is_some_and(|(root_type, access, multiplicity)| {
                        multiplicity != StructuralMultiplicity::Linear
                            && super::super::operation_contracts::structural_access_can_supply(access, parameter.access)
                            && abstract_operations::control_flow::address_joins::borrowed_referent_type(
                                types.values().copied(), root_type, &binding.argument.path, parameter.access,
                            ) == Some(parameter.structural_type)
                    })
                        } else if !binding.argument.path.is_empty() {
                            parameter.access == StructuralAccess::Owned
                        && source.is_some_and(|(root_type, access, multiplicity)| {
                            access == StructuralAccess::Owned
                                && (multiplicity == StructuralMultiplicity::Affine
                                    && matches!(parameter.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Linear)
                                    || multiplicity == StructuralMultiplicity::Unrestricted
                                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted)
                                &&
                            crate::unit_validation::structural_catalog::resolve_structural_path(
                                types,
                                root_type,
                                &binding.argument.path,
                            ) == Some(parameter.structural_type)
                                && structural_source_contract(
                                    function, binding.argument.place,
                                ).is_some_and(|source| {
                                    let (whole, projected) = source.selected_qualifications(&binding.argument.path);
                                    parameter.qualifications.iter().all(|domain| whole.contains(domain))
                                        && parameter.projected_qualifications.iter().all(|row| projected.contains(row))
                                })
                        })
                        } else {
                            source.is_some_and(|(source_type, access, multiplicity)| {
                                match parameter.access {
                                    StructuralAccess::Owned => source_type == parameter.structural_type
                                        && access == StructuralAccess::Owned
                                        && multiplicity == parameter.multiplicity,StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                                        let presentation = terminal_semantics::borrowed_referent_presentation(
                                            types.values().copied(), source_type, parameter.structural_type);
                                        let lends_backing = matches!(presentation,
                                            Some(terminal_semantics::BorrowedReferentPresentation::AddressOfBacking { .. }));
                                        presentation.is_some()
                                            && (access == StructuralAccess::Owned && lends_backing
                                                || access != StructuralAccess::Owned
                                                    && super::super::operation_contracts::structural_access_can_supply(access, parameter.access))
                                            && multiplicity != StructuralMultiplicity::Linear
                                    },
                                }
                        })
                        };
                        let claim_hit = function.entry_claim_declared(binding.argument.place)
                            || function.content_entry_claim_declared(binding.argument.place);
                        if binding.parameter != parameter.place
                            || binding.argument.access != parameter.access
                            || !exact_source
                            || (claim_hit
                                && parameter.access != StructuralAccess::Owned
                                && !borrowed_address)
                        {
                            return Err(invalid());
                        }
                    }
                }
            }
            Ok(())
        }

        fn qualified_owned_arrival(
            parameter: &StructuralParameterDeclaration,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
        ) -> bool {
            parameter.access == StructuralAccess::Owned
        && !qualified_owned_bytes(parameter, types)
        && types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Record { .. }
                        | StructuralTypeShape::Sum { .. }
                        | StructuralTypeShape::Mixed { .. }
                        | StructuralTypeShape::FixedArray { .. }
                )
            })
        && (!parameter.qualifications.is_empty() || !parameter.projected_qualifications.is_empty())
        && crate::unit_validation::structural_catalog::structural_qualifications_match(
            parameter.structural_type,
            &parameter.qualifications,
            domains,
        )
        && crate::unit_validation::structural_catalog::structural_projected_qualifications_match(
            parameter.structural_type,
            &parameter.projected_qualifications,
            types,
            domains,
        )
        }

        fn byte_leaf_predicates(
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
            root_type: StructuralTypeId,
            path: &[terminal_psi::StructuralPathSegment],
            parameter: &StructuralParameterDeclaration,
        ) -> bool {
            let Some((terminal_psi::StructuralPathSegment::Field(identity), prefix)) =
                path.split_last()
            else {
                return false;
            };
            let Some(field) = terminal_semantics::runtime_structural_path_tip(
                types.values().copied(),
                root_type,
                prefix,
            )
            .and_then(|parent| match &parent.shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                    terminal_psi::live_field_by_identity(fields, *identity)
                }
                _ => None,
            }) else {
                return false;
            };
            field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|carrier| {
                    types
                        .get(&parameter.structural_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(declared) if declared == carrier
                            )
                        })
                })
                && terminal_semantics::byte_qualification_predicates_in(
                    domains.values().copied(),
                    parameter.structural_type,
                    &parameter.qualifications,
                )
                .is_some_and(|required| field.field_type.byte_predicates().implies(required))
        }

        fn address_join(
            parameter: &StructuralParameterDeclaration,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            abstract_operations::control_flow::address_joins::is_address_join(
                parameter,
                |identity| types.get(&identity).copied(),
            )
        }

        fn owner_loan(
            parameter: &StructuralParameterDeclaration,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            abstract_operations::control_flow::owner_loans::is_bounded_owner_loan(
                parameter,
                |identity| types.get(&identity).copied(),
            )
        }

        fn shared_view_carrier(
            parameter: &StructuralParameterDeclaration,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan(
                parameter,
                |identity| types.get(&identity).copied(),
                &types.values().map(|row| (*row).clone()).collect::<Vec<_>>(),
            )
        }

        fn qualified_owned_bytes(
            parameter: &StructuralParameterDeclaration,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            !parameter.qualifications.is_empty()
                && abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
                    parameter,
                    |identity| types.get(&identity).copied(),
                )
        }

        fn qualified_shared_view(
            parameter: &StructuralParameterDeclaration,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            !parameter.qualifications.is_empty()
                && abstract_operations::control_flow::owner_loans::is_shared_byte_view(
                    parameter,
                    |identity| types.get(&identity).copied(),
                )
        }

        fn qualified_shared_view_source(
            function: &PsiOptimizationFunction,
            argument: &terminal_psi::StructuralArgument,
            parameter: &StructuralParameterDeclaration,
        ) -> bool {
            if !argument.path.is_empty() {
                return false;
            }
            let forwarded = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .any(|source| {
                    source.place == argument.place
                        && source.structural_type == parameter.structural_type
                        && source.access == parameter.access
                        && source.multiplicity == parameter.multiplicity
                        && source.qualifications == parameter.qualifications
                        && source.projected_qualifications.is_empty()
                });
            let literal = || {
                function.block_nodes().any(|node| match &node.operation {
                    O::EstablishByteSequenceLiteral {
                        place: declaration,
                        qualifications,
                        ..
                    } => {
                        declaration.id == argument.place
                            && *qualifications == parameter.qualifications
                            && matches!(
                                declaration.kind,
                                StructuralPlaceKind::ByteSequenceLiteral { structural_type, .. }
                                    if structural_type == parameter.structural_type
                            )
                    }
                    _ => false,
                })
            };
            forwarded || literal()
        }

        fn plain(parameter: &StructuralParameterDeclaration) -> bool {
            (match parameter.access {
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow => {
                    parameter.multiplicity == StructuralMultiplicity::Unrestricted
                }
                StructuralAccess::Owned => matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Unrestricted
                        | StructuralMultiplicity::Affine
                        | StructuralMultiplicity::Linear
                ),
            }) && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        }

        fn source_contract(
            function: &PsiOptimizationFunction,
            place: PlaceId,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> Option<(StructuralTypeId, StructuralAccess, StructuralMultiplicity)> {
            if let Some(parameter) = function
                .structural_parameters
                .iter()
                .chain(function.block_structural_parameters())
                .find(|parameter| parameter.place == place)
            {
                return (plain(parameter)
                    || parameter.qualifications.is_empty()
                        && view_descriptor_parameter(parameter, types.values().copied()))
                .then_some((
                    parameter.structural_type,
                    parameter.access,
                    parameter.multiplicity,
                ));
            }
            function
                .block_nodes()
                .find_map(|node| match &node.operation {
                    O::EstablishByteSequenceLiteral {
                        place: declaration, ..
                    } if declaration.id == place => match declaration.kind {
                        StructuralPlaceKind::ByteSequenceLiteral {
                            structural_type, ..
                        } => Some((
                            structural_type,
                            StructuralAccess::SharedBorrow,
                            StructuralMultiplicity::Unrestricted,
                        )),
                        _ => None,
                    },
                    operation => {
                        let source =
                    super::super::operation_contracts::structural_operation_result_contract(
                        function, operation,
                    )?;
                        let (_, result) = operation.structural_result()?;
                        if result.place != place
                            || (source.access != StructuralAccess::Owned
                                && (source.multiplicity == StructuralMultiplicity::Linear
                                    || !result.claims.is_empty()))
                            || !result.qualifications.is_empty()
                            || !result.projected_qualifications.is_empty()
                        {
                            return None;
                        }
                        let backing = source.backing_access(types.values().copied())?;
                        let access = if source.access == StructuralAccess::Owned
                            && types
                                .get(&source.structural_type)
                                .is_some_and(|row| shared_view_declaration(row))
                        {
                            backing
                        } else {
                            source.access
                        };
                        Some((source.structural_type, access, source.multiplicity))
                    }
                })
        }
    }
    pub(crate) use control_flow::ControlCyclePolicy;
    pub(crate) use fact_index::reconstruct_fact_index;

    pub(crate) fn validate_function(
        unit: &PsiOptimizationUnit,
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
        structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        structural_domains: &BTreeMap<
            StructuralDomainId,
            &terminal_psi::StructuralDomainDeclaration,
        >,
        cycle_policy: &control_flow::ControlCyclePolicy,
        call_accesses: &mut current_ownership::CallAccesses,
        descriptor_requirements: &mut Vec<
            terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement,
        >,
        ownership_frontiers: &mut Vec<optimization_unit::OwnershipFrontierFact>,
        reference_memberships: Option<&mut ReferenceMembershipValidation<'_>>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if let Some(subject) = function.nominal_cleanup_subject
            && (function.attachment != Some(subject)
                || !structural_types.contains_key(&subject)
                || functions
                    .values()
                    .filter(|candidate| candidate.nominal_cleanup_subject == Some(subject))
                    .count()
                    != 1)
        {
            return Err(catalog_mismatch(function));
        }
        if !valid_service_ceiling(&function.published_service_ceiling, services) {
            return Err(
                OptimizationUnitValidationError::InvalidFunctionServiceCeiling(function.machine),
            );
        }
        let (byte_sequence_literals, trivial_affine_locals) = validate_function_structural_catalog(
            function,
            &unit.identities,
            structural_types,
            structural_domains,
        )?;
        validate_provider_attachment_specialization(function, boundary_machines, structural_types)?;
        structural_roots::validate_structural_root_uniqueness(function)?;

        parameters::validate_entry_claim_index(function)?;
        let blocks = control_flow::index_blocks(function)?;
        parameters::validate_parameter_metadata(function)?;
        structural_transfers::validate(function, structural_types, structural_domains)?;
        let control_flow = control_flow::validate_nodes_and_edges(function, blocks)?;
        control_flow::validate_total_cfg(
            function,
            &control_flow.blocks,
            &control_flow.successors,
            cycle_policy.admits(function.machine),
        )?;
        results::validate_function_results(function)?;

        validate_byte_sequence_literal_witnesses(function, &byte_sequence_literals)?;
        validate_trivial_affine_local_witnesses(function, &trivial_affine_locals)?;
        structural_roots::validate_structural_place_availability(
            function,
            &control_flow.blocks,
            &control_flow.predecessors,
        )?;
        structural_roots::validate_structural_root_operations(
            unit,
            function,
            structural_types,
            structural_domains,
            &control_flow.predecessors,
            reference_memberships,
            call_accesses.view_graph(),
        )?;
        crate::unit_validation::references::validate_function_references(
            function,
            functions,
            structural_types,
            &unit.dynamic_dispatch,
        )?;
        byte_concatenation::validate(
            function,
            structural_types,
            structural_domains,
            &control_flow.predecessors,
        )?;
        byte_field_freshness::validate(function, structural_types, &control_flow.predecessors)?;
        descriptor_sources::validate_whole_freshness(
            function,
            structural_types,
            &control_flow.predecessors,
        )?;
        case_arguments::validate(function, structural_types, &control_flow.predecessors)?;
        byte_predicates::validate_indexed_byte_stores(
            unit,
            function,
            structural_types,
            &control_flow.predecessors,
        )?;
        mutable_views::validate(function, structural_types)?;
        provenance::validate_provenance_work_effects(function)?;
        fact_index::validate_fact_index(function)?;
        validate_values_and_bindings(
            unit,
            function,
            &control_flow.blocks,
            &control_flow.predecessors,
            functions,
            boundary_machines,
            services,
            structural_types,
            structural_domains,
            call_accesses,
        )?;
        validate_places_and_claims(function)?;
        let requirements = current_ownership::validate_current_ownership_frontier_collecting(
            unit,
            function,
            &control_flow.blocks,
            &control_flow.predecessors,
            functions,
            boundary_machines,
            structural_types,
            call_accesses,
            ownership_frontiers,
        )?;
        descriptor_requirements.extend(requirements);
        Ok(())
    }
}
mod identity_indexes {
    use crate::OptimizationUnitValidationError;
    use optimization_unit::{
        PsiOptimizationUnit, canonical_ownership_frontier_snapshot,
        recompute_psi_optimization_unit_identity,
    };
    use psi_work::WorkSchedule;

    pub(super) fn validate_identity_and_fact_indexes(
        unit: &PsiOptimizationUnit,
    ) -> Result<(), OptimizationUnitValidationError> {
        let recomputed = recompute_psi_optimization_unit_identity(unit);
        if unit.identity != recomputed {
            return Err(OptimizationUnitValidationError::ContentIdentityMismatch {
                stored: unit.identity,
                recomputed,
            });
        }
        if unit.work_schedule != WorkSchedule::CURRENT.identity() {
            return Err(OptimizationUnitValidationError::WrongWorkSchedule);
        }
        if unit
            .accepted_obligation_facts
            .iter()
            .any(|fact| fact.psi != unit.psi || !fact.has_canonical_identity())
            || unit.accepted_obligation_facts.windows(2).any(|pair| {
                (pair[0].machine, pair[0].owner, pair[0].obligation)
                    >= (pair[1].machine, pair[1].owner, pair[1].obligation)
            })
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
        }
        if unit.ownership_frontier_facts.iter().any(|fact| {
            fact.psi != unit.psi
                || !fact.has_canonical_identity()
                || !canonical_ownership_frontier_snapshot(&fact.snapshot)
        }) || unit
            .ownership_frontier_facts
            .windows(2)
            .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
        {
            return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
        }
        Ok(())
    }
}
mod no_normal_calls {
    use crate::OptimizationUnitValidationError;
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use semantic_vocabulary::{BlockId, MachineId};
    use std::collections::BTreeMap;
    use terminal_semantics::control_graph::{NormalCompletionDisposition, NormalCompletionGraph};

    struct CurrentCompletionGraph<'a> {
        functions: &'a BTreeMap<MachineId, &'a PsiOptimizationFunction>,
    }

    impl NormalCompletionGraph for CurrentCompletionGraph<'_> {
        type Error = &'static str;

        fn entry(&mut self, machine: MachineId) -> Result<BlockId, Self::Error> {
            self.functions
                .get(&machine)
                .map(|function| function.entry)
                .ok_or("no-normal invocation has no exact current callee")
        }

        fn disposition(
            &self,
            machine: MachineId,
            block: BlockId,
        ) -> Result<NormalCompletionDisposition, Self::Error> {
            let block = self
                .functions
                .get(&machine)
                .and_then(|function| {
                    function
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == block)
                })
                .ok_or("no-normal invocation has no exact current block")?;
            let node = block
                .nodes
                .last()
                .ok_or("no-normal invocation has no current terminator")?;
            Ok(match &node.operation {
                O::Return { .. } | O::ReturnUnit { .. } | O::ReturnStructural { .. } => {
                    NormalCompletionDisposition::Return
                }
                O::Crash { .. } | O::ExternalCompletion { .. } => NormalCompletionDisposition::Stop,
                O::InvokeNoNormal { callee, .. } => NormalCompletionDisposition::Invoke(*callee),
                O::Jump { .. } | O::Conditional { .. } | O::StructuralCase { .. } => {
                    NormalCompletionDisposition::Successors(
                        node.successors.iter().map(|edge| edge.target).collect(),
                    )
                }
                _ => {
                    return Err(
                        "no-normal invocation ends in an unclassified current control node",
                    );
                }
            })
        }
    }

    pub(super) fn validate(
        unit: &PsiOptimizationUnit,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    ) -> Result<(), OptimizationUnitValidationError> {
        let mut graph = CurrentCompletionGraph { functions };
        let mut admitted = Vec::new();
        for function in &unit.functions {
            for node in function.block_nodes() {
                let O::InvokeNoNormal { callee, .. } = &node.operation else {
                    continue;
                };
                if admitted.contains(callee) {
                    continue;
                }
                let invalid = |reason| OptimizationUnitValidationError::InvalidNoNormalInvocation {
                    machine: function.machine,
                    callee: *callee,
                    reason,
                };
                if terminal_semantics::control_graph::normal_completion_possible_in(
                    &mut graph, *callee,
                )
                .map_err(invalid)?
                {
                    return Err(invalid("current module callee may complete normally"));
                }
                admitted.push(*callee);
            }
        }
        Ok(())
    }
}
pub(crate) mod operation_contracts {
    use crate::OptimizationUnitValidationError;
    use crate::unit_validation::derived_metadata::dominators;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
    };
    use std::collections::{BTreeMap, BTreeSet};

    mod affine_calls {
        use crate::unit_validation::operation_contracts::plain_calls::plain_structural_call;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn exact_plain_affine_structural_call(
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            let Some(call) = plain_structural_call(operation, callee) else {
                return false;
            };
            let [parameter] = callee.structural_parameters.as_slice() else {
                return false;
            };
            let [argument] = call.structural_arguments else {
                return false;
            };
            let [block] = callee.blocks.as_slice() else {
                return false;
            };
            let [node] = block.nodes.as_slice() else {
                return false;
            };
            let O::ReturnStructural {
                source,
                returned_claims,
                trivial_affine_locals,
                cleanup_actions,
                ..
            } = &node.operation
            else {
                return false;
            };

            block.id == callee.entry
                && block.parameters.is_empty()
                && *source == parameter.place
                && returned_claims.is_empty()
                && trivial_affine_locals.is_empty()
                && cleanup_actions.is_empty()
                && callee.parameters.is_empty()
                && call.arguments.is_empty()
                && parameter.position == 0
                && parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && parameter.structural_type == call.signature.structural_type
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && argument.access == terminal_psi::StructuralAccess::Owned
                && argument.path.is_empty()
                && call.signature.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && call.signature.qualifications.is_empty()
                && call.signature.projected_qualifications.is_empty()
                && call.result.structural_type == call.signature.structural_type
                && call.result.multiplicity == call.signature.multiplicity
                && call.selected_evidence.is_empty()
                && callee.entry_claims.is_empty()
                && call.contract.outcome_specific_ensures.is_empty()
                && callee.published_service_ceiling.is_empty()
                && finite_owned_shape(
                    types,
                    call.signature.structural_type,
                    &mut BTreeSet::new(),
                    &mut BTreeSet::new(),
                )
        }

        fn finite_owned_shape(
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            root: StructuralTypeId,
            active: &mut BTreeSet<StructuralTypeId>,
            complete: &mut BTreeSet<StructuralTypeId>,
        ) -> bool {
            if complete.contains(&root) {
                return true;
            }
            if !active.insert(root) {
                return false;
            }
            let Some(declaration) = types.get(&root) else {
                return false;
            };
            let valid = match &declaration.shape {
                terminal_psi::StructuralTypeShape::Record { fields, .. } => fields
                    .iter()
                    .all(|field| finite_owned_field(types, active, complete, field)),
                terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } if *length > 0 => finite_owned_shape(types, *element, active, complete),
                terminal_psi::StructuralTypeShape::Mixed { fields, cases } => fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| case.fields.iter()))
                    .all(|field| finite_owned_field(types, active, complete, field)),
                _ => false,
            };
            active.remove(&root);
            if valid {
                complete.insert(root);
            }
            valid
        }

        fn finite_owned_field(
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            active: &mut BTreeSet<StructuralTypeId>,
            complete: &mut BTreeSet<StructuralTypeId>,
            field: &terminal_psi::StructuralFieldDeclaration,
        ) -> bool {
            !field.relevance.is_erased()
                && match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => {
                        finite_owned_shape(types, child, active, complete)
                    }
                    terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::Atomic(_)
                    | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                    | terminal_psi::StructuralFieldType::IeeeFloat(_)
                    | terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) => true,
                    _ => false,
                }
        }
    }
    mod atomic_coherence {
        use abstract_operations::{AbstractOperation, happens_before_atomic_coherence_violation};

        use crate::{OperationSite, OptimizationUnitValidationError};
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::BlockId;
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn validate_function_atomic_coherence(
            function: &PsiOptimizationFunction,
            structural_types: &BTreeMap<
                semantic_vocabulary::StructuralTypeId,
                &terminal_psi::StructuralTypeDeclaration,
            >,
            predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            dominators: &optimization_unit::DominatorSets,
        ) -> Result<(), OptimizationUnitValidationError> {
            let sequences: BTreeMap<BlockId, Vec<&AbstractOperation>> = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        block.nodes.iter().map(|node| &node.operation).collect(),
                    )
                })
                .collect();
            match happens_before_atomic_coherence_violation(
                &sequences,
                predecessors,
                |producer, consumer| {
                    dominators
                        .get(&consumer)
                        .is_some_and(|set| set.contains(&producer))
                },
                function.entry,
                |predecessor, successor, root| {
                    let predecessor = function.block(predecessor)?;
                    let target = function.block(successor)?;
                    let is_parameter = target.structural_parameter(root).is_some();
                    let mut mapped = None;
                    for edge in predecessor
                        .successors()
                        .filter(|edge| edge.target == successor)
                    {
                        if is_parameter
                            && edge
                                .structural_bindings
                                .iter()
                                .filter(|binding| binding.parameter == root)
                                .count()
                                != 1
                        {
                            return None;
                        }
                        let (source, path) =
                            crate::unit_validation::arrival_storage::projection_before_edge(
                                edge, root,
                            );
                        let before = (source, path.to_vec());
                        if mapped.as_ref().is_some_and(|earlier| earlier != &before) {
                            return None;
                        }
                        mapped = Some(before);
                    }
                    mapped
                },
                |left, left_path, right, right_path| {
                    abstract_operations::atomic::atomic_projections_may_alias(
                        &function.structural_places,
                        std::iter::once(function.structural_parameters.as_slice()).chain(
                            function
                                .blocks
                                .iter()
                                .map(|block| block.structural_parameters.as_slice()),
                        ),
                        structural_types.values().copied(),
                        left,
                        left_path,
                        right,
                        right_path,
                    )
                },
                |place, path, field| {
                    let root = optimization_unit::field_evidence(function, place)?.root_type?;
                    let member = terminal_semantics::projected_field_declaration(
                        structural_types.values().copied(),
                        root,
                        path,
                        field,
                    )?;
                    let mut complete = path.to_vec();
                    complete.push(terminal_psi::StructuralPathSegment::Field(member.identity));
                    Some(complete)
                },
            ) {
                Some((block, node, violation)) => Err(
                    OptimizationUnitValidationError::AtomicEventCoherenceMismatch(
                        OperationSite::new(function.machine, block, node as u32),
                        violation,
                    ),
                ),
                None => Ok(()),
            }
        }
    }
    mod boundaries {
        use crate::unit_validation::operation_contracts::structural_access::structural_source_contract;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::StructuralDomainId;
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn boundary_requirements_match(
            caller: &PsiOptimizationFunction,
            arguments: &[terminal_psi::StructuralArgument],
            boundary: &terminal_psi::BoundaryMachineDeclaration,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
        ) -> bool {
            boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
                && boundary.requires.iter().all(|requirement| {
                    domains.contains_key(&requirement.domain)
                        && arguments
                            .get(requirement.argument_index as usize)
                            .is_some_and(|argument| {
                                structural_source_contract(caller, argument.place).is_some_and(
                                    |source| {
                                        let mut path = argument.path.clone();
                                        path.extend_from_slice(&requirement.path);
                                        source.carries_qualification(&path, requirement.domain)
                                    },
                                )
                            })
                })
        }

        pub(crate) fn boundary_completion_matches(
            caller: &PsiOptimizationFunction,
            arguments: &[terminal_psi::StructuralArgument],
            sources: &[abstract_operations::CompletionClaimSource],
            receipts: &[terminal_psi::CompletionReceipt],
        ) -> bool {
            let mut expected_sources = caller
                .entry_claim_declarations
                .iter()
                .cloned()
                .map(|entry| abstract_operations::CompletionClaimSource {
                    claim: entry.claim,
                    entry: Some(entry),
                    content: None,
                })
                .collect::<Vec<_>>();
            for content in &caller.content_entry_claims {
                if let Some(source) = expected_sources
                    .iter_mut()
                    .find(|source| source.claim == content.claim)
                {
                    source.content = Some(content.clone());
                } else {
                    expected_sources.push(abstract_operations::CompletionClaimSource {
                        claim: content.claim,
                        entry: None,
                        content: Some(content.clone()),
                    });
                }
            }
            expected_sources.sort();
            if sources != expected_sources
                || receipts.windows(2).any(|pair| pair[0] >= pair[1])
                || receipts
                    .iter()
                    .map(|receipt| receipt.claim)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != receipts.len()
            {
                return false;
            }
            receipts.iter().all(|receipt| {
                arguments
                    .get(receipt.argument_index as usize)
                    .is_some_and(|argument| {
                        argument.access == terminal_psi::StructuralAccess::Owned
                    })
            })
        }
    }
    mod claim_transfers {
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{ContentTerm, PlaceId, Proposition, ScalarTerm};
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn function_entry_claim_paths(
            function: &PsiOptimizationFunction,
        ) -> Option<Vec<terminal_psi::TerminalDynamicClaimPath>> {
            if !function.content_entry_claims.is_empty() {
                return None;
            }
            let mut paths = Vec::new();
            for claim in &function.entry_claim_declarations {
                let argument_index = function
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == claim.input)?;
                paths.push(terminal_psi::TerminalDynamicClaimPath {
                    argument_index: u32::try_from(argument_index).ok()?,
                    path: claim.path.clone(),
                });
            }
            paths.sort();
            if paths.windows(2).any(|pair| pair[0] == pair[1]) {
                return None;
            }
            Some(paths)
        }

        pub(crate) fn validate_slot_claim_transfers(
            slot: &terminal_psi::TerminalDynamicSlot,
            arguments: &[terminal_psi::StructuralArgument],
            transfers: &[terminal_psi::ClaimTransfer],
        ) -> bool {
            let paths = transfers
                .iter()
                .map(|transfer| (transfer.argument_index, &transfer.path))
                .collect::<BTreeSet<_>>();
            transfers.windows(2).all(|pair| pair[0] < pair[1])
                && transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == transfers.len()
                && paths.len() == transfers.len()
                && transfers.iter().all(|transfer| {
                    arguments
                        .get(transfer.argument_index as usize)
                        .is_some_and(|argument| {
                            argument.access == terminal_psi::StructuralAccess::Owned
                        })
                        && slot.entry_claim_paths.iter().any(|expected| {
                            expected.argument_index == transfer.argument_index
                                && expected.path == transfer.path
                        })
                })
                && slot.entry_claim_paths.iter().all(|expected| {
                    paths.contains(&(expected.argument_index, &expected.path))
                        || expected.path.iter().any(|segment| {
                            matches!(segment, terminal_psi::StructuralPathSegment::Case(_))
                        })
                })
        }

        pub(crate) fn validate_internal_claim_transfers(
            caller: &PsiOptimizationFunction,
            callee: &PsiOptimizationFunction,
            arguments: &[terminal_psi::StructuralArgument],
            transfers: &[terminal_psi::ClaimTransfer],
        ) -> bool {
            for (argument, parameter) in arguments.iter().zip(&callee.structural_parameters) {
                if argument.access != terminal_psi::StructuralAccess::Owned {
                    let mut caller_paths = caller
                        .entry_claim_declarations
                        .iter()
                        .filter(|claim| {
                            claim.input == argument.place && claim.path.starts_with(&argument.path)
                        })
                        .map(|claim| &claim.path[argument.path.len()..])
                        .collect::<Vec<_>>();
                    let mut callee_paths = callee
                        .entry_claim_declarations
                        .iter()
                        .filter(|claim| claim.input == parameter.place)
                        .map(|claim| claim.path.as_slice())
                        .collect::<Vec<_>>();
                    caller_paths.sort();
                    callee_paths.sort();
                    if caller_paths != callee_paths {
                        return false;
                    }
                }
                if !argument.path.is_empty()
                    && (caller.content_entry_claim_declared(argument.place)
                        || callee.content_entry_claim_declared(parameter.place))
                {
                    return false;
                }
                let mut caller_content = caller
                    .content_entry_claims
                    .iter()
                    .filter(|claim| claim.input.root == argument.place)
                    .map(|claim| (&claim.input.segments, &claim.projections))
                    .collect::<Vec<_>>();
                let mut callee_content = callee
                    .content_entry_claims
                    .iter()
                    .filter(|claim| claim.input.root == parameter.place)
                    .map(|claim| (&claim.input.segments, &claim.projections))
                    .collect::<Vec<_>>();
                caller_content.sort();
                callee_content.sort();
                if caller_content != callee_content {
                    return false;
                }
            }
            let callee_claims = callee
                .entry_claim_declarations
                .iter()
                .map(|claim| (claim.claim, claim.input))
                .chain(
                    callee
                        .content_entry_claims
                        .iter()
                        .map(|claim| (claim.claim, claim.input.root)),
                )
                .collect::<BTreeMap<_, _>>();
            if transfers.len() != callee_claims.len()
                || transfers.windows(2).any(|pair| pair[0] >= pair[1])
                || transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != transfers.len()
            {
                return false;
            }
            for transfer in transfers {
                let Some(argument) = arguments.get(transfer.argument_index as usize) else {
                    return false;
                };
                let Some(parameter) = callee
                    .structural_parameters
                    .get(transfer.argument_index as usize)
                else {
                    return false;
                };
                if !callee
                    .entry_claim_declarations
                    .iter()
                    .any(|claim| claim.input == parameter.place && claim.path == transfer.path)
                    && !(transfer.path.is_empty()
                        && callee.content_entry_claim_declared(parameter.place))
                {
                    return false;
                }
                let _ = argument;
            }
            callee_claims.into_values().all(|input| {
                terminal_psi::structural_parameter_position(&callee.structural_parameters, input)
                    .is_some_and(|index| {
                        transfers
                            .iter()
                            .any(|transfer| transfer.argument_index as usize == index)
                    })
            })
        }

        pub(crate) fn proposition_structural_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
            fn scalar_term_roots(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
                match term {
                    ScalarTerm::StructuralRead { root, .. } => {
                        roots.insert(*root);
                        for operand in term.operands() {
                            scalar_term_roots(operand, roots);
                        }
                    }
                    semantic_vocabulary::scalar_term_predicate_quint_pattern!(root, ..) => {
                        roots.insert(*root);
                    }
                    _ => {
                        for operand in term.operands() {
                            scalar_term_roots(operand, roots);
                        }
                    }
                }
            }

            fn content_term_roots(term: &ContentTerm, roots: &mut BTreeSet<PlaceId>) {
                match term {
                    ContentTerm::Projection { subject, .. } => {
                        roots.insert(subject.root);
                    }
                    ContentTerm::Separate(terms) => {
                        for term in terms {
                            content_term_roots(term, roots);
                        }
                    }
                }
            }

            fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
                match proposition {
                    semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                        let mut pending = vec![left, right];
                        while let Some(term) = pending.pop() {
                            if let semantic_vocabulary::IntegerMathTerm::Embed(scalar) = term {
                                scalar_term_roots(scalar, roots);
                            } else {
                                pending.extend(term.operands());
                            }
                        }
                    }
                    semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                        scalar_term_roots(left, roots);
                        scalar_term_roots(right, roots);
                    }
                    Proposition::IeeeFloatComparison { left, right, .. } => {
                        roots.insert(left.root());
                        roots.insert(right.root());
                    }
                    Proposition::ByteSequenceEqual { left, right } => {
                        roots.insert(left.root());
                        roots.insert(right.root());
                    }
                    Proposition::StructuralCaseMembership { subject, .. } => {
                        roots.insert(subject.root());
                    }
                    Proposition::StructuralIdentity { left, right } => {
                        roots.insert(left.root());
                        roots.insert(right.root());
                    }
                    Proposition::ContentConservation(conservation) => {
                        content_term_roots(conservation.left(), roots);
                        content_term_roots(conservation.right(), roots);
                    }
                    Proposition::Conjunction(propositions)
                    | Proposition::Disjunction(propositions) => {
                        for proposition in propositions {
                            collect(proposition, roots);
                        }
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        collect(premise, roots);
                        collect(conclusion, roots);
                    }
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
                }
            }

            let mut roots = BTreeSet::new();
            collect(proposition, &mut roots);
            roots
        }
    }
    pub(crate) use claim_transfers::{function_entry_claim_paths, validate_slot_claim_transfers};
    mod node_contracts {
        use crate::unit_validation::operation_contracts::scalar_types::operation_scalar_types_match;
        use crate::unit_validation::operation_contracts::service_calls::{
            operation_service_contract_matches, operation_structural_call_contract_matches,
        };
        use crate::{OperationSite, OptimizationUnitValidationError};
        use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
        use semantic_vocabulary::{
            BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId, ValueId,
        };
        use std::collections::BTreeMap;

        #[allow(clippy::too_many_arguments)]
        pub(super) fn validate_node_operation_contracts(
            unit: &optimization_unit::PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            block: &optimization_unit::OptimizationBlock,
            node_index: usize,
            node: &optimization_unit::OptimizationNode,
            definitions: &BTreeMap<ValueId, ValueDefinition>,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            boundary_machines: &BTreeMap<
                BoundaryMachineId,
                &terminal_psi::BoundaryMachineDeclaration,
            >,
            services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
            structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            structural_domains: &BTreeMap<
                StructuralDomainId,
                &terminal_psi::StructuralDomainDeclaration,
            >,
            call_accesses: &crate::current_ownership::CallAccesses,
        ) -> Result<(), OptimizationUnitValidationError> {
            if !operation_scalar_types_match(
                unit,
                function,
                &node.operation,
                definitions,
                functions,
                boundary_machines,
            ) {
                return Err(
                    OptimizationUnitValidationError::ScalarOperationContractMismatch(
                        OperationSite::new(
                            function.machine,
                            block.id,
                            u32::try_from(node_index).expect("unit node index fits u32"),
                        ),
                    ),
                );
            }
            if !operation_structural_call_contract_matches(
                unit,
                function,
                &node.operation,
                functions,
                boundary_machines,
                structural_types,
                structural_domains,
                call_accesses,
            ) {
                return Err(
                    OptimizationUnitValidationError::StructuralCallContractMismatch(
                        OperationSite::new(
                            function.machine,
                            block.id,
                            u32::try_from(node_index).expect("unit node index fits u32"),
                        ),
                    ),
                );
            }
            if !operation_service_contract_matches(
                unit,
                function,
                &node.operation,
                functions,
                boundary_machines,
                services,
            ) {
                return Err(
                    OptimizationUnitValidationError::OperationServiceContractMismatch(
                        OperationSite::new(
                            function.machine,
                            block.id,
                            u32::try_from(node_index).expect("unit node index fits u32"),
                        ),
                    ),
                );
            }
            Ok(())
        }
    }
    mod payloadless_cases {
        use crate::unit_validation::operation_contracts::claim_transfers::proposition_structural_roots;
        use crate::unit_validation::operation_contracts::plain_calls::plain_structural_call;
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractStructuralCallResult;
        use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
        use semantic_vocabulary::StructuralDomainId;
        use semantic_vocabulary::{StructuralPlaceKind, StructuralTypeId};
        use std::collections::BTreeMap;
        use terminal_psi::StructuralDomainDeclaration;
        use terminal_psi::StructuralTypeDeclaration;

        pub(crate) fn plain_scalar_sum_call(
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> bool {
            let Some(call) = plain_structural_call(operation, callee) else {
                return false;
            };
            matches!(
                call.signature.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine
                    | terminal_psi::StructuralMultiplicity::Unrestricted
            ) && call.signature.qualifications.is_empty()
                && call.signature.projected_qualifications.is_empty()
                && call.selected_evidence.is_empty()
                && call.contract.outcome_specific_ensures.is_empty()
                && callee.structural_parameters.iter().all(|parameter| {
                    matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow
                    ) && parameter.multiplicity
                        == terminal_psi::StructuralMultiplicity::Unrestricted
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                })
                && types
                    .get(&call.signature.structural_type)
                    .is_some_and(|declaration| match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Sum { cases } => {
                            cases.iter().all(|case| {
                                case.fields.iter().all(|field| {
                                    !field.relevance.is_erased()
                                        && field.field_type.scalar_type().is_some()
                                })
                            })
                        }
                        _ => false,
                    })
        }

        pub(crate) fn case_establishment_matches(
            function: &PsiOptimizationFunction,
            operation: &O,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let O::EstablishCase {
                psi_operation,
                result,
                result_case,
                fields,
            } = operation
            else {
                return false;
            };
            operation_result_placement(function, result, *psi_operation)
                && matches!(
                    result.multiplicity,
                    terminal_psi::StructuralMultiplicity::Unrestricted
                        | terminal_psi::StructuralMultiplicity::Affine
                )
                && super::records::constructor_qualifications_match(
                    function,
                    result,
                    fields,
                    types,
                    domains,
                    Some(*result_case),
                )
                && result.claims.is_empty()
                && result.qualification_establishments.is_empty()
                && terminal_semantics::descriptor_results::owned_shared_byte_descriptor_shape(
                    types.values().copied(),
                    result.structural_type,
                )
                && types
                    .get(&result.structural_type)
                    .is_some_and(|declaration| {
                        let (common, cases) = match &declaration.shape {
                            terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                                (fields.as_slice(), cases)
                            }
                            _ => return false,
                        };
                        cases.iter().any(|case| {
                            let roster = common
                                .iter()
                                .chain(&case.fields)
                                .filter(|field| !field.relevance.is_erased())
                                .cloned()
                                .collect::<Vec<_>>();
                            case.id == *result_case
                                && super::records::field_initializers_match(
                                    function,
                                    *psi_operation,
                                    result,
                                    &roster,
                                    fields,
                                    types,
                                    domains,
                                )
                        })
                    })
        }

        pub(crate) fn exact_payloadless_case_return_exits(
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let Some(signature) = callee.result.structural() else {
                return false;
            };
            if !signature.qualifications.is_empty()
                || !signature.projected_qualifications.is_empty()
                || signature.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || callee.block_nodes().any(|node| {
                    matches!(
                        node.operation,
                        abstract_operations::call_or_call_structural_pattern!(..)
                            | O::BoundaryCall { .. }
                    )
                })
            {
                return false;
            }
            let mut exits = 0_usize;
            for block in &callee.blocks {
                let Some(node) = block.nodes.last() else {
                    return false;
                };
                let O::ReturnStructural {
                    source,
                    returned_claims,
                    ..
                } = &node.operation
                else {
                    continue;
                };
                if !returned_claims.is_empty() {
                    return false;
                }
                let Some(producer) = callee.structural_places.iter().find_map(|place| {
                    (place.id == *source)
                        .then_some(place.kind)
                        .and_then(|kind| match kind {
                            StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type,
                            } if structural_type == signature.structural_type => Some(producer),
                            _ => None,
                        })
                }) else {
                    return false;
                };
                let Some(producer) =
                    callee
                        .block_nodes()
                        .map(|node| &node.operation)
                        .find(|operation| {
                            matches!(
                                operation,
                                O::EstablishCase { psi_operation, .. }
                                    if *psi_operation == producer
                            )
                        })
                else {
                    return false;
                };
                let O::EstablishCase { result, fields, .. } = producer else {
                    return false;
                };
                if result.place != *source
                    || !fields.is_empty()
                    || result.structural_type != signature.structural_type
                    || !case_establishment_matches(callee, producer, types, domains)
                {
                    return false;
                }
                exits += 1;
            }
            exits != 0
        }

        pub(crate) fn exact_payloadless_structural_call(
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let Some(call) = plain_structural_call(operation, callee) else {
                return false;
            };
            let Some(callee_result) = callee.result.structural() else {
                return false;
            };
            callee.structural_parameters.iter().all(|parameter| {
                parameter.access == terminal_psi::StructuralAccess::Owned
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
                    && !terminal_psi::structural_type_has_borrowed_storage_by(
                        |id| types.get(&id).copied(),
                        parameter.structural_type,
                    )
            }) && call.structural_arguments.len() == callee.structural_parameters.len()
                && call.result.structural_type == callee_result.structural_type
                && call.result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && call.result.multiplicity == callee_result.multiplicity
                && call.result.qualifications == callee_result.qualifications
                && call.result.projected_qualifications == callee_result.projected_qualifications
                && call.contract.outcome_specific_ensures.iter().all(|row| {
                    proposition_structural_roots(&row.proposition)
                        .into_iter()
                        .all(|root| root == callee_result.place)
                })
                && exact_payloadless_case_return_exits(callee, types, domains)
        }

        pub(crate) fn payloadless_selected_evidence_surface_matches(
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let O::CallStructural {
                result:
                    AbstractStructuralCallResult::Structural {
                        selected_evidence, ..
                    },
                ..
            } = operation
            else {
                return true;
            };
            selected_evidence.is_empty()
                || (exact_payloadless_structural_call(operation, callee, types, domains)
                    && callee
                        .verified_contract
                        .as_ref()
                        .is_some_and(|contract| !contract.outcome_specific_ensures.is_empty()))
        }

        pub(crate) fn validate_structural_call_result(
            result: &terminal_psi::StructuralOperationResult,
            callee: &PsiOptimizationFunction,
            exact_claim_free: bool,
            claim_transfers: &[terminal_psi::ClaimTransfer],
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> bool {
            let Some(signature) = callee.result.structural() else {
                return false;
            };
            if result.structural_type != signature.structural_type
                || result.multiplicity != signature.multiplicity
                || result.qualifications != signature.qualifications
                || !signature
                    .projected_qualifications
                    .iter()
                    .all(|row| result.projected_qualifications.contains(row))
                || result
                    .qualifications
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || result.claims.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return false;
            }
            if terminal_semantics::validate_result_claim_catalog(
                types.values().copied(),
                result.structural_type,
                &result.claims,
                callee
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .filter_map(|node| match &node.operation {
                        O::ReturnStructural {
                            returned_claims, ..
                        } => Some(returned_claims.as_slice()),
                        _ => None,
                    }),
            )
            .is_err()
            {
                return false;
            }
            if exact_claim_free {
                return true;
            }
            if !claim_transfers.is_empty()
                && callee.entry_claim_declarations.is_empty()
                && callee.content_entry_claims.is_empty()
            {
                return false;
            }
            match result.multiplicity {
                terminal_psi::StructuralMultiplicity::Linear => {
                    matches!(result.claims.as_slice(), [claim] if claim.path.is_empty())
                }
                terminal_psi::StructuralMultiplicity::Affine => {
                    result.claims.iter().all(|claim| !claim.path.is_empty())
                }
                terminal_psi::StructuralMultiplicity::Unrestricted => result.claims.is_empty(),
            }
        }
    }
    mod plain_calls {
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractStructuralCallResult;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::ValueId;

        pub(crate) struct PlainStructuralCall<'a> {
            pub(crate) arguments: &'a [ValueId],
            pub(crate) structural_arguments: &'a [terminal_psi::StructuralArgument],
            pub(crate) result: &'a terminal_psi::StructuralOperationResult,
            pub(crate) selected_evidence: &'a [terminal_psi::OutcomeSpecificCallEvidence],
            pub(crate) signature: &'a terminal_psi::StructuralResultDeclaration,
            pub(crate) contract: &'a terminal_psi::MachineContract,
        }

        pub(crate) fn plain_structural_call<'a>(
            operation: &'a O,
            callee: &'a PsiOptimizationFunction,
        ) -> Option<PlainStructuralCall<'a>> {
            let O::CallStructural {
                result:
                    AbstractStructuralCallResult::Structural {
                        result,
                        selected_evidence,
                    },
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } = operation
            else {
                return None;
            };
            let signature = callee.result.structural()?;
            let contract = callee.verified_contract.as_ref()?;
            (result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && *crash_continuations == contract.crash_routes
                && callee.entry_claim_declarations.is_empty()
                && callee.content_entry_claims.is_empty()
                && callee.evidence_contract_lanes.is_empty()
                && contract.requires.is_empty()
                && contract.ensures.is_empty())
            .then_some(PlainStructuralCall {
                arguments: arguments.as_slice(),
                structural_arguments: structural_arguments.as_slice(),
                result,
                selected_evidence: selected_evidence.as_slice(),
                signature,
                contract,
            })
        }
    }
    mod records {
        use crate::unit_validation::operation_contracts::plain_calls::plain_structural_call;
        use crate::unit_validation::operation_contracts::structural_access::structural_source_contract;
        use crate::unit_validation::references::referent;
        use crate::unit_validation::structural_catalog::resolve_structural_path;
        use crate::unit_validation::structural_catalog::shared_view_declaration;
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractStructuralCallResult;
        use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
        use semantic_vocabulary::OperationId;
        use semantic_vocabulary::StructuralDomainId;
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::RecordFieldInitializer;
        use terminal_psi::StructuralArgument;
        use terminal_psi::StructuralDomainDeclaration;
        use terminal_psi::StructuralFieldDeclaration;
        use terminal_psi::StructuralOperationResult;
        use terminal_psi::StructuralTypeDeclaration;
        use terminal_psi::nonlinear_multiplicity_pair_pattern;
        use terminal_psi::{
            ByteSequenceCarrier, RecordFieldValue, StructuralAccess, StructuralFieldType,
            StructuralMultiplicity, StructuralTypeShape,
        };

        pub(crate) fn plain_record_call(
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> bool {
            let Some(call) = plain_structural_call(operation, callee) else {
                return false;
            };
            matches!(
                call.signature.multiplicity,
                nonlinear_multiplicity_pair_pattern!()
            ) && call.signature.qualifications.is_empty()
                && call.signature.projected_qualifications.is_empty()
                && call.selected_evidence.is_empty()
                && callee.entry_claims.is_empty()
                && call.contract.outcome_specific_ensures.is_empty()
                && callee.structural_parameters.iter().all(|parameter| {
                    matches!(
                        parameter.multiplicity,
                        nonlinear_multiplicity_pair_pattern!()
                    ) && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                })
                && plain_record(types, call.signature.structural_type)
        }

        pub(crate) fn zeroed_establishment_matches(
            function: &PsiOptimizationFunction,
            operation: &O,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let O::EstablishZeroedStructural {
                psi_operation,
                result,
            } = operation
            else {
                return false;
            };
            operation_result_placement(function, result, *psi_operation)
                && matches!(result.multiplicity, nonlinear_multiplicity_pair_pattern!())
                && result.qualifications.is_empty()
                && terminal_semantics::zeroed_projected_byte_qualifications_hold(
                    types.values().copied(),
                    domains.values().copied(),
                    result.structural_type,
                    &result.projected_qualifications,
                )
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && terminal_semantics::zeroed_structural_shape(
                    types.values().copied(),
                    result.structural_type,
                )
        }

        pub(crate) fn record_establishment_matches(
            function: &PsiOptimizationFunction,
            operation: &O,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let O::EstablishRecord {
                psi_operation,
                result,
                fields,
            } = operation
            else {
                return false;
            };
            let exact_result_place = operation_result_placement(function, result, *psi_operation);
            let complete_root_custody = match result.multiplicity {
                StructuralMultiplicity::Linear => {
                    matches!(result.claims.as_slice(), [claim] if claim.path.is_empty())
                }
                nonlinear_multiplicity_pair_pattern!() => result.claims.is_empty(),
            };
            if !exact_result_place
                || !complete_root_custody
                || !constructor_qualifications_match(function, result, fields, types, domains, None)
                || !constructible_record(types, result.structural_type)
            {
                return false;
            }
            let Some(StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::Record {
                        fields: declarations,
                        ..
                    },
                ..
            }) = types.get(&result.structural_type).copied()
            else {
                return false;
            };
            let fields_match = field_initializers_match(
                function,
                *psi_operation,
                result,
                declarations,
                fields,
                types,
                domains,
            );
            if !fields_match {
                return false;
            }
            let mut projected = Vec::new();
            for declaration in declarations
                .iter()
                .filter(|field| !field.relevance.is_erased())
            {
                if !matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
                    continue;
                }
                let Some(RecordFieldValue::Structural(argument)) = fields
                    .iter()
                    .find(|binding| binding.field == declaration.id)
                    .map(|binding| &binding.value)
                else {
                    return false;
                };
                let Some(source) = structural_source_contract(function, argument.place) else {
                    return false;
                };
                let (whole, children) = source.selected_qualifications(&argument.path);
                terminal_psi::StructuralPathQualification::append_under(
                    &whole,
                    &children,
                    &[terminal_psi::StructuralPathSegment::Field(
                        declaration.identity,
                    )],
                    &mut projected,
                );
            }
            projected.sort();
            projected.dedup();
            result.projected_qualifications == projected
        }

        pub(super) fn constructor_qualifications_match(
            function: &PsiOptimizationFunction,
            result: &StructuralOperationResult,
            fields: &[RecordFieldInitializer],
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            selected_case: Option<semantic_vocabulary::StructuralCaseId>,
        ) -> bool {
            if !result.qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !crate::unit_validation::structural_catalog::structural_projected_qualifications_match(
            result.structural_type,
            &result.projected_qualifications,
            types,
            domains,
        )
    {
        return false;
    }
            let Some(declaration) = types.get(&result.structural_type) else {
                return false;
            };
            result.projected_qualifications.iter().all(|qualification| {
                use terminal_psi::StructuralPathSegment::{Case, Field};
                let (declarations, offset, identity, suffix) =
                    match (&declaration.shape, qualification.path.as_slice()) {
                        (
                            StructuralTypeShape::Record { fields, .. }
                            | StructuralTypeShape::Mixed { fields, .. },
                            [Field(identity), suffix @ ..],
                        ) => (fields.as_slice(), 0, *identity, suffix),
                        (
                            StructuralTypeShape::Sum { cases }
                            | StructuralTypeShape::Mixed { cases, .. },
                            [Case(case), Field(identity), suffix @ ..],
                        ) if selected_case == Some(*case) => {
                            let Some(case) = terminal_psi::find_structural_case(cases, *case)
                            else {
                                return false;
                            };
                            let offset = match &declaration.shape {
                                StructuralTypeShape::Mixed { fields, .. } => fields
                                    .iter()
                                    .filter(|field| !field.relevance.is_erased())
                                    .count(),
                                _ => 0,
                            };
                            (case.fields.as_slice(), offset, *identity, suffix)
                        }
                        _ => return false,
                    };
                let Some(position) = declarations
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .position(|field| field.identity == identity)
                else {
                    return false;
                };
                let Some(initializer) = fields.get(offset + position) else {
                    return false;
                };
                let RecordFieldValue::Structural(argument) = &initializer.value else {
                    return false;
                };
                let Some(source) = structural_source_contract(function, argument.place) else {
                    return false;
                };
                let mut selected = argument.path.clone();
                selected.extend_from_slice(suffix);
                source.carries_qualification(&selected, qualification.domain)
            })
        }

        fn constructor_argument_contract<'a>(
            function: &'a PsiOptimizationFunction,
            argument: &StructuralArgument,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> Option<super::structural_access::StructuralSourceContract<'a>> {
            let mut source = structural_source_contract(function, argument.place)?;
            if !argument.path.is_empty() {
                if argument.access != StructuralAccess::Owned
                    || !argument.path.iter().all(|segment| {
                        matches!(
                            segment,
                            terminal_psi::StructuralPathSegment::Field(_)
                                | terminal_psi::StructuralPathSegment::FixedIndex(_)
                        )
                    })
                {
                    return None;
                }
                let root =
                    crate::current_ownership::partial_affine_root_type(function, argument.place)?;
                source.structural_type = resolve_structural_path(types, root, &argument.path)?;
            }
            Some(source)
        }

        pub(crate) fn field_initializers_match(
            function: &PsiOptimizationFunction,
            constructor: OperationId,
            result: &StructuralOperationResult,
            declarations: &[StructuralFieldDeclaration],
            fields: &[RecordFieldInitializer],
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            let declarations = declarations
                .iter()
                .filter(|field| !field.relevance.is_erased());
            if declarations.clone().count() != fields.len() {
                return false;
            }
            declarations.zip(fields).all(|(declaration, initializer)| {
                if declaration.id != initializer.field || declaration.relevance.is_erased() {
                    return false;
                }
                match (&declaration.field_type, &initializer.value) {
                    (
                        StructuralFieldType::Structural(expected),
                        RecordFieldValue::Structural(argument),
                    ) => {
                        structural_initializer_matches(function, result, *expected, argument, types)
                    }
                    (field_type, RecordFieldValue::Structural(argument))
                        if field_type.byte_sequence_carrier().is_some() =>
                    {
                        let Some(source) = structural_source_contract(function, argument.place)
                        else {
                            return false;
                        };
                        let Some(declared_source) = types.get(&source.structural_type) else {
                            return false;
                        };
                        let carrier_matches = match field_type.byte_sequence_carrier() {
                            Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
                                source.access == StructuralAccess::Owned
                                    && source.multiplicity == StructuralMultiplicity::Unrestricted
                                    && declared_source.shape
                                        == StructuralTypeShape::ByteSequence(carrier)
                            }
                            Some(carrier) if carrier.is_shared_readable_view() => {
                                matches!(
                                    source.access,
                                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                                ) && declared_source.shape.is_shared_readable_byte_view()
                            }
                            _ => false,
                        };
                        argument.access == StructuralAccess::Owned
                            && argument.path.is_empty()
                            && argument.place != result.place
                            && carrier_matches
                            && source.declared_byte_predicates(domains).is_some()
                            && !source_has_claims(function, argument.place)
                            && matches!(source.multiplicity, nonlinear_multiplicity_pair_pattern!())
                            && (result.multiplicity != StructuralMultiplicity::Unrestricted
                                || source.multiplicity == StructuralMultiplicity::Unrestricted)
                            && (field_type.byte_predicates().is_empty()
                                || constructed_field_predicates(
                                    function,
                                    constructor,
                                    argument.place,
                                    types,
                                    domains,
                                )
                                .is_some_and(|available| {
                                    available.implies(field_type.byte_predicates())
                                }))
                    }
                    (
                        field_type,
                        RecordFieldValue::Scalar {
                            value,
                            range_obligation,
                        },
                    ) => {
                        field_type.scalar_type().is_some_and(|scalar_type| {
                            function
                                .parameters
                                .iter()
                                .chain(function.blocks.iter().flat_map(|block| {
                                    block.parameters.iter().chain(block.definitions())
                                }))
                                .any(|definition| {
                                    definition.value == *value
                                        && definition.scalar_type == scalar_type
                                })
                        }) && terminal_semantics::scalar_field_requires_evidence(field_type)
                            == range_obligation.is_some()
                    }
                    _ => false,
                }
            })
        }

        fn constructed_field_predicates(
            function: &PsiOptimizationFunction,
            constructor: OperationId,
            source: PlaceId,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> Option<terminal_psi::ByteSequencePredicateSet> {
            let whole = crate::unit_validation::function_structure::whole_source_predicates(
                types.values().copied(),
                domains.values().copied(),
                function,
                source,
            );
            let captured =
                crate::unit_validation::function_structure::copied_field_predicates_at_transfer(
                    types.values().copied(),
                    function,
                    source,
                    constructor,
                );
            match (whole, captured) {
                (Some(mut predicates), Some(captured)) => {
                    for predicate in captured.predicates() {
                        predicates.insert(predicate);
                    }
                    Some(predicates)
                }
                (whole, captured) => whole.or(captured),
            }
        }

        fn source_has_claims(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
            function.entry_claim_declared(place)
                || function.content_entry_claim_declared(place)
                || function.block_nodes().any(|node| {
                    let result = match &node.operation {
                        O::EstablishScalarArray { result, .. }
                        | O::EstablishStructuralArray { result, .. }
                        | abstract_operations::establish_case_or_record_result_pattern!(result)
                        | O::EstablishZeroedStructural { result, .. }
                        | O::EstablishReference { result, .. }
                        | O::EstablishAccessReference { result, .. }
                        | O::MoveStructuralField { result, .. }
                        | O::CallStructural {
                            result: AbstractStructuralCallResult::Structural { result, .. },
                            ..
                        }
                        | O::BoundaryCall {
                            result: abstract_operations::AbstractBoundaryResult::Structural(result),
                            ..
                        } => result,
                        _ => return false,
                    };
                    result.place == place && !result.claims.is_empty()
                })
        }

        fn plain_record(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            record_type(types, root, false, false)
        }

        pub(crate) fn constructible_record(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            record_type(types, root, true, true)
        }

        fn shared_view_member(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            child: StructuralTypeId,
        ) -> bool {
            types
                .get(&child)
                .is_some_and(|row| shared_view_declaration(row))
        }

        fn record_type(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
            references: bool,
            shared_view_members: bool,
        ) -> bool {
            let mut pending = vec![(root, false)];
            let mut active = BTreeSet::new();
            let mut complete = BTreeSet::new();
            while let Some((current, exiting)) = pending.pop() {
                if exiting {
                    active.remove(&current);
                    complete.insert(current);
                    continue;
                }
                if complete.contains(&current) {
                    continue;
                }
                if !active.insert(current) {
                    return false;
                }
                let fields: Vec<&StructuralFieldDeclaration> = match types
                    .get(&current)
                    .copied()
                    .map(|declaration| &declaration.shape)
                {
                    Some(StructuralTypeShape::Record { fields, .. }) => fields.iter().collect(),
                    Some(StructuralTypeShape::Sum { cases }) => {
                        cases.iter().flat_map(|case| case.fields.iter()).collect()
                    }
                    Some(StructuralTypeShape::Mixed { fields, cases }) => fields
                        .iter()
                        .chain(cases.iter().flat_map(|case| case.fields.iter()))
                        .collect(),
                    Some(StructuralTypeShape::FixedArray { element, .. }) => {
                        pending.push((current, true));
                        pending.push((*element, false));
                        continue;
                    }
                    Some(StructuralTypeShape::PrimitiveScalar(_)) => Vec::new(),
                    Some(StructuralTypeShape::Reference { .. })
                        if references && referent(types, current).is_some() =>
                    {
                        Vec::new()
                    }
                    Some(_) if shared_view_members && shared_view_member(types, current) => {
                        Vec::new()
                    }
                    _ => return false,
                };
                pending.push((current, true));
                for field in fields {
                    if field.relevance.is_erased() {
                        if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                            continue;
                        }
                        return false;
                    }
                    match field.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::Atomic(_)
                        | StructuralFieldType::AtomicBoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::RefinedScalar(_)
                        | StructuralFieldType::BoundedInteger(_) => {}
                        StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                        )
                        | StructuralFieldType::PredicateByteSequence {
                            carrier: terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                            ..
                        } if references || shared_view_members => {}
                        StructuralFieldType::Structural(child) => pending.push((child, false)),
                        StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                        )
                        | StructuralFieldType::PredicateByteSequence {
                            carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                            ..
                        } => {}
                        _ => return false,
                    }
                }
            }
            true
        }

        pub(crate) fn constructible(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            let mut pending = vec![root];
            let mut active = BTreeSet::new();
            let mut complete = BTreeSet::new();
            while let Some(current) = pending.pop() {
                if complete.contains(&current) {
                    continue;
                }
                if !active.insert(current) {
                    return false;
                }
                let Some(declaration) = types.get(&current).copied() else {
                    return false;
                };
                let matching: Vec<&StructuralFieldDeclaration> = match &declaration.shape {
                    StructuralTypeShape::Record { fields, .. } => fields.iter().collect(),
                    StructuralTypeShape::Sum { cases } => {
                        cases.iter().flat_map(|case| case.fields.iter()).collect()
                    }
                    StructuralTypeShape::Mixed { fields, cases } => fields
                        .iter()
                        .chain(cases.iter().flat_map(|case| case.fields.iter()))
                        .collect(),
                    StructuralTypeShape::FixedArray { element, .. } => {
                        pending.push(*element);
                        Vec::new()
                    }
                    StructuralTypeShape::Reference { .. } if referent(types, current).is_some() => {
                        Vec::new()
                    }
                    _ => return false,
                };
                for field in matching {
                    if field.relevance.is_erased() {
                        return false;
                    }
                    match field.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::Atomic(_)
                        | StructuralFieldType::AtomicBoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::RefinedScalar(_)
                        | StructuralFieldType::BoundedInteger(_) => {}
                        StructuralFieldType::Structural(child) => pending.push(child),
                        _ => return false,
                    }
                }
                active.remove(&current);
                complete.insert(current);
            }
            true
        }

        pub(super) fn structural_initializer_matches(
            function: &PsiOptimizationFunction,
            result: &StructuralOperationResult,
            expected: StructuralTypeId,
            argument: &StructuralArgument,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        ) -> bool {
            let Some(source) = constructor_argument_contract(function, argument, types) else {
                return false;
            };
            argument.access == StructuralAccess::Owned
                && (source.access == StructuralAccess::Owned
                    || (source.access == StructuralAccess::SharedBorrow
                        && shared_view_member(types, expected)))
                && source.structural_type == expected
                && !source_has_claims(function, argument.place)
                && argument.place != result.place
                && matches!(source.multiplicity, nonlinear_multiplicity_pair_pattern!())
                && (result.multiplicity != StructuralMultiplicity::Unrestricted
                    || source.multiplicity == StructuralMultiplicity::Unrestricted)
        }
    }
    mod scalar_arrays {
        use crate::unit_validation::operation_contracts::plain_calls::plain_structural_call;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeMap;

        pub(crate) fn scalar_array_establishment_matches(
            function: &PsiOptimizationFunction,
            operation: &O,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<
                semantic_vocabulary::StructuralDomainId,
                &terminal_psi::StructuralDomainDeclaration,
            >,
        ) -> bool {
            let O::EstablishScalarArray {
                psi_operation,
                result,
                elements,
            } = operation
            else {
                return false;
            };
            let Some((scalar_type, count)) = terminal_semantics::scalar_array_leaf_shape(
                types.values().copied(),
                result.structural_type,
            ) else {
                return false;
            };
            result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && result.qualification_establishments.is_empty()
                && terminal_semantics::scalar_array_qualifications_hold(
                    types.values().copied(),
                    domains.values().copied(),
                    result.structural_type,
                    &result.qualifications,
                    elements,
                    |element| {
                        terminal_semantics::scalar_constant_value(element, |value| {
                            if let Some((ty, literal)) =
                                function
                                    .block_nodes()
                                    .find_map(|node| match node.operation {
                                        O::IntegerConstant {
                                            result,
                                            scalar_type,
                                            value: literal,
                                            ..
                                        } if result == value => Some((scalar_type, literal)),
                                        _ => None,
                                    })
                            {
                                return Some(terminal_semantics::ScalarConstantOrigin::Literal(
                                    ty, literal,
                                ));
                            }
                            let owner = function.blocks.iter().find(|block| {
                                block
                                    .parameters
                                    .iter()
                                    .any(|parameter| parameter.value == value)
                            })?;
                            if owner.id == function.entry {
                                return None;
                            }
                            let incoming = function
                                .block_nodes()
                                .flat_map(|node| &node.successors)
                                .filter(|edge| edge.target == owner.id)
                                .map(|edge| edge.binding(value).map(|binding| binding.argument))
                                .collect::<Option<Vec<_>>>()?;
                            Some(terminal_semantics::ScalarConstantOrigin::Incoming(incoming))
                        })
                    },
                )
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && u64::try_from(elements.len()).ok() == Some(count)
                && operation_result_placement(function, result, *psi_operation)
                && elements.iter().all(|element| {
                    function
                        .parameters
                        .iter()
                        .chain(
                            function.blocks.iter().flat_map(|block| {
                                block.parameters.iter().chain(block.definitions())
                            }),
                        )
                        .any(|definition| {
                            definition.value == *element && definition.scalar_type == scalar_type
                        })
                })
        }

        pub(crate) fn plain_scalar_array_call(
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            let Some(call) = plain_structural_call(operation, callee) else {
                return false;
            };
            call.signature.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && call.signature.qualifications.is_empty()
                && call.signature.projected_qualifications.is_empty()
                && call.selected_evidence.is_empty()
                && call.contract.outcome_specific_ensures.is_empty()
                && callee.structural_parameters.iter().all(|parameter| {
                    parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                })
                && terminal_semantics::scalar_array_leaf_shape(
                    types.values().copied(),
                    call.signature.structural_type,
                )
                .is_some()
        }

        pub(crate) fn structural_array_establishment_matches(
            function: &PsiOptimizationFunction,
            operation: &O,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            let O::EstablishStructuralArray {
                psi_operation,
                result,
                elements,
            } = operation
            else {
                return false;
            };
            let Some(terminal_psi::StructuralTypeDeclaration {
                shape:
                    terminal_psi::StructuralTypeShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    },
                ..
            }) = types.get(&result.structural_type).copied()
            else {
                return false;
            };
            let mut projected = Vec::new();
            for (position, argument) in elements.iter().enumerate() {
                let Some(source) =
                    super::structural_access::structural_source_contract(function, argument.place)
                else {
                    return false;
                };
                let Ok(position) = u64::try_from(position) else {
                    return false;
                };
                let (whole, children) = source.selected_qualifications(&argument.path);
                terminal_psi::StructuralPathQualification::append_under(
                    &whole,
                    &children,
                    &[terminal_psi::StructuralPathSegment::FixedIndex(position)],
                    &mut projected,
                );
            }
            projected.sort();
            projected.dedup();
            matches!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
                    | terminal_psi::StructuralMultiplicity::Affine
            ) && result.qualifications.is_empty()
                && result.projected_qualifications == projected
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && u64::try_from(elements.len()).ok() == Some(*length)
                && operation_result_placement(function, result, *psi_operation)
                && elements.iter().all(|argument| {
                    super::records::structural_initializer_matches(
                        function, result, *element, argument, types,
                    )
                })
        }
    }
    mod scalar_types {
        use abstract_operations::AbstractStructuralCallResult;
        use abstract_operations::{
            AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
            IntegerShiftPolicy,
        };
        use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
        use semantic_vocabulary::{
            BoundaryMachineId, IntegerCarrier, IntegerType, MachineId, ScalarType,
            StructuralPlaceKind, ValueId,
        };
        use std::collections::BTreeMap;

        mod byte_views {
            use abstract_operations::AbstractOperation as O;
            use optimization_unit::ValueDefinition;
            use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
            use std::collections::BTreeMap;

            pub(super) fn types_match(
                operation: &O,
                definitions: &BTreeMap<ValueId, ValueDefinition>,
            ) -> bool {
                let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
                match operation {
        O::EstablishByteSequenceConcatenation { left, right, .. } => [left, right].into_iter().all(|operand| matches!(scalar(operand.length()), Some(ScalarType::Integer(integer)) if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))),
        abstract_operations::byte_sequence_store_pair_pattern!( length, .. ) => {
            matches!(scalar(*length), Some(ScalarType::Integer(integer))
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
        }
        abstract_operations::byte_sequence_write_pair_pattern!(index,
            value,
            length, ..) => {
            matches!(scalar(*value), Some(ScalarType::Integer(integer))
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 8))
                && matches!(scalar(*index), Some(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                && scalar(*index) == scalar(*length)
        }
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => {
            matches!(scalar(*start), Some(ScalarType::Integer(integer))
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                && scalar(*start) == scalar(*end)
                && scalar(*start) == scalar(*length)
        }
        abstract_operations::byte_read_or_field_read_pattern!(result,
            index,
            length, ..) => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 8))
                && matches!(scalar(*index), Some(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                && scalar(*index) == scalar(*length)
        }
        O::ByteSequenceLength { result, .. }
        | O::StructuralByteSequenceFieldLength { result, .. } => {
            matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
        }
        O::ByteSequencePredicate {
            result, operand, ..
        } => {
            result.scalar_type == ScalarType::Boolean
                && matches!(scalar(operand.length()), Some(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
        }
        O::ByteSequenceEqual {
            result, left, right, ..
        } => {
            result.scalar_type == ScalarType::Boolean
                && [left, right].iter().all(|operand|
                    matches!(scalar(operand.length()), Some(ScalarType::Integer(integer))
                        if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64)))
        }
        _ => false,
    }
            }
        }
        mod element_views {
            use abstract_operations::AbstractOperation as O;
            use optimization_unit::{PsiOptimizationFunction, ValueDefinition};
            use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
            use std::collections::BTreeMap;

            pub(super) fn types_match(
                function: &PsiOptimizationFunction,
                operation: &O,
                definitions: &BTreeMap<ValueId, ValueDefinition>,
            ) -> bool {
                if matches!(operation, O::EstablishElementView { .. } | O::ElementViewSubslice { .. })
        && crate::unit_validation::operation_contracts::structural_access::structural_operation_result_contract(function, operation).is_none()
    {
        return false;
    }
                let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
                let u64_scalar = |value: &ValueId| {
                    matches!(scalar(*value), Some(ScalarType::Integer(integer))
            if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                };
                match operation {
                    O::ElementViewSubslice {
                        start, end, length, ..
                    } => u64_scalar(start) && u64_scalar(end) && u64_scalar(length),
                    O::ElementViewRead {
                        result,
                        index,
                        length,
                        ..
                    } => {
                        scalar(result.value) == Some(result.scalar_type)
                            && u64_scalar(index)
                            && scalar(*index) == scalar(*length)
                    }
                    O::ElementViewWrite {
                        index,
                        value,
                        length,
                        ..
                    } => {
                        u64_scalar(index)
                            && scalar(*value).is_some()
                            && scalar(*index) == scalar(*length)
                    }
                    O::ElementViewLength { result, .. } => {
                        matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 64))
                    }
                    O::EstablishElementView { region, .. } => match region {
                        terminal_psi::ElementViewRegion::WholeCollection => true,
                        terminal_psi::ElementViewRegion::WholeRepresentation {
                            source_length,
                            ..
                        } => u64_scalar(source_length),
                        terminal_psi::ElementViewRegion::RepresentationBytes {
                            start, end, ..
                        } => u64_scalar(start) && u64_scalar(end),
                    },
                    _ => false,
                }
            }
        }
        pub(crate) fn operation_scalar_types_match(
            unit: &optimization_unit::PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            operation: &O,
            definitions: &BTreeMap<ValueId, ValueDefinition>,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            boundary_machines: &BTreeMap<
                BoundaryMachineId,
                &terminal_psi::BoundaryMachineDeclaration,
            >,
        ) -> bool {
            let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
            let integer = |value: ValueId, expected: IntegerType| {
                scalar(value) == Some(ScalarType::Integer(expected))
            };
            let ieee_float =
                |value: ValueId, expected| scalar(value) == Some(ScalarType::IeeeFloat(expected));
            let fixed = |integer: IntegerType| integer.carrier() == IntegerCarrier::Fixed;
            let binary = |left: ValueId, right: ValueId, expected: IntegerType| {
                integer(left, expected) && integer(right, expected)
            };
            let paths = operation.structural_paths();
            if paths.iter().flat_map(|path| path.iter()).any(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::RepresentationView { .. }
                )
            }) {
                return false;
            }
            let selectors_match = paths.into_iter().flatten()
        .filter_map(terminal_psi::StructuralPathSegment::runtime_index).all(|(index, _)| {
        matches!(scalar(index), Some(ScalarType::Integer(integer)) if integer.bits() <= 64)
    });
            if !selectors_match {
                return false;
            }
            match operation {
                O::EstablishActivationClaim { committed, .. }
                | O::SplitActivationClaim {
                    committed_at: committed,
                    ..
                } => {
                    matches!(scalar(*committed), Some(ScalarType::Integer(integer)) if fixed(integer) && integer.bits() == 64 && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                }
                O::ReleaseActivationClaim { .. } | O::MergeActivationClaim { .. } => true,
                O::StorageAddress { result, .. } => {
                    matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address())
                        && scalar(result.value) == Some(result.scalar_type)
                }
                O::ActivateTaskRegister {
                    result,
                    selector,
                    entries,
                    ..
                } => {
                    let unsigned = |ty, bits| {
                        matches!(ty, ScalarType::Integer(integer)
                if fixed(integer) && integer.bits() == bits && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                    };
                    (2..=8192).contains(entries)
                        && unsigned(result.scalar_type, 8)
                        && scalar(result.value) == Some(result.scalar_type)
                        && scalar(*selector).is_some_and(|ty| unsigned(ty, 16))
                }
                O::ReloadSegmentDescriptors {
                    result,
                    selector,
                    entries,
                    ..
                } => {
                    let unsigned = |ty, bits| {
                        matches!(ty, ScalarType::Integer(integer)
                if fixed(integer) && integer.bits() == bits && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                    };
                    (1..=8192).contains(entries)
                        && unsigned(result.scalar_type, 8)
                        && scalar(result.value) == Some(result.scalar_type)
                        && scalar(*selector).is_some_and(|ty| unsigned(ty, 16))
                }
                O::ReadTaskRegister { result, .. } | O::ReadSegmentSelector { result, .. } => {
                    matches!(result.scalar_type, ScalarType::Integer(integer)
            if fixed(integer) && integer.bits() == 16 && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                        && scalar(result.value) == Some(result.scalar_type)
                }
                O::GlobalDescriptorTable { .. } => true,
                O::RegisterAccess {
                    service,
                    event,
                    result,
                    ..
                } => {
                    let matches_carrier = |scalar, bits| {
                        matches!(scalar, ScalarType::Integer(integer)
                if fixed(integer) && integer.bits() == bits
                    && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned)
                    };
                    let signature = event.operation();
                    signature.is_admitted()
                        && service.is_some() == signature.requires_service_authority()
                        && match (signature.result_carrier(), result) {
                            (None, None) => true,
                            (Some(carrier), Some(result)) => {
                                matches_carrier(result.scalar_type, carrier.bits())
                                    && scalar(result.value) == Some(result.scalar_type)
                            }
                            _ => false,
                        }
                        && event
                            .inputs()
                            .zip(signature.input_carriers())
                            .all(|(value, carrier)| {
                                scalar(*value)
                                    .is_some_and(|scalar| matches_carrier(scalar, carrier.bits()))
                            })
                }
                O::EstablishAccessReference { .. } => true,
                O::EstablishReference { storage, .. } => storage.as_ref().is_none_or(|view| {
                    matches!(scalar(view.byte_offset), Some(ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 64 && fixed(integer))
                }),
                O::SelectImplementation { result, table, .. } => {
                    matches!(result.scalar_type, ScalarType::Integer(integer)
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 64 && fixed(integer))
                        && unit.dynamic_dispatch.table(*table).is_some()
                }
                O::CallStructural {
                    callee:
                        abstract_operations::AbstractCallTarget::Dynamic {
                            selector,
                            interface,
                            slot,
                        },
                    arguments,
                    result,
                    ..
                } => {
                    let Some(signature) =
                        unit.dynamic_dispatch
                            .interface(*interface)
                            .and_then(|interface| {
                                usize::try_from(*slot)
                                    .ok()
                                    .and_then(|slot| interface.slots.get(slot))
                            })
                    else {
                        return false;
                    };
                    matches!(scalar(*selector), Some(ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 64 && fixed(integer))
                        && arguments.len() == signature.scalar_parameters.len()
                        && arguments
                            .iter()
                            .zip(&signature.scalar_parameters)
                            .all(|(argument, expected)| scalar(*argument) == Some(*expected))
                        && match result {
                            AbstractStructuralCallResult::Unit => signature.result.is_unit(),
                            AbstractStructuralCallResult::Scalar(result) => {
                                signature.result.scalar() == Some(result.scalar_type)
                            }
                            AbstractStructuralCallResult::Structural { .. } => matches!(
                                signature.result,
                                terminal_psi::BoundaryMachineResult::Structural(_)
                            ),
                        }
                }
                O::EstablishByteSequenceConcatenation { .. }
                | abstract_operations::byte_sequence_store_pair_pattern!(..)
                | abstract_operations::byte_sequence_write_pair_pattern!(..)
                | abstract_operations::byte_read_predicate_equal_pattern!(..)
                | O::ByteSequenceLength { .. }
                | abstract_operations::byte_len_or_field_read_pattern!(..)
                | O::ByteSequenceSubslice { .. } => byte_views::types_match(operation, definitions),
                O::EstablishElementView { .. }
                | O::ElementViewLength { .. }
                | abstract_operations::element_view_read_write_pair_pattern!(..)
                | O::ElementViewSubslice { .. } => {
                    element_views::types_match(function, operation, definitions)
                }
                abstract_operations::primitive_local_triple_pattern!(value, ..)
                | O::StructuralScalarFieldStore { value, .. } => {
                    scalar(value.value) == Some(value.scalar_type)
                }
                O::StructuralCaseMembership { result, .. } => {
                    result.scalar_type == ScalarType::Boolean
                }
                O::AtomicEvent { event, .. } => {
                    use abstract_operations::AbstractAtomicEvent as E;
                    event.ordering_is_legal()
                        && event.custody_is_consistent()
                        && match event {
                            E::Load { result, .. } => {
                                scalar(result.value) == Some(result.scalar_type)
                            }
                            E::Store { value, .. } => scalar(*value).is_some(),
                            E::ReadModifyWrite { operand, prior, .. } => {
                                scalar(*operand) == Some(prior.scalar_type)
                                    && scalar(prior.value) == Some(prior.scalar_type)
                            }
                            E::Swap { value, prior, .. } => {
                                scalar(*value) == Some(prior.scalar_type)
                                    && scalar(prior.value) == Some(prior.scalar_type)
                            }
                            E::CompareExchange {
                                expected,
                                replacement,
                                observed,
                                ..
                            } => {
                                scalar(*expected) == Some(observed.scalar_type)
                                    && scalar(*replacement) == Some(observed.scalar_type)
                                    && scalar(observed.value) == Some(observed.scalar_type)
                            }
                            E::CompareExchangeOnce {
                                expected,
                                replacement,
                                outcome,
                                custody,
                                ..
                            } => {
                                scalar(*expected).is_some()
                                    && scalar(*expected) == scalar(*replacement)
                                    && terminal_psi::types::atomic_once_result_is_plain(
                                        outcome, *custody,
                                    )
                                    && unit
                                        .structural_types
                                        .iter()
                                        .find(|row| row.id == outcome.structural_type)
                                        .is_some_and(|declaration| {
                                            terminal_psi::types::atomic_once_outcome_cases(
                                                declaration,
                                                scalar(*expected).unwrap(),
                                                *custody,
                                            )
                                            .is_some()
                                        })
                            }
                            E::Fence { .. } => true,
                        }
                }
                O::PrimitiveScalarRead { .. }
                | O::EstablishScalarArray { .. }
                | abstract_operations::structural_array_or_case_pattern!(..)
                | abstract_operations::byte_literal_establish_pair_pattern!(..)
                | O::EstablishZeroedStructural { .. }
                | O::EstablishStructuralLocal { .. }
                | O::ReleaseReference { .. }
                | O::EstablishNativeCodeLayout { .. }
                | O::EstablishRecord { .. }
                | abstract_operations::move_or_store_structural_field_pattern!()
                | O::StructuralByteViewFieldStore { .. }
                | O::StructuralLeafCopy { .. }
                | O::MoveStructuralValue { .. }
                | O::TransformContent { .. }
                | O::PortWrite { .. }
                | O::StructuralCase { .. }
                | O::ReturnUnit { .. }
                | abstract_operations::return_structural_or_crash_pattern!() => true,
                O::StructuralScalarFieldRead { result, .. } => {
                    matches!(
                        result.scalar_type,
                        ScalarType::Boolean | ScalarType::Integer(_)
                    )
                }
                O::IntegerConstant {
                    scalar_type, value, ..
                } => match scalar_type {
                    ScalarType::Integer(integer) => integer.admits(*value),
                    ScalarType::Boolean | ScalarType::IeeeFloat(_) => false,
                },
                O::IeeeFloatConstant { .. } => true,
                O::FloatResultPolicy { format, policy, .. } => {
                    policy.accepts_operand_types(*format, scalar)
                }
                O::IeeeFloatCompare {
                    format,
                    left,
                    right,
                    ..
                } => ieee_float(*left, *format) && ieee_float(*right, *format),
                O::NearestIeeeFloatFusedMultiplyAdd {
                    format,
                    left,
                    right,
                    addend,
                    ..
                } => {
                    ieee_float(*left, *format)
                        && ieee_float(*right, *format)
                        && ieee_float(*addend, *format)
                }
                O::BooleanConstant { .. } => true,
                O::BooleanNot { operand, .. } => scalar(*operand) == Some(ScalarType::Boolean),
                O::BooleanEqual { left, right, .. } => {
                    scalar(*left) == Some(ScalarType::Boolean)
                        && scalar(*right) == Some(ScalarType::Boolean)
                }
                abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
                | O::IntegerLessOrEqual { left, right, .. } => {
                    match (scalar(*left), scalar(*right)) {
                        (Some(ScalarType::Integer(left)), Some(ScalarType::Integer(right))) => {
                            left == right || (!left.is_address() && !right.is_address())
                        }
                        _ => false,
                    }
                }
                O::IntegerBitwiseNot {
                    scalar_type,
                    operand,
                    ..
                } => integer(*operand, *scalar_type),
                O::IntegerWiden {
                    source_type,
                    target_type,
                    operand,
                    ..
                } => integer(*operand, *source_type) && source_type.can_widen_to(*target_type),
                O::ScalarRepresentationView {
                    source_type,
                    target_type,
                    operand,
                    ..
                } => {
                    scalar(*operand) == Some(*source_type)
                        && terminal_semantics::ScalarRepresentationViewPair::classify(
                            *source_type,
                            *target_type,
                        )
                        .is_some()
                }
                O::IntegerExactCast {
                    source_type,
                    target_type,
                    operand,
                    ..
                } => {
                    integer(*operand, *source_type)
                        && source_type.can_exact_cast_to(*target_type)
                        && !source_type.can_widen_to(*target_type)
                        && source_type != target_type
                }
                O::IntegerBitwiseAnd {
                    scalar_type,
                    left,
                    right,
                    ..
                }
                | abstract_operations::bitwise_or_xor_pair_pattern!(scalar_type, left, right, ..) => {
                    binary(*left, *right, *scalar_type)
                }
                O::IntegerArithmetic {
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                    ..
                } => {
                    if arithmetic.is(IntegerOverflowPolicy::Exact, IntegerArithmeticOperator::Add) {
                        matches!((scalar(*left), scalar(*right)),
                    (Some(ScalarType::Integer(left)), Some(ScalarType::Integer(right)))
                        if scalar_type.admits_exact_add_operands(left, right))
                    } else if arithmetic.is(
                        IntegerOverflowPolicy::Exact,
                        IntegerArithmeticOperator::Subtract,
                    ) {
                        matches!((scalar(*left), scalar(*right)),
                    (Some(ScalarType::Integer(left)), Some(ScalarType::Integer(right)))
                        if scalar_type.admits_exact_subtract_operands(left, right))
                    } else {
                        (arithmetic.obligation().is_none() || fixed(*scalar_type))
                            && binary(*left, *right, *scalar_type)
                    }
                }
                O::IntegerShift {
                    value_type,
                    count_type,
                    policy,
                    value,
                    count,
                    ..
                } => {
                    let carriers_admitted = match policy {
                        IntegerShiftPolicy::Wrapping => true,
                        IntegerShiftPolicy::Exact { .. } => {
                            fixed(*value_type) && fixed(*count_type)
                        }
                    };
                    carriers_admitted
                        && integer(*value, *value_type)
                        && integer(*count, *count_type)
                }
                O::TrappingInteger {
                    scalar_type,
                    operand_type,
                    operation,
                    ..
                } => {
                    use terminal_psi::TrappingIntegerOperation as T;
                    fixed(*scalar_type)
                        && fixed(*operand_type)
                        && match *operation {
                            terminal_psi::trapping_arith_set_pattern!(left, right, ..) => {
                                operand_type == scalar_type && binary(left, right, *scalar_type)
                            }
                            T::ShiftLeft { value, count } | T::ShiftRight { value, count } => {
                                integer(value, *scalar_type) && integer(count, *operand_type)
                            }
                            T::Convert { operand } => integer(operand, *operand_type),
                        }
                }
                O::Jump { .. } => true,
                O::Conditional { condition, .. } => scalar(*condition) == Some(ScalarType::Boolean),
                O::Return {
                    result,
                    value,
                    scalar_type,
                    ..
                } => {
                    scalar(*value) == Some(*scalar_type)
                        && matches!(
                            function.result,
                            abstract_operations::AbstractFunctionResult::Scalar(signature)
                                if signature.value == *result && signature.scalar_type == *scalar_type
                        )
                }
                O::Call {
                    result: _,
                    scalar_type,
                    callee,
                    arguments,
                    ..
                } => functions.get(callee).is_some_and(|callee| {
                    callee.structural_parameters.is_empty()
                        && callee.entry_claim_declarations.is_empty()
                        && !callee.structural_places.iter().any(|place| {
                            matches!(
                                place.kind,
                                StructuralPlaceKind::Parameter { .. }
                                    | StructuralPlaceKind::Result
                                    | StructuralPlaceKind::ProviderAttachment { .. }
                            )
                        })
                        && matches!(
                            callee.result,
                            abstract_operations::AbstractFunctionResult::Scalar(signature)
                                if signature.scalar_type == *scalar_type
                        )
                        && arguments.len() == callee.parameters.len()
                        && arguments
                            .iter()
                            .zip(&callee.parameters)
                            .all(|(argument, parameter)| {
                                scalar(*argument) == Some(parameter.scalar_type)
                            })
                }),
                O::CallStructural {
                    callee,
                    arguments,
                    result: AbstractStructuralCallResult::Unit,
                    ..
                } => callee
                    .machine()
                    .and_then(|machine| functions.get(&machine))
                    .is_some_and(|callee| {
                        matches!(
                            callee.result,
                            abstract_operations::AbstractFunctionResult::Unit
                        ) && arguments.len() == callee.parameters.len()
                            && arguments.iter().zip(&callee.parameters).all(
                                |(argument, parameter)| {
                                    scalar(*argument) == Some(parameter.scalar_type)
                                },
                            )
                    }),
                O::CallStructural {
                    result: AbstractStructuralCallResult::Scalar(result),
                    callee,
                    arguments,
                    ..
                } => callee
                    .machine()
                    .and_then(|machine| functions.get(&machine))
                    .is_some_and(|callee| {
                        arguments.len() == callee.parameters.len()
                            && arguments.iter().zip(&callee.parameters).all(
                                |(argument, parameter)| {
                                    scalar(*argument) == Some(parameter.scalar_type)
                                },
                            )
                            && matches!(
                                callee.result,
                                abstract_operations::AbstractFunctionResult::Scalar(signature)
                                    if signature.scalar_type == result.scalar_type
                            )
                    }),
                O::CallStructural {
                    callee,
                    arguments,
                    result: AbstractStructuralCallResult::Structural { .. },
                    ..
                } => callee
                    .machine()
                    .and_then(|machine| functions.get(&machine))
                    .is_some_and(|callee| {
                        callee.parameters.len() == arguments.len()
                            && callee.parameters.iter().zip(arguments).all(
                                |(parameter, argument)| {
                                    scalar(*argument) == Some(parameter.scalar_type)
                                },
                            )
                            && matches!(
                                callee.result,
                                abstract_operations::AbstractFunctionResult::Structural(_)
                            )
                    }),
                O::InvokeNoNormal {
                    callee, arguments, ..
                } => functions.get(callee).is_some_and(|callee| {
                    arguments.len() == callee.parameters.len()
                        && arguments
                            .iter()
                            .zip(&callee.parameters)
                            .all(|(value, parameter)| scalar(*value) == Some(parameter.scalar_type))
                }),
                O::ExternalCompletion {
                    boundary,
                    kind,
                    arguments,
                    ..
                } => boundary_machines.get(boundary).is_some_and(|boundary| {
                    boundary.completion == semantic_vocabulary::BoundaryCompletion::External(*kind)
                        && boundary.result.is_unit()
                        && arguments.len() == boundary.scalar_parameters.len()
                        && arguments
                            .iter()
                            .zip(&boundary.scalar_parameters)
                            .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
                }),
                O::BoundaryCall {
                    result,
                    boundary,
                    arguments,
                    ..
                } => boundary_machines.get(boundary).is_some_and(|boundary| {
                    boundary.completion == semantic_vocabulary::BoundaryCompletion::NormalReturn
                        && (match (&boundary.result, result) {
                            (
                                terminal_psi::BoundaryMachineResult::Unit,
                                abstract_operations::AbstractBoundaryResult::Unit,
                            ) => true,
                            (
                                terminal_psi::BoundaryMachineResult::Scalar(expected),
                                abstract_operations::AbstractBoundaryResult::Scalar(actual),
                            ) => actual.scalar_type == expected.scalar_type,
                            (
                                terminal_psi::BoundaryMachineResult::Structural(expected),
                                abstract_operations::AbstractBoundaryResult::Structural(actual),
                            ) => {
                                actual.structural_type == expected.structural_type
                                    && actual.multiplicity == expected.multiplicity
                                    && actual.qualifications == expected.qualifications
                                    && actual.projected_qualifications
                                        == expected.projected_qualifications
                                    && actual.claims.is_empty()
                            }
                            _ => false,
                        })
                        && arguments.len() == boundary.scalar_parameters.len()
                        && arguments
                            .iter()
                            .zip(&boundary.scalar_parameters)
                            .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
                }),
            }
        }
    }
    mod service_calls {
        use crate::unit_validation::operation_contracts::affine_calls::exact_plain_affine_structural_call;
        use crate::unit_validation::operation_contracts::boundaries::{
            boundary_completion_matches, boundary_requirements_match,
        };
        use crate::unit_validation::operation_contracts::claim_transfers::validate_internal_claim_transfers;
        use crate::unit_validation::operation_contracts::payloadless_cases::{
            case_establishment_matches, exact_payloadless_structural_call,
            payloadless_selected_evidence_surface_matches, plain_scalar_sum_call,
            validate_structural_call_result,
        };
        use crate::unit_validation::operation_contracts::records::{
            plain_record_call, record_establishment_matches, zeroed_establishment_matches,
        };
        use crate::unit_validation::operation_contracts::scalar_arrays::{
            plain_scalar_array_call, scalar_array_establishment_matches,
        };
        use crate::unit_validation::operation_contracts::structural_access::{
            StructuralProjectionPolicy, structural_arguments_match,
        };
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::AbstractStructuralCallResult;
        use optimization_unit::{PsiOptimizationFunction, operation_result_placement};
        use semantic_vocabulary::{
            BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
        };
        use std::collections::BTreeMap;

        pub(crate) fn operation_service_contract_matches(
            unit: &optimization_unit::PsiOptimizationUnit,
            caller: &PsiOptimizationFunction,
            operation: &O,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
            services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
        ) -> bool {
            let reached_is_published = |reached: &[ServiceId]| {
                reached
                    .iter()
                    .all(|service| caller.published_service_ceiling.contains(service))
            };
            match operation {
                O::Call { callee, .. } | O::InvokeNoNormal { callee, .. } => functions
                    .get(callee)
                    .is_some_and(|callee| reached_is_published(&callee.published_service_ceiling)),
                O::CallStructural { .. } => {
                    unit.call_machine_references(operation).all(|machine| {
                        functions.get(&machine).is_some_and(|callee| {
                            reached_is_published(&callee.published_service_ceiling)
                        })
                    })
                }
                O::BoundaryCall { boundary, .. } => {
                    boundaries.get(boundary).is_some_and(|boundary| {
                        reached_is_published(&boundary.published_service_ceiling)
                    })
                }
                O::PortWrite { service, .. } => {
                    services.contains_key(service)
                        && caller.published_service_ceiling.contains(service)
                }
                _ => true,
            }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn operation_structural_call_contract_matches(
            unit: &optimization_unit::PsiOptimizationUnit,
            caller: &PsiOptimizationFunction,
            operation: &O,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            boundary_machines: &BTreeMap<
                BoundaryMachineId,
                &terminal_psi::BoundaryMachineDeclaration,
            >,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
            call_accesses: &crate::current_ownership::CallAccesses,
        ) -> bool {
            match operation {
        O::InvokeNoNormal { callee, structural_arguments, claim_transfers, requirement_obligations, crash_continuations, psi_edge, .. } => functions.get(callee).is_some_and(|callee| {
            let surface = callee.verified_contract.as_ref().is_some_and(|contract| requirement_obligations.len() == contract.requires.len() && crash_continuations == &contract.crash_routes);
            surface && crate::unit_validation::operation_contracts::structural_access::structural_arguments_match_at_invocation(
                caller,
                Some(semantic_vocabulary::InvocationSite::InvokeNoNormal(*psi_edge)),
                structural_arguments,
                &callee.structural_parameters,
                types,
                domains,
                StructuralProjectionPolicy::Projected,
            ) && validate_internal_claim_transfers(caller, callee, structural_arguments, claim_transfers)
        }),
        O::CallStructural {
            callee:
                abstract_operations::AbstractCallTarget::Dynamic {
                    interface, slot, ..
                },
            structural_arguments,
            claim_transfers,
            result,
            ..
        } => {
            let Some(signature) =
                unit.dynamic_dispatch
                    .interface(*interface)
                    .and_then(|interface| {
                        usize::try_from(*slot)
                            .ok()
                            .and_then(|slot| interface.slots.get(slot))
                    })
            else {
                return false;
            };
            let policy = match result {
                AbstractStructuralCallResult::Unit => StructuralProjectionPolicy::Unit,
                AbstractStructuralCallResult::Scalar(_) => StructuralProjectionPolicy::Projected,
                AbstractStructuralCallResult::Structural { .. } => {
                    StructuralProjectionPolicy::Projected
                }
            };
            let result_matches = match result {
                AbstractStructuralCallResult::Unit => signature.result.is_unit(),
                AbstractStructuralCallResult::Scalar(result) => {
                    signature.result.scalar() == Some(result.scalar_type)
                }
                AbstractStructuralCallResult::Structural {
                    result,
                    selected_evidence,
                } => {
                    selected_evidence.is_empty()
                        && result.qualification_establishments.is_empty()
                        && !terminal_psi::structural_type_has_borrowed_storage_by(
                            |id| types.get(&id).copied(),
                            result.structural_type,
                        )
                        && signature.matches_operation_result(
                            &terminal_psi::OperationResult::Structural(result.clone()),
                        )
                }
            };
            result_matches
                && super::validate_slot_claim_transfers(
                    signature,
                    structural_arguments,
                    claim_transfers,
                )
                && structural_arguments_match(
                    caller,
                    operation.psi_operation(),
                    structural_arguments,
                    &signature.structural_parameters,
                    types,
                    domains,
                    policy,
                )
        }
        O::EstablishStructuralArray { .. } => {
            super::scalar_arrays::structural_array_establishment_matches(caller, operation, types)
        }
        O::EstablishScalarArray { .. } => {
            scalar_array_establishment_matches(caller, operation, types, domains)
        }
        O::EstablishCase { .. } => case_establishment_matches(caller, operation, types, domains),
        O::EstablishRecord { .. } => {
            record_establishment_matches(caller, operation, types, domains)
        }
        O::EstablishZeroedStructural { .. } => {
            zeroed_establishment_matches(caller, operation, types, domains)
        }
        O::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            result: result @ (AbstractStructuralCallResult::Unit
            | AbstractStructuralCallResult::Scalar(_)),
            ..
        } => callee
            .machine()
            .and_then(|machine| functions.get(&machine))
            .is_some_and(|callee| {
                let projection = match result {
                    AbstractStructuralCallResult::Unit => StructuralProjectionPolicy::Unit,
                    _ => StructuralProjectionPolicy::Projected,
                };
                direct_call_arguments_match(
                    caller,
                    operation,
                    callee,
                    structural_arguments,
                    claim_transfers,
                    types,
                    domains,
                    projection,
                )
            }),
        O::CallStructural {
            result: AbstractStructuralCallResult::Structural { result, .. },
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => callee
            .machine()
            .and_then(|machine| functions.get(&machine))
            .is_some_and(|callee| {
                if callee.result.structural().is_some_and(|signature| {
                    crate::unit_validation::references::contains_reference(
                        types,
                        signature.structural_type,
                    )
                }) {
                    return reference_call_matches(caller, operation, callee, types, domains);
                }
                direct_call_arguments_match(
                    caller,
                    operation,
                    callee,
                    structural_arguments,
                    claim_transfers,
                    types,
                    domains,
                    StructuralProjectionPolicy::Projected,
                ) && validate_structural_call_result(
                    result,
                    callee,
                    exact_payloadless_structural_call(operation, callee, types, domains)
                        || plain_scalar_array_call(operation, callee, types)
                        || plain_scalar_sum_call(operation, callee, types)
                        || plain_record_call(operation, callee, types)
                        || exact_plain_affine_structural_call(operation, callee, types),
                    claim_transfers,
                    types,
                ) && structural_call_contract_surface_matches(operation, callee)
                    && payloadless_selected_evidence_surface_matches(
                        operation, callee, types, domains,
                    )
                    && (result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                        || !terminal_semantics::descriptor_results::is_shared_descriptor(
                            types.values().copied(),
                            result.structural_type,
                        )
                        || descriptor_call_origins_match(
                            caller,
                            result.place,
                            functions,
                            boundary_machines,
                            types,
                            &unit.dynamic_dispatch,
                            call_accesses,
                        ))
            }),
        O::BoundaryCall {
            boundary,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            structural_arguments_match(
                caller,
                operation.psi_operation(),
                structural_arguments,
                &boundary.structural_parameters,
                types,
                domains,
                StructuralProjectionPolicy::Boundary,
            ) && boundary_requirements_match(caller, structural_arguments, boundary, domains)
                && boundary_completion_matches(
                    caller,
                    structural_arguments,
                    completion_claim_sources,
                    completion_receipts,
                )
        }),
        _ => true,
    }
        }

        #[allow(clippy::too_many_arguments)]
        fn direct_call_arguments_match(
            caller: &PsiOptimizationFunction,
            operation: &O,
            callee: &PsiOptimizationFunction,
            structural_arguments: &[terminal_psi::StructuralArgument],
            claim_transfers: &[terminal_psi::ClaimTransfer],
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
            projection: StructuralProjectionPolicy,
        ) -> bool {
            structural_arguments_match(
                caller,
                operation.psi_operation(),
                structural_arguments,
                &callee.structural_parameters,
                types,
                domains,
                projection,
            ) && validate_internal_claim_transfers(
                caller,
                callee,
                structural_arguments,
                claim_transfers,
            )
        }

        fn structural_call_contract_surface_matches(
            operation: &O,
            callee: &PsiOptimizationFunction,
        ) -> bool {
            let O::CallStructural {
                requirement_obligations,
                crash_continuations,
                result: AbstractStructuralCallResult::Structural { .. },
                ..
            } = operation
            else {
                return false;
            };
            match &callee.verified_contract {
                Some(contract) => {
                    requirement_obligations.len() == contract.requires.len()
                        && *crash_continuations == contract.crash_routes
                }
                None => requirement_obligations.is_empty(),
            }
        }

        fn reference_call_matches(
            caller: &PsiOptimizationFunction,
            operation: &O,
            callee: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
        ) -> bool {
            let O::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } = operation
            else {
                return false;
            };
            let Some(signature) = callee.result.structural().filter(|signature| {
                crate::unit_validation::references::contains_reference(
                    types,
                    signature.structural_type,
                )
            }) else {
                return false;
            };
            let Some(contract) = callee.verified_contract.as_ref() else {
                return false;
            };
            let checks = [
                (
                    "sig_type",
                    result.structural_type == signature.structural_type,
                ),
                ("sig_mult", result.multiplicity == signature.multiplicity),
                (
                    "sig_qual",
                    result.qualifications == signature.qualifications,
                ),
                (
                    "sig_pqual",
                    signature
                        .projected_qualifications
                        .iter()
                        .all(|row| result.projected_qualifications.contains(row)),
                ),
                (
                    "contains_ref",
                    crate::unit_validation::references::contains_reference(
                        types,
                        result.structural_type,
                    ),
                ),
                (
                    "ref_or_construct",
                    crate::unit_validation::references::referent(types, result.structural_type)
                        .is_some()
                        || crate::unit_validation::operation_contracts::constructible(
                            types,
                            result.structural_type,
                        )
                        || match types
                            .get(&result.structural_type)
                            .map(|declaration| &declaration.shape)
                        {
                            Some(terminal_psi::StructuralTypeShape::Reference {
                                referent, ..
                            }) => crate::unit_validation::operation_contracts::constructible(
                                types, *referent,
                            ),
                            _ => false,
                        },
                ),
                (
                    "affine",
                    result.multiplicity == terminal_psi::StructuralMultiplicity::Affine,
                ),
                ("claims_empty", result.claims.is_empty()),
                (
                    "place_kind",
                    operation_result_placement(caller, result, *psi_operation),
                ),
                ("ct_empty", claim_transfers.is_empty()),
                ("ec_empty", callee.entry_claims.is_empty()),
                ("cec_empty", callee.content_entry_claims.is_empty()),
                ("args_len", arguments.len() == callee.parameters.len()),
                (
                    "req_len",
                    requirement_obligations.len() == contract.requires.len(),
                ),
                ("crash", *crash_continuations == contract.crash_routes),
                (
                    "args_match",
                    structural_arguments_match(
                        caller,
                        operation.psi_operation(),
                        structural_arguments,
                        &callee.structural_parameters,
                        types,
                        domains,
                        StructuralProjectionPolicy::Unit,
                    ),
                ),
            ];

            checks.iter().all(|(_, ok)| *ok)
        }

        fn descriptor_call_origins_match(
            caller: &PsiOptimizationFunction,
            result: semantic_vocabulary::PlaceId,
            functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
            boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
            call_accesses: &crate::current_ownership::CallAccesses,
        ) -> bool {
            if let Some(matched) = call_accesses.has_descriptor_call_origins(caller.machine, result)
            {
                return matched;
            }
            let types = types.values().copied().cloned().collect::<Vec<_>>();
            let Some(graph) = crate::descriptor_results::optimized_catalog_graph(
                functions.values().copied(),
                boundaries.values().copied(),
                &types,
                dynamic_dispatch,
                |_, node| Some(std::borrow::Cow::Borrowed(&node.operation)),
            ) else {
                return false;
            };
            terminal_semantics::descriptor_results::reconstruct_call_origins(
                &types,
                &graph,
                caller.machine,
                result,
            )
            .is_some()
        }
    }
    mod structural_access {
        use crate::unit_validation::function_structure::whole_source_predicates;
        use crate::unit_validation::structural_catalog::{
            resolve_runtime_projection, resolve_structural_path,
        };
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::StructuralCallFormal;
        use abstract_operations::element_view_or_byte_subslice_pattern;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralAccess;
        use terminal_psi::StructuralArgument;
        use terminal_psi::StructuralDomainDeclaration;
        use terminal_psi::StructuralOperationResult;
        use terminal_psi::StructuralParameterDeclaration;
        use terminal_psi::StructuralPathQualification;
        use terminal_psi::StructuralPathSegment;
        use terminal_psi::StructuralTypeDeclaration;
        use terminal_psi::TerminalIdentityTable;
        use terminal_semantics::borrowed_referent_matches;
        use terminal_semantics::byte_parameter_predicates_in;
        use terminal_semantics::descriptor_results::PayloadMembershipContract;
        use terminal_semantics::reference_preserves_ancestor_qualifications;
        use terminal_semantics::structural_paths_may_overlap;

        mod descriptor_access {
            use super::*;

            pub(super) fn backing_access<'types>(
                caller: &PsiOptimizationFunction,
                place: PlaceId,
                types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
            ) -> Option<terminal_psi::StructuralAccess> {
                let declared = structural_source_contract(caller, place)?
                    .claimed_backing_access(types.clone());
                let mut place = place;
                let mut visited = Vec::new();
                loop {
                    if visited.contains(&place) {
                        return None;
                    }
                    visited.push(place);
                    let contract = structural_source_contract(caller, place)?;
                    let Some(result) = contract.operation_result else {
                        return Some(declared);
                    };
                    let mut producers = caller.block_nodes().filter(|node| {
                        node.operation
                            .structural_result()
                            .is_some_and(|(_, candidate)| candidate.place == place)
                    });
                    let producer = producers.next()?;
                    if producers.next().is_some()
                        || producer.operation.structural_result()?.1 != result
                    {
                        return None;
                    }
                    let source = match &producer.operation {
                        O::EstablishElementView { source, .. }
                        | abstract_operations::element_view_or_byte_subslice_pattern!(source, ..) => {
                            source
                        }
                        _ => return Some(declared),
                    };
                    let source_contract = structural_source_contract(caller, source.place)?;
                    if !terminal_semantics::descriptor_source_projection_accessible(
                        types.clone(),
                        source_contract.structural_type,
                        source_contract.multiplicity,
                        source_contract.access,
                        &source.path,
                        source.access,
                    ) {
                        return None;
                    }
                    place = source.place;
                }
            }
        }
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum StructuralProjectionPolicy {
            Unit,
            Projected,
            Boundary,
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) struct StructuralSourceContract<'a> {
            caller: &'a PsiOptimizationFunction,
            place: PlaceId,
            pub(crate) structural_type: StructuralTypeId,
            pub(crate) multiplicity: terminal_psi::StructuralMultiplicity,
            pub(crate) access: StructuralAccess,
            qualifications: &'a [StructuralDomainId],
            projected_qualifications: &'a [StructuralPathQualification],
            additional_input_qualifications: &'a [StructuralPathQualification],
            operation_result: Option<&'a StructuralOperationResult>,
        }

        impl StructuralSourceContract<'_> {
            pub(crate) fn operation_result(&self) -> Option<&StructuralOperationResult> {
                self.operation_result
            }

            pub(crate) fn backing_access<'types>(
                &self,
                types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            ) -> Option<StructuralAccess> {
                descriptor_access::backing_access(self.caller, self.place, types)
            }

            fn claimed_backing_access<'types>(
                &self,
                types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            ) -> StructuralAccess {
                if self.access == StructuralAccess::Owned
                    && let Some(result) = self.operation_result
                    && let Some(access) =
                        terminal_semantics::descriptor_results::result_access(types.clone(), result)
                {
                    return access;
                }
                if self.access == StructuralAccess::Owned
                    && let Some(access) =
                        terminal_semantics::descriptor_results::descriptor_value_access(
                            types,
                            self.structural_type,
                            self.multiplicity,
                        )
                {
                    access
                } else {
                    self.access
                }
            }

            pub(crate) fn payload_contract(&self) -> PayloadMembershipContract {
                PayloadMembershipContract {
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    carrier: self.structural_type,
                    qualifications: self.qualifications.to_vec(),
                    projected_qualifications: self.projected_qualifications.to_vec(),
                }
            }

            pub(crate) fn has_only_element_memberships(&self) -> bool {
                self.qualifications.is_empty()
                    && self
                        .projected_qualifications
                        .iter()
                        .all(|row| row.is_element_membership())
            }

            pub(crate) fn declared_byte_predicates(
                &self,
                domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            ) -> Option<terminal_psi::ByteSequencePredicateSet> {
                if !self.projected_qualifications.is_empty() || self.has_result_evidence() {
                    return None;
                }
                terminal_semantics::byte_qualification_predicates_in(
                    domains.values().copied(),
                    self.structural_type,
                    self.qualifications,
                )
            }

            pub(crate) fn is_unqualified(&self) -> bool {
                self.qualifications.is_empty() && self.projected_qualifications.is_empty()
            }

            pub(crate) fn has_result_evidence(&self) -> bool {
                self.operation_result.is_some_and(|result| {
                    !result.claims.is_empty() || !result.qualification_establishments.is_empty()
                })
            }

            pub(crate) fn carries_qualification(
                &self,
                path: &[StructuralPathSegment],
                domain: StructuralDomainId,
            ) -> bool {
                if path.is_empty() {
                    self.qualifications.contains(&domain)
                } else {
                    self.projected_qualifications.iter().any(|qualification| {
                        StructuralPathQualification::selected_suffix(&qualification.path, path)
                            .is_some_and(|suffix| suffix.is_empty())
                            && qualification.domain == domain
                    })
                }
            }

            pub(crate) fn memberships_disjoint_from_write(
                &self,
                path: &[StructuralPathSegment],
            ) -> bool {
                terminal_semantics::structural_memberships_disjoint_from_write(
                    self.qualifications,
                    self.projected_qualifications,
                    path,
                )
            }

            pub(crate) fn restorable_qualifications(
                &self,
                path: &[StructuralPathSegment],
            ) -> Option<(Vec<StructuralDomainId>, Vec<StructuralPathQualification>)> {
                StructuralPathQualification::select_restorable(
                    self.qualifications,
                    self.projected_qualifications,
                    path,
                )
            }

            pub(crate) fn supplies_qualifications(
                &self,
                path: &[StructuralPathSegment],
                required: &[StructuralDomainId],
                projected: &[StructuralPathQualification],
            ) -> bool {
                required
                    .iter()
                    .all(|domain| self.carries_qualification(path, *domain))
                    && projected.iter().all(|qualification| {
                        let mut selected = path.to_vec();
                        selected.extend(qualification.path.iter().cloned());
                        self.carries_qualification(&selected, qualification.domain)
                    })
            }

            pub(crate) fn reference_ancestor_qualifications_preserved<'types>(
                &self,
                types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
                domains: impl Iterator<Item = &'types StructuralDomainDeclaration> + Clone,
                identities: &TerminalIdentityTable,
                path: &[StructuralPathSegment],
                access: StructuralAccess,
            ) -> bool {
                reference_preserves_ancestor_qualifications(
                    types,
                    domains,
                    identities,
                    self.qualifications,
                    self.projected_qualifications,
                    path,
                    access,
                )
            }

            pub(crate) fn reference_memberships<'types>(
                &self,
                types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
                domains: impl Iterator<Item = &'types StructuralDomainDeclaration> + Clone,
                identities: &TerminalIdentityTable,
                path: &[StructuralPathSegment],
                access: StructuralAccess,
                reference: StructuralTypeId,
                storage: Option<&terminal_psi::ReferenceStorageView>,
            ) -> Result<Vec<StructuralPathQualification>, &'static str> {
                if !reference_preserves_ancestor_qualifications(
                    types.clone(),
                    domains,
                    identities,
                    self.qualifications,
                    self.projected_qualifications,
                    path,
                    access,
                ) {
                    return Err(
                        "reference formation would hide an ancestor qualification obligation",
                    );
                }
                terminal_semantics::reference_carrier_memberships(
                    types,
                    self.structural_type,
                    self.qualifications,
                    self.projected_qualifications,
                    path,
                    access,
                    reference,
                    storage,
                )
            }

            pub(crate) fn selected_qualifications(
                &self,
                path: &[StructuralPathSegment],
            ) -> (Vec<StructuralDomainId>, Vec<StructuralPathQualification>) {
                StructuralPathQualification::select(
                    self.qualifications,
                    self.projected_qualifications,
                    path,
                )
            }

            pub(crate) fn borrowed_qualifications_match(
                &self,
                path: &[StructuralPathSegment],
                parameter: &StructuralParameterDeclaration,
            ) -> bool {
                let (whole, projected) = self.selected_qualifications(path);
                parameter
                    .qualifications
                    .iter()
                    .all(|domain| whole.contains(domain))
                    && parameter
                        .projected_qualifications
                        .iter()
                        .all(|row| projected.contains(row))
                    && (parameter.access == StructuralAccess::SharedBorrow
                        || (whole
                            .iter()
                            .all(|domain| parameter.qualifications.contains(domain))
                            && projected
                                .iter()
                                .all(|row| parameter.projected_qualifications.contains(row))))
            }
        }

        pub(crate) fn structural_arguments_match<'declaration, Formal>(
            caller: &PsiOptimizationFunction,
            consumer: Option<semantic_vocabulary::OperationId>,
            arguments: &[StructuralArgument],
            parameters: &'declaration [Formal],
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            projection: StructuralProjectionPolicy,
        ) -> bool
        where
            StructuralCallFormal<'declaration>: From<&'declaration Formal>,
        {
            structural_arguments_match_at_invocation(
                caller,
                consumer.map(Into::into),
                arguments,
                parameters,
                types,
                domains,
                projection,
            )
        }

        pub(crate) fn structural_arguments_match_at_invocation<'declaration, Formal>(
            caller: &PsiOptimizationFunction,
            consumer: Option<semantic_vocabulary::InvocationSite>,
            arguments: &[StructuralArgument],
            parameters: &'declaration [Formal],
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
            projection: StructuralProjectionPolicy,
        ) -> bool
        where
            StructuralCallFormal<'declaration>: From<&'declaration Formal>,
        {
            if arguments.len() != parameters.len() {
                return false;
            }
            for (argument, parameter) in arguments.iter().zip(parameters) {
                let formal = StructuralCallFormal::from(parameter);
                let parameter = &StructuralParameterDeclaration {
                    place: argument.place,
                    position: formal.position,
                    structural_type: formal.structural_type,
                    multiplicity: formal.multiplicity,
                    access: formal.access,
                    qualifications: formal.qualifications.to_vec(),
                    projected_qualifications: formal.projected_qualifications.to_vec(),
                    additional_input_qualifications: formal
                        .additional_input_qualifications
                        .to_vec(),
                };
                let case_projection = argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::Case(_)));
                if case_projection
                    && (argument.access == StructuralAccess::Owned
                        || matches!(projection, StructuralProjectionPolicy::Boundary))
                {
                    return false;
                }
                if let Some((carrier, qualifications, bytes)) =
                    caller.block_nodes().find_map(|node| match &node.operation {
                        O::EstablishByteSequenceLiteral {
                            place,
                            structural_type,
                            qualifications,
                            bytes,
                            ..
                        } if place.id == argument.place => {
                            Some((structural_type.id, qualifications, bytes))
                        }
                        _ => None,
                    })
                    && !terminal_semantics::byte_literal_qualifications_hold_in(
                        domains.values().copied(),
                        carrier,
                        qualifications,
                        bytes,
                    )
                {
                    return false;
                }

                let Some(source) = structural_source_contract(caller, argument.place) else {
                    return false;
                };
                if types.get(&parameter.structural_type).is_some_and(|row| {
                    matches!(
                        row.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    )
                }) && argument.access != StructuralAccess::Owned
                    && (source.backing_access(types.values().copied()).is_none()
                        || !terminal_semantics::descriptor_source_projection_accessible(
                            types.values().copied(),
                            source.structural_type,
                            source.multiplicity,
                            source.access,
                            &argument.path,
                            argument.access,
                        ))
                {
                    return false;
                }
                let borrowed_owner_expected = parameter.access != StructuralAccess::Owned
                    && types
                        .get(&parameter.structural_type)
                        .is_some_and(|declaration| {
                            declaration.shape.is_bounded_owned_byte_sequence()
                        });
                let bounded_owner_presentation = borrowed_owner_expected
            && projection != StructuralProjectionPolicy::Boundary
            && !source.has_result_evidence()
            && (((source.access != StructuralAccess::Owned
                || !argument.path.is_empty()
                || caller.arrival_parameter(argument.place).is_some_and(|arrival| {
                    abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
                        arrival,
                        |identity| types.get(&identity).copied(),
                    )
                }))
                && terminal_semantics::bounded_byte_owner_capacity_in(
                types.values().copied(),
                domains.values().copied(),
                &StructuralParameterDeclaration {
                    place: argument.place,
                    position: parameter.position,
                    structural_type: source.structural_type,
                    multiplicity: source.multiplicity,
                    access: source.access,
                    qualifications: source.qualifications.to_vec(),
                    projected_qualifications: source.projected_qualifications.to_vec(),
                    additional_input_qualifications: source.additional_input_qualifications.to_vec(),
                },
                argument,
                parameter,
            )
            .is_some())
                || owned_bounded_owner_loan(caller, argument, parameter, &source, types, domains))
            && !caller.entry_claim_declared(argument.place)
            && !caller.content_entry_claim_declared(argument.place);

                if argument.path.contains(&StructuralPathSegment::Referent) {
                    let selected = crate::unit_validation::references::call_source_type(
                        caller, types, argument,
                    );
                    let admitted = !matches!(projection, StructuralProjectionPolicy::Boundary)
                        && argument.access == parameter.access
                        && parameter.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                        && (bounded_owner_presentation
                            || (selected.is_some_and(|actual| {
                                borrowed_referent_matches(
                                    types.values().copied(),
                                    actual,
                                    parameter.structural_type,
                                )
                            }) && source
                                .borrowed_qualifications_match(&argument.path, parameter)));
                    if !admitted {
                        return false;
                    }
                    continue;
                }
                if argument.access != StructuralAccess::SharedBorrow
                    && parameter.access != StructuralAccess::SharedBorrow
                {
                    let (_, selected) = source.selected_qualifications(&argument.path);
                    for row in selected {
                        let Some(crossing) = row
                            .path
                            .iter()
                            .position(|segment| *segment == StructuralPathSegment::Referent)
                        else {
                            continue;
                        };
                        let mut descriptor = argument.path.clone();
                        descriptor.extend_from_slice(&row.path[..crossing]);
                        let needs_preservation = terminal_semantics::runtime_structural_path_tip(
                            types.values().copied(),
                            source.structural_type,
                            &descriptor,
                        )
                        .is_none_or(|declaration| {
                            !matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::Reference {
                                    access: StructuralAccess::SharedBorrow,
                                    ..
                                }
                            )
                        });
                        if needs_preservation && !parameter.projected_qualifications.contains(&row)
                        {
                            return false;
                        }
                    }
                }
                if argument.access != StructuralAccess::Owned
                    && !source
                        .backing_access(types.values().copied())
                        .is_some_and(|access| structural_access_can_supply(access, argument.access))
                {
                    return false;
                }
                let static_borrowed_path = !argument.path.is_empty()
                    && matches!(
                        (source.access, argument.access),
                        (
                            StructuralAccess::MutableBorrow,
                            StructuralAccess::SharedBorrow
                                | StructuralAccess::MutableBorrow
                                | StructuralAccess::WriteOnlyBorrow
                        ) | (
                            StructuralAccess::SharedBorrow,
                            StructuralAccess::SharedBorrow
                        ) | (
                            StructuralAccess::WriteOnlyBorrow,
                            StructuralAccess::WriteOnlyBorrow
                        ) | (
                            StructuralAccess::Owned,
                            StructuralAccess::MutableBorrow
                                | StructuralAccess::WriteOnlyBorrow
                                | StructuralAccess::SharedBorrow
                        )
                    )
                    && argument.path.iter().all(|segment| match segment {
                        StructuralPathSegment::Field(identity) => {
                            *identity != terminal_psi::TerminalIdentityIndex::ABSENT
                        }
                        StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::Case(_)
                        | StructuralPathSegment::RuntimeIndex { .. } => true,
                        StructuralPathSegment::AllElements
                        | StructuralPathSegment::Referent
                        | StructuralPathSegment::RepresentationView { .. }
                        | StructuralPathSegment::FixedRange { .. } => false,
                    });
                let path_shape_matches = match projection {
                    StructuralProjectionPolicy::Unit => {
                        argument.path.is_empty()
                            || static_borrowed_path
                            || is_nonempty_field_path(&argument.path)
                            || matches!(
                                argument.path.as_slice(),
                                [StructuralPathSegment::FixedIndex(_)]
                                    | [
                                        StructuralPathSegment::FixedIndex(_),
                                        StructuralPathSegment::FixedIndex(_),
                                    ]
                            )
                            || (source.access == StructuralAccess::Owned
                                && argument.access == StructuralAccess::Owned
                                && source.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Affine
                                && parameter.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Affine
                                && is_partial_affine_path(
                                    types,
                                    source.structural_type,
                                    &argument.path,
                                ))
                    }
                    StructuralProjectionPolicy::Projected
                    | StructuralProjectionPolicy::Boundary => true,
                };
                let completed_result = source.operation_result.is_some_and(|result| {
                    matches!(
                        result.multiplicity,
                        terminal_psi::StructuralMultiplicity::Affine
                            | terminal_psi::StructuralMultiplicity::Unrestricted
                    ) && result.claims.is_empty()
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.qualification_establishments.is_empty()
                }) && !caller.entry_claim_declared(argument.place)
                    && !caller.content_entry_claim_declared(argument.place);
                let completed_affine_result = completed_result
                    && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine;
                let unrestricted_mutable_subloan = static_borrowed_path
                    && argument.access == StructuralAccess::MutableBorrow
                    && parameter.access == StructuralAccess::MutableBorrow
                    && source.access == StructuralAccess::MutableBorrow
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
                let unrestricted_shared_subloan = (is_nonempty_field_path(&argument.path)
                    || static_borrowed_path)
                    && argument.access == StructuralAccess::SharedBorrow
                    && parameter.access == StructuralAccess::SharedBorrow
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
                let view_loan = argument.access != StructuralAccess::Owned
                    && matches!(
                        source.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                    && types
                        .get(&source.structural_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::ElementView { .. }
                            ) || (argument.path.is_empty()
                                && declaration.shape.is_borrowed_byte_view())
                        })
                    && structural_access_can_supply(source.access, argument.access)
                    && argument.access == parameter.access
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
                let buffer_presentation = (projection == StructuralProjectionPolicy::Boundary
                    || (projection == StructuralProjectionPolicy::Unit
                        && unrestricted_mutable_subloan))
                    && terminal_semantics::boundary_buffer_capacity(
                        types.values().copied(),
                        source.structural_type,
                        argument,
                        parameter,
                    )
                    .is_some();
                let shared_buffer_presentation = parameter.multiplicity
                    == terminal_psi::StructuralMultiplicity::Unrestricted
                    && matches!(
                        source.multiplicity,
                        terminal_psi::StructuralMultiplicity::Unrestricted
                            | terminal_psi::StructuralMultiplicity::Affine
                    )
                    && source.projected_qualifications.is_empty()
                    && !source.has_result_evidence()
                    && terminal_semantics::shared_boundary_buffer_capacity(
                        types.values().copied(),
                        source.structural_type,
                        argument,
                        parameter,
                    )
                    .is_some();
                let view_field_presentation = (projection == StructuralProjectionPolicy::Boundary
                    || (projection == StructuralProjectionPolicy::Unit
                        && unrestricted_shared_subloan))
                    && terminal_semantics::stored_view_field_presentation(
                        types.values().copied(),
                        source.structural_type,
                        argument,
                        parameter,
                    )
                    .is_some_and(|declared| {
                        byte_parameter_predicates_in(domains.values().copied(), parameter)
                            .is_some_and(|required| declared.implies(required))
                    });
                let byte_field_presentation =
                    buffer_presentation || shared_buffer_presentation || view_field_presentation;
                let presents_fixed_array = |actual: &StructuralParameterDeclaration| {
                    terminal_semantics::fixed_byte_array_loan(
                        types.values().copied(),
                        domains.values().copied(),
                        actual,
                        argument,
                        parameter,
                    )
                    .is_some()
                        || terminal_semantics::fixed_element_array_extent(
                            types.values().copied(),
                            actual,
                            argument,
                            parameter,
                        )
                        .is_some()
                };
                let fixed_array_presentation = matches!(
                    projection,
                    StructuralProjectionPolicy::Unit
                        | StructuralProjectionPolicy::Projected
                        | StructuralProjectionPolicy::Boundary
                ) && !source.has_result_evidence()
                    && presents_fixed_array(&StructuralParameterDeclaration {
                        place: argument.place,
                        position: parameter.position,
                        structural_type: source.structural_type,
                        multiplicity: source.multiplicity,
                        access: source.access,
                        qualifications: source.qualifications.to_vec(),
                        projected_qualifications: source.projected_qualifications.to_vec(),
                        additional_input_qualifications: source
                            .additional_input_qualifications
                            .to_vec(),
                    })
                    && !caller.entry_claim_declared(argument.place)
                    && !caller.content_entry_claim_declared(argument.place);
                if borrowed_owner_expected && !bounded_owner_presentation {
                    return false;
                }
                if matches!(
                    argument.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) && types
                    .get(&source.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_composite_shape())
                {
                    let Some((whole, projected)) = StructuralPathQualification::select_restorable(
                        source.qualifications,
                        source.projected_qualifications,
                        &argument.path,
                    ) else {
                        return false;
                    };
                    if !bounded_owner_presentation
                        && (whole != parameter.qualifications
                            || projected != parameter.projected_qualifications)
                    {
                        return false;
                    }
                }
                let actual_type = if argument.access == parameter.access
                    && argument.access != StructuralAccess::Owned
                {
                    terminal_semantics::borrowed_primitive_projection_declaration(
                        types.values().copied(),
                        source.structural_type,
                        &argument.path,
                        Some(parameter.structural_type),
                    )
                } else {
                    None
                }
                .or_else(|| {
                    resolve_runtime_projection(types, source.structural_type, &argument.path)
                });
                let type_matches = actual_type.is_some_and(|actual| {
                    actual == parameter.structural_type
                        || (argument.access != StructuralAccess::Owned
                            && borrowed_referent_matches(
                                types.values().copied(),
                                actual,
                                parameter.structural_type,
                            ))
                });
                if actual_type.is_none()
                    && !byte_field_presentation
                    && !fixed_array_presentation
                    && !bounded_owner_presentation
                {
                    return false;
                }
                let unrestricted_owned_mutable_subloan = static_borrowed_path
                    && argument.access == StructuralAccess::MutableBorrow
                    && parameter.access == StructuralAccess::MutableBorrow
                    && source.access == StructuralAccess::Owned
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && (source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                        || completed_affine_result)
                    && type_matches;
                if (!path_shape_matches && !fixed_array_presentation)
                    || (!type_matches
                        && !fixed_array_presentation
                        && !byte_field_presentation
                        && !bounded_owner_presentation)
                    || argument.access != parameter.access
                    || (!structural_access_can_supply(source.access, argument.access)
                        && !(source.access == StructuralAccess::MutableBorrow
                            && argument.access == StructuralAccess::Owned
                            && !argument.path.is_empty()))
                {
                    return false;
                }
                let indexed_write_only_path_is_material = || {
                    !argument
                        .path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                        || (is_material_write_only_type(types, source.structural_type)
                            && actual_type
                                .and_then(|actual| types.get(&actual))
                                .is_some_and(|declaration| {
                                    matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                                            | terminal_psi::StructuralTypeShape::Record { .. }
                                    )
                                }))
                };
                let unrestricted_write_only_subloan = !argument.path.is_empty()
                    && argument.access == StructuralAccess::WriteOnlyBorrow
                    && parameter.access == StructuralAccess::WriteOnlyBorrow
                    && matches!(
                        source.access,
                        StructuralAccess::Owned
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    )
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && indexed_write_only_path_is_material();
                if projection == StructuralProjectionPolicy::Unit
                    && argument.access == StructuralAccess::WriteOnlyBorrow
                    && argument
                        .path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                    && !unrestricted_write_only_subloan
                {
                    return false;
                }
                let affine_owner_loan = argument.access != StructuralAccess::Owned
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && source.access == StructuralAccess::Owned
                    && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    && (argument.access == StructuralAccess::SharedBorrow
                        || (argument.path.is_empty()
                            && types
                                .get(&source.structural_type)
                                .is_some_and(|declaration| {
                                    !declaration.shape.is_borrowed_byte_carrier()
                                        && !matches!(
                                            declaration.shape,
                                            terminal_psi::StructuralTypeShape::ElementView { .. }
                                        )
                                })));
                let completed_result_loan = completed_result
                    && argument.access != StructuralAccess::Owned
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
                let actual_multiplicity = if affine_owner_loan
                    || completed_result_loan
                    || view_loan
                    || shared_buffer_presentation
                {
                    terminal_psi::StructuralMultiplicity::Unrestricted
                } else if argument.path.is_empty() {
                    source.multiplicity
                } else if unrestricted_write_only_subloan
                    || fixed_array_presentation
                    || bounded_owner_presentation
                    || unrestricted_mutable_subloan
                    || unrestricted_owned_mutable_subloan
                    || unrestricted_shared_subloan
                    || (buffer_presentation
                        && source.access == StructuralAccess::MutableBorrow
                        && source.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted)
                {
                    terminal_psi::StructuralMultiplicity::Unrestricted
                } else if parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    && ((source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && is_partial_affine_path(types, source.structural_type, &argument.path))
                        || (argument.access == StructuralAccess::Owned
                            && source.access == StructuralAccess::MutableBorrow))
                {
                    terminal_psi::StructuralMultiplicity::Affine
                } else {
                    terminal_psi::StructuralMultiplicity::Linear
                };
                let copied_byte_predicates = projection != StructuralProjectionPolicy::Boundary
            && argument.path.is_empty()
            && argument.access == StructuralAccess::Owned
            && parameter.access == StructuralAccess::Owned
            && source.structural_type == parameter.structural_type
            && source.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && parameter.projected_qualifications.is_empty()
            && !caller.entry_claim_declared(argument.place)
            && !caller.content_entry_claim_declared(argument.place)
            && byte_parameter_predicates_in(
                domains.values().copied(),
                parameter,
            )
            .zip(consumer.and_then(|consumer| {
                crate::unit_validation::function_structure::copied_field_predicates_at_invocation(
                    types.values().copied(),
                    caller,
                    argument.place,
                    consumer,
                )
            }))
            .is_some_and(|(required, available)| available.implies(required));
                let byte_content_qualifications = argument.path.is_empty()
                    && argument.access == StructuralAccess::SharedBorrow
                    && parameter.access == StructuralAccess::SharedBorrow
                    && types.values().any(|row| {
                        row.id == parameter.structural_type
                            && matches!(
                                row.shape,
                                terminal_psi::StructuralTypeShape::ByteSequence(_)
                            )
                    })
                    && byte_parameter_predicates_in(domains.values().copied(), parameter)
                        .zip(whole_source_predicates(
                            types.values().copied(),
                            domains.values().copied(),
                            caller,
                            argument.place,
                        ))
                        .is_some_and(|(required, available)| available.implies(required));
                if actual_multiplicity != parameter.multiplicity
                    || (!bounded_owner_presentation
                        && !view_field_presentation
                        && !copied_byte_predicates
                        && !byte_content_qualifications
                        && parameter.qualifications.iter().any(|qualification| {
                            !source.carries_qualification(&argument.path, *qualification)
                        }))
                    || parameter
                        .projected_qualifications
                        .iter()
                        .any(|qualification| {
                            let mut path = argument.path.clone();
                            path.extend(qualification.path.iter().cloned());
                            !source.carries_qualification(&path, qualification.domain)
                        })
                    || (projection == StructuralProjectionPolicy::Unit
                        && !bounded_owner_presentation
                        && !argument.path.is_empty()
                        && (!source.qualifications.is_empty()
                            || ((unrestricted_write_only_subloan
                                || unrestricted_mutable_subloan
                                || unrestricted_owned_mutable_subloan)
                                && !parameter.qualifications.is_empty())))
                {
                    return false;
                }
            }
            for first in 0..arguments.len() {
                for second in first + 1..arguments.len() {
                    let left = &arguments[first];
                    let right = &arguments[second];
                    if left.place == right.place
                        && structural_paths_may_overlap(&left.path, &right.path)
                        && (structural_access_is_exclusive(left.access)
                            || structural_access_is_exclusive(right.access)
                            || ((left.access == StructuralAccess::Owned
                                || right.access == StructuralAccess::Owned)
                                && structural_source_contract(caller, left.place).is_some_and(
                                    |source| {
                                        source.access == StructuralAccess::Owned
                                            && source.multiplicity
                                                == terminal_psi::StructuralMultiplicity::Affine
                                    },
                                )))
                    {
                        return false;
                    }
                }
            }
            true
        }

        fn is_material_write_only_type(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            use terminal_psi::{StructuralFieldType, StructuralTypeShape};
            let mut pending = vec![root];
            let mut visited = BTreeSet::new();
            while let Some(current) = pending.pop() {
                if !visited.insert(current) {
                    continue;
                }
                let Some(declaration) = types.get(&current) else {
                    return false;
                };
                match &declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::AtomicCell(_) => {}
                    StructuralTypeShape::Record { fields, .. } => {
                        for field in fields {
                            if field.relevance.is_erased() {
                                return false;
                            }
                            match field.field_type {
                                StructuralFieldType::Scalar(_)
                                | StructuralFieldType::Atomic(_)
                                | StructuralFieldType::AtomicBoundedInteger(_)
                                | StructuralFieldType::IeeeFloat(_) => {}
                                StructuralFieldType::Structural(child) => pending.push(child),
                                StructuralFieldType::ByteSequence(_)
                                | StructuralFieldType::PredicateByteSequence { .. }
                                | StructuralFieldType::RefinedScalar(_)
                                | StructuralFieldType::BoundedInteger(_)
                                | StructuralFieldType::Erased { .. } => return false,
                            }
                        }
                    }
                    StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
                    StructuralTypeShape::Opaque
                    | StructuralTypeShape::Reference { .. }
                    | StructuralTypeShape::ByteSequence(_)
                    | StructuralTypeShape::ElementView { .. }
                    | terminal_psi::sum_or_mixed_shape_pattern!() => return false,
                }
            }
            true
        }

        pub(crate) fn owned_bounded_owner_loan(
            caller: &PsiOptimizationFunction,
            argument: &StructuralArgument,
            parameter: &StructuralParameterDeclaration,
            source: &StructuralSourceContract<'_>,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            if !argument.path.is_empty()
                || !matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || parameter.access != argument.access
                || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
                || source.access != StructuralAccess::Owned
                || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !source.projected_qualifications.is_empty()
                || !source.operation_result.is_some_and(|result| {
                    result.place == argument.place
                        && result.structural_type == source.structural_type
                        && result.multiplicity == source.multiplicity
                        && result.claims.is_empty()
                        && result.qualification_establishments.is_empty()
                })
                || !types
                    .get(&source.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
                || source.structural_type != parameter.structural_type
            {
                return false;
            }
            let Some(required) = byte_parameter_predicates_in(domains.values().copied(), parameter)
            else {
                return false;
            };
            if argument.access != StructuralAccess::SharedBorrow
                && !source
                    .declared_byte_predicates(domains)
                    .is_some_and(|declared| required.implies(declared))
            {
                return false;
            }
            required.is_empty()
                || whole_source_predicates(
                    types.values().copied(),
                    domains.values().copied(),
                    caller,
                    argument.place,
                )
                .is_some_and(|available| available.implies(required))
        }

        pub(crate) fn structural_source_contract(
            caller: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<StructuralSourceContract<'_>> {
            caller
                .arrival_parameter(place)
                .map(|parameter| StructuralSourceContract {
                    caller,
                    place,
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    qualifications: &parameter.qualifications,
                    projected_qualifications: &parameter.projected_qualifications,
                    additional_input_qualifications: &parameter.additional_input_qualifications,
                    operation_result: None,
                })
                .or_else(|| {
                    caller
                        .block_nodes()
                        .filter_map(|node| {
                            structural_operation_result_contract(caller, &node.operation)
                        })
                        .find(|source| {
                            source
                                .operation_result
                                .is_some_and(|result| result.place == place)
                        })
                })
                .or_else(|| trivial_affine_local_contract(caller, place))
                .or_else(|| {
                    caller.block_nodes().find_map(|node| {
                        let O::EstablishByteSequenceLiteral {
                            place: declaration,
                            structural_type,
                            qualifications,
                            ..
                        } = &node.operation
                        else {
                            return None;
                        };
                        (declaration.id == place).then_some(StructuralSourceContract {
                            caller,
                            place,
                            structural_type: structural_type.id,
                            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::SharedBorrow,
                            qualifications: qualifications.as_slice(),
                            projected_qualifications: &[],
                            additional_input_qualifications: &[],
                            operation_result: None,
                        })
                    })
                })
        }

        fn trivial_affine_local_contract(
            caller: &PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<StructuralSourceContract<'_>> {
            caller.block_nodes().find_map(|node| match &node.operation {
                O::EstablishTrivialAffineLocal {
                    place: declaration,
                    structural_type,
                    ..
                } if declaration.id == place => Some(StructuralSourceContract {
                    caller,
                    place,
                    structural_type: structural_type.id,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: &[],
                    projected_qualifications: &[],
                    additional_input_qualifications: &[],
                    operation_result: None,
                }),
                _ => None,
            })
        }

        pub(crate) fn structural_operation_result_contract<'a>(
            caller: &'a PsiOptimizationFunction,
            operation: &'a O,
        ) -> Option<StructuralSourceContract<'a>> {
            let metadata = operation.structural_result_metadata()?;
            let access = match metadata.role {
                abstract_operations::StructuralResultRole::OwnedValue => StructuralAccess::Owned,
                abstract_operations::StructuralResultRole::PrimitiveLocalStorage => {
                    StructuralAccess::MutableBorrow
                }
                abstract_operations::StructuralResultRole::ViewRecipe => {
                    view_access(caller, operation)?
                }
            };
            let result = metadata.result;
            Some(StructuralSourceContract {
                caller,
                place: result.place,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access,
                qualifications: &result.qualifications,
                projected_qualifications: &result.projected_qualifications,
                additional_input_qualifications: &result.additional_input_qualifications,
                operation_result: Some(result),
            })
        }

        fn view_access(
            caller: &PsiOptimizationFunction,
            operation: &O,
        ) -> Option<StructuralAccess> {
            use semantic_vocabulary::StructuralPlaceKind;
            use terminal_psi::{
                StructuralAccess as Access, StructuralMultiplicity as Multiplicity,
            };

            let (_, initial) = operation.structural_result()?;
            let mut place = initial.place;
            let mut current = Some(operation);
            let mut expected_type = Some(initial.structural_type);
            let mut requested = None;
            let mut required = Access::SharedBorrow;
            for _ in 0..caller.structural_places.len() {
                let mut declarations = caller
                    .structural_places
                    .iter()
                    .filter(|row| row.id == place);
                let declaration = declarations.next()?;
                if declarations.next().is_some() {
                    return None;
                }
                if let Some(parameter) = caller.arrival_parameter(place) {
                    if expected_type.is_some_and(|identity| identity != parameter.structural_type)
                        || (expected_type.is_some()
                            && !matches!(
                                parameter.access,
                                Access::SharedBorrow | Access::MutableBorrow
                            ))
                        || !structural_access_can_supply(parameter.access, required)
                    {
                        return None;
                    }
                    return requested;
                }
                let (producer, structural_type) = match declaration.kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } => (producer, structural_type),
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if required == Access::SharedBorrow
                        && expected_type.is_none_or(|identity| identity == structural_type) =>
                    {
                        return requested;
                    }
                    _ => return None,
                };
                let recorded = caller
                    .block_nodes()
                    .find(|node| node.operation.psi_operation() == Some(producer))?;
                let operation = current.take().unwrap_or(&recorded.operation);
                if operation != &recorded.operation {
                    return None;
                }
                let (actual_producer, result) = operation.structural_result()?;
                if producer != actual_producer
                    || result.place != place
                    || result.structural_type != structural_type
                    || expected_type.is_some_and(|identity| identity != structural_type)
                {
                    return None;
                }
                match operation {
                    element_view_or_byte_subslice_pattern!(source, ..) => {
                        if !result.qualifications.is_empty()
                            || result
                                .projected_qualifications
                                .iter()
                                .any(|row| !row.is_element_membership())
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                        {
                            return None;
                        }
                        let access = match result.multiplicity {
                            Multiplicity::Unrestricted => Access::SharedBorrow,
                            Multiplicity::Affine => Access::MutableBorrow,
                            _ => return None,
                        };
                        if !structural_access_can_supply(access, required) {
                            return None;
                        }
                        requested.get_or_insert(access);
                        required = access;
                        if !structural_access_can_supply(source.access, required) {
                            return None;
                        }
                        expected_type = if source.path.is_empty() {
                            Some(
                                if let Some(parameter) = caller.arrival_parameter(source.place) {
                                    parameter.structural_type
                                } else {
                                    let declaration = caller
                                        .structural_places
                                        .iter()
                                        .find(|declaration| declaration.id == source.place)?;
                                    match declaration.kind {
                                        StructuralPlaceKind::OperationResult {
                                            structural_type,
                                            ..
                                        }
                                        | StructuralPlaceKind::ByteSequenceLiteral {
                                            structural_type,
                                            ..
                                        } => structural_type,
                                        _ => return None,
                                    }
                                },
                            )
                        } else {
                            None
                        };
                        place = source.place;
                    }
                    O::EstablishElementView {
                        source,
                        destination,
                        ..
                    } => {
                        if *destination != result.place
                            || !matches!(
                                (source.access, result.multiplicity),
                                (Access::SharedBorrow, Multiplicity::Unrestricted)
                                    | (Access::MutableBorrow, Multiplicity::Affine)
                            )
                            || !structural_access_can_supply(source.access, required)
                            || !result.qualifications.is_empty()
                            || result
                                .projected_qualifications
                                .iter()
                                .any(|row| !row.is_element_membership())
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                        {
                            return None;
                        }
                        requested.get_or_insert(source.access);
                        required = source.access;
                        expected_type = None;
                        place = source.place;
                    }
                    _ => {
                        let source = structural_operation_result_contract(caller, operation)?;
                        let access = if expected_type.is_some() && source.access == Access::Owned {
                            operation.materialized_structural_result()?;
                            match source.multiplicity {
                                Multiplicity::Unrestricted => Access::SharedBorrow,
                                Multiplicity::Affine => Access::MutableBorrow,
                                Multiplicity::Linear => return None,
                            }
                        } else {
                            source.access
                        };
                        return if structural_access_can_supply(access, required) {
                            requested
                        } else {
                            None
                        };
                    }
                }
            }
            None
        }

        pub(crate) fn structural_access_can_supply(
            source: StructuralAccess,
            presented: StructuralAccess,
        ) -> bool {
            match source {
                StructuralAccess::Owned => true,
                StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => matches!(
                    presented,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ),
                StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
            }
        }

        pub(crate) fn structural_access_is_exclusive(access: StructuralAccess) -> bool {
            matches!(
                access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
        }

        pub(crate) fn is_nonempty_field_path(path: &[StructuralPathSegment]) -> bool {
            !path.is_empty()
                && path
                    .iter()
                    .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        }

        pub(crate) fn is_partial_affine_path(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> bool {
            !path.is_empty() && resolve_structural_path(types, root, path).is_some()
        }
    }
    mod structural_cases {
        use crate::OptimizationUnitValidationError;
        use crate::unit_validation::operation_contracts::structural_access::structural_source_contract;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
        use std::collections::BTreeMap;

        pub(super) fn validate(
            function: &PsiOptimizationFunction,
            source: PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
            successors: &[abstract_operations::AbstractStructuralCaseSuccessor],
            blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> Result<(), OptimizationUnitValidationError> {
            let invalid = || OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
                machine: function.machine,
                source,
            };
            let signature = structural_source_contract(function, source).ok_or_else(invalid)?;
            if signature.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return Err(invalid());
            }
            let selected = crate::unit_validation::structural_catalog::structural_projection_type(
                types.values().copied(),
                signature.structural_type,
                path,
            )
            .ok_or_else(invalid)?;
            let declaration = types.get(&selected).ok_or_else(invalid)?;
            let cases = match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases }
                | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
                _ => return Err(invalid()),
            };
            if successors.len() != cases.len() {
                return Err(invalid());
            }
            for (successor, case) in successors.iter().zip(cases) {
                if successor.case != case.id {
                    return Err(invalid());
                }
                let target = blocks.get(&successor.target).ok_or_else(invalid)?;
                if successor.payloads.len() != target.parameters.len()
                    || successor.structural_bindings.len() != target.structural_parameters.len()
                {
                    return Err(invalid());
                }
                for (payload, parameter) in successor.payloads.iter().zip(&target.parameters) {
                    let field = case
                        .fields
                        .iter()
                        .find(|field| field.id == payload.field)
                        .ok_or_else(invalid)?;
                    if payload.parameter != parameter.value
                        || payload.scalar_type != parameter.scalar_type
                        || field.relevance.is_erased()
                        || field.field_type.scalar_type() != Some(payload.scalar_type)
                    {
                        return Err(invalid());
                    }
                }
            }
            Ok(())
        }
    }
    mod values {
        use crate::OptimizationUnitValidationError;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{PsiOptimizationFunction, ValueDefinition, ValueDefinitionSite};
        use semantic_vocabulary::{BlockId, StructuralTypeId, ValueId};
        use std::collections::BTreeMap;

        pub(super) fn collect_value_definitions(
            function: &PsiOptimizationFunction,
        ) -> Result<BTreeMap<ValueId, ValueDefinition>, OptimizationUnitValidationError> {
            let mut definitions = BTreeMap::new();
            for definition in function.parameters.iter().chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter().chain(block.definitions())),
            ) {
                if definitions.insert(definition.value, *definition).is_some() {
                    return Err(OptimizationUnitValidationError::DuplicateValue(
                        definition.value,
                    ));
                }
            }
            Ok(definitions)
        }

        pub(super) fn validate_node_uses(
            function: &PsiOptimizationFunction,
            block: &optimization_unit::OptimizationBlock,
            node_index: usize,
            node: &optimization_unit::OptimizationNode,
            definitions: &BTreeMap<ValueId, ValueDefinition>,
            dominators: &optimization_unit::DominatorSets,
        ) -> Result<(), OptimizationUnitValidationError> {
            for use_site in &node.uses {
                let Some(definition) = definitions.get(&use_site.value) else {
                    return Err(OptimizationUnitValidationError::UndefinedValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                };
                match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => {}
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == block.id => {
                        if usize::try_from(node).expect("u32 fits usize") >= node_index {
                            return Err(OptimizationUnitValidationError::UseBeforeDefinition {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                }
            }
            Ok(())
        }

        pub(super) fn validate_successor_bindings(
            function: &PsiOptimizationFunction,
            node: &optimization_unit::OptimizationNode,
            definitions: &BTreeMap<ValueId, ValueDefinition>,
            blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
            structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> Result<(), OptimizationUnitValidationError> {
            if let O::StructuralCase {
                source,
                path,
                cases,
            } = &node.operation
            {
                return super::structural_cases::validate(
                    function,
                    *source,
                    path,
                    cases,
                    blocks,
                    structural_types,
                );
            }
            for edge in &node.successors {
                let target = blocks.get(&edge.target).expect("successor validated");
                if edge.bindings.len() != target.parameters.len() {
                    return Err(OptimizationUnitValidationError::BindingArityMismatch {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    });
                }
                for (binding, parameter) in edge.bindings.iter().zip(&target.parameters) {
                    let source_type = definitions
                        .get(&binding.argument)
                        .map(|row| row.scalar_type);
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || source_type != Some(parameter.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::BindingTypeMismatch {
                            machine: function.machine,
                            edge: edge.psi_edge,
                            value: binding.argument,
                        });
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) use records::constructible;
    pub(crate) use structural_access::{
        owned_bounded_owner_loan, structural_access_can_supply,
        structural_operation_result_contract, structural_source_contract,
    };

    pub(crate) fn validate_values_and_bindings(
        unit: &optimization_unit::PsiOptimizationUnit,
        function: &PsiOptimizationFunction,
        blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
        predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
        structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        structural_domains: &BTreeMap<
            StructuralDomainId,
            &terminal_psi::StructuralDomainDeclaration,
        >,
        call_accesses: &crate::current_ownership::CallAccesses,
    ) -> Result<(), OptimizationUnitValidationError> {
        let definitions = values::collect_value_definitions(function)?;
        let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
        atomic_coherence::validate_function_atomic_coherence(
            function,
            structural_types,
            predecessors,
            &dominators,
        )?;
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                values::validate_node_uses(
                    function,
                    block,
                    node_index,
                    node,
                    &definitions,
                    &dominators,
                )?;
                node_contracts::validate_node_operation_contracts(
                    unit,
                    function,
                    block,
                    node_index,
                    node,
                    &definitions,
                    functions,
                    boundary_machines,
                    services,
                    structural_types,
                    structural_domains,
                    call_accesses,
                )?;
                values::validate_successor_bindings(
                    function,
                    node,
                    &definitions,
                    blocks,
                    structural_types,
                )?;
            }
        }
        Ok(())
    }
}
pub(crate) mod references {
    use crate::OptimizationUnitValidationError;
    use crate::StructuralTypes;
    use crate::unit_validation::catalog_mismatch;
    use crate::unit_validation::operation_contracts::{constructible, structural_source_contract};
    use abstract_operations::AbstractOperation as O;
    use abstract_operations::AbstractStructuralCallResult;
    use optimization_unit::PsiOptimizationFunction;
    use semantic_vocabulary::MachineId;
    use semantic_vocabulary::ReferenceOriginExtent;
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::ReferenceOriginProjection;
    use terminal_psi::StructuralAccess;
    use terminal_psi::StructuralArgument;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralTypeDeclaration;
    use terminal_psi::TerminalDynamicDispatchCatalog;
    use terminal_semantics::reference_source_projection_accessible;
    use terminal_semantics::reference_source_storage_path;

    mod access_paths {
        use super::{
            PsiOptimizationFunction, StructuralTypeId, access_covers, leaf_type,
            reference_source_type, structural_source_contract,
        };
        use crate::StructuralTypes;
        use std::collections::BTreeMap;

        pub(crate) fn call_source_type(
            function: &PsiOptimizationFunction,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            source: &terminal_psi::StructuralArgument,
        ) -> Option<StructuralTypeId> {
            if !source
                .path
                .contains(&terminal_psi::StructuralPathSegment::Referent)
            {
                return reference_source_type(function, types, source);
            }
            if source
                .path
                .iter()
                .any(|segment| segment.runtime_index().is_some())
            {
                return None;
            }
            let (current, prefix_start) = access_referent_tail(function, types, source)?;
            if source.access != terminal_psi::StructuralAccess::Owned
                && let Some(endpoint) =
                    terminal_semantics::borrowed_primitive_projection_declaration(
                        types.values().copied(),
                        current,
                        &source.path[prefix_start..],
                        None,
                    )
            {
                return Some(endpoint);
            }
            leaf_type(types, current, &source.path[prefix_start..])
        }

        pub(crate) fn access_referent_tail(
            function: &PsiOptimizationFunction,
            types: &(impl StructuralTypes + ?Sized),
            source: &terminal_psi::StructuralArgument,
        ) -> Option<(StructuralTypeId, usize)> {
            if source.access == terminal_psi::StructuralAccess::Owned {
                return None;
            }
            let signature = structural_source_contract(function, source.place)?;
            if signature.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || signature.has_result_evidence()
            {
                return None;
            }
            project_referent_tail(
                types,
                signature.structural_type,
                signature.backing_access(types.declarations())?,
                source,
            )
        }

        pub(crate) fn project_referent_tail(
            types: &(impl StructuralTypes + ?Sized),
            mut current: StructuralTypeId,
            mut authority: terminal_psi::StructuralAccess,
            source: &terminal_psi::StructuralArgument,
        ) -> Option<(StructuralTypeId, usize)> {
            if source.access == terminal_psi::StructuralAccess::Owned {
                return None;
            }
            let mut prefix_start = 0;
            for (position, segment) in source.path.iter().enumerate() {
                match segment {
                    terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
                    | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => {}
                    terminal_psi::StructuralPathSegment::Referent => {
                        current =
                            access_path_type(types, current, &source.path[prefix_start..position])?;
                        let (referent, access) =
                            types
                                .declaration(current)
                                .and_then(|declaration| match declaration.shape {
                                    terminal_psi::StructuralTypeShape::Reference {
                                        referent,
                                        access,
                                    } => Some((referent, access)),
                                    _ => None,
                                })?;
                        if authority == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                            return None;
                        }
                        authority = if access_covers(authority, access) {
                            access
                        } else if access_covers(access, authority) {
                            authority
                        } else {
                            return None;
                        };
                        current = referent;
                        prefix_start = position + 1;
                    }
                    _ => return None,
                }
            }
            if !access_covers(authority, source.access) {
                return None;
            }
            Some((current, prefix_start))
        }

        pub(crate) fn access_path_type(
            types: &(impl StructuralTypes + ?Sized),
            mut current: StructuralTypeId,
            path: &[terminal_psi::StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            for segment in path {
                current = match segment {
                    terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => {
                        match types.declaration(current)?.shape {
                            terminal_psi::StructuralTypeShape::FixedArray { element, .. }
                            | terminal_psi::StructuralTypeShape::ElementView { element, .. } => {
                                element
                            }
                            _ => return None,
                        }
                    }
                    _ => leaf_type(types, current, std::slice::from_ref(segment))?,
                };
            }
            Some(current)
        }
    }
    pub(crate) use access_paths::{
        access_path_type, access_referent_tail, call_source_type, project_referent_tail,
    };

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct ReferenceIdentity {
        pub(crate) place: PlaceId,
        pub(crate) path: Vec<StructuralPathSegment>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct ReferenceOriginIdentity {
        pub(crate) place: PlaceId,
        pub(crate) path: Vec<ReferenceOriginProjection>,
        pub(crate) extent: ReferenceOriginExtent,
    }

    impl From<ReferenceIdentity> for ReferenceOriginIdentity {
        fn from(identity: ReferenceIdentity) -> Self {
            Self {
                place: identity.place,
                extent: ReferenceOriginExtent::Projected,
                path: identity
                    .path
                    .into_iter()
                    .map(ReferenceOriginProjection::Exact)
                    .collect(),
            }
        }
    }

    impl ReferenceOriginIdentity {
        pub(crate) fn exact_identity(&self) -> Option<ReferenceIdentity> {
            if self.extent != ReferenceOriginExtent::Projected {
                return None;
            }
            Some(ReferenceIdentity {
                place: self.place,
                path: self
                    .path
                    .iter()
                    .map(|projection| match projection {
                        ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                        ReferenceOriginProjection::Element => None,
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ReferenceParent {
        Root(ReferenceOrigin),
        Reference(ReferenceIdentity),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum ReferenceOrigin {
        Invocation {
            result: ReferenceIdentity,
            carrier_type: StructuralTypeId,
            path: Vec<ReferenceOriginProjection>,
            extent: ReferenceOriginExtent,
        },
        Primitive(PlaceId),
        IngressLeaf(ReferenceOriginIdentity),
        Alternatives {
            place: PlaceId,
            origins: Vec<ReferenceOriginIdentity>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LiveReference {
        pub(crate) identity: ReferenceIdentity,
        pub(crate) carrier: PlaceId,
        pub(crate) carrier_path: Vec<StructuralPathSegment>,
        pub(crate) root: ReferenceOrigin,
        pub(crate) parent: ReferenceParent,
    }

    pub(crate) fn contains_reference(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        root: StructuralTypeId,
    ) -> bool {
        terminal_psi::structural_type_contains_reference_by(|id| types.get(&id).copied(), root)
    }

    pub(crate) fn leaf_paths(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        root: StructuralTypeId,
        maximum_leaves: usize,
    ) -> Option<Vec<Vec<StructuralPathSegment>>> {
        Some(
            terminal_semantics::reference_leaves(types.values().copied(), root, maximum_leaves)?
                .into_iter()
                .map(|leaf| leaf.path)
                .collect(),
        )
    }

    pub(crate) fn referent(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
    ) -> Option<StructuralTypeId> {
        match types.get(&structural_type)?.shape {
            terminal_psi::StructuralTypeShape::Reference {
                referent,
                access:
                    StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow,
            } => types.contains_key(&referent).then_some(referent),
            _ => None,
        }
    }

    pub fn reference_projection(
        function: &PsiOptimizationFunction,
        types: &[StructuralTypeDeclaration],
        destination: PlaceId,
        path: &[StructuralPathSegment],
        required_access: StructuralAccess,
    ) -> Option<StructuralTypeId> {
        let source = structural_source_contract(function, destination)?;
        (source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            && !source.has_result_evidence()
            && reference_source_projection_accessible(
                types.iter(),
                source.structural_type,
                source.multiplicity,
                source.access,
                path,
                required_access,
            )
            && !function.entry_claim_declared(destination)
            && !function.content_entry_claim_declared(destination))
        .then_some(source.structural_type)
    }

    pub(crate) fn record_field_destination<'path>(
        function: &PsiOptimizationFunction,
        types: &(impl StructuralTypes + ?Sized),
        destination: PlaceId,
        path: &'path [StructuralPathSegment],
        required_access: StructuralAccess,
    ) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
        if !path.contains(&StructuralPathSegment::Referent) {
            return None;
        }
        let (referent, prefix) = access_referent_tail(
            function,
            types,
            &StructuralArgument {
                place: destination,
                path: path.to_vec(),
                access: required_access,
            },
        )?;
        Some((referent, &path[prefix..]))
    }

    pub(crate) fn leaf_referent(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        current: StructuralTypeId,
        fields: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let current = leaf_type(types, current, fields)?;
        referent(types, current).or_else(|| {
            types
                .get(&current)
                .map(|declaration| match declaration.shape {
                    terminal_psi::StructuralTypeShape::Reference { referent, .. } => referent,
                    _ => current,
                })
        })
    }

    pub(crate) fn leaf_type(
        types: &(impl StructuralTypes + ?Sized),
        mut current: StructuralTypeId,
        fields: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        let mut payload_fields = None;
        for segment in fields {
            if let StructuralPathSegment::Case(case) = segment {
                if payload_fields.is_some() {
                    return None;
                }
                let cases = types.declaration(current)?.shape.sum_cases()?;
                let mut matching = cases.iter().filter(|candidate| candidate.id == *case);
                payload_fields = Some(matching.next()?.fields.as_slice());
                if matching.next().is_some() {
                    return None;
                }
                continue;
            }
            if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
                return None;
            }
            if let StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } = segment
            {
                if matches!(
                    types.declaration(current)?.shape,
                    terminal_psi::StructuralTypeShape::Reference { .. }
                ) {
                    return None;
                }
                current = terminal_semantics::representation_view_declaration(
                    types.declarations(),
                    *structural_type,
                    *stored_scalar,
                )?
                .id;
                continue;
            }
            if let StructuralPathSegment::FixedIndex(index) = segment {
                let declaration = types.declaration(current)?;
                let terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } = &declaration.shape
                else {
                    return None;
                };
                if index >= length {
                    return None;
                }
                current = *element;
                continue;
            }
            let StructuralPathSegment::Field(identity) = segment else {
                return None;
            };
            let declaration = types.declaration(current)?;
            let selected_fields = payload_fields
                .take()
                .or_else(|| declaration.shape.direct_fields())?;
            let field = terminal_psi::live_field(selected_fields, *identity)?;
            current = match &field.field_type {
                terminal_psi::StructuralFieldType::Structural(child) => *child,
                terminal_psi::StructuralFieldType::Scalar(scalar) => types
                    .declarations()
                    .find(|declaration| {
                        matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::PrimitiveScalar(shape)
                                if shape == *scalar
                        )
                    })
                    .map(|declaration| declaration.id)?,
                _ => return None,
            };
        }
        payload_fields.is_none().then_some(current)
    }

    pub(crate) fn reference_access(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
    ) -> Option<StructuralAccess> {
        types
            .get(&structural_type)
            .and_then(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
    }

    pub(crate) use terminal_psi::structural_access_covers as access_covers;

    pub(crate) fn leaf_access_within(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        source: StructuralAccess,
    ) -> bool {
        leaf_type(types, root, path)
            .and_then(|leaf| reference_access(types, leaf))
            .is_some_and(|granted| access_covers(source, granted))
    }

    pub(crate) fn projected_carrier_type(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: &StructuralArgument,
    ) -> Option<StructuralTypeId> {
        let (StructuralPathSegment::Referent, fields) = source.path.split_last()? else {
            return None;
        };
        let current = structural_source_contract(function, source.place)?.structural_type;
        leaf_referent(types, current, fields)
    }

    pub(crate) fn is_reference_projection(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: &StructuralArgument,
    ) -> bool {
        projected_carrier_type(function, types, source).is_some()
            && source.access != StructuralAccess::Owned
    }

    pub(crate) fn storage_projection(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: &StructuralArgument,
    ) -> Option<super::structural_catalog::StructuralStorageProjection> {
        if source.path.last() == Some(&StructuralPathSegment::Referent) {
            return projected_carrier_type(function, types, source)
                .map(super::structural_catalog::StructuralStorageProjection::Endpoint);
        }
        let contract = structural_source_contract(function, source.place)?;
        super::structural_catalog::structural_storage_projection(
            types.values().copied(),
            contract.structural_type,
            &source.path,
        )
    }

    pub(crate) fn reference_source_accessible(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: &StructuralArgument,
    ) -> bool {
        if source.access == StructuralAccess::Owned {
            return false;
        }
        let Some(contract) = structural_source_contract(function, source.place) else {
            return false;
        };
        if contract.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || contract.has_result_evidence()
        {
            return false;
        }
        let path = reference_source_storage_path(
            types.values().copied(),
            contract.structural_type,
            &source.path,
        );
        if !reference_source_projection_accessible(
            types.values().copied(),
            contract.structural_type,
            contract.multiplicity,
            contract.access,
            path,
            source.access,
        ) {
            return false;
        }
        path.contains(&StructuralPathSegment::Referent)
            || (!function.entry_claim_declared(source.place)
                && !function.content_entry_claim_declared(source.place))
    }

    pub(crate) fn reference_source_type(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: &StructuralArgument,
    ) -> Option<StructuralTypeId> {
        if !reference_source_accessible(function, types, source) {
            return None;
        }
        let contract = structural_source_contract(function, source.place)?;
        let path = reference_source_storage_path(
            types.values().copied(),
            contract.structural_type,
            &source.path,
        );
        if !source.path.contains(&StructuralPathSegment::Referent) {
            let contract = structural_source_contract(function, source.place)?;
            if !access_covers(contract.access, source.access)
                || contract.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || !contract.is_unqualified()
                || contract.has_result_evidence()
                || function.entry_claim_declared(source.place)
                || function.content_entry_claim_declared(source.place)
            {
                return None;
            }
            return super::structural_catalog::structural_borrowed_storage_projection(
                types.values().copied(),
                contract.structural_type,
                &source.path,
                source.access,
            )
            .and_then(|projection| match projection {
                super::structural_catalog::StructuralStorageProjection::Endpoint(endpoint) => {
                    Some(endpoint)
                }
                _ => None,
            });
        }
        let source = StructuralArgument {
            place: source.place,
            access: source.access,
            path: path.to_vec(),
        };
        projected_carrier_type(function, types, &source)
    }

    fn direct_reference_parameter<'a>(
        function: &'a PsiOptimizationFunction,
        source: &StructuralArgument,
    ) -> Option<&'a terminal_psi::StructuralParameterDeclaration> {
        let parameter = function.structural_parameter(source.place)?;
        (matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        ) && access_covers(parameter.access, source.access)
            && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
            && !function.entry_claim_declared(source.place)
            && !function.content_entry_claim_declared(source.place))
        .then_some(parameter)
    }

    pub(crate) fn formal_origin(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        function: &PsiOptimizationFunction,
        source: &StructuralArgument,
    ) -> Option<ReferenceOrigin> {
        let parameter = function.structural_parameter(source.place)?;
        let path = reference_source_storage_path(
            types.values().copied(),
            parameter.structural_type,
            &source.path,
        );
        if path.is_empty()
            && matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            )
        {
            Some(ReferenceOrigin::Primitive(source.place))
        } else if let Some((StructuralPathSegment::Referent, path)) = path.split_last()
            && matches!(
                parameter.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
        {
            Some(ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place: source.place,
                    path: path.to_vec(),
                }
                .into(),
            ))
        } else if !path.is_empty()
            && direct_reference_parameter(function, source).is_some()
            && path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RepresentationView { .. }
                )
            })
        {
            Some(ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place: source.place,
                    path: path.to_vec(),
                }
                .into(),
            ))
        } else {
            None
        }
    }

    pub(crate) struct FormalReferenceSource {
        pub(crate) endpoint: StructuralTypeId,
        pub(crate) extent: ReferenceOriginExtent,
        pub(crate) transfers: bool,
    }

    pub(crate) fn formal_reference_source(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        function: &PsiOptimizationFunction,
        origin: &terminal_psi::ReferenceOriginSource,
    ) -> Option<FormalReferenceSource> {
        let parameter = function.structural_parameter(origin.place)?;
        let binding = terminal_semantics::bind_reference_origin(
            types.values().copied(),
            parameter.structural_type,
            origin,
        )
        .ok()?;
        let transfers = binding.transfers_existing(
            types.values().copied(),
            parameter.structural_type,
            parameter.access,
        );
        let source = binding.source;
        let endpoint = binding.endpoint;
        reference_source_type(function, types, &source)?;
        formal_origin(types, function, &source)?;
        Some(FormalReferenceSource {
            endpoint,
            extent: binding.extent,
            transfers,
        })
    }

    fn operation_structural_result(
        operation: &O,
    ) -> Option<&terminal_psi::StructuralOperationResult> {
        match operation {
            abstract_operations::establish_set_quint_pattern!(result, ..)
            | abstract_operations::establish_case_or_record_result_pattern!(result)
            | O::EstablishZeroedStructural { result, .. }
            | O::EstablishReference { result, .. }
            | O::EstablishAccessReference { result, .. }
            | O::MoveStructuralField { result, .. }
            | O::MoveStructuralValue { result, .. }
            | O::TransformContent { result, .. }
            | O::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } => Some(result),
            O::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some(result),
            _ => None,
        }
    }

    pub(crate) fn validate_function_references(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    ) -> Result<(), OptimizationUnitValidationError> {
        let invalid = || catalog_mismatch(function);
        let is_reference = |structural_type| {
            types.get(&structural_type).is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Reference { .. }
                )
            })
        };
        for node in function.block_nodes() {
            let Some(result) = operation_structural_result(&node.operation) else {
                continue;
            };
            if let O::MoveStructuralValue { source, .. } | O::TransformContent { source, .. } =
                &node.operation
            {
                let signature =
                    structural_source_contract(function, source.place).ok_or_else(invalid)?;
                if source.access != StructuralAccess::Owned
                    || signature.access != StructuralAccess::Owned
                    || terminal_semantics::canonical_static_projection(
                        types.values().copied(),
                        signature.structural_type,
                        &source.path,
                    )
                    .is_none_or(|(_, selected)| selected.id != result.structural_type)
                {
                    return Err(invalid());
                }
                continue;
            }
            if contains_reference(types, result.structural_type)
                && !is_reference(result.structural_type)
                && (!matches!(
                    node.operation,
                    O::EstablishRecord { .. }
                        | O::EstablishStructuralArray { .. }
                        | O::CallStructural {
                            result: AbstractStructuralCallResult::Structural { .. },
                            ..
                        }
                ) || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                    || !result.qualifications.is_empty()
                    || !result.claims.is_empty())
            {
                return Err(invalid());
            }
            if is_reference(result.structural_type)
                && !matches!(
                    node.operation,
                    O::EstablishReference { .. }
                        | O::EstablishAccessReference { .. }
                        | O::CallStructural {
                            result: AbstractStructuralCallResult::Structural { .. },
                            ..
                        }
                )
            {
                return Err(invalid());
            }
        }
        for parameter in function
            .structural_parameters
            .iter()
            .filter(|parameter| contains_reference(types, parameter.structural_type))
        {
            if parameter.multiplicity
                != if parameter.access == StructuralAccess::Owned {
                    terminal_psi::StructuralMultiplicity::Affine
                } else {
                    terminal_psi::StructuralMultiplicity::Unrestricted
                }
                || !parameter.qualifications.is_empty()
                || !constructible(types, parameter.structural_type)
                || function.entry_claim_declared(parameter.place)
                || function.content_entry_claim_declared(parameter.place)
            {
                return Err(invalid());
            }
        }
        for parameter in function
            .block_structural_parameters()
            .filter(|parameter| contains_reference(types, parameter.structural_type))
        {
            if parameter.access != StructuralAccess::Owned
                || !matches!(
                    parameter.multiplicity,
                    terminal_psi::StructuralMultiplicity::Affine
                        | terminal_psi::StructuralMultiplicity::Unrestricted
                )
                || !parameter.qualifications.is_empty()
                || !constructible(types, parameter.structural_type)
            {
                return Err(invalid());
            }
        }
        if function
            .block_edges()
            .flat_map(|edge| &edge.structural_bindings)
            .any(|binding| {
                structural_source_contract(function, binding.argument.place)
                    .is_some_and(|source| contains_reference(types, source.structural_type))
                    && binding.argument.access != StructuralAccess::Owned
            })
        {
            return Err(invalid());
        }
        let Some(result) = function.result.structural() else {
            return Ok(());
        };
        if !contains_reference(types, result.structural_type) {
            return if result.reference_sources.is_empty() {
                Ok(())
            } else {
                Err(invalid())
            };
        }
        let constructible_result = match types
            .get(&result.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => {
                constructible(types, *referent)
            }
            _ => false,
        };
        if referent(types, result.structural_type).is_none()
            && !constructible(types, result.structural_type)
            && !constructible_result
        {
            return Err(invalid());
        }
        if result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
        {
            return Err(invalid());
        }
        let mut paths = leaf_paths(
            types,
            result.structural_type,
            result.reference_sources.len(),
        )
        .ok_or_else(invalid)?;
        paths.sort();
        if paths.len() != result.reference_sources.len() {
            return Err(invalid());
        }
        if result.reference_sources.iter().any(|mapping| {
            mapping.sources.len() > 1
                || mapping.sources.iter().any(|source| {
                    matches!(
                        source,
                        terminal_psi::StructuralReferenceOrigin::Invocation { .. }
                    ) || source.extent() == ReferenceOriginExtent::WithinRegion
                        || source.path().contains(&ReferenceOriginProjection::Element)
                })
        }) {
            validate_alternative_result_dependencies(function, functions, types, dynamic_dispatch)?;
        }
        let mut sources = BTreeMap::new();
        for (path, mapping) in paths.iter().zip(&result.reference_sources) {
            let expected_referent =
                leaf_referent(types, result.structural_type, path).ok_or_else(invalid)?;
            if mapping.path != *path
                || mapping.sources.is_empty()
                || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid());
            }
            for formal in &mapping.sources {
                let terminal_psi::StructuralReferenceOrigin::Parameter(formal) = formal else {
                    let terminal_psi::StructuralReferenceOrigin::Invocation {
                        carrier_type,
                        path: source_path,
                        extent,
                        access,
                    } = formal
                    else {
                        unreachable!()
                    };
                    if *extent != ReferenceOriginExtent::Projected
                        || terminal_semantics::reference_origin_type(
                            types.values().copied(),
                            *carrier_type,
                            source_path,
                        ) != Some(expected_referent)
                        || !leaf_access_within(types, result.structural_type, path, *access)
                    {
                        return Err(invalid());
                    }
                    continue;
                };
                let projected =
                    formal_reference_source(types, function, formal).ok_or_else(invalid)?;
                let shared_descendant = leaf_type(types, result.structural_type, path)
                    .and_then(|leaf| reference_access(types, leaf))
                    == Some(StructuralAccess::SharedBorrow)
                    && !projected.transfers
                    && function.structural_parameters.iter().any(|parameter| {
                        parameter.place == formal.place
                            && parameter.access != StructuralAccess::Owned
                    });
                if !matches!(
                    formal.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
                ) || sources
                    .insert((formal.place, formal.path.clone()), shared_descendant)
                    .is_some_and(|previous_shared| !previous_shared || !shared_descendant)
                    || !function.structural_parameter(formal.place).is_some()
                    || (projected.extent == ReferenceOriginExtent::Projected
                        && projected.endpoint != expected_referent)
                    || !leaf_access_within(types, result.structural_type, path, formal.access)
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    fn validate_alternative_result_dependencies(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    ) -> Result<(), OptimizationUnitValidationError> {
        let invalid = || catalog_mismatch(function);
        let mut pending = vec![(function.machine, false)];
        let mut active = BTreeSet::new();
        let mut completed = BTreeSet::new();
        while let Some((current, exiting)) = pending.pop() {
            if exiting {
                active.remove(&current);
                completed.insert(current);
                continue;
            }
            if completed.contains(&current) {
                continue;
            }
            if !active.insert(current) {
                return Err(invalid());
            }
            pending.push((current, true));
            let current = functions.get(&current).ok_or_else(invalid)?;
            for node in current.block_nodes() {
                if let O::CallStructural {
                    callee,
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } = &node.operation
                    && contains_reference(types, result.structural_type)
                {
                    match callee {
                        abstract_operations::AbstractCallTarget::Machine(machine) => {
                            pending.push((*machine, false));
                        }
                        abstract_operations::AbstractCallTarget::Dynamic {
                            interface,
                            slot,
                            ..
                        } => {
                            for table in &dynamic_dispatch.tables {
                                if table.interface == *interface {
                                    let row = table.rows.get(*slot as usize).ok_or_else(invalid)?;
                                    if let Some(machine) = row {
                                        pending.push((*machine, false));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn access_reference_source_compatible(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: StructuralTypeId,
        result: StructuralTypeId,
        correspondence: &abstract_operations::AccessReferenceBacking,
    ) -> bool {
        source == correspondence.source_type()
            && matches!(types.get(&result).map(|row| &row.shape),
            Some(terminal_psi::StructuralTypeShape::Reference { referent, access })
                if *referent == correspondence.carrier()
                    && matches!(access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow))
    }
}
mod runtime_evidence {
    use crate::OptimizationUnitValidationError;
    use abstract_operations::{AbstractOperation as O, AbstractStructuralCallResult as R};
    use optimization_unit::PsiOptimizationUnit;
    use terminal_psi::{DirectCallRole, EvidenceContractLaneKind};
    use terminal_semantics::evidence_calls::{
        RuntimeEvidenceBody, RuntimeEvidenceInputs, RuntimeEvidenceOperation,
        RuntimeEvidenceResult, validate_runtime_evidence_inputs_in,
    };

    pub(super) fn validate(
        unit: &PsiOptimizationUnit,
    ) -> Result<(), OptimizationUnitValidationError> {
        validate_current(unit)
            .map_err(|reason| OptimizationUnitValidationError::InvalidRuntimeEvidence { reason })
    }
    fn validate_current(unit: &PsiOptimizationUnit) -> Result<(), &'static str> {
        let Some(catalog) = &unit.runtime_evidence else {
            if unit
                .functions
                .iter()
                .any(|function| !function.evidence_contract_lanes.is_empty())
            {
                return Err("current evidence lanes have no verified declaration catalog");
            }
            return Ok(());
        };
        if catalog.psi() != unit.psi {
            return Err("runtime evidence declaration catalog belongs to another Terminal input");
        }
        let calls = catalog
            .calls()
            .iter()
            .filter(|call| unit.function(call.caller).is_some())
            .cloned()
            .collect::<Vec<_>>();
        let lanes = unit
            .functions
            .iter()
            .flat_map(|function| function.evidence_contract_lanes.iter().cloned())
            .collect::<Vec<_>>();
        for function in &unit.functions {
            let expected = catalog
                .lanes()
                .iter()
                .filter(|lane| lane.machine == function.machine)
                .collect::<Vec<_>>();
            if !function.evidence_contract_lanes.iter().eq(expected) {
                return Err("optimizer changed the verified evidence declaration lane");
            }
        }
        let consumes_snapshots = calls
            .iter()
            .any(|call| call.runtime_call.is_some() && !call.evidence_arguments.is_empty());
        let checks_calls = consumes_snapshots
            || lanes
                .iter()
                .any(|lane| lane.kind == EvidenceContractLaneKind::Requires);
        let graph = if consumes_snapshots {
            crate::descriptor_results::optimized_graph(unit)
                .ok_or("runtime evidence has no reconstructible current descriptor graph")?
        } else {
            Vec::new()
        };
        let mut bodies = Vec::with_capacity(unit.functions.len());
        for function in &unit.functions {
            let mut declarations = catalog
                .declarations()
                .iter()
                .filter(|(machine, _)| *machine == function.machine);
            let (_, declaration) = declarations
                .next()
                .ok_or("runtime evidence body lost its checked declaration")?;
            if declarations.next().is_some() {
                return Err("runtime evidence body has ambiguous checked declarations");
            }
            let mut operations = Vec::new();
            for block in function.blocks.iter().filter(|_| checks_calls) {
                for (index, node) in block.nodes.iter().enumerate() {
                    let (
                        site,
                        callee,
                        role,
                        arguments,
                        structural_arguments,
                        result,
                        selected_evidence,
                    ) = match &node.operation {
                        O::Call {
                            psi_operation,
                            callee,
                            arguments,
                            result,
                            scalar_type,
                            ..
                        } => (
                            (*psi_operation).into(),
                            *callee,
                            DirectCallRole::Scalar,
                            arguments.clone(),
                            Vec::new(),
                            RuntimeEvidenceResult::Scalar {
                                id: *result,
                                scalar_type: *scalar_type,
                            },
                            Vec::new(),
                        ),
                        O::CallStructural {
                            psi_operation,
                            callee,
                            arguments,
                            structural_arguments,
                            result,
                            ..
                        } => {
                            let Some(callee) = callee.machine() else {
                                continue;
                            };
                            let (role, result, selected) = match result {
                                R::Unit => (
                                    DirectCallRole::Unit,
                                    RuntimeEvidenceResult::Unit,
                                    Vec::new(),
                                ),
                                R::Scalar(value) => (
                                    DirectCallRole::Scalar,
                                    RuntimeEvidenceResult::Scalar {
                                        id: value.value,
                                        scalar_type: value.scalar_type,
                                    },
                                    Vec::new(),
                                ),
                                R::Structural {
                                    result,
                                    selected_evidence,
                                } => (
                                    DirectCallRole::Structural,
                                    RuntimeEvidenceResult::Structural(result.clone()),
                                    selected_evidence.clone(),
                                ),
                            };
                            (
                                (*psi_operation).into(),
                                callee,
                                role,
                                arguments.clone(),
                                structural_arguments.clone(),
                                result,
                                selected,
                            )
                        }
                        O::InvokeNoNormal {
                            psi_edge,
                            callee,
                            arguments,
                            structural_arguments,
                            ..
                        } => (
                            semantic_vocabulary::InvocationSite::InvokeNoNormal(*psi_edge),
                            *callee,
                            DirectCallRole::NoNormal,
                            arguments.clone(),
                            structural_arguments.clone(),
                            RuntimeEvidenceResult::NoNormal,
                            Vec::new(),
                        ),
                        _ => continue,
                    };
                    operations.push(RuntimeEvidenceOperation {
                        site,
                        position: (
                            block.id,
                            u32::try_from(index)
                                .map_err(|_| "current evidence operation position exceeds u32")?,
                        ),
                        callee,
                        role,
                        arguments,
                        structural_arguments,
                        result,
                        selected_evidence,
                    });
                }
            }
            let observes = calls.iter().any(|call| {
                call.caller == function.machine
                    && call.runtime_call.is_some()
                    && !call.evidence_arguments.is_empty()
            });
            let mut values = Vec::new();
            if observes {
                values.extend(function.parameters.iter().map(|value| value.value));
                for call in &operations {
                    values.extend_from_slice(&call.arguments);
                    if let Some((value, _)) = call.result.scalar() {
                        values.push(value);
                    }
                }
                values.sort_unstable();
                values.dedup();
            }
            let scalar_expressions = values
                .into_iter()
                .filter_map(|value| {
                    crate::descriptor_results::scalar_expression(function, value)
                        .map(|expression| (value, expression))
                })
                .collect();
            let payload_contracts = if observes {
                let current = terminal_semantics::descriptor_results::descriptor_machine(
                    &graph,
                    function.machine,
                )
                .ok_or("current evidence body has no descriptor machine")?;
                current
                    .place_types
                    .keys()
                    .filter_map(|root| {
                        super::operation_contracts::structural_source_contract(function, *root)
                            .map(|source| (*root, source.payload_contract()))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            bodies.push(RuntimeEvidenceBody {
                id: function.machine,
                entry: function.entry,
                declaration: declaration.clone(),
                parameters: function
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .collect(),
                structural_parameters: function.structural_parameters.clone(),
                outcome_specific_ensures: function
                    .verified_contract
                    .as_ref()
                    .ok_or("runtime evidence body has no verified contract")?
                    .outcome_specific_ensures
                    .clone(),
                calls: operations,
                scalar_expressions,
                payload_contracts,
            });
        }
        validate_runtime_evidence_inputs_in(&RuntimeEvidenceInputs {
            identities: &unit.identities,
            structural_types: &unit.structural_types,
            structural_domains: &unit.structural_domains,
            proposition_applications: catalog.applications(),
            evidence_terms: catalog.terms(),
            evidence_contract_lanes: &lanes,
            proof_output_calls: &calls,
            machines: &bodies,
            graph: &graph,
        })
    }
}
pub(crate) mod services {
    use crate::unit_validation::structural_catalog::structural_qualifications_match;
    use crate::{OperationSite, OptimizationUnitValidationError};
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
    use semantic_vocabulary::{
        BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
    };
    use std::collections::{BTreeMap, BTreeSet};

    pub(crate) fn index_service_catalog(
        unit: &PsiOptimizationUnit,
    ) -> Result<
        BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
        OptimizationUnitValidationError,
    > {
        let mut services = BTreeMap::new();
        let mut identities = BTreeSet::new();
        for declaration in unit.services.iter() {
            if services.insert(declaration.id, declaration).is_some() {
                return Err(OptimizationUnitValidationError::DuplicateService(
                    declaration.id,
                ));
            }
            if declaration.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                || !identities.insert(declaration.identity)
            {
                return Err(OptimizationUnitValidationError::InvalidServiceIdentity(
                    declaration.id,
                ));
            }
        }
        for declaration in unit.services.iter() {
            let mut parents = BTreeSet::new();
            for parent in &declaration.parents {
                if *parent == declaration.id
                    || !parents.insert(*parent)
                    || !services.contains_key(parent)
                {
                    return Err(OptimizationUnitValidationError::InvalidServiceParent {
                        service: declaration.id,
                        parent: *parent,
                    });
                }
            }
            if declaration
                .parents
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(OptimizationUnitValidationError::NonCanonicalServiceParents(
                    declaration.id,
                ));
            }
        }

        fn visit(
            id: ServiceId,
            services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
            active: &mut BTreeSet<ServiceId>,
            complete: &mut BTreeSet<ServiceId>,
        ) -> Result<(), OptimizationUnitValidationError> {
            if complete.contains(&id) {
                return Ok(());
            }
            if !active.insert(id) {
                return Err(OptimizationUnitValidationError::RecursiveServiceHierarchy(
                    id,
                ));
            }
            for parent in &services[&id].parents {
                visit(*parent, services, active, complete)?;
            }
            active.remove(&id);
            complete.insert(id);
            Ok(())
        }

        let mut active = BTreeSet::new();
        let mut complete = BTreeSet::new();
        for id in services.keys().copied() {
            visit(id, &services, &mut active, &mut complete)?;
        }
        for declaration in services.values() {
            for parent in &declaration.parents {
                if let Some(ancestor) = services[parent]
                    .parents
                    .iter()
                    .find(|ancestor| !declaration.parents.contains(ancestor))
                {
                    return Err(
                        OptimizationUnitValidationError::IncompleteServiceParentClosure {
                            service: declaration.id,
                            ancestor: *ancestor,
                        },
                    );
                }
            }
        }
        Ok(services)
    }

    pub(crate) fn valid_service_ceiling(
        ceiling: &[ServiceId],
        services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
    ) -> bool {
        let mut seen = BTreeSet::new();
        ceiling.iter().all(|service| {
            seen.insert(*service)
                && services.get(service).is_some_and(|declaration| {
                    declaration
                        .parents
                        .iter()
                        .all(|parent| ceiling.contains(parent))
                })
        }) && ceiling.windows(2).all(|pair| pair[0] < pair[1])
    }

    pub(crate) fn validate_root_service_reach(
        unit: &PsiOptimizationUnit,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if !valid_service_ceiling(&unit.root_service_reach.concrete, services) {
            return Err(OptimizationUnitValidationError::InvalidRootConcreteServiceReach);
        }
        let mut requirement_identities = BTreeSet::new();
        for (index, dependency) in unit
            .root_service_reach
            .installation_dependencies
            .iter()
            .enumerate()
        {
            if dependency.requirement_identity == terminal_psi::TerminalIdentityIndex::ABSENT
                || !requirement_identities.insert(dependency.requirement_identity)
                || !valid_service_ceiling(&dependency.upper_bound, services)
            {
                return Err(
                    OptimizationUnitValidationError::InvalidRootInstallationReachDependency(index),
                );
            }
        }
        if unit
            .root_service_reach
            .installation_dependencies
            .windows(2)
            .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
        {
            return Err(
                OptimizationUnitValidationError::NonCanonicalRootInstallationReachDependencies,
            );
        }
        let derived = derive_root_service_reach(unit, functions, boundaries, services)?;
        if derived.concrete != unit.root_service_reach.concrete {
            return Err(
                OptimizationUnitValidationError::RootConcreteServiceReachMismatch {
                    declared: unit.root_service_reach.concrete.clone(),
                    derived: derived.concrete,
                },
            );
        }
        if derived.installation_dependencies != unit.root_service_reach.installation_dependencies {
            return Err(OptimizationUnitValidationError::RootInstallationReachDependenciesMismatch);
        }
        Ok(())
    }

    pub(crate) fn derive_root_service_reach(
        unit: &PsiOptimizationUnit,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
    ) -> Result<terminal_psi::TerminalRootServiceReach, OptimizationUnitValidationError> {
        let dependencies = unit
            .root_service_reach
            .installation_dependencies
            .iter()
            .map(|dependency| (dependency.requirement_identity, dependency))
            .collect::<BTreeMap<_, _>>();
        let mut pending = vec![unit.entry];
        let mut visited = BTreeSet::new();
        let mut concrete = BTreeSet::new();
        let mut used_dependencies = BTreeSet::new();
        while let Some(machine) = pending.pop() {
            if !visited.insert(machine) {
                continue;
            }
            let function = functions.get(&machine).copied().ok_or(
                OptimizationUnitValidationError::MissingEntryMachine(machine),
            )?;
            concrete.extend(function.declared_service_reach.iter().copied());
            for operation in function.block_nodes().map(|node| &node.operation) {
                pending.extend(
                    operation
                        .cleanup_rosters()
                        .flat_map(|(_, actions)| actions)
                        .filter_map(|action| match action {
                            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                                Some(cleanup.cleanup_machine)
                            }
                            _ => None,
                        }),
                );
                pending.extend(unit.call_machine_references(operation));
                match operation {
                    O::BoundaryCall { boundary, .. } => {
                        let declaration = boundaries.get(boundary).copied().ok_or(
                            OptimizationUnitValidationError::OperationServiceContractMismatch(
                                OperationSite::new(function.machine, function.entry, 0),
                            ),
                        )?;
                        if let Some(dependency) = dependencies.get(&declaration.identity) {
                            let published = declaration
                                .fixed_service_reach
                                .iter()
                                .chain(&dependency.upper_bound)
                                .copied()
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .collect::<Vec<_>>();
                            if declaration.published_service_ceiling != published {
                                return Err(
                                OptimizationUnitValidationError::RootInstallationReachBoundaryMismatch(
                                    *boundary,
                                ),
                            );
                            }
                            concrete.extend(declaration.fixed_service_reach.iter().copied());
                            used_dependencies.insert(declaration.identity);
                        } else {
                            concrete.extend(declaration.published_service_ceiling.iter().copied());
                        }
                    }
                    O::PortWrite { service, .. } => {
                        concrete.insert(*service);
                        if let Some(declaration) = services.get(service) {
                            concrete.extend(declaration.parents.iter().copied());
                        }
                    }
                    _ => {}
                }
            }
        }
        let installation_dependencies = unit
            .root_service_reach
            .installation_dependencies
            .iter()
            .filter(|dependency| used_dependencies.contains(&dependency.requirement_identity))
            .cloned()
            .collect();
        Ok(terminal_psi::TerminalRootServiceReach {
            concrete: concrete.into_iter().collect(),
            installation_dependencies,
        })
    }

    pub(crate) fn refresh_root_service_reach(
        unit: &mut PsiOptimizationUnit,
    ) -> Result<(), OptimizationUnitValidationError> {
        let reach = {
            let functions = unit
                .functions
                .iter()
                .map(|function| (function.machine, function))
                .collect::<BTreeMap<_, _>>();
            let boundaries = unit
                .boundary_machines
                .iter()
                .map(|boundary| (boundary.id, boundary))
                .collect::<BTreeMap<_, _>>();
            let services = unit
                .services
                .iter()
                .map(|service| (service.id, service))
                .collect::<BTreeMap<_, _>>();
            derive_root_service_reach(unit, &functions, &boundaries, &services)?
        };
        unit.root_service_reach = reach;
        Ok(())
    }

    pub(crate) fn validate_provider_service_refinements(
        unit: &PsiOptimizationUnit,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    ) -> Result<(), OptimizationUnitValidationError> {
        for provider in &unit.provider_candidates {
            let invalid = || OptimizationUnitValidationError::InvalidProviderServiceRefinement {
                boundary: provider.boundary,
                candidate: provider.candidate,
            };
            let candidate = functions.get(&provider.candidate).ok_or_else(invalid)?;
            let boundary = boundaries.get(&provider.boundary).ok_or_else(invalid)?;
            if provider.refinement.realized_service_ceiling != candidate.published_service_ceiling
                || provider
                    .refinement
                    .realized_service_ceiling
                    .iter()
                    .any(|service| !boundary.published_service_ceiling.contains(service))
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    pub(crate) fn boundary_structural_signature_matches(
        boundary: &terminal_psi::BoundaryMachineDeclaration,
        types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    ) -> bool {
        if boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
            && boundary.attachment.is_some()
        {
            return false;
        }
        structural_signature_matches(
            &boundary.structural_parameters,
            boundary.attachment,
            types,
            domains,
        ) && boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
            && boundary.requires.iter().all(|requirement| {
                boundary
                    .structural_parameters
                    .get(requirement.argument_index as usize)
                    .is_some_and(|parameter| {
                        domains.get(&requirement.domain).is_some_and(|domain| {
                            terminal_semantics::qualification_path_carrier(
                                types.values().copied(),
                                parameter.structural_type,
                                &requirement.path,
                            ) == Some(domain.carrier)
                        })
                    })
            })
            && boundary
                .normal_qualifications
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && boundary.normal_qualifications.iter().all(|guarantee| {
                boundary
                    .structural_parameters
                    .get(guarantee.argument_index as usize)
                    .is_some_and(|parameter| {
                        parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                            && domains.get(&guarantee.domain).is_some_and(|domain| {
                                terminal_semantics::qualification_path_carrier(
                                    types.values().copied(),
                                    parameter.structural_type,
                                    &guarantee.path,
                                ) == Some(domain.carrier)
                            })
                    })
            })
    }

    pub(crate) fn structural_signature_matches(
        parameters: &[terminal_psi::StructuralParameterDeclaration],
        attachment: Option<StructuralTypeId>,
        types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    ) -> bool {
        if attachment.is_some_and(|attachment| !types.contains_key(&attachment)) {
            return false;
        }
        let mut places = BTreeSet::new();
        parameters.iter().enumerate().all(|(position, parameter)| {
            u32::try_from(position).ok() == Some(parameter.position)
                && places.insert(parameter.place)
                && types.contains_key(&parameter.structural_type)
                && structural_qualifications_match(
                    parameter.structural_type,
                    &parameter.qualifications,
                    domains,
                )
        })
    }
}
pub(crate) mod structural_catalog {
    use crate::OptimizationUnitValidationError;
    use optimization_unit::PsiOptimizationUnit;
    use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
    use std::collections::BTreeMap;

    mod catalog {
        use crate::OptimizationUnitValidationError;
        use crate::unit_validation::structural_catalog::content_projection::validate_structural_content_projection;
        use crate::unit_validation::structural_catalog::type_declarations::{
            storage_graph_error, validate_structural_cases, validate_structural_fields,
        };
        use optimization_unit::PsiOptimizationUnit;
        use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralTypeDeclaration;

        pub(super) fn index_structural_types(
            unit: &PsiOptimizationUnit,
        ) -> Result<
            BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            OptimizationUnitValidationError,
        > {
            terminal_semantics::quotients::validate_quotient_representations(
                &unit.structural_types,
                unit.structural_types.quotients(),
            )
            .map_err(OptimizationUnitValidationError::InvalidQuotientCatalog)?;
            terminal_semantics::quotients::validate_quotient_representation_identities(
                &unit.structural_types,
                unit.structural_types.quotients(),
                &unit.identities,
            )
            .map_err(OptimizationUnitValidationError::InvalidQuotientCatalog)?;
            let mut types = BTreeMap::new();
            let mut type_names = BTreeSet::new();
            for declaration in &unit.structural_types {
                terminal_semantics::validate_inline_carriers(&unit.structural_types, declaration)
                    .map_err(
                    |error| OptimizationUnitValidationError::InvalidInlineCarrier {
                        structural_type: declaration.id,
                        field: error.field,
                        reason: error.reason,
                    },
                )?;
                if types.insert(declaration.id, declaration).is_some() {
                    return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                        declaration.id,
                    ));
                }
                if declaration.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || !type_names.insert(declaration.identity)
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralTypeIdentity(
                            declaration.id,
                        ),
                    );
                }
            }
            if unit
                .structural_types
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            {
                return Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder);
            }
            let storage_graph = terminal_semantics::storage::type_graph::StorageTypeGraph::new(
                unit.structural_types.iter(),
            )
            .map_err(storage_graph_error)?;
            let attachment_storage = storage_graph
                .storage_reachable_from(
                    unit.functions
                        .iter()
                        .filter_map(|function| function.attachment),
                )
                .map_err(storage_graph_error)?;
            let mut referent_ids = BTreeSet::new();
            for referent in unit.structural_types.referents() {
                if types.contains_key(&referent.id) || !referent_ids.insert(referent.id) {
                    return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                        referent.id,
                    ));
                }
                if referent.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || !type_names.insert(referent.identity)
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralTypeIdentity(referent.id),
                    );
                }
            }
            if unit
                .structural_types
                .referents()
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            {
                return Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder);
            }
            for declaration in &unit.structural_types {
                match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Opaque => {
                        if declaration.quotient_carrier.is_some()
                            || declaration.scalar_restriction.is_some()
                            || !declaration.requires.is_empty()
                        {
                            return Err(
                                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(
                                    declaration.id,
                                ),
                            );
                        }
                    }
                    terminal_psi::StructuralTypeShape::Reference { .. } => {}
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => {}
                    terminal_psi::StructuralTypeShape::AtomicCell(cell) => {
                        if !cell.is_supported() || declaration.scalar_restriction.is_some() {
                            return Err(
                                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(
                                    declaration.id,
                                ),
                            );
                        }
                    }
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                        | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) => {}
                    terminal_psi::StructuralTypeShape::FixedArray { .. } => {}
                    terminal_psi::StructuralTypeShape::ElementView { .. } => {}
                    terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                        validate_structural_fields(
                            declaration.id,
                            None,
                            fields,
                            attachment_storage.binary_search(&declaration.id).is_ok(),
                        )?;
                    }
                    terminal_psi::StructuralTypeShape::Sum { cases } => {
                        validate_structural_cases(declaration.id, cases)?;
                    }
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                        validate_structural_fields(declaration.id, None, fields, false)?;
                        validate_structural_cases(declaration.id, cases)?;
                    }
                }
            }
            for declaration in &unit.structural_types {
                let referenced = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Opaque
                    | terminal_psi::StructuralTypeShape::Reference { .. } => Vec::new(),
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                    | terminal_psi::StructuralTypeShape::AtomicCell(_)
                    | terminal_psi::StructuralTypeShape::ByteSequence(_) => Vec::new(),
                    terminal_psi::StructuralTypeShape::Record { fields, .. } => fields
                        .iter()
                        .filter_map(|field| match field.field_type {
                            terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                            _ => None,
                        })
                        .collect(),
                    terminal_psi::StructuralTypeShape::FixedArray { element, .. }
                    | terminal_psi::StructuralTypeShape::ElementView { element, .. } => {
                        vec![*element]
                    }
                    terminal_psi::StructuralTypeShape::Sum { cases } => cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .filter_map(|field| match field.field_type {
                            terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                            _ => None,
                        })
                        .collect(),
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => fields
                        .iter()
                        .chain(cases.iter().flat_map(|case| &case.fields))
                        .filter_map(|field| match field.field_type {
                            terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                            _ => None,
                        })
                        .collect(),
                };
                if let Some(target) = referenced.iter().find(|target| !types.contains_key(target)) {
                    return Err(OptimizationUnitValidationError::UnknownStructuralType(
                        *target,
                    ));
                }
                if let terminal_psi::StructuralTypeShape::Reference {
                    referent, access, ..
                } = &declaration.shape
                {
                    let admissible = match access {
                        terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::SharedBorrow => {
                            referent_ids.contains(referent)
                                || types.get(referent).is_some_and(|referent| {
                                    !matches!(
                                        referent.shape,
                                        terminal_psi::StructuralTypeShape::Reference { .. }
                                    )
                                })
                        }
                        terminal_psi::StructuralAccess::WriteOnlyBorrow => matches!(
                            types.get(referent).map(|referent| &referent.shape),
                            Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
                        ),
                        _ => false,
                    };
                    if !admissible {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralTypeIdentity(
                                declaration.id,
                            ),
                        );
                    }
                }
            }
            storage_graph
                .validate_acyclic()
                .map_err(storage_graph_error)?;
            for declaration in &unit.structural_types {
                if let terminal_psi::StructuralTypeShape::ElementView { element, .. } =
                    declaration.shape
                    && crate::unit_validation::references::contains_reference(&types, element)
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralTypeIdentity(
                            declaration.id,
                        ),
                    );
                }
            }
            Ok(types)
        }

        pub(super) fn index_structural_domains<'unit>(
            unit: &'unit PsiOptimizationUnit,
            types: &BTreeMap<StructuralTypeId, &'unit StructuralTypeDeclaration>,
        ) -> Result<
            BTreeMap<StructuralDomainId, &'unit terminal_psi::StructuralDomainDeclaration>,
            OptimizationUnitValidationError,
        > {
            let mut domains = BTreeMap::new();
            let mut names = BTreeSet::new();
            let mut semantic_domains = BTreeSet::new();
            let mut family_byte_predicates = BTreeMap::new();
            for declaration in unit.structural_domains.iter() {
                if domains.insert(declaration.id, declaration).is_some() {
                    return Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
                        declaration.id,
                    ));
                }
                if declaration.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || !names.insert(declaration.identity)
                    || !semantic_domains.insert((declaration.semantic_domain, declaration.carrier))
                    || family_byte_predicates
                        .insert(declaration.semantic_domain, declaration.byte_predicate)
                        .is_some_and(|previous| {
                            previous.is_none() || previous != declaration.byte_predicate
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralDomainIdentity(
                            declaration.id,
                        ),
                    );
                }
            }
            if unit
                .structural_domains
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            {
                return Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder);
            }
            if let Some(carrier) = unit
                .structural_domains
                .iter()
                .map(|declaration| declaration.carrier)
                .find(|carrier| !types.contains_key(carrier))
            {
                return Err(OptimizationUnitValidationError::UnknownStructuralType(
                    carrier,
                ));
            }
            for declaration in unit.structural_domains.iter() {
                if !declaration.required_memberships_are_well_formed(&unit.structural_domains)
                    || (declaration.complete_predicate_theory
                        && declaration
                            .required_domain_closure(&unit.structural_domains)
                            .is_none_or(|nodes| {
                                nodes.iter().any(|node| {
                                    !node.complete_predicate_theory || node.predicate.is_none()
                                })
                            }))
                    || declaration
                        .establishment_routes
                        .as_deref()
                        .unwrap_or_default()
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || declaration
                        .establishment_routes
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .any(|route| {
                            [route.declaration().identity(), route.identity()]
                                .iter()
                                .any(|identity| {
                                    unit.identities.text(*identity).is_none_or(str::is_empty)
                                })
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralDomainIdentity(
                            declaration.id,
                        ),
                    );
                }
                if declaration.predicate.as_ref().is_some_and(|predicate| {
                    !terminal_semantics::domain_predicate_is_closed(
                        types.values().copied(),
                        declaration.carrier,
                        predicate,
                    )
                }) {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralDomainIdentity(
                            declaration.id,
                        ),
                    );
                }
                if declaration.byte_predicate.is_some()
                    && (declaration.content_projection.is_some()
                        || !declaration.has_no_required_memberships()
                        || declaration.establishment_routes.is_some()
                        || !matches!(
                            types[&declaration.carrier].shape,
                            terminal_psi::StructuralTypeShape::ByteSequence(_)
                        ))
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralDomainIdentity(
                            declaration.id,
                        ),
                    );
                }
                if declaration
                    .content_projection
                    .as_ref()
                    .is_some_and(|projection| {
                        !validate_structural_content_projection(
                            declaration.semantic_domain,
                            declaration.carrier,
                            projection,
                            &unit.identities,
                            types,
                        )
                    })
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralDomainContentProjection(
                            declaration.id,
                        ),
                    );
                }
            }
            Ok(domains)
        }
    }
    mod content_projection {
        use semantic_vocabulary::{
            ContentProjectionExpression, ContentProjectionScalar, StructuralTypeId,
        };
        use std::collections::BTreeMap;

        pub(crate) fn validate_content_projection_scalar(
            value: &ContentProjectionScalar,
            carrier: StructuralTypeId,
            identities: &terminal_psi::TerminalIdentityTable,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            depth: usize,
        ) -> bool {
            if depth > 256 {
                return false;
            }
            match value {
                ContentProjectionScalar::SubjectField(path)
                | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                    if path.is_empty() || path.iter().any(String::is_empty) {
                        return false;
                    }
                    let mut current = carrier;
                    for (index, segment) in path.iter().enumerate() {
                        let Some(declaration) = types.get(&current) else {
                            return false;
                        };
                        let terminal_psi::StructuralTypeShape::Record { fields, .. } =
                            &declaration.shape
                        else {
                            return false;
                        };
                        let Some(identity) = identities.index_of(segment) else {
                            return false;
                        };
                        let Some(field) = terminal_psi::field_by_identity(fields, identity) else {
                            return false;
                        };
                        let last = index + 1 == path.len();
                        match (&field.field_type, last) {
                            (terminal_psi::StructuralFieldType::Structural(next), false) => {
                                current = *next;
                            }
                            (terminal_psi::StructuralFieldType::Scalar(_), true) => {}
                            _ => return false,
                        }
                    }
                    true
                }
                ContentProjectionScalar::Natural(value) => {
                    !value.is_empty()
                        && value.bytes().all(|byte| byte.is_ascii_digit())
                        && (value == "0" || !value.starts_with('0'))
                }
                ContentProjectionScalar::Successor(inner) => {
                    validate_content_projection_scalar(inner, carrier, identities, types, depth + 1)
                }
                ContentProjectionScalar::Add(left, right)
                | ContentProjectionScalar::Subtract(left, right)
                | ContentProjectionScalar::Multiply(left, right) => {
                    validate_content_projection_scalar(left, carrier, identities, types, depth + 1)
                        && validate_content_projection_scalar(
                            right,
                            carrier,
                            identities,
                            types,
                            depth + 1,
                        )
                }
            }
        }

        pub(crate) fn validate_content_projection_expression(
            expression: &ContentProjectionExpression,
            carrier: StructuralTypeId,
            identities: &terminal_psi::TerminalIdentityTable,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            match expression {
                ContentProjectionExpression::IntervalSet(members) => {
                    members.iter().all(|(start, end)| {
                        validate_content_projection_scalar(start, carrier, identities, types, 0)
                            && validate_content_projection_scalar(
                                end, carrier, identities, types, 0,
                            )
                    })
                }
                ContentProjectionExpression::CountedQuantity(magnitude) => {
                    validate_content_projection_scalar(magnitude, carrier, identities, types, 0)
                }
            }
        }

        pub(crate) fn validate_structural_content_projection(
            semantic_domain: semantic_vocabulary::DomainSemanticId,
            carrier: StructuralTypeId,
            projection: &terminal_psi::StructuralContentProjection,
            identities: &terminal_psi::TerminalIdentityTable,
            types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> bool {
            let shape_matches_algebra = matches!(
                (&projection.expression, projection.algebra.kind),
                (
                    ContentProjectionExpression::IntervalSet(_),
                    semantic_vocabulary::ContentAlgebraKind::IntervalSet
                ) | (
                    ContentProjectionExpression::CountedQuantity(_),
                    semantic_vocabulary::ContentAlgebraKind::CountedQuantity
                )
            );
            projection.identity.domain.get() == semantic_domain.get()
                && projection.identity.projection_report_fingerprint != 0
                && !projection.algebra.parameter.is_empty()
                && shape_matches_algebra
                && validate_content_projection_expression(
                    &projection.expression,
                    carrier,
                    identities,
                    types,
                )
                && language_semantics::content::terminal_projection_report_fingerprint(
                    &projection.algebra,
                    &projection.expression,
                ) == projection.identity.projection_report_fingerprint
        }
    }
    mod function_catalog {
        use crate::OptimizationUnitValidationError;
        use crate::unit_validation::catalog_mismatch;
        use crate::unit_validation::services::structural_signature_matches;
        use crate::unit_validation::structural_catalog::paths::{
            resolve_structural_path, structural_projected_qualifications_match,
            structural_qualifications_match,
        };
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{
            BlockId, BoundaryMachineId, ClaimId, OperationId, StructuralDomainId,
            StructuralPlaceKind, StructuralTypeId,
        };
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::EntryClaim;
        use terminal_psi::StructuralPlaceDeclaration;
        use terminal_psi::StructuralTypeDeclaration;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum ValidatorStructuralRootKey {
            Parameter(u32),
            BlockParameter(BlockId, u32),
            Result,
            OperationResult(OperationId),
            ByteSequenceLiteral(u32),
            ProviderAttachment(
                StructuralTypeId,
                semantic_vocabulary::StructuralFieldId,
                BoundaryMachineId,
            ),
            TrivialAffineLocal(u32),
        }

        pub(crate) fn structural_root_key(kind: StructuralPlaceKind) -> ValidatorStructuralRootKey {
            match kind {
                StructuralPlaceKind::Parameter { position, .. } => {
                    ValidatorStructuralRootKey::Parameter(position)
                }
                StructuralPlaceKind::BlockParameter { block, position } => {
                    ValidatorStructuralRootKey::BlockParameter(block, position)
                }
                StructuralPlaceKind::Result => ValidatorStructuralRootKey::Result,
                StructuralPlaceKind::OperationResult { producer, .. } => {
                    ValidatorStructuralRootKey::OperationResult(producer)
                }
                StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal,
                    ..
                } => ValidatorStructuralRootKey::ByteSequenceLiteral(declaration_ordinal),
                StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field,
                    boundary,
                } => ValidatorStructuralRootKey::ProviderAttachment(attachment, field, boundary),
                StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    ..
                } => ValidatorStructuralRootKey::TrivialAffineLocal(declaration_ordinal),
            }
        }

        pub(crate) fn validate_function_structural_catalog(
            function: &PsiOptimizationFunction,
            identities: &terminal_psi::TerminalIdentityTable,
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
        ) -> Result<
            (
                Vec<(StructuralPlaceDeclaration, StructuralTypeDeclaration)>,
                Vec<(StructuralPlaceDeclaration, StructuralTypeDeclaration)>,
            ),
            OptimizationUnitValidationError,
        > {
            let mismatch = || catalog_mismatch(function);
            if !structural_signature_matches(
                &function.structural_parameters,
                function.attachment,
                types,
                domains,
            ) {
                return Err(mismatch());
            }
            let mut parameter_places = BTreeSet::new();
            for (position, parameter) in function.structural_parameters.iter().enumerate() {
                if parameter.position != u32::try_from(position).map_err(|_| mismatch())?
                    || !parameter_places.insert(parameter.place)
                    || !types.contains_key(&parameter.structural_type)
                    || !structural_qualifications_match(
                        parameter.structural_type,
                        &parameter.qualifications,
                        domains,
                    )
                    || !structural_projected_qualifications_match(
                        parameter.structural_type,
                        &parameter.projected_qualifications,
                        types,
                        domains,
                    )
                {
                    return Err(mismatch());
                }
            }
            let mut places = BTreeMap::new();
            for place in &function.structural_places {
                if places.insert(place.id, place.kind).is_some() {
                    return Err(mismatch());
                }
                let known_type = match place.kind {
                    StructuralPlaceKind::BlockParameter { block, position } => function
                        .block(block)
                        .and_then(|candidate| {
                            candidate.structural_parameters.get(position as usize)
                        })
                        .is_some_and(|parameter| {
                            parameter.place == place.id && parameter.position == position
                        }),
                    StructuralPlaceKind::Parameter { position } => function
                        .structural_parameters
                        .get(position as usize)
                        .is_some_and(|parameter| {
                            parameter.place == place.id && parameter.position == position
                        }),
                    StructuralPlaceKind::Result => function
                        .result
                        .structural()
                        .is_some_and(|result| result.place == place.id),
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } => {
                        types.contains_key(&structural_type)
                            && function.block_nodes().any(|node| {
                                node.operation.structural_result().is_some_and(
                                    |(candidate, result)| {
                                        candidate == producer
                                            && result.place == place.id
                                            && result.structural_type == structural_type
                                    },
                                )
                            })
                    }
                    StructuralPlaceKind::ByteSequenceLiteral { .. } => true,
                    StructuralPlaceKind::TrivialAffineLocal { .. } => true,
                    StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                        types.contains_key(&attachment) && function.attachment == Some(attachment)
                    }
                };
                if !known_type {
                    return Err(mismatch());
                }
            }
            for parameter in &function.structural_parameters {
                if places.get(&parameter.place)
                    != Some(&StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    })
                {
                    return Err(mismatch());
                }
                if parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                    && !function.entry_claim_declared(parameter.place)
                {
                    return Err(mismatch());
                }
            }
            if let Some(result) = function.result.structural()
                && (places.get(&result.place) != Some(&StructuralPlaceKind::Result)
                    || !types.contains_key(&result.structural_type)
                    || !structural_qualifications_match(
                        result.structural_type,
                        &result.qualifications,
                        domains,
                    )
                    || !structural_projected_qualifications_match(
                        result.structural_type,
                        &result.projected_qualifications,
                        types,
                        domains,
                    ))
            {
                return Err(mismatch());
            }
            for node in function.block_nodes() {
                let structural_result = node.operation.structural_result();
                if let Some((_, result)) = structural_result
                    && (!structural_qualifications_match(
                        result.structural_type,
                        &result.qualifications,
                        domains,
                    ) || !structural_projected_qualifications_match(
                        result.structural_type,
                        &result.projected_qualifications,
                        types,
                        domains,
                    ))
                {
                    return Err(mismatch());
                }
                let expected = match &node.operation {
                    O::EstablishByteSequenceLiteral { place, .. } => Some((place.id, place.kind)),
                    O::EstablishTrivialAffineLocal { .. } => None,
                    _ => structural_result.map(|(producer, result)| {
                        (
                            result.place,
                            StructuralPlaceKind::OperationResult {
                                producer,
                                structural_type: result.structural_type,
                            },
                        )
                    }),
                };
                if expected.is_some_and(|(place, kind)| places.get(&place) != Some(&kind)) {
                    return Err(mismatch());
                }
            }
            let mut claim_inputs = Vec::new();
            for (index, claim) in function.entry_claim_declarations.iter().enumerate() {
                let expected = ClaimId::new(
                    u64::try_from(index)
                        .map_err(|_| mismatch())?
                        .checked_add(1)
                        .ok_or_else(mismatch)?,
                )
                .ok_or_else(mismatch)?;
                let Some(parameter) = function.structural_parameter(claim.input) else {
                    return Err(mismatch());
                };
                if claim.claim != expected
                    || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    || resolve_structural_path(types, parameter.structural_type, &claim.path)
                        .is_none()
                    || claim_inputs.iter().any(|previous: &&EntryClaim| {
                        previous.input == claim.input
                            && (previous.path.starts_with(&claim.path)
                                || claim.path.starts_with(&previous.path))
                    })
                {
                    return Err(mismatch());
                }
                claim_inputs.push(claim);
            }
            if function
                .block_entry_claims
                .windows(2)
                .any(|pair| pair[0].claim >= pair[1].claim)
            {
                return Err(mismatch());
            }
            let mut block_claim_inputs = Vec::new();
            for claim in &function.block_entry_claims {
                let Some(parameter) = function
                    .block_structural_parameters()
                    .find(|parameter| parameter.place == claim.input)
                else {
                    return Err(mismatch());
                };
                if parameter.access != terminal_psi::StructuralAccess::Owned
                    || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    || resolve_structural_path(types, parameter.structural_type, &claim.path)
                        .is_none()
                    || block_claim_inputs.iter().any(|previous: &&EntryClaim| {
                        previous.input == claim.input
                            && (previous.path.starts_with(&claim.path)
                                || claim.path.starts_with(&previous.path))
                    })
                {
                    return Err(mismatch());
                }
                block_claim_inputs.push(claim);
            }
            if function
                .content_entry_claims
                .iter()
                .enumerate()
                .any(|(index, claim)| {
                    let expected = u64::try_from(index)
                        .ok()
                        .and_then(|index| index.checked_add(1))
                        .and_then(ClaimId::new);
                    let structural_binding_matches = function
                        .entry_claim_declarations
                        .iter()
                        .find(|entry| entry.claim == claim.claim)
                        .is_none_or(|entry| {
                            entry.input == claim.input.root
                                && Some(&claim.input.segments)
                                    == entry
                                        .path
                                        .iter()
                                        .map(|segment| {
                                            match segment {
                                    terminal_psi::StructuralPathSegment::Field(identity) => {
                                        Some(semantic_vocabulary::ContentPlaceSegment::Field(
                                            identities
                                                .text(*identity)
                                                .unwrap_or_default()
                                                .to_string(),
                                        ))
                                    }
                                    terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                                        Some(semantic_vocabulary::ContentPlaceSegment::FixedIndex(
                                            *index,
                                        ))
                                    }
                                    terminal_psi::StructuralPathSegment::AllElements
                                    | terminal_psi::StructuralPathSegment::Case(_)
                                    | terminal_psi::StructuralPathSegment::RepresentationView {
                                        ..
                                    }
                                    | terminal_psi::StructuralPathSegment::Referent
                                    | terminal_psi::StructuralPathSegment::FixedRange { .. }
                                    | terminal_psi::StructuralPathSegment::RuntimeIndex {
                                        ..
                                    } => None,
                                }
                                        })
                                        .collect::<Option<Vec<_>>>()
                                        .as_ref()
                        });
                    expected != Some(claim.claim)
                        || claim.input.version != semantic_vocabulary::ContentPlaceVersion::Entry
                        || !parameter_places.contains(&claim.input.root)
                        || claim.projections.is_empty()
                        || claim.projections.windows(2).any(|pair| pair[0] >= pair[1])
                        || !structural_binding_matches
                })
            {
                return Err(mismatch());
            }
            for projection in function
                .content_entry_claims
                .iter()
                .flat_map(|claim| &claim.projections)
            {
                let owner = domains.values().find_map(|domain| {
                    domain
                        .content_projection
                        .as_ref()
                        .filter(|owner| owner.identity.domain == projection.projection.domain)
                });
                if !owner.is_some_and(|owner| {
                    owner.identity == projection.projection && owner.algebra == projection.algebra
                }) {
                    return Err(
                        OptimizationUnitValidationError::ContentProjectionOwnerMismatch(
                            projection.projection,
                        ),
                    );
                }
            }
            let mut byte_sequence_literals = function
                .structural_places
                .iter()
                .filter_map(|place| match place.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal,
                        structural_type,
                    } => Some((*place, declaration_ordinal, structural_type)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            byte_sequence_literals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
            if byte_sequence_literals.iter().enumerate().any(
                |(expected, (_, declaration_ordinal, _))| {
                    u32::try_from(expected).ok() != Some(*declaration_ordinal)
                },
            ) {
                return Err(
                    OptimizationUnitValidationError::NonCanonicalByteSequenceLiterals(
                        function.machine,
                    ),
                );
            }
            let byte_sequence_literals = byte_sequence_literals
        .into_iter()
        .map(|(place, _, structural_type)| {
            let declaration = types.get(&structural_type).ok_or(
                OptimizationUnitValidationError::UnknownStructuralType(structural_type),
            )?;
            if !matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
            ) {
                return Err(
                    OptimizationUnitValidationError::ByteSequenceLiteralDeclarationRequiresBorrowedView {
                        machine: function.machine,
                        place: place.id,
                    },
                );
            }
            Ok((place, (*declaration).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

            let mut trivial_affine_locals = function
                .structural_places
                .iter()
                .filter_map(|place| match place.kind {
                    StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        structural_type,
                        ..
                    } => Some((*place, declaration_ordinal, structural_type)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            trivial_affine_locals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
            if trivial_affine_locals.iter().enumerate().any(
                |(expected, (_, declaration_ordinal, _))| {
                    u32::try_from(expected).ok() != Some(*declaration_ordinal)
                },
            ) {
                return Err(
                    OptimizationUnitValidationError::NonCanonicalTrivialAffineLocals(
                        function.machine,
                    ),
                );
            }
            let trivial_affine_locals = trivial_affine_locals
        .into_iter()
        .map(|(place, _, structural_type)| {
            let declaration = types.get(&structural_type).ok_or(
                OptimizationUnitValidationError::UnknownStructuralType(structural_type),
            )?;
            if !matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Record { ref fields, .. } if fields.is_empty()
            ) {
                return Err(
                    OptimizationUnitValidationError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                        machine: function.machine,
                        place: place.id,
                    },
                );
            }
            Ok((place, (*declaration).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
            Ok((byte_sequence_literals, trivial_affine_locals))
        }
    }
    mod paths {
        use crate::StructuralTypes;
        use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
        use std::collections::BTreeMap;
        use terminal_psi::StructuralDomainDeclaration;
        use terminal_psi::StructuralFieldType;
        use terminal_psi::StructuralPathSegment;
        use terminal_psi::StructuralTypeDeclaration;
        use terminal_semantics::primitive_projection_declaration;

        pub(crate) fn structural_qualifications_match(
            carrier: StructuralTypeId,
            qualifications: &[StructuralDomainId],
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
                && qualifications.iter().all(|domain| {
                    domains
                        .get(domain)
                        .is_some_and(|domain| domain.carrier == carrier)
                })
        }

        pub(crate) fn structural_projected_qualifications_match(
            root: StructuralTypeId,
            qualifications: &[terminal_psi::StructuralPathQualification],
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
        ) -> bool {
            !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
                && qualifications.iter().all(|qualification| {
                    !qualification.path.is_empty()
                        && !qualification.path.iter().any(|segment| {
                            matches!(segment, StructuralPathSegment::RepresentationView { .. })
                        })
                        && terminal_semantics::qualification_path_carrier(
                            types.values().copied(),
                            root,
                            &qualification.path,
                        )
                        .is_some_and(|projected| {
                            domains
                                .get(&qualification.domain)
                                .is_some_and(|domain| domain.carrier == projected)
                        })
                })
        }

        pub(crate) fn resolve_structural_path(
            types: &(impl StructuralTypes + ?Sized),
            structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            resolve_path(
                types.declarations(),
                |identity| types.declaration(identity),
                structural_type,
                path,
                StructuralFieldType::canonical_leaf_type,
            )
        }

        pub(crate) fn resolve_runtime_projection(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            resolve_structural_path(types, structural_type, path).or_else(|| {
                primitive_projection_declaration(types.values().copied(), structural_type, path)
            })
        }

        pub(crate) fn resolve_leaf_copy_path(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            expected: StructuralTypeId,
        ) -> Option<StructuralTypeId> {
            if path
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
            {
                return None;
            }
            let expected = types.get(&expected).copied()?;
            resolve_path(
                types.values().copied(),
                |identity| types.get(&identity).copied(),
                structural_type,
                path,
                |field| field.leaf_copy_type(Some(expected)),
            )
        }

        pub(crate) fn structural_projection_type<'types>(
            declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralTypeId> {
            resolve_path(
                declarations.clone(),
                |identity| {
                    declarations
                        .clone()
                        .find(|declaration| declaration.id == identity)
                },
                structural_type,
                path,
                abstract_operations::control_flow::owner_loans::loaned_field_type,
            )
            .or_else(|| primitive_projection_declaration(declarations, structural_type, path))
        }

        fn resolve_path<'types>(
            declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            declaration: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
            mut structural_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            field_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
        ) -> Option<StructuralTypeId> {
            declaration(structural_type)?;
            let mut payload_fields = None;
            for (ordinal, segment) in path.iter().enumerate() {
                let declaration = declaration(structural_type)?;
                if let StructuralPathSegment::FixedRange { start, end } = segment {
                    let terminal_psi::StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(length),
                        ..
                    } = declaration.shape
                    else {
                        return None;
                    };
                    if payload_fields.is_some()
                        || start > end
                        || *end > length
                        || !matches!(
                            path.get(ordinal + 1),
                            Some(StructuralPathSegment::RepresentationView { .. })
                        )
                    {
                        return None;
                    }
                    continue;
                }
                if let StructuralPathSegment::Case(case) = segment {
                    if payload_fields.is_some() {
                        return None;
                    }
                    let (terminal_psi::StructuralTypeShape::Sum { cases }
                    | terminal_psi::StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                    else {
                        return None;
                    };
                    payload_fields = Some(
                        cases
                            .iter()
                            .find(|candidate| candidate.id == *case)?
                            .fields
                            .as_slice(),
                    );
                    continue;
                }
                if payload_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
                    return None;
                }
                structural_type = match (segment, &declaration.shape) {
                    (
                        StructuralPathSegment::RepresentationView {
                            structural_type: view,
                            stored_scalar,
                        },
                        shape,
                    ) => {
                        if matches!(shape, terminal_psi::StructuralTypeShape::Reference { .. }) {
                            return None;
                        }
                        terminal_semantics::representation_view_declaration(
                            declarations.clone(),
                            *view,
                            *stored_scalar,
                        )?
                        .id
                    }
                    (StructuralPathSegment::Field(identity), shape) => {
                        let fields = match payload_fields.take() {
                            Some(fields) => fields,
                            None => match shape {
                                terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                                    fields.as_slice()
                                }
                                _ => return None,
                            },
                        };
                        let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                        match &field.field_type {
                            StructuralFieldType::Structural(next) => *next,
                            leaf => field_type(leaf)?.leaf_declaration(declarations.clone())?.id,
                        }
                    }
                    (
                        StructuralPathSegment::FixedIndex(index),
                        terminal_psi::StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        },
                    ) if index < length => *element,
                    (
                        StructuralPathSegment::RuntimeIndex { .. },
                        terminal_psi::StructuralTypeShape::FixedArray { element, .. },
                    ) => *element,
                    (
                        StructuralPathSegment::FixedIndex(_),
                        terminal_psi::StructuralTypeShape::ElementView { element, .. },
                    ) => *element,
                    (
                        StructuralPathSegment::RuntimeIndex { .. },
                        terminal_psi::StructuralTypeShape::ElementView { element, .. },
                    ) => *element,
                    _ => return None,
                };
            }
            payload_fields.is_none().then_some(structural_type)
        }

        pub(crate) fn byte_carrier_leaf_window(
            types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
            expected: StructuralTypeId,
        ) -> bool {
            if path
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
            {
                return false;
            }
            let Some((StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last() else {
                return false;
            };
            let Some(parent) = resolve_structural_path(types, root, prefix) else {
                return false;
            };
            let Some(declaration) = types.get(&parent) else {
                return false;
            };
            let terminal_psi::StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } = &declaration.shape
            else {
                return false;
            };
            let byte_type = semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .expect("u8 is valid"),
            );
            if !types.values().any(|candidate| {
                candidate.id == *element
                    && candidate.shape
                        == terminal_psi::StructuralTypeShape::PrimitiveScalar(byte_type)
            }) {
                return false;
            };
            let Some(layout) =
                terminal_semantics::byte_carrier_scalar_layout(types.values().copied(), expected)
            else {
                return false;
            };
            index
                .checked_add(layout.size)
                .is_some_and(|end| end <= *length)
        }

        impl StructuralTypes for BTreeMap<StructuralTypeId, &StructuralTypeDeclaration> {
            fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
                self.get(&id).copied()
            }

            fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone {
                self.values().copied()
            }
        }
    }
    mod provider_specialization {
        use crate::OptimizationUnitValidationError;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::{BoundaryMachineId, StructuralPlaceKind, StructuralTypeId};
        use std::collections::BTreeMap;

        pub(crate) fn validate_provider_attachment_specialization(
            function: &PsiOptimizationFunction,
            _boundary_machines: &BTreeMap<
                BoundaryMachineId,
                &terminal_psi::BoundaryMachineDeclaration,
            >,
            _types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        ) -> Result<(), OptimizationUnitValidationError> {
            if function
                .structural_places
                .iter()
                .any(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
            {
                return Err(
                    OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(
                        function.machine,
                    ),
                );
            }
            Ok(())
        }
    }
    mod storage_projection {
        use super::paths::structural_projection_type;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{
            ByteSequenceCarrier, ByteSequencePredicateSet, StructuralPathSegment,
            StructuralTypeDeclaration, StructuralTypeShape,
        };
        use terminal_semantics::borrowed_referent_matches;

        #[cfg(test)]
        mod tests {
            use super::{StructuralStorageProjection, structural_storage_projection};
            use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};
            use terminal_psi::{
                BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess,
                StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
                StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
            };

            const OWNER: ByteSequenceCarrier = ByteSequenceCarrier::BoundedOwned { capacity: 16 };
            const VIEW: ByteSequenceCarrier = ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            };

            fn type_id(raw: u64) -> StructuralTypeId {
                StructuralTypeId::new(raw).unwrap()
            }

            fn utf8() -> ByteSequencePredicateSet {
                ByteSequencePredicateSet::from_bits(1).unwrap()
            }

            fn field(
                raw: u64,
                identity: terminal_psi::TerminalIdentityIndex,
                relevance: BindingRelevance,
                field_type: StructuralFieldType,
            ) -> StructuralFieldDeclaration {
                StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: StructuralFieldId::new(raw).unwrap(),
                    identity,
                    relevance,
                    field_type,
                    storage: None,
                }
            }

            fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
                StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: type_id(raw),
                    identity: terminal_psi::TerminalIdentityIndex(10 + raw as u32),
                    shape,
                }
            }

            fn catalog() -> Vec<StructuralTypeDeclaration> {
                let qualified = |carrier| StructuralFieldType::PredicateByteSequence {
                    carrier,
                    predicates: utf8(),
                };
                vec![
                    declaration(
                        1,
                        StructuralTypeShape::Record {
                            fields: vec![
                                field(1, idx("text"), BindingRelevance::Relevant, qualified(OWNER)),
                                field(2, idx("name"), BindingRelevance::Relevant, qualified(VIEW)),
                                field(
                                    3,
                                    idx("proof"),
                                    BindingRelevance::Erased,
                                    StructuralFieldType::ByteSequence(OWNER),
                                ),
                            ],
                            storage: terminal_psi::StructuralRecordStorage::Packed,
                        },
                    ),
                    declaration(
                        2,
                        StructuralTypeShape::Sum {
                            cases: vec![
                                StructuralCaseDeclaration {
                                    id: StructuralCaseId::new(1).unwrap(),
                                    identity: idx("Empty"),
                                    fields: Vec::new(),
                                },
                                StructuralCaseDeclaration {
                                    id: StructuralCaseId::new(2).unwrap(),
                                    identity: idx("Full"),
                                    fields: vec![field(
                                        4,
                                        idx("bytes"),
                                        BindingRelevance::Relevant,
                                        StructuralFieldType::ByteSequence(OWNER),
                                    )],
                                },
                            ],
                        },
                    ),
                ]
            }

            fn idx(name: &str) -> terminal_psi::TerminalIdentityIndex {
                let index = match name {
                    "text" => 1,
                    "name" => 2,
                    "bytes" => 3,
                    "proof" => 4,
                    "Empty" => 5,
                    "Full" => 6,
                    _ => 0,
                };
                terminal_psi::TerminalIdentityIndex(index)
            }

            fn named(identity: terminal_psi::TerminalIdentityIndex) -> StructuralPathSegment {
                StructuralPathSegment::Field(identity)
            }

            #[test]
            fn byte_fields_without_a_declared_carrier_name_their_field_storage() {
                let types = catalog();
                let owner =
                    structural_storage_projection(types.iter(), type_id(1), &[named(idx("text"))]);
                let view =
                    structural_storage_projection(types.iter(), type_id(1), &[named(idx("name"))]);
                assert_eq!(
                    owner,
                    Some(StructuralStorageProjection::ByteField {
                        carrier: OWNER,
                        predicates: utf8(),
                    })
                );
                assert_eq!(
                    view,
                    Some(StructuralStorageProjection::ByteField {
                        carrier: VIEW,
                        predicates: utf8(),
                    })
                );
                let (owner, view) = (owner.unwrap(), view.unwrap());
                assert!(owner.matches(types.iter(), owner));
                assert!(!owner.matches(types.iter(), view));
                assert_eq!(
                    structural_storage_projection(types.iter(), type_id(1), &[named(idx("proof"))]),
                    None
                );
            }

            #[test]
            fn a_selected_payload_byte_field_names_its_case_storage() {
                let types = catalog();
                let full = StructuralPathSegment::Case(StructuralCaseId::new(2).unwrap());
                assert_eq!(
                    structural_storage_projection(
                        types.iter(),
                        type_id(2),
                        &[full, named(idx("bytes"))]
                    ),
                    Some(StructuralStorageProjection::ByteField {
                        carrier: OWNER,
                        predicates: ByteSequencePredicateSet::EMPTY,
                    })
                );
                let empty = StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap());
                assert_eq!(
                    structural_storage_projection(
                        types.iter(),
                        type_id(2),
                        &[empty, named(idx("bytes"))]
                    ),
                    None
                );
            }

            #[test]
            fn a_declared_owner_carrier_keeps_its_endpoint_spelling() {
                let mut types = catalog();
                types.push(declaration(3, StructuralTypeShape::ByteSequence(OWNER)));
                assert_eq!(
                    structural_storage_projection(types.iter(), type_id(1), &[named(idx("text"))]),
                    Some(StructuralStorageProjection::Endpoint(type_id(3)))
                );
                assert!(matches!(
                    structural_storage_projection(types.iter(), type_id(1), &[named(idx("name"))]),
                    Some(StructuralStorageProjection::ByteField { carrier: VIEW, .. })
                ));
            }
        }
        pub(crate) fn shared_view_declaration(declaration: &StructuralTypeDeclaration) -> bool {
            declaration.shape.is_borrowed_byte_view()
                || matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum StructuralStorageProjection {
            Endpoint(StructuralTypeId),
            CollectionWindow {
                backing: StructuralTypeId,
                offset: u64,
                length: u64,
            },
            ByteField {
                carrier: ByteSequenceCarrier,
                predicates: ByteSequencePredicateSet,
            },
        }

        impl StructuralStorageProjection {
            pub fn matches<'types>(
                self,
                declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
                expected: Self,
            ) -> bool {
                match (self, expected) {
                    (Self::Endpoint(actual), Self::Endpoint(expected)) => {
                        borrowed_referent_matches(declarations, actual, expected)
                    }
                    (
                        Self::CollectionWindow {
                            backing: actual,
                            offset,
                            length,
                        },
                        Self::CollectionWindow {
                            backing: expected,
                            offset: expected_offset,
                            length: expected_length,
                        },
                    ) => {
                        offset == expected_offset
                            && length == expected_length
                            && borrowed_referent_matches(declarations, actual, expected)
                    }
                    (actual @ Self::ByteField { .. }, expected @ Self::ByteField { .. }) => {
                        actual == expected
                    }
                    _ => false,
                }
            }
        }

        pub fn structural_storage_projection<'types>(
            declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralStorageProjection> {
            if matches!(path.last(), Some(StructuralPathSegment::FixedRange { .. })) {
                let window = terminal_semantics::fixed_collection_storage_window(
                    declarations.clone(),
                    root,
                    path,
                )?;
                Some(StructuralStorageProjection::CollectionWindow {
                    backing: structural_projection_type(declarations, root, window.backing_path)?,
                    offset: window.offset,
                    length: window.length,
                })
            } else {
                structural_projection_type(declarations.clone(), root, path)
                    .map(StructuralStorageProjection::Endpoint)
                    .or_else(|| byte_field(declarations, root, path))
            }
        }

        pub fn structural_borrowed_storage_projection<'types>(
            declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
            access: terminal_psi::StructuralAccess,
        ) -> Option<StructuralStorageProjection> {
            if access != terminal_psi::StructuralAccess::Owned
                && let Some(endpoint) =
                    terminal_semantics::borrowed_primitive_projection_declaration(
                        declarations.clone(),
                        root,
                        path,
                        None,
                    )
            {
                return Some(StructuralStorageProjection::Endpoint(endpoint));
            }
            structural_storage_projection(declarations, root, path)
        }

        fn byte_field<'types>(
            declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<StructuralStorageProjection> {
            let (StructuralPathSegment::Field(identity), container) = path.split_last()? else {
                return None;
            };
            let (container, case) = match container.split_last() {
                Some((StructuralPathSegment::Case(case), container)) => (container, Some(case)),
                _ => (container, None),
            };
            let owner = structural_projection_type(declarations.clone(), root, container)?;
            let shape = &declarations
                .into_iter()
                .find(|declaration| declaration.id == owner)?
                .shape;
            let fields = match case {
                Some(case) => shape
                    .sum_cases()?
                    .iter()
                    .find(|candidate| candidate.id == *case)?
                    .fields
                    .as_slice(),
                None => shape.direct_fields()?,
            };
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
            Some(StructuralStorageProjection::ByteField {
                carrier: field.field_type.byte_sequence_carrier()?,
                predicates: field.field_type.byte_predicates(),
            })
        }
    }
    mod type_declarations {
        use crate::OptimizationUnitValidationError;
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeSet;

        pub(crate) fn validate_structural_fields(
            structural_type: StructuralTypeId,
            case: Option<semantic_vocabulary::StructuralCaseId>,
            fields: &[terminal_psi::StructuralFieldDeclaration],
            has_provider_attachment: bool,
        ) -> Result<(), OptimizationUnitValidationError> {
            if fields.windows(2).any(|pair| pair[0].id >= pair[1].id) {
                return Err(
                    OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                        structural_type,
                        case,
                    },
                );
            }
            let mut identities = BTreeSet::new();
            for field in fields {
                if field.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || !identities.insert(field.identity)
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                            structural_type,
                            field: field.id,
                        },
                    );
                }
                let invalid_erased =
                    || OptimizationUnitValidationError::InvalidErasedStructuralField {
                        structural_type,
                        field: field.id,
                    };
                match (&field.field_type, field.relevance) {
                    (
                        terminal_psi::StructuralFieldType::Atomic(_)
                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_),
                        _,
                    ) if field.field_type.atomic_scalar_type().is_none() => {
                        return Err(
                            OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                                structural_type,
                                field: field.id,
                            },
                        );
                    }
                    (
                        terminal_psi::StructuralFieldType::PredicateByteSequence {
                            carrier,
                            predicates,
                        },
                        terminal_psi::BindingRelevance::Relevant,
                    ) if !predicates.is_empty() && carrier.supports_field_predicates() => {}
                    (terminal_psi::StructuralFieldType::PredicateByteSequence { .. }, _) => {
                        return Err(invalid_erased());
                    }
                    (terminal_psi::StructuralFieldType::Erased { type_identity }, _)
                        if *type_identity == terminal_psi::TerminalIdentityIndex::ABSENT =>
                    {
                        return Err(invalid_erased());
                    }
                    (
                        terminal_psi::StructuralFieldType::Erased { .. },
                        terminal_psi::BindingRelevance::Erased,
                    ) => {}
                    (
                        terminal_psi::StructuralFieldType::Erased { .. },
                        terminal_psi::BindingRelevance::Relevant,
                    ) if has_provider_attachment => {}
                    (
                        terminal_psi::StructuralFieldType::Erased { .. },
                        terminal_psi::BindingRelevance::Relevant,
                    ) => return Err(invalid_erased()),
                    (
                        terminal_psi::StructuralFieldType::Scalar(_)
                        | terminal_psi::StructuralFieldType::Atomic(_)
                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                        | terminal_psi::StructuralFieldType::RefinedScalar(_)
                        | terminal_psi::StructuralFieldType::BoundedInteger(_)
                        | terminal_psi::StructuralFieldType::IeeeFloat(_)
                        | terminal_psi::StructuralFieldType::Structural(_),
                        terminal_psi::BindingRelevance::Erased,
                    ) => return Err(invalid_erased()),
                    (
                        terminal_psi::StructuralFieldType::Scalar(_)
                        | terminal_psi::StructuralFieldType::Atomic(_)
                        | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
                        | terminal_psi::StructuralFieldType::RefinedScalar(_)
                        | terminal_psi::StructuralFieldType::BoundedInteger(_)
                        | terminal_psi::StructuralFieldType::IeeeFloat(_)
                        | terminal_psi::StructuralFieldType::ByteSequence(_)
                        | terminal_psi::StructuralFieldType::Structural(_),
                        terminal_psi::BindingRelevance::Relevant,
                    )
                    | (
                        terminal_psi::StructuralFieldType::ByteSequence(_),
                        terminal_psi::BindingRelevance::Erased,
                    ) => {}
                }
            }
            Ok(())
        }

        pub(crate) fn validate_structural_cases(
            structural_type: StructuralTypeId,
            cases: &[terminal_psi::StructuralCaseDeclaration],
        ) -> Result<(), OptimizationUnitValidationError> {
            if cases.is_empty() {
                return Err(OptimizationUnitValidationError::EmptyStructuralSum(
                    structural_type,
                ));
            }
            if cases.windows(2).any(|pair| pair[0].id >= pair[1].id) {
                return Err(
                    OptimizationUnitValidationError::NonCanonicalStructuralCaseOrder(
                        structural_type,
                    ),
                );
            }
            let mut identities = BTreeSet::new();
            for case in cases {
                if case.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                    || !identities.insert(case.identity)
                {
                    return Err(
                        OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                            structural_type,
                            case: case.id,
                        },
                    );
                }
            }
            for case in cases {
                validate_structural_fields(structural_type, Some(case.id), &case.fields, false)?;
            }
            Ok(())
        }

        pub(super) fn storage_graph_error(
            error: terminal_semantics::storage::type_graph::StorageGraphError,
        ) -> OptimizationUnitValidationError {
            use terminal_semantics::storage::type_graph::StorageGraphError;
            match error {
                StorageGraphError::MissingType(id) => {
                    OptimizationUnitValidationError::UnknownStructuralType(id)
                }
                StorageGraphError::DuplicateType(id) => {
                    OptimizationUnitValidationError::DuplicateStructuralType(id)
                }
                StorageGraphError::Cycle(id) => {
                    OptimizationUnitValidationError::RecursiveStructuralType(id)
                }
            }
        }

        #[test]
        fn shared_storage_graph_preserves_native_error_identities() {
            use terminal_semantics::storage::type_graph::StorageGraphError;
            let id = StructuralTypeId::new(29).unwrap();
            for (source, expected) in [
                (
                    StorageGraphError::MissingType(id),
                    OptimizationUnitValidationError::UnknownStructuralType(id),
                ),
                (
                    StorageGraphError::DuplicateType(id),
                    OptimizationUnitValidationError::DuplicateStructuralType(id),
                ),
                (
                    StorageGraphError::Cycle(id),
                    OptimizationUnitValidationError::RecursiveStructuralType(id),
                ),
            ] {
                assert_eq!(storage_graph_error(source), expected);
            }
        }
    }
    mod witnesses {
        use crate::unit_validation::derived_metadata::expected_provenance;
        use crate::{OperationSite, OptimizationUnitValidationError};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{PsiOptimizationFunction, PsiProvenance};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::StructuralPlaceDeclaration;
        use terminal_psi::StructuralTypeDeclaration;

        pub(crate) fn validate_byte_sequence_literal_witnesses(
            function: &PsiOptimizationFunction,
            expected_literals: &[(StructuralPlaceDeclaration, StructuralTypeDeclaration)],
        ) -> Result<(), OptimizationUnitValidationError> {
            let mut expected = expected_literals
                .iter()
                .map(|(place, structural_type)| (place.id, (*place, structural_type)))
                .collect::<BTreeMap<_, _>>();
            let mut actual = 0_usize;
            for node in function.block_nodes() {
                let O::EstablishByteSequenceLiteral {
                    place,
                    structural_type,
                    ..
                } = &node.operation
                else {
                    continue;
                };
                actual += 1;
                if expected
                    .remove(&place.id)
                    .is_none_or(|(expected_place, expected_type)| {
                        *place != expected_place || structural_type != expected_type
                    })
                {
                    return Err(
                        OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                            function.machine,
                        ),
                    );
                }
            }
            if actual != expected_literals.len() || !expected.is_empty() {
                return Err(
                    OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                        function.machine,
                    ),
                );
            }
            Ok(())
        }

        pub(crate) fn validate_trivial_affine_local_witnesses(
            function: &PsiOptimizationFunction,
            expected_locals: &[(StructuralPlaceDeclaration, StructuralTypeDeclaration)],
        ) -> Result<(), OptimizationUnitValidationError> {
            let explicit = function
                .block_nodes()
                .filter_map(|node| match &node.operation {
                    O::EstablishTrivialAffineLocal {
                        psi_operation,
                        place,
                        structural_type,
                    } => Some((*psi_operation, *place, structural_type)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let structural_returns = function
                .block_nodes()
                .filter_map(|node| match &node.operation {
                    O::ReturnStructural {
                        trivial_affine_locals,
                        cleanup_actions,
                        ..
                    } => Some((trivial_affine_locals, cleanup_actions)),
                    _ => None,
                })
                .collect::<Vec<_>>();

            if !explicit.is_empty() {
                let exact = structural_returns
                    .iter()
                    .all(|(compressed, _)| compressed.is_empty())
                    && explicit.len() == expected_locals.len()
                    && explicit.iter().zip(expected_locals).all(
                        |((_, actual_place, actual_type), (expected_place, expected_type))| {
                            actual_place == expected_place && *actual_type == expected_type
                        },
                    );
                if !exact {
                    return Err(
                        OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                            function.machine,
                        ),
                    );
                }
                return Ok(());
            }

            if !expected_locals.is_empty() && structural_returns.len() != 1 {
                return Err(
                    OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                        function.machine,
                    ),
                );
            }

            let executable_operations = function
                .block_nodes()
                .filter(|node| !matches!(node.operation, O::ReturnStructural { .. }))
                .flat_map(|node| expected_provenance(&node.operation))
                .filter_map(|site| match site {
                    PsiProvenance::Operation(operation) => Some(operation),
                    PsiProvenance::Edge(_) => None,
                })
                .collect::<BTreeSet<_>>();

            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let O::ReturnStructural {
                        source,
                        trivial_affine_locals,
                        cleanup_actions,
                        ..
                    } = &node.operation
                    else {
                        continue;
                    };
                    if trivial_affine_locals.is_empty() && expected_locals.is_empty() {
                        continue;
                    }
                    let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                    let mut hidden_operations = BTreeSet::new();
                    if trivial_affine_locals.len() != expected_locals.len()
                        || trivial_affine_locals.iter().zip(expected_locals).any(
                            |(
                                (operation, actual_place, actual_type),
                                (expected_place, expected_type),
                            )| {
                                actual_place != expected_place
                                    || actual_type != expected_type
                                    || !hidden_operations.insert(*operation)
                                    || executable_operations.contains(operation)
                            },
                        )
                    {
                        return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch(
                        OperationSite::new(function.machine, block.id, node_index),
                    ),
                );
                    }

                    let Some(returned_parameter) = function.structural_parameters.first() else {
                        return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch(
                        OperationSite::new(function.machine, block.id, node_index),
                    ),
                );
                    };
                    let Some(result) = function.result.structural() else {
                        return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch(
                        OperationSite::new(function.machine, block.id, node_index),
                    ),
                );
                    };
                    if !function.parameters.is_empty()
                        || returned_parameter.place != *source
                        || returned_parameter.multiplicity
                            != terminal_psi::StructuralMultiplicity::Linear
                        || result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                        || returned_parameter.structural_type != result.structural_type
                        || returned_parameter.qualifications != result.qualifications
                        || returned_parameter.projected_qualifications
                            != result.projected_qualifications
                        || returned_parameter.place == result.place
                        || function
                            .structural_parameters
                            .iter()
                            .skip(1)
                            .any(|parameter| {
                                parameter.multiplicity
                                    != terminal_psi::StructuralMultiplicity::Affine
                                    || !parameter.qualifications.is_empty()
                                    || !parameter.projected_qualifications.is_empty()
                            })
                    {
                        return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch(
                        OperationSite::new(function.machine, block.id, node_index),
                    ),
                );
                    }
                    let expected_discards = trivial_affine_locals
                        .iter()
                        .rev()
                        .map(|(_, local, _)| local.id)
                        .chain(
                            function
                                .structural_parameters
                                .iter()
                                .skip(1)
                                .rev()
                                .map(|parameter| parameter.place),
                        )
                        .collect::<Vec<_>>();
                    if *cleanup_actions
                        != expected_discards
                            .into_iter()
                            .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                            .collect::<Vec<_>>()
                    {
                        return Err(
                            OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch(
                                OperationSite::new(function.machine, block.id, node_index),
                            ),
                        );
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) use paths::structural_projection_type;
    pub(crate) use storage_projection::shared_view_declaration;
    pub use storage_projection::{
        StructuralStorageProjection, structural_borrowed_storage_projection,
        structural_storage_projection,
    };

    pub(crate) use function_catalog::{structural_root_key, validate_function_structural_catalog};
    pub(crate) use paths::{
        byte_carrier_leaf_window, resolve_leaf_copy_path, resolve_runtime_projection,
        resolve_structural_path, structural_projected_qualifications_match,
        structural_qualifications_match,
    };
    pub(crate) use provider_specialization::validate_provider_attachment_specialization;
    pub(crate) use witnesses::{
        validate_byte_sequence_literal_witnesses, validate_trivial_affine_local_witnesses,
    };

    pub(crate) fn index_structural_catalogs(
        unit: &PsiOptimizationUnit,
    ) -> Result<
        (
            BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
            BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
        ),
        OptimizationUnitValidationError,
    > {
        let types = catalog::index_structural_types(unit)?;
        let domains = catalog::index_structural_domains(unit, &types)?;
        Ok((types, domains))
    }
}
pub(crate) fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_control_cycles(
        unit,
        &function_structure::ControlCyclePolicy::default(),
    )
}

pub(crate) fn revalidate_rewrite_output(
    output: &mut PsiOptimizationUnit,
    machine: MachineId,
) -> Result<&PsiOptimizationFunction, OptimizationUnitValidationError> {
    let output_function = output
        .function_mut(machine)
        .expect("output function exists");
    output_function.facts = function_structure::reconstruct_fact_index(output_function);
    output_function.declared_places =
        derived_metadata::reconstruct_declared_places(output_function)?;
    services::refresh_root_service_reach(output)?;
    output.identity = recompute_psi_optimization_unit_identity(output);
    validate_psi_optimization_unit(output)?;
    Ok(output.function(machine).expect("output function exists"))
}

pub fn validate_psi_optimization_unit_with_admitted_cycle_machines(
    unit: &PsiOptimizationUnit,
    admitted_machines: &[MachineId],
) -> Result<(), OptimizationUnitValidationError> {
    let mut policy = function_structure::ControlCyclePolicy::default();
    for &machine in admitted_machines {
        policy.admit(machine);
    }
    validate_psi_optimization_unit_with_control_cycles(unit, &policy)
}

pub fn validate_psi_structural_catalog(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    structural_catalog::index_structural_catalogs(unit).map(|_| ())
}

pub fn reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(
    unit: &PsiOptimizationUnit,
    admitted_machines: &[MachineId],
) -> Result<Vec<optimization_unit::OwnershipFrontierFact>, OptimizationUnitValidationError> {
    identity_indexes::validate_identity_and_fact_indexes(unit)?;
    let mut policy = function_structure::ControlCyclePolicy::default();
    for &machine in admitted_machines {
        policy.admit(machine);
    }
    let mut facts = catalogs::index_and_validate_unit_catalogs(unit, &policy)?.ownership_frontiers;
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Ok(facts)
}

pub(crate) fn validate_psi_optimization_unit_with_control_cycles(
    unit: &PsiOptimizationUnit,
    cycle_policy: &function_structure::ControlCyclePolicy,
) -> Result<(), OptimizationUnitValidationError> {
    identity_indexes::validate_identity_and_fact_indexes(unit)?;
    let indexes = catalogs::index_and_validate_unit_catalogs(unit, cycle_policy)?;
    no_normal_calls::validate(unit, &indexes.machines)?;
    runtime_evidence::validate(unit)?;
    affine_authority::validate_retained_ownership_authority(unit)?;
    catalogs::validate_final_authorities(unit, &indexes)
}
