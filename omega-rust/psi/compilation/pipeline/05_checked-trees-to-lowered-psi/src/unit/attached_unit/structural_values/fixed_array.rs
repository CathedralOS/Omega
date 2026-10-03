//! Element-wise fixed-array literals of scalar-leaf structural elements.
//!
//! Terminal Psi has no operation that assembles completed owned elements into
//! an array. A literal whose every element is itself a literal (records,
//! closed primitive arrays, omitted zeros, nested such arrays) needs none:
//! its whole value is a roster of scalar leaves. Every leaf evaluates in
//! authored order through the ordinary scalar route; the array then
//! establishes as one `EstablishZeroedStructural` and each authored leaf
//! stores into it, record leaves through `StructuralScalarFieldStore` (which
//! owns a bounded field's range obligation) and primitive-array leaves
//! through `WriteOnlyPrimitiveStore`. Omitted members keep the zero the
//! establishment wrote. Element places and call products would transfer an
//! existing owner, so they keep the element-custody refusal.
use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, OperationResult, PlaceId,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, allocate_dense, obligation_id, place_id, unsupported,
};
use super::emission;
use checked_trees::{
    CheckedStructuralRecordFieldValue, CheckedStructuralValueHandle, CheckedStructuralValueKind,
};
use terminal_psi::StructuralPathSegment;

/// One authored leaf's destination beneath the zeroed array, and the slot in
/// the emission's value list where its evaluated scalar waits.
enum LeafStore {
    Field {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        bounded: bool,
        position: usize,
    },
    Primitive {
        path: Vec<StructuralPathSegment>,
        position: usize,
    },
}

impl emission::Emission<'_, '_, '_> {
    pub(super) fn fixed_array(
        &mut self,
        elements: &[CheckedStructuralValueHandle],
    ) -> Result<PlaceId, LoweringError> {
        if !terminal_semantics::zeroed_structural_shape(
            self.structural_types.iter(),
            self.structural_type,
        ) {
            return unsupported("fixed array literal has no Terminal establishment");
        }
        let StructuralTypeShape::FixedArray { element, length } =
            self.shape(self.structural_type)?.clone()
        else {
            return unsupported("fixed array literal has a nonarray carrier");
        };
        if u64::try_from(elements.len()).ok() != Some(length) {
            return unsupported("fixed array literal changed its declared extent");
        }
        let source_count = self.values.len();
        let mut stores = Vec::new();
        for (index, value) in elements.iter().enumerate() {
            let index = u64::try_from(index)
                .map_err(|_| LoweringError::Unsupported("fixed array index overflow"))?;
            self.leaf_stores(
                *value,
                element,
                vec![StructuralPathSegment::FixedIndex(index)],
                source_count,
                &mut stores,
            )?;
        }
        // Private control flow remaps saved scalar values. Read their final
        // slots only after the last leaf has evaluated.
        let place = self.establish_zeroed()?;
        for store in stores {
            let kind = match store {
                LeafStore::Field {
                    path,
                    field,
                    bounded,
                    position,
                } => OperationKind::StructuralScalarFieldStore {
                    destination: place,
                    path,
                    field,
                    value: self.values[position].id,
                    range_obligation: if bounded {
                        Some(obligation_id(allocate_dense(
                            &mut self.calls.next_obligation_identity,
                        )?))
                    } else {
                        None
                    },
                },
                LeafStore::Primitive { path, position } => OperationKind::WriteOnlyPrimitiveStore {
                    destination: place,
                    path,
                    value: self.values[position].id,
                },
            };
            let operation = self.operations.allocate();
            self.operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: OperationResult::Unit,
                kind,
            });
        }
        self.values.truncate(source_count);
        Ok(place)
    }

    /// Establish the current carrier's whole zero as a fresh owned place.
    pub(super) fn establish_zeroed(&mut self) -> Result<PlaceId, LoweringError> {
        let place = place_id(allocate_dense(self.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: self.structural_type,
                multiplicity: self.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishZeroedStructural {},
        });
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: self.structural_type,
            },
        });
        Ok(place)
    }

    fn shape(
        &self,
        structural_type: StructuralTypeId,
    ) -> Result<&StructuralTypeShape, LoweringError> {
        self.structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .map(|declaration| &declaration.shape)
            .ok_or(LoweringError::Unsupported(
                "fixed array member structural type missing",
            ))
    }

    /// Evaluate one literal member's authored leaves in order and record where
    /// each lands beneath the array root.
    fn leaf_stores(
        &mut self,
        value: CheckedStructuralValueHandle,
        structural_type: StructuralTypeId,
        path: Vec<StructuralPathSegment>,
        source_count: usize,
        stores: &mut Vec<LeafStore>,
    ) -> Result<(), LoweringError> {
        let node = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(value)
            .clone();
        match node.kind {
            CheckedStructuralValueKind::ZeroedStructural {}
            | CheckedStructuralValueKind::ZeroedScalarArray { .. } => Ok(()),
            CheckedStructuralValueKind::FixedArray { elements } => {
                let StructuralTypeShape::FixedArray { element, length } =
                    self.shape(structural_type)?.clone()
                else {
                    return unsupported("nested fixed array literal has a nonarray carrier");
                };
                if u64::try_from(elements.len()).ok() != Some(length) {
                    return unsupported("nested fixed array literal changed its declared extent");
                }
                for (index, value) in elements.into_iter().enumerate() {
                    let mut element_path = path.clone();
                    element_path.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(index).map_err(|_| {
                            LoweringError::Unsupported("fixed array index overflow")
                        })?,
                    ));
                    self.leaf_stores(value, element, element_path, source_count, stores)?;
                }
                Ok(())
            }
            CheckedStructuralValueKind::ScalarArray { elements } => {
                let mut extents = Vec::new();
                let mut carrier = structural_type;
                while let StructuralTypeShape::FixedArray { element, length } =
                    self.shape(carrier)?.clone()
                {
                    extents.push(length);
                    carrier = element;
                }
                if !matches!(
                    self.shape(carrier)?,
                    StructuralTypeShape::PrimitiveScalar(_)
                ) || extents
                    .iter()
                    .try_fold(1u64, |count, extent| count.checked_mul(*extent))
                    != u64::try_from(elements.len()).ok()
                {
                    return unsupported("primitive array member changed its declared leaves");
                }
                for (ordinal, element) in elements.into_iter().enumerate() {
                    let evaluated = self.scalar(
                        CheckedScalarExpressionRole::StructuralArrayElement {
                            expression: node.expression,
                            element_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("scalar array leaf ordinal overflow")
                            })?,
                        },
                        element,
                        source_count,
                    )?;
                    // Row-major: the last extent varies fastest.
                    let mut indices = Vec::with_capacity(extents.len());
                    let mut remainder = ordinal as u64;
                    for extent in extents.iter().rev() {
                        indices.push(StructuralPathSegment::FixedIndex(remainder % extent));
                        remainder /= extent;
                    }
                    let mut leaf_path = path.clone();
                    leaf_path.extend(indices.into_iter().rev());
                    stores.push(LeafStore::Primitive {
                        path: leaf_path,
                        position: self.values.len(),
                    });
                    self.values.push(evaluated);
                }
                Ok(())
            }
            CheckedStructuralValueKind::Record {
                data_symbol,
                fields,
            } => {
                let StructuralTypeShape::Record {
                    fields: shape_fields,
                } = self.shape(structural_type)?.clone()
                else {
                    return unsupported("record array member has a nonrecord shape");
                };
                let fields = self
                    .checked
                    .facts
                    .values
                    .structural_values
                    .record_fields
                    .span(fields)
                    .ok_or(LoweringError::Unsupported("record fields are stale"))?
                    .to_vec();
                let record = self
                    .checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == data_symbol)
                    .ok_or(LoweringError::Unsupported("record declaration missing"))?;
                let declarations = self.checked.data_members(record);
                for (ordinal, field) in fields.iter().enumerate() {
                    let identity = declarations
                        .iter()
                        .find_map(|member| match member {
                            checked_trees::data::DataMember::Field(declaration)
                                if declaration.symbol == field.field =>
                            {
                                Some(declaration.path_identity())
                            }
                            _ => None,
                        })
                        .ok_or(LoweringError::Unsupported(
                            "record field declaration missing",
                        ))?;
                    let target = shape_fields
                        .iter()
                        .find(|candidate| candidate.identity == identity)
                        .ok_or(LoweringError::Unsupported("record field identity missing"))?;
                    match field.value {
                        CheckedStructuralRecordFieldValue::Scalar(value) => {
                            let evaluated = self.scalar(
                                CheckedScalarExpressionRole::RecordField {
                                    expression: node.expression,
                                    field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                        LoweringError::Unsupported("record field ordinal overflow")
                                    })?,
                                },
                                value,
                                source_count,
                            )?;
                            if !matches!(
                                target.field_type,
                                StructuralFieldType::Scalar(_)
                                    | StructuralFieldType::IeeeFloat(_)
                                    | StructuralFieldType::BoundedInteger(_)
                                    | StructuralFieldType::RefinedScalar(_)
                            ) || target.field_type.scalar_type() != Some(evaluated.scalar_type)
                            {
                                return unsupported(
                                    "record operand requires its exact scalar carrier and range evidence",
                                );
                            }
                            stores.push(LeafStore::Field {
                                path: path.clone(),
                                field: target.id,
                                bounded: terminal_semantics::scalar_field_requires_evidence(
                                    &target.field_type,
                                ),
                                position: self.values.len(),
                            });
                            self.values.push(evaluated);
                        }
                        CheckedStructuralRecordFieldValue::Structural(child) => {
                            let StructuralFieldType::Structural(child_type) = target.field_type
                            else {
                                // An omitted scalar member keeps the zero the
                                // whole establishment wrote.
                                if target.field_type.scalar_type().is_some()
                                    && matches!(
                                        self.checked
                                            .facts
                                            .values
                                            .structural_values
                                            .nodes
                                            .get(child)
                                            .kind,
                                        CheckedStructuralValueKind::ZeroedStructural { .. }
                                    )
                                {
                                    continue;
                                }
                                return unsupported(
                                    "record array member field has no scalar-leaf establishment",
                                );
                            };
                            let mut child_path = path.clone();
                            child_path.push(StructuralPathSegment::Field(identity));
                            self.leaf_stores(child, child_type, child_path, source_count, stores)?;
                        }
                    }
                }
                Ok(())
            }
            _ => unsupported("fixed array literal has no Terminal establishment"),
        }
    }
}
