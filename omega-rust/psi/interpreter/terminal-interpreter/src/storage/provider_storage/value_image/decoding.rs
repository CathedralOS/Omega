//! Recover the current represented value, without reconstructing its custody.
//!
//! The resident lifecycle separately owns the original T and its obligations.
//! This decoder stages only its current physical cells, using the same field
//! extents and retained nested layouts as initialization. It cannot mint an
//! owner, recover a loan from an address, or substitute for authored custody.

use super::geometry::ImageGeometry;
use super::{field_schema, record_layout, require_runtime_fields, zeroed};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
};
use crate::engine::execution::TerminalExecution;
use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
use crate::storage::record::storage::ConstructorStorage;
use layout_plans::ByteOrder;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeShape,
    TerminalAccessLifecyclePlan,
};

pub(crate) fn restore(
    execution: &TerminalExecution<'_>,
    plan: &TerminalAccessLifecyclePlan,
    image: &[u8],
    destination: &StructuralRuntimePlace,
) -> Result<ConstructorStorage, TerminalInterpretError> {
    let types = execution.structural_types();
    let Some(StructuralTypeShape::Record { fields, .. }) =
        types.get(&plan.schema).map(|row| &row.shape)
    else {
        return Err(malformed());
    };
    let geometry = ImageGeometry::new(execution, plan.target.as_ref())?;
    require_runtime_fields(&geometry, fields)?;
    let entries = plan.placement.access().plan().entries();
    if entries.len() != plan.image_fields.len() {
        return Err(malformed());
    }
    let mut decoder = ValueImageDecoder {
        execution,
        storage: ConstructorStorage::new(execution.account.as_ref()),
        home: destination.clone(),
        active: Vec::new(),
        geometry,
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
            &geometry,
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
        decoder.field(field, bytes.bytes(), None)?;
    }
    Ok(decoder.storage)
}

struct ValueImageDecoder<'execution, 'program> {
    execution: &'execution TerminalExecution<'program>,
    storage: ConstructorStorage,
    home: StructuralRuntimePlace,
    active: Vec<StructuralTypeId>,
    geometry: ImageGeometry<'execution, 'program>,
}

fn scalar(scalar: ScalarType, bytes: &[u8]) -> Result<TerminalScalarValue, TerminalInterpretError> {
    let width = terminal_semantics::reference_storage_byte_width(scalar).ok_or_else(malformed)?;
    if bytes.len() != usize::from(width) || bytes.len() > 16 {
        return Err(malformed());
    }
    let mut encoded = [0_u8; 16];
    encoded[..bytes.len()].copy_from_slice(bytes);
    crate::storage::primitive_storage::encoding::decode(scalar, u128::from_le_bytes(encoded))
}

impl ValueImageDecoder<'_, '_> {
    fn field(
        &mut self,
        field: &StructuralFieldDeclaration,
        bytes: &[u8],
        case: Option<semantic_vocabulary::StructuralCaseId>,
    ) -> Result<(), TerminalInterpretError> {
        if let Some(carrier) = field.field_type.scalar_type() {
            self.storage.stage_scalar_endpoint(
                &PrimitiveStorageEndpoint::ScalarField(match case {
                    Some(_) => {
                        StructuralScalarRuntimeField::case_payload(self.home.clone(), field.id)
                    }
                    None => StructuralScalarRuntimeField::member(self.home.clone(), field.id),
                }),
                scalar(carrier, bytes)?,
            )
        } else {
            if let Some(case) = case {
                self.home.path.push(StructuralPathSegment::Case(case));
            }
            self.home
                .path
                .push(StructuralPathSegment::Field(field.identity));
            self.structure(
                field.field_type.as_structural().ok_or_else(malformed)?,
                bytes,
            )?;
            self.home.path.pop();
            if case.is_some() {
                self.home.path.pop();
            }
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
        let types = self.execution.structural_types();
        let declaration = types.declaration(&root)?;
        let (size, alignment) = self.geometry.layout(root)?;
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
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                let (stride, _) = self.geometry.layout(*element)?;
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
                require_runtime_fields(&self.geometry, fields)?;
                let layout = record_layout(
                    self.execution,
                    &self.geometry,
                    root,
                    fields,
                    storage,
                    size,
                    alignment,
                )?;
                for field in fields {
                    let name = format!("field:{:?}", field.id);
                    let schema = field_schema(self.execution, &self.geometry, field, &name, None)?;
                    let mut contents = zeroed(schema.byte_size, self.execution.account.as_ref())?;
                    layout_plans::decode_aggregate_field_into(
                        &layout,
                        &schema,
                        ByteOrder::LittleEndian,
                        bytes,
                        contents.bytes_mut(),
                    )
                    .map_err(TerminalInterpretError::ValueImage)?;
                    self.field(field, contents.bytes(), None)?;
                }
            }
            StructuralTypeShape::Sum { cases } => self.sum(root, &[], cases, bytes)?,
            StructuralTypeShape::Mixed { fields, cases } => self.sum(root, fields, cases, bytes)?,
            _ => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident image is outside the retained schema reflection contract",
                ));
            }
        }
        self.active.pop();
        Ok(())
    }

    fn sum(
        &mut self,
        root: StructuralTypeId,
        common: &[StructuralFieldDeclaration],
        cases: &[terminal_psi::StructuralCaseDeclaration],
        bytes: &[u8],
    ) -> Result<(), TerminalInterpretError> {
        let layout = self.geometry.sum_layout(root)?;
        let width = self.geometry.tag_width()?;
        let mut tag = [0_u8; 8];
        tag[..width].copy_from_slice(bytes.get(..width).ok_or_else(malformed)?);
        let case_position = usize::try_from(u64::from_le_bytes(tag)).map_err(|_| malformed())?;
        let case = cases.get(case_position).ok_or(
            TerminalInterpretError::ContentTransformationMismatch(
                "resident image contains an invalid case tag",
            ),
        )?;
        self.storage.stage_case(self.home.clone(), root, case.id)?;
        self.sum_fields(common, &layout.common_field_offsets, bytes, None)?;
        self.sum_fields(
            &case.fields,
            &layout.case_field_offsets[case_position],
            bytes,
            Some(case.id),
        )
    }

    fn sum_fields(
        &mut self,
        fields: &[StructuralFieldDeclaration],
        offsets: &[u64],
        bytes: &[u8],
        case: Option<semantic_vocabulary::StructuralCaseId>,
    ) -> Result<(), TerminalInterpretError> {
        require_runtime_fields(&self.geometry, fields)?;
        if fields.len() != offsets.len() {
            return Err(malformed());
        }
        for (field, offset) in fields.iter().zip(offsets) {
            let schema = field_schema(self.execution, &self.geometry, field, "", None)?;
            let start = usize::try_from(*offset).map_err(|_| malformed())?;
            let length = usize::try_from(schema.byte_size).map_err(|_| malformed())?;
            let end = start.checked_add(length).ok_or_else(malformed)?;
            self.field(field, bytes.get(start..end).ok_or_else(malformed)?, case)?;
        }
        Ok(())
    }
}
