//! Frame-owned byte arguments distinguish immutable values from field loans.

use crate::byte_sequences::ByteSequenceView;
use crate::custody::bind_structural_arguments;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::scalar_array::TerminalScalarArrayValue;
use crate::structural_inputs::byte_arrays;
use crate::structural_inputs::byte_arrays::storage::ArrayStorage;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::{TerminalScalarCaseValue, TerminalStructuralValue};
use semantic_vocabulary::MachineId;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;
#[derive(Clone)]
pub(crate) enum ByteSequenceBinding {
    Immutable(ByteSequenceView),
    MutableArray {
        array: ArrayStorage,
        array_type: StructuralTypeId,
        offset: u64,
        length: u64,
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
    pub(crate) fn immutable(&self) -> Result<&ByteSequenceView, TerminalInterpretError> {
        match self {
            Self::Immutable(view) => Ok(view),
            Self::MutableField { .. } | Self::MutableArray { .. } => {
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            }
        }
    }

    pub(crate) fn validate_mutable_referent(
        &self,
        structural_types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if let Self::MutableArray {
            array,
            array_type,
            offset,
            length,
            referent,
        } = self
        {
            let (backing_path, exact_window) = match referent.path.split_last() {
                Some((StructuralPathSegment::FixedByteRange { start, end }, backing)) => (
                    backing,
                    *start == *offset && start.checked_add(*length) == Some(*end),
                ),
                _ => (
                    referent.path.as_slice(),
                    *offset == 0
                        && byte_arrays::byte_array_length(structural_types, *array_type)
                            == Some(*length),
                ),
            };
            return if value == referent
                && array.referent.opaque_identity == referent.opaque_identity
                && array.referent.path == backing_path
                && exact_window
                && referent.qualifications.is_empty()
                && byte_arrays::byte_array_length(structural_types, *array_type).is_some_and(
                    |extent| offset.checked_add(*length).is_some_and(|end| end <= extent),
                ) {
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
            || (!referent.qualifications.is_empty()
                && !structural_types
                    .get(&referent.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                    }))
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
                && declaration.field_type.byte_sequence_carrier()
                    == Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity: *capacity,
                    })
        }) {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(crate) struct StructuralCallArguments {
    pub(crate) local_identity_cursor: Option<u64>,
    pub(crate) values: BTreeMap<PlaceId, TerminalStructuralValue>,
    pub(crate) byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    pub(crate) scalar_arrays: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    pub(crate) scalar_cases: BTreeMap<PlaceId, TerminalScalarCaseValue>,
    pub(crate) element_views: BTreeMap<PlaceId, crate::element_views::ElementView>,
}

impl TerminalExecution {
    /// Reborrow a live byte descriptor, retaining its home for exclusive use
    /// and exposing only an immutable snapshot when authority is attenuated.
    pub(crate) fn reborrow_byte_sequence_binding(
        &self,
        binding: &ByteSequenceBinding,
        value: &TerminalStructuralValue,
        access: StructuralAccess,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) {
            return Err(invalid());
        }
        if let ByteSequenceBinding::Immutable(view) = binding {
            return if access == StructuralAccess::SharedBorrow
                && view.matches_descriptor_home(value)
            {
                Ok(binding.clone())
            } else {
                Err(invalid())
            };
        }
        binding.validate_mutable_referent(&self.program.structural_types, value)?;
        if access == StructuralAccess::MutableBorrow {
            return Ok(binding.clone());
        }
        let snapshot = match binding {
            ByteSequenceBinding::MutableArray {
                array,
                offset,
                length,
                ..
            } => self.stored_byte_array_snapshot(
                array,
                usize::try_from(*offset).map_err(|_| invalid())?,
                usize::try_from(offset.checked_add(*length).ok_or_else(invalid)?)
                    .map_err(|_| invalid())?,
            )?,
            ByteSequenceBinding::MutableField { field, .. } => self
                .structural_byte_sequence_fields
                .get(field)
                .ok_or_else(invalid)?
                .clone(),
            ByteSequenceBinding::Immutable(_) => unreachable!(),
        };
        Ok(ByteSequenceBinding::Immutable(
            snapshot.with_descriptor_home(value.clone()),
        ))
    }

    pub(crate) fn prepare_structural_call_arguments(
        &self,
        callee: MachineId,
        arguments: &[StructuralArgument],
    ) -> Result<StructuralCallArguments, TerminalInterpretError> {
        let machine = self
            .program
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
        let mut scalar_cases = BTreeMap::new();
        let mut opaque_parameters = Vec::new();
        let mut opaque_arguments = Vec::new();
        for (position, (parameter, argument)) in machine
            .structural_parameters
            .iter()
            .zip(arguments)
            .enumerate()
        {
            if self.scalar_case_values.contains_key(&argument.place) {
                if machine.structural_parameters[..position]
                    .iter()
                    .zip(&arguments[..position])
                    .any(|(prior, actual)| {
                        actual.place == argument.place
                            && (prior.multiplicity != StructuralMultiplicity::Unrestricted
                                || parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    })
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                let value = self.prepare_scalar_case_argument(machine, parameter, argument)?;
                if scalar_cases.insert(parameter.place, value).is_some() {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
            } else if terminal_semantics::scalar_array_leaf_shape(
                self.program.structural_types.values(),
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
        if opaque_parameters
            .iter()
            .zip(&opaque_arguments)
            .any(|(parameter, _argument)| {
                matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ) && self
                    .program
                    .structural_types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        declaration.shape.is_borrowed_byte_carrier()
                            || matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
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
            prepared.scalar_cases = scalar_cases;
            return Ok(prepared);
        }
        let values = self.resolve_reference_call_arguments(&opaque_arguments)?;
        let byte_sequences =
            self.bind_byte_sequence_arguments(&opaque_parameters, &opaque_arguments, &values)?;
        let element_views =
            self.bind_element_view_arguments(&opaque_parameters, &opaque_arguments, &values)?;
        Ok(StructuralCallArguments {
            local_identity_cursor: self.local_structural_identities.cursor(),
            values: bind_structural_arguments(&opaque_parameters, &values)?,
            byte_sequences,
            scalar_arrays,
            scalar_cases,
            element_views,
        })
    }
}
