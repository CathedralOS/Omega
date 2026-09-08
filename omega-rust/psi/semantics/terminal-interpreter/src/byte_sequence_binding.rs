//! Frame-owned byte arguments distinguish immutable values from field loans.

use super::*;

#[derive(Clone)]
pub(super) enum ByteSequenceBinding {
    Immutable(ByteSequenceView),
    MutableField {
        field: StructuralByteSequenceRuntimeField,
        capacity: u64,
        parent_type: StructuralTypeId,
        referent: TerminalStructuralValue,
    },
}

impl ByteSequenceBinding {
    pub(super) fn immutable(&self) -> Result<&ByteSequenceView, TerminalInterpretError> {
        match self {
            Self::Immutable(view) => Ok(view),
            Self::MutableField { .. } => Err(TerminalInterpretError::VerifiedOperationMalformed),
        }
    }

    pub(super) fn validate_mutable_referent(
        &self,
        structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let Self::MutableField {
            field,
            capacity,
            parent_type,
            referent,
        } = self
        else {
            return Err(invalid());
        };
        let Some((StructuralPathSegment::Field(identity), prefix)) = referent.path.split_last()
        else {
            return Err(invalid());
        };
        if value != referent
            || field.parent.opaque_identity != referent.opaque_identity
            || field.parent.path != prefix
            || !referent.qualifications.is_empty()
        {
            return Err(invalid());
        }
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record { fields },
            ..
        }) = structural_types.get(parent_type)
        else {
            return Err(invalid());
        };
        if !fields.iter().any(|declaration| {
            declaration.id == field.field
                && declaration.identity == *identity
                && !declaration.relevance.is_erased()
                && declaration.field_type
                    == terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned {
                            capacity: *capacity,
                        },
                    )
        }) {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(super) struct StructuralCallArguments {
    pub(super) values: Vec<TerminalStructuralValue>,
    pub(super) byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
}

impl TerminalExecution {
    pub(super) fn prepare_structural_call_arguments(
        &self,
        callee: MachineId,
        arguments: &[StructuralArgument],
    ) -> Result<StructuralCallArguments, TerminalInterpretError> {
        let machine = self
            .machines
            .get(&callee)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee))?;
        if machine.result == TerminalMachineResult::Unit
            && machine
                .structural_parameters
                .iter()
                .zip(arguments)
                .any(|(parameter, argument)| {
                    !argument.path.is_empty()
                        && parameter.access == StructuralAccess::MutableBorrow
                        && self
                            .structural_types
                            .get(&parameter.structural_type)
                            .is_some_and(|declaration| {
                                declaration.shape
                                    == StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                                    )
                            })
                })
        {
            // Share only exact referent preparation. External buffer staging and
            // replacement are not part of an ordinary Unit call.
            return self
                .prepare_boundary_arguments(&machine.structural_parameters, arguments)?
                .into_call_arguments(&machine.structural_parameters);
        }
        let values = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            arguments,
        )?;
        let byte_sequences =
            self.bind_byte_sequence_arguments(&machine.structural_parameters, arguments, &values)?;
        Ok(StructuralCallArguments {
            values,
            byte_sequences,
        })
    }
}
