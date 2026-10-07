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
    established_slots: Vec<semantic_vocabulary::BoundaryMachineId>,
}

impl EstablishedEntry {
    pub(super) fn inputs(&self) -> TerminalStructuralInputs<'_> {
        TerminalStructuralInputs {
            arguments: &self.arguments,
            zero_initialized_arguments: &self.zero_initialized,
            established_erased_fields: &self.erased_fields,
            ..TerminalStructuralInputs::default()
        }
    }

    pub(super) fn established_slots(&self) -> &[semantic_vocabulary::BoundaryMachineId] {
        &self.established_slots
    }
}

pub(super) fn establish(
    program: &TerminalProgram,
    provisions: Option<&TerminalRuntimeProvisions>,
) -> Result<EstablishedEntry, TerminalRuntimeError> {
    let entry = program.entry();
    if !entry.scalar_parameters.is_empty() || !entry.placed_view_inputs.is_empty() {
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
        established_slots: Vec::new(),
    };
    let receiver = provisions.and_then(TerminalRuntimeProvisions::receiver);
    if let Some(receiver) = receiver {
        let Some(provisions) = provisions else {
            return Err(TerminalRuntimeError::InvalidProvision(
                "receiver lost its root provisions",
            ));
        };
        for field in receiver.fused_service_fields() {
            let mut rows = provisions.fused_service_slots().filter(|(row, _)| {
                row.field_path() == field.field_path()
                    && row.carrier_type_identity() == field.carrier_type_identity()
            });
            let Some((row, slots)) = rows.next() else {
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
            // The driver establishes all of this root's erased capabilities
            // for its own isolated domain. These slots are retained even when
            // the whole receiver has no runtime data projection.
            arrival.established_slots.extend_from_slice(slots);
            if matches!(
                receiver.projection(),
                CheckedProgramEntryReceiverProjection::Retained { .. }
            ) {
                arrival.erased_fields.push(TerminalEstablishedErasedField {
                    argument_index: 0,
                    field_path: field.field_path().to_vec(),
                    carrier_type_identity: field.carrier_type_identity().to_owned(),
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
                    argument_index: 0,
                    field_path: field.field_path().to_vec(),
                    carrier_type_identity: field.carrier_type_identity().to_owned(),
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
            let [parameter] = entry.structural_parameters else {
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
                path: Vec::new(),
            });
            arrival.zero_initialized.push(0);
        }
    }
    arrival.established_slots.sort_unstable();
    arrival.established_slots.dedup();
    Ok(arrival)
}
