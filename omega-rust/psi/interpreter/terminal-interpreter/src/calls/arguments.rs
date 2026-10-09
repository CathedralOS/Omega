//! Prepare ordinary calls once, preserving each argument's authenticated carrier.
//!
//! Owned scalar arrays already own snapshots. Borrowed arrays
//! retain their live collection home instead. Opaque aggregate values
//! need fresh subtree storage for unrestricted owned occurrences. Frames must
//! preserve this partition instead of reclassifying every parameter by type.
//! Every constructed sum owns ordinary placed discriminator/payload storage.
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::bind_structural_arguments;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::scalar_array::TerminalScalarArrayValue;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{MachineId, PlaceId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralParameterDeclaration, StructuralTypeShape,
};

pub(crate) struct StructuralCallArguments {
    pub(crate) local_identity_cursor: Option<u64>,
    /// Ordered declarations for the opaque carriers, not the full signature.
    pub(crate) descriptor_parameters: Vec<StructuralParameterDeclaration>,
    pub(crate) values: BTreeMap<PlaceId, TerminalStructuralValue>,
    pub(crate) byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    pub(crate) primitive_loans: BTreeMap<PlaceId, crate::primitive_storage::PrimitiveLoan>,
    pub(crate) scalar_arrays: BTreeMap<PlaceId, TerminalScalarArrayValue>,
    pub(crate) element_views: BTreeMap<PlaceId, crate::element_views::ElementView>,
}

impl StructuralCallArguments {
    /// Every declared argument has exactly one primary carrier. Companion loans,
    /// bytes and views decorate opaque carriers; they are not extra arguments.
    /// Retain declaration order so snapshot staging cannot reinterpret a packed
    /// array as an absent opaque aggregate.
    pub(crate) fn validate_carrier_partition(
        &self,
        parameters: &[StructuralParameterDeclaration],
    ) -> Result<(), TerminalInterpretError> {
        if self.values.len() + self.scalar_arrays.len() != parameters.len()
            || self.descriptor_parameters.len() != self.values.len()
        {
            return Err(malformed());
        }
        let mut descriptors = self.descriptor_parameters.iter();
        for parameter in parameters {
            let opaque = self.values.contains_key(&parameter.place);
            let arrays = self.scalar_arrays.contains_key(&parameter.place);
            if usize::from(opaque) + usize::from(arrays) != 1
                || (opaque && descriptors.next() != Some(parameter))
            {
                return Err(malformed());
            }
        }
        if descriptors.next().is_some() {
            return Err(malformed());
        }
        Ok(())
    }
}

impl TerminalExecution<'_> {
    /// Resolves the scalar argument values and prepares the structural
    /// arguments for one call site.
    pub(crate) fn resolve_call_arguments(
        &self,
        callee: MachineId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
    ) -> Result<(Vec<TerminalScalarValue>, StructuralCallArguments), TerminalInterpretError> {
        let scalar_arguments = self.scalar_values_at(arguments)?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        Ok((scalar_arguments, prepared_arguments))
    }

    pub(crate) fn prepare_structural_call_arguments(
        &self,
        callee: MachineId,
        arguments: &[StructuralArgument],
    ) -> Result<StructuralCallArguments, TerminalInterpretError> {
        let machine = self.program.verified_machine(callee)?;
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
                self.program.structural_types().values(),
                parameter.structural_type,
            )
            .is_some()
                && parameter.access == StructuralAccess::Owned
            {
                let value = self.prepare_scalar_array_argument(machine, parameter, argument)?;
                if scalar_arrays.insert(parameter.place, value).is_some() {
                    return Err(malformed());
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
                    .structural_types()
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::PrimitiveScalar(_)
                                | StructuralTypeShape::FixedArray { .. }
                                | StructuralTypeShape::ElementView { .. }
                        ) || declaration.shape.is_borrowed_byte_carrier()
                            || declaration.shape.is_bounded_owned_byte_sequence()
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
        let mut values = self.resolve_reference_call_arguments(&opaque_arguments)?;
        let byte_sequences =
            self.bind_byte_sequence_arguments(&opaque_parameters, &opaque_arguments, &values)?;
        // An owned byte copy was just checked against every domain its formal
        // names, so the callee holds the copy under those domains.
        for (value, parameter) in values.iter_mut().zip(&opaque_parameters) {
            if parameter.access == StructuralAccess::Owned
                && byte_sequences.contains_key(&parameter.place)
            {
                value.absorb_qualifications(&parameter.qualifications);
            }
        }
        let element_views =
            self.bind_element_view_arguments(&opaque_parameters, &opaque_arguments, &values)?;
        Ok(StructuralCallArguments {
            local_identity_cursor: self.local_structural_identities.cursor(),
            values: bind_structural_arguments(&opaque_parameters, &values)?,
            descriptor_parameters: opaque_parameters,
            byte_sequences,
            primitive_loans: BTreeMap::new(),
            scalar_arrays,
            element_views,
        })
    }
}
