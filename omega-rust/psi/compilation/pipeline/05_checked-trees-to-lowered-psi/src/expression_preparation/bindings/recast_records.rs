//! Captured byte-backed record loans and scalar observations of their layout.
//! Preparing a loan retains normalized geometry, not a list of source reads.
//! Each use resolves current backing custody; byte reads retain independent
//! bounds and source replay remains with its existing owner.
use super::super::{
    CheckedTrees, LoweringError, PrimitiveType, ScalarType, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;

#[derive(Clone)]
pub(crate) struct ByteOverlayField {
    pub offset: u64,
    pub scalar_type: ScalarType,
    pub stored_type: ScalarType,
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
        let projection =
            validation::recast_scalar_path_projection(&checked.typed, self.referee, path).ok_or(
                LoweringError::Unsupported(
                    "record byte overlay lost its checked projection geometry",
                ),
            )?;
        self.projection(projection, scalar_type)
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
    use checked_trees::{
        CheckedScalarExpression, CheckedStructuralPredicatePathSegment as Predicate,
        CheckedUnitStructuralPathSegment as Projection,
    };
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
    let projection_segment = |segment: &Predicate| match segment {
        Predicate::Field(field) => Ok(Projection::Field(field.clone())),
        Predicate::FixedIndex(index) => Ok(Projection::FixedIndex(*index)),
        _ => unsupported("record byte observation needs a statically bounded layout path"),
    };
    let mut leaf_path = path
        .iter()
        .map(projection_segment)
        .collect::<Result<Vec<_>, _>>()?;
    for selector in std::iter::once(index).chain(inner_indexes) {
        let CheckedScalarExpression::IntegerLiteral { literal } = selector else {
            return unsupported("record byte observation needs retained runtime array bounds");
        };
        leaf_path.push(Projection::FixedIndex(literal.value_u64().ok_or(
            LoweringError::Unsupported("record byte observation has an invalid array index"),
        )?));
    }
    leaf_path.extend(
        element_path
            .iter()
            .map(projection_segment)
            .collect::<Result<Vec<_>, _>>()?,
    );
    let scalar_type = terminal_scalar_type(primitive)?;
    let projection =
        loan.layout
            .scalar_path_projection(&leaf_path)
            .ok_or(LoweringError::Unsupported(
                "record byte observation lost its normalized scalar leaf",
            ))?;
    let projection = loan.overlay.projection(projection, scalar_type)?;
    let source = bindings.byte_record_backing(loan.source)?;
    let (prefix, coordinate) = match &loan.runtime_index {
        Some(selector) => {
            let primitive_type = loan.selector_primitive.ok_or(LoweringError::Unsupported(
                "record byte observation lost its captured selector type",
            ))?;
            let selector = match selector {
                checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                    CheckedScalarExpression::Parameter {
                        position: *position as usize,
                        primitive_type,
                    }
                }
                checked_trees::CheckedRuntimeIndex::Local { position } => {
                    CheckedScalarExpression::Local {
                        position: *position as usize,
                        primitive_type,
                    }
                }
                _ => {
                    return unsupported(
                        "record byte observation cannot reload a mutable captured selector",
                    );
                }
            };
            (
                loan.carrier_path.as_slice(),
                crate::expression_preparation::prepare_expression::exact_u64_index(
                    bindings.expression(&selector)?,
                )?,
            )
        }
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
