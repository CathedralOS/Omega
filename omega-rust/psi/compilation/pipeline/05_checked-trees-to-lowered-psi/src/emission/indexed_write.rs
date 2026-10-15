//! Runtime-indexed scalar writes through a `&mut` byte view, a bounded byte
//! field, or a `&mut [T]` element view.
//!
//! The three leaves emit one shape from their plan rows: the checked index
//! expression, the stored value (a pure `AssignmentValue` expression, or the
//! scalar result of the statement's own call), and the index and byte
//! predicate judgments checking recorded on the row (`decisions`) against
//! the authored indexed target. The operation frame captures the index as a
//! Terminal `u64` coordinate before evaluating the RHS and forwards it through
//! scalar continuations. Emission observes the destination's length and
//! ends in a write whose obligation retains the index and any target field
//! bounds through the shared Terminal judgment; value and obligation
//! identities are allocated in exactly that order.
//!
//! What differs stays with the leaf: its destination check, the stored
//! value's type (`u8`, or the view's element), its length observation
//! (`ByteSequenceLength`, the byte field's reused or fresh field length, or
//! `ElementViewLength`), its final operation, and its diagnostic wording.
use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees, LoweringError,
    OperationKind, PlaceId, PrimitiveType, ScalarType, StructuralFieldId, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, ValueDeclaration, ValueId, allocate_dense,
    direct_expression_contains_short_circuit, emit_direct_expression, emit_u64_coordinate,
    obligation_id, terminal_scalar_type, unsupported, validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, scalar_result_declaration};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::{
    CheckedIndexedWriteDecisions, CheckedIntegerAssignmentDecision, CheckedScalarStoreValue,
    CheckedStoreLeaf, CheckedStorePlan,
};
use symbols::SymbolHandle;

/// The indexed leaf one write stores into, read from its store plan: the
/// selector, the stored scalar and the decisions checking recorded.
#[derive(Clone, Copy)]
pub(crate) struct IndexedWriteLeaf<'a> {
    statement_index: u32,
    index: &'a CheckedScalarExpression,
    role: CheckedScalarExpressionRole,
    value: &'a CheckedScalarStoreValue,
    decisions: CheckedIndexedWriteDecisions,
    /// A view element's declared primitive and receiving-range judgment;
    /// `None` for a byte, which stores a `u8`.
    element: Option<(
        PrimitiveType,
        arena::Handle<CheckedIntegerAssignmentDecision>,
    )>,
}

/// One indexed write as the ordered frame emits it: the store with the
/// lowered storage its indexed carrier resolves in.
#[derive(Clone, Copy)]
pub(crate) enum IndexedWriteTarget<'a> {
    /// The frame resolves the admitted root once, whether its backing is an
    /// owned home or a mutable borrowed view. Both use the same byte operation.
    Bytes {
        store: &'a CheckedStorePlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    /// A bounded byte field below a parameter named by authored position,
    /// which `destination` resolves in the same roster.
    ByteField(&'a CheckedStorePlan),
    /// The frame already resolved the view's home place and type from its
    /// parameter or local root.
    ElementView {
        store: &'a CheckedStorePlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

/// Source decisions remain independent: an in-bounds index supplies the count
/// conversion, but cannot authorize a value that violates a byte predicate.
pub(crate) struct IndexedWriteDecisions {
    pub(crate) index: bool,
    pub(crate) write: bool,
}

/// Emission's operand rejection for one carrier. The byte carriers'
/// destination custody checks report it too.
struct Wording {
    operands: &'static str,
}

const BYTE_VIEW: Wording = Wording {
    operands: "byte-view write requires exact mutable byte custody and scalar operands",
};

const BYTE_FIELD: Wording = Wording {
    operands: "byte replacement requires bounded bytes and exact branch-free operands",
};

const ELEMENT_VIEW: Wording = Wording {
    operands: "element-view write requires scalar operands",
};

impl<'a> IndexedWriteLeaf<'a> {
    /// The two independent decisions checking recorded on the plan: the
    /// index decides count conversion and plain element bounds; byte
    /// predicates additionally require the completed write-preservation mark.
    pub(crate) fn source_decisions(self) -> IndexedWriteDecisions {
        IndexedWriteDecisions {
            index: self.decisions.index_in_bounds,
            write: self.decisions.predicates_preserved,
        }
    }

    pub(crate) fn statement_index(self) -> u32 {
        self.statement_index
    }

    pub(crate) fn index(self) -> &'a CheckedScalarExpression {
        self.index
    }

    /// Every leaf's value is the one shared store vocabulary.
    pub(crate) fn value(self) -> &'a CheckedScalarStoreValue {
        self.value
    }

    /// The stored value's type as the plan records it.
    pub(crate) fn value_type(self) -> PrimitiveType {
        self.element
            .map_or(PrimitiveType::U8, |(element, _)| element)
    }

    /// The store's value as one direct expression; a computation is
    /// evaluated by the operation frame through the shared scalar evaluator
    /// and supplied to [`emit`] as an established value instead.
    pub(crate) fn lower_value(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        bindings: &ScalarBindings,
        checked: &CheckedTrees,
        state: SymbolHandle,
        values: &[ValueDeclaration],
    ) -> Result<LoweredDirectExpression, LoweringError> {
        crate::emission::structural_scalar_store_source::direct_value(
            identities,
            checked,
            state,
            self.statement_index,
            self.role,
            self.value,
            terminal_scalar_type(self.value_type())?,
            bindings,
            values,
        )
    }
}

/// A leaf's destination after its emission-time check.
pub(crate) struct Destination {
    place: PlaceId,
    pub(crate) descriptor_steps: Vec<super::runtime_elements::ProjectionStep>,
    /// The Terminal scalar type the stored value must carry.
    value_type: ScalarType,
    shape: DestinationShape,
}

/// What a leaf's length observation and final write name besides the place.
enum DestinationShape {
    ByteView,
    ByteField {
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
    },
    ElementView {
        descriptor: terminal_psi::StructuralArgument,
        path: Vec<StructuralPathSegment>,
        leaf: terminal_semantics::ElementViewLeaf,
    },
}

impl<'a> IndexedWriteTarget<'a> {
    pub(crate) fn store(self) -> &'a CheckedStorePlan {
        match self {
            Self::ByteField(store) => store,
            Self::Bytes { store, .. } | Self::ElementView { store, .. } => store,
        }
    }

    fn wording(self) -> &'static Wording {
        match self {
            Self::Bytes { .. } => &BYTE_VIEW,
            Self::ByteField(_) => &BYTE_FIELD,
            Self::ElementView { .. } => &ELEMENT_VIEW,
        }
    }

    /// The store's indexed leaf: a byte element or a view element.
    pub(crate) fn leaf(self) -> Result<IndexedWriteLeaf<'a>, LoweringError> {
        let store = self.store();
        let Some((role, value)) = store.value.scalar() else {
            return unsupported("indexed write stores no scalar value");
        };
        let (index, decisions, element) = match &store.destination.leaf {
            CheckedStoreLeaf::ByteElement { index, decisions } => (index, *decisions, None),
            CheckedStoreLeaf::ViewElement {
                index,
                element,
                range_decision,
                decisions,
                ..
            } => (index, *decisions, Some((*element, *range_decision))),
            _ => return unsupported("indexed write names no indexed element"),
        };
        Ok(IndexedWriteLeaf {
            statement_index: store.statement_index,
            index,
            role,
            value,
            decisions,
            element,
        })
    }

    /// The leaf's own emission-time destination check. A byte leaf's
    /// custody failure reports the leaf's operand wording, which has always
    /// covered custody and operands together.
    ///
    /// `parameters` pairs each authored parameter position with the
    /// declaration it lowered into. A byte leaf's root is authored; the
    /// declaration's own `position` can be dense (the ordinary machine
    /// counts structural parameters only), so a scalar formal ahead of the
    /// root would make a lookup by declaration position select the next
    /// borrowed parameter, or none.
    pub(crate) fn destination(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        parameters: &[(u32, StructuralParameterDeclaration)],
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<Destination, LoweringError> {
        let operands = self.wording().operands;
        match self {
            Self::Bytes {
                place,
                structural_type,
                ..
            } => {
                if !structural_types.iter().any(|declaration| {
                    declaration.id == structural_type
                        && matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
                }) {
                    return unsupported("byte write root has no byte carrier");
                }
                Ok(Destination {
                    place,
                    descriptor_steps: Vec::new(),
                    value_type: terminal_scalar_type(PrimitiveType::U8)?,
                    shape: DestinationShape::ByteView,
                })
            }
            Self::ByteField(store) => {
                let Some(position) = store.destination.root.parameter_position() else {
                    return unsupported("byte replacement destination has no parameter root");
                };
                let (carrier_path, field_identity) = store.destination.field().ok_or(
                    LoweringError::Unsupported("byte replacement destination names no byte field"),
                )?;
                let (_, parameter) = parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte replacement destination is absent",
                    ))?;
                let (path, field) =
                    crate::emission::structural_scalar_store::lower_structural_field_place(
                        identities,
                        carrier_path,
                        field_identity,
                        parameter,
                        structural_types,
                    )?;
                // A domain-carrying bounded field (`[u8; N] in Utf8`) keeps the
                // plain carrier's layout and store route. Its byte restrictions
                // remain independent of the index. The final write consumes
                // the exact source preservation decision, covering its byte
                // classes beside the bound; index authority alone never does.
                if !matches!(
                    field.field_type,
                    StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    ) | StructuralFieldType::PredicateByteSequence {
                        carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                        ..
                    }
                ) {
                    return unsupported(operands);
                }
                Ok(Destination {
                    place: parameter.place,
                    descriptor_steps: Vec::new(),
                    value_type: terminal_scalar_type(PrimitiveType::U8)?,
                    shape: DestinationShape::ByteField {
                        path,
                        field: field.id,
                    },
                })
            }
            Self::ElementView {
                store,
                place,
                structural_type,
            } => {
                let CheckedStoreLeaf::ViewElement { element_path, .. } = &store.destination.leaf
                else {
                    return unsupported("element-view write names no view element");
                };
                let (mut descriptor_steps, endpoint, _) =
                    super::structural_scalar_store::lower_window_path(
                        identities,
                        structural_type,
                        &store.destination.path,
                        structural_types,
                    )?;
                let selected_type = match endpoint {
                    super::structural_scalar_store::WindowLeaf::Subtree { structural_type } => {
                        structural_type
                    }
                    super::structural_scalar_store::WindowLeaf::Field(field) => {
                        descriptor_steps.push(super::runtime_elements::ProjectionStep::Static(
                            StructuralPathSegment::Field(field.identity),
                        ));
                        field
                            .field_type
                            .as_structural()
                            .ok_or(LoweringError::Unsupported(
                                "element-view destination field has no structural descriptor",
                            ))?
                    }
                };
                let element = structural_types
                    .iter()
                    .find_map(|declaration| match declaration.shape {
                        StructuralTypeShape::ElementView { element, .. }
                            if declaration.id == selected_type =>
                        {
                            Some(element)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "element-view write destination is not an element view",
                    ))?;
                let path = crate::expression_preparation::bindings::structural_paths::lower_structural_path(
            identities,element_path)?;
                let leaf = terminal_semantics::element_view_leaf(structural_types, element, &path)
                    .ok_or(LoweringError::Unsupported(
                        "element-view write path is not an admitted scalar leaf",
                    ))?;
                Ok(Destination {
                    place,
                    descriptor_steps,
                    value_type: leaf.scalar_type,
                    shape: DestinationShape::ElementView {
                        descriptor: terminal_psi::StructuralArgument::whole(
                            place,
                            terminal_psi::StructuralAccess::MutableBorrow,
                        ),
                        path,
                        leaf,
                    },
                })
            }
        }
    }
}

impl Destination {
    pub(crate) fn capture_descriptor(
        &mut self,
        source: PlaceId,
        mut path: Vec<StructuralPathSegment>,
    ) -> Result<(), LoweringError> {
        match &mut self.shape {
            DestinationShape::ElementView { descriptor, .. } => {
                descriptor.place = source;
                descriptor.path = path;
            }
            DestinationShape::ByteView if path.is_empty() => {}
            DestinationShape::ByteField { .. } if path.is_empty() => {
                // A captured field view is now a whole descriptor alias; its
                // original predicate decisions remain on this same store.
                self.shape = DestinationShape::ByteView;
            }
            DestinationShape::ByteField { path: carrier, .. } => {
                if !matches!(path.pop(), Some(StructuralPathSegment::Field(_))) {
                    return unsupported("captured byte field lost its declared field projection");
                }
                *carrier = path;
            }
            _ => return unsupported("captured byte descriptor is not a whole view"),
        }
        self.place = source;
        self.descriptor_steps.clear();
        Ok(())
    }

    pub(crate) fn set_descriptor_path(&mut self, path: Vec<StructuralPathSegment>) {
        if let DestinationShape::ElementView { descriptor, .. } = &mut self.shape {
            descriptor.path = path;
        }
    }

    /// Consume source decisions against the one prepared destination. Refined
    /// elements require both the selected coordinate and the complete receiving
    /// interval; byte predicates retain their independent preservation decision.
    pub(crate) fn source_decisions(
        &self,
        checked: &CheckedTrees,
        leaf: IndexedWriteLeaf<'_>,
    ) -> Result<IndexedWriteDecisions, LoweringError> {
        let mut decisions = leaf.source_decisions();
        if let (Some((element, range_decision)), DestinationShape::ElementView { leaf, .. }) =
            (leaf.element, &self.shape)
        {
            decisions.write = decisions.index
                && match &leaf.restriction {
                    None => true,
                    Some(terminal_psi::StructuralFieldType::BoundedInteger(bounds)) => {
                        super::structural_scalar_store::source_range_decided(
                            checked,
                            element,
                            range_decision,
                            Some(*bounds),
                        )?
                    }
                    // A numeric source receipt cannot authorize an arbitrary
                    // predicate, even when the index leg has been decided.
                    Some(_) => false,
                };
        }
        Ok(decisions)
    }

    /// The destination's live length. A bound proven against an earlier
    /// field `.len` observation discharges a byte field store only when the
    /// operand names that same dominating value; otherwise mint a current
    /// observation, whose bound the certificate must then derive from the
    /// extent itself, the same rule the field read uses. A byte store
    /// preserves the field's extent, so a fresh field observation also stands
    /// as the dominating record for later accesses.
    fn length(
        &self,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<ValueId, LoweringError> {
        let kind = match &self.shape {
            DestinationShape::ByteView => OperationKind::ByteSequenceLength {
                source: terminal_psi::StructuralArgument::whole(
                    self.place,
                    terminal_psi::StructuralAccess::SharedBorrow,
                ),
            },
            DestinationShape::ByteField { path, field } => {
                if let Some(length) = operations
                    .length_observations
                    .field_bytes
                    .iter()
                    .rev()
                    .find_map(|(place, carrier, named, value)| {
                        (*place == self.place && carrier == path && named == field)
                            .then_some(*value)
                    })
                {
                    return Ok(length);
                }
                OperationKind::StructuralByteSequenceFieldLength {
                    source: self.place,
                    path: path.clone(),
                    field: *field,
                }
            }
            DestinationShape::ElementView { descriptor, .. } => OperationKind::ElementViewLength {
                source: descriptor.clone(),
            },
        };
        let length = value_id(allocate_dense(next_value)?);
        operations.push_leaf(
            scalar_result_declaration(length, terminal_scalar_type(PrimitiveType::U64)?),
            kind,
        );
        if let DestinationShape::ByteField { path, field } = &self.shape {
            operations.length_observations.field_bytes.push((
                self.place,
                path.clone(),
                *field,
                length,
            ));
        }
        Ok(length)
    }
}

/// The value one indexed write stores: a direct expression emitted beside
/// the selector, or a value the operation frame already established by
/// evaluating a computation. A computation completes in a fresh block whose
/// parameters rebind the dense namespace by position, including the selector
/// the frame captured before the computation.
#[derive(Clone, Copy)]
pub(crate) enum IndexedWriteValue<'a> {
    Expression(&'a LoweredDirectExpression),
    Established(ValueDeclaration),
}

/// Emit one indexed write after the frame lowered its index and value: the
/// prepared destination, the shared operand check, the index coordinate,
/// the value, the destination's length, and the write with its obligation.
pub(crate) fn emit(
    target: IndexedWriteTarget<'_>,
    destination: Destination,
    decisions: IndexedWriteDecisions,
    index: &LoweredDirectExpression,
    value: IndexedWriteValue<'_>,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let operands = target.wording().operands;
    let value_type = match value {
        IndexedWriteValue::Expression(expression) => expression.scalar_type(),
        IndexedWriteValue::Established(value) => value.scalar_type,
    };
    if !matches!(index.scalar_type(), ScalarType::Integer(_))
        || value_type != destination.value_type
        || direct_expression_contains_short_circuit(index)
        || matches!(value, IndexedWriteValue::Expression(expression)
            if direct_expression_contains_short_circuit(expression))
    {
        return unsupported(operands);
    }
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(index, &types)?;
    if let IndexedWriteValue::Expression(expression) = value {
        validate_direct_parameter_types(expression, &types)?;
    }
    let index = emit_index_coordinate(
        index,
        false,
        values,
        next_value,
        next_obligation,
        operations,
    )?;
    let value = match value {
        IndexedWriteValue::Expression(expression) => {
            emit_direct_expression(expression, values, next_value, operations)
        }
        IndexedWriteValue::Established(value) => value.id,
    };
    let length = destination.length(next_value, operations)?;
    let obligation = obligation_id(allocate_dense(next_obligation)?);
    if decisions.write {
        operations.source_checked_obligations.push(obligation);
    }
    let place = destination.place;
    Ok(match destination.shape {
        DestinationShape::ByteView => OperationKind::ByteSequenceWrite {
            destination: place,
            index,
            value,
            length,
            obligation,
        },
        DestinationShape::ByteField { path, field } => {
            OperationKind::StructuralByteSequenceFieldByteStore {
                destination: place,
                path,
                field,
                index,
                value,
                length,
                obligation,
            }
        }
        DestinationShape::ElementView {
            descriptor, path, ..
        } => OperationKind::ElementViewWrite {
            destination: descriptor,
            path,
            index,
            value,
            length,
            obligation,
        },
    })
}

/// Evaluate the authored integer before converting to Terminal's count
/// coordinate. Widening the expression itself would change narrow or wrapping
/// arithmetic. The source index bound also proves signed nonnegativity; it
/// cannot establish the stored value's domain. Capture this coordinate before
/// evaluating the RHS, so that its effects cannot retarget the assignment.
pub(crate) fn emit_index_coordinate(
    expression: &LoweredDirectExpression,
    bound_decided: bool,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ValueId, LoweringError> {
    let operand = emit_direct_expression(expression, values, next_value, operations);
    emit_u64_coordinate(
        operand,
        expression.scalar_type(),
        bound_decided,
        next_value,
        next_obligation,
        operations,
    )
}
