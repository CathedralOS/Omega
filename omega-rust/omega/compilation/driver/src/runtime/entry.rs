//! Actual host-owned entry backing, not values invented from a parameter list.

use compilation_products::TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalEstablishedErasedField, TerminalProgram, TerminalStructuralInputs,
    TerminalStructuralValue,
};
use terminal_psi::CheckedProgramEntryReceiverProjection;

use super::TerminalRuntimeError;

pub(super) struct EstablishedEntry {
    arguments: Vec<TerminalStructuralValue>,
    zero_initialized: Vec<u32>,
    erased_fields: Vec<TerminalEstablishedErasedField>,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
    establishments: Vec<terminal_installation::TerminalPlacedViewEstablishment>,
}

impl EstablishedEntry {
    pub(super) fn inputs(&self) -> TerminalStructuralInputs<'_> {
        TerminalStructuralInputs {
            arguments: &self.arguments,
            zero_initialized_arguments: &self.zero_initialized,
            established_erased_fields: &self.erased_fields,
            private_entry_ram: &self.private_entry_ram,
            placed_view_establishments: &self.establishments,
            ..TerminalStructuralInputs::default()
        }
    }
}

pub(super) fn establish(
    program: &TerminalProgram,
    installation: &terminal_interpreter::TerminalRuntimeSupplies,
    provisions: Option<&TerminalRuntimeProvisions>,
) -> Result<EstablishedEntry, TerminalRuntimeError> {
    let entry = program
        .entry_installed(installation)
        .map_err(TerminalRuntimeError::Requirements)?;
    let recipes = provisions
        .map(TerminalRuntimeProvisions::private_entry_ram)
        .unwrap_or_default();
    if !entry.scalar_parameters.is_empty() || entry.placed_view_inputs().count() != recipes.len() {
        return Err(TerminalRuntimeError::missing(
            "program-entry-arrival-values",
            "isolated hosted entry does not supply scalar arguments or borrowed placed-view inputs",
        ));
    }
    if provisions.is_some_and(|provisions| {
        provisions.target_slot().schema != target::ProgramEntrySchema::HostedApplication
    }) {
        return Err(TerminalRuntimeError::missing(
            "program-entry-arrival-values",
            "this isolated runtime establishes a hosted application root, not freestanding image/storage arrivals",
        ));
    }
    let mut arrival = EstablishedEntry {
        arguments: Vec::new(),
        zero_initialized: Vec::new(),
        erased_fields: Vec::new(),
        private_entry_ram: recipes.to_vec(),
        establishments: Vec::new(),
    };
    let receiver = provisions.and_then(TerminalRuntimeProvisions::receiver);
    let context_position = receiver
        .and_then(|receiver| match receiver.projection() {
            CheckedProgramEntryReceiverProjection::Retained { terminal_self, .. } => entry
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == terminal_self),
            _ => None,
        })
        .unwrap_or(0) as u32;
    if let Some(receiver) = receiver {
        let identities = &program.module().identities;
        let Some(provisions) = provisions else {
            return Err(TerminalRuntimeError::InvalidProvision(
                "receiver lost its root provisions",
            ));
        };
        let plan_identities = provisions.plan_identities();
        for field in receiver.fused_service_fields() {
            let mut rows = provisions.fused_service_slots().filter(|(row, _)| {
                row.field_path().len() == field.field_path().len()
                    && row.field_path().iter().zip(field.field_path()).all(
                        |(row_segment, field_segment)| {
                            identities.text(*field_segment) == plan_identities.text(*row_segment)
                        },
                    )
                    && plan_identities.text(row.carrier_type_identity())
                        == identities.text(field.carrier_type_identity())
            });
            let Some((row, _)) = rows.next() else {
                return Err(TerminalRuntimeError::InvalidProvision(
                    "root Binding field lost its admitted establishment",
                ));
            };
            if rows.next().is_some()
                || row.source_signature_identity().bytes() != provisions.source_signature_identity()
                || row.target_slot() != provisions.target_slot()
            {
                return Err(TerminalRuntimeError::InvalidProvision(
                    "root Binding occurrence differs from its actual entry",
                ));
            }
            // Establish the exact erased service occurrence. Its zero payload
            // is not a grant of containing-process-domain authority; the host
            // delegates that separately for the actual runtime instance.
            if matches!(
                receiver.projection(),
                CheckedProgramEntryReceiverProjection::Retained { .. }
            ) {
                arrival.erased_fields.push(TerminalEstablishedErasedField {
                    argument_index: context_position,
                    field_path: field
                        .field_path()
                        .iter()
                        .map(|segment| identities.text(*segment).unwrap_or_default().to_string())
                        .collect(),
                    carrier_type_identity: identities
                        .text(field.carrier_type_identity())
                        .unwrap_or_default()
                        .to_string(),
                });
            }
        }
        if provisions.fused_services().len() != receiver.fused_service_fields().len() {
            return Err(TerminalRuntimeError::InvalidProvision(
                "runtime root has extraneous Binding establishments",
            ));
        }
        if matches!(
            receiver.projection(),
            CheckedProgramEntryReceiverProjection::Retained { .. }
        ) {
            for field in receiver.determined_evidence_fields() {
                arrival.erased_fields.push(TerminalEstablishedErasedField {
                    argument_index: context_position,
                    field_path: field
                        .field_path()
                        .iter()
                        .map(|segment| identities.text(*segment).unwrap_or_default().to_string())
                        .collect(),
                    carrier_type_identity: identities
                        .text(field.carrier_type_identity())
                        .unwrap_or_default()
                        .to_string(),
                });
            }
        }
    }
    match receiver.map(|receiver| receiver.projection()) {
        None | Some(CheckedProgramEntryReceiverProjection::Erased { .. }) => {
            if !entry.structural_parameters.is_empty() {
                return Err(TerminalRuntimeError::missing(
                    "program-entry-arrival-values",
                    "retained structural parameters have no admitted hosted receiver establishment",
                ));
            }
        }
        Some(CheckedProgramEntryReceiverProjection::Retained { terminal_self, .. }) => {
            let Some(parameter) = entry
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == terminal_self)
            else {
                return Err(TerminalRuntimeError::InvalidProvision(
                    "hosted receiver projection does not name the whole entry arrival",
                ));
            };
            let Some(receiver) = receiver else {
                return Err(TerminalRuntimeError::InvalidProvision(
                    "retained receiver custody disappeared",
                ));
            };
            if parameter.place != terminal_self
                || parameter.structural_type != receiver.terminal_receiver_type()
            {
                return Err(TerminalRuntimeError::InvalidProvision(
                    "hosted receiver differs from its admitted Terminal projection",
                ));
            }
            if !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(TerminalRuntimeError::missing(
                    "program-entry-arrival-qualifications",
                    "the root has no actual runtime facts for required receiver qualifications",
                ));
            }
            // This creates owned interpreter storage. The nonzero identity is
            // local to this fresh instance, never an address or a host loan.
            arrival.arguments.push(TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: receiver.terminal_receiver_type(),
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                path: Vec::new(),
            });
            arrival.zero_initialized.push(context_position);
        }
    }
    let context = arrival.arguments.pop();
    for (ordinal, parameter) in entry.structural_parameters.iter().enumerate() {
        if ordinal == context_position as usize {
            if let Some(context) = context.as_ref() {
                arrival.arguments.push(context.clone());
                continue;
            }
        }
        let index = recipes
            .iter()
            .position(|recipe| recipe.input().runtime_position as usize == ordinal)
            .ok_or(TerminalRuntimeError::InvalidProvision(
                "entry parameter has no checked private RAM construction",
            ))?;
        let recipe = &recipes[index];
        if recipe.terminal() != program.terminal_psi()
            || recipe.input().machine
                != program
                    .entry_installed(installation)
                    .map_err(TerminalRuntimeError::Requirements)?
                    .machine
            || recipe.correspondence().carrier != parameter.structural_type
        {
            return Err(TerminalRuntimeError::InvalidProvision(
                "private entry RAM recipe differs from its actual invocation",
            ));
        }
        let (original_identity, descriptor_identity) =
            terminal_installation::private_entry_ram_identity_pair(program.module(), index).ok_or(
                TerminalRuntimeError::InvalidProvision(
                    "private entry RAM identity namespace is exhausted",
                ),
            )?;
        arrival.arguments.push(TerminalStructuralValue::minted(
            descriptor_identity,
            parameter.structural_type,
            Vec::new(),
        ));
        arrival
            .establishments
            .push(terminal_installation::TerminalPlacedViewEstablishment {
                input: recipe.input().clone(),
                referent: TerminalStructuralValue::minted(
                    original_identity,
                    recipe.schema(),
                    Vec::new(),
                ),
            });
    }
    Ok(arrival)
}
