//! Bind emitted whole-value copies to retained checked formation and operation identity.
use crate::emission::operation_emission::buffer::SourceQuotientAdaptation;
use crate::lowering_error::LoweringError;
use crate::terminal_identities::{lookup_machine_id, lookup_type_id};
use checked_trees::{CheckedQuotientAdaptationLicense, CheckedStructuralValueKind, CheckedTrees};
use terminal_psi::{
    TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense, TerminalQuotientCatalog,
    TerminalQuotientRepresentation,
};

pub(super) fn retain(
    checked: &CheckedTrees,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    emitted: &[SourceQuotientAdaptation],
    calls: &[crate::emission::operation_emission::buffer::SourceQuotientCall],
    machine_ids: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<TerminalQuotientCatalog, LoweringError> {
    let mut catalog = TerminalQuotientCatalog::default();
    // Storage may enter through a parameter or a stored field without a mint
    // in this module. Retain every demanded quotient declaration independently
    // of the executable adaptation roster.
    for (reference, _) in checked.typed.type_reference_table.type_reference_rows() {
        let Some(definition) = checked.typed.closed_data_definition(reference) else {
            continue;
        };
        let Some(quotient) = definition.quotient.as_ref() else {
            continue;
        };
        let identity =
            validation::stored_structural_type_identity(&checked.typed, reference, &[], &[]);
        let Some((_, quotient_id)) = type_ids.iter().find(|(id, _)| *id == identity) else {
            continue;
        };
        if catalog
            .representations
            .iter()
            .any(|row| row.quotient == *quotient_id)
        {
            continue;
        }
        let carrier = lookup_type_id(
            type_ids,
            &validation::stored_structural_type_identity(
                &checked.typed,
                quotient.carrier,
                &[],
                &[],
            ),
        )?;
        let formation = validation::checked_quotient_relation_identity(
            &checked.typed,
            reference,
            &checked.facts.quotient_formations,
        )
        .map_err(|error| LoweringError::InvalidQuotientCorrespondence(vec![error]))?;
        catalog
            .representations
            .push(TerminalQuotientRepresentation {
                quotient_storage_identity: storage_identity(structural_types, *quotient_id)?,
                carrier_storage_identity: storage_identity(structural_types, carrier)?,
                quotient: *quotient_id,
                carrier,
                formation,
            });
    }

    for call in calls {
        let node = checked.facts.values.structural_values.nodes.get(call.value);
        let CheckedStructuralValueKind::QuotientCall {
            request,
            representative_state,
            ..
        } = &node.kind
        else {
            return Err(LoweringError::Unsupported(
                "quotient call occurrence has no checked invocation",
            ));
        };
        let mut rows = checked.facts.quotient_operations.iter().filter(|row| {
            row.request == *request
                && row.public_machine == node.machine
                && row.public_state == node.state
                && row.representative_state == *representative_state
        });
        let row = rows.next().ok_or(LoweringError::Unsupported(
            "quotient call lost its checked correspondence",
        ))?;
        if rows.next().is_some() {
            return Err(LoweringError::Unsupported(
                "quotient call has ambiguous correspondence",
            ));
        }
        let (representative, _) =
            validation::find_machine_by_entry_state(&checked.typed, *representative_state).ok_or(
                LoweringError::Unsupported("quotient representative is absent"),
            )?;
        let theorems = row.theorems.iter().map(|theorem| {
            let (machine, state) = validation::find_machine_by_entry_state(&checked.typed, *theorem)
                .ok_or(LoweringError::Unsupported("quotient theorem declaration is absent"))?;
            if !machine.body_is_present || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !matches!(checked.typed.type_reference_table.type_reference(state.return_type), typed_trees::types::TypeReferenceNode::Unit)
            { return Err(LoweringError::Unsupported("quotient theorem lost bodyful resultless admission")); }
            Ok(terminal_psi::TerminalQuotientTheorem {
                declaration: crate::machine_lowering::declarations::identity(identities, checked, exact_toolchain_sources, machine.symbol)
                    .map_err(LoweringError::Unsupported)?,
                admission: terminal_psi::TerminalQuotientTheoremAdmission::SourceCheckedBodyfulResultless,
            })
        }).collect::<Result<Vec<_>, LoweringError>>()?;
        catalog
            .operations
            .push(terminal_psi::TerminalQuotientOperation {
                public_machine: lookup_machine_id(machine_ids, node.machine)?,
                representative_machine: lookup_machine_id(machine_ids, representative.symbol)?,
                call: call.call,
                correspondence: row.correspondence.clone(),
                theorems,
            });
    }
    for row in emitted {
        let node = checked.facts.values.structural_values.nodes.get(row.value);
        let CheckedStructuralValueKind::QuotientAdaptation {
            quotient, license, ..
        } = node.kind
        else {
            return Err(LoweringError::Unsupported(
                "quotient copy lost its checked adaptation",
            ));
        };
        let formation = validation::checked_quotient_relation_identity(
            &checked.typed,
            quotient,
            &checked.facts.quotient_formations,
        )
        .map_err(|error| LoweringError::InvalidQuotientCorrespondence(vec![error]))?;
        let quotient_id = lookup_type_id(
            type_ids,
            &validation::stored_structural_type_identity(&checked.typed, quotient, &[], &[]),
        )?;
        // A mint and a result adaptation carry the quotient as their result
        // and the carrier as their source; an input adaptation reads the
        // quotient and writes the carrier.
        let (carrier, license) = match license {
            CheckedQuotientAdaptationLicense::Mint => {
                (row.source_type, TerminalQuotientAdaptationLicense::Mint)
            }
            CheckedQuotientAdaptationLicense::Input { request, position } => {
                let operation = operation_index(checked, calls, node.machine, request)?;
                (
                    row.result_type,
                    TerminalQuotientAdaptationLicense::Input {
                        operation,
                        position,
                    },
                )
            }
            CheckedQuotientAdaptationLicense::Result { request } => {
                let operation = operation_index(checked, calls, node.machine, request)?;
                (
                    row.source_type,
                    TerminalQuotientAdaptationLicense::Result { operation },
                )
            }
        };
        let representation = TerminalQuotientRepresentation {
            quotient_storage_identity: storage_identity(structural_types, quotient_id)?,
            carrier_storage_identity: storage_identity(structural_types, carrier)?,
            quotient: quotient_id,
            carrier,
            formation,
        };
        let index = if let Some(index) = catalog
            .representations
            .iter()
            .position(|existing| existing == &representation)
        {
            index
        } else {
            let index = catalog.representations.len();
            catalog.representations.push(representation);
            index
        };
        catalog.adaptations.push(TerminalQuotientAdaptation {
            machine: lookup_machine_id(machine_ids, node.machine)?,
            copy: row.copy,
            source: row.source,
            result: row.result,
            source_type: row.source_type,
            result_type: row.result_type,
            representation: u32::try_from(index).map_err(|_| {
                LoweringError::Unsupported("quotient representation roster overflow")
            })?,
            license,
        });
    }
    Ok(catalog)
}

fn operation_index(
    checked: &CheckedTrees,
    calls: &[crate::emission::operation_emission::buffer::SourceQuotientCall],
    machine: symbols::SymbolHandle,
    request: typed_trees::expression::ExpressionHandle,
) -> Result<u32, LoweringError> {
    let mut matches = calls.iter().enumerate().filter(|(_, call)| {
        let node = checked.facts.values.structural_values.nodes.get(call.value);
        node.machine == machine && matches!(&node.kind,
            CheckedStructuralValueKind::QuotientCall { request: selected, .. } if *selected == request)
    });
    let (index, _) = matches.next().ok_or(LoweringError::Unsupported(
        "quotient adaptation has no emitted call",
    ))?;
    if matches.next().is_some() {
        return Err(LoweringError::Unsupported(
            "quotient adaptation has ambiguous emitted calls",
        ));
    }
    u32::try_from(index)
        .map_err(|_| LoweringError::Unsupported("quotient operation roster overflow"))
}

fn storage_identity(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    id: semantic_vocabulary::StructuralTypeId,
) -> Result<terminal_psi::TerminalIdentityIndex, LoweringError> {
    let mut rows = declarations.iter().filter(|row| row.id == id);
    let row = rows.next().ok_or(LoweringError::Unsupported(
        "quotient storage declaration is absent",
    ))?;
    if rows.next().is_some() {
        return Err(LoweringError::Unsupported(
            "quotient storage declaration is ambiguous",
        ));
    }
    Ok(row.identity)
}
