//! Prepared scalar expressions, qualification lookup, and ordered leaf emission.

use super::boolean::{LoweredBooleanReturnExpression, emit_boolean_expression};
use super::buffer::OperationBuffer;
use super::integer::LoweredIntegerBinaryKind;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::terminal_identities::obligation_id;
use crate::terminal_identities::place_id;
use crate::terminal_identities::value_id;
use semantic_vocabulary::{
    IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId, QualifiedScalarType,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

/// Reserved place-identity namespace for element hoists minted inside
/// operation emission: the dense place counter the machine frames own
/// cannot reach it, so a copy's fresh place never collides with declared
/// places regardless of emission order.
const TERMINAL_HOISTED_ELEMENT_PLACE_BASE: u64 = 1 << 62;

/// Reserved obligation-identity namespace for the inner selectors of one
/// nested element read (`self.a[i][k]`). A read owns the obligation after its
/// own operation identity for its outer selector, the convention every
/// runtime-indexed read shares; each inner `RuntimeIndex` segment needs a
/// distinct obligation of the same operation, so it takes
/// `base + operation * stride + ordinal` here. The dense operation and
/// contract obligations stay far below this base, and structural Unit call
/// requirements start at `1 << 63`, above it.
const TERMINAL_NESTED_SELECTOR_OBLIGATION_BASE: u64 = 1 << 61;
pub(crate) const TERMINAL_NESTED_SELECTOR_LIMIT: usize = 7;
const TERMINAL_NESTED_SELECTOR_STRIDE: u64 = 8;

/// Case-qualified storage must first be observed under its selecting edge's
/// authority. Copying the leaf's immediate carrier preserves that authority without
/// extending the ordinary primitive projection grammar across overlaid payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrimitiveReadSource {
    Place(PlaceId),
    CaseLeaf {
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        structural_type: StructuralTypeId,
    },
}

impl From<PlaceId> for PrimitiveReadSource {
    fn from(place: PlaceId) -> Self {
        Self::Place(place)
    }
}

impl PrimitiveReadSource {
    /// Canonical origin for symbolic field terms and runtime-input identity.
    /// This is an identity projection, not evidence that the case is active.
    pub(crate) fn field_origin(
        &self,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> (
        PlaceId,
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    ) {
        match self {
            Self::Place(source) => (*source, path.to_vec()),
            Self::CaseLeaf {
                source,
                path: carrier,
                ..
            } => {
                let mut canonical = carrier.clone();
                canonical.extend_from_slice(path);
                (*source, canonical)
            }
        }
    }
}

/// One element-path step of an `IndexedPrimitiveRead` after lowering: a
/// static segment spelled verbatim, or a runtime selector evaluated like an
/// `inner_indexes` operand and emitted as its own `RuntimeIndex` segment in
/// the read's path — `cells[i].data[j]` keeps `data`'s `Field` step between
/// the two runtime selectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredElementPathSegment {
    /// A statically spelled path step.
    Static(terminal_psi::StructuralPathSegment),
    /// A runtime selector emitted with its own `index < extent` obligation.
    RuntimeIndex {
        selector: LoweredDirectExpression,
        extent: Option<LoweredDirectExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredDirectExpression {
    PrimitiveRead {
        source: PrimitiveReadSource,
        path: Vec<terminal_psi::StructuralPathSegment>,
        scalar_type: ScalarType,
    },
    StructuralField {
        source: PrimitiveReadSource,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
        scalar_type: ScalarType,
    },
    ByteSequenceLength {
        source: PlaceId,
        scalar_type: ScalarType,
    },
    ByteSequenceFieldLength {
        source: PlaceId,
        path: Vec<LoweredElementPathSegment>,
        field: StructuralFieldId,
        scalar_type: ScalarType,
    },
    ByteSequenceRead {
        source: PlaceId,
        index: Box<LoweredDirectExpression>,
        scalar_type: ScalarType,
    },
    ByteSequenceFieldRead {
        source: PlaceId,
        path: Vec<LoweredElementPathSegment>,
        field: StructuralFieldId,
        index: Box<LoweredDirectExpression>,
        scalar_type: ScalarType,
    },
    /// Read one scalar of a runtime-selected fixed-array element: `path`
    /// resolves from `source` to the array itself and `index` is the runtime
    /// `u64` selector, evaluated first and then spelled as the read's
    /// `RuntimeIndex` segment. `element_path` is empty for a primitive
    /// element; for a record element it follows the selector down to one
    /// scalar field (`cells[i].hp`), which the verifier resolves against the
    /// element's declaration.
    /// `inner_indexes` select again, in order, inside a selected element
    /// that is itself a fixed array; each becomes its own `RuntimeIndex`
    /// segment after the outer one.
    IndexedPrimitiveRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        index: Box<LoweredDirectExpression>,
        inner_indexes: Vec<LoweredDirectExpression>,
        element_path: Vec<LoweredElementPathSegment>,
        scalar_type: ScalarType,
    },
    /// A bounded-leaf field of a runtime-selected record element
    /// (`cells[i].v` where `v` declares a range). The runtime read cannot
    /// carry the bound, so emission hoists: a `StructuralLeafCopy` at the
    /// `RuntimeIndex` materializes the element on a fresh place, and the
    /// canonical field read on that place supplies the declared range.
    /// `path` is the static prefix ending at the fixed array, `carrier` the
    /// element path's structural prefix, `field` its leaf, and `element`
    /// the materialized element's structural type.
    IndexedElementFieldRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        index: Box<LoweredDirectExpression>,
        element: StructuralTypeId,
        carrier: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
        scalar_type: ScalarType,
    },
    ElementViewLength {
        source: PlaceId,
        scalar_type: ScalarType,
    },
    /// One scalar of one element of an established view: the element itself
    /// when `path` is empty, or the leaf `path` projects to inside a record
    /// element.
    ElementViewRead {
        source: PlaceId,
        index: Box<LoweredDirectExpression>,
        path: Vec<terminal_psi::StructuralPathSegment>,
        scalar_type: ScalarType,
    },
    Parameter {
        position: usize,
        scalar_type: ScalarType,
    },
    /// Proof-only reference to one erased formal in the enclosing contract
    /// roster. Runtime emission and evaluation reject this form; only scalar
    /// contract terms may carry it.
    ErasedParameter {
        position: usize,
        scalar_type: ScalarType,
    },
    Local {
        position: usize,
        scalar_type: ScalarType,
    },
    IntegerLiteral {
        value: IntegerValue,
        scalar_type: ScalarType,
    },
    IeeeFloatLiteral {
        value: IeeeFloatValue,
    },
    IntegerBinary {
        kind: LoweredIntegerBinaryKind,
        scalar_type: ScalarType,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    /// Composite arithmetic whose operands must be evaluated once before
    /// the count is reused by the two positive scale factors.
    SaturatingShiftLeft {
        scalar_type: ScalarType,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    IntegerBitwiseNot {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerWiden {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerExactCast {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    /// One place's stored bits re-viewed at `scalar_type`, a carrier of the
    /// same width in another family (`ScalarRepresentationView`). An integer
    /// landing on `bool` emits its `{0, 1}` landing obligation; every other
    /// admitted pair is total.
    RepresentationView {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    /// Selected Trapping conversion: the operand's exact value when it is
    /// representable in `scalar_type`, otherwise a `Trap` at this operation.
    IntegerTrappingCast {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    Boolean {
        expression: Box<LoweredBooleanReturnExpression>,
    },
}

impl LoweredDirectExpression {
    pub(crate) fn value_type(
        &self,
        parameters: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        let position = match self {
            Self::Parameter { position, .. } | Self::Local { position, .. } => Some(*position),
            Self::Boolean { expression } => match expression.as_ref() {
                LoweredBooleanReturnExpression::Parameter { position }
                | LoweredBooleanReturnExpression::Local { position } => Some(*position),
                _ => None,
            },
            _ => None,
        };
        if let Some(position) = position {
            let value_type =
                parameters
                    .get(position)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "scalar value has no typed parameter",
                    ))?;
            if value_type.scalar_type != self.scalar_type() {
                return unsupported("scalar value carrier disagrees with its parameter");
            }
            Ok(value_type)
        } else {
            Ok(self.scalar_type().into())
        }
    }

    pub(crate) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. }
            | Self::ErasedParameter { scalar_type, .. }
            | Self::PrimitiveRead { scalar_type, .. }
            | Self::StructuralField { scalar_type, .. }
            | Self::ByteSequenceLength { scalar_type, .. }
            | Self::ByteSequenceFieldLength { scalar_type, .. }
            | Self::ByteSequenceRead { scalar_type, .. }
            | Self::ByteSequenceFieldRead { scalar_type, .. }
            | Self::IndexedPrimitiveRead { scalar_type, .. }
            | Self::IndexedElementFieldRead { scalar_type, .. }
            | Self::ElementViewLength { scalar_type, .. }
            | Self::ElementViewRead { scalar_type, .. }
            | Self::Local { scalar_type, .. }
            | Self::IntegerLiteral { scalar_type, .. }
            | Self::IntegerBinary { scalar_type, .. }
            | Self::SaturatingShiftLeft { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerWiden { scalar_type, .. }
            | Self::IntegerExactCast { scalar_type, .. }
            | Self::RepresentationView { scalar_type, .. }
            | Self::IntegerTrappingCast { scalar_type, .. } => *scalar_type,
            Self::IeeeFloatLiteral { value } => ScalarType::IeeeFloat(value.format()),
            Self::Boolean { .. } => ScalarType::Boolean,
        }
    }
}

pub(crate) fn emit_scalar_leaf(
    kind: OperationKind,
    scalar_type: ScalarType,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    let id = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .expect("generated value identity advances after a scalar leaf");
    let operation = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type,
        }),
        kind,
    });
    id
}

pub(crate) fn emit_byte_length(
    source: PlaceId,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    // Immutable descriptor extent is unchanged on this emission path. Reuse
    // its dominating observation so later bounds retain the selected guard's
    // exact value identity; branch emitters scope this cache to their path.
    if let Some(value) = operations
        .byte_lengths
        .iter()
        .rev()
        .find_map(|(place, value)| (*place == source).then_some(*value))
    {
        return value;
    }
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    let value = emit_scalar_leaf(
        OperationKind::ByteSequenceLength { source },
        scalar_type,
        next_value_identity,
        operations,
    );
    operations.byte_lengths.push((source, value));
    value
}

pub(crate) fn emit_element_length(
    source: PlaceId,
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    // Immutable descriptor extent is unchanged on this emission path. Reuse
    // its dominating observation so later bounds retain the selected guard's
    // exact value identity; branch emitters scope this cache to their path.
    if let Some(value) = operations
        .element_lengths
        .iter()
        .rev()
        .find_map(|(place, value)| (*place == source).then_some(*value))
    {
        return value;
    }
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    let value = emit_scalar_leaf(
        OperationKind::ElementViewLength { source },
        scalar_type,
        next_value_identity,
        operations,
    );
    operations.element_lengths.push((source, value));
    value
}

/// Runtime selectors use the same per-operation namespace across primitive and
/// byte-field observations. A field length and its read may share selectors,
/// but each operation owns its own independently reconstructed bounds.
fn selector_obligation(operation: u64, ordinal: usize) -> semantic_vocabulary::ObligationId {
    assert!(
        ordinal < TERMINAL_NESTED_SELECTOR_LIMIT,
        "preparation bounds the nested selector count"
    );
    obligation_id(
        operation
            .checked_mul(TERMINAL_NESTED_SELECTOR_STRIDE)
            .and_then(|offset| offset.checked_add(TERMINAL_NESTED_SELECTOR_OBLIGATION_BASE))
            .and_then(|base| base.checked_add(ordinal as u64 + 1))
            .expect("nested selector obligations fit their reserved namespace"),
    )
}

pub(crate) fn emit_prepared_path(
    path: &[LoweredElementPathSegment],
    first_ordinal: usize,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> Vec<terminal_psi::StructuralPathSegment> {
    let mut ordinal = first_ordinal;
    path.iter()
        .map(|segment| match segment {
            LoweredElementPathSegment::Static(segment) => segment.clone(),
            LoweredElementPathSegment::RuntimeIndex { selector, extent } => {
                let index =
                    emit_direct_expression(selector, parameters, next_value_identity, operations);
                if let Some(extent) = extent {
                    emit_direct_expression(extent, parameters, next_value_identity, operations);
                }
                let obligation = selector_obligation(operations.next_identity, ordinal);
                ordinal += 1;
                terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation }
            }
        })
        .collect()
}

pub(crate) fn path_with_current_bounds(
    path: &[terminal_psi::StructuralPathSegment],
    operation: u64,
) -> Vec<terminal_psi::StructuralPathSegment> {
    let mut ordinal = 0;
    path.iter()
        .map(|segment| match segment {
            terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } => {
                let obligation = selector_obligation(operation, ordinal);
                ordinal += 1;
                terminal_psi::StructuralPathSegment::RuntimeIndex {
                    index: *index,
                    obligation,
                }
            }
            segment => segment.clone(),
        })
        .collect()
}

/// Establish the selected carrier at the observation point. Both primitive and
/// declared-field reads use this operation; caller shape supplies no authority.
/// The emitted case-leaf copy independently owes the current active-case fact.
pub(super) fn emit_read_source(
    source: &PrimitiveReadSource,
    operations: &mut OperationBuffer,
) -> PlaceId {
    match source {
        PrimitiveReadSource::Place(place) => *place,
        PrimitiveReadSource::CaseLeaf {
            source,
            path,
            structural_type,
        } => {
            let producer = operations.allocate();
            let place = place_id(
                TERMINAL_HOISTED_ELEMENT_PLACE_BASE
                    .checked_add(producer.get())
                    .expect("hoisted leaf places admit a reserved namespace"),
            );
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: producer,
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place,
                    structural_type: *structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::StructuralCaseLeafCopy {
                    source: *source,
                    path: path.clone(),
                },
            });
            place
        }
    }
}

pub(crate) fn emit_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredDirectExpression::PrimitiveRead {
            source,
            path,
            scalar_type,
        } => {
            let source = emit_read_source(source, operations);
            emit_scalar_leaf(
                OperationKind::PrimitiveScalarRead {
                    source,
                    path: path.clone(),
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::StructuralField {
            source,
            path,
            field,
            scalar_type,
        } => {
            let source = emit_read_source(source, operations);
            emit_scalar_leaf(
                OperationKind::IntegerStructuralField {
                    source,
                    path: path.clone(),
                    field: *field,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::ErasedParameter { .. } => {
            unreachable!("erased formal is proof-only and has no runtime operand")
        }
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => parameters[*position].id,
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => emit_scalar_leaf(
            OperationKind::IntegerConstant { value: *value },
            *scalar_type,
            next_value_identity,
            operations,
        ),
        LoweredDirectExpression::ByteSequenceLength { source, .. } => {
            emit_byte_length(*source, next_value_identity, operations)
        }
        LoweredDirectExpression::ByteSequenceFieldLength {
            source,
            path,
            field,
            scalar_type,
        } => {
            let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
            let path = path_with_current_bounds(&path, operations.next_identity);
            // Field length is live storage metadata, not a whole-view extent.
            // Emit at this occurrence; replacement and calls can change it.
            // Record the observation so an indexed read on the same field
            // binds its bound against this exact dominating value identity.
            let value = emit_scalar_leaf(
                OperationKind::StructuralByteSequenceFieldLength {
                    source: *source,
                    path: path.clone(),
                    field: *field,
                },
                *scalar_type,
                next_value_identity,
                operations,
            );
            operations
                .field_byte_lengths
                .push((*source, path.clone(), *field, value));
            value
        }
        LoweredDirectExpression::ByteSequenceRead {
            source,
            index,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            let length = operations
                .byte_lengths
                .iter()
                .rev()
                .find_map(|(place, value)| (*place == *source).then_some(*value))
                .unwrap_or_else(|| emit_byte_length(*source, next_value_identity, operations));
            // A missing dominating observation may still form a valid read
            // shape. Its canonical bounds certificate must then be produced;
            // constructing a fresh length does not prove the read is in bounds.
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            emit_scalar_leaf(
                OperationKind::ByteSequenceRead {
                    source: *source,
                    index,
                    length,
                    obligation,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::ByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            scalar_type,
        } => {
            // Evaluate the carrier before the byte selector, once each. Length
            // and read share these exact SSA selectors, not replayed expressions.
            let path = emit_prepared_path(path, 0, parameters, next_value_identity, operations);
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            // A bound proven against an earlier field `.len` observation
            // only discharges this read when the operand names that same
            // dominating value; otherwise mint a current observation, whose
            // bound the certificate must then derive from the extent itself.
            let length = operations
                .field_byte_lengths
                .iter()
                .rev()
                .find_map(|(place, carrier, named, value)| {
                    (*place == *source
                        && terminal_semantics::structural_paths_select_same_location(
                            carrier, &path,
                        )
                        && *named == *field)
                        .then_some(*value)
                })
                .unwrap_or_else(|| {
                    emit_scalar_leaf(
                        OperationKind::StructuralByteSequenceFieldLength {
                            source: *source,
                            path: path_with_current_bounds(&path, operations.next_identity),
                            field: *field,
                        },
                        ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
                        ),
                        next_value_identity,
                        operations,
                    )
                });
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            emit_scalar_leaf(
                OperationKind::StructuralByteSequenceFieldRead {
                    source: *source,
                    path: path_with_current_bounds(&path, operations.next_identity),
                    field: *field,
                    index,
                    length,
                    obligation,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::IndexedPrimitiveRead {
            source,
            path,
            index,
            inner_indexes,
            element_path,
            scalar_type,
        } => {
            // Selectors evaluate in authored order, outermost array first.
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            let inner_indexes = inner_indexes
                .iter()
                .map(|inner| {
                    emit_direct_expression(inner, parameters, next_value_identity, operations)
                })
                .collect::<Vec<_>>();
            // The certificate proves `index < declared extent` against the
            // array's statically declared length, so no length observation is
            // needed: the verifier resolves the extent from the prefix.
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            let mut path = path.clone();
            path.push(terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation });
            let inner_count = inner_indexes.len();
            for (ordinal, index) in inner_indexes.into_iter().enumerate() {
                let obligation = selector_obligation(operations.next_identity, ordinal);
                path.push(terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation });
            }
            path.extend(emit_prepared_path(
                element_path,
                inner_count,
                parameters,
                next_value_identity,
                operations,
            ));
            emit_scalar_leaf(
                OperationKind::PrimitiveScalarRead {
                    source: *source,
                    path,
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::IndexedElementFieldRead {
            source,
            path,
            index,
            element,
            carrier,
            field,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            // Same obligation convention as the flat indexed read: the
            // certificate proves `index < declared extent` against the
            // array's statically declared length.
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("copy obligation follows its operation identity"),
            );
            let mut path = path.clone();
            path.push(terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation });
            let operation = operations.allocate();
            // The copy's place borrows the reserved high namespace so it
            // cannot collide with the dense place counter the machine
            // frames own.
            let place = place_id(
                TERMINAL_HOISTED_ELEMENT_PLACE_BASE
                    .checked_add(operation.get())
                    .expect("hoisted element places admit a reserved namespace"),
            );
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    qualification_establishments: Vec::new(),
                    place,
                    structural_type: *element,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::StructuralLeafCopy {
                    source: *source,
                    path,
                },
            });
            let kind = match scalar_type {
                ScalarType::Boolean => OperationKind::BooleanStructuralField {
                    source: place,
                    path: carrier.clone(),
                    field: *field,
                },
                _ => OperationKind::IntegerStructuralField {
                    source: place,
                    path: carrier.clone(),
                    field: *field,
                },
            };
            emit_scalar_leaf(kind, *scalar_type, next_value_identity, operations)
        }
        LoweredDirectExpression::ElementViewLength { source, .. } => {
            emit_element_length(*source, next_value_identity, operations)
        }
        LoweredDirectExpression::ElementViewRead {
            source,
            index,
            path,
            scalar_type,
        } => {
            let index = emit_direct_expression(index, parameters, next_value_identity, operations);
            let length = operations
                .element_lengths
                .iter()
                .rev()
                .find_map(|(place, value)| (*place == *source).then_some(*value))
                .unwrap_or_else(|| emit_element_length(*source, next_value_identity, operations));
            // A missing dominating observation may still form a valid read
            // shape. Its canonical bounds certificate must then be produced;
            // constructing a fresh length does not prove the read is in bounds.
            let obligation = obligation_id(
                operations
                    .next_identity
                    .checked_add(1)
                    .expect("read obligation follows its operation identity"),
            );
            emit_scalar_leaf(
                OperationKind::ElementViewRead {
                    source: *source,
                    index,
                    length,
                    obligation,
                    path: path.clone(),
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::IeeeFloatLiteral { value } => emit_scalar_leaf(
            OperationKind::IeeeFloatConstant { value: *value },
            ScalarType::IeeeFloat(value.format()),
            next_value_identity,
            operations,
        ),
        LoweredDirectExpression::SaturatingShiftLeft {
            scalar_type,
            left,
            right,
        } => {
            let left_value =
                emit_direct_expression(left, parameters, next_value_identity, operations);
            let right_value =
                emit_direct_expression(right, parameters, next_value_identity, operations);
            let bindings = [
                ValueDeclaration {
                    id: left_value,
                    scalar_type: *scalar_type,
                    qualifications: Default::default(),
                },
                ValueDeclaration {
                    id: right_value,
                    scalar_type: right.scalar_type(),
                    qualifications: Default::default(),
                },
            ];
            let left = LoweredDirectExpression::Parameter {
                position: 0,
                scalar_type: *scalar_type,
            };
            let right = LoweredDirectExpression::Parameter {
                position: 1,
                scalar_type: right.scalar_type(),
            };
            // Right shift has exactly the authored formation condition: a
            // representable count below the value width, without a value-
            // overflow condition. Keep that certificate even though scaling
            // uses total masked shifts. An optimizer may erase this unused
            // result only after independent Terminal verification.
            let count_witness = LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactShiftRight,
                scalar_type: *scalar_type,
                left: Box::new(left.clone()),
                right: Box::new(right.clone()),
            };
            emit_direct_expression(&count_witness, &bindings, next_value_identity, operations);
            let expanded = crate::expression_preparation::saturating_shift::expansion(
                *scalar_type,
                left,
                right,
            );
            emit_direct_expression(&expanded, &bindings, next_value_identity, operations)
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a binary operation");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: kind.operation(operation, left, right),
            });
            id
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after bitwise complement");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerBitwiseNot { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer widening");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerWiden { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after an exact integer cast");
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerExactCast {
                    operand,
                    obligation: obligation_id(
                        operation
                            .get()
                            .checked_add(1)
                            .expect("exact-cast obligation follows its operation identity"),
                    ),
                },
            });
            id
        }
        LoweredDirectExpression::RepresentationView {
            scalar_type,
            operand,
        } => {
            let source_type = operand.scalar_type();
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a representation view");
            let operation = operations.allocate();
            // The landing obligation follows its operation identity exactly as
            // an exact cast's does; replay admitted the pair, so a pair the
            // shared table does not classify cannot reach emission.
            let landing_obligation = terminal_semantics::ScalarRepresentationViewPair::classify(
                source_type,
                *scalar_type,
            )
            .filter(|pair| pair.requires_landing())
            .map(|_| {
                obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("landing obligation follows its operation identity"),
                )
            });
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::ScalarRepresentationView {
                    operand,
                    landing_obligation,
                },
            });
            id
        }
        LoweredDirectExpression::IntegerTrappingCast {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            emit_scalar_leaf(
                OperationKind::TrappingInteger {
                    operation: terminal_psi::TrappingIntegerOperation::Convert { operand },
                },
                *scalar_type,
                next_value_identity,
                operations,
            )
        }
        LoweredDirectExpression::Boolean { expression } => {
            emit_boolean_expression(expression, parameters, next_value_identity, operations)
        }
    }
}

#[cfg(test)]
#[path = "expressions/path_tests.rs"]
mod path_tests;
