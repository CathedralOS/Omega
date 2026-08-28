//! Transactional insertion of compiler-private callback address stores.

use omega_abstract_operations::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, BoundaryFootprintFragment,
    BoundaryFootprintFragmentOrigin, PermissionRealizationCandidateKind,
};
use omega_backend_plan::{
    CallbackRegistrarAssignedOperandBinding, CallbackRegistrarPhysicalDestinationKind,
};
use omega_calling_conventions::{CallSignature, compose_state_footprints};
use psi_arena::Arena;
use psi_diagnostics::Diagnostic;
use std::collections::BTreeMap;

pub(super) fn insert_callback_private_address_store_operations(
    plan: &mut omega_abstract_operations::AbstractOperationPlan,
    bindings: &[CallbackRegistrarAssignedOperandBinding],
    entry_boundary_plan: Option<&omega_calling_conventions::BoundaryEntryPlan>,
) -> Result<(), Diagnostic> {
    if bindings.is_empty() {
        return Ok(());
    }
    let original = plan.clone();
    let mut instructions = Arena::with_capacity(original.code.instructions.len() + bindings.len());
    let mut functions = Arena::with_capacity(original.code.functions.len());
    let mut old_to_new = BTreeMap::new();
    let mut inserted_kinds = Vec::with_capacity(bindings.len());
    let mut inserted_count = 0usize;

    for (_, function) in original.code.functions.iter() {
        let old_instructions = original
            .code
            .instructions
            .span(function.instructions)
            .ok_or_else(|| {
                Diagnostic::error("callback store insertion found an invalid function span")
            })?;
        let mut rebuilt = Vec::with_capacity(old_instructions.len() + bindings.len());
        for (relative_index, instruction) in old_instructions.iter().enumerate() {
            let old_index = function.instructions.start().arena_index()
                + u32::try_from(relative_index).map_err(|_| {
                    Diagnostic::error("callback store instruction index overflowed")
                })?;
            let old_handle = psi_arena::Handle::from_parts(
                old_index,
                function.instructions.start().generation(),
            );
            let anchored = bindings
                .iter()
                .enumerate()
                .filter(|(_, binding)| binding.abstract_instruction == old_handle)
                .collect::<Vec<_>>();
            if !anchored.is_empty()
                && !matches!(
                    instruction.kind,
                    AbstractOperationKind::HostOperation {
                        provenance: Some(_),
                        ..
                    }
                )
            {
                return Err(Diagnostic::error(
                    "callback address store lost its exact registrar host-operation anchor",
                ));
            }
            for (binding_index, binding) in anchored {
                let CallbackRegistrarPhysicalDestinationKind::Field { layout_demand, .. } =
                    &binding.destination.kind
                else {
                    return Err(store_error(
                        binding_index,
                        "direct parameter remains fenced",
                    ));
                };
                let omega_target_operations::TargetInstructionOperandKind::RuntimeStorageAddress {
                    region,
                    byte_offset,
                } = binding.target_operand.kind
                else {
                    return Err(store_error(
                        binding_index,
                        "operand is not RuntimeStorageAddress",
                    ));
                };
                let target_offset = byte_offset
                    .checked_add(layout_demand.offset)
                    .ok_or_else(|| store_error(binding_index, "destination offset overflowed"))?;
                let kind = AbstractOperationKind::WriteFunctionAddressToRuntimeStorage {
                    function: binding.destination.binding.demand.function_identity,
                    target_region: region,
                    target_offset,
                };
                inserted_kinds.push(kind.clone());
                rebuilt.push(AbstractOperation {
                    kind,
                    source_key: instruction.source_key,
                    source_statement: instruction.source_statement,
                });
                inserted_count += 1;
            }
            old_to_new.insert(
                old_index,
                u32::try_from(instructions.len() + rebuilt.len() + 1)
                    .map_err(|_| Diagnostic::error("callback store remap index overflowed"))?,
            );
            rebuilt.push(instruction.clone());
        }
        let span = instructions.insert_many(rebuilt);
        functions.insert(AbstractFunctionPlan {
            symbol: function.symbol.clone(),
            identity: function.identity,
            instructions: span,
        });
    }
    if inserted_count != bindings.len() {
        return Err(Diagnostic::error(format!(
            "callback address store insertion placed {inserted_count} row(s), expected {}",
            bindings.len()
        )));
    }

    let mut candidates = original.permission_realization_candidates.clone();
    for candidate in &mut candidates {
        if let PermissionRealizationCandidateKind::SelectedInstruction { instruction_index } =
            &mut candidate.kind
        {
            let new_index = *old_to_new.get(instruction_index).ok_or_else(|| {
                Diagnostic::error("callback store insertion lost permission instruction remap")
            })?;
            *instruction_index = new_index;
        }
    }

    let mut candidate = original;
    candidate.code.instructions = instructions;
    candidate.code.functions = functions;
    candidate.permission_realization_candidates = candidates;
    candidate
        .semantics
        .ownership
        .install_permission_realization_candidates(
            &candidate.permission_realization_candidates,
            candidate.code.instructions.len(),
        )
        .map_err(|error| {
            Diagnostic::error(format!(
                "callback store insertion could not replay permission realizations: {error:?}"
            ))
        })?;
    extend_address_store_footprint(&mut candidate, entry_boundary_plan, &inserted_kinds)?;
    *plan = candidate;
    Ok(())
}

fn extend_address_store_footprint(
    plan: &mut omega_abstract_operations::AbstractOperationPlan,
    entry_boundary_plan: Option<&omega_calling_conventions::BoundaryEntryPlan>,
    inserted: &[AbstractOperationKind],
) -> Result<(), Diagnostic> {
    let boundary_plan = entry_boundary_plan.cloned().ok_or_else(|| {
        Diagnostic::error(
            "callback address stores require an exact validated root boundary footprint plan",
        )
    })?;
    let signature = CallSignature {
        parameters: boundary_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: boundary_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let boundary =
        omega_calling_conventions::validate_boundary_entry_plan(boundary_plan, &signature)
            .map_err(|error| Diagnostic::error(error.0))?;
    let added =
        omega_instruction_selection::derive_boundary_compiler_body_place_address_write_footprint(
            &boundary,
            inserted.iter(),
        )
        .map_err(|error| Diagnostic::error(error.0))?;
    let footprints = &mut plan.semantics.boundaries.footprints;
    if let Some(fragment) = footprints.fragments.iter_mut().find(|fragment| {
        fragment.origin == BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite
    }) {
        let composed = compose_state_footprints([&fragment.evidence, &added]);
        omega_calling_conventions::validate_state_footprint(&boundary, &composed)
            .map_err(|error| Diagnostic::error(error.0))?;
        fragment.evidence = composed;
    } else {
        footprints
            .retain_validated_fragment(
                &boundary,
                BoundaryFootprintFragment {
                    origin: BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
                    evidence: added,
                },
            )
            .map_err(|error| Diagnostic::error(error.0))?;
    }
    Ok(())
}

fn store_error(index: usize, message: &str) -> Diagnostic {
    Diagnostic::error(format!("callback address store {index}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callback_registrar_assigned_operands::tests::{
        build_target, fixture, nested_fixture, plan as plan_assigned, shared_root_fixture,
        with_formal_operand_kind,
    };
    use omega_abstract_operations::{
        AbstractPermissionEvent, InstructionOperandKind, PermissionRealizationCandidate,
        PermissionRealizationCandidateKind, RuntimeStorageRegion,
    };
    use omega_target::NativeTarget;

    fn boundary(
        fixture: &crate::callback_registrar_assigned_operands::tests::Fixture,
    ) -> omega_calling_conventions::BoundaryEntryPlan {
        let mut boundary = fixture.placements[0]
            .private_materialization
            .as_ref()
            .unwrap()
            .registrar_boundary_entry_plan
            .clone();
        boundary.call.callback_materializations.clear();
        boundary
    }

    fn prepared(
        fixture: crate::callback_registrar_assigned_operands::tests::Fixture,
    ) -> (
        crate::callback_registrar_assigned_operands::tests::Fixture,
        std::sync::Arc<[CallbackRegistrarAssignedOperandBinding]>,
    ) {
        with_formal_operand_kind(
            fixture,
            InstructionOperandKind::RuntimeStorageAddress {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 32,
            },
        )
    }

    #[test]
    fn inserts_exact_contiguous_stores_before_registrar_and_rederives_handles() {
        let (mut prepared_fixture, prior) = prepared(shared_root_fixture());
        let original_registrar = prior[0].abstract_instruction;
        prepared_fixture
            .abstract_operations
            .semantics
            .ownership
            .permissions
            .insert(AbstractPermissionEvent {
                source_event_index: 71,
                ..AbstractPermissionEvent::default()
            });
        prepared_fixture
            .abstract_operations
            .permission_realization_candidates
            .push(PermissionRealizationCandidate {
                source_event_index: 71,
                kind: PermissionRealizationCandidateKind::SelectedInstruction {
                    instruction_index: original_registrar.arena_index(),
                },
            });
        let root_boundary = boundary(&prepared_fixture);
        insert_callback_private_address_store_operations(
            &mut prepared_fixture.abstract_operations,
            &prior,
            Some(&root_boundary),
        )
        .unwrap();

        let registrar_index = original_registrar.arena_index() + 2;
        for (ordinal, binding) in prior.iter().enumerate() {
            let handle = psi_arena::Handle::from_parts(
                original_registrar.arena_index() + u32::try_from(ordinal).unwrap(),
                original_registrar.generation(),
            );
            let operation = prepared_fixture
                .abstract_operations
                .code
                .instructions
                .get(handle);
            let CallbackRegistrarPhysicalDestinationKind::Field { layout_demand, .. } =
                &binding.destination.kind
            else {
                unreachable!()
            };
            assert!(matches!(
                operation.kind,
                AbstractOperationKind::WriteFunctionAddressToRuntimeStorage {
                    function,
                    target_region: RuntimeStorageRegion::Machine,
                    target_offset,
                } if function == binding.destination.binding.demand.function_identity
                    && target_offset == 32 + layout_demand.offset
            ));
        }
        let registrar =
            psi_arena::Handle::from_parts(registrar_index, original_registrar.generation());
        assert!(matches!(
            prepared_fixture
                .abstract_operations
                .code
                .instructions
                .get(registrar)
                .kind,
            AbstractOperationKind::HostOperation { .. }
        ));
        assert_eq!(
            prepared_fixture
                .abstract_operations
                .permission_realization_candidates[0]
                .kind,
            PermissionRealizationCandidateKind::SelectedInstruction {
                instruction_index: registrar_index,
            }
        );
        assert_eq!(
            prepared_fixture
                .abstract_operations
                .semantics
                .ownership
                .realization_instruction_indices
                .iter()
                .map(|(_, index)| *index)
                .collect::<Vec<_>>(),
            vec![registrar_index]
        );

        prepared_fixture.target_operations = build_target(
            &prepared_fixture,
            NativeTarget::windows_x64(),
            &prepared_fixture.abstract_operations,
        )
        .unwrap();
        prepared_fixture.assigned_operations =
            omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
                &prepared_fixture.target_operations,
            );
        let rebound = plan_assigned(&prepared_fixture);
        assert_eq!(
            rebound[0].abstract_instruction.arena_index(),
            registrar_index
        );
        assert_eq!(
            rebound[1].abstract_instruction,
            rebound[0].abstract_instruction
        );
    }

    #[test]
    fn rejects_stale_anchor_missing_footprint_and_operand_shape_transactionally() {
        let (mut prepared_fixture, prior) = prepared(fixture(1));
        let original = prepared_fixture.abstract_operations.clone();
        let root_boundary = boundary(&prepared_fixture);
        let mut stale = prior.to_vec();
        stale[0].abstract_instruction = psi_arena::Handle::from_parts(
            stale[0].abstract_instruction.arena_index(),
            stale[0].abstract_instruction.generation() + 1,
        );
        assert!(
            insert_callback_private_address_store_operations(
                &mut prepared_fixture.abstract_operations,
                &stale,
                Some(&root_boundary),
            )
            .is_err()
        );
        assert_eq!(prepared_fixture.abstract_operations, original);
        assert!(
            insert_callback_private_address_store_operations(
                &mut prepared_fixture.abstract_operations,
                &prior,
                None,
            )
            .is_err()
        );
        assert_eq!(prepared_fixture.abstract_operations, original);

        let (mut wrong, bindings) = with_formal_operand_kind(
            fixture(1),
            InstructionOperandKind::DataAddress {
                data: psi_arena::Handle::invalid(),
            },
        );
        let before = wrong.abstract_operations.clone();
        let wrong_boundary = boundary(&wrong);
        assert!(
            insert_callback_private_address_store_operations(
                &mut wrong.abstract_operations,
                &bindings,
                Some(&wrong_boundary),
            )
            .is_err()
        );
        assert_eq!(wrong.abstract_operations, before);
    }

    #[test]
    fn nested_field_remains_fenced_before_address_store_insertion() {
        let (mut fixture, bindings) = prepared(nested_fixture());
        let original = fixture.abstract_operations.clone();
        let boundary = boundary(&fixture);
        assert!(
            insert_callback_private_address_store_operations(
                &mut fixture.abstract_operations,
                &bindings,
                Some(&boundary),
            )
            .is_err()
        );
        assert_eq!(fixture.abstract_operations, original);
    }
}
