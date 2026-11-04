//! Turn an admitted completed-image observation into ordinary immutable records.
//! The addresses and lengths come from the supplied native artifact. Runtime
//! identities here name only the metadata's own storage, never code addresses.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarAggregateValue,
    TerminalScalarValue, TerminalStructuralValue,
};
use crate::engine::execution::TerminalExecution;
use crate::storage::element_views::{ElementView, immutable_sequence::ImmutableStructuralSequence};
use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
use crate::storage::record::storage::ConstructorStorage;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerType, IntegerValue, MachineId, ScalarType,
};
use terminal_psi::{
    Operation, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape,
};

fn number(integer: IntegerType, value: u64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: integer,
        value: IntegerValue::Unsigned(u128::from(value)),
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn execute_native_code_layout(
        &mut self,
        operation: &Operation,
        machine: MachineId,
    ) -> Result<(), TerminalInterpretError> {
        let environment = self.native_code_layouts.clone().ok_or(
            TerminalInterpretError::UnsupportedCapability(
                crate::TerminalUnsupportedCapability::NativeCodeLayout { machine },
            ),
        )?;
        let layout =
            environment
                .layout(machine)
                .ok_or(TerminalInterpretError::UnsupportedCapability(
                    crate::TerminalUnsupportedCapability::NativeCodeLayout { machine },
                ))?;
        let result = operation.result.structural().ok_or_else(malformed)?;
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || self.structural_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let StructuralTypeShape::Record { fields, .. } =
            &self.structural_declaration(result.structural_type)?.shape
        else {
            return Err(malformed());
        };
        let [entry, regions] = fields.as_slice() else {
            return Err(malformed());
        };
        let StructuralFieldType::Scalar(ScalarType::Integer(entry_type)) = entry.field_type else {
            return Err(malformed());
        };
        let StructuralFieldType::Structural(view_type) = regions.field_type else {
            return Err(malformed());
        };
        let entry_field = entry.id;
        let regions_identity = regions.identity;
        let StructuralTypeShape::ElementView { element, .. } =
            self.structural_declaration(view_type)?.shape
        else {
            return Err(malformed());
        };
        let element_type = element;
        let StructuralTypeShape::Record { fields, .. } =
            &self.structural_declaration(element_type)?.shape
        else {
            return Err(malformed());
        };
        let [base, length] = fields.as_slice() else {
            return Err(malformed());
        };
        let StructuralFieldType::Scalar(ScalarType::Integer(base_type)) = base.field_type else {
            return Err(malformed());
        };
        let StructuralFieldType::Scalar(ScalarType::Integer(length_type)) = length.field_type
        else {
            return Err(malformed());
        };
        let base_field = base.id;
        let length_field = length.id;
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        let root = StructuralRuntimePlace::from(&value);
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        storage.stage_scalar_endpoint(
            &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::member(
                root.clone(),
                entry_field,
            )),
            number(entry_type, layout.entry),
        )?;
        let account = self.account.clone();
        let elements = ImmutableStructuralSequence::from_values(
            layout.regions.len(),
            layout.regions.iter().map(|region| {
                let aggregate = TerminalScalarAggregateValue {
                    structural_type: element_type,
                    result_case: None,
                    fields: vec![
                        (
                            vec![CanonicalStructuralPathSegment::Field(base_field)],
                            number(base_type, region.base),
                        ),
                        (
                            vec![CanonicalStructuralPathSegment::Field(length_field)],
                            number(length_type, region.length),
                        ),
                    ],
                };
                aggregate
                    .validate_against(self.structural_declaration(element_type)?, |identity| {
                        self.structural_types().get(&identity)
                    })?;
                let element = TerminalStructuralValue::minted(
                    self.local_structural_identities.allocate()?,
                    element_type,
                    Vec::new(),
                );
                storage.stage_scalar_aggregate(
                    self,
                    &aggregate,
                    &StructuralRuntimePlace::from(&element),
                )?;
                Ok(element)
            }),
            account.as_ref(),
        )?;
        let mut descriptor = value.clone();
        descriptor.structural_type = view_type;
        descriptor
            .path
            .push(StructuralPathSegment::Field(regions_identity));
        let view = ElementView::immutable_sequence(elements, descriptor.clone(), result.place);
        storage.publish(self)?;
        self.structural_element_view_fields
            .insert(StructuralRuntimePlace::from(&descriptor), view)?;
        self.structural_values.insert(result.place, value)?;
        self.track_affine_result(result);
        Ok(())
    }
}
