//! Scalar stores through representation views, including fields of a record
//! view. Source replay establishes the loan and projection; the shared byte
//! destination evaluates selectors before the RHS, proves its stored integer
//! carrier, then encodes that one SSA value into the original backing.
//!
//! Replay rejoins the checked plan to the recast loan: the named target's
//! `LocalData` statement unwraps `Borrow` and the recast `Cast` to the place
//! the loan carries, and that place's parameter root and structural path
//! must equal the plan's. A direct field of a record view appends the source
//! leaf the recast judgment matched at the same offset and width
//! (`record_view_leaf_destination`, shared with the ordinary store replay).
//! Emission follows the view's read decomposition:
//!
//! - a leaf field of the view's own integer width emits one
//!   `WriteOnlyPrimitiveStore`, the authored value retagged to the leaf's
//!   declared carrier by a wrapping cast;
//! - a `u8` leaf below a bounded-owned byte field emits one
//!   `WriteOnlyPrimitiveStore` per little-endian footprint byte: byte `j`
//!   selects `index + j` — the folded literal offset or an exact add that
//!   owes its own bound obligation — and carries the `value >> 8j` low byte.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError,
    OperationKind, PrimitiveType, ScalarType, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueDeclaration, ValueId,
    allocate_dense, emit_direct_expression, obligation_id, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::{LoweredDirectExpression, emit_scalar_leaf};
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::emission::runtime_elements::{self, ProjectionStep};
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::source_custody;
use crate::expression_preparation::wrapping_cast::wrapping_cast;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::TableAssignment;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedRepresentationViewStorePlan, CheckedRuntimeIndex, CheckedStoreRoot, CheckedStoreValue,
};
use semantic_vocabulary::IntegerValue;
use symbols::SymbolHandle;
use terminal_psi::ByteSequenceCarrier;

/// Rejoin the checked plan to its authored `view = v` assignment: the named
/// destination's recast loan carries exactly the plan's rejoined place, the
/// view reads the loan's declared referee, and the value is the bound
/// `AssignmentValue` expression or the statement's own scalar call result.
pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    assignment: &TableAssignment,
    write: &CheckedRepresentationViewStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) = source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine {
        return unsupported("representation view store changed its authored machine");
    }
    // `view = v` names a scalar view local; `view.field = v` names one direct
    // field of a record view local.
    let (view_local, view_field) = match checked.expression_table.expression(assignment.target) {
        ExpressionNode::Name(name) => {
            if !name.symbol.is_valid()
                || name.head_symbol != name.symbol
                || checked
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return unsupported(
                    "representation view store substituted its view local destination",
                );
            }
            (name.symbol, None)
        }
        ExpressionNode::Member(member) => {
            let ExpressionNode::Name(name) = checked.expression_table.expression(member.receiver)
            else {
                return unsupported("representation view store field has no view local receiver");
            };
            if !name.symbol.is_valid()
                || name.head_symbol != name.symbol
                || !member.member_symbol.is_valid()
                || checked
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return unsupported(
                    "representation view store substituted its view local destination",
                );
            }
            (name.symbol, Some(member.member_symbol))
        }
        _ => return unsupported("representation view store lost its named destination"),
    };
    let CheckedStoreRoot::Parameter { position } = write.destination else {
        return unsupported("representation view store has no parameter root");
    };
    let parameter = checked
        .state_parameters(state)
        .get(position as usize)
        .ok_or(LoweringError::Unsupported(
            "representation view store lost its authored parameter",
        ))?;
    // The view local is the `&mut`/`&write` recast loan whose statement
    // precedes the assignment; unwrap its value to the carried place.
    let Some((_, local)) = checked
        .state_local_declaration(state, view_local)
        .filter(|(ordinal, _)| *ordinal < write.statement_index as usize)
    else {
        return unsupported("representation view store names an unknown view local");
    };
    let mut cursor = local.initial_value;
    if !cursor.is_valid() {
        return unsupported("representation view store's loan lost its establishment");
    }
    let place = loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Mutable =>
            {
                cursor = borrow.target;
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                cursor = cast.value;
            }
            _ => break cursor,
        }
    };
    if !checked.expression_table.expression_is_valid(place) {
        return unsupported("representation view store's loan lost its carried place");
    }
    let TypeReferenceNode::Reference {
        access: language_core::ReferenceAccess::Mutable,
        referee,
        ..
    } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return unsupported("representation view store's local lost its mutable view type");
    };
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(write.statement_index as usize),
        place,
    )?;
    let mut expected_path = source.path.clone();
    match view_field {
        None => {
            if checked.primitive_type_reference(*referee) != Some(write.view) {
                return unsupported("representation view store changed its view carrier");
            }
        }
        // A record view's field writes the source leaf the recast judgment
        // matched at the same offset and width, recomputed here from the
        // loaned place's declared type rather than taken from the plan.
        Some(_) => {
            let rejoined = record_view_leaf_destination(
                checked,
                machine,
                state_symbol,
                write.statement_index as usize,
                assignment.target,
            )?
            .ok_or(LoweringError::Unsupported(
                "representation view store's record field has no recast loan",
            ))?;
            if rejoined.view != write.view || rejoined.leaf != write.element {
                return unsupported("representation view store changed its view carrier");
            }
            expected_path = rejoined.path;
        }
    }
    if source.root != parameter.symbol || expected_path != write.path {
        return unsupported("representation view store differs from its loaned place");
    }
    let statement_index = write.statement_index;
    match &write.value {
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
                return unsupported(
                    "representation view store's scalar result replaced its selection",
                );
            }
            if !matches!(
                checked.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            ) {
                return unsupported(
                    "representation view store's scalar result has no producing call",
                );
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
                .ok_or(LoweringError::Unsupported(
                    "representation view store lost its authored value",
                ))?;
            if binding.expression != assignment.value || expression != retained {
                return unsupported("representation view store substituted its authored value");
            }
            source_custody::validate_pure(checked, binding, terminal_scalar_type(write.view)?)?;
        }
        CheckedStoreValue::Computation(_) => {
            return unsupported("representation view store kept a computation");
        }
    }
    Ok(())
}

/// A store through one direct field of a typed record recast view local
/// (`let view: &mut View = &mut place as &mut View; view.field = v`), rejoined
/// to the source leaf the recast judgment matched at the same offset and
/// width: the loaned place's root, its path followed by the leaf's segments,
/// and the carriers on both sides. `None` when the target is not such a
/// field, so other store routes keep their own replay.
pub(crate) struct RecordViewLeafDestination {
    pub(crate) root: SymbolHandle,
    pub(crate) path: Vec<CheckedUnitStructuralPathSegment>,
    pub(crate) view: PrimitiveType,
    pub(crate) leaf: PrimitiveType,
}

pub(crate) fn record_view_leaf_destination(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: checked_trees::expression::ExpressionHandle,
) -> Result<Option<RecordViewLeafDestination>, LoweringError> {
    let (owner, state) = source_custody::authored_state(checked, state_symbol)?;
    let ExpressionNode::Member(member) = checked.expression_table.expression(target) else {
        return Ok(None);
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(member.receiver) else {
        return Ok(None);
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || !member.member_symbol.is_valid()
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return Ok(None);
    }
    let Some((_, local)) = checked
        .state_local_declaration(state, name.symbol)
        .filter(|(ordinal, _)| *ordinal < statement_index)
    else {
        return Ok(None);
    };
    let TypeReferenceNode::Reference {
        access: language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::WriteOnly,
        referee,
        ..
    } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return Ok(None);
    };
    if local.is_mutable || checked.primitive_type_reference(*referee).is_some() {
        return Ok(None);
    }
    let mut cursor = local.initial_value;
    let mut recast = false;
    let place = loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Borrow(borrow)
                if matches!(
                    borrow.access,
                    language_core::ReferenceAccess::Mutable
                        | language_core::ReferenceAccess::WriteOnly
                ) =>
            {
                cursor = borrow.target;
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                recast = true;
                cursor = cast.value;
            }
            _ => break cursor,
        }
    };
    if !recast || !checked.expression_table.expression_is_valid(place) {
        return Ok(None);
    }
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index),
        place,
    )?;
    let source_type =
        validation::declared_place_type_raw(&checked.typed, owner, Some(state), place).ok_or(
            LoweringError::Unsupported("record view store lost its source place type"),
        )?;
    let Some(leaf) = validation::recast_record_source_leaf(
        &checked.typed,
        source_type,
        *referee,
        member.member_symbol,
    ) else {
        // Byte-backed views have no single typed source leaf. Their checked
        // destination remains the view-local projection: ordinary source
        // replay authenticates that path, and the captured-loan emitter
        // independently resolves its footprint into original backing bytes.
        return Ok(None);
    };
    let mut path = source.path;
    for segment in &leaf.path {
        path.push(match segment {
            validation::RecastLeafSegment::Field(symbol) => {
                CheckedUnitStructuralPathSegment::Field(
                    crate::scalar_graph::scalar_computations::fields::field_path_identity(
                        checked, *symbol,
                    )?,
                )
            }
            validation::RecastLeafSegment::Element(index) => {
                CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(*index).map_err(
                    |_| LoweringError::Unsupported("record view store element exceeds u64"),
                )?)
            }
        });
    }
    Ok(Some(RecordViewLeafDestination {
        root: source.root,
        path,
        view: leaf.view_primitive,
        leaf: leaf.primitive,
    }))
}

/// Emit the store's operations: the whole value lowered at the view carrier,
/// then one `WriteOnlyPrimitiveStore` per emitted footprint byte (or a single
/// store for a same-width leaf). Each byte selects `index + j` with its own
/// index obligation, exactly as the recast judgment's byte decomposition
/// requires.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    write: &CheckedRepresentationViewStorePlan,
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<OperationKind>, LoweringError> {
    // The value rejoin and any field selector read run inside the scalar
    // namespace; the dense form needs the resolved observations a byte
    // leaf's structural-field selector lowers against.
    let bindings = evaluation.scalar_bindings.clone().unwrap_or_else(|| {
        ScalarBindings::new(
            values
                .iter()
                .take(source_value_count)
                .map(|value| value.scalar_type),
        )
        .with_primitive_storage(&evaluation.primitive_storage)
        .with_view_locals(&evaluation.view_locals)
        .with_element_views(&evaluation.element_views)
        .with_structural_parameters(&evaluation.structural_parameters)
        .with_structural_locals(&evaluation.structural_locals)
        .with_resolved_structural_observations(
            &evaluation.structural_fields,
            &evaluation.structural_cases,
        )
    });
    let CheckedStoreRoot::Parameter { position } = write.destination else {
        return unsupported("representation view store has no parameter root");
    };
    let (_, parameter) = evaluation
        .structural_parameters
        .iter()
        .find(|(authored, _)| authored == &position)
        .ok_or(LoweringError::Unsupported(
            "representation view store parameter is absent",
        ))?;
    let view_scalar = terminal_scalar_type(write.view)?;
    let Some((leaf, prefix)) = write.path.split_last() else {
        return unsupported("representation view store lost its leaf segment");
    };
    let destination = parameter.place;
    let mut kinds = Vec::new();
    match leaf {
        // A whole-field leaf of the view's width: one primitive store whose
        // value is the authored value retagged to the leaf's declared
        // carrier.
        CheckedUnitStructuralPathSegment::Field(identity) => {
            let (steps, walked) =
                walk_carrier_prefix(parameter.structural_type, prefix, structural_types)?;
            let Walked::Type(owner) = walked else {
                return unsupported("representation view store leaf owner is not structural");
            };
            let declaration = declaration_at(structural_types, owner)?;
            let (StructuralTypeShape::Record { fields }
            | StructuralTypeShape::Mixed { fields, .. }) = &declaration.shape
            else {
                return unsupported("representation view store leaf owner is not a record");
            };
            let mut matching = fields
                .iter()
                .filter(|field| field.identity == *identity && !field.relevance.is_erased());
            let field = matching.next().ok_or(LoweringError::Unsupported(
                "representation view store leaf is absent",
            ))?;
            if matching.next().is_some() {
                return unsupported("representation view store leaf is ambiguous");
            }
            let leaf_scalar = leaf_scalar_type(&field.field_type)?;
            let indexes = runtime_elements::evaluate_selectors(
                &steps,
                checked,
                machine,
                state,
                write.statement_index,
                evaluation,
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
            )?;
            let path = runtime_elements::complete_path(steps, indexes, calls)?;
            let operand = lower_value(write, &bindings, checked, state, values, view_scalar)?;
            let casted = view_leaf_image(operand, view_scalar, leaf_scalar)?;
            let value = emit_direct_expression(&casted, values, next_value, operations);
            let range_obligation =
                if matches!(field.field_type, StructuralFieldType::BoundedInteger(_)) {
                    Some(obligation_id(allocate_dense(
                        &mut calls.next_obligation_identity,
                    )?))
                } else {
                    None
                };
            kinds.push(OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field: field.id,
                value,
                range_obligation,
            });
        }
        CheckedUnitStructuralPathSegment::FixedIndex(_)
        | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
            let destination = prepare_byte_destination(
                destination,
                parameter.structural_type,
                &write.path,
                checked,
                machine,
                state,
                write.statement_index,
                evaluation,
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                structural_types,
            )?;
            let operand = lower_value(write, &bindings, checked, state, values, view_scalar)?;
            let value = ValueDeclaration {
                qualifications: Default::default(),
                id: emit_direct_expression(&operand, values, next_value, operations),
                scalar_type: view_scalar,
            };
            kinds.extend(destination.emit(value, view_scalar, 0, next_value, operations, calls)?);
        }
        _ => {
            return unsupported("representation view store leaf path is incompatible");
        }
    }
    Ok(kinds)
}

/// An established byte location, with every selector evaluated before the RHS.
/// Primitive re-views and record projections share this physical write path.
pub(crate) struct ByteStoreDestination {
    destination: semantic_vocabulary::PlaceId,
    prefix_path: Vec<StructuralPathSegment>,
    index: ByteStoreIndex,
    capacity: u64,
}

enum ByteStoreIndex {
    Fixed(u64),
    Runtime(ValueId),
}

impl ByteStoreDestination {
    /// Add one declared array coordinate to the captured byte location. The
    /// dimension bound is independent of the enclosing byte capacity: an
    /// in-bounds byte address cannot justify selecting another array member.
    pub(crate) fn project_index(
        &mut self,
        index: ValueId,
        stride: u64,
        extent: u64,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
        let coordinate =
            crate::expression_preparation::bindings::recast_records::bounded_coordinate(
                LoweredDirectExpression::Parameter {
                    position: 0,
                    scalar_type,
                },
                extent,
            )?;
        let scaled = LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type,
            left: Box::new(coordinate),
            right: Box::new(LoweredDirectExpression::IntegerLiteral {
                value: IntegerValue::Unsigned(u128::from(stride)),
                scalar_type,
            }),
        };
        let index_value = ValueDeclaration {
            id: index,
            scalar_type,
            qualifications: Default::default(),
        };
        let scaled = emit_direct_expression(&scaled, &[index_value], next_value, operations);
        let base = match self.index {
            ByteStoreIndex::Fixed(index) => emit_scalar_leaf(
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(u128::from(index)),
                },
                scalar_type,
                next_value,
                operations,
            ),
            ByteStoreIndex::Runtime(index) => index,
        };
        let base_value = ValueDeclaration {
            id: base,
            ..index_value
        };
        let scaled_value = ValueDeclaration {
            id: scaled,
            ..index_value
        };
        let sum = LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactAdd,
            scalar_type,
            left: Box::new(LoweredDirectExpression::Parameter {
                position: 0,
                scalar_type,
            }),
            right: Box::new(LoweredDirectExpression::Parameter {
                position: 1,
                scalar_type,
            }),
        };
        self.index = ByteStoreIndex::Runtime(emit_direct_expression(
            &sum,
            &[base_value, scaled_value],
            next_value,
            operations,
        ));
        Ok(())
    }

    /// Prove semantic-to-stored representability before extracting any bytes.
    /// Reusing the completed SSA value is essential: repeated RHS reads could
    /// observe bytes already changed by earlier stores in the same assignment.
    pub(crate) fn emit(
        self,
        value: ValueDeclaration,
        stored_type: ScalarType,
        field_offset: u64,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Vec<OperationKind>, LoweringError> {
        let (ScalarType::Integer(source_integer), ScalarType::Integer(stored_integer)) =
            (value.scalar_type, stored_type)
        else {
            return unsupported("byte representation store requires an integer carrier");
        };
        if !stored_integer.bits().is_multiple_of(8) {
            return unsupported("byte representation store requires a whole-byte encoding");
        }
        let encoded = if value.scalar_type == stored_type {
            value
        } else {
            let kind = if source_integer.can_widen_to(stored_integer) {
                OperationKind::IntegerWiden { operand: value.id }
            } else {
                OperationKind::IntegerExactCast {
                    operand: value.id,
                    obligation: obligation_id(allocate_dense(&mut calls.next_obligation_identity)?),
                }
            };
            ValueDeclaration {
                qualifications: Default::default(),
                id: emit_scalar_leaf(kind, stored_type, next_value, operations),
                scalar_type: stored_type,
            }
        };
        let byte_scalar = terminal_scalar_type(PrimitiveType::U8)?;
        let count_scalar = terminal_scalar_type(PrimitiveType::U64)?;
        let byte_count = u64::from(stored_integer.bits()) / 8;
        let mut kinds = Vec::new();
        for byte_offset in 0..byte_count {
            let offset =
                field_offset
                    .checked_add(byte_offset)
                    .ok_or(LoweringError::Unsupported(
                        "byte representation store offset overflows",
                    ))?;
            let mut path = self.prefix_path.clone();
            if let ByteStoreIndex::Runtime(base) = self.index {
                let index = if offset == 0 {
                    base
                } else {
                    let step = emit_scalar_leaf(
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(u128::from(offset)),
                        },
                        count_scalar,
                        next_value,
                        operations,
                    );
                    emit_scalar_leaf(
                        OperationKind::ExactIntegerAdd {
                            left: base,
                            right: step,
                            obligation: obligation_id(allocate_dense(
                                &mut calls.next_obligation_identity,
                            )?),
                        },
                        count_scalar,
                        next_value,
                        operations,
                    )
                };
                path.push(StructuralPathSegment::RuntimeIndex {
                    index,
                    obligation: calls.allocate_requirement()?,
                });
            } else {
                let ByteStoreIndex::Fixed(fixed_index) = self.index else {
                    return unsupported("byte representation store lost its coordinate");
                };
                let selected =
                    fixed_index
                        .checked_add(offset)
                        .ok_or(LoweringError::Unsupported(
                            "byte representation store index overflows",
                        ))?;
                if selected >= self.capacity {
                    return unsupported("byte representation store exceeds its backing capacity");
                }
                path.push(StructuralPathSegment::FixedIndex(selected));
            }
            let operand = LoweredDirectExpression::Parameter {
                position: 0,
                scalar_type: stored_type,
            };
            let shifted = if byte_offset == 0 {
                operand
            } else {
                LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::WrappingShiftRight,
                    scalar_type: stored_type,
                    left: Box::new(operand),
                    right: Box::new(LoweredDirectExpression::IntegerLiteral {
                        value: IntegerValue::Unsigned(u128::from(byte_offset * 8)),
                        scalar_type: stored_type,
                    }),
                }
            };
            let byte_expression = wrapping_cast(shifted, stored_integer, byte_scalar)?;
            let byte = emit_direct_expression(&byte_expression, &[encoded], next_value, operations);
            kinds.push(OperationKind::WriteOnlyPrimitiveStore {
                destination: self.destination,
                path,
                value: byte,
            });
        }
        Ok(kinds)
    }
}

/// Resolve a captured byte path using the same carrier walk as a primitive
/// representation view. The caller owns root authority and evaluates the RHS
/// only after this returns, preserving ordinary assignment sequencing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_byte_destination(
    destination: semantic_vocabulary::PlaceId,
    root_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ByteStoreDestination, LoweringError> {
    let Some((leaf, prefix)) = path.split_last() else {
        return unsupported("byte representation store has no backing coordinate");
    };
    let (mut steps, walked) = walk_carrier_prefix(root_type, prefix, structural_types)?;
    let capacity = byte_capacity(walked, structural_types)?;
    let fixed_index = match leaf {
        CheckedUnitStructuralPathSegment::FixedIndex(index) => *index,
        CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
            steps.push(runtime_elements::selector_step(selector));
            0
        }
        _ => return unsupported("byte representation store has no element coordinate"),
    };
    let mut indexes = runtime_elements::evaluate_selectors(
        &steps,
        checked,
        machine,
        state,
        statement_index,
        evaluation,
        source_value_count,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        calls,
    )?;
    let runtime_index = if matches!(leaf, CheckedUnitStructuralPathSegment::RuntimeIndex(_)) {
        steps.pop();
        Some(indexes.pop().ok_or(LoweringError::Unsupported(
            "byte store lost its evaluated coordinate",
        ))?)
    } else {
        None
    };
    let prefix_path = runtime_elements::complete_path(steps, indexes, calls)?;
    Ok(ByteStoreDestination {
        destination,
        prefix_path,
        index: match runtime_index {
            Some(value) => ByteStoreIndex::Runtime(value),
            None => ByteStoreIndex::Fixed(fixed_index),
        },
        capacity,
    })
}

fn byte_capacity(
    walked: Walked<'_>,
    types: &[StructuralTypeDeclaration],
) -> Result<u64, LoweringError> {
    match walked {
        Walked::Carrier(ByteSequenceCarrier::BoundedOwned { capacity, .. }) => Ok(*capacity),
        Walked::Type(array) => {
            let StructuralTypeShape::FixedArray { element, length } =
                &declaration_at(types, array)?.shape
            else {
                return unsupported("byte representation store carrier is not a fixed array");
            };
            let StructuralTypeShape::PrimitiveScalar(scalar) =
                &declaration_at(types, *element)?.shape
            else {
                return unsupported("byte representation store carrier element is not scalar");
            };
            if *scalar != terminal_scalar_type(PrimitiveType::U8)? {
                return unsupported("byte representation store carrier element is not a byte");
            }
            Ok(*length)
        }
        _ => unsupported("byte representation store requires a bounded backing"),
    }
}

/// Where the carrier prefix's walk ends: inside a structural type (the
/// same-width leaf's record owner, or an array/view element's parent) or on
/// a byte carrier's field, whose `u8` elements the byte arm indexes.
enum Walked<'a> {
    Type(StructuralTypeId),
    Carrier(&'a ByteSequenceCarrier),
}

/// Walk the checked store path's carrier prefix beneath the destination
/// parameter's structural type: `Field` segments resolve against their
/// record's unique relevant field, `FixedIndex` bounds-checks a fixed
/// array, and `RuntimeIndex` mints the matching projection step. The walk
/// ends at the leaf's owner — a record for the same-width arm, a byte
/// carrier for the byte arm.
fn walk_carrier_prefix<'a>(
    root_type: StructuralTypeId,
    prefix: &[CheckedUnitStructuralPathSegment],
    types: &'a [StructuralTypeDeclaration],
) -> Result<(Vec<ProjectionStep>, Walked<'a>), LoweringError> {
    let mut walked = Walked::Type(root_type);
    let mut steps = Vec::with_capacity(prefix.len());
    for segment in prefix {
        let Walked::Type(owner) = walked else {
            return unsupported("representation view store carrier path is incompatible");
        };
        let declaration = declaration_at(types, owner)?;
        match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                let (StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. }) = &declaration.shape
                else {
                    return unsupported("representation view store carrier is not a record");
                };
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == *identity && !field.relevance.is_erased());
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "representation view store carrier field is absent",
                ))?;
                if matching.next().is_some() {
                    return unsupported("representation view store carrier field is ambiguous");
                }
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identity.clone(),
                )));
                walked = match &field.field_type {
                    StructuralFieldType::Structural(nested) => Walked::Type(*nested),
                    StructuralFieldType::ByteSequence(carrier)
                    | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
                        Walked::Carrier(carrier)
                    }
                    _ => {
                        return unsupported(
                            "representation view store carrier has a scalar member",
                        );
                    }
                };
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                    return unsupported(
                        "representation view store fixed index requires a fixed array",
                    );
                };
                if *index >= *length {
                    return unsupported("representation view store fixed index is out of bounds");
                }
                steps.push(ProjectionStep::Static(StructuralPathSegment::FixedIndex(
                    *index,
                )));
                walked = Walked::Type(*element);
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(runtime) => {
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, .. }
                    | StructuralTypeShape::ElementView { element } => *element,
                    _ => {
                        return unsupported(
                            "representation view store runtime index requires an array",
                        );
                    }
                };
                let step = match runtime {
                    CheckedRuntimeIndex::AssignmentIndex { depth } => {
                        ProjectionStep::AssignmentIndex { depth: *depth }
                    }
                    // A borrow-mirrored index names the calling state's own
                    // scalar parameter, which the scalar source frame reads
                    // directly rather than a bound selector row.
                    CheckedRuntimeIndex::Parameter { position } => {
                        ProjectionStep::RuntimeIndexParameter {
                            position: *position,
                        }
                    }
                    _ => {
                        return unsupported(
                            "representation view store carrier index is unsupported",
                        );
                    }
                };
                // A path contains occurrences, not a set of selectors:
                // identical indexes at different depths still select twice.
                steps.push(step);
                walked = Walked::Type(element);
            }
            _ => return unsupported("representation view store carrier path is incompatible"),
        }
    }
    Ok((steps, walked))
}

/// Retag the authored store value from the view's carrier onto the leaf's
/// declared carrier. Two integer carriers spell the modular image through
/// `wrapping_cast`, exactly like the read side's equal-width re-view.
///
/// An integer view reaching a Boolean leaf lands the leaf's `{0, 1}` bit
/// image as `operand != 0` — total for every admitted operand, and exact
/// for the bounded views the recast judgment restricts to a one-byte
/// carrier. A Boolean view reaching an integer leaf lands `0` or `1`: a
/// runtime Boolean needs the conditional landing only the scalar graph's
/// branch machinery spells, so the flat form admits the `BooleanConstant`
/// whose value folds to the literal and refuses every other Boolean
/// expression.
fn view_leaf_image(
    operand: LoweredDirectExpression,
    view: ScalarType,
    leaf: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    match (view, leaf) {
        (ScalarType::Integer(view_integer), ScalarType::Integer(_)) => {
            wrapping_cast(operand, view_integer, leaf)
        }
        (ScalarType::Integer(_), ScalarType::Boolean) => {
            let zero = match view {
                ScalarType::Integer(integer) => match integer.sign() {
                    semantic_vocabulary::IntegerSign::Signed => IntegerValue::Signed(0),
                    semantic_vocabulary::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
                },
                _ => unreachable!("integer view checked above"),
            };
            Ok(LoweredDirectExpression::Boolean {
                expression: Box::new(LoweredBooleanReturnExpression::Not {
                    operand: Box::new(LoweredBooleanReturnExpression::IntegerComparison {
                        kind: LoweredIntegerComparisonKind::Equal,
                        left: Box::new(operand),
                        right: Box::new(LoweredDirectExpression::IntegerLiteral {
                            value: zero,
                            scalar_type: view,
                        }),
                    }),
                }),
            })
        }
        (ScalarType::Boolean, ScalarType::Integer(_)) => match operand {
            LoweredDirectExpression::Boolean { expression } => {
                let LoweredBooleanReturnExpression::Constant { value } = *expression else {
                    return unsupported(
                        "representation view store cannot land a runtime Boolean on an integer leaf",
                    );
                };
                let ScalarType::Integer(carrier) = leaf else {
                    unreachable!("integer leaf checked above");
                };
                let value = match carrier.sign() {
                    semantic_vocabulary::IntegerSign::Signed => IntegerValue::Signed(value.into()),
                    semantic_vocabulary::IntegerSign::Unsigned => {
                        IntegerValue::Unsigned(value.into())
                    }
                };
                Ok(LoweredDirectExpression::IntegerLiteral {
                    value,
                    scalar_type: leaf,
                })
            }
            _ => unsupported(
                "representation view store cannot land a runtime Boolean on an integer leaf",
            ),
        },
        _ => unsupported("representation view store pairs incompatible carriers"),
    }
}

/// The scalar type of a primitive leaf field's declared carrier.
fn leaf_scalar_type(field_type: &StructuralFieldType) -> Result<ScalarType, LoweringError> {
    match field_type {
        StructuralFieldType::Scalar(scalar) => Ok(*scalar),
        StructuralFieldType::BoundedInteger(bounded) => {
            Ok(ScalarType::Integer(bounded.integer_type()))
        }
        _ => unsupported("representation view store leaf is not an integer scalar"),
    }
}

/// One structural type declaration by identity.
fn declaration_at(
    types: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Result<&StructuralTypeDeclaration, LoweringError> {
    let mut matching = types
        .iter()
        .filter(|declaration| declaration.id == identity);
    let declaration = matching.next().ok_or(LoweringError::Unsupported(
        "representation view store carrier type is absent",
    ))?;
    if matching.next().is_some() {
        return unsupported("representation view store carrier type is ambiguous");
    }
    Ok(declaration)
}

/// Lower the store's checked value to the view carrier: a bound
/// `AssignmentValue` expression, or the statement's own scalar result read
/// from the dense frame's last position.
fn lower_value(
    write: &CheckedRepresentationViewStorePlan,
    bindings: &ScalarBindings,
    checked: &CheckedTrees,
    state: SymbolHandle,
    values: &[ValueDeclaration],
    view_scalar: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    match &write.value {
        CheckedStoreValue::Pure(_) => bindings.expression_at(
            checked,
            state,
            write.statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
        ),
        CheckedStoreValue::ScalarResult { position } => {
            let position = usize::try_from(*position).map_err(|_| {
                LoweringError::Unsupported(
                    "representation view store's scalar result position overflows",
                )
            })?;
            if position.checked_add(1) != Some(values.len()) {
                return unsupported(
                    "representation view store's scalar result is not the latest value",
                );
            }
            if values[position].scalar_type != view_scalar {
                return unsupported("representation view store's scalar result changed its type");
            }
            Ok(LoweredDirectExpression::Local {
                position,
                scalar_type: view_scalar,
            })
        }
        CheckedStoreValue::Computation(_) => {
            unsupported("representation view store kept a computation")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrier_path_retains_repeated_runtime_selector_occurrences() {
        let byte = crate::terminal_identities::structural_type_id(1);
        let inner = crate::terminal_identities::structural_type_id(2);
        let outer = crate::terminal_identities::structural_type_id(3);
        let types = [
            StructuralTypeDeclaration {
                id: byte,
                identity: "u8".into(),
                shape: StructuralTypeShape::PrimitiveScalar(
                    terminal_scalar_type(PrimitiveType::U8).unwrap(),
                ),
            },
            StructuralTypeDeclaration {
                id: inner,
                identity: "[u8;2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: byte,
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                id: outer,
                identity: "[[u8;2];2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: inner,
                    length: 2,
                },
            },
        ];
        let selector =
            CheckedUnitStructuralPathSegment::RuntimeIndex(CheckedRuntimeIndex::Parameter {
                position: 0,
            });
        let (steps, terminal) =
            walk_carrier_prefix(outer, &[selector.clone(), selector], &types).unwrap();
        assert_eq!(
            steps,
            vec![ProjectionStep::RuntimeIndexParameter { position: 0 }; 2]
        );
        assert!(matches!(terminal, Walked::Type(leaf) if leaf == byte));
    }
}
