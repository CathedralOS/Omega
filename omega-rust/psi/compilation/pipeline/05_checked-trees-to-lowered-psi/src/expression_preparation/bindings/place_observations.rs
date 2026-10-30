//! Captured place observations and byte-backed record layout observations.
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
    pub stored_type: ScalarType,
    pub indices: Vec<ByteOverlayIndex>,
}

#[derive(Clone)]
pub(crate) struct ByteOverlayIndex {
    pub selector: checked_trees::CheckedRuntimeIndex,
    pub stride: u64,
    pub extent: u64,
}

#[derive(Clone)]
pub(crate) struct ByteOverlaySource {
    pub(crate) referee: typed_trees::types::TypeReferenceHandle,
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
        // The geometry carries one entry per runtime-index segment of `path`,
        // in path order.
        let selectors = path.iter().filter_map(|segment| match segment {
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
                Some(selector)
            }
            _ => None,
        });
        for (index, selector) in geometry.indices.into_iter().zip(selectors) {
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

    /// Checking's recast judgment bounds the record's footprint by the bytes
    /// remaining at the overlay's offset, and Terminal re-proves each byte
    /// read; the projection supplies only the field's geometry.
    pub(crate) fn projection(
        &self,
        projection: validation::RecastScalarProjection,
        scalar_type: ScalarType,
    ) -> Result<ByteOverlayField, LoweringError> {
        Ok(ByteOverlayField {
            offset: u64::try_from(projection.offset).map_err(|_| {
                LoweringError::Unsupported("record byte overlay offset exceeds u64")
            })?,
            stored_type: projection
                .stored_integer
                .map(ScalarType::Integer)
                .unwrap_or(scalar_type),
            indices: Vec::new(),
        })
    }
}

/// The established place a loan resolves to at observation: a structural
/// parameter (by dense roster index) or a primitive/structural local (by symbol).
/// A view never substitutes its own type for the original backing's identity.
#[derive(Clone, Copy)]
pub(crate) enum LoanedSource {
    Parameter(u32),
    Local(symbols::SymbolHandle),
}

/// A place loan retains storage and its observation view, not a table of reads.
/// Shared by the state's scalar namespaces; each observation resolves the live
/// backing anew. Geometry cannot grant access or replace source replay.
#[derive(Clone)]
pub(crate) struct PlaceObservation {
    pub(crate) statement_index: u32,
    pub(crate) symbol: symbols::SymbolHandle,
    /// The checked loan origin remains the sole identity/path authority.
    pub(crate) loan: checked_trees::CheckedPlaceLoanPlan,
    /// Private selector homes, in the checked prefix's runtime-index order.
    pub(crate) selectors: Vec<semantic_vocabulary::PlaceId>,
    pub(crate) capture: Result<PlaceObservationCapture, LoweringError>,
}

#[derive(Clone)]
pub(crate) struct PlaceObservationCapture {
    pub(crate) coordinate: Option<semantic_vocabulary::PlaceId>,
    pub(crate) source: LoanedSource,
    pub(crate) carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
    pub(crate) observation: ObservationKind,
}

#[derive(Clone)]
pub(crate) enum ObservationKind {
    /// Ordinary declared structural storage; only its captured coordinates
    /// need private homes. Field observations retain their checked paths.
    Structural,
    ByteRecord {
        overlay: ByteOverlaySource,
        layout: validation::RecastScalarLayout,
    },
    Primitive(PrimitiveObservation),
}

/// The declaration-owned scalar leaf and its observation carrier are distinct:
/// a conditional Boolean view never changes the storage's declared type.
#[derive(Clone)]
pub(crate) struct PrimitiveObservation {
    pub(crate) scalar_type: ScalarType,
    pub(crate) viewed_type: ScalarType,
    pub(crate) field: Option<semantic_vocabulary::StructuralFieldId>,
    pub(crate) path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    pub(crate) element_path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) view_container: bool,
}

pub(crate) fn byte_record_observation(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    loans: &[PlaceObservation],
    bindings: &super::ScalarBindings,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
    symbol: symbols::SymbolHandle,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    index: &checked_trees::CheckedScalarExpression,
    element_path: &[checked_trees::CheckedScalarReadProjection],
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
    let ObservationKind::ByteRecord { overlay, layout } = &loan.observation else {
        return Ok(None);
    };
    // Geometry consumes only path positions. The scratch assignment markers
    // below stand for these ordered expressions, never emitted source evidence.
    let mut selectors = Vec::new();
    let mut leaf_path = Vec::new();
    append_observation_path(identities, bindings, path, &mut leaf_path, &mut selectors)?;
    if let CheckedScalarExpression::IntegerLiteral { literal } = index {
        leaf_path.push(Projection::FixedIndex(literal.value_u64().ok_or(
            LoweringError::Unsupported("record byte observation has an invalid array index"),
        )?));
    } else {
        append_coordinate(
            bindings.expression(identities, proven_indexes, index)?,
            &mut leaf_path,
            &mut selectors,
        )?;
    }
    for step in element_path {
        match step {
            checked_trees::CheckedScalarReadProjection::Static(segment) => append_observation_path(
                identities,
                bindings,
                std::slice::from_ref(segment),
                &mut leaf_path,
                &mut selectors,
            )?,
            checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                append_coordinate(
                    bindings.expression(identities, proven_indexes, &selector.expression)?,
                    &mut leaf_path,
                    &mut selectors,
                )?
            }
        }
    }
    let scalar_type = terminal_scalar_type(primitive)?;
    let geometry = layout
        .scalar_path_geometry(&leaf_path)
        .ok_or(LoweringError::Unsupported(
            "record byte observation lost its normalized scalar leaf",
        ))?;
    let projection = overlay.geometry(geometry, &leaf_path, scalar_type)?;
    let source = bindings.observation_backing(loan.source)?;
    let (prefix, coordinate) = match &loan.runtime_index {
        Some(_) => (
            loan.carrier_path.as_slice(),
            bindings.observation_coordinate(symbol)?,
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
            // The bounded coordinate lies in `[0, extent - 1]`, so the
            // product fits whenever the dimension's own byte size
            // `extent * stride` does — a fact the checked layout supplies.
            index.extent.checked_mul(index.stride).is_some(),
            selector,
            coordinate_literal(index.stride),
        );
        // The recast admission proved `coordinate + footprint <= region` for
        // this captured (or literal) coordinate, and `scaled` is a byte offset
        // inside that footprint: `coordinate + scaled` stays below the
        // region's end, so the add is representable already.
        coordinate = integer_coordinate_binary(
            crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
            true,
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
            formation_decided: false,
        }
    };
    let byte_type = terminal_scalar_type(PrimitiveType::U8)?;
    let first_byte = LoweredDirectExpression::IndexedPrimitiveRead {
        qualifications: Default::default(),
        source,
        path: path.to_vec(),
        index: Box::new(index),
        extent: None,
        element_path: Vec::new(),
        scalar_type: byte_type,
        bound_decided: false,
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
    // The inner subtract is the enforcement point: it faults unless
    // `selector <= maximum`. Once it stands, its result lies in
    // `[0, maximum]`, so the outer subtract's `maximum - inner` difference
    // cannot underflow — its representability is already established.
    Ok(integer_coordinate_binary(
        ExactSubtract,
        true,
        coordinate_literal(maximum),
        integer_coordinate_binary(ExactSubtract, false, coordinate_literal(maximum), selector),
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
    formation_decided: bool,
    left: LoweredDirectExpression,
    right: LoweredDirectExpression,
) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerBinary {
        kind,
        scalar_type: left.scalar_type(),
        left: Box::new(left),
        right: Box::new(right),
        formation_decided,
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
            Predicate::Referent => path.push(Projection::Referent),
            Predicate::Field(field) => path.push(Projection::Field(field.clone())),
            Predicate::FixedIndex(index) => path.push(Projection::FixedIndex(*index)),
            Predicate::RuntimeIndex(index) => {
                let selector = match index {
                    CheckedRuntimeIndex::StorageRead { symbol } => {
                        bindings.runtime_index_storage(*symbol)?
                    }
                    CheckedRuntimeIndex::Parameter { position } => {
                        bindings.runtime_index_parameter(*position)?
                    }
                    CheckedRuntimeIndex::Local { position } => {
                        bindings.runtime_index_local(*position)?
                    }
                    CheckedRuntimeIndex::StructuralField(field) => {
                        bindings.runtime_index_field(identities, field)?
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
