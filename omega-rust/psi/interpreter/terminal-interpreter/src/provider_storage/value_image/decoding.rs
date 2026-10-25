//! Recover the current represented value, without reconstructing its custody.
//!
//! The resident lifecycle separately owns the original T and its obligations.
//! This decoder stages only its current physical cells, using the same field
//! extents and retained nested layouts as initialization. It cannot mint an
//! owner, recover a loan from an address, or substitute for authored custody.

use super::{field_schema, record_layout, require_runtime_fields, zeroed};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::primitive_storage::PrimitiveStorageEndpoint;
use crate::record::storage::ConstructorStorage;
use crate::values::{StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue};
use layout_plans::ByteOrder;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeShape,
    TerminalPlacementLifecyclePlan,
};

pub(crate) fn restore(
    execution: &TerminalExecution<'_>,
    plan: &TerminalPlacementLifecyclePlan,
    image: &[u8],
    destination: &StructuralRuntimePlace,
) -> Result<ConstructorStorage, TerminalInterpretError> {
    let types = execution.program.structural_types();
    let Some(StructuralTypeShape::Record { fields, .. }) =
        types.get(&plan.schema).map(|row| &row.shape)
    else {
        return Err(malformed());
    };
    require_runtime_fields(execution, fields)?;
    let entries = plan.placement.access().plan().entries();
    if entries.len() != plan.image_fields.len() {
        return Err(malformed());
    }
    let mut decoder = ValueImageDecoder {
        execution,
        storage: ConstructorStorage::new(execution.account.as_ref()),
        home: destination.clone(),
        active: Vec::new(),
    };
    for (identity, entry) in plan.image_fields.iter().zip(entries) {
        let field = fields
            .iter()
            .find(|field| field.id == *identity)
            .ok_or_else(malformed)?;
        let layout_entry = plan
            .placement
            .layout()
            .entries
            .iter()
            .find(|layout| layout.field == entry.field())
            .ok_or_else(malformed)?;
        let schema = field_schema(
            execution,
            field,
            entry.field(),
            layout_entry.member_identity,
        )?;
        let mut bytes = zeroed(schema.byte_size, execution.account.as_ref())?;
        layout_plans::decode_aggregate_field_into(
            plan.placement.layout(),
            &schema,
            ByteOrder::LittleEndian,
            image,
            bytes.bytes_mut(),
        )
        .map_err(TerminalInterpretError::ValueImage)?;
        decoder.field(field, bytes.bytes())?;
    }
    Ok(decoder.storage)
}

struct ValueImageDecoder<'execution, 'program> {
    execution: &'execution TerminalExecution<'program>,
    storage: ConstructorStorage,
    home: StructuralRuntimePlace,
    active: Vec<StructuralTypeId>,
}

fn scalar(scalar: ScalarType, bytes: &[u8]) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let width = terminal_semantics::reference_storage_byte_width(scalar).ok_or_else(malformed)?;
    if bytes.len() != usize::from(width) || bytes.len() > 16 {
        return Err(malformed());
    }
    let mut encoded = [0_u8; 16];
    encoded[..bytes.len()].copy_from_slice(bytes);
    crate::primitive_storage::encoding::decode(scalar, u128::from_le_bytes(encoded))
}

impl ValueImageDecoder<'_, '_> {
    fn field(
        &mut self,
        field: &StructuralFieldDeclaration,
        bytes: &[u8],
    ) -> Result<(), TerminalInterpretError> {
        if let Some(carrier) = field.field_type.scalar_type() {
            self.storage.stage_scalar_endpoint(
                &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::member(
                    self.home.clone(),
                    field.id,
                )),
                scalar(carrier, bytes)?,
            )
        } else {
            self.home
                .path
                .push(StructuralPathSegment::Field(field.identity));
            self.structure(
                field.field_type.as_structural().ok_or_else(malformed)?,
                bytes,
            )?;
            self.home.path.pop();
            Ok(())
        }
    }

    fn structure(
        &mut self,
        root: StructuralTypeId,
        bytes: &[u8],
    ) -> Result<(), TerminalInterpretError> {
        if self.active.contains(&root) || self.active.len() >= 128 {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "resident image has cyclic or excessive structural depth",
            ));
        }
        self.active.push(root);
        let types = self.execution.program.structural_types();
        let declaration = types.get(&root).ok_or_else(malformed)?;
        let (size, alignment) = terminal_semantics::reference_storage_layout(types.values(), root)
            .ok_or_else(malformed)?;
        if u64::try_from(bytes.len()).ok() != Some(size) {
            return Err(malformed());
        }
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(carrier) => {
                self.storage.stage_scalar_endpoint(
                    &PrimitiveStorageEndpoint::Scalar(self.home.clone()),
                    scalar(*carrier, bytes)?,
                )?;
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let (stride, _) =
                    terminal_semantics::reference_storage_layout(types.values(), *element)
                        .ok_or_else(malformed)?;
                let stride = usize::try_from(stride).map_err(|_| malformed())?;
                if stride == 0 {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "resident zero-runtime elements require authored custody",
                    ));
                }
                if let Some(StructuralTypeShape::PrimitiveScalar(carrier)) =
                    types.get(element).map(|row| &row.shape)
                {
                    let values = bytes
                        .chunks_exact(stride)
                        .map(|bytes| scalar(*carrier, bytes))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.storage
                        .stage_array(self.execution, root, &values, &self.home)?;
                } else {
                    for element_index in 0..*length {
                        let start = usize::try_from(element_index)
                            .ok()
                            .and_then(|index| index.checked_mul(stride))
                            .ok_or_else(malformed)?;
                        let end = start.checked_add(stride).ok_or_else(malformed)?;
                        self.home
                            .path
                            .push(StructuralPathSegment::FixedIndex(element_index));
                        self.structure(*element, bytes.get(start..end).ok_or_else(malformed)?)?;
                        self.home.path.pop();
                    }
                }
            }
            StructuralTypeShape::Record { fields, storage } => {
                require_runtime_fields(self.execution, fields)?;
                let layout = record_layout(self.execution, root, fields, storage, size, alignment)?;
                for field in fields {
                    let name = format!("field:{:?}", field.id);
                    let schema = field_schema(self.execution, field, &name, None)?;
                    let mut contents = zeroed(schema.byte_size, self.execution.account.as_ref())?;
                    layout_plans::decode_aggregate_field_into(
                        &layout,
                        &schema,
                        ByteOrder::LittleEndian,
                        bytes,
                        contents.bytes_mut(),
                    )
                    .map_err(TerminalInterpretError::ValueImage)?;
                    self.field(field, contents.bytes())?;
                }
            }
            _ => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident image is outside the retained schema reflection contract",
                ));
            }
        }
        self.active.pop();
        Ok(())
    }
}
