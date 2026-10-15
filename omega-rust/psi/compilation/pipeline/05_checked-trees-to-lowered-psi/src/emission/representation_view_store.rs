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
    CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError, OperationKind, PrimitiveType,
    ScalarType, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, ValueDeclaration, ValueId, allocate_dense,
    emit_direct_expression, obligation_id, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::{LoweredDirectExpression, emit_scalar_leaf};
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::emission::runtime_elements::{self, ProjectionStep};
use crate::expression_preparation::integer_bit_image::integer_bit_image;
use crate::structural_types::unique_declaration;
use checked_trees::{CheckedRuntimeIndex, CheckedStorePlan, CheckedStoreRoot};
use semantic_vocabulary::IntegerValue;
use symbols::SymbolHandle;
use terminal_psi::ByteSequenceCarrier;

/// Emit the store's operations: the whole value lowered at the view carrier,
/// then one `WriteOnlyPrimitiveStore` per emitted footprint byte (or a single
/// store for a same-width leaf). Each byte selects `index + j` with its own
/// index obligation, exactly as the recast judgment's byte decomposition
/// requires.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    store: &CheckedStorePlan,
    view: PrimitiveType,
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
    // The value rejoin and any field selector read run inside the body's
    // scalar namespace, with the resolved observations a byte leaf's
    // structural-field selector lowers against.
    let bindings = evaluation
        .scalar_bindings
        .clone()
        .with_resolved_structural_observations(
            &evaluation.structural_fields,
            &evaluation.structural_cases,
        );
    let CheckedStoreRoot::Parameter { position } = store.destination.root else {
        return unsupported("representation view store has no parameter root");
    };
    let Some((role, value)) = store.value.scalar() else {
        return unsupported("representation view store stores no scalar value");
    };
    let (_, parameter) = evaluation
        .structural_parameters
        .iter()
        .find(|(authored, _)| authored == &position)
        .ok_or(LoweringError::Unsupported(
            "representation view store parameter is absent",
        ))?;
    let view_scalar = terminal_scalar_type(view)?;
    let Some((leaf, prefix)) = store.destination.path.split_last() else {
        return unsupported("representation view store lost its leaf segment");
    };
    let destination = parameter.place;
    let mut kinds = Vec::new();
    match leaf {
        // A whole-field leaf of the view's width: one primitive store whose
        // value is the authored value retagged to the leaf's declared
        // carrier.
        CheckedUnitStructuralPathSegment::Field(identity) => {
            let (steps, walked) = walk_carrier_prefix(
                identities,
                parameter.structural_type,
                prefix,
                structural_types,
            )?;
            let Walked::Type(owner) = walked else {
                return unsupported("representation view store leaf owner is not structural");
            };
            let declaration = unique_declaration(
                structural_types,
                owner,
                "representation view store carrier type is absent",
                "representation view store carrier type is ambiguous",
            )?;
            let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &declaration.shape else {
                return unsupported("representation view store leaf owner is not a record");
            };
            let mut matching = fields.iter().filter(|field| {
                field.identity == identities.intern(&identity.spelling())
                    && !field.relevance.is_erased()
            });
            let field = matching.next().ok_or(LoweringError::Unsupported(
                "representation view store leaf is absent",
            ))?;
            if matching.next().is_some() {
                return unsupported("representation view store leaf is ambiguous");
            }
            let leaf_scalar = leaf_scalar_type(&field.field_type)?;
            let indexes = runtime_elements::evaluate_selectors(
                identities,
                &steps,
                checked,
                machine,
                state,
                store.statement_index,
                evaluation,
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
            )?;
            let path = runtime_elements::complete_path(steps, indexes, calls, operations)?;
            let operand = crate::emission::structural_scalar_store_source::direct_value(
                identities,
                checked,
                state,
                store.statement_index,
                role,
                value,
                view_scalar,
                &bindings,
                values,
            )?;
            let casted = view_leaf_image(operand, view_scalar, leaf_scalar)?;
            let value = emit_direct_expression(&casted, values, next_value, operations);
            let range_obligation =
                if terminal_semantics::scalar_field_requires_evidence(&field.field_type) {
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
                identities,
                destination,
                parameter.structural_type,
                &store.destination.path,
                None,
                false,
                checked,
                machine,
                state,
                store.statement_index,
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
            let operand = crate::emission::structural_scalar_store_source::direct_value(
                identities,
                checked,
                state,
                store.statement_index,
                role,
                value,
                view_scalar,
                &bindings,
                values,
            )?;
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
    /// `true` when the destination's byte window was decided at source
    /// checking — a recast loan's formation admission proved
    /// `index + view footprint <= region length` — so the per-byte
    /// obligations emitted below are source-checked rather than re-proven.
    decided: bool,
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
            crate::expression_preparation::bindings::place_observations::bounded_coordinate(
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
            // The bounded coordinate lies in `[0, extent - 1]`, so the
            // product fits whenever `extent * stride` does — a fact the
            // checked layout supplies.
            formation_decided: extent.checked_mul(stride).is_some(),
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
            // The recast admission proved `base + footprint <= capacity` for
            // this coordinate, and `scaled` is a byte offset inside that
            // footprint: `base + scaled` stays below capacity, so the add is
            // representable already.
            formation_decided: true,
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
                    let add_obligation =
                        obligation_id(allocate_dense(&mut calls.next_obligation_identity)?);
                    // `base + offset < base + footprint <= capacity`: the
                    // byte offset stays inside the admitted window, so the
                    // add is representable already.
                    if self.decided {
                        operations.source_checked_obligations.push(add_obligation);
                    }
                    emit_scalar_leaf(
                        OperationKind::ExactIntegerAdd {
                            left: base,
                            right: step,
                            obligation: add_obligation,
                        },
                        count_scalar,
                        next_value,
                        operations,
                    )
                };
                let obligation = calls.allocate_requirement()?;
                // The selected byte lies below `base + footprint`, which the
                // formation admission bounded by the region length.
                if self.decided {
                    operations.source_checked_obligations.push(obligation);
                }
                path.push(StructuralPathSegment::RuntimeIndex { index, obligation });
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
                    formation_decided: false,
                }
            };
            let byte_expression = integer_bit_image(shifted, stored_integer, byte_scalar)?;
            let byte = emit_direct_expression(&byte_expression, &[encoded], next_value, operations);
            kinds.push(OperationKind::WriteOnlyPrimitiveStore {
                range_obligation: None,
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    destination: semantic_vocabulary::PlaceId,
    root_type: StructuralTypeId,
    path: &[CheckedUnitStructuralPathSegment],
    captured_index: Option<ValueId>,
    decided: bool,
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
    let (mut steps, walked) = walk_carrier_prefix(identities, root_type, prefix, structural_types)?;
    let capacity = byte_capacity(walked, structural_types)?;
    let fixed_index = match leaf {
        CheckedUnitStructuralPathSegment::FixedIndex(index) => *index,
        CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
            if captured_index.is_none() {
                steps.push(runtime_elements::selector_step(selector));
            }
            0
        }
        _ => return unsupported("byte representation store has no element coordinate"),
    };
    let mut indexes = runtime_elements::evaluate_selectors(
        identities,
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
    if captured_index.is_some()
        && !matches!(leaf, CheckedUnitStructuralPathSegment::RuntimeIndex(_))
    {
        return unsupported("byte store snapshot replaced a fixed coordinate");
    }
    let runtime_index = if let Some(index) = captured_index {
        Some(index)
    } else if matches!(leaf, CheckedUnitStructuralPathSegment::RuntimeIndex(_)) {
        steps.pop();
        Some(
            indexes
                .pop()
                .ok_or(LoweringError::Unsupported(
                    "byte store lost its evaluated coordinate",
                ))?
                .index,
        )
    } else {
        None
    };
    let prefix_path = runtime_elements::complete_path(steps, indexes, calls, operations)?;
    Ok(ByteStoreDestination {
        destination,
        prefix_path,
        index: match runtime_index {
            Some(value) => ByteStoreIndex::Runtime(value),
            None => ByteStoreIndex::Fixed(fixed_index),
        },
        capacity,
        decided,
    })
}

fn byte_capacity(
    walked: Walked<'_>,
    types: &[StructuralTypeDeclaration],
) -> Result<u64, LoweringError> {
    match walked {
        Walked::Carrier(ByteSequenceCarrier::BoundedOwned { capacity, .. }) => Ok(*capacity),
        Walked::Type(array) => {
            let StructuralTypeShape::FixedArray { element, length } = &unique_declaration(
                types,
                array,
                "representation view store carrier type is absent",
                "representation view store carrier type is ambiguous",
            )?
            .shape
            else {
                return unsupported("byte representation store carrier is not a fixed array");
            };
            let StructuralTypeShape::PrimitiveScalar(scalar) = &unique_declaration(
                types,
                *element,
                "representation view store carrier type is absent",
                "representation view store carrier type is ambiguous",
            )?
            .shape
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        let declaration = unique_declaration(
            types,
            owner,
            "representation view store carrier type is absent",
            "representation view store carrier type is ambiguous",
        )?;
        match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &declaration.shape
                else {
                    return unsupported("representation view store carrier is not a record");
                };
                let mut matching = fields.iter().filter(|field| {
                    field.identity == identities.intern(&identity.spelling())
                        && !field.relevance.is_erased()
                });
                let field = matching.next().ok_or(LoweringError::Unsupported(
                    "representation view store carrier field is absent",
                ))?;
                if matching.next().is_some() {
                    return unsupported("representation view store carrier field is ambiguous");
                }
                steps.push(ProjectionStep::Static(StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
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
                    | StructuralTypeShape::ElementView { element, .. } => *element,
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
/// `integer_bit_image`, exactly like the read side's equal-width re-view.
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
            integer_bit_image(operand, view_integer, leaf)
        }
        (ScalarType::Integer(_), ScalarType::Boolean) => {
            let zero = match view {
                ScalarType::Integer(integer) => integer.zero(),
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
                let Some(carrier) = leaf.as_integer() else {
                    unreachable!("integer leaf checked above");
                };
                let value = IntegerValue::for_sign(carrier.sign(), i128::from(value))
                    .expect("a Boolean leaf fits every integer carrier");
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
        StructuralFieldType::RefinedScalar(refinement) => Ok(refinement.carrier()),
        _ => unsupported("representation view store leaf is not an integer scalar"),
    }
}
