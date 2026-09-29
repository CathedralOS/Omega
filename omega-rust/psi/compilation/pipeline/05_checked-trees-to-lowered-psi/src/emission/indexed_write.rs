//! Runtime-indexed scalar writes through a `&mut` byte view, a bounded byte
//! field, or a `&mut [T]` element view.
//!
//! The three leaves replay and emit one shape. Replay rejoins the authored
//! indexed target, requires the builtin index meaning, and binds the
//! `AssignmentIndex { depth: 0 }` operand and the stored value (a pure
//! `AssignmentValue` expression, or the scalar result of the statement's own
//! call) to the checked plan. Emission converts the index to Terminal's `u64`
//! coordinate, evaluates the value, observes the destination's length, and
//! ends in a write that owes one bound obligation; value and obligation
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
use crate::expression_preparation::source_custody;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::TableAssignment;
use checked_trees::{
    CheckedByteSequenceWritePlan, CheckedElementViewWritePlan, CheckedStoreRoot, CheckedStoreValue,
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
    /// The byte view names its `&mut` parameter by authored position.
    ByteView {
        write: &'a CheckedByteSequenceWritePlan,
        parameters: &'a [StructuralParameterDeclaration],
    },
    /// The byte field names its root parameter by authored position.
    ByteField {
        store: &'a CheckedStructuralByteSequenceFieldByteStorePlan,
        parameters: &'a [StructuralParameterDeclaration],
    },
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
    index_meaning: &'static str,
    lost_operand: &'static str,
    substituted_operand: &'static str,
    result_replaced_selection: &'static str,
    result_without_call: &'static str,
    result_position: &'static str,
    result_not_established: &'static str,
    result_type: &'static str,
    /// Replay's refusal of a computation value. The shared store vocabulary
    /// can carry one for every leaf; none of these leaves lowers it.
    kept_computation: &'static str,
    /// Lowering's refusal of the same computation value.
    keeps_computation: &'static str,
    /// Emission's operand rejection. The byte leaves' destination custody
    /// checks report it too.
    operands: &'static str,
}

const BYTE_VIEW: Wording = Wording {
    lost_index: "byte-view write lost its authored index",
    index_meaning: "byte-view write changed its selected index meaning",
    lost_operand: "byte-view write lost its operand source",
    substituted_operand: "byte-view write substituted an authored operand",
    result_replaced_selection: "byte-view write replaced a selected RHS with a result",
    result_without_call: "byte-view write call result has no authored call",
    result_position: "byte store call-result position exceeds usize",
    result_not_established: "byte store call result is not this statement's established scalar result",
    result_type: "byte store call result differs from its element type",
    kept_computation: "byte-view write kept a computation value",
    keeps_computation: "byte-view write keeps a computation value",
    operands: "byte-view write requires exact mutable byte custody and scalar operands",
};

const BYTE_FIELD: Wording = Wording {
    lost_index: "byte replacement lost its authored index",
    index_meaning: "byte replacement changed its selected index meaning",
    lost_operand: "byte replacement lost a scalar source binding",
    substituted_operand: "byte replacement substituted its evaluated scalar operand",
    result_replaced_selection: "byte replacement replaced a selected RHS with a result",
    result_without_call: "byte replacement call result has no authored call",
    result_position: "byte store call-result position exceeds usize",
    result_not_established: "byte store call result is not this statement's established scalar result",
    result_type: "byte store call result differs from its element type",
    kept_computation: "byte replacement kept a computation value",
    keeps_computation: "byte replacement keeps a computation value",
    operands: "byte replacement requires bounded bytes and exact branch-free operands",
};

const ELEMENT_VIEW: Wording = Wording {
    lost_index: "element-view write lost its authored index",
    index_meaning: "element-view write changed its selected index meaning",
    lost_operand: "element-view write lost its operand source",
    substituted_operand: "element-view write substituted an authored operand",
    result_replaced_selection: "element-view write replaced a selected RHS with a result",
    result_without_call: "element-view write call result has no authored call",
    result_position: "element-view store result position exceeds usize",
    result_not_established: "element-view store call result is not this statement's established scalar result",
    result_type: "element-view store call result differs from its element type",
    kept_computation: "element-view write kept a computation value",
    keeps_computation: "element-view write keeps a computation value",
    operands: "element-view write requires scalar operands",
};

impl<'a> IndexedWriteLeaf<'a> {
    pub(crate) fn statement_index(self) -> u32 {
        match self {
            Self::ByteView(write) => write.statement_index,
            Self::ByteField(store) => store.statement_index,
            Self::ElementView(write) => write.statement_index,
        }
    }

    fn index(self) -> &'a CheckedScalarExpression {
        match self {
            Self::ByteView(write) => &write.index,
            Self::ByteField(store) => &store.index,
            Self::ElementView(write) => &write.index,
        }
    }

    /// Every leaf's value is the one shared store vocabulary.
    fn value(self) -> &'a CheckedStoreValue {
        match self {
            Self::ByteView(write) => &write.value,
            Self::ByteField(store) => &store.value,
            Self::ElementView(write) => &write.value,
        }
    }

    /// The stored value's type as the plan records it.
    fn value_type(self) -> PrimitiveType {
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
    /// call operation established it as the most recent scalar value.
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
                if values[position].scalar_type != scalar_type {
                    return unsupported(wording.result_type);
                }
                Ok(LoweredDirectExpression::Local {
                    position,
                    scalar_type,
                })
            }
            CheckedStoreValue::Computation(_) => unsupported(wording.keeps_computation),
        }
    }
}

/// Rejoin one indexed write's checked plan to its authored assignment. Each
/// leaf checks its own destination ahead of the index meaning, except the byte
/// field's carrier path and field, which follow it.
pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    assignment: &TableAssignment,
    leaf: IndexedWriteLeaf<'_>,
) -> Result<(), LoweringError> {
    let wording = leaf.wording();
    let (owner, state) = source_custody::authored_state(checked, state_symbol)?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported(wording.lost_index);
    };
    let collection = || {
        crate::emission::call_source_custody::projected_receivers::store_destination(
            checked,
            machine,
            state_symbol,
            None,
            indexed.collection,
        )
    };
    let builtin_coordinates = || {
        validation::place_has_builtin_coordinates(
            &checked.typed,
            owner,
            Some(state),
            assignment.target,
        )
    };
    match leaf {
        IndexedWriteLeaf::ByteView(write) => {
            // A byte view has no local-rooted lowering.
            let Some(position) = write.destination.parameter_position() else {
                return unsupported("byte-view write has no parameter root");
            };
            let parameter = checked
                .state_parameters(state)
                .get(position as usize)
                .ok_or(LoweringError::Unsupported(
                    "byte-view write has no authored parameter",
                ))?;
            let source = collection()?;
            if owner.symbol != machine
                || source.root != parameter.symbol
                || !source.path.is_empty()
                || !builtin_coordinates()
                || !matches!(
                    checked
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    checked_trees::types::TypeReferenceNode::Reference {
                        access: language_core::ReferenceAccess::Mutable,
                        ..
                    }
                )
            {
                return unsupported("byte-view write changed its authored mutable destination");
            }
        }
        IndexedWriteLeaf::ByteField(_) => {
            if owner.symbol != machine
                || !builtin_coordinates()
                || matches!(
                    checked.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                )
            {
                return unsupported("byte replacement requires exact builtin indexed custody");
            }
        }
        IndexedWriteLeaf::ElementView(write) => {
            let (destination_root, source_root, source_path_is_empty) = match write.destination {
                CheckedStoreRoot::Parameter { position } => {
                    let parameter_symbol = checked
                        .state_parameters(state)
                        .get(position as usize)
                        .map(|parameter| parameter.symbol)
                        .ok_or(LoweringError::Unsupported(
                            "element-view write has no authored parameter",
                        ))?;
                    let source = collection()?;
                    (parameter_symbol, source.root, source.path.is_empty())
                }
                CheckedStoreRoot::Local { symbol } => {
                    authored_view_local(
                        checked,
                        checked.statement_table.statements(state.statement_nodes),
                        symbol,
                    )?;
                    // The view local is the destination's own carrier: the
                    // indexed collection must be its bare authored name, which
                    // the projected receiver walk has no arm for since it is
                    // neither owned storage nor a shared-borrow loan.
                    let ExpressionNode::Name(name) =
                        checked.expression_table.expression(indexed.collection)
                    else {
                        return unsupported(
                            "element-view write substituted its view local collection",
                        );
                    };
                    if !name.symbol.is_valid()
                        || name.head_symbol != name.symbol
                        || name.symbol != symbol
                        || checked
                            .expression_table
                            .name_path_members(name.members)
                            .len()
                            != 1
                    {
                        return unsupported(
                            "element-view write substituted its view local collection",
                        );
                    }
                    (symbol, symbol, true)
                }
            };
            if owner.symbol != machine
                || source_root != destination_root
                || !source_path_is_empty
                || !builtin_coordinates()
            {
                return unsupported("element-view write changed its authored mutable destination");
            }
        }
    }
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == assignment.target
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) {
        return unsupported(wording.index_meaning);
    }
    if let IndexedWriteLeaf::ByteField(store) = leaf {
        let source = collection()?;
        // A byte-field element has no local-rooted lowering.
        let Some(position) = store.destination.parameter_position() else {
            return unsupported("byte replacement has no parameter root");
        };
        let parameter = checked
            .state_parameters(state)
            .get(position as usize)
            .ok_or(LoweringError::Unsupported(
                "byte replacement has no authored parameter",
            ))?;
        let mut path = store.carrier_path.clone();
        path.push(CheckedUnitStructuralPathSegment::Field(
            store.field_identity.clone(),
        ));
        if source.root != parameter.symbol || source.path != path {
            return unsupported("byte replacement differs from its authored field");
        }
    }
    let statement_index = leaf.statement_index();
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            statement_index,
            CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
        )
        .ok_or(LoweringError::Unsupported(wording.lost_operand))?;
    if binding.expression != indexed.index || expression != leaf.index() {
        return unsupported(wording.substituted_operand);
    }
    source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(
            source_custody::locate(
                checked,
                state_symbol,
                statement_index,
                CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
            )?
            .primitive_type,
        )?,
    )?;
    match leaf.value() {
        // A store reading its own statement's call result has no authored
        // scalar expression to select: the right-hand side is the call. The
        // producing call is rejoined where the operation order and the value
        // are lowered; a selected expression here would mean the checked
        // stage chose a different source.
        CheckedStoreValue::ScalarResult { .. } => {
            if checked
                .facts
                .values
                .scalar_expressions
                .expressions
                .iter()
                .any(|expression| {
                    expression.state == state_symbol
                        && expression.statement_ordinal == statement_index
                        && expression.role == CheckedScalarExpressionRole::AssignmentValue
                })
            {
                return unsupported(wording.result_replaced_selection);
            }
            if !matches!(
                checked.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            ) {
                return unsupported(wording.result_without_call);
            }
        }
        CheckedStoreValue::Pure(retained) => {
            let (binding, expression) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(
                    state_symbol,
                    statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .ok_or(LoweringError::Unsupported(wording.lost_operand))?;
            if binding.expression != assignment.value || expression != retained {
                return unsupported(wording.substituted_operand);
            }
            source_custody::validate_pure(
                checked,
                binding,
                terminal_scalar_type(leaf.value_type())?,
            )?;
        }
        CheckedStoreValue::Computation(_) => {
            return unsupported(wording.kept_computation);
        }
    }
    Ok(())
}

/// A mutable-view local's authored establishment: the statement binding its
/// symbol must still carry a `&mut` declared type, the same rejoin a
/// `&mut`-borrowed destination parameter gets through its authored parameter.
fn authored_view_local(
    checked: &CheckedTrees,
    statements: &[typed_trees::statement::StatementNode],
    symbol: SymbolHandle,
) -> Result<(), LoweringError> {
    let mut found = false;
    for statement in statements {
        let checked_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.symbol != symbol {
            continue;
        }
        if found {
            return unsupported("element-view write names a duplicated view local");
        }
        found = true;
        if !matches!(
            checked
                .type_reference_table
                .type_reference(local.type_reference),
            checked_trees::types::TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::Mutable,
                ..
            }
        ) {
            return unsupported("element-view write local lost its authored mutability");
        }
    }
    if !found {
        return unsupported("element-view write names an unknown view local");
    }
    Ok(())
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
    ElementView,
}

impl<'a> IndexedWriteTarget<'a> {
    pub(crate) fn leaf(self) -> IndexedWriteLeaf<'a> {
        match self {
            Self::ByteView { write, .. } => IndexedWriteLeaf::ByteView(write),
            Self::ByteField { store, .. } => IndexedWriteLeaf::ByteField(store),
            Self::ElementView { write, .. } => IndexedWriteLeaf::ElementView(write),
        }
    }

    /// The leaf's own emission-time destination check. A byte leaf's
    /// custody failure reports the leaf's operand wording, which has always
    /// covered custody and operands together.
    fn destination(
        self,
        structural_types: &[StructuralTypeDeclaration],
        operands: &'static str,
    ) -> Result<Destination, LoweringError> {
        match self {
            Self::ByteView { write, parameters } => {
                let Some(position) = write.destination.parameter_position() else {
                    return unsupported("byte-view write destination has no parameter root");
                };
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.position == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte-view write destination is absent",
                    ))?;
                if parameter.access != StructuralAccess::MutableBorrow
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !structural_types.iter().any(|declaration| {
                        declaration.id == parameter.structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                        | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
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
            Self::ByteField { store, parameters } => {
                let Some(position) = store.destination.parameter_position() else {
                    return unsupported("byte replacement destination has no parameter root");
                };
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.position == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte replacement destination is absent",
                    ))?;
                let (path, field) =
                    crate::emission::structural_scalar_store::lower_structural_field_place(
                        store.statement_index,
                        store.statement_index,
                        position,
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
                place,
                structural_type,
                ..
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
                let value_type = structural_types
                    .iter()
                    .find_map(|declaration| match declaration.shape {
                        StructuralTypeShape::PrimitiveScalar(scalar_type)
                            if declaration.id == element =>
                        {
                            Some(scalar_type)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "element-view write element is not a scalar leaf",
                    ))?;
                Ok(Destination {
                    place,
                    value_type,
                    shape: DestinationShape::ElementView,
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
            DestinationShape::ElementView => {
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

/// Emit one indexed write after the frame lowered its index and value: the
/// leaf's destination check, the shared operand check, the index coordinate,
/// the value, the destination's length, and the write with its obligation.
pub(crate) fn emit(
    target: IndexedWriteTarget<'_>,
    structural_types: &[StructuralTypeDeclaration],
    index: &LoweredDirectExpression,
    value: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let operands = target.leaf().wording().operands;
    let destination = target.destination(structural_types, operands)?;
    if !matches!(index.scalar_type(), ScalarType::Integer(_))
        || value.scalar_type() != destination.value_type
        || direct_expression_contains_short_circuit(index)
        || direct_expression_contains_short_circuit(value)
    {
        return unsupported(operands);
    }
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(index, &types)?;
    validate_direct_parameter_types(value, &types)?;
    let index = emit_index_coordinate(index, values, next_value, next_obligation, operations)?;
    let value = emit_direct_expression(value, values, next_value, operations);
    let length = destination.length(next_value, operations)?;
    let obligation = obligation_id(allocate_dense(next_obligation)?);
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
        DestinationShape::ElementView => OperationKind::ElementViewWrite {
            destination: place,
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
