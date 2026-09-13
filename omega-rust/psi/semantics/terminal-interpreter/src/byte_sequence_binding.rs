//! Frame-owned byte arguments distinguish immutable values from field loans.

use super::*;

#[derive(Clone)]
pub(super) enum ByteSequenceBinding {
    Immutable(ByteSequenceView),
    MutableArray {
        array: StructuralRuntimePlace,
        array_type: StructuralTypeId,
        referent: TerminalStructuralValue,
    },
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
            Self::MutableField { .. } | Self::MutableArray { .. } => {
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            }
        }
    }

    pub(super) fn validate_mutable_referent(
        &self,
        structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if let Self::MutableArray {
            array,
            array_type,
            referent,
        } = self
        {
            return if value == referent
                && *array == StructuralRuntimePlace::from(referent)
                && referent.qualifications.is_empty()
                && structural_byte_arrays::byte_array_length(structural_types, *array_type)
                    .is_some()
            {
                Ok(())
            } else {
                Err(invalid())
            };
        }
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
    pub(super) values: BTreeMap<PlaceId, TerminalStructuralValue>,
    pub(super) byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    pub(super) scalar_arrays: BTreeMap<PlaceId, TerminalScalarArrayValue>,
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
        if machine.structural_parameters.len() != arguments.len() {
            return Err(TerminalInterpretError::StructuralArgumentCount {
                expected: machine.structural_parameters.len(),
                actual: arguments.len(),
            });
        }
        // Bind payload-bearing arrays directly to callee places. The remaining
        // arguments use the existing opaque/borrowed-backing preparation; their
        // filtered positions must never be rebound against the full signature.
        let mut scalar_arrays = BTreeMap::new();
        let mut opaque_parameters = Vec::new();
        let mut opaque_arguments = Vec::new();
        for (parameter, argument) in machine.structural_parameters.iter().zip(arguments) {
            if terminal_semantics::scalar_array_leaf_shape(
                self.structural_types.values(),
                parameter.structural_type,
            )
            .is_some()
                && (parameter.access == StructuralAccess::Owned
                    || self.scalar_array_values.contains_key(&argument.place))
            {
                let value = self.prepare_scalar_array_argument(machine, parameter, argument)?;
                if scalar_arrays.insert(parameter.place, value).is_some() {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
            } else {
                opaque_parameters.push(parameter.clone());
                opaque_arguments.push(argument.clone());
            }
        }
        let scalar_case_result = machine.result.structural().is_some_and(|result| {
            matches!(
                result.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            ) && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && self
                    .structural_types
                    .get(&result.structural_type)
                    .is_some_and(|declaration| {
                        matches!(&declaration.shape, StructuralTypeShape::Sum { cases }
                        if cases.iter().all(|case| case.fields.iter().all(|field|
                            field.relevance == terminal_psi::BindingRelevance::Relevant
                                && field.field_type.scalar_type().is_some())))
                    })
        });
        if (machine.result == TerminalMachineResult::Unit || scalar_case_result)
            && opaque_parameters
                .iter()
                .zip(&opaque_arguments)
                .any(|(parameter, _argument)| {
                    parameter.access == StructuralAccess::MutableBorrow
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
            // replacement are not part of an ordinary call. Its result form
            // does not change the borrowed input's referent or backing.
            let mut prepared = self
                .prepare_boundary_arguments(&opaque_parameters, &opaque_arguments)?
                .into_call_arguments(&opaque_parameters)?;
            prepared.scalar_arrays = scalar_arrays;
            return Ok(prepared);
        }
        let values = self.resolve_reference_call_arguments(&opaque_arguments)?;
        let byte_sequences =
            self.bind_byte_sequence_arguments(&opaque_parameters, &opaque_arguments, &values)?;
        Ok(StructuralCallArguments {
            values: bind_structural_arguments(&opaque_parameters, &values)?,
            byte_sequences,
            scalar_arrays,
        })
    }
}
