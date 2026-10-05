//! Resolve an observation to its established storage, preserving reference
//! carriers and captured addresses. Declaration geometry chooses the read path;
//! the emitted operation still owes live readable custody to Terminal replay.

use super::{
    LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralLocalBinding,
    StructuralTypeDeclaration, primitive_storage_place, unsupported,
};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(crate) fn primitive_storage_read(
    storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    locals: &[StructuralLocalBinding],
    declarations: &[StructuralTypeDeclaration],
    symbol: symbols::SymbolHandle,
    scalar_type: ScalarType,
) -> Result<(PlaceId, Vec<StructuralPathSegment>), LoweringError> {
    if !symbol.is_valid() {
        return unsupported("primitive storage read has no source identity");
    }
    let mut matching = locals
        .iter()
        .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
    let local = matching.next();
    if matching.next().is_some() || (local.is_some() && storage.iter().any(|row| row.0 == symbol)) {
        return unsupported("primitive storage read has ambiguous established storage");
    }
    let Some(local) = local else {
        return Ok((
            primitive_storage_place(storage, symbol, scalar_type)?,
            Vec::new(),
        ));
    };
    if local.argument.access == StructuralAccess::WriteOnlyBorrow || !local.argument.path.is_empty()
    {
        return unsupported("primitive storage read changes its established carrier custody");
    }
    let declaration = |identity| {
        let mut rows = declarations.iter().filter(|row| row.id == identity);
        let result = rows.next();
        if rows.next().is_some() { None } else { result }
    };
    let root = declaration(local.structural_type).ok_or(LoweringError::Unsupported(
        "primitive storage read has no unique carrier declaration",
    ))?;
    let (observed, path) = match root.shape {
        StructuralTypeShape::Reference { referent, access } => {
            if !matches!(
                access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) {
                return unsupported("primitive storage read crosses an unreadable reference");
            }
            (
                declaration(referent).ok_or(LoweringError::Unsupported(
                    "primitive storage read has no unique referent declaration",
                ))?,
                vec![StructuralPathSegment::Referent],
            )
        }
        _ => (root, Vec::new()),
    };
    if observed.shape != StructuralTypeShape::PrimitiveScalar(scalar_type) {
        return unsupported("primitive storage read changes its declared endpoint type");
    }
    Ok((local.argument.place, path))
}

/// Read a declaration-time captured place under its admitted observation view.
/// This deliberately does not establish a Reference of the viewed type: some
/// representation views, notably byte-to-Boolean, owe a value landing check.
pub(crate) fn captured_observation(
    bindings: &super::ScalarBindings,
    symbol: symbols::SymbolHandle,
    scalar_type: ScalarType,
) -> Result<
    Option<crate::emission::operation_emission::expressions::LoweredDirectExpression>,
    LoweringError,
> {
    use super::place_observations::ObservationKind;
    use crate::emission::operation_emission::expressions::LoweredDirectExpression as Direct;
    let mut matching = bindings
        .place_observations
        .iter()
        .filter(|loan| loan.symbol == symbol);
    let Some(loan) = matching.next() else {
        return Ok(None);
    };
    if !symbol.is_valid() || matching.next().is_some() {
        return unsupported("primitive observation has ambiguous captured storage");
    }
    let capture = loan.capture.as_ref().map_err(Clone::clone)?;
    let ObservationKind::Primitive(observation) = &capture.observation else {
        return Ok(None);
    };
    if observation.viewed_type != scalar_type
        || bindings
            .structural_locals
            .iter()
            .any(|row| row.root == facts::PlaceRoot::Symbol(symbol))
        || bindings.primitive_storage.iter().any(|row| row.0 == symbol)
    {
        return unsupported("primitive observation changes its captured view or carrier identity");
    }
    let source = bindings.observation_backing(capture.source)?;
    let read = if capture.runtime_index.is_some() {
        let index = Box::new(bindings.observation_coordinate(symbol)?);
        if observation.view_container {
            return unsupported("captured primitive observation requires fixed-array storage");
        }
        Direct::IndexedPrimitiveRead {
            source,
            path: capture.carrier_path.clone(),
            index,
            inner_indexes: Vec::new(),
            element_path: observation.element_path.iter().cloned().map(
                crate::emission::operation_emission::expressions::LoweredElementPathSegment::Static,
            ).collect(),
            scalar_type: observation.scalar_type,
            bound_decided: false,
        }
    } else if let Some(field) = observation.field {
        match observation.scalar_type {
            ScalarType::Boolean => Direct::Boolean {
                expression: Box::new(crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression::StructuralField {
                    source: source.into(),
                    path: observation.path.clone(),
                    field,
                }),
            },
            ScalarType::Integer(_) => Direct::StructuralField {
                source: source.into(),
                path: observation.path.clone(),
                field,
                scalar_type: observation.scalar_type,
            },
            ScalarType::IeeeFloat(_) => Direct::PrimitiveRead {
                source: source.into(),
                path: capture.carrier_path.clone(),
                scalar_type: observation.scalar_type,
            },
        }
    } else if observation.scalar_type != observation.viewed_type
        && matches!(
            capture.carrier_path.last(),
            Some(StructuralPathSegment::FixedIndex(_))
        )
    {
        // Byte-region views use the same region operation for literal and
        // captured dynamic coordinates, retaining per-byte bounds checks.
        let Some((StructuralPathSegment::FixedIndex(index), path)) =
            capture.carrier_path.split_last()
        else {
            unreachable!()
        };
        Direct::IndexedPrimitiveRead {
            source,
            path: path.to_vec(),
            index: Box::new(Direct::IntegerLiteral {
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                scalar_type: super::terminal_scalar_type(typed_trees::types::PrimitiveType::U64)?,
            }),
            inner_indexes: Vec::new(),
            element_path: Vec::new(),
            scalar_type: observation.scalar_type,
            bound_decided: false,
        }
    } else {
        Direct::PrimitiveRead {
            source: source.into(),
            path: capture.carrier_path.clone(),
            scalar_type: observation.scalar_type,
        }
    };
    Ok(Some(
        if observation.scalar_type == observation.viewed_type {
            read
        } else {
            crate::expression_preparation::representation_view::replay(
                read,
                observation.viewed_type,
            )?
        },
    ))
}
