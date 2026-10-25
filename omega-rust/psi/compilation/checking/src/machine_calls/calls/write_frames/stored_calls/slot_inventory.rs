//! Declared storage supplies the complete writable-slot inventory; unknown is not empty.
//! Completed operands retain their exact source cuts and carrier/backing association.

use super::super as frames;
use super::*;

pub(super) fn destination_inventory(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Option<Vec<SlotInventory>> {
    let inventory = parameter_destination_inventory(program, program.state_parameters(state))?;
    (!inventory.template_dependent).then_some(inventory.slots)
}

/// Form the output namespace from an authenticated invocation occurrence.
/// This carries no spatial relation or permission by itself.
/// Opaque calls with possible reference outputs require a complete call-site
/// relation even when failed attribution produced no loans. This is an
/// obligation query, not provider admission and not an empty-summary fallback.
pub(in crate::machine_calls::calls::write_frames) fn provider_output_relation_required(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    target: symbols::SymbolHandle,
    call_expression: ExpressionHandle,
) -> bool {
    let parameters = if let Some((definition, signature)) =
        crate::selected_requirement_signature(program, target)
        && definition.is_boundary
    {
        let signature = if call_expression.is_valid() {
            let Some(call) = program.expression_table.call_node(call_expression) else {
                return true;
            };
            if call.target_symbol != target {
                return true;
            }
            let Some(signature) = program.requirement_call_signature(machine, call) else {
                return true;
            };
            signature
        } else {
            signature
        };
        program.state_signature_parameters(signature)
    } else if let Some(state) = crate::call_target_state(program, target)
        && let Some(machine) = program.machine_holding_state(state.symbol)
        && matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::Boundary
                | language_semantics::MachineSupplyMode::AdmissionClaim
                | language_semantics::MachineSupplyMode::ExternalRealization { .. }
        )
    {
        program.state_parameters(state)
    } else {
        return false;
    };
    // A symbolic-only frontier supplies no concrete destination relation yet.
    // Its transfer remains incomplete and invalidates possible origins; it
    // never becomes an empty preserving effect. Known slots still require a
    // relation beside symbolic leaves, and an uncomputable inventory fails
    // closed. Exact applied parameters above prevent concrete calls from
    // borrowing this deferral from an open declaration.
    parameter_destination_inventory(program, parameters)
        .is_none_or(|inventory| !inventory.slots.is_empty())
}

pub(super) fn parameter_destination_inventory(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
) -> Option<DestinationInventory> {
    let mut leaves = Vec::new();
    let mut template_dependent = false;
    for parameter in parameters {
        let shell = crate::reference_shell(program, parameter.type_reference);
        if shell.is_some_and(|shell| !shell.access.is_exclusive()) {
            continue;
        }
        let reference = shell.map_or(parameter.type_reference, |shell| shell.referee);
        let mut pending = vec![(
            reference,
            Vec::new(),
            shell.is_some(),
            shell.is_none_or(|shell| shell.access.is_readable()),
            Vec::new(),
            Vec::<Vec<PlaceSegment>>::new(),
        )];
        while let Some((reference, prefix, borrowed, readable, mut visited, ancestors)) =
            pending.pop()
        {
            // A recursive pointee needs demand-bounded captured destinations;
            // do not turn an incomplete reachable inventory into an empty one.
            if visited.contains(&reference) {
                return None;
            }
            visited.push(reference);
            let inventory = stored_origins::declared_origins_for_query(
                program,
                parameter.symbol,
                parameter.name.as_str(),
                reference,
                true,
            )?;
            for mut leaf in inventory.references {
                let leaf_type = stored_origins::projected_storage_type(
                    program,
                    reference,
                    &leaf.local_segments,
                )?;
                let Some(shell) = crate::reference_shell(program, leaf_type) else {
                    let leaf_type = program
                        .type_reference_table
                        .unconstrained_type_reference(leaf_type);
                    if matches!(program.type_reference_table.type_reference(leaf_type),
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                        if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter)
                    {
                        template_dependent = true;
                        continue;
                    }
                    let (definition, _) = frames::type_instantiation::DeclarationScopes::default()
                        .definition(program, leaf_type, 0)?;
                    if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
                    {
                        return None;
                    }
                    continue;
                };
                let mut segments = prefix.clone();
                segments.extend_from_slice(&leaf.local_segments);
                leaf.local_segments = segments.clone();
                leaf.local_path = frames::place_paths::coordinate_storage_path(
                    program,
                    parameter.name.as_str(),
                    &segments,
                )
                .0;
                if borrowed {
                    leaves.push(SlotInventory {
                        origin: leaf,
                        access: shell.access,
                        ancestors: ancestors.clone(),
                    });
                }
                // A pointer under write-only storage cannot be loaded to
                // reach another destination. An owned/readable carrier can
                // lend the storage behind its exclusive pointer fields.
                if readable && shell.access.is_exclusive() {
                    let mut child_ancestors = ancestors.clone();
                    child_ancestors.push(segments.clone());
                    pending.push((
                        shell.referee,
                        segments,
                        true,
                        shell.access.is_readable(),
                        visited.clone(),
                        child_ancestors,
                    ));
                }
            }
        }
    }
    Some(DestinationInventory {
        slots: leaves,
        template_dependent,
    })
}

/// Both source-call effects and portable boundary publication require the
/// complete writable frontier. A value containing an exclusive pointer can
/// expose output storage even without an outer reference parameter.
pub(crate) fn provider_completion_covers_writable_slots(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    outputs: &[crate::borrow_contracts::ProviderOutput],
) -> Option<crate::borrow_contracts::BorrowRelationReadiness> {
    let inventory = parameter_destination_inventory(program, parameters)?;
    if !inventory.slots.iter().all(|slot| {
        outputs.iter().any(|output| {
            let crate::borrow_contracts::ProviderOutputDestination::Parameter(position) =
                output.destination;
            parameters
                .get(position)
                .is_some_and(|parameter| parameter.symbol == slot.origin.local_symbol)
                && opaque_outputs::owner_segments(&output.owner_path).as_ref()
                    == Some(&slot.origin.local_segments)
                && output.access == slot.access
        })
    }) {
        return None;
    }
    Some(if inventory.template_dependent {
        crate::borrow_contracts::BorrowRelationReadiness::TemplateDependent
    } else {
        crate::borrow_contracts::BorrowRelationReadiness::Complete
    })
}

/// Seed the incoming pointee slots before the ordinary transfer walk. Missing
/// rows after that walk are unknown, never evidence that a slot was untouched.
/// Unsupported reachable inventories add no knowledge; any demanded summary
/// still fails in `destination_inventory`/`collect_exit`.
pub(in crate::machine_calls::calls::write_frames) fn seed_incoming_slots(
    program: &dyn crate::CheckingInput,
    state: &State,
    stored: &mut [StoredLocalOrigins],
) {
    let Some(inventory) = destination_inventory(program, state) else {
        return;
    };
    for declaration in inventory {
        if declaration.ancestors.is_empty() {
            continue;
        }
        let mut slot = declaration.origin;
        let Some(record) = stored
            .iter_mut()
            .find(|record| record.local_symbol == slot.local_symbol)
        else {
            continue;
        };
        if record
            .references
            .iter()
            .any(|leaf| leaf.local_segments == slot.local_segments)
        {
            continue;
        }
        slot.origin = Some(FramePlaceOrigin {
            path: slot.local_path.clone(),
            precision: FramePathPrecision::Exact,
            source: FrameSourcePlace {
                root: slot.local_symbol,
                segments: slot.local_segments.clone(),
                builtin_coordinates: true,
                ..Default::default()
            },
        });
        record.references.push(slot);
    }
}
