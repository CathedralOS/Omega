//! Lowering checked scalar and boolean expressions.
//!
//! Bounds decisions join each retained selector's exact authored occurrence
//! while preparing that read, regardless of its surrounding expression. The
//! row root is not a selector identity: projections, comparisons and nested
//! indexing contain independent reads. Binding rebases preserve the selected
//! storage and value; generated selectors have no authored occurrence. In
//! particular, a representation view's byte expansion cannot inherit this
//! decision for the additional offsets it creates.

use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment,
};
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::bindings::structural_fields::CasePayloadRead;
use crate::expression_preparation::bindings::view_locals::{ViewCarrier, ViewLocalBinding};
use crate::expression_preparation::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, LoweringError, PlaceId, PrimitiveType, ScalarType,
    StructuralParameterDeclaration, StructuralTypeId, unsupported,
};

/// Lower one checked scalar expression against no prepared sources. A
/// producer that retains structural bindings, view locals or scalar homes
/// builds a `PreparedExpressionContext` and calls `lower_scalar` instead.
pub(crate) fn lower_checked_scalar_expression(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedScalarExpression,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
) -> Result<LoweredDirectExpression, LoweringError> {
    let structural_fields =
        crate::expression_preparation::bindings::StructuralFieldBindings::default();
    PreparedExpressionContext {
        structural_fields: &structural_fields,
        structural_cases: &[],
        primitive_storage: &[],
        view_locals: &[],
        structural_locals: &[],
        scalar_bindings: None,
        proven_indexes,
    }
    .lower_scalar(identities, expression)
}

/// Range checking publishes these handles in arena-position/generation order.
/// Consume that ordering without another index allocation or source walk.
pub(crate) fn selector_bound_is_decided(
    selector: &checked_trees::CheckedScalarSelector,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
) -> bool {
    selector.occurrence.is_valid()
        && proven_indexes
            .binary_search_by_key(
                &checked_trees::operators::handle_position(selector.occurrence),
                |occurrence| checked_trees::operators::handle_position(*occurrence),
            )
            .is_ok()
}

/// The prepared sources one checked-expression traversal borrows: the body's
/// field and case bindings, primitive storage, view locals and structural
/// locals, the authored scalar bindings, and the range checker's proven
/// selectors.
///
/// The producing body prepares these once and spells them at its single entry.
/// Every recursive step and every bounded sub-operation borrows this context,
/// so a new prepared source joins the traversal, its length and byte
/// observations and its runtime selector resolution in one place instead of at
/// fifteen parameter lists re-spelled at each call.
pub(crate) struct PreparedExpressionContext<'a> {
    pub(crate) structural_fields:
        &'a crate::expression_preparation::bindings::StructuralFieldBindings,
    pub(crate) structural_cases:
        &'a [crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    pub(crate) primitive_storage: &'a [(symbols::SymbolHandle, PlaceId, ScalarType)],
    pub(crate) view_locals: &'a [ViewLocalBinding],
    pub(crate) structural_locals:
        &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
    pub(crate) scalar_bindings: Option<&'a crate::expression_preparation::bindings::ScalarBindings>,
    pub(crate) proven_indexes: &'a [checked_trees::expression::ExpressionHandle],
}

impl<'a> PreparedExpressionContext<'a> {
    /// Lower one checked scalar expression against these prepared sources.
    pub(crate) fn lower_scalar(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        match expression {
            CheckedScalarExpression::StructuralSequenceLength { root, path } => self
                .sequence_observation(identities, *root, path, |identities, selector| {
                    self.lower_scalar(identities, selector)
                })?
                .ok_or(LoweringError::Unsupported(
                    "length observation requires a live sequence descriptor",
                )),
            CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            } => {
                let scalar_type = terminal_scalar_type(*primitive_type)?;
                if let Some(bindings) = self.scalar_bindings
                && let Some(observation) =
                    crate::expression_preparation::bindings::primitive_reads::captured_observation(
                        bindings,
                        *symbol,
                        scalar_type,
                    )?
            {
                return Ok(observation);
            }
                let (source, path) =
                    crate::expression_preparation::bindings::primitive_storage_read(
                        self.primitive_storage,
                        self.structural_locals,
                        self.structural_fields.declarations(),
                        *symbol,
                        scalar_type,
                    )?;
                Ok(LoweredDirectExpression::PrimitiveRead {
                    qualifications: Default::default(),
                    source: source.into(),
                    path,
                    scalar_type,
                })
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            } => Ok(LoweredDirectExpression::Parameter {
                position: *position,
                scalar_type: terminal_scalar_type(*primitive_type)?,
            }),
            CheckedScalarExpression::ErasedParameter {
                position,
                primitive_type,
            } => Ok(LoweredDirectExpression::ErasedParameter {
                position: *position,
                scalar_type: terminal_scalar_type(*primitive_type)?,
            }),
            CheckedScalarExpression::Local {
                position,
                primitive_type,
            } => Ok(LoweredDirectExpression::Local {
                position: *position,
                scalar_type: terminal_scalar_type(*primitive_type)?,
            }),
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                primitive_type,
            } => {
                let scalar_type = terminal_scalar_type(*primitive_type)?;
                // Carrier type does not choose the storage path. Case payloads,
                // indexed elements and inline fields retain the same authority
                // and geometry for every scalar format.
                if matches!(
                    path.first(),
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                        _
                    ))
                ) {
                    return Ok(
            match crate::expression_preparation::bindings::structural_fields::resolve_case_payload(
                        identities,
                        self.structural_fields,
                        *parameter_position,
                        path,
                        scalar_type,
                    )? {
                        CasePayloadRead::Observation { expression } => expression,
                        CasePayloadRead::Established { position } => {
                            LoweredDirectExpression::Parameter {
                                position,
                                scalar_type,
                            }
                        }
                    },
                );
                }
                crate::expression_preparation::bindings::structural_fields::resolve(
                    identities,
                    self.structural_fields,
                    *parameter_position,
                    path,
                    scalar_type,
                )
            }
            CheckedScalarExpression::IntegerLiteral { literal } => {
                let scalar_type = integer_landing_scalar_type(literal)?;
                Ok(LoweredDirectExpression::IntegerLiteral {
                    value: integer_value(literal, scalar_type)?,
                    scalar_type,
                })
            }
            CheckedScalarExpression::IeeeFloatLiteral { value } => {
                Ok(LoweredDirectExpression::IeeeFloatLiteral { value: *value })
            }
            CheckedScalarExpression::ImplementationSelection {
                implementation,
                interface_identity,
            } => Ok(LoweredDirectExpression::ImplementationSelection {
                implementation: *implementation,
                interface_identity: *interface_identity,
                scalar_type: crate::emission::scalar_types::selector_scalar_type(),
            }),
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                formation_decided,
            } => {
                let scalar_type = terminal_scalar_type(*primitive_type)?;
                let left = self.lower_scalar(identities, left)?;
                let right = self.lower_scalar(identities, right)?;
                Ok(LoweredDirectExpression::IntegerBinary {
                    kind: match kind {
                        CheckedIntegerBinaryKind::ExactAdd => LoweredIntegerBinaryKind::ExactAdd,
                        CheckedIntegerBinaryKind::ExactSubtract => {
                            LoweredIntegerBinaryKind::ExactSubtract
                        }
                        CheckedIntegerBinaryKind::ExactMultiply => {
                            LoweredIntegerBinaryKind::ExactMultiply
                        }
                        CheckedIntegerBinaryKind::ExactDivide => {
                            LoweredIntegerBinaryKind::ExactDivide
                        }
                        CheckedIntegerBinaryKind::ExactRemainder => {
                            LoweredIntegerBinaryKind::ExactRemainder
                        }
                        CheckedIntegerBinaryKind::WrappingDivide => {
                            LoweredIntegerBinaryKind::WrappingDivide
                        }
                        CheckedIntegerBinaryKind::WrappingRemainder => {
                            LoweredIntegerBinaryKind::WrappingRemainder
                        }
                        CheckedIntegerBinaryKind::SaturatingDivide => {
                            LoweredIntegerBinaryKind::SaturatingDivide
                        }
                        CheckedIntegerBinaryKind::SaturatingRemainder => {
                            LoweredIntegerBinaryKind::SaturatingRemainder
                        }
                        CheckedIntegerBinaryKind::WrappingAdd => {
                            LoweredIntegerBinaryKind::WrappingAdd
                        }
                        CheckedIntegerBinaryKind::SaturatingAdd => {
                            LoweredIntegerBinaryKind::SaturatingAdd
                        }
                        CheckedIntegerBinaryKind::WrappingSubtract => {
                            LoweredIntegerBinaryKind::WrappingSubtract
                        }
                        CheckedIntegerBinaryKind::SaturatingSubtract => {
                            LoweredIntegerBinaryKind::SaturatingSubtract
                        }
                        CheckedIntegerBinaryKind::WrappingMultiply => {
                            LoweredIntegerBinaryKind::WrappingMultiply
                        }
                        CheckedIntegerBinaryKind::SaturatingMultiply => {
                            LoweredIntegerBinaryKind::SaturatingMultiply
                        }
                        CheckedIntegerBinaryKind::BitwiseAnd => {
                            LoweredIntegerBinaryKind::BitwiseAnd
                        }
                        CheckedIntegerBinaryKind::BitwiseOr => LoweredIntegerBinaryKind::BitwiseOr,
                        CheckedIntegerBinaryKind::BitwiseXor => {
                            LoweredIntegerBinaryKind::BitwiseXor
                        }
                        CheckedIntegerBinaryKind::WrappingShiftLeft => {
                            LoweredIntegerBinaryKind::WrappingShiftLeft
                        }
                        CheckedIntegerBinaryKind::WrappingShiftRight => {
                            LoweredIntegerBinaryKind::WrappingShiftRight
                        }
                        CheckedIntegerBinaryKind::ExactShiftLeft => {
                            LoweredIntegerBinaryKind::ExactShiftLeft
                        }
                        CheckedIntegerBinaryKind::ExactShiftRight => {
                            LoweredIntegerBinaryKind::ExactShiftRight
                        }
                        CheckedIntegerBinaryKind::SaturatingShiftRight => {
                            LoweredIntegerBinaryKind::ExactShiftRight
                        }
                        CheckedIntegerBinaryKind::SaturatingShiftLeft => {
                            return super::saturating_shift::left(scalar_type, left, right);
                        }
                        // Each Trapping primitive keeps its own Terminal operation and
                        // operation-level crash site; it is never weakened into the
                        // Exact, Wrapping, or Saturating sibling.
                        CheckedIntegerBinaryKind::TrappingShiftLeft => {
                            LoweredIntegerBinaryKind::TrappingShiftLeft
                        }
                        CheckedIntegerBinaryKind::TrappingShiftRight => {
                            LoweredIntegerBinaryKind::TrappingShiftRight
                        }
                        CheckedIntegerBinaryKind::TrappingAdd => {
                            LoweredIntegerBinaryKind::TrappingAdd
                        }
                        CheckedIntegerBinaryKind::TrappingSubtract => {
                            LoweredIntegerBinaryKind::TrappingSubtract
                        }
                        CheckedIntegerBinaryKind::TrappingMultiply => {
                            LoweredIntegerBinaryKind::TrappingMultiply
                        }
                        CheckedIntegerBinaryKind::TrappingDivide => {
                            LoweredIntegerBinaryKind::TrappingDivide
                        }
                        CheckedIntegerBinaryKind::TrappingRemainder => {
                            LoweredIntegerBinaryKind::TrappingRemainder
                        }
                    },
                    scalar_type,
                    left: Box::new(left),
                    right: Box::new(right),
                    formation_decided: *formation_decided,
                })
            }
            CheckedScalarExpression::IntegerBitwiseNot {
                primitive_type,
                operand,
            } => Ok(LoweredDirectExpression::IntegerBitwiseNot {
                scalar_type: terminal_scalar_type(*primitive_type)?,
                operand: Box::new(self.lower_scalar(identities, operand)?),
            }),
            CheckedScalarExpression::IntegerWiden {
                primitive_type,
                operand,
            } => Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type: terminal_scalar_type(*primitive_type)?,
                operand: Box::new(self.lower_scalar(identities, operand)?),
            }),
            CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path,
                index,
                element_path,
                primitive_type,
            } => {
                let bound_decided = selector_bound_is_decided(index, self.proven_indexes);
                if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = *root
                    && let Some(bindings) = self.scalar_bindings
                    && let Some(observation) = bindings.byte_record_observation(
                        identities,
                        self.proven_indexes,
                        symbol,
                        path,
                        &index.expression,
                        element_path,
                        *primitive_type,
                    )?
                {
                    return Ok(observation);
                }

                let lower_selector =
                    |builder: &mut terminal_psi::TerminalIdentityTableBuilder,
                     selector: &CheckedScalarExpression| {
                        self.lower_scalar(builder, selector)
                            .and_then(exact_u64_index)
                    };
                let index = Box::new(lower_selector(identities, &index.expression)?);
                // Bounded bytes read their live length, never capacity. A projected
                // byte field also retains its declaration and containing selectors.
                if element_path.is_empty()
                && *primitive_type == PrimitiveType::U8
                && matches!(
                    path.last(),
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                        _
                    ))
                )
                && let crate::emission::operation_emission::boolean::LoweredByteSequenceOperand {
                    source,
                    path,
                    field: Some(field),
                } = self.byte_sequence_observation(identities, *root, path)?
            {
                return Ok(LoweredDirectExpression::ByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    index,
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                    bound_decided,
                });
            }
                // Geometry needs selector positions, not their expressions or proof
                // identities. Scratch markers associate each declaration walk step
                // with the read's ordered, evaluated selector, never emitted authority.
                let mut selectors = Vec::new();
                let geometry = element_path
                    .iter()
                    .map(|step| match step {
                        checked_trees::CheckedScalarReadProjection::Static(segment) => {
                            segment.clone()
                        }
                        checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                            let depth = selectors.len() as u32;
                            selectors.push(selector);
                            checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                            )
                        }
                    })
                    .collect::<Vec<_>>();
                let (home, structural_type) = self.observation_home(*root)?;
                let observation =
                crate::expression_preparation::bindings::structural_fields::resolve_indexed_element(identities, self.structural_fields, &home, structural_type, path, &geometry)?;
                let crate::expression_preparation::bindings::structural_fields::IndexedElementObservation {
                source, path, element_path: explicit_path, scalar_type, extent,
            } = observation;
                if scalar_type != terminal_scalar_type(*primitive_type)? {
                    return unsupported("indexed read changes its declared scalar carrier");
                }
                use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment as Prepared;
                let mut element_path = Vec::with_capacity(explicit_path.len());
                for step in explicit_path {
                    let dynamic_extent = matches!(
                        element_path.last(),
                        Some(LoweredElementPathSegment::ViewExtent { .. })
                    );
                    element_path.push(match step {
                        Prepared::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                        ) => {
                            let selector =
                                selectors
                                    .get(depth as usize)
                                    .ok_or(LoweringError::Unsupported(
                                        "indexed read lost its selector position",
                                    ))?;
                            LoweredElementPathSegment::RuntimeIndex {
                                selector: exact_u64_index(lower_selector(
                                    identities,
                                    &selector.expression,
                                )?)?,
                                extent: None,
                                bound_decided: selector_bound_is_decided(
                                    selector,
                                    self.proven_indexes,
                                ),
                            }
                        }
                        Prepared::Static(terminal_psi::StructuralPathSegment::FixedIndex(
                            position,
                        )) if dynamic_extent => LoweredElementPathSegment::RuntimeIndex {
                            selector: LoweredDirectExpression::IntegerLiteral {
                                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                                    position,
                                )),
                                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                            },
                            extent: None,
                            bound_decided: false,
                        },
                        Prepared::Static(segment) => LoweredElementPathSegment::Static(segment),
                        Prepared::ViewExtent { bytes } => {
                            LoweredElementPathSegment::ViewExtent { source, bytes }
                        }
                        _ => return unsupported("indexed read has an unsupported projection"),
                    });
                }
                // Count emitted selectors, including literal indexes into live
                // descriptors. Source spelling alone misses those obligations.
                let selectors = path
                    .iter()
                    .filter(|step| {
                        matches!(
                            step,
                            terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                        )
                    })
                    .count()
                    + element_path
                        .iter()
                        .filter(|step| {
                            matches!(step, LoweredElementPathSegment::RuntimeIndex { .. })
                        })
                        .count();
                if selectors
                > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
            {
                return unsupported("indexed read exceeds its selector namespace");
            }
                Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                    qualifications: Default::default(),
                    source,
                    path,
                    index,
                    extent,
                    element_path,
                    scalar_type,
                    bound_decided,
                })
            }
            // The source place is lowered at its declared carrier -- every
            // declared-type check of that read still applies -- and the view
            // replays the carrier relation instead of retagging the read.
            CheckedScalarExpression::RepresentationView {
                primitive_type,
                operand,
            } => {
                let operand = self.lower_scalar(identities, operand)?;
                crate::expression_preparation::representation_view::replay(
                    operand,
                    terminal_scalar_type(*primitive_type)?,
                )
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type,
                operand,
                formation_decided,
                ..
            } => crate::expression_preparation::integer_coercion::coerce_integer_carrier(
                self.lower_scalar(identities, operand)?,
                terminal_scalar_type(*primitive_type)?,
                *formation_decided,
            ),
            CheckedScalarExpression::Boolean(expression) => Ok(LoweredDirectExpression::Boolean {
                expression: Box::new(self.lower_boolean(identities, expression)?),
            }),
        }
    }

    /// Lower one checked Boolean expression against these prepared sources.
    fn lower_boolean(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
    ) -> Result<LoweredBooleanReturnExpression, LoweringError> {
        let mut lower =
            |expression: &CheckedBooleanExpression| self.lower_boolean(identities, expression);
        Ok(match expression {
            CheckedBooleanExpression::Constant(value) => {
                LoweredBooleanReturnExpression::Constant { value: *value }
            }
            CheckedBooleanExpression::Parameter { position } => {
                LoweredBooleanReturnExpression::Parameter {
                    position: *position,
                }
            }
            CheckedBooleanExpression::StorageRead { symbol } => {
                if let Some(bindings) = self.scalar_bindings
                && let Some(observation) =
                    crate::expression_preparation::bindings::primitive_reads::captured_observation(
                        bindings,
                        *symbol,
                        ScalarType::Boolean,
                    )?
            {
                return Ok(LoweredBooleanReturnExpression::ScalarObservation {
                    operand: Box::new(observation),
                });
            }
                let (source, path) =
                    crate::expression_preparation::bindings::primitive_storage_read(
                        self.primitive_storage,
                        self.structural_locals,
                        self.structural_fields.declarations(),
                        *symbol,
                        ScalarType::Boolean,
                    )?;
                LoweredBooleanReturnExpression::PrimitiveRead {
                    qualifications: Default::default(),
                    source: source.into(),
                    path,
                }
            }
            CheckedBooleanExpression::Local { position } => LoweredBooleanReturnExpression::Local {
                position: *position,
            },
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => {
                if !self.structural_fields.is_empty()
                    && matches!(
                        path.first(),
                        Some(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                            _
                        ))
                    )
                {
                    return Ok(
                    match crate::expression_preparation::bindings::structural_fields::resolve_case_payload(
                        identities,
                        self.structural_fields,
                        *parameter_position,
                        path,
                        ScalarType::Boolean,
                    )? {
                        CasePayloadRead::Observation { expression } =>
                            crate::expression_preparation::bindings::structural_fields::boolean_observation(
            expression),
                        CasePayloadRead::Established { position } => {
                            LoweredBooleanReturnExpression::Parameter { position }
                        }
                    },
                );
                }
                if !self.structural_fields.is_empty() {
                    let observation =
                        crate::expression_preparation::bindings::structural_fields::resolve(
                            identities,
                            self.structural_fields,
                            *parameter_position,
                            path,
                            ScalarType::Boolean,
                        )?;
                    return Ok(
                    crate::expression_preparation::bindings::structural_fields::boolean_observation(
                        observation,
                    ),
                );
                }
                let path = path
                    .iter()
                    .map(|segment| match segment {
                        checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                            Ok(identity.spelling())
                        }
                        checked_trees::CheckedStructuralPredicatePathSegment::Case(_)
                        | checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                        | checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => {
                            unsupported("case-payload predicates are contract-only")
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
                    parameter_position: *parameter_position,
                    path,
                }
            }
            CheckedBooleanExpression::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => {
                let operand = self.byte_sequence_observation(identities, *root, path)?;
                LoweredBooleanReturnExpression::ByteSequencePredicate {
                    predicate: *predicate,
                    source: operand.source,
                    path: operand.path,
                    field: operand.field,
                }
            }
            CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
                let operand =
                    |builder: &mut terminal_psi::TerminalIdentityTableBuilder,
                     operand: &checked_trees::CheckedByteSequenceOperand| {
                        self.byte_sequence_observation(builder, operand.root, &operand.path)
                    };
                LoweredBooleanReturnExpression::ByteSequenceEqual {
                    left: operand(identities, left)?,
                    right: operand(identities, right)?,
                }
            }
            CheckedBooleanExpression::Not(operand) => LoweredBooleanReturnExpression::Not {
                operand: Box::new(lower(operand)?),
            },
            CheckedBooleanExpression::Equal { left, right } => {
                LoweredBooleanReturnExpression::Equal {
                    left: Box::new(lower(left)?),
                    right: Box::new(lower(right)?),
                }
            }
            CheckedBooleanExpression::IntegerMathComparison { .. } => {
                return unsupported(
                    "mathematical contract predicate has no executable Boolean value",
                );
            }
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                LoweredBooleanReturnExpression::IntegerComparison {
                    kind: match kind {
                        CheckedIntegerComparisonKind::Equal => LoweredIntegerComparisonKind::Equal,
                        CheckedIntegerComparisonKind::LessThan => {
                            LoweredIntegerComparisonKind::LessThan
                        }
                        CheckedIntegerComparisonKind::LessOrEqual => {
                            LoweredIntegerComparisonKind::LessOrEqual
                        }
                    },
                    left: Box::new(self.lower_scalar(identities, left)?),
                    right: Box::new(self.lower_scalar(identities, right)?),
                }
            }
            CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
                let subject = subject.parameter_field().ok_or(LoweringError::Unsupported(
                    "contract result cannot be used as an executable entry observation",
                ))?;
                let (source, path, case) =
                    crate::expression_preparation::bindings::structural_cases::resolve(
                        identities,
                        self.structural_cases,
                        &subject,
                        case,
                    )?;
                LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case }
            }
            CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::StructuralIdentity { .. }
            | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
            | CheckedBooleanExpression::ErasedParameter { .. } => {
                return unsupported("structural equality is contract-only terminal vocabulary");
            }
            CheckedBooleanExpression::And { left, right } => LoweredBooleanReturnExpression::And {
                left: Box::new(lower(left)?),
                right: Box::new(lower(right)?),
            },
            CheckedBooleanExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
                left: Box::new(lower(left)?),
                right: Box::new(lower(right)?),
            },
        })
    }

    /// Captured actuals and current locals retain the same complete storage home.
    /// An endpoint declaration never replaces the declaration of its physical root.
    fn observation_home(
        &self,
        root: checked_trees::CheckedStorageRoot,
    ) -> Result<(terminal_psi::StructuralArgument, StructuralTypeId), LoweringError> {
        let captured = self
            .scalar_bindings
            .map(|bindings| bindings.captured_storage_home(root))
            .transpose()?
            .flatten();
        let (home, structural_type) = match captured {
            Some(home) => home,
            None => self.structural_fields.observation_home(
                root,
                self.structural_locals,
                self.view_locals,
            )?,
        };
        if !matches!(
            home.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::reading_borrow_pair_pattern!()
        ) {
            return unsupported("storage observation requires readable access");
        }
        Ok((home, structural_type))
    }

    /// Predicates and comparisons observe the same completed descriptor. The
    /// storage root selects its live home; the projection selects the byte carrier.
    /// A local observation is never re-rooted onto an entry parameter.
    fn byte_sequence_observation(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root: checked_trees::CheckedStorageRoot,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Result<
        crate::emission::operation_emission::boolean::LoweredByteSequenceOperand,
        LoweringError,
    > {
        use crate::emission::operation_emission::boolean::LoweredByteSequenceOperand;
        let observation = self
            .sequence_observation(
                identities,
                root,
                &path
                    .iter()
                    .cloned()
                    .map(checked_trees::CheckedScalarReadProjection::Static)
                    .collect::<Vec<_>>(),
                |_, _| unsupported("byte observation has an unevaluated selector"),
            )?
            .ok_or(LoweringError::Unsupported(
                "byte observation has no readable byte extent",
            ))?;
        match observation {
            LoweredDirectExpression::ByteSequenceFieldLength {
                source,
                path,
                field,
                ..
            } => Ok(LoweredByteSequenceOperand {
                source,
                path,
                field: Some(field),
            }),
            LoweredDirectExpression::ByteSequenceLength { source, path, .. } => {
                Ok(LoweredByteSequenceOperand {
                    source,
                    path,
                    field: None,
                })
            }
            _ => unsupported("byte observation requires byte elements"),
        }
    }

    fn sequence_observation(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root: checked_trees::CheckedStorageRoot,
        path: &[checked_trees::CheckedScalarReadProjection],
        mut lower_selector: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            &CheckedScalarExpression,
        ) -> Result<LoweredDirectExpression, LoweringError>,
    ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
        use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment as Prepared;
        use checked_trees::{CheckedRuntimeIndex, CheckedScalarReadProjection as Projection};
        let (home, structural_type) = self.observation_home(root)?;
        if !matches!(
            home.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        ) {
            return unsupported("length observation requires readable storage");
        }
        let mut selectors = Vec::new();
        let geometry = path
            .iter()
            .map(|step| match step {
                Projection::Static(segment) => segment.clone(),
                Projection::RuntimeIndex(selector) => {
                    let depth = selectors.len() as u32;
                    selectors.push(selector);
                    checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                        CheckedRuntimeIndex::AssignmentIndex { depth },
                    )
                }
            })
            .collect::<Vec<_>>();
        let Some((steps, field, carrier)) =
        crate::expression_preparation::bindings::structural_fields::prepare_sequence_observation(
            identities,
            structural_type,
            self.structural_fields.declarations(),
            &geometry,
        )?
    else {
        return Ok(None);
    };
        let source = home.place;
        let mut path = home
            .path
            .into_iter()
            .map(LoweredElementPathSegment::Static)
            .collect::<Vec<_>>();
        for step in steps {
            // Static evidence producers may retain a literal view index as
            // geometry. It is still an executable selection below a live extent,
            // just as in ordinary prepared argument paths; inline array indexes
            // remain static.
            let dynamic_extent = matches!(
                path.last(),
                Some(LoweredElementPathSegment::ViewExtent { .. })
            );
            path.push(match step {
                Prepared::Static(terminal_psi::StructuralPathSegment::FixedIndex(position))
                    if dynamic_extent =>
                {
                    LoweredElementPathSegment::RuntimeIndex {
                        selector: LoweredDirectExpression::IntegerLiteral {
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                                position,
                            )),
                            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                        },
                        extent: None,
                        bound_decided: false,
                    }
                }
                Prepared::Static(segment) => LoweredElementPathSegment::Static(segment),
                Prepared::ViewExtent { bytes } => {
                    LoweredElementPathSegment::ViewExtent { source, bytes }
                }
                Prepared::RuntimeIndex(CheckedRuntimeIndex::AssignmentIndex { depth }) => {
                    let selector =
                        selectors
                            .get(depth as usize)
                            .ok_or(LoweringError::Unsupported(
                                "length observation lost its evaluated selector",
                            ))?;
                    LoweredElementPathSegment::RuntimeIndex {
                        selector: exact_u64_index(lower_selector(
                            identities,
                            &selector.expression,
                        )?)?,
                        extent: None,
                        bound_decided: selector_bound_is_decided(selector, self.proven_indexes),
                    }
                }
                Prepared::RuntimeIndex(index) => LoweredElementPathSegment::RuntimeIndex {
                    selector: exact_u64_index(
                        self.element_path_runtime_selector(identities, &index)?,
                    )?,
                    extent: None,
                    bound_decided: false,
                },
                _ => return unsupported("length observation requires a completed selector"),
            });
        }
        if path
            .iter()
            .filter(|step| matches!(step, LoweredElementPathSegment::RuntimeIndex { .. }))
            .count()
            > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
        {
            return unsupported("length observation exceeds its runtime selector namespace");
        }
        let scalar_type = terminal_scalar_type(PrimitiveType::U64)?;
        Ok(Some(match (carrier, field) {
            (ViewCarrier::Bytes, Some(field)) => LoweredDirectExpression::ByteSequenceFieldLength {
                source,
                path,
                field,
                scalar_type,
            },
            (ViewCarrier::Bytes | ViewCarrier::FixedBytes, None) => {
                LoweredDirectExpression::ByteSequenceLength {
                    source,
                    path,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    scalar_type,
                }
            }
            (ViewCarrier::Elements { .. }, None) => LoweredDirectExpression::ElementViewLength {
                source,
                path,
                access: terminal_psi::StructuralAccess::SharedBorrow,
                scalar_type,
            },
            _ => return unsupported("length observation has an inconsistent sequence endpoint"),
        }))
    }

    /// Terminal element reads take a `u64` position. Any integer carrier may index
    /// once its bounds are proven: a narrower unsigned index widens totally, and a
    /// signed index lands through an exact cast whose Terminal obligation proves it
    /// nonnegative, so it never silently becomes unsigned.
    /// The direct expression one element-path runtime selector evaluates to:
    /// `cells[i].data[j]`'s `j` rides the same structural field bindings a
    /// carrier path's `CheckedRuntimeIndex` spells. Parameter selectors resolve
    /// through the typed authored namespace before emission.
    fn element_path_runtime_selector(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        index: &checked_trees::CheckedRuntimeIndex,
    ) -> Result<LoweredDirectExpression, LoweringError> {
        match index {
            checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => self
                .scalar_bindings
                .ok_or(LoweringError::Unsupported(
                    "storage selector has no established scalar homes",
                ))?
                .runtime_index_storage(*symbol),
            checked_trees::CheckedRuntimeIndex::Parameter { position } => self
                .scalar_bindings
                .ok_or(LoweringError::Unsupported(
                    "element selector has no typed scalar namespace",
                ))?
                .runtime_index_parameter(*position),
            checked_trees::CheckedRuntimeIndex::Local { position } => self
                .scalar_bindings
                .ok_or(LoweringError::Unsupported(
                    "element selector has no typed scalar namespace",
                ))?
                .runtime_index_local(*position),
            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                let observation =
                    crate::expression_preparation::bindings::structural_fields::resolve_typed(
                        identities,
                        self.structural_fields,
                        field.parameter_position,
                        &field.path,
                    )?;
                if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
                {
                    return unsupported("element path runtime selector is not an integer read");
                }
                Ok(observation)
            }
            _ => unsupported("element path runtime selector is not a parameter or field read"),
        }
    }
}

pub(crate) fn exact_u64_index(
    index: LoweredDirectExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    let u64_type = terminal_scalar_type(PrimitiveType::U64)?;
    match index.scalar_type() {
        scalar_type if scalar_type == u64_type => Ok(index),
        ScalarType::Integer(integer)
            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned =>
        {
            Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type: u64_type,
                operand: Box::new(index),
            })
        }
        ScalarType::Integer(_) => Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type: u64_type,
            operand: Box::new(index),
            formation_decided: false,
        }),
        _ => unsupported("indexed reads require an integer index"),
    }
}
