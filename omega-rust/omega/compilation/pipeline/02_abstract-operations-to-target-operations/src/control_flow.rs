/*
  Common target control-flow graph construction (`lower_control_flow`,
  `LiveDefinitions`, `ReferenceEdgeStorage`), block dominance (`dominance`),
  terminators (`terminator`), parallel edge transfers (`transfers`), structural
  case dispatch (`structural_case`), and interrupt/image lowering.

  Every function lowers through a single block-structured control graph where
  scalar and structural definitions belong to dominating blocks and reference
  descriptor storage converges to a fixed point across post-transfer,
  post-cleanup edges.
*/
use super::function_signature::integer_parameters;
use super::scalar::byte_views::{is_immutable_byte_carrier_parameter, is_owned_byte_parameter};
use super::scalar::element_views::is_element_view_parameter;
use super::unit::known_integers::KnownUnitInteger;
use crate::LoweringError;
use crate::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
};
use aggregate_results::block_home;
use references::{merge_arrivals, merge_borrowed_arrivals};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    BoundarySettlementBinding, ScalarFunctionAbi, TargetFunction, TargetUnitScalarHomeRequirement,
    TerminalPsiProvenance,
};
use terminal_psi::{StructuralAccess, StructuralPathSegment, TerminalAffineCleanupAction};
#[path = "control_flow_operations.rs"]
mod cf_operations;
pub(crate) use cf_operations::{operations, scalar_sources};
#[path = "control_flow_calls.rs"]
mod cf_calls;
pub(crate) use cf_calls::{
    borrowed_calls, call_signature, calls, owned_arguments, selected_boundary_calls,
};
#[path = "control_flow_storage.rs"]
mod cf_storage;
pub(crate) use cf_storage::{
    access_lifecycles, aggregate_borrows, aggregate_results, atomic_access, borrowed_windows,
    byte_concatenation, byte_write, element_view_write, leaf_copy, observations, primitive_storage,
    records,
};
#[path = "references.rs"]
pub(crate) mod references;
mod dominance {
    use std::collections::{BTreeSet, VecDeque};

    pub(crate) fn schedule(
        incoming: &[Vec<usize>],
        outgoing: &[Vec<usize>],
        entry: usize,
    ) -> Option<Vec<(usize, Option<usize>)>> {
        let solver_order = reverse_postorder(outgoing, entry)?;
        if solver_order.len() != incoming.len() {
            return None;
        }
        let dominators = solve_sets(incoming, outgoing, entry, &solver_order)?;
        let mut order = Vec::with_capacity(incoming.len());
        let mut visited = BTreeSet::new();
        let mut pending = VecDeque::from([entry]);
        while let Some(position) = pending.pop_front() {
            if visited.insert(position) {
                order.push(position);
                pending.extend(outgoing[position].iter().copied());
            }
        }
        let depths: Vec<usize> = dominators
            .iter()
            .map(|words| words.iter().map(|word| word.count_ones() as usize).sum())
            .collect();
        order
            .into_iter()
            .map(|position| {
                if position == entry {
                    Some((position, None))
                } else {
                    dominator_positions(&dominators[position])
                        .filter(|dominator| *dominator != position)
                        .max_by_key(|dominator| depths[*dominator])
                        .map(|dominator| (position, Some(dominator)))
                }
            })
            .collect()
    }

    // Block numbers can run opposite to control flow. Use graph order for
    // convergence while keeping the returned lowering schedule breadth-first.
    fn reverse_postorder(outgoing: &[Vec<usize>], entry: usize) -> Option<Vec<usize>> {
        let mut reached = BTreeSet::new();
        let mut pending = vec![(entry, false)];
        let mut postorder = Vec::new();
        while let Some((position, finished)) = pending.pop() {
            if finished {
                postorder.push(position);
            } else if reached.insert(position) {
                let successors = outgoing.get(position)?;
                pending.push((position, true));
                pending.extend(successors.iter().rev().map(|target| (*target, false)));
            }
        }
        postorder.reverse();
        Some(postorder)
    }

    fn solve_sets(
        incoming: &[Vec<usize>],
        outgoing: &[Vec<usize>],
        entry: usize,
        order: &[usize],
    ) -> Option<Vec<Vec<u64>>> {
        let mut complete = vec![0_u64; incoming.len().max(outgoing.len()).div_ceil(64)];
        for position in order {
            complete[*position / 64] |= 1_u64 << (*position % 64);
        }
        let mut dominators = vec![complete; incoming.len()];
        dominators.get_mut(entry)?.fill(0);
        dominators[entry][entry / 64] |= 1_u64 << (entry % 64);
        let mut dirty = vec![true; incoming.len()];
        dirty[entry] = false;
        let mut pending_count = incoming.len() - 1;
        while pending_count != 0 {
            for position in order.iter().copied() {
                // Clear before evaluation so a changed self-loop stays pending.
                if !std::mem::replace(dirty.get_mut(position)?, false) {
                    continue;
                }
                pending_count -= 1;
                let (first, others) = incoming.get(position)?.split_first()?;
                let mut current = dominators.get(*first)?.clone();
                for predecessor in others {
                    for (word, predecessor_word) in
                        current.iter_mut().zip(dominators.get(*predecessor)?)
                    {
                        *word &= *predecessor_word;
                    }
                }
                current[position / 64] |= 1_u64 << (position % 64);
                if current != dominators[position] {
                    dominators[position] = current;
                    for successor in outgoing.get(position)? {
                        if *successor != entry
                            && !std::mem::replace(dirty.get_mut(*successor)?, true)
                        {
                            pending_count += 1;
                        }
                    }
                }
            }
        }
        Some(dominators)
    }

    fn dominator_positions(words: &[u64]) -> impl Iterator<Item = usize> + '_ {
        words.iter().enumerate().flat_map(|(word_index, &word)| {
            let mut remaining = word;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let bit = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                Some(word_index * 64 + bit)
            })
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{dominator_positions, schedule};
        use std::collections::{BTreeSet, VecDeque};

        fn original_schedule(
            incoming: &[Vec<usize>],
            outgoing: &[Vec<usize>],
            entry: usize,
        ) -> Option<Vec<(usize, Option<usize>)>> {
            let mut reachable = BTreeSet::new();
            let mut pending = vec![entry];
            while let Some(position) = pending.pop() {
                if reachable.insert(position) {
                    pending.extend(outgoing.get(position)?.iter().copied());
                }
            }
            if reachable.len() != incoming.len() {
                return None;
            }
            let mut dominators = vec![reachable; incoming.len()];
            dominators[entry] = BTreeSet::from([entry]);
            loop {
                let mut changed = false;
                for (position, predecessors) in incoming.iter().enumerate() {
                    if position == entry {
                        continue;
                    }
                    let (first, others) = predecessors.split_first()?;
                    let mut current = dominators[*first].clone();
                    for predecessor in others {
                        current.retain(|candidate| dominators[*predecessor].contains(candidate));
                    }
                    current.insert(position);
                    if current != dominators[position] {
                        dominators[position] = current;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut order = Vec::with_capacity(incoming.len());
            let mut visited = BTreeSet::new();
            let mut pending = VecDeque::from([entry]);
            while let Some(position) = pending.pop_front() {
                if visited.insert(position) {
                    order.push(position);
                    pending.extend(outgoing[position].iter().copied());
                }
            }
            order
                .into_iter()
                .map(|position| {
                    if position == entry {
                        Some((position, None))
                    } else {
                        dominators[position]
                            .iter()
                            .copied()
                            .filter(|dominator| *dominator != position)
                            .max_by_key(|dominator| dominators[*dominator].len())
                            .map(|dominator| (position, Some(dominator)))
                    }
                })
                .collect()
        }

        fn compare(outgoing: &[Vec<usize>], entry: usize) {
            let mut incoming = vec![Vec::new(); outgoing.len()];
            for (from, targets) in outgoing.iter().enumerate() {
                for &to in targets {
                    incoming[to].push(from);
                }
            }
            assert_eq!(
                schedule(&incoming, outgoing, entry),
                original_schedule(&incoming, outgoing, entry),
                "entry={entry}, outgoing={outgoing:?}",
            );
        }

        #[test]
        fn reverse_numbered_chain_keeps_each_immediate_parent_and_breadth_first_order() {
            let count = 8192;
            let mut outgoing = vec![Vec::new(); count];
            let mut incoming = vec![Vec::new(); count];
            for position in 1..count {
                outgoing[position].push(position - 1);
                incoming[position - 1].push(position);
            }
            let schedule = schedule(&incoming, &outgoing, count - 1).unwrap();
            assert_eq!(schedule.len(), count);
            assert!(schedule.into_iter().eq(
                (0..count).rev().map(|position| {
                    (position, (position != count - 1).then_some(position + 1))
                })
            ));
        }

        #[test]
        fn scheduled_dominators_match_original_sets_for_all_small_graphs_and_boundaries() {
            for count in 1..=4 {
                for mask in 0_u32..(1_u32 << (count * count)) {
                    let mut outgoing = vec![Vec::new(); count];
                    for (from, targets) in outgoing.iter_mut().enumerate() {
                        for to in 0..count {
                            if mask & (1 << (from * count + to)) != 0 {
                                targets.push(to);
                            }
                        }
                    }
                    for entry in 0..count {
                        compare(&outgoing, entry);
                    }
                }
            }
            compare(&[], 0);
            compare(&[vec![1, 2], vec![3], vec![3], vec![1, 2, 4], vec![]], 0);
            compare(&[vec![1, 1, 2], vec![3, 3], vec![3], vec![]], 0);
            for count in [63, 64, 65, 127, 128, 130] {
                let mut outgoing = vec![Vec::new(); count];
                for (from, targets) in outgoing.iter_mut().enumerate() {
                    targets.push((from + 1) % count);
                    if from % 3 == 0 {
                        targets.push((from + 2) % count);
                        targets.push((from + 2) % count);
                    }
                }
                compare(&outgoing, count / 2);
                outgoing[0].clear();
                compare(&outgoing, count / 2);
            }
            assert_eq!(schedule(&[vec![], vec![]], &[vec![1], vec![]], 0), None);
            assert_eq!(schedule(&[vec![]], &[vec![1]], 0), None);
            assert_eq!(
                schedule(&[vec![]], &[vec![1]], 0),
                original_schedule(&[vec![]], &[vec![1]], 0)
            );
            for (incoming, outgoing, entry) in [
                (vec![vec![], vec![0]], vec![vec![1], vec![], vec![]], 0),
                (vec![vec![]], vec![vec![], vec![]], 0),
            ] {
                assert_eq!(
                    schedule(&incoming, &outgoing, entry),
                    original_schedule(&incoming, &outgoing, entry),
                );
            }
            assert_eq!(
                dominator_positions(&[1_u64 << 63, 3, 2]).collect::<Vec<_>>(),
                vec![63, 64, 65, 129]
            );
        }
    }
}

mod interrupt_installation {
    use super::RowSite;
    use crate::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use provider_admission::AdmittedInstalledInterruptCohort;
    use semantic_vocabulary::{OperationId, ScalarType};
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
        BoundarySettlementRealization, CompilerBuiltinExecution, TargetInstallationCall,
        TargetUnitOperation,
    };

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        scope: &provider_admission::AdmittedInstalledInterruptScope,
        calls: [TargetInstallationCall; 2],
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        settlement: &BoundarySettlementBinding,
    ) -> Result<(), LoweringError> {
        let machine = site.function.machine;
        let invalid = |reason| LoweringError::InvalidInstalledInterrupt {
            machine,
            operation: scope.operation(),
            reason,
        };
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            arguments,
            structural_arguments,
            result: AbstractBoundaryResult::Scalar(result),
            ..
        } = operation
        else {
            return Err(invalid(
                "installation must produce one Boolean result from structural arguments",
            ));
        };
        let kind = language_core::static_interrupts::StaticInterruptOperation::Install;
        if declaration.id != *boundary
            || !arguments.is_empty()
            || result.scalar_type != ScalarType::Boolean
            || declaration
                .declaration
                .as_ref()
                .and_then(|row| row.execution.static_interrupt_operation())
                != Some(kind)
            || settlement.execution
                != BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::StaticInterrupt {
                        boundary: *boundary,
                        operation: kind,
                    },
                )
            || settlement.realization
                != BoundarySettlementRealization::Builtin(BoundaryRealization::StaticInterrupt {
                    boundary: *boundary,
                    operation: kind,
                })
        {
            return Err(invalid(
                "installation declaration or settled execution differs from its admitted operation",
            ));
        }
        if scope.boundary() != *boundary || scope.arguments().as_slice() != structural_arguments {
            return Err(invalid(
                "installation substituted its admitted boundary or actual references",
            ));
        }
        let result_home =
            super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
        site.operations
            .push(TargetUnitOperation::InstallStaticInterrupt {
                psi_operation: *psi_operation,
                boundary: *boundary,
                arguments: scope.arguments().clone(),
                calls,
                result_home,
            });
        site.provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn scope(
        operation: OperationId,
        cohort: Option<&AdmittedInstalledInterruptCohort>,
    ) -> Result<&provider_admission::AdmittedInstalledInterruptScope, &'static str> {
        let mut scopes = cohort
            .ok_or("installation has no admitted scope cohort")?
            .scopes()
            .iter()
            .filter(|scope| scope.operation() == operation);
        match (scopes.next(), scopes.next()) {
            (Some(scope), None) => Ok(scope),
            _ => Err("installation has no unique admitted scope"),
        }
    }
}

mod program_image {
    use super::RowSite;
    use crate::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use semantic_vocabulary::{BoundaryMachineId, IntegerType, ScalarType};
    use std::collections::BTreeMap;
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
        BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
        TargetUnitScalarHomeRequirement,
    };

    pub(crate) fn lower(
        operation: &AbstractOperation,
        target: target::NativeTarget,
        declarations: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (_, _, live, operations, provenance) = site.parts();
        let AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } = operation
        else {
            return Err(LoweringError::ProviderInstallationIdentityMismatch);
        };
        let mismatch = LoweringError::BoundaryRealizationMismatch(*boundary);
        let binding = settlements
            .get(boundary)
            .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
        let declaration = declarations
            .get(boundary)
            .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
        let execution = BoundaryExecutionBinding::CompilerBuiltin(
            CompilerBuiltinExecution::ProgramImageRetainedSuffixStart,
        );
        let scalar = ScalarType::Integer(IntegerType::address(64).map_err(|_| mismatch.clone())?);
        let AbstractBoundaryResult::Scalar(result) = result else {
            return Err(mismatch);
        };
        if target != target::NativeTarget::uefi_x64()
            || binding.execution != execution
            || binding.realization
                != BoundarySettlementRealization::Builtin(
                    BoundaryRealization::ProgramImageRetainedSuffixStart,
                )
            || !declaration.scalar_parameters.is_empty()
            || !declaration.structural_parameters.is_empty()
            || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
            || result.scalar_type != scalar
            || !arguments.is_empty()
            || !structural_arguments.is_empty()
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
        {
            return Err(mismatch);
        }
        let shape = crate::scalar_abi::fixed_native_scalar_shape(scalar).ok_or(mismatch)?;
        let result_home = TargetUnitScalarHomeRequirement {
            defining_operation: *psi_operation,
            source_value: result.value,
            scalar_type: scalar,
            shape,
        };
        crate::unit::known_integers::insert_known_unit_integer(
            &mut live.integers,
            result.value,
            crate::unit::known_integers::KnownUnitInteger::Home(result_home),
        )?;
        provenance.operations.push(*psi_operation);
        operations.push(TargetUnitOperation::ProgramImageAddress {
            psi_operation: *psi_operation,
            boundary: *boundary,
            execution,
            result_home,
        });
        Ok(())
    }
}

mod row {
    use super::LiveDefinitions;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractFunction;
    use target_operations::{TargetUnitOperation, TerminalPsiProvenance};

    pub(crate) type RowParts<'s, 'a> = (
        &'a AbstractFunction,
        &'a StructuralTypeLookup<'a>,
        &'s mut LiveDefinitions,
        &'s mut Vec<TargetUnitOperation>,
        &'s mut TerminalPsiProvenance,
    );

    pub(crate) type RowView<'s, 'a> = (
        &'a AbstractFunction,
        &'a StructuralTypeLookup<'a>,
        &'s LiveDefinitions,
        &'s mut Vec<TargetUnitOperation>,
        &'s mut TerminalPsiProvenance,
    );

    pub(crate) struct RowSite<'a> {
        pub(crate) function: &'a AbstractFunction,
        pub(crate) types: &'a StructuralTypeLookup<'a>,
        pub(crate) live: &'a mut LiveDefinitions,
        pub(crate) operations: &'a mut Vec<TargetUnitOperation>,
        pub(crate) provenance: &'a mut TerminalPsiProvenance,
    }

    impl<'a> RowSite<'a> {
        pub(crate) fn new(
            function: &'a AbstractFunction,
            types: &'a StructuralTypeLookup<'a>,
            live: &'a mut LiveDefinitions,
            operations: &'a mut Vec<TargetUnitOperation>,
            provenance: &'a mut TerminalPsiProvenance,
        ) -> Self {
            Self {
                function,
                types,
                live,
                operations,
                provenance,
            }
        }

        pub(crate) fn parts(&mut self) -> RowParts<'_, 'a> {
            (
                self.function,
                self.types,
                self.live,
                self.operations,
                self.provenance,
            )
        }

        pub(crate) fn view(&mut self) -> RowView<'_, 'a> {
            (
                self.function,
                self.types,
                self.live,
                self.operations,
                self.provenance,
            )
        }

        pub(crate) fn establish_home(
            &mut self,
            psi_operation: semantic_vocabulary::OperationId,
            result: &terminal_psi::StructuralOperationResult,
        ) -> Result<target_operations::TargetStructuralHomeRequirement, crate::LoweringError>
        {
            super::aggregate_results::established_home(
                self.live,
                psi_operation,
                result,
                self.types,
                self.function.machine,
            )
        }
    }
}

mod run_custody {
    use super::RowSite;
    use crate::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use target_operations::{
        BoundaryExecutionBinding, BoundarySettlementBinding, CompilerBuiltinExecution,
    };
    use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment};

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        declaration: &BoundaryMachineDeclaration,
        settlement: &BoundarySettlementBinding,
    ) -> Result<(), LoweringError> {
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            structural_arguments,
            arguments,
            result: AbstractBoundaryResult::Structural(result),
            ..
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(
                site.function.machine,
            ));
        };
        let invalid = |reason| LoweringError::InvalidAccessLifecycle {
            machine: site.function.machine,
            operation: *psi_operation,
            boundary: *boundary,
            reason,
        };
        let contract = terminal_semantics::empty_run_release_contract(
            site.types.catalog().as_slice(),
            declaration,
        )
        .map_err(invalid)?;
        let [source] = structural_arguments.as_slice() else {
            return Err(invalid("empty-run release requires one owned source"));
        };
        if !arguments.is_empty()
            || source.access != StructuralAccess::Owned
            || settlement.execution
                != BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::EmptyRunRelease,
                )
            || result.structural_type != contract.result.structural_type
            || result.multiplicity != contract.result.multiplicity
        {
            return Err(invalid("empty-run release changed its admitted invocation"));
        }
        let (source_type, _) =
            super::aggregate_results::value_input(site.function, site.types, site.live, source)?;
        if source_type != declaration.structural_parameters[0].structural_type {
            return Err(invalid("empty-run release changed its owned carrier"));
        }
        let root_type = site
            .live
            .structural_observation_type(site.function, source.place)
            .ok_or_else(|| invalid("empty-run release lost its source root"))?;
        let mut path = source.path.clone();
        path.push(StructuralPathSegment::Field(contract.storage));
        super::leaf_copy::copy_extent(
            *psi_operation,
            result,
            source.place,
            &path.into(),
            root_type,
            site,
        )
    }
}

mod static_interrupts {
    use super::RowSite;
    use crate::LoweringError;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use language_core::static_interrupts::StaticInterruptOperation;
    use target_operations::{
        BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
        BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
    };
    use terminal_psi::{StructuralAccess, StructuralTypeShape};

    pub(crate) fn lower(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        settlement: &BoundarySettlementBinding,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let invalid = || LoweringError::unsupported_control_flow(function.machine);
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            structural_arguments,
            arguments,
            result,
            ..
        } = operation
        else {
            return Err(invalid());
        };
        if declaration.id != *boundary || settlement.boundary != *boundary {
            return Err(invalid());
        }
        let operation = declaration
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.static_interrupt_operation())
            .filter(|operation| *operation != StaticInterruptOperation::Install)
            .ok_or_else(invalid)?;
        if settlement.execution
            != BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::StaticInterrupt {
                    boundary: *boundary,
                    operation,
                },
            )
            || settlement.realization
                != BoundarySettlementRealization::Builtin(BoundaryRealization::StaticInterrupt {
                    boundary: *boundary,
                    operation,
                })
        {
            return Err(invalid());
        }
        let [token] = structural_arguments.as_slice() else {
            return Err(invalid());
        };
        let [parameter] = declaration.structural_parameters.as_slice() else {
            return Err(invalid());
        };
        if !arguments.is_empty() {
            return Err(invalid());
        }
        let expected_access = if operation != StaticInterruptOperation::FinishCompletion {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::Owned
        };
        if token.access != expected_access || parameter.access != expected_access {
            return Err(invalid());
        }
        let token_type = if token.access == StructuralAccess::Owned {
            super::aggregate_results::value_input(function, types, live, token)?.0
        } else {
            super::references::reference_source_type(function, live, types, token)
                .ok_or_else(invalid)?
        };
        if token_type != parameter.structural_type {
            return Err(invalid());
        }
        let result_home = match (operation, result) {
            (StaticInterruptOperation::FinishCompletion, AbstractBoundaryResult::Unit) => None,
            (
                StaticInterruptOperation::Context | StaticInterruptOperation::BeginCompletion,
                AbstractBoundaryResult::Structural(result),
            ) => {
                if operation == StaticInterruptOperation::Context {
                    if !matches!(
                        types.get(result.structural_type).map(|row| &row.shape),
                        Some(StructuralTypeShape::Reference {
                            access: StructuralAccess::SharedBorrow,
                            ..
                        })
                    ) {
                        return Err(invalid());
                    }
                    super::references::boundary_result_references(function, types, result, live)?;
                }
                Some(super::aggregate_results::established_home(
                    live,
                    *psi_operation,
                    result,
                    types,
                    function.machine,
                )?)
            }
            _ => return Err(invalid()),
        };
        operations.push(TargetUnitOperation::StaticInterrupt {
            psi_operation: *psi_operation,
            boundary: *boundary,
            operation,
            token: token.clone(),
            result_home,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}

mod structural_case {
    use super::aggregate_results::value_home_layout;
    use super::terminator::plain_home_cleanup;
    use super::{LiveDefinitions, RowSite};
    use crate::LoweringError;
    use crate::structural_layout::reference_projection_regions;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
    use target_operations::{
        TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlTerminator,
        TargetScalarBlockValue, TargetStructuralCaseSource, TargetStructuralParameter,
    };
    use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
    use terminal_psi::StructuralAccess;
    use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

    pub(crate) fn observe(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let AbstractOperation::StructuralCaseMembership {
            psi_operation,
            result,
            source,
            path,
            case,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let identity = live
            .structural_observation_type(function, *source)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let (tag_byte_offset, case_tag, runs) = case_projection(identity, path, *case, types)?;
        if result.scalar_type != ScalarType::Boolean {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let indices = super::scalar_sources::runtime_indices(
            runs,
            function,
            &super::scalar_sources::ScalarSources::from(&*live),
        )?;
        super::primitive_storage::retain_result(*psi_operation, *result, live)?;
        operations.push(TargetUnitOperation::StructuralCaseMembership {
            psi_operation: *psi_operation,
            result: *result,
            source: *source,
            path: path.clone(),
            tag_byte_offset,
            case: *case,
            case_tag,
            indices,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    pub(crate) fn parameter_root(
        prepared: &crate::function_signature::PreparedFunctionSignature,
        source: PlaceId,
    ) -> Option<&TargetStructuralParameter> {
        prepared
            .parameter(source)
            .filter(|parameter| parameter.access != StructuralAccess::WriteOnlyBorrow)
    }

    fn case_source(
        function: &AbstractFunction,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
        source: PlaceId,
    ) -> Result<TargetStructuralCaseSource, LoweringError> {
        if let Some(home) = live.structural_homes.get(&source) {
            return Ok(TargetStructuralCaseSource::Home(home.clone()));
        }
        if let Some(parameter) = super::transfers::address_parameter(function, live, source) {
            let borrowed = matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            );
            if !borrowed {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let layout = value_home_layout(parameter.structural_type, structural_types)?;
            return Ok(TargetStructuralCaseSource::BorrowedBlockParameter {
                parameter: parameter.clone(),
                layout,
            });
        }
        let parameter = parameter_root(prepared, source)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let declaration = function
            .structural_parameter(source)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let borrowed = matches!(
            declaration.access,
            terminal_psi::reading_borrow_pair_pattern!()
        );
        if (!borrowed && declaration.access != StructuralAccess::Owned)
            || parameter.access != declaration.access
            || parameter.multiplicity != declaration.multiplicity
            || parameter.structural_type != declaration.structural_type
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let layout = super::aggregate_results::value_home_layout(
            parameter.structural_type,
            structural_types,
        )?;
        if borrowed {
            if parameter.shape.class != calling_conventions::ValueClass::BorrowedReference {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            return Ok(TargetStructuralCaseSource::BorrowedParameter {
                parameter: parameter.clone(),
                layout,
            });
        }
        if layout.shape() != parameter.shape {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(TargetStructuralCaseSource::Parameter {
            parameter: parameter.clone(),
            layout,
        })
    }

    #[allow(clippy::type_complexity)]
    fn case_projection(
        identity: StructuralTypeId,
        path: &[StructuralPathSegment],
        case: semantic_vocabulary::StructuralCaseId,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<(u32, u32, Vec<(semantic_vocabulary::ValueId, u32)>), LoweringError> {
        let invalid = || LoweringError::UnknownStructuralType(identity);
        let regions = crate::structural_layout::reference_projection_regions(identity, path, types)
            .ok_or_else(invalid)?;
        let (identity, _, tag_byte_offset, leaf_runs) =
            crate::structural_layout::runtime_projection(regions.root, regions.path, types)?;
        let runs = regions.selector_runs(leaf_runs);
        let declaration = types.get(identity).ok_or_else(invalid)?;
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            &declaration.shape
        else {
            return Err(invalid());
        };
        let case_tag = cases
            .iter()
            .position(|candidate| candidate.id == case)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .ok_or_else(invalid)?;
        Ok((tag_byte_offset, case_tag, runs))
    }

    pub(crate) fn lower(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &mut LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<TargetControlTerminator, LoweringError> {
        let AbstractOperation::StructuralCase {
            source,
            path,
            cases,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let home = case_source(function, prepared, live, structural_types, *source)?;
        let regions = reference_projection_regions(home.structural_type(), path, structural_types)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if regions
            .pointer_slots
            .iter()
            .any(|slot| !slot.indices.is_empty())
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (selected_type, _, byte_offset) =
            crate::structural_layout::resolve_structural_projection_path(
                regions.root,
                regions.path,
                structural_types,
            )?;
        let declaration = structural_types
            .get(selected_type)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let (StructuralTypeShape::Sum {
            cases: declared_cases,
        }
        | StructuralTypeShape::Mixed {
            cases: declared_cases,
            ..
        }) = &declaration.shape
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let layout =
            crate::structural_layout::structural_sum_layout(selected_type, structural_types)?;
        if cases.len() != declared_cases.len() || cases.len() != layout.cases.len() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut lowered = Vec::with_capacity(cases.len());
        let mut reference_exits = Vec::with_capacity(cases.len());
        for (case_ordinal, (case, declared)) in cases.iter().zip(declared_cases).enumerate() {
            let target = function
                .block_entry(case.target)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            if case.case != declared.id
                || target.structural_parameters.len() != case.structural_bindings.len()
                || case.payloads.len() != target.parameters.len()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let mut edge_live = live.clone();
            super::transfers::re_home_bound_references(
                function,
                structural_types,
                &case.structural_bindings,
                &target.structural_parameters,
                &mut edge_live,
            )?;
            plain_home_cleanup(
                function,
                &mut edge_live,
                structural_types,
                &case.cleanup_actions,
            )?;
            reference_exits.push(edge_live.reference_edge_exit(case.target));
            let fields = declared
                .fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            if fields.len() != layout.cases[case_ordinal].fields.len() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let mut payloads = Vec::with_capacity(case.payloads.len());
            for (payload, parameter) in case.payloads.iter().zip(&target.parameters) {
                let field_ordinal = fields
                    .iter()
                    .position(|field| field.id == payload.field)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let field = fields[field_ordinal];
                let shape = crate::scalar_abi::fixed_native_scalar_shape(payload.scalar_type)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let field_layout = layout.cases[case_ordinal].fields[field_ordinal];
                if field.field_type.scalar_type() != Some(payload.scalar_type)
                    || field_layout.shape != shape
                    || parameter.value != payload.parameter
                    || parameter.scalar_type != payload.scalar_type
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                payloads.push(TargetControlCasePayload {
                    field: payload.field,
                    field_byte_offset: byte_offset
                        .checked_add(field_layout.byte_offset)
                        .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
                    parameter: TargetScalarBlockValue {
                        block: target.block,
                        value: parameter.value,
                        scalar_type: parameter.scalar_type,
                    },
                });
            }
            lowered.push(TargetControlCaseSuccessor {
                psi_edge: case.psi_edge,
                case: case.case,
                case_tag: i32::try_from(case_ordinal)
                    .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
                target: case.target,
                payloads,
                structural_bindings: case.structural_bindings.clone(),
                cleanup_actions: case.cleanup_actions.clone(),
            });
        }
        live.finish_reference_edges(reference_exits);
        provenance
            .edges
            .extend(cases.iter().map(|case| case.psi_edge));
        Ok(TargetControlTerminator::StructuralCase {
            source: home,
            path: path.clone(),
            byte_offset,
            layout,
            cases: lowered,
        })
    }
}

mod terminator {
    use super::LiveDefinitions;
    use super::{observations, structural_case};
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{
        AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractParameter,
    };
    use calling_conventions::{CallingPolicy, evaluate_call_plan};
    use semantic_vocabulary::BoundaryMachineId;
    use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
    use std::collections::BTreeMap;
    use target_operations::{BoundarySettlementBinding, TargetStructuralReturnSource};
    use target_operations::{
        TargetBooleanExpression, TargetScalarExpression, TerminalPsiProvenance,
    };
    use target_operations::{TargetControlSuccessor, TargetControlTerminator};
    use terminal_psi::BoundaryMachineDeclaration;
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction};
    use terminal_semantics::descriptor_results::is_descriptor;

    fn disposable_view_carrier(
        live: &LiveDefinitions,
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        place: PlaceId,
    ) -> Option<StructuralMultiplicity> {
        let (structural_type, multiplicity) = if live.views.contains_key(&place) {
            let result = live.view_result(function, place)?;
            if !matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !result.claims.is_empty()
                || !terminal_semantics::view_descriptor_projected_qualifications(
                    types.values().copied(),
                    result.structural_type,
                    &result.projected_qualifications,
                )
            {
                return None;
            }
            (result.structural_type, result.multiplicity)
        } else {
            if !live.block_views.contains(&place) {
                return None;
            }
            let parameter = function.arrival_parameter(place)?;
            if !terminal_semantics::view_descriptor_parameter(parameter, types.values().copied()) {
                return None;
            }
            (parameter.structural_type, parameter.multiplicity)
        };
        terminal_semantics::descriptor_results::is_descriptor(
            types.values().copied(),
            structural_type,
        )
        .then_some(multiplicity)
    }

    fn retirable_reference_arrival(
        live: &LiveDefinitions,
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        place: PlaceId,
    ) -> bool {
        let Some(parameter) = function.arrival_parameter(place) else {
            return false;
        };
        if parameter.access == StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || function
                .entry_claims
                .iter()
                .any(|claim| claim.input == place)
        {
            return false;
        }
        let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
            types.get(parameter.structural_type).map(|ty| &ty.shape)
        else {
            return false;
        };
        live.references
            .get(&(place, Vec::new()))
            .is_some_and(|descriptor| {
                descriptor.carrier_type == parameter.structural_type
                    && descriptor.referent == *referent
            })
    }

    fn owned_root_type(
        live: &LiveDefinitions,
        function: &AbstractFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        if let Some((_, structural_type)) = live.trivial_affine_locals.get(&place) {
            return Some(*structural_type);
        }
        if let Some(home) = live.structural_homes.get(&place) {
            return (home.access() == StructuralAccess::Owned).then_some(home.structural_type());
        }
        if let Some(parameter) = function
            .arrival_parameter(place)
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
        {
            return Some(parameter.structural_type);
        }
        None
    }

    fn live_home_or_arrival(
        live: &LiveDefinitions,
        function: &AbstractFunction,
        place: PlaceId,
    ) -> bool {
        owned_root_type(live, function, place).is_some()
    }

    fn residual_cleanup(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
        discard: &terminal_psi::StructuralAffineDiscard,
    ) -> bool {
        owned_root_type(live, function, discard.place).is_some_and(|root| {
            structural_types.subtree(root, &discard.path) == Some(discard.structural_type)
        })
    }

    pub(crate) fn plain_home_cleanup(
        function: &AbstractFunction,
        live: &mut LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
        actions: &[TerminalAffineCleanupAction],
    ) -> Result<(), LoweringError> {
        for action in actions {
            match action {
                TerminalAffineCleanupAction::RetireHome(place) => {
                    if owned_root_type(live, function, *place).is_none()
                        && !live.retired_reference_roots.contains(place)
                        && disposable_view_carrier(live, function, structural_types, *place)
                            .is_none()
                        && !retirable_reference_arrival(live, function, structural_types, *place)
                    {
                        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                            eprintln!(
                                "CHECKSITE descriptor-home-retirement machine={:?} place={place:?} references={:?} home={:?} arrival={:?} view={:?} block_view={} retired={}",
                                function.machine,
                                live.references
                                    .keys()
                                    .filter(|(root, _)| root == place)
                                    .collect::<Vec<_>>(),
                                live.structural_homes.get(place),
                                function.arrival_parameter(*place),
                                live.views.get(place),
                                live.block_views.contains(place),
                                live.retired_reference_roots.contains(place)
                            );
                        }
                        return Err(LoweringError::InvalidStructuralValueInput {
                            machine: function.machine,
                            place: *place,
                            reason: "home retirement has no owned home or materialized view carrier",
                        });
                    }
                    live.references.retain(|(carrier, path), _| {
                        carrier != place
                            || path.contains(&terminal_psi::StructuralPathSegment::Referent)
                    });
                }
                TerminalAffineCleanupAction::DiscardRoot(place) => {
                    if live.views.contains_key(place) || live.block_views.contains(place) {
                        let carrier =
                            disposable_view_carrier(live, function, structural_types, *place);
                        if carrier != Some(StructuralMultiplicity::Affine) {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!(
                                    "descriptor-discard refusal machine={:?} action={action:?} helper={carrier:?} view={:?} result={:?} arrival={:?} block_view={} retired={} home={:?}",
                                    function.machine,
                                    live.views.get(place),
                                    live.view_result(function, *place),
                                    function.arrival_parameter(*place),
                                    live.block_views.contains(place),
                                    live.retired_reference_roots.contains(place),
                                    live.structural_homes.get(place).map(|home| (
                                        home.structural_type(),
                                        home.access(),
                                        home.multiplicity(),
                                        home.qualifications(),
                                        home.projected_qualifications(),
                                    )),
                                );
                            }
                            return Err(LoweringError::InvalidStructuralValueInput {
                                machine: function.machine,
                                place: *place,
                                reason: "view discard has no affine descriptor carrier",
                            });
                        }
                        continue;
                    }
                    if live
                        .references
                        .keys()
                        .any(|(carrier, _)| *carrier == *place)
                        || live.retired_reference_roots.contains(place)
                    {
                        super::references::discard_owned(function, structural_types, live, *place)?;
                        continue;
                    }
                    if !live_home_or_arrival(live, function, *place) {
                        return Err(LoweringError::InvalidStructuralValueInput {
                            machine: function.machine,
                            place: *place,
                            reason: "root discard has no live home or arrival",
                        });
                    }
                    super::references::discard_owned(function, structural_types, live, *place)?;
                }
                TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    if !residual_cleanup(function, live, structural_types, discard) {
                        return Err(LoweringError::InvalidStructuralValueInput {
                            machine: function.machine,
                            place: discard.place,
                            reason: "residual discard does not match the owned storage subtree",
                        });
                    }
                    super::references::discard_subtree(
                        function,
                        structural_types,
                        live,
                        discard.place,
                        &discard.path,
                    )?;
                }
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    if owned_root_type(live, function, cleanup.place)
                        .and_then(|root| structural_types.subtree(root, &cleanup.path))
                        != Some(cleanup.structural_type)
                        || !cleanup.requirement_obligations.is_empty()
                    {
                        return Err(LoweringError::InvalidStructuralValueInput {
                            machine: function.machine,
                            place: cleanup.place,
                            reason: "nominal cleanup receiver or requirement obligations are not realized",
                        });
                    }
                    super::references::discard_subtree(
                        function,
                        structural_types,
                        live,
                        cleanup.place,
                        &cleanup.path,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn lower_terminator(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &mut LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
        target: target::NativeTarget,
        boundary_machines: &BTreeMap<BoundaryMachineId, &BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
        selected_calls: &crate::selected_boundary_calls::SelectedCallsByCall,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<TargetControlTerminator, LoweringError> {
        let cleanup = |live: &mut LiveDefinitions,
                       actions: &[TerminalAffineCleanupAction]|
         -> Result<Vec<TerminalAffineCleanupAction>, LoweringError> {
            plain_home_cleanup(function, live, structural_types, actions)?;
            Ok(actions.to_vec())
        };
        let successor = |live: &mut LiveDefinitions,
                         edge: &abstract_operations::AbstractSuccessor|
         -> Result<TargetControlSuccessor, LoweringError> {
            let destination = function
                .block_entry(edge.target)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            super::transfers::re_home_bound_references(
                function,
                structural_types,
                &edge.structural_bindings,
                &destination.structural_parameters,
                live,
            )?;
            Ok(TargetControlSuccessor {
                psi_edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
                structural_bindings: edge.structural_bindings.clone(),
                cleanup_actions: cleanup(live, &edge.cleanup_actions)?,
            })
        };
        match operation {
            AbstractOperation::ExternalCompletion {
                psi_edge,
                boundary,
                kind,
                arguments,
                structural_arguments,
                requirement_obligations,
                frontier_lower_bound,
            } => {
                let declaration = boundary_machines
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                if declaration.completion
                    != semantic_vocabulary::BoundaryCompletion::External(*kind)
                    || !declaration.result.is_unit()
                    || declaration.scalar_parameters.len() != arguments.len()
                    || declaration.structural_parameters.len() != structural_arguments.len()
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
                let shapes = declaration
                    .scalar_parameters
                    .iter()
                    .map(|scalar_type| {
                        crate::scalar_abi::fixed_native_scalar_shape(*scalar_type)
                            .ok_or(LoweringError::BoundaryRealizationMismatch(*boundary))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let signature = calling_conventions::CallSignature {
                    parameters: shapes,
                    result: None,
                };
                let native_plan = || {
                    evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
                        .map_err(LoweringError::AbiPlan)
                };
                let key = (
                    function.machine,
                    semantic_vocabulary::InvocationSite::ExternalCompletion(*psi_edge),
                    *boundary,
                );
                let (binding, call_plan) = if let Some(selected) = selected_calls.get(&key) {
                    let call_plan = native_plan()?;
                    (
                        target_operations::ExternalCompletionBinding::Checked {
                            callee: selected.callee,
                            origin: selected.origin.clone(),
                            call_plan: call_plan.clone(),
                        },
                        call_plan,
                    )
                } else {
                    let binding = settlements
                        .get(boundary)
                        .cloned()
                        .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
                    let call_plan = match &binding.realization {
                        target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                            foreign,
                        ) => calling_conventions::validate_boundary_entry_plan(
                            foreign.boundary_entry_plan.clone(),
                            &signature,
                        )
                        .map_err(LoweringError::AbiPlan)?
                        .plan()
                        .call
                        .clone(),
                        _ => native_plan()?,
                    };
                    (
                        target_operations::ExternalCompletionBinding::Boundary(binding),
                        call_plan,
                    )
                };
                let arguments = arguments
                    .iter()
                    .zip(&call_plan.parameters)
                    .zip(&declaration.scalar_parameters)
                    .enumerate()
                    .map(|(position, ((source, placement), expected))| {
                        let source = super::scalar_sources::source(*source, function, live)?;
                        if source.scalar_type() != *expected {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        Ok(target_operations::TargetUnitScalarCallArgument {
                            parameter_index: u32::try_from(position).map_err(|_| {
                                LoweringError::BoundaryRealizationMismatch(*boundary)
                            })?,
                            source,
                            placement: placement.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                provenance.edges.push(*psi_edge);
                Ok(TargetControlTerminator::ExternalCompletion {
                    psi_edge: *psi_edge,
                    boundary: *boundary,
                    kind: *kind,
                    binding,
                    arguments,
                    structural_arguments: structural_arguments.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                })
            }
            AbstractOperation::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                provenance.edges.push(*psi_edge);
                Ok(TargetControlTerminator::Crash {
                    psi_edge: *psi_edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                })
            }
            AbstractOperation::ReturnStructural {
                psi_edge,
                source,
                returned_claims,
                trivial_affine_locals,
                cleanup_actions,
            } => {
                let result = function
                    .result
                    .structural()
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                plain_home_cleanup(function, live, structural_types, cleanup_actions)?;
                let cleanup_actions = cleanup_actions.clone();
                if is_descriptor(structural_types.values().copied(), result.structural_type) {
                    let source_type = live
                        .structural_homes
                        .get(source)
                        .filter(|home| {
                            home.multiplicity() == result.multiplicity
                                && terminal_semantics::view_descriptor_projected_qualifications(
                                    structural_types.values().copied(),
                                    home.structural_type(),
                                    home.projected_qualifications(),
                                )
                        })
                        .map(|home| home.structural_type())
                        .or_else(|| {
                            live.views
                                .get(source)
                                .filter(|_| {
                                    result.multiplicity == StructuralMultiplicity::Unrestricted
                                        || disposable_view_carrier(
                                            live,
                                            function,
                                            structural_types,
                                            *source,
                                        ) == Some(StructuralMultiplicity::Affine)
                                })
                                .map(|(_, identity)| *identity)
                        })
                        .or_else(|| {
                            function
                                .arrival_parameter(*source)
                                .filter(|parameter| {
                                    (function
                                    .structural_parameters
                                    .iter()
                                    .any(|entry| entry.place == *source)
                                    || live.block_views.contains(source))
                                    && matches!(
                                        parameter.access,
                                        StructuralAccess::Owned
                                            | StructuralAccess::SharedBorrow
                                            | StructuralAccess::MutableBorrow
                                    )
                                    && (result.multiplicity != StructuralMultiplicity::Affine
                                        || parameter.access != StructuralAccess::SharedBorrow)
                                    && (parameter.multiplicity == result.multiplicity
                                        || result.multiplicity == StructuralMultiplicity::Affine
                                            && parameter.access == StructuralAccess::MutableBorrow
                                            && parameter.multiplicity
                                                == StructuralMultiplicity::Unrestricted)
                                    && terminal_semantics::view_descriptor_projected_qualifications(
                                        structural_types.values().copied(),
                                        parameter.structural_type,
                                        &parameter.projected_qualifications,
                                    )
                                })
                                .map(|parameter| parameter.structural_type)
                        });
                    let invalid = |reason| LoweringError::InvalidStructuralValueInput {
                        machine: function.machine,
                        place: *source,
                        reason,
                    };
                    if source_type != Some(result.structural_type) {
                        return Err(invalid(
                            "descriptor return source has no matching live carrier",
                        ));
                    }
                    if !matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    ) {
                        return Err(invalid("descriptor return requires nonlinear custody"));
                    }
                    if !terminal_semantics::view_descriptor_projected_qualifications(
                        structural_types.values().copied(),
                        result.structural_type,
                        &result.projected_qualifications,
                    ) {
                        return Err(invalid(
                            "descriptor return has non-descriptor child memberships",
                        ));
                    }
                    if !returned_claims.is_empty() {
                        return Err(invalid("descriptor return has unlowered returned claims"));
                    }
                    if !trivial_affine_locals.is_empty() {
                        return Err(invalid("descriptor return has unlowered local cleanup"));
                    }
                    if prepared.call_plan.result.as_ref().is_none_or(|placement| {
                        placement.shape != calling_conventions::ValueShape::integer(16, 8)
                    }) {
                        return Err(invalid("descriptor return has no two-word ABI placement"));
                    }
                    provenance.edges.push(*psi_edge);
                    return Ok(TargetControlTerminator::ReturnStructural {
                        psi_edge: *psi_edge,
                        source: target_operations::TargetStructuralReturnSource::SharedDescriptor {
                            place: *source,
                        },
                        cleanup_actions,
                    });
                }
                if let Some(parameter) = function.structural_parameter(*source) {
                    let actual = prepared
                        .parameter(*source)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                    let placement =
                        prepared.call_plan.result.as_ref().ok_or_else(|| {
                            LoweringError::unsupported_control_flow(function.machine)
                        })?;
                    if parameter.access != StructuralAccess::Owned
                        || parameter.structural_type != result.structural_type
                        || parameter.multiplicity != result.multiplicity
                        || !trivial_affine_locals.is_empty()
                        || actual.shape != placement.shape
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    provenance.edges.push(*psi_edge);
                    return Ok(TargetControlTerminator::ReturnStructural {
                        psi_edge: *psi_edge,
                        source: TargetStructuralReturnSource::Parameter(actual.clone()),
                        cleanup_actions,
                    });
                }
                let home = live
                    .structural_homes
                    .get(source)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if result.structural_type != home.structural_type()
                    || result.multiplicity != home.multiplicity()
                    || prepared
                        .call_plan
                        .result
                        .as_ref()
                        .is_none_or(|placement| placement.shape != home.layout.shape())
                    || !trivial_affine_locals.is_empty()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                provenance.edges.push(*psi_edge);
                Ok(TargetControlTerminator::ReturnStructural {
                    psi_edge: *psi_edge,
                    source: target_operations::TargetStructuralReturnSource::Home(home.clone()),
                    cleanup_actions,
                })
            }
            AbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
                cleanup_actions,
            } => {
                let expected = function
                    .result
                    .scalar()
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if *result != expected.value || *scalar_type != expected.scalar_type {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                plain_home_cleanup(function, live, structural_types, cleanup_actions)?;
                let expression = if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                    TargetScalarExpression::IeeeFloat(super::scalar_sources::source(
                        *value, function, live,
                    )?)
                } else {
                    observations::scalar_values(live, &prepared.scalar_parameters)?
                        .remove(value)
                        .ok_or(LoweringError::UnknownValue(*value))?
                        .into_expression(*value)?
                };
                if expression.scalar_type() != *scalar_type {
                    return Err(LoweringError::ValueTypeMismatch(*value));
                }
                provenance.edges.push(*psi_edge);
                Ok(TargetControlTerminator::ReturnScalar {
                    psi_edge: *psi_edge,
                    source_value: *value,
                    expression,
                    cleanup_actions: cleanup_actions.clone(),
                })
            }
            AbstractOperation::StructuralCase { .. } => structural_case::lower(
                operation,
                function,
                prepared,
                live,
                structural_types,
                provenance,
            ),
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            } => {
                if function.result != AbstractFunctionResult::Unit {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                plain_home_cleanup(function, live, structural_types, cleanup_actions)?;
                provenance.edges.push(*psi_edge);
                Ok(TargetControlTerminator::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: cleanup_actions.clone(),
                })
            }
            AbstractOperation::Jump {
                psi_edge,
                target,
                bindings,
                structural_bindings,
                cleanup_actions,
            } => {
                if cleanup_actions.iter().any(|action| {
                    let TerminalAffineCleanupAction::DiscardResidual(discard) = action else {
                        return false;
                    };
                    structural_bindings.iter().any(|binding| {
                        binding.argument.place == discard.place
                            && (binding.argument.path.starts_with(&discard.path)
                                || discard.path.starts_with(&binding.argument.path))
                    })
                }) {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let destination = function
                    .block_entry(*target)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                super::transfers::re_home_bound_references(
                    function,
                    structural_types,
                    structural_bindings,
                    &destination.structural_parameters,
                    live,
                )?;
                let cleanup_actions = cleanup(live, cleanup_actions)?;
                provenance.edges.push(*psi_edge);
                Ok(TargetControlTerminator::Jump {
                    successor: TargetControlSuccessor {
                        psi_edge: *psi_edge,
                        target: *target,
                        bindings: bindings.clone(),
                        structural_bindings: structural_bindings.clone(),
                        cleanup_actions,
                    },
                })
            }
            AbstractOperation::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let expression = if let Some((position, parameter)) = prepared
                    .scalar_parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.value == *condition
                            && parameter.scalar_type == ScalarType::Boolean
                    }) {
                    TargetBooleanExpression::Parameter {
                        source_value: *condition,
                        parameter_index: position,
                        location: crate::scalar::scalar_parameter_location(
                            &AbstractParameter {
                                value: *condition,
                                scalar_type: ScalarType::Boolean,
                            },
                            &parameter.placement,
                        )?,
                    }
                } else if let Some((_, value)) = live.booleans.get(condition) {
                    TargetBooleanExpression::Immediate {
                        source_value: *condition,
                        value: *value,
                    }
                } else if let Some(home) = live.scalar_homes.get(condition)
                    && home.scalar_type == ScalarType::Boolean
                {
                    TargetBooleanExpression::ScalarHome(*home)
                } else if let Some(parameter) = live.scalar_block_parameters.get(condition)
                    && parameter.scalar_type == ScalarType::Boolean
                {
                    TargetBooleanExpression::BlockParameter(*parameter)
                } else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                provenance
                    .edges
                    .extend([when_true.psi_edge, when_false.psi_edge]);
                let mut true_exit = live.clone();
                let mut false_exit = live.clone();
                let lowered_true = successor(&mut true_exit, when_true)?;
                let lowered_false = successor(&mut false_exit, when_false)?;
                live.finish_reference_edges(vec![
                    true_exit.reference_edge_exit(when_true.target),
                    false_exit.reference_edge_exit(when_false.target),
                ]);
                Ok(TargetControlTerminator::Conditional {
                    condition_source: *condition,
                    condition: expression,
                    when_true: lowered_true,
                    when_false: lowered_false,
                })
            }
            _ => Err(LoweringError::unsupported_control_flow(function.machine)),
        }
    }
}

mod transfers {
    use super::{KnownUnitInteger, LiveDefinitions, references};
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::control_flow::address_joins::is_static_projection;
    use abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractOperation, ValueBinding,
    };
    use semantic_vocabulary::{BlockId, ScalarType, ValueId};
    use std::collections::BTreeSet;
    use target_operations::TargetScalarBlockValue;
    use terminal_semantics::descriptor_results::result_access;

    pub(crate) fn validate_parameters(
        function: &AbstractFunction,
        definitions: &mut BTreeSet<ValueId>,
    ) -> Result<(), LoweringError> {
        for block in function
            .block_entries
            .iter()
            .filter(|block| block.block != function.entry)
        {
            for parameter in &block.parameters {
                if !matches!(
                    parameter.scalar_type,
                    ScalarType::Boolean | ScalarType::IeeeFloat(_)
                ) && !matches!(parameter.scalar_type, ScalarType::Integer(integer)
                    if crate::scalar_abi::fixed_native_integer_shape(integer).is_some())
                {
                    return Err(LoweringError::ValueTypeMismatch(parameter.value));
                }
                if !definitions.insert(parameter.value) {
                    return Err(LoweringError::DuplicateValue(parameter.value));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn re_home_bound_references(
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        bindings: &[abstract_operations::AbstractStructuralBinding],
        parameters: &[terminal_psi::StructuralParameterDeclaration],
        live: &mut LiveDefinitions,
    ) -> Result<(), LoweringError> {
        if bindings.len() != parameters.len() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let borrowed_sources =
            references::capture_borrowed_arrivals(function, live, types, bindings, parameters)?;
        let mut captured = Vec::new();
        let mut selected_sources = BTreeSet::new();
        let mut destinations = BTreeSet::new();
        for (binding, parameter) in bindings.iter().zip(parameters) {
            if binding.parameter != parameter.place {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if parameter.access != terminal_psi::StructuralAccess::Owned
                || binding.argument.access != terminal_psi::StructuralAccess::Owned
            {
                continue;
            }
            let copies =
                parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted;
            if copies {
                let source_contract = live
                    .structural_homes
                    .get(&binding.argument.place)
                    .map(|home| (home.access(), home.multiplicity()))
                    .or_else(|| {
                        function
                            .structural_parameters
                            .iter()
                            .chain(function.block_structural_parameters())
                            .find(|source| source.place == binding.argument.place)
                            .map(|source| (source.access, source.multiplicity))
                    });
                if source_contract
                    != Some((
                        terminal_psi::StructuralAccess::Owned,
                        terminal_psi::StructuralMultiplicity::Unrestricted,
                    ))
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
            }
            for (source, descriptor) in &live.references {
                if source.0 != binding.argument.place {
                    continue;
                }
                let Some(relative) = source.1.strip_prefix(binding.argument.path.as_slice()) else {
                    continue;
                };
                if relative.contains(&terminal_psi::StructuralPathSegment::Referent) {
                    continue;
                }
                let destination = (parameter.place, relative.to_vec());
                if references::leaf_referent(types, parameter.structural_type, relative)
                    != Some(descriptor.referent)
                    || (!copies && !selected_sources.insert(source.clone()))
                    || !destinations.insert(destination.clone())
                    || (copies
                        && !types
                            .get(descriptor.carrier_type)
                            .is_some_and(|declaration| {
                                matches!(
                                    declaration.shape,
                                    terminal_psi::StructuralTypeShape::Reference {
                                        access: terminal_psi::StructuralAccess::SharedBorrow,
                                        ..
                                    }
                                )
                            }))
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                captured.push((
                    source.clone(),
                    destination,
                    descriptor.clone(),
                    references::leaf_type(types, parameter.structural_type, relative)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                ));
            }
        }
        live.references.retain(|(root, _), _| {
            !parameters.iter().any(|parameter| {
                parameter.place == *root
                    && parameter.access == terminal_psi::StructuralAccess::Owned
            })
        });
        for source in &selected_sources {
            live.references.remove(source);
        }
        for (_, destination, mut storage, carrier_type) in captured {
            storage.carrier_type = carrier_type;
            live.references.insert(destination, storage);
        }
        live.borrowed_reference_sources = borrowed_sources;
        Ok(())
    }

    pub(crate) fn enter(block: &AbstractBlockEntry, live: &mut LiveDefinitions) {
        for parameter in &block.parameters {
            match parameter.scalar_type {
                ScalarType::Integer(scalar_type) => {
                    live.integers.insert(
                        parameter.value,
                        KnownUnitInteger::BlockParameter {
                            block: block.block,
                            value: parameter.value,
                            scalar_type,
                        },
                    );
                }
                ScalarType::Boolean | ScalarType::IeeeFloat(_) => {
                    live.scalar_block_parameters.insert(
                        parameter.value,
                        TargetScalarBlockValue {
                            block: block.block,
                            value: parameter.value,
                            scalar_type: parameter.scalar_type,
                        },
                    );
                }
            }
        }
    }

    pub(crate) fn is_address_join(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        abstract_operations::control_flow::address_joins::is_address_join(parameter, |identity| {
            structural_types.get(identity).copied()
        })
    }

    pub(crate) fn address_parameter<'a>(
        function: &'a AbstractFunction,
        live: &LiveDefinitions,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<&'a terminal_psi::StructuralParameterDeclaration> {
        if !live.address_joins.contains(&place) {
            return None;
        }
        function.block_structural_parameter(place)
    }

    fn address_join_argument(
        function: &AbstractFunction,
        live: &LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
        binding: &abstract_operations::AbstractStructuralBinding,
        parameter: &terminal_psi::StructuralParameterDeclaration,
    ) -> bool {
        let argument = &binding.argument;
        if argument.access != parameter.access
            || !abstract_operations::control_flow::address_joins::is_address_projection(
                &argument.path,
            )
        {
            return false;
        }
        let accessible = |root, multiplicity, access| {
            terminal_semantics::reference_source_projection_accessible(
                structural_types.values().copied(),
                root,
                multiplicity,
                access,
                &argument.path,
                argument.access,
            )
        };
        let root_type = if let Some(home) = live.structural_homes.get(&argument.place) {
            (home.place() == argument.place
                && (parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                    || home.access() == terminal_psi::StructuralAccess::Owned)
                && accessible(home.structural_type(), home.multiplicity(), home.access()))
            .then(|| home.structural_type())
        } else if let Some(source) = function.structural_parameter(argument.place) {
            (abstract_operations::control_flow::address_joins::loan_access_permitted(
                structural_types.values().copied(),
                source.structural_type,
                &argument.path,
                source.access,
                parameter.access,
            ) && accessible(source.structural_type, source.multiplicity, source.access))
            .then_some(source.structural_type)
        } else if live.address_joins.contains(&argument.place) {
            function
                .block_structural_parameters()
                .find(|source| source.place == argument.place)
                .filter(|source| {
                    abstract_operations::control_flow::address_joins::loan_access_permitted(
                        structural_types.values().copied(),
                        source.structural_type,
                        &argument.path,
                        source.access,
                        parameter.access,
                    ) && accessible(source.structural_type, source.multiplicity, source.access)
                })
                .map(|source| source.structural_type)
        } else {
            (parameter.access == terminal_psi::StructuralAccess::SharedBorrow)
                .then_some(())
                .and_then(|_| {
                    let (producer, structural_type) = live.views.get(&argument.place)?;
                    let (_, result) = function
                        .operations
                        .iter()
                        .find(|operation| operation.psi_operation() == Some(*producer))?
                        .structural_result()?;
                    let access = result_access(structural_types.values().copied(), result)?;
                    (result.structural_type == *structural_type
                        && accessible(*structural_type, result.multiplicity, access))
                    .then_some(*structural_type)
                })
        };
        root_type.and_then(|root| {
            abstract_operations::control_flow::address_joins::borrowed_referent_type(
                structural_types.values().copied(),
                root,
                &argument.path,
                argument.access,
            )
        }) == Some(parameter.structural_type)
    }

    pub(crate) fn validate_successors(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        live: &LiveDefinitions,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Result<(), LoweringError> {
        let validate =
            |target: BlockId,
             bindings: &[ValueBinding],
             structural: &[abstract_operations::AbstractStructuralBinding]| {
                let block = function
                    .block_entry(target)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if bindings.len() != block.parameters.len() {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                if structural.len() != block.structural_parameters.len() {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                for (binding, parameter) in structural.iter().zip(&block.structural_parameters) {
                    if is_address_join(parameter, structural_types) {
                        if binding.parameter != parameter.place
                            || !address_join_argument(
                                function,
                                live,
                                structural_types,
                                binding,
                                parameter,
                            )
                        {
                            return Err(LoweringError::StructuralStateBindingMismatch {
                                machine: function.machine,
                                reason: format!(
                                    "borrowed address edge to {target:?}: binding {binding:?} does not satisfy {parameter:?}; live home {:?}; address arrival {}",
                                    live.structural_homes.get(&binding.argument.place).map(
                                        |home| (
                                            home.structural_type(),
                                            home.access(),
                                            home.multiplicity(),
                                            home.has_claims(),
                                        )
                                    ),
                                    live.address_joins.contains(&binding.argument.place),
                                ),
                            });
                        }
                        continue;
                    }
                    if terminal_semantics::view_descriptor_parameter(
                        parameter,
                        structural_types.values().copied(),
                    ) {
                        let selected = crate::scalar::descriptor_operands::descriptor_projection(
                            function,
                            structural_types,
                            live.element_view_backings(),
                            &binding.argument,
                        )?;
                        let invalid = |reason| LoweringError::DescriptorStateBindingMismatch {
                            machine: function.machine,
                            source: binding.argument.place,
                            parameter: parameter.place,
                            supplied_type: selected.0,
                            required_type: parameter.structural_type,
                            supplied_access: binding.argument.access,
                            required_access: parameter.access,
                            reason,
                        };
                        if binding.parameter != parameter.place {
                            return Err(invalid("descriptor edge names a different parameter"));
                        }
                        if binding.argument.access != parameter.access {
                            return Err(invalid("descriptor edge changes its admitted access"));
                        }
                        if selected.0 != parameter.structural_type
                            && !crate::scalar::descriptor_operands::storage_backs_view_parameter(
                                selected.0,
                                &binding.argument.path,
                                parameter,
                                structural_types,
                            )
                        {
                            return Err(invalid(
                                "descriptor edge changes its value representation",
                            ));
                        }
                        continue;
                    }
                    if references::contains_reference(structural_types, parameter.structural_type)
                        && (parameter.access != terminal_psi::StructuralAccess::Owned
                            || binding.argument.access != terminal_psi::StructuralAccess::Owned)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    let place = binding.argument.place;
                    let source_type = function
                        .structural_parameter(place)
                        .map(|source| source.structural_type)
                        .or_else(|| {
                            live.structural_homes
                                .get(&place)
                                .map(|home| home.structural_type())
                        })
                        .or_else(|| {
                            live.views
                                .get(&place)
                                .map(|(_, structural_type)| *structural_type)
                        })
                        .or_else(|| {
                            (live.block_views.contains(&place)
                                || live.owned_arrivals.contains(&place))
                            .then(|| {
                                function
                                    .block_structural_parameters()
                                    .find(|source| source.place == place)
                                    .map(|source| source.structural_type)
                            })
                            .flatten()
                        });
                    let source_storage_contract = live
                        .structural_homes
                        .get(&place)
                        .map(|home| {
                            home.access() == parameter.access
                                && (!binding.argument.path.is_empty()
                                    || home.multiplicity() == parameter.multiplicity)
                        })
                        .or_else(|| {
                            function
                                .structural_parameters
                                .iter()
                                .chain(function.block_structural_parameters())
                                .find(|source| source.place == place)
                                .map(|source| {
                                    source.access == parameter.access
                                        && (!binding.argument.path.is_empty()
                                            || source.multiplicity == parameter.multiplicity)
                                })
                        });
                    let leaf_copy_owned_contract = || {
                        function
                            .operations
                            .iter()
                            .find_map(|operation| match operation {
                                AbstractOperation::StructuralLeafCopy {
                                    source: backing,
                                    path: abstract_operations::StructuralCopyPath::Ordinary(path),
                                    result,
                                    ..
                                } if result.place == place
                                    && result.structural_type == parameter.structural_type
                                    && result.multiplicity
                                        == terminal_psi::StructuralMultiplicity::Unrestricted
                                    && result.claims.is_empty() =>
                                {
                                    let root = function
                                        .arrival_parameter(*backing)
                                        .map(|root| root.structural_type)
                                        .or_else(|| {
                                            function
                                                .block_structural_parameters()
                                                .find(|source| source.place == *backing)
                                                .map(|source| source.structural_type)
                                        });
                                    Some(root.is_some_and(|root| {
                                        structural_types.subtree(root, path)
                                            == Some(parameter.structural_type)
                                    }))
                                }
                                _ => None,
                            })
                    };
                    let argument_type = source_type.and_then(|root| {
                        if binding.argument.path.is_empty() {
                            Some(root)
                        } else {
                            structural_types.subtree(root, &binding.argument.path)
                        }
                    });
                    let crosses_loan_leaf = live.references.keys().any(|(carrier, leaf_path)| {
                        *carrier == place
                            && binding.argument.path.len() > leaf_path.len()
                            && binding.argument.path.starts_with(leaf_path)
                    });
                    if binding.parameter != parameter.place
                        || (parameter.access == terminal_psi::StructuralAccess::Owned
                            && source_storage_contract != Some(true)
                            && leaf_copy_owned_contract() != Some(true))
                        || !argument_type.is_some_and(|actual| {
                            if parameter.access == terminal_psi::StructuralAccess::Owned {
                                actual == parameter.structural_type
                                    || terminal_semantics::descriptor_value_transfer_matches(
                                        structural_types.values().copied(),
                                        actual,
                                        parameter.structural_type,
                                        parameter.multiplicity,
                                    )
                            } else {
                                terminal_semantics::borrowed_referent_matches(
                                    structural_types.values().copied(),
                                    actual,
                                    parameter.structural_type,
                                )
                            }
                        })
                        || binding.argument.access != parameter.access
                        || crosses_loan_leaf
                        || (!binding.argument.path.is_empty()
                            && (!is_static_projection(&binding.argument.path)
                                || (parameter.access != terminal_psi::StructuralAccess::Owned
                                    && !structural_types
                                        .get(parameter.structural_type)
                                        .is_some_and(|declaration| {
                                            declaration.shape.is_bounded_owned_byte_sequence()
                                        }))))
                        || !matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::SharedBorrow
                                | terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::Owned
                        )
                    {
                        return Err(LoweringError::StructuralStateBindingMismatch {
                            machine: function.machine,
                            reason: format!(
                                "binding={binding:?}; parameter={parameter:?}; source_type={source_type:?}; argument_type={argument_type:?}; source_storage_contract={source_storage_contract:?}; leaf_copy_owned_contract={:?}; crosses_loan_leaf={crosses_loan_leaf}",
                                leaf_copy_owned_contract(),
                            ),
                        });
                    }
                }
                for (binding, parameter) in bindings.iter().zip(&block.parameters) {
                    let scalar_type = live
                        .integers
                        .get(&binding.argument)
                        .map(|known| ScalarType::Integer(known.scalar_type()))
                        .or_else(|| {
                            live.scalar_homes
                                .get(&binding.argument)
                                .map(|home| home.scalar_type)
                        })
                        .or_else(|| {
                            live.booleans
                                .contains_key(&binding.argument)
                                .then_some(ScalarType::Boolean)
                        })
                        .or_else(|| {
                            live.scalar_block_parameters
                                .get(&binding.argument)
                                .map(|source| source.scalar_type)
                        })
                        .or_else(|| {
                            live.ieee_float_constants
                                .get(&binding.argument)
                                .map(|(_, value)| ScalarType::IeeeFloat(value.format()))
                        })
                        .or_else(|| {
                            function
                                .parameters
                                .iter()
                                .find(|parameter| parameter.value == binding.argument)
                                .map(|parameter| parameter.scalar_type)
                        });
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || scalar_type != Some(parameter.scalar_type)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                }
                Ok(())
            };
        match operation {
            AbstractOperation::Jump {
                target,
                bindings,
                structural_bindings,
                ..
            } => validate(*target, bindings, structural_bindings),
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                validate(
                    when_true.target,
                    &when_true.bindings,
                    &when_true.structural_bindings,
                )?;
                validate(
                    when_false.target,
                    &when_false.bindings,
                    &when_false.structural_bindings,
                )
            }
            AbstractOperation::Return { .. }
            | AbstractOperation::InvokeNoNormal { .. }
            | AbstractOperation::ExternalCompletion { .. }
            | AbstractOperation::Crash { .. }
            | AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::ReturnUnit { .. }
            | AbstractOperation::StructuralCase { .. } => Ok(()),
            _ => Err(LoweringError::unsupported_control_flow(function.machine)),
        }
    }
}
use operations::lower_operation;
use row::RowSite;
use target_operations::{TargetControlBlock, TargetControlGraph, TargetScalarBlockParameter};
use terminator::lower_terminator;

#[derive(Clone)]
pub(crate) struct ReferenceEdgeStorage {
    target: semantic_vocabulary::BlockId,
    borrowed_sources: references::BorrowedSources,
    references: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), references::ReferenceStorage>,
    retired_reference_roots: BTreeSet<PlaceId>,
}

// Saved block states share unchanged homes. An insertion copies the map only
// when another state still needs its previous contents.
pub(crate) type ScalarHomes = std::sync::Arc<BTreeMap<ValueId, TargetUnitScalarHomeRequirement>>;

#[derive(Clone)]
pub(crate) struct LiveDefinitions {
    borrowed_reference_sources: references::BorrowedSources,
    stored_reference_effects: references::StoredEffectCatalog,
    reference_position: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
    access_formals: BTreeMap<PlaceId, terminal_psi::TerminalAccessViewInput>,
    structural_homes: BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    integers: BTreeMap<ValueId, KnownUnitInteger>,
    booleans: BTreeMap<ValueId, (OperationId, bool)>,
    scalar_homes: ScalarHomes,
    ieee_float_constants: BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    scalar_block_parameters: BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
    views: BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    block_views: BTreeSet<PlaceId>,
    address_joins: BTreeSet<PlaceId>,
    owned_arrivals: BTreeSet<PlaceId>,
    lengths: BTreeMap<ValueId, PlaceId>,
    trivial_affine_locals: BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    references: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), references::ReferenceStorage>,
    retired_reference_roots: BTreeSet<PlaceId>,
    reference_edge_exits: Vec<ReferenceEdgeStorage>,
}

impl LiveDefinitions {
    fn element_view_backings(&self) -> super::scalar::element_views::ElementViewBackings<'_> {
        super::scalar::element_views::ElementViewBackings::new(
            &self.structural_homes,
            &self.views,
            &self.address_joins,
            scalar_sources::ScalarSources::from(self),
        )
    }

    fn view_result<'function>(
        &self,
        function: &'function AbstractFunction,
        place: PlaceId,
    ) -> Option<&'function terminal_psi::StructuralOperationResult> {
        self.element_view_backings().view_result(function, place)
    }

    fn reference_edge_exit(&self, target: semantic_vocabulary::BlockId) -> ReferenceEdgeStorage {
        ReferenceEdgeStorage {
            target,
            borrowed_sources: self.borrowed_reference_sources.clone(),
            references: self.references.clone(),
            retired_reference_roots: self.retired_reference_roots.clone(),
        }
    }

    fn finish_reference_edges(&mut self, exits: Vec<ReferenceEdgeStorage>) {
        self.reference_edge_exits = exits;
    }

    fn reference_arrival(
        &self,
        target: semantic_vocabulary::BlockId,
        function: &AbstractFunction,
        storage: &BTreeSet<PlaceId>,
    ) -> Result<ReferenceEdgeStorage, LoweringError> {
        let scoped = |mut exit: ReferenceEdgeStorage| {
            exit.references
                .retain(|(carrier, _), _| storage.contains(carrier));
            exit
        };
        if self.reference_edge_exits.is_empty() {
            return Ok(scoped(self.reference_edge_exit(target)));
        }
        let mut exits = self
            .reference_edge_exits
            .iter()
            .filter(|exit| exit.target == target);
        let mut arrival = scoped(
            exits
                .next()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .clone(),
        );
        for other in exits {
            let other = scoped(other.clone());
            merge_borrowed_arrivals(&mut arrival.borrowed_sources, &other.borrowed_sources);
            references::merge_arrivals(function, &mut arrival.references, &other.references)?;
            arrival
                .retired_reference_roots
                .retain(|root| other.retired_reference_roots.contains(root));
        }
        Ok(arrival)
    }

    fn structural_observation_type(
        &self,
        function: &AbstractFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        if let Some(home) = self.structural_homes.get(&place) {
            return (home.access() != StructuralAccess::WriteOnlyBorrow)
                .then(|| home.structural_type());
        }
        if let Some((_, structural_type)) = self.views.get(&place) {
            return Some(*structural_type);
        }
        let parameter = function
            .arrival_parameter(place)
            .filter(|parameter| parameter.access != StructuralAccess::WriteOnlyBorrow)?;
        let entry = function
            .structural_parameters
            .iter()
            .any(|entry| entry.place == place);
        (entry || self.address_joins.contains(&place) || self.block_views.contains(&place))
            .then_some(parameter.structural_type)
    }
}

pub(crate) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    borrowed_reference_sources: references::BorrowedSources,
    stored_reference_effects: references::StoredEffectCatalog,
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    selected_calls: &crate::selected_boundary_calls::SelectedCallsByCall,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    native_callbacks: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
    installed_interrupts: Option<&provider_admission::AdmittedInstalledInterruptCohort>,
) -> Result<TargetFunction, LoweringError> {
    if function.block_entries.is_empty() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    if !matches!(
        function.result,
        AbstractFunctionResult::Unit
            | AbstractFunctionResult::Scalar(_)
            | AbstractFunctionResult::Structural(_)
    ) || !function.structural_parameters.iter().all(|parameter| {
        (parameter.access != StructuralAccess::Owned
            && matches!(
                parameter.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ))
            || owned_arguments::is_owned_parameter(parameter, structural_types)
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let prepared =
        super::function_signature::prepare_function_signature(function, target, structural_types)?;
    let parameters_by_place = super::function_signature::parameters_by_place(&prepared.parameters);
    let mut definitions = BTreeSet::new();
    for parameter in &function.parameters {
        if !definitions.insert(parameter.value) {
            return Err(LoweringError::DuplicateValue(parameter.value));
        }
    }
    transfers::validate_parameters(function, &mut definitions)?;
    for operation in &function.operations {
        let result = match operation {
            abstract_operations::constant_value_triple_pattern!(result, ..)
            | AbstractOperation::IeeeFloatCompare { result, .. }
            | AbstractOperation::FloatResultPolicy { result, .. }
            | abstract_operations::integer_widen_or_exact_cast_pattern!(result, ..)
            | AbstractOperation::ScalarRepresentationView { result, .. }
            | abstract_operations::integer_equal_less_pair_pattern!(result, ..)
            | AbstractOperation::IntegerLessOrEqual { result, .. }
            | abstract_operations::boolean_not_or_equal_pair_pattern!(result, ..) => Some(*result),
            AbstractOperation::IntegerArithmetic { result, .. }
            | AbstractOperation::TrappingInteger { result, .. }
            | AbstractOperation::IntegerBitwiseAnd { result, .. }
            | abstract_operations::bitwise_or_xor_pair_pattern!(result, ..)
            | AbstractOperation::IntegerBitwiseNot { result, .. }
            | AbstractOperation::IntegerShift { result, .. }
            | AbstractOperation::Call { result, .. } => Some(*result),
            AbstractOperation::ByteSequenceLength { result, .. }
            | AbstractOperation::ByteSequenceRead { result, .. }
            | AbstractOperation::StorageAddress { result, .. }
            | AbstractOperation::PrimitiveScalarRead { result, .. }
            | AbstractOperation::StructuralScalarFieldRead { result, .. }
            | abstract_operations::byte_len_or_field_read_pattern!(result, ..)
            | AbstractOperation::StructuralCaseMembership { result, .. }
            | AbstractOperation::SelectImplementation { result, .. }
            | AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Scalar(result),
                ..
            } => Some(result.value),
            _ => None,
        };
        if let Some(result) = result
            && !definitions.insert(result)
        {
            return Err(LoweringError::DuplicateValue(result));
        }
    }
    let mut places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    if places.len() != function.structural_parameters.len() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let entry_references = references::entry(function, structural_types)?;
    for entry in &function.block_entries {
        for (position, parameter) in entry.structural_parameters.iter().enumerate() {
            if entry.block == function.entry
                || parameter.position as usize != position
                || (!super::scalar::byte_views::is_byte_parameter(parameter, structural_types)
                    && !is_immutable_byte_carrier_parameter(parameter, structural_types)
                    && !is_element_view_parameter(parameter, structural_types)
                    && !transfers::is_address_join(parameter, structural_types)
                    && !is_owned_byte_parameter(parameter, structural_types)
                    && !abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan(
                        parameter,
                        |identity| structural_types.get(identity).copied(),
                        &structural_types
                            .values()
                            .map(|row| (*row).clone())
                            .collect::<Vec<_>>(),
                    )
                    && !owned_arguments::is_owned_parameter(parameter, structural_types))
                || !places.insert(parameter.place)
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
    }
    for operation in &function.operations {
        let established = match operation {
            AbstractOperation::EstablishPrimitiveLocal { result, .. }
            | AbstractOperation::EstablishScalarArray { result, .. }
            | AbstractOperation::EstablishStructuralArray { result, .. }
            | AbstractOperation::EstablishByteSequenceConcatenation { result, .. }
            | AbstractOperation::EstablishNativeCodeLayout { result, .. }
            | AbstractOperation::EstablishRecord { result, .. }
            | AbstractOperation::EstablishZeroedStructural { result, .. }
            | AbstractOperation::EstablishCase { result, .. }
            | AbstractOperation::EstablishReference { result, .. }
            | AbstractOperation::EstablishAccessReference { result, .. }
            | AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | abstract_operations::element_view_or_byte_subslice_pattern!(result, ..)
            | AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some(result.place),
            AbstractOperation::EstablishElementView { destination, .. } => Some(*destination),
            AbstractOperation::EstablishByteSequenceLiteral { place, .. } => Some(place.id),
            _ => None,
        };
        if established.is_some_and(|place| !places.insert(place)) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    let entries = &function.block_entries;
    if entries.is_empty()
        || entries[0].operation_offset != 0
        || entries.iter().any(|entry| {
            entry.block == function.entry
                && !entry.parameters.is_empty()
                && entry.parameters != function.parameters
        })
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut ranges = Vec::new();
    let mut incoming = vec![Vec::new(); entries.len()];
    let mut outgoing = vec![Vec::new(); entries.len()];
    let mut entry_position = None;
    for (position, entry) in entries.iter().enumerate() {
        if entries[..position]
            .iter()
            .any(|earlier| earlier.block == entry.block)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if entry.block == function.entry {
            entry_position = Some(position);
        }
        let end = entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if entry.operation_offset >= end || end > function.operations.len() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        ranges.push(entry.operation_offset..end);
        let targets = match &function.operations[end - 1] {
            AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::InvokeNoNormal { .. }
            | AbstractOperation::ExternalCompletion { .. }
            | AbstractOperation::Crash { .. } => Vec::new(),
            abstract_operations::return_or_return_unit_pattern!(cleanup_actions, ..)
                if cleanup_actions.iter().all(|action| {
                    matches!(
                        action,
                        TerminalAffineCleanupAction::DiscardRoot(_)
                            | TerminalAffineCleanupAction::RetireHome(_)
                            | TerminalAffineCleanupAction::DiscardResidual(_)
                            | TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                }) =>
            {
                Vec::new()
            }
            AbstractOperation::Jump { target, .. } => {
                vec![*target]
            }
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                vec![when_true.target, when_false.target]
            }
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        for target_block in targets {
            let target_position = entries
                .iter()
                .position(|candidate| candidate.block == target_block)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if !outgoing[position].contains(&target_position) {
                outgoing[position].push(target_position);
                incoming[target_position].push(position);
            }
        }
    }
    let entry_position =
        entry_position.ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if !incoming[entry_position].is_empty() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let schedule = dominance::schedule(&incoming, &outgoing, entry_position)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let mut reference_storage = vec![BTreeSet::new(); entries.len()];
    for &(position, dominator) in &schedule {
        let mut storage = match dominator {
            Some(dominator) => {
                let mut storage = reference_storage[dominator].clone();
                storage.extend(
                    function.operations[ranges[dominator].clone()]
                        .iter()
                        .filter_map(|operation| {
                            operation
                                .structural_result()
                                .map(|(_, result)| result.place)
                        }),
                );
                storage
            }
            None => function
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect(),
        };
        storage.extend(
            entries[position]
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place),
        );
        reference_storage[position] = storage;
    }
    let mut live_exits: Vec<Option<LiveDefinitions>> = vec![None; entries.len()];
    let mut reference_entries = vec![None; entries.len()];
    let mut borrowed_entries = vec![None; entries.len()];
    let mut drained_entries = vec![None; entries.len()];
    let mut lowered = vec![None; entries.len()];
    let mut block_provenance = vec![TerminalPsiProvenance::default(); entries.len()];
    let initial = LiveDefinitions {
        borrowed_reference_sources,
        stored_reference_effects,
        reference_position: (entries[entry_position].block, 0),
        access_formals: references::access_formals(function, structural_types)?,
        structural_homes: BTreeMap::new(),
        integers: integer_parameters(function.machine, &prepared.scalar_parameters)?,
        booleans: BTreeMap::new(),
        scalar_homes: ScalarHomes::default(),
        ieee_float_constants: BTreeMap::new(),
        scalar_block_parameters: BTreeMap::new(),
        views: BTreeMap::new(),
        block_views: BTreeSet::new(),
        address_joins: BTreeSet::new(),
        owned_arrivals: function
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
            .map(|parameter| parameter.place)
            .collect(),
        lengths: BTreeMap::new(),
        trivial_affine_locals: BTreeMap::new(),
        references: entry_references.clone(),
        retired_reference_roots: BTreeSet::new(),
        reference_edge_exits: Vec::new(),
    };
    loop {
        let mut changed = false;
        let mut deferred = false;
        'blocks: for &(position, dominator) in &schedule {
            let mut live = match dominator {
                None => initial.clone(),
                Some(dominator) => match live_exits[dominator].as_ref() {
                    Some(exit) => exit.clone(),
                    None => {
                        deferred = true;
                        continue;
                    }
                },
            };
            if position != entry_position {
                let mut predecessors = incoming[position]
                    .iter()
                    .filter_map(|predecessor| live_exits[*predecessor].as_ref());
                let Some(predecessor) = predecessors.next() else {
                    deferred = true;
                    continue;
                };
                let mut arrival = match predecessor.reference_arrival(
                    entries[position].block,
                    function,
                    &reference_storage[position],
                ) {
                    Ok(arrival) => arrival,
                    Err(_) => {
                        deferred = true;
                        continue;
                    }
                };
                for predecessor in predecessors {
                    let other = match predecessor.reference_arrival(
                        entries[position].block,
                        function,
                        &reference_storage[position],
                    ) {
                        Ok(arrival) => arrival,
                        Err(_) => {
                            deferred = true;
                            continue 'blocks;
                        }
                    };
                    if merge_arrivals(function, &mut arrival.references, &other.references).is_err()
                    {
                        deferred = true;
                        continue 'blocks;
                    }
                    references::merge_borrowed_arrivals(
                        &mut arrival.borrowed_sources,
                        &other.borrowed_sources,
                    );
                    arrival
                        .retired_reference_roots
                        .retain(|root| other.retired_reference_roots.contains(root));
                }
                live.references = arrival.references;
                live.borrowed_reference_sources = arrival.borrowed_sources;
                live.retired_reference_roots = arrival.retired_reference_roots;
            }
            live.reference_edge_exits.clear();
            if reference_entries[position].as_ref() == Some(&live.references)
                && borrowed_entries[position].as_ref() == Some(&live.borrowed_reference_sources)
                && drained_entries[position].as_ref() == Some(&live.retired_reference_roots)
            {
                continue;
            }
            changed = true;
            reference_entries[position] = Some(live.references.clone());
            borrowed_entries[position] = Some(live.borrowed_reference_sources.clone());
            drained_entries[position] = Some(live.retired_reference_roots.clone());
            if position != entry_position {
                transfers::enter(&entries[position], &mut live);
                for parameter in
                    entries[position]
                        .structural_parameters
                        .iter()
                        .filter(|parameter| {
                            matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow
                                    | StructuralAccess::MutableBorrow
                                    | StructuralAccess::WriteOnlyBorrow
                            )
                        })
                {
                    if transfers::is_address_join(parameter, structural_types) {
                        live.address_joins.insert(parameter.place);
                    } else {
                        live.block_views.insert(parameter.place);
                    }
                }
                for parameter in entries[position]
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::Owned)
                {
                    let home = block_home(entries[position].block, parameter, structural_types)?;
                    if live
                        .structural_homes
                        .insert(parameter.place, home)
                        .is_some()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                }
                live.owned_arrivals.extend(
                    entries[position]
                        .structural_parameters
                        .iter()
                        .filter(|parameter| parameter.access == StructuralAccess::Owned)
                        .map(|parameter| parameter.place),
                );
            }
            let range = ranges[position].clone();
            let mut operations = Vec::new();
            block_provenance[position] = TerminalPsiProvenance::default();
            let provenance = &mut block_provenance[position];
            for (operation_index, operation) in function.operations[range.start..range.end - 1]
                .iter()
                .enumerate()
            {
                live.reference_position = (
                    entries[position].block,
                    u32::try_from(operation_index)
                        .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
                );
                lower_operation(
                    operation,
                    &mut RowSite::new(
                        function,
                        structural_types,
                        &mut live,
                        &mut operations,
                        provenance,
                    ),
                    target,
                    functions,
                    dynamic_dispatch,
                    boundary_machines,
                    settlements,
                    selected_calls,
                    scalar_abis,
                    native_callbacks,
                    installed_interrupts,
                    &prepared,
                    &parameters_by_place,
                )?;
            }
            live.reference_position = (
                entries[position].block,
                u32::try_from(range.end - range.start - 1)
                    .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
            );
            transfers::validate_successors(
                &function.operations[range.end - 1],
                function,
                &live,
                structural_types,
            )?;
            calls::lower_cleanup_calls(
                &function.operations[range.end - 1],
                function,
                target,
                functions,
                structural_types,
                &prepared,
                &live,
                &mut operations,
            )?;
            let terminator = if matches!(
                &function.operations[range.end - 1],
                AbstractOperation::InvokeNoNormal { .. }
            ) {
                calls::lower_no_normal(
                    &function.operations[range.end - 1],
                    function,
                    target,
                    functions,
                    structural_types,
                    &prepared,
                    &parameters_by_place,
                    &live,
                    &operations,
                    provenance,
                )?
            } else {
                lower_terminator(
                    &function.operations[range.end - 1],
                    function,
                    &prepared,
                    &mut live,
                    structural_types,
                    target,
                    boundary_machines,
                    settlements,
                    selected_calls,
                    provenance,
                )?
            };
            lowered[position] = Some(TargetControlBlock {
                block: entries[position].block,
                structural_parameters: entries[position].structural_parameters.clone(),
                parameters: entries[position]
                    .parameters
                    .iter()
                    .map(|parameter| TargetScalarBlockParameter {
                        value: parameter.value,
                        scalar_type: parameter.scalar_type,
                    })
                    .collect(),
                operations,
                terminator,
            });
            live_exits[position] = Some(live);
        }
        if !changed {
            if deferred {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            break;
        }
    }
    for (position, predecessors) in incoming.iter().enumerate() {
        let mut arrivals = predecessors.iter().map(|predecessor| {
            live_exits[*predecessor]
                .as_ref()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .reference_arrival(
                    entries[position].block,
                    function,
                    &reference_storage[position],
                )
        });
        let Some(first) = arrivals.next() else {
            continue;
        };
        let mut arrival = first?;
        for other in arrivals {
            let other = other?;
            references::merge_arrivals(function, &mut arrival.references, &other.references)?;
            merge_borrowed_arrivals(&mut arrival.borrowed_sources, &other.borrowed_sources);
        }
        if reference_entries[position].as_ref() != Some(&arrival.references)
            || borrowed_entries[position].as_ref() != Some(&arrival.borrowed_sources)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    let blocks = lowered
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let mut provenance = TerminalPsiProvenance::default();
    for block in block_provenance {
        provenance.operations.extend(block.operations);
        provenance.edges.extend(block.edges);
    }
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        nominal_cleanup_subject: function.nominal_cleanup_subject,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance,
        graph: TargetControlGraph {
            structural_types: structural_types.catalog().clone(),
            call_plan: prepared.call_plan.into_call_plan(),
            scalar_parameters: prepared.scalar_parameters,
            parameters: prepared.parameters,
            entry: function.entry,
            blocks,
        },
    })
}
