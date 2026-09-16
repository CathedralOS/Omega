//! Structural operations of the interpreter loop: descriptor stores,
//! scalar cases, byte-sequence literals and reads, trivial affine
//! locals, records, field stores and field reads.

use crate::terminal_interpreter::byte_sequence_binding::ByteSequenceBinding;
use crate::terminal_interpreter::byte_sequence_view::ByteSequenceView;
use crate::terminal_interpreter::custody::{
    direct_scalar_field_type, resolve_structural_arguments,
};
use crate::terminal_interpreter::execution::{OperationFlow, TerminalExecution};
use crate::terminal_interpreter::scalar_operations::terminal_scalar_belongs_to_type;
use crate::terminal_interpreter::values::{StructuralRuntimePlace, StructuralScalarRuntimeField};
use crate::terminal_interpreter::{
    TerminalInterpretError, TerminalScalarCaseValue, TerminalScalarValue, TerminalStructuralValue,
};
use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType};
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
};

impl TerminalExecution {
    pub(super) fn execute_store_dynamic_descriptor(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StoreDynamicDescriptor { descriptor_ordinal } = operation.kind else {
            unreachable!("dispatched execute_store_dynamic_descriptor")
        };
        if operation.result != terminal_psi::OperationResult::Unit
            || !self
                .dynamic_descriptor_templates
                .contains_key(&(self.current_machine, descriptor_ordinal))
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_establish_scalar_case(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishScalarCase {
            result_case,
            ref fields,
        } = operation.kind
        else {
            unreachable!("dispatched execute_establish_scalar_case")
        };
        let terminal_psi::OperationResult::Structural(result) = &operation.result else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || !matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Sum { cases },
            ..
        }) = self.structural_types.get(&result.structural_type)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let Some(selected) = cases.iter().find(|case| case.id == result_case) else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if selected.fields.len() != fields.len() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut payload = Vec::with_capacity(fields.len());
        for (declaration, binding) in selected.fields.iter().zip(fields) {
            let value = self
                .values
                .get(&binding.value)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedValueMissing(binding.value))?;
            if declaration.id != binding.field
                || declaration.field_type.scalar_type() != Some(value.scalar_type())
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            if let terminal_psi::StructuralFieldType::BoundedInteger(bounds) =
                declaration.field_type
            {
                let TerminalScalarValue::Integer { value, .. } = value else {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                };
                if !bounds.contains(value) {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
            }
            payload.push((binding.field, value));
        }
        self.scalar_case_values.insert(
            result.place,
            TerminalScalarCaseValue {
                structural_type: result.structural_type,
                result_case,
                fields: payload,
            },
        );
        if result.multiplicity == StructuralMultiplicity::Affine {
            self.live_affine_frontier.insert(StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            });
        }
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_establish_byte_sequence_literal(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishByteSequenceLiteral {
            destination,
            ref bytes,
        } = operation.kind
        else {
            unreachable!("dispatched execute_establish_byte_sequence_literal")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let Some(terminal_psi::StructuralPlaceDeclaration {
            kind:
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                },
            ..
        }) = machine
            .structural_places
            .iter()
            .find(|place| place.id == destination)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if let Some(previous) = self.structural_values.get(&destination) {
            // Reentering the unique literal producer retains the
            // same immutable value. Fuel was charged above;
            // existing aliases keep their exact original bytes.
            if previous.opaque_identity != destination.get()
                || previous.structural_type != *structural_type
                || !previous.path.is_empty()
                || !previous.qualifications.is_empty()
                || self.byte_sequence_values.get(&destination).is_none_or(
                    |binding| !matches!(binding.immutable(), Ok(view) if view.bytes() == bytes),
                )
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
        } else {
            if self
                .byte_sequence_values
                .insert(
                    destination,
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes.clone())),
                )
                .is_some()
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            self.structural_values.insert(
                destination,
                TerminalStructuralValue {
                    opaque_identity: destination.get(),
                    structural_type: *structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                },
            );
        }
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_establish_trivial_affine_local(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind else {
            unreachable!("dispatched execute_establish_trivial_affine_local")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit)
            || self.structural_values.contains_key(&destination)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let Some(terminal_psi::StructuralPlaceDeclaration {
            kind:
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                },
            ..
        }) = machine
            .structural_places
            .iter()
            .find(|place| place.id == destination)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.structural_values.insert(
            destination,
            TerminalStructuralValue {
                opaque_identity: destination.get(),
                structural_type: *structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        );
        self.live_affine_frontier.insert(StructuralAffineDiscard {
            place: destination,
            path: Vec::new(),
            structural_type: *structural_type,
        });
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_establish_record(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishRecord { ref fields } = operation.kind else {
            unreachable!("dispatched execute_establish_record")
        };
        self.establish_record(
            operation
                .result
                .structural()
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?,
            fields,
        )?;
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_structural_scalar_field_store(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StructuralScalarFieldStore {
            destination,
            ref path,
            field,
            value,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_structural_scalar_field_store")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let access = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == destination)
            .filter(|parameter| {
                matches!(
                    parameter.access,
                    StructuralAccess::Owned
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ) && matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                ) && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            .map(|parameter| parameter.access)
            .or_else(|| {
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.operations)
                    .filter(|producer| {
                        matches!(
                            producer.kind,
                            OperationKind::EstablishRecord { .. }
                                | OperationKind::CallStructural { .. }
                                | OperationKind::CallStructuralWithScalarArguments { .. }
                        )
                    })
                    .filter_map(|producer| producer.result.structural())
                    .find(|result| {
                        result.place == destination
                            && matches!(
                                result.multiplicity,
                                StructuralMultiplicity::Unrestricted
                                    | StructuralMultiplicity::Affine
                            )
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                    })
                    .map(|_| StructuralAccess::Owned)
            })
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let source = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        let parent = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            &[StructuralArgument {
                place: destination,
                path: path.clone(),
                access,
            }],
        )?
        .pop()
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if direct_scalar_field_type(&self.structural_types, parent.structural_type, field)
            != Some(source.scalar_type())
            || !terminal_scalar_belongs_to_type(source)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.structural_scalar_fields.insert(
            StructuralScalarRuntimeField {
                parent: StructuralRuntimePlace::from(&parent),
                field,
            },
            source,
        );
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_structural_case_membership(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::StructuralCaseMembership {
            source,
            ref path,
            case,
        } = operation.kind
        else {
            unreachable!("dispatched execute_structural_case_membership")
        };
        let active_case = self.observe_structural_case(source, path)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(active_case == case),
        );
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_boolean_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BooleanStructuralField {
            source,
            ref path,
            field,
        } = operation.kind
        else {
            unreachable!("dispatched execute_boolean_structural_field")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_value = self.structural_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        let carrier = terminal_semantics::record_field_carrier(
            self.structural_types.values(),
            structural_value.structural_type,
            path,
        )
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let mut parent = StructuralRuntimePlace::from(structural_value);
        parent.path.extend(carrier.path);
        let value = self
            .structural_scalar_fields
            .get(&StructuralScalarRuntimeField { parent, field })
            .copied()
            .ok_or(TerminalInterpretError::StructuralBooleanFieldMissing { source, field })?;
        let TerminalScalarValue::Boolean(value) = value else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(value),
        );
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_integer_structural_field(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerStructuralField {
            source,
            ref path,
            field,
        } = operation.kind
        else {
            unreachable!("dispatched execute_integer_structural_field")
        };
        let result = operation.result.expect_scalar();
        if !matches!(result.scalar_type, ScalarType::Integer(_)) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_value = self.structural_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )?;
        let carrier = terminal_semantics::record_field_carrier(
            self.structural_types.values(),
            structural_value.structural_type,
            path,
        )
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if direct_scalar_field_type(&self.structural_types, carrier.structural_type, field)
            != Some(result.scalar_type)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut parent = StructuralRuntimePlace::from(structural_value);
        parent.path.extend(carrier.path);
        let value = self
            .structural_scalar_fields
            .get(&StructuralScalarRuntimeField { parent, field })
            .copied()
            .ok_or(TerminalInterpretError::StructuralScalarFieldMissing { source, field })?;
        if value.scalar_type() != result.scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(result.id, value);
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_byte_sequence_read(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ByteSequenceRead {
            source,
            index,
            length,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_byte_sequence_read")
        };
        let result = operation.result.expect_scalar();
        let byte_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).expect("u8 is valid");
        let count_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).expect("u64 is valid");
        if result.scalar_type != ScalarType::Integer(byte_type) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let count = |operand| -> Result<u64, TerminalInterpretError> {
            let value = self
                .values
                .get(&operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?;
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = value
            else {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            };
            if *scalar_type != count_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            u64::try_from(*value).map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)
        };
        let byte_index = count(index)?;
        let byte_length = count(length)?;
        let bytes = self
            .byte_sequence_values
            .get(&source)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                source,
            ))?
            .immutable()?;
        if u64::try_from(bytes.len()).ok() != Some(byte_length) || byte_index >= byte_length {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let byte_index = usize::try_from(byte_index)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        let byte = bytes
            .get(byte_index)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: byte_type,
                value: IntegerValue::Unsigned(u128::from(byte)),
            },
        );
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_byte_sequence_length(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ByteSequenceLength { source } = operation.kind else {
            unreachable!("dispatched execute_byte_sequence_length")
        };
        let result = operation.result.expect_scalar();
        let integer_type =
            IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).expect("u64 is valid");
        if result.scalar_type != ScalarType::Integer(integer_type) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let length = self.byte_sequence_length(source)?;
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: integer_type,
                value: IntegerValue::Unsigned(u128::from(length)),
            },
        );
        Ok(OperationFlow::Advance)
    }
}
