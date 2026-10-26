//! Exact scalar-leaf record codecs, shared by value transport and borrowed views.
//! Physical placement comes from the retained target layout. The semantic
//! declaration independently authenticates the complete leaf roster and types.

use super::super::scalar_codec::{decode, encode, scalar_extent};
use super::original_homes::{exact_layout, rejection};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalEffectRejection, TerminalProgram,
    TerminalScalarAggregateValue, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::StructuralTypeShape;

pub(super) struct ScalarField {
    pub path: Vec<Path>,
    pub offset: usize,
    pub logical: ScalarType,
    pub stored: ScalarType,
}
pub(super) struct StorageCodec {
    pub structural_type: StructuralTypeId,
    pub extent: usize,
    pub alignment: usize,
    pub fields: Vec<ScalarField>,
}
impl StorageCodec {
    pub fn admit(
        program: &TerminalProgram,
        layouts: &[NativeBorrowedStorageLayout],
        structural_type: StructuralTypeId,
    ) -> Result<Self, TerminalEffectRejection> {
        let layout = exact_layout(layouts, structural_type)?;
        if layout.discriminators().next().is_some() || !layout.opaque_regions().is_empty() {
            return Err(rejection(
                "native import needs a resident tag or opaque-region codec",
            ));
        }
        let declaration = program
            .structural_type(structural_type)
            .ok_or_else(|| rejection("native codec has no semantic structural declaration"))?;
        if declaration.quotient_carrier.is_some() {
            return Err(rejection("native import needs its exact quotient codec"));
        }
        if let StructuralTypeShape::PrimitiveScalar(logical) = declaration.shape {
            let [recipe] = layout.fields() else {
                return Err(rejection("native primitive lost its one scalar recipe"));
            };
            if layout.field_path(recipe).len() != 0
                || !layout.field_repetitions(recipe).is_empty()
                || recipe.byte_offset != 0
                || recipe.logical_scalar != logical
                || scalar_extent(recipe.stored_scalar)? != layout.byte_size as usize
            {
                return Err(rejection(
                    "native primitive recipe changes its scalar owner",
                ));
            }
            return Ok(Self {
                structural_type,
                extent: layout.byte_size as usize,
                alignment: usize::from(layout.alignment),
                fields: vec![ScalarField {
                    path: Vec::new(),
                    offset: 0,
                    logical,
                    stored: recipe.stored_scalar,
                }],
            });
        }
        if !matches!(&declaration.shape, StructuralTypeShape::Record { .. }) {
            return Err(rejection(
                "native import needs a supported complete scalar record codec",
            ));
        }
        let mut fields = Vec::new();
        for field in super::record_fields::scalar_fields(program, structural_type)? {
            let path = field.path;
            let logical = field.logical;
            let mut recipes = layout
                .fields()
                .iter()
                .filter(|recipe| layout.field_path(recipe).eq(path.iter().copied()));
            let recipe = recipes
                .next()
                .ok_or_else(|| rejection("native record lost a semantic field's layout"))?;
            if recipes.next().is_some()
                || !layout.field_repetitions(recipe).is_empty()
                || recipe.logical_scalar != logical
            {
                return Err(rejection(
                    "native record field recipe changes its semantic carrier",
                ));
            }
            let offset = recipe.byte_offset as usize;
            let extent = scalar_extent(recipe.stored_scalar)?;
            if offset
                .checked_add(extent)
                .is_none_or(|end| end > layout.byte_size as usize)
                || fields.iter().any(|existing: &ScalarField| {
                    offset < existing.offset + scalar_extent(existing.stored).unwrap_or(usize::MAX)
                        && existing.offset < offset + extent
                })
            {
                return Err(rejection(
                    "native record field geometry overlaps or exceeds storage",
                ));
            }
            fields.push(ScalarField {
                path,
                offset,
                logical,
                stored: recipe.stored_scalar,
            });
        }
        if fields.len() != layout.fields().len()
            || layout.byte_size == 0
            || !layout.alignment.is_power_of_two()
        {
            return Err(rejection(
                "native record does not cover its complete scalar layout",
            ));
        }
        Ok(Self {
            structural_type,
            extent: layout.byte_size as usize,
            alignment: usize::from(layout.alignment),
            fields,
        })
    }
    pub fn encode(
        &self,
        values: &[TerminalBoundaryStorageValue],
    ) -> Result<Vec<u8>, TerminalEffectRejection> {
        if values.len() != self.fields.len() {
            return Err(rejection(
                "native record payload has an incomplete field roster",
            ));
        }
        let mut bytes = vec![0; self.extent];
        for field in &self.fields {
            let mut matches = values.iter().filter(|value| value.path == field.path);
            let value = matches
                .next()
                .ok_or_else(|| rejection("native record field is absent"))?;
            let TerminalStorageContents::Scalar {
                value,
                stored_scalar,
            } = value.contents
            else {
                return Err(rejection("native record field is not scalar"));
            };
            if matches.next().is_some()
                || stored_scalar != field.stored
                || value.scalar_type() != field.logical
            {
                return Err(rejection(
                    "native record field changes its exact scalar codec",
                ));
            }
            let encoded = encode(value, field.stored)?;
            let size = scalar_extent(field.stored)?;
            bytes[field.offset..field.offset + size].copy_from_slice(&encoded[..size]);
        }
        Ok(bytes)
    }
    pub fn decode_fields(
        &self,
        bytes: &[u8],
    ) -> Result<Vec<TerminalBoundaryStorageValue>, TerminalEffectRejection> {
        if bytes.len() != self.extent {
            return Err(rejection(
                "native result changed its declared record extent",
            ));
        }
        self.fields
            .iter()
            .map(|field| {
                let size = scalar_extent(field.stored)?;
                Ok(TerminalBoundaryStorageValue {
                    path: field.path.clone(),
                    contents: TerminalStorageContents::Scalar {
                        value: decode(
                            &bytes[field.offset..field.offset + size],
                            field.stored,
                            field.logical,
                        )?,
                        stored_scalar: field.stored,
                    },
                })
            })
            .collect()
    }
    pub fn aggregate(
        &self,
        bytes: &[u8],
    ) -> Result<TerminalScalarAggregateValue, TerminalEffectRejection> {
        let fields = self
            .decode_fields(bytes)?
            .into_iter()
            .map(|field| {
                let [Path::Field(id)] = field.path.as_slice() else {
                    return Err(rejection(
                        "native result requires a non-flat aggregate publisher",
                    ));
                };
                let TerminalStorageContents::Scalar { value, .. } = field.contents else {
                    return Err(rejection("native result lost its scalar payload"));
                };
                Ok((*id, value))
            })
            .collect::<Result<Vec<(StructuralFieldId, TerminalScalarValue)>, _>>()?;
        Ok(TerminalScalarAggregateValue {
            structural_type: self.structural_type,
            result_case: None,
            fields,
        })
    }
}
