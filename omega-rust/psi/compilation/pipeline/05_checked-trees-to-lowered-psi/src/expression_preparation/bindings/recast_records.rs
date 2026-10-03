//! Captured byte-backed record loans and scalar observations of their layout.
//! Preparing a loan retains normalized geometry, not a list of source reads.
//! Each use resolves current backing custody; byte reads retain independent
//! bounds and source replay remains with its existing owner.
use super::super::{
    CheckedTrees, LoweringError, PrimitiveType, ScalarType, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct ByteOverlayField {
    pub offset: u64,
    pub scalar_type: ScalarType,
    pub stored_type: ScalarType,
    pub indices: Vec<ByteOverlayIndex>,
}

#[derive(Clone)]
pub(crate) struct ByteOverlayIndex {
    pub selector: checked_trees::CheckedRuntimeIndex,
    pub stride: u64,
    pub extent: u64,
}

pub(crate) struct ByteOverlaySource {
    pub(crate) referee: typed_trees::types::TypeReferenceHandle,
    pub(crate) remaining_bytes: Option<u64>,
}

impl ByteOverlaySource {
    pub(crate) fn field(
        &self,
        checked: &CheckedTrees,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        field: symbols::SymbolHandle,
        scalar_type: ScalarType,
    ) -> Result<ByteOverlayField, LoweringError> {
        let projection =
            validation::recast_scalar_projection(&checked.typed, self.referee, path, field).ok_or(
                LoweringError::Unsupported(
                    "record byte overlay lost its checked projection geometry",
                ),
            )?;
        self.projection(projection, scalar_type)
    }

    pub(crate) fn scalar_path(
        &self,
        checked: &CheckedTrees,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ByteOverlayField, LoweringError> {
        let geometry = validation::recast_scalar_path_geometry(&checked.typed, self.referee, path)
            .ok_or(LoweringError::Unsupported(
                "record byte overlay lost its checked projection geometry",
            ))?;
        self.geometry(geometry, path, scalar_type)
    }

    pub(crate) fn geometry(
        &self,
        geometry: validation::RecastScalarGeometry,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ByteOverlayField, LoweringError> {
        let mut field = self.projection(geometry.projection, scalar_type)?;
        for index in geometry.indices {
            let Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector)) =
                path.get(index.path_index)
            else {
                return unsupported("record byte overlay substituted its runtime coordinate");
            };
            field.indices.push(ByteOverlayIndex {
                selector: selector.clone(),
                stride: u64::try_from(index.stride)
                    .map_err(|_| LoweringError::Unsupported("record array stride exceeds u64"))?,
                extent: u64::try_from(index.extent)
                    .map_err(|_| LoweringError::Unsupported("record array extent exceeds u64"))?,
            });
        }
        Ok(field)
    }

    pub(crate) fn projection(
        &self,
        projection: validation::RecastScalarProjection,
        scalar_type: ScalarType,
    ) -> Result<ByteOverlayField, LoweringError> {
        if terminal_scalar_type(projection.primitive)? != scalar_type
            || self.remaining_bytes.is_some_and(|remaining| {
                u64::try_from(projection.footprint).map_or(true, |footprint| footprint > remaining)
            })
        {
            return unsupported("record byte overlay changed its carrier or footprint");
        }
        Ok(ByteOverlayField {
            offset: u64::try_from(projection.offset).map_err(|_| {
                LoweringError::Unsupported("record byte overlay offset exceeds u64")
            })?,
            scalar_type,
            stored_type: projection
                .stored_integer
                .map(ScalarType::Integer)
                .unwrap_or(scalar_type),
            indices: Vec::new(),
        })
    }
}

/// The established place a shared record loan resolves to at observation: a
/// structural parameter (by dense roster index) or an owned structural local
/// (by symbol, resolved through the ordinary argument join).
#[derive(Clone, Copy)]
pub(crate) enum LoanedSource {
    Parameter(u32),
    Local(symbols::SymbolHandle),
}

/// A recast loan retains storage and layout, not a table of source reads.
/// Shared by the state's scalar namespaces; each observation resolves the live
/// backing anew. Geometry cannot grant access or replace source replay.
pub(crate) struct ByteRecordLoan {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) capture: Result<ByteRecordCapture, LoweringError>,
}

pub(crate) struct ByteRecordCapture {
    pub(crate) source: LoanedSource,
    pub(crate) carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
    pub(crate) selector_primitive: Option<PrimitiveType>,
    pub(crate) overlay: ByteOverlaySource,
    pub(crate) layout: validation::RecastScalarLayout,
}

pub(crate) fn byte_record_observation(
    loans: &[ByteRecordLoan],
    bindings: &super::ScalarBindings,
    symbol: symbols::SymbolHandle,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    index: &checked_trees::CheckedScalarExpression,
    inner_indexes: &[checked_trees::CheckedScalarExpression],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    primitive: PrimitiveType,
) -> Result<Option<LoweredDirectExpression>, LoweringError> {
    use checked_trees::{CheckedScalarExpression, CheckedUnitStructuralPathSegment as Projection};
    let mut matching = loans.iter().filter(|loan| loan.symbol == symbol);
    let Some(loan) = matching.next() else {
        return Ok(None);
    };
    if !symbol.is_valid() || matching.next().is_some() {
        return unsupported("record byte observation has ambiguous loan identity");
    }
    // Preparing a declaration must not reject an unused unsupported loan.
    // Its observation, if any, owns the retained capture failure.
    let loan = loan.capture.as_ref().map_err(Clone::clone)?;
    // Geometry consumes only path positions. The scratch assignment markers
    // below stand for these ordered expressions, never emitted source evidence.
    let mut selectors = Vec::new();
    let mut leaf_path = Vec::new();
    append_observation_path(bindings, path, &mut leaf_path, &mut selectors)?;
    for selector in std::iter::once(index).chain(inner_indexes) {
        if let CheckedScalarExpression::IntegerLiteral { literal } = selector {
            leaf_path.push(Projection::FixedIndex(literal.value_u64().ok_or(
                LoweringError::Unsupported("record byte observation has an invalid array index"),
            )?));
        } else {
            append_coordinate(
                bindings.expression(selector)?,
                &mut leaf_path,
                &mut selectors,
            )?;
        }
    }
    append_observation_path(bindings, element_path, &mut leaf_path, &mut selectors)?;
    let scalar_type = terminal_scalar_type(primitive)?;
    let geometry =
        loan.layout
            .scalar_path_geometry(&leaf_path)
            .ok_or(LoweringError::Unsupported(
                "record byte observation lost its normalized scalar leaf",
            ))?;
    let projection = loan.overlay.geometry(geometry, &leaf_path, scalar_type)?;
    if projection.indices.len() != selectors.len() {
        return unsupported("record byte observation changed its runtime coordinate roster");
    }
    let source = bindings.byte_record_backing(loan.source)?;
    let (prefix, coordinate) = match &loan.runtime_index {
        Some(_) => (
            loan.carrier_path.as_slice(),
            bindings.byte_record_coordinate(symbol)?,
        ),
        None => {
            let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) =
                loan.carrier_path.split_last()
            else {
                return unsupported("record byte observation lost its captured byte coordinate");
            };
            (
                prefix,
                LoweredDirectExpression::IntegerLiteral {
                    value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                    scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                },
            )
        }
    };
    let mut coordinate = coordinate;
    for (ordinal, (index, selector)) in projection.indices.iter().zip(selectors).enumerate() {
        if index.selector
            != (checked_trees::CheckedRuntimeIndex::AssignmentIndex {
                depth: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("record byte coordinate ordinal overflow")
                })?,
            })
        {
            return unsupported("record byte observation reordered its runtime coordinates");
        }
        let selector = bounded_coordinate(selector, index.extent)?;
        let scaled = integer_coordinate_binary(
            crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactMultiply,
            selector,
            coordinate_literal(index.stride),
        );
        coordinate = integer_coordinate_binary(
            crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
            coordinate,
            scaled,
        );
    }
    byte_overlay_read(
        source,
        prefix,
        coordinate,
        projection.offset,
        projection.stored_type,
        scalar_type,
    )
    .map(Some)
}

/// Observe just the selected field's bytes at its exact layout offset. The
/// ordinary region re-view owns the byte assembly and each runtime-index proof;
/// the source recast's footprint judgment cannot discharge those obligations.
pub(crate) fn byte_overlay_read(
    source: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    index: LoweredDirectExpression,
    offset: u64,
    stored_type: ScalarType,
    scalar_type: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let index_type = index.scalar_type();
    let index = if offset == 0 {
        index
    } else {
        LoweredDirectExpression::IntegerBinary {
            kind: crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
            scalar_type: index_type,
            left: Box::new(index),
            right: Box::new(LoweredDirectExpression::IntegerLiteral {
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(offset)),
                scalar_type: index_type,
            }),
        }
    };
    let byte_type = terminal_scalar_type(PrimitiveType::U8)?;
    let first_byte = LoweredDirectExpression::IndexedPrimitiveRead {
        source,
        path: path.to_vec(),
        index: Box::new(index),
        inner_indexes: Vec::new(),
        element_path: Vec::new(),
        scalar_type: byte_type,
    };
    let stored = if stored_type == byte_type {
        first_byte
    } else {
        crate::expression_preparation::representation_view::replay(first_byte, stored_type)?
    };
    if stored_type == scalar_type {
        return Ok(stored);
    }
    match (stored_type, scalar_type) {
        (ScalarType::Integer(source_type), ScalarType::Integer(target_type))
            if source_type.can_widen_to(target_type) =>
        {
            Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type,
                operand: Box::new(stored),
            })
        }
        _ => unsupported("record byte overlay cannot decode its stored integer carrier"),
    }
}

/// Keep the exact selected coordinate while requiring its declared dimension's
/// bound. The inner ExactSubtract proves index <= extent-1; the outer one
/// returns that same index. The operand occurs once and no assumed fact is made.
pub(crate) fn bounded_coordinate(
    selector: LoweredDirectExpression,
    extent: u64,
) -> Result<LoweredDirectExpression, LoweringError> {
    use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactSubtract;
    let maximum = extent.checked_sub(1).ok_or(LoweringError::Unsupported(
        "record byte observation selects an empty array",
    ))?;
    let selector = crate::expression_preparation::prepare_expression::exact_u64_index(selector)?;
    Ok(integer_coordinate_binary(
        ExactSubtract,
        coordinate_literal(maximum),
        integer_coordinate_binary(ExactSubtract, coordinate_literal(maximum), selector),
    ))
}

fn coordinate_literal(value: u64) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerLiteral {
        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(value)),
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is an admitted scalar carrier"),
        ),
    }
}

fn integer_coordinate_binary(
    kind: crate::emission::operation_emission::integer::LoweredIntegerBinaryKind,
    left: LoweredDirectExpression,
    right: LoweredDirectExpression,
) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerBinary {
        kind,
        scalar_type: left.scalar_type(),
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn append_coordinate(
    selector: LoweredDirectExpression,
    path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    selectors: &mut Vec<LoweredDirectExpression>,
) -> Result<(), LoweringError> {
    let depth = u32::try_from(selectors.len()).map_err(|_| {
        LoweringError::Unsupported("record byte observation has too many coordinates")
    })?;
    path.push(
        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
            checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
        ),
    );
    selectors.push(selector);
    Ok(())
}

fn append_observation_path(
    bindings: &super::ScalarBindings,
    supplied: &[checked_trees::CheckedStructuralPredicatePathSegment],
    path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    selectors: &mut Vec<LoweredDirectExpression>,
) -> Result<(), LoweringError> {
    use checked_trees::{
        CheckedRuntimeIndex, CheckedStructuralPredicatePathSegment as Predicate,
        CheckedUnitStructuralPathSegment as Projection,
    };
    for segment in supplied {
        match segment {
            Predicate::Field(field) => path.push(Projection::Field(field.clone())),
            Predicate::FixedIndex(index) => path.push(Projection::FixedIndex(*index)),
            Predicate::RuntimeIndex(index) => {
                let selector = match index {
                    CheckedRuntimeIndex::Parameter { position } => {
                        bindings.runtime_index_parameter(*position)?
                    }
                    CheckedRuntimeIndex::Local { position } => {
                        bindings.runtime_index_local(*position)?
                    }
                    CheckedRuntimeIndex::StructuralField(field) => {
                        bindings.runtime_index_field(field)?
                    }
                    CheckedRuntimeIndex::AssignmentIndex { .. } => {
                        return unsupported(
                            "record byte observation has an assignment-only coordinate",
                        );
                    }
                };
                append_coordinate(selector, path, selectors)?;
            }
            Predicate::Case(_) => {
                return unsupported("record byte observation cannot reinterpret a case payload");
            }
        }
    }
    Ok(())
}
