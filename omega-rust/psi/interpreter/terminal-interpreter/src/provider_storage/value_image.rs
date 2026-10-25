//! Read one owned value into a staged complete placement image.
//!
//! The lifecycle producer retained the exact schema-field/placement-slot join.
//! Runtime paths select cells by declaration identity; normalized layout names
//! are only keys within the already validated materializer input. Nested values
//! use their retained physical geometry, including authored fragments and array
//! strides. This reader neither moves custody nor writes provider storage.
//! Initialization must commit this image together with the original value's
//! custody; serializing equal bits is not a replacement for that value.
//! Every image payload uses the interpreter's byte-storage owner, including
//! nested field and array scratch. The shared codec writes into that private
//! funded image rather than allocating a second hidden staging image. Layout
//! metadata is not a payload and this is not a generic temporary-memory bound.

use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use evaluation_accounting::EvaluationInvocationAccount;
use layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, IntegerInterpretation,
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
};
use semantic_vocabulary::{AuthoredFieldStorage, IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralArgument, StructuralFieldDeclaration, StructuralPathSegment, StructuralRecordStorage,
    StructuralTypeShape, TerminalPlacementLifecyclePlan,
};

mod decoding;
pub(crate) use decoding::restore;

pub(crate) fn prepare(
    execution: &TerminalExecution<'_>,
    plan: &TerminalPlacementLifecyclePlan,
    source: &StructuralArgument,
    byte_order: ByteOrder,
) -> Result<ByteSequenceStorage, TerminalInterpretError> {
    let value = execution.resolve_live_projection(source.place, &source.path, source.access)?;
    if source.access != terminal_psi::StructuralAccess::Owned
        || value.structural_type != plan.schema
    {
        return Err(TerminalInterpretError::ContentTransformationMismatch(
            "placement image requires its exact owned schema value",
        ));
    }
    let types = execution.program.structural_types();
    let Some(StructuralTypeShape::Record { fields, .. }) = types
        .get(&plan.schema)
        .map(|declaration| &declaration.shape)
    else {
        return Err(malformed());
    };
    require_runtime_fields(execution, fields)?;
    let mut reader = ValueImageReader {
        execution,
        source,
        byte_order,
        path: source.path.clone(),
        active: Vec::new(),
    };
    let mut schemas = Vec::with_capacity(plan.image_fields.len());
    let mut values = Vec::with_capacity(plan.image_fields.len());
    let entries = plan.placement.access().plan().entries();
    if entries.len() != plan.image_fields.len() {
        return Err(malformed());
    }
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
        let (schema, value) = reader.field(field, entry.field(), layout_entry.member_identity)?;
        schemas.push(schema);
        values.push(value);
    }
    materialize(
        plan.placement.layout(),
        &schemas,
        &values,
        byte_order,
        execution.account.as_ref(),
    )
}

struct ValueImageReader<'execution, 'program, 'source> {
    execution: &'execution TerminalExecution<'program>,
    source: &'source StructuralArgument,
    byte_order: ByteOrder,
    path: Vec<StructuralPathSegment>,
    active: Vec<StructuralTypeId>,
}

impl ValueImageReader<'_, '_, '_> {
    fn scalar(&self, scalar: ScalarType) -> Result<ByteSequenceStorage, TerminalInterpretError> {
        // Use the ordinary observation route: an owned record field is not a
        // reference-forming primitive home. The shared reader already joins
        // field namespaces, retained codecs and captured backing correctly.
        let value = self
            .execution
            .read_primitive_place(self.source.place, &self.path, scalar)?;
        let bits = crate::primitive_storage::encoding::encode(scalar, value)?;
        let width = usize::from(
            terminal_semantics::reference_storage_byte_width(scalar).ok_or_else(malformed)?,
        );
        let encoded = match self.byte_order {
            ByteOrder::LittleEndian => bits.to_le_bytes(),
            ByteOrder::BigEndian => bits.to_be_bytes(),
        };
        let bytes = match self.byte_order {
            ByteOrder::LittleEndian => &encoded[..width],
            ByteOrder::BigEndian => &encoded[16 - width..],
        };
        ByteSequenceStorage::copy(bytes, self.execution.account.as_ref())
    }

    fn field(
        &mut self,
        field: &StructuralFieldDeclaration,
        name: &str,
        member_identity: Option<u64>,
    ) -> Result<
        (
            AggregateFieldSchema,
            AggregateFieldValue<ByteSequenceStorage>,
        ),
        TerminalInterpretError,
    > {
        let schema = field_schema(self.execution, field, name, member_identity)?;
        self.path.push(StructuralPathSegment::Field(field.identity));
        let bytes = match field.field_type.scalar_type() {
            Some(scalar) => self.scalar(scalar),
            None => self.structure(field.field_type.as_structural().ok_or_else(malformed)?),
        }?;
        self.path.pop();
        let value = AggregateFieldValue::from_storage(name, bytes)
            .map_err(TerminalInterpretError::ValueImage)?;
        Ok((schema, value))
    }

    fn structure(
        &mut self,
        root: StructuralTypeId,
    ) -> Result<ByteSequenceStorage, TerminalInterpretError> {
        if self.active.contains(&root) || self.active.len() >= 128 {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "owned image has cyclic or excessive structural depth",
            ));
        }
        self.active.push(root);
        let types = self.execution.program.structural_types();
        let declaration = types.get(&root).ok_or_else(malformed)?;
        let (size, alignment) = terminal_semantics::reference_storage_layout(types.values(), root)
            .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                "owned image requires closed declared storage",
            ))?;
        let result = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => self.scalar(*scalar),
            StructuralTypeShape::FixedArray { element, length } => {
                let (stride, _) =
                    terminal_semantics::reference_storage_layout(types.values(), *element)
                        .ok_or_else(malformed)?;
                let mut bytes = zeroed(size, self.execution.account.as_ref())?;
                // Zero-byte arrays retain custody but need no per-element image work.
                if stride != 0 {
                    for index in 0..*length {
                        self.path.push(StructuralPathSegment::FixedIndex(index));
                        let element = self.structure(*element)?;
                        self.path.pop();
                        let start =
                            usize::try_from(index.checked_mul(stride).ok_or_else(malformed)?)
                                .map_err(|_| malformed())?;
                        let end = start
                            .checked_add(element.bytes().len())
                            .ok_or_else(malformed)?;
                        bytes
                            .bytes_mut()
                            .get_mut(start..end)
                            .ok_or_else(malformed)?
                            .copy_from_slice(element.bytes());
                    }
                }
                Ok(bytes)
            }
            StructuralTypeShape::Record { fields, storage } => {
                require_runtime_fields(self.execution, fields)?;
                let layout = record_layout(self.execution, root, fields, storage, size, alignment)?;
                let mut schemas = Vec::new();
                let mut values = Vec::new();
                for field in fields {
                    let name = format!("field:{:?}", field.id);
                    let (schema, value) = self.field(field, &name, None)?;
                    schemas.push(schema);
                    values.push(value);
                }
                materialize(
                    &layout,
                    &schemas,
                    &values,
                    self.byte_order,
                    self.execution.account.as_ref(),
                )
            }
            _ => Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "owned image shape is outside the retained schema reflection contract",
            )),
        };
        self.active.pop();
        result
    }
}

fn field_schema(
    execution: &TerminalExecution<'_>,
    field: &StructuralFieldDeclaration,
    name: &str,
    member_identity: Option<u64>,
) -> Result<AggregateFieldSchema, TerminalInterpretError> {
    let types = execution.program.structural_types();
    let size = match field.field_type.scalar_type() {
        Some(scalar) => terminal_semantics::reference_storage_byte_width(scalar).map(u64::from),
        None => field
            .field_type
            .as_structural()
            .and_then(|child| terminal_semantics::reference_storage_size(types.values(), child)),
    }
    .ok_or_else(malformed)?;
    let repeated =
        field
            .field_type
            .as_structural()
            .and_then(|child| match types.get(&child)?.shape {
                StructuralTypeShape::FixedArray { element, length } => {
                    let (size, alignment) =
                        terminal_semantics::reference_storage_layout(types.values(), element)?;
                    Some((size, alignment, length))
                }
                _ => None,
            });
    match (repeated, member_identity) {
        (Some((size, alignment, count)), Some(identity)) => {
            AggregateFieldSchema::new_repeated_numbered(name, identity, size, alignment, count)
        }
        (Some((size, alignment, count)), None) => {
            AggregateFieldSchema::new_repeated(name, size, alignment, count)
        }
        (None, Some(identity)) => AggregateFieldSchema::new_numbered(name, identity, size),
        (None, None) => AggregateFieldSchema::new(name, size),
    }
    .map_err(TerminalInterpretError::ValueImage)
}

/// Encode and decode adapt the same retained physical geometry. Neither path
/// evaluates a policy or recovers a semantic field from its diagnostic name.
fn record_layout(
    execution: &TerminalExecution<'_>,
    root: StructuralTypeId,
    fields: &[StructuralFieldDeclaration],
    storage: &StructuralRecordStorage,
    size: u64,
    alignment: u64,
) -> Result<LayoutPlanReport, TerminalInterpretError> {
    let types = execution.program.structural_types();
    let mut layout = LayoutPlanReport {
        schema_report_fingerprint: 0,
        entries: Vec::new(),
        offsets: None,
        size: Some(size),
        align: alignment,
    };
    for field in fields {
        let name = format!("field:{:?}", field.id);
        let append = |layout: &mut LayoutPlanReport, placement| {
            layout.entries.push(LayoutFieldEntryReport {
                field: name.clone(),
                member_identity: None,
                placement,
            });
        };
        match (storage, &field.storage) {
            (StructuralRecordStorage::Packed, _) => {
                let region = terminal_semantics::reference_storage_region(
                    types.values(),
                    root,
                    &[StructuralPathSegment::Field(field.identity)],
                )
                .ok_or_else(malformed)?;
                append(
                    &mut layout,
                    LayoutPlacementReport::At {
                        offset: region.byte_offset,
                    },
                );
            }
            (
                StructuralRecordStorage::Authored(_),
                Some(AuthoredFieldStorage::Fragmented(fragments)),
            ) => {
                for fragment in fragments.fragments() {
                    append(
                        &mut layout,
                        LayoutPlacementReport::Bits {
                            container: fragment.container_byte_offset,
                            container_width: fragment.container_width_bits,
                            destination_lsb: fragment.destination_lsb,
                            source_lsb: fragment.source_lsb,
                            width: fragment.width,
                        },
                    );
                }
            }
            (
                StructuralRecordStorage::Authored(_),
                Some(AuthoredFieldStorage::Repeated {
                    byte_offset,
                    element_stride,
                }),
            ) => {
                let child = field
                    .field_type
                    .as_structural()
                    .and_then(|child| types.get(&child))
                    .ok_or_else(malformed)?;
                let StructuralTypeShape::FixedArray { length, .. } = child.shape else {
                    return Err(malformed());
                };
                for index in 0..length {
                    let offset = byte_offset
                        .checked_add(index.checked_mul(*element_stride).ok_or_else(malformed)?)
                        .ok_or_else(malformed)?;
                    append(&mut layout, LayoutPlacementReport::At { offset });
                }
            }
            (
                StructuralRecordStorage::Authored(_),
                Some(AuthoredFieldStorage::Scalar {
                    byte_offset,
                    stored_scalar: Some(ScalarType::Integer(integer)),
                }),
            ) => {
                append(
                    &mut layout,
                    LayoutPlacementReport::IntegerAt {
                        offset: *byte_offset,
                        stored_width: u64::from(integer.bits()),
                        interpretation: match integer.sign() {
                            IntegerSign::Signed => IntegerInterpretation::Signed,
                            IntegerSign::Unsigned => IntegerInterpretation::Unsigned,
                        },
                    },
                );
            }
            (StructuralRecordStorage::Authored(_), Some(placement)) => {
                append(
                    &mut layout,
                    LayoutPlacementReport::At {
                        offset: placement.byte_offset().ok_or_else(malformed)?,
                    },
                );
            }
            _ => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "owned image record has no resolved storage recipe",
                ));
            }
        }
    }
    Ok(layout)
}

/// The current private primitive has no authored PlacementCustody argument.
/// Retaining a snapshot would conserve these fields, but cannot substitute for
/// the specification's explicit agreement. Refuse before initialization writes
/// until that agreement has a checked producer and a retained consumer. This
/// also keeps skipped reference/loan custody from disappearing into a private
/// resident snapshot outside the ordinary live-loan map.
fn require_runtime_fields(
    execution: &TerminalExecution<'_>,
    fields: &[StructuralFieldDeclaration],
) -> Result<(), TerminalInterpretError> {
    let types = execution.program.structural_types();
    for field in fields {
        if field.relevance.is_erased()
            || field.field_type.as_structural().is_some_and(|child| {
                terminal_semantics::reference_storage_size(types.values(), child) == Some(0)
            })
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "placement with non-runtime fields requires an authored custody agreement",
            ));
        }
    }
    Ok(())
}

fn zeroed(
    size: u64,
    account: Option<&EvaluationInvocationAccount>,
) -> Result<ByteSequenceStorage, TerminalInterpretError> {
    let length = usize::try_from(size).map_err(|_| malformed())?;
    ByteSequenceStorage::zeroed(length, account)
}

fn materialize(
    layout: &LayoutPlanReport,
    schemas: &[AggregateFieldSchema],
    values: &[AggregateFieldValue<ByteSequenceStorage>],
    byte_order: ByteOrder,
    account: Option<&EvaluationInvocationAccount>,
) -> Result<ByteSequenceStorage, TerminalInterpretError> {
    let mut bytes = zeroed(layout.size.ok_or_else(malformed)?, account)?;
    layout_plans::stage_aggregate_layout(layout, schemas, values, byte_order, bytes.bytes_mut())
        .map_err(TerminalInterpretError::ValueImage)?;
    Ok(bytes)
}
