//! Runtime-indexed scalar writes through a `&mut` byte view, a bounded byte
//! field, or a `&mut [T]` element view.
//!
//! The three leaves replay and emit one shape. Replay rejoins the authored
//! indexed target, requires the builtin index meaning, and binds the
//! exact `AssignmentIndex` occurrence and the stored value (a pure
//! `AssignmentValue` expression, or the scalar result of the statement's own
//! call) to the checked plan. Emission converts the index to Terminal's `u64`
//! coordinate, evaluates the value, observes the destination's length, and
//! ends in a write whose obligation retains the index and any target field
//! bounds through the shared Terminal judgment; value and obligation
//! identities are allocated in exactly that order.
//!
//! What differs stays with the leaf: its destination check, the stored
//! value's type (`u8`, or the view's element), its length observation
//! (`ByteSequenceLength`, the byte field's reused or fresh field length, or
//! `ElementViewLength`), its final operation, and its diagnostic wording.
use super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind, OperationResult,
    PlaceId, PrimitiveType, ScalarType, StructuralAccess, StructuralFieldId, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueDeclaration, ValueId,
    allocate_dense, direct_expression_contains_short_circuit, emit_direct_expression,
    emit_u64_coordinate, obligation_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::TableAssignment;
use checked_trees::{
    CheckedByteSequenceWritePlan, CheckedElementViewWritePlan, CheckedStoreValue,
    CheckedStructuralByteSequenceFieldByteStorePlan,
};
use symbols::SymbolHandle;

/// The destination family of one runtime-indexed scalar write.
#[derive(Clone, Copy)]
pub(crate) enum IndexedWriteLeaf<'a> {
    /// A write through a whole `&mut` byte-view parameter.
    ByteView(&'a CheckedByteSequenceWritePlan),
    /// A byte replacement inside a bounded byte field under a parameter.
    ByteField(&'a CheckedStructuralByteSequenceFieldByteStorePlan),
    /// A write through a `&mut [T]` view rooted at a parameter or a `&mut`
    /// local.
    ElementView(&'a CheckedElementViewWritePlan),
}

/// One indexed write as the ordered frame emits it: the leaf's plan with the
/// lowered storage its destination resolves in.
#[derive(Clone, Copy)]
pub(crate) enum IndexedWriteTarget<'a> {
    /// The byte view names its `&mut` parameter by authored position, which
    /// `emit` resolves in the frame's authored parameter roster.
    ByteView(&'a CheckedByteSequenceWritePlan),
    /// An established bounded owner, resolved in the frame's local namespace.
    OwnedBytes {
        write: &'a CheckedByteSequenceWritePlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    /// The byte field names its root parameter by authored position, which
    /// `emit` resolves in the same roster.
    ByteField(&'a CheckedStructuralByteSequenceFieldByteStorePlan),
    /// The frame already resolved the view's home place and type from its
    /// parameter or local root.
    ElementView {
        write: &'a CheckedElementViewWritePlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
}

/// The diagnostics one leaf reports. Each leaf spells its own complete
/// messages so every rejection keeps its established `&'static str` text.
struct Wording {
    lost_index: &'static str,
    lost_operand: &'static str,
    result_position: &'static str,
    result_not_established: &'static str,
    /// Emission's operand rejection. The byte leaves' destination custody
    /// checks report it too.
    operands: &'static str,
}

const BYTE_VIEW: Wording = Wording {
    lost_index: "byte-view write lost its authored index",
    lost_operand: "byte-view write lost its operand source",
    result_position: "byte store call-result position exceeds usize",
    result_not_established: "byte store call result is not this statement's established scalar result",
    operands: "byte-view write requires exact mutable byte custody and scalar operands",
};

const BYTE_FIELD: Wording = Wording {
    lost_index: "byte replacement lost its authored index",
    lost_operand: "byte replacement lost a scalar source binding",
    result_position: "byte store call-result position exceeds usize",
    result_not_established: "byte store call result is not this statement's established scalar result",
    operands: "byte replacement requires bounded bytes and exact branch-free operands",
};

const ELEMENT_VIEW: Wording = Wording {
    lost_index: "element-view write lost its authored index",
    lost_operand: "element-view write lost its operand source",
    result_position: "element-view store result position exceeds usize",
    result_not_established: "element-view store call result is not this statement's established scalar result",
    operands: "element-view write requires scalar operands",
};

impl<'a> IndexedWriteLeaf<'a> {
    /// Join the final source decision at emission, after execution plans have
    /// been constructed. The decision covers the byte predicate as well as
    /// the bound; scalar-index admission alone is insufficient for a write.
    pub(crate) fn formation_is_proven(
        self,
        checked: &CheckedTrees,
        state: SymbolHandle,
    ) -> Result<bool, LoweringError> {
        if matches!(self, Self::ElementView(_)) {
            return Ok(false);
        }
        let (_, authored) = crate::declarations::state_declaration(checked, state)?;
        let Some(checked_trees::statement::StatementNode::Assignment(assignment)) = checked
            .statement_table
            .statements(authored.statement_nodes)
            .get(self.statement_index() as usize)
        else {
            return unsupported(self.wording().lost_index);
        };
        Ok(checked
            .facts
            .byte_write_is_proven(indexed_target(checked, state, assignment, self)?))
    }

    pub(crate) fn statement_index(self) -> u32 {
        match self {
            Self::ByteView(write) => write.statement_index,
            Self::ByteField(store) => store.statement_index,
            Self::ElementView(write) => write.statement_index,
        }
    }

    /// The bound-row role the authored target's own indexed step mints, or
    /// `None` when an element-view write rejoins a `&mut collection[index]`
    /// view local's establishment — that selector carries no authored target
    /// coordinate, so the caller checks the coordinate itself instead.
    pub(crate) fn index_role(
        self,
        checked: &CheckedTrees,
        state: SymbolHandle,
    ) -> Result<Option<CheckedScalarExpressionRole>, LoweringError> {
        let (_, authored) = crate::declarations::state_declaration(checked, state)?;
        let Some(checked_trees::statement::StatementNode::Assignment(assignment)) = checked
            .statement_table
            .statements(authored.statement_nodes)
            .get(self.statement_index() as usize)
        else {
            return unsupported(self.wording().lost_index);
        };
        let target = indexed_target(checked, state, assignment, self)?;
        let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(target) else {
            return unsupported(self.wording().lost_index);
        };
        let Some(depth) =
            validation::assignment_target_selectors(&checked.typed, assignment.target)
                .iter()
                .position(|selector| *selector == indexed.index)
                .and_then(|depth| u32::try_from(depth).ok())
        else {
            return match self {
                Self::ElementView(_) => Ok(None),
                _ => unsupported(self.wording().lost_operand),
            };
        };
        Ok(Some(CheckedScalarExpressionRole::AssignmentIndex { depth }))
    }

    pub(crate) fn index(self) -> &'a CheckedScalarExpression {
        match self {
            Self::ByteView(write) => &write.index,
            Self::ByteField(store) => &store.index,
            Self::ElementView(write) => &write.index,
        }
    }

    /// Every leaf's value is the one shared store vocabulary.
    pub(crate) fn value(self) -> &'a CheckedStoreValue {
        match self {
            Self::ByteView(write) => &write.value,
            Self::ByteField(store) => &store.value,
            Self::ElementView(write) => &write.value,
        }
    }

    /// The stored value's type as the plan records it.
    pub(crate) fn value_type(self) -> PrimitiveType {
        match self {
            Self::ByteView(_) | Self::ByteField(_) => PrimitiveType::U8,
            Self::ElementView(write) => write.element,
        }
    }

    fn wording(self) -> &'static Wording {
        match self {
            Self::ByteView(_) => &BYTE_VIEW,
            Self::ByteField(_) => &BYTE_FIELD,
            Self::ElementView(_) => &ELEMENT_VIEW,
        }
    }

    /// The store's value in the assignment's scalar namespace: an authored
    /// expression rejoined at `AssignmentValue`, or the scalar result of the
    /// call this same statement performs. That result binds no local and
    /// carries no `AssignmentValue` row, so it is read back where the ordered
    /// call operation established it as the most recent scalar value. A
    /// computation is not a direct expression: the operation frame evaluates
    /// it through the shared scalar evaluator and supplies the established
    /// value to [`emit`] instead.
    pub(crate) fn lower_value(
        self,
        bindings: &ScalarBindings,
        checked: &CheckedTrees,
        state: SymbolHandle,
        values: &[ValueDeclaration],
    ) -> Result<LoweredDirectExpression, LoweringError> {
        let wording = self.wording();
        match self.value() {
            CheckedStoreValue::Pure(_) => bindings.expression_at(
                checked,
                state,
                self.statement_index(),
                CheckedScalarExpressionRole::AssignmentValue,
            ),
            CheckedStoreValue::ScalarResult { position } => {
                let position = usize::try_from(*position)
                    .map_err(|_| LoweringError::Unsupported(wording.result_position))?;
                if position.checked_add(1) != Some(values.len()) {
                    return unsupported(wording.result_not_established);
                }
                let scalar_type = terminal_scalar_type(self.value_type())?;
                Ok(LoweredDirectExpression::Local {
                    position,
                    scalar_type,
                })
            }
            CheckedStoreValue::Computation(_) => {
                unsupported("an indexed write's computed value is evaluated by its operation frame")
            }
        }
    }
}

/// Peel only the retained element projection, authenticating every member
/// against its resolved declaration. The remaining index is the descriptor's
/// selector; its occurrence and value still use ordinary assignment replay.
fn indexed_target(
    checked: &CheckedTrees,
    state_symbol: SymbolHandle,
    assignment: &TableAssignment,
    leaf: IndexedWriteLeaf<'_>,
) -> Result<checked_trees::expression::ExpressionHandle, LoweringError> {
    let IndexedWriteLeaf::ElementView(write) = leaf else {
        return Ok(assignment.target);
    };
    let (machine, state) = crate::declarations::state_declaration(checked, state_symbol)?;
    let mut target = assignment.target;
    for segment in write.path.iter().rev() {
        match (segment, checked.expression_table.expression(target)) {
            (CheckedUnitStructuralPathSegment::Field(_), ExpressionNode::Member(member))
                if member.case_variant.is_none() =>
            {
                let reference = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(state),
                    member.receiver,
                )
                .and_then(|reference| checked.typed.unwrapped_type_reference(reference))
                .ok_or(LoweringError::Unsupported(
                    "element-view field lost its authored owner",
                ))?;
                let owner = checked.typed.closed_data_definition(reference).ok_or(
                    LoweringError::Unsupported("element-view field has no record declaration"),
                )?;
                validation::exact_data_member_field(
                    &checked.typed,
                    owner,
                    member.member_symbol,
                    member.member.as_str(),
                    None,
                )
                .or_else(|| {
                    checked.typed.closed_data_member_field(
                        reference,
                        member.member_symbol,
                        member.member.as_str(),
                    )
                })
                .ok_or(LoweringError::Unsupported(
                    "element-view field changed its declaration",
                ))?;
                target = member.receiver;
            }
            (CheckedUnitStructuralPathSegment::FixedIndex(_), ExpressionNode::Indexed(indexed)) => {
                target = indexed.collection;
            }
            _ => return unsupported("element-view write changed its authored element projection"),
        }
    }
    // A `&mut collection[index]` element view local carries no indexed step
    // in this target — `target.field = value` — so the rejoin continues at
    // its establishment, whose authored indexed expression is the selector.
    if let ExpressionNode::Name(name) = checked.expression_table.expression(target)
        && let Some(indexed) = element_view_local_indexed(checked, state, name.symbol)
    {
        return Ok(indexed);
    }
    Ok(target)
}

/// The authored `collection[index]` a `&mut T` element view local borrowed at
/// its establishment. A `local.field = value` store through that view writes
/// the borrowed element, and its checked plan's index coordinate rejoins this
/// indexed expression's selector.
fn element_view_local_indexed(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    symbol: SymbolHandle,
) -> Option<checked_trees::expression::ExpressionHandle> {
    let local = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol => {
                Some(local)
            }
            _ => None,
        })?;
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    matches!(borrow.access, language_core::ReferenceAccess::Mutable).then_some(borrow.target)
}

/// A leaf's destination after its emission-time check.
struct Destination {
    place: PlaceId,
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
        path: Vec<StructuralPathSegment>,
    },
}

impl<'a> IndexedWriteTarget<'a> {
    pub(crate) fn leaf(self) -> IndexedWriteLeaf<'a> {
        match self {
            Self::ByteView(write) => IndexedWriteLeaf::ByteView(write),
            Self::OwnedBytes { write, .. } => IndexedWriteLeaf::ByteView(write),
            Self::ByteField(store) => IndexedWriteLeaf::ByteField(store),
            Self::ElementView { write, .. } => IndexedWriteLeaf::ElementView(write),
        }
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
    fn destination(
        self,
        parameters: &[(u32, StructuralParameterDeclaration)],
        structural_types: &[StructuralTypeDeclaration],
        operands: &'static str,
    ) -> Result<Destination, LoweringError> {
        match self {
            Self::OwnedBytes {
                place,
                structural_type,
                ..
            } => {
                if !structural_types.iter().any(|declaration| {
                    declaration.id == structural_type
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                }) {
                    return unsupported("byte write local is not a bounded owner");
                }
                Ok(Destination {
                    place,
                    value_type: terminal_scalar_type(PrimitiveType::U8)?,
                    shape: DestinationShape::ByteView,
                })
            }
            Self::ByteView(write) => {
                let Some(position) = write.destination.parameter_position() else {
                    return unsupported("byte-view write destination has no parameter root");
                };
                let (_, parameter) = parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte-view write destination is absent",
                    ))?;
                if parameter.access != StructuralAccess::MutableBorrow
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                    || !structural_types.iter().any(|declaration| {
                        declaration.id == parameter.structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                        | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            )
                    })
                {
                    return unsupported(operands);
                }
                Ok(Destination {
                    place: parameter.place,
                    value_type: terminal_scalar_type(PrimitiveType::U8)?,
                    shape: DestinationShape::ByteView,
                })
            }
            Self::ByteField(store) => {
                let Some(position) = store.destination.parameter_position() else {
                    return unsupported("byte replacement destination has no parameter root");
                };
                let (_, parameter) = parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte replacement destination is absent",
                    ))?;
                let (path, field) =
                    crate::emission::structural_scalar_store::lower_structural_field_place(
                        store.statement_index,
                        store.statement_index,
                        parameter.position,
                        &store.carrier_path,
                        &store.field_identity,
                        parameter,
                        structural_types,
                        crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                    )?;
                // A domain-carrying bounded field (`[u8; N] in Utf8`) keeps the
                // plain carrier's layout and store route. Its byte restrictions
                // are not this producer's to discharge: the verifier
                // reconstructs the store's goal from the destination
                // declaration, adding the stored byte's class legs (ASCII,
                // nonzero) to the index bound, and independently rechecks that
                // a UTF-8 field's contents are all-ASCII on every path into the
                // store. The checker's own preservation proof is not carried as
                // authority.
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
                    value_type: terminal_scalar_type(PrimitiveType::U8)?,
                    shape: DestinationShape::ByteField {
                        path,
                        field: field.id,
                    },
                })
            }
            Self::ElementView {
                write,
                place,
                structural_type,
            } => {
                let element = structural_types
                    .iter()
                    .find_map(|declaration| match declaration.shape {
                        StructuralTypeShape::ElementView { element }
                            if declaration.id == structural_type =>
                        {
                            Some(element)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "element-view write destination is not an element view",
                    ))?;
                let path = crate::expression_preparation::bindings::structural_paths::lower_structural_path(&write.path)?;
                let value_type =
                    terminal_semantics::element_view_leaf(structural_types, element, &path)
                        .map(|leaf| leaf.scalar_type)
                        .ok_or(LoweringError::Unsupported(
                            "element-view write path is not an admitted scalar leaf",
                        ))?;
                Ok(Destination {
                    place,
                    value_type,
                    shape: DestinationShape::ElementView { path },
                })
            }
        }
    }
}

impl Destination {
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
            DestinationShape::ByteView => OperationKind::ByteSequenceLength { source: self.place },
            DestinationShape::ByteField { path, field } => {
                if let Some(length) = operations.field_byte_lengths.iter().rev().find_map(
                    |(place, carrier, named, value)| {
                        (*place == self.place && carrier == path && named == field)
                            .then_some(*value)
                    },
                ) {
                    return Ok(length);
                }
                OperationKind::StructuralByteSequenceFieldLength {
                    source: self.place,
                    path: path.clone(),
                    field: *field,
                }
            }
            DestinationShape::ElementView { .. } => {
                OperationKind::ElementViewLength { source: self.place }
            }
        };
        let length = value_id(allocate_dense(next_value)?);
        let id = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: length,
                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
            }),
            kind,
        });
        if let DestinationShape::ByteField { path, field } = &self.shape {
            operations
                .field_byte_lengths
                .push((self.place, path.clone(), *field, length));
        }
        Ok(length)
    }
}

/// The value one indexed write stores: a direct expression emitted beside
/// the selector, or a value the operation frame already established by
/// evaluating a computation. A computation completes in a fresh block whose
/// parameters rebind the dense namespace by position, so the selector lowered
/// against that namespace is emitted after it, as it is after a value call.
#[derive(Clone, Copy)]
pub(crate) enum IndexedWriteValue<'a> {
    Expression(&'a LoweredDirectExpression),
    Established(ValueDeclaration),
}

/// Emit one indexed write after the frame lowered its index and value: the
/// leaf's destination check, the shared operand check, the index coordinate,
/// the value, the destination's length, and the write with its obligation.
/// `parameters` is the frame's authored-to-lowered parameter roster, in which
/// a byte leaf resolves its root.
pub(crate) fn emit(
    target: IndexedWriteTarget<'_>,
    formation_decided: bool,
    parameters: &[(u32, StructuralParameterDeclaration)],
    structural_types: &[StructuralTypeDeclaration],
    index: &LoweredDirectExpression,
    value: IndexedWriteValue<'_>,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let operands = target.leaf().wording().operands;
    let destination = target.destination(parameters, structural_types, operands)?;
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
    let index = emit_index_coordinate(index, values, next_value, next_obligation, operations)?;
    let value = match value {
        IndexedWriteValue::Expression(expression) => {
            emit_direct_expression(expression, values, next_value, operations)
        }
        IndexedWriteValue::Established(value) => value.id,
    };
    let length = destination.length(next_value, operations)?;
    let obligation = obligation_id(allocate_dense(next_obligation)?);
    if formation_decided {
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
        DestinationShape::ElementView { path } => OperationKind::ElementViewWrite {
            destination: place,
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
/// arithmetic. The ordinary exact-cast obligation proves fit (including signed
/// nonnegativity); the consuming store separately proves the current live bound.
fn emit_index_coordinate(
    expression: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ValueId, LoweringError> {
    let operand = emit_direct_expression(expression, values, next_value, operations);
    emit_u64_coordinate(
        operand,
        expression.scalar_type(),
        next_value,
        next_obligation,
        operations,
    )
}
