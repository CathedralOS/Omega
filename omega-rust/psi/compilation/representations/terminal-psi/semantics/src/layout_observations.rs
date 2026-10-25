//! Receiving semantics for a portable layout observation. The canonical
//! carrier supplies identity and validated record storage; the environment
//! supplies physical address geometry. No source, provider recognizer, host
//! object size, or activation-frame bookkeeping participates.

use semantic_vocabulary::{PhysicalDataLayout, ScalarType, StructuralTypeId};
use terminal_psi::{LayoutObservationCarrier, LayoutObservationKind, Operation, OperationKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutObservationError {
    InvalidResult,
    InvalidPhysicalParameters,
    MissingPhysicalParameters,
    MissingCarrier(StructuralTypeId),
    RecursiveCarrier(StructuralTypeId),
    NestingLimit(StructuralTypeId),
    UnavailableCarrier(StructuralTypeId),
    UnsupportedCarrier(StructuralTypeId),
    UnsupportedScalar(ScalarType),
    ExtentOverflow,
}

impl std::fmt::Display for LayoutObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidResult => write!(formatter, "layout observation must produce u64"),
            Self::InvalidPhysicalParameters => {
                write!(formatter, "invalid physical data-layout parameters")
            }
            Self::MissingPhysicalParameters => write!(
                formatter,
                "layout observation requires explicit physical address geometry"
            ),
            Self::MissingCarrier(carrier) => write!(
                formatter,
                "layout observation carrier {carrier:?} is absent"
            ),
            Self::RecursiveCarrier(carrier) => write!(
                formatter,
                "layout observation carrier {carrier:?} has circular storage"
            ),
            Self::NestingLimit(carrier) => write!(
                formatter,
                "layout observation carrier {carrier:?} exceeds the storage nesting limit"
            ),
            Self::UnavailableCarrier(carrier) => write!(
                formatter,
                "layout observation carrier {carrier:?} has no admitted layout in this context"
            ),
            Self::UnsupportedCarrier(carrier) => write!(
                formatter,
                "layout observation carrier {carrier:?} has no supported complete storage geometry"
            ),
            Self::UnsupportedScalar(scalar) => write!(
                formatter,
                "layout observation scalar {scalar:?} has no supported storage geometry"
            ),
            Self::ExtentOverflow => write!(formatter, "layout observation extent exceeds u64"),
        }
    }
}
impl std::error::Error for LayoutObservationError {}

/// Formation is target-neutral, including for inactive catalog bodies.
/// Physical completeness is a separate receiving obligation.
pub fn validate_layout_observation(
    operation: &Operation,
    carrier_exists: impl Fn(StructuralTypeId) -> bool,
) -> Result<(), LayoutObservationError> {
    let OperationKind::LayoutObservation { carrier, .. } = operation.kind else {
        return Err(LayoutObservationError::InvalidResult);
    };
    if operation.result.scalar().is_none_or(|result| {
        !result.scalar_type.as_integer().is_some_and(|integer| {
            !integer.is_address()
                && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                && integer.bits() == 64
        })
    }) {
        return Err(LayoutObservationError::InvalidResult);
    }
    if let LayoutObservationCarrier::Structural(carrier) = carrier
        && !carrier_exists(carrier)
    {
        return Err(LayoutObservationError::MissingCarrier(carrier));
    }
    Ok(())
}

pub fn observe_layout<'types>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    physical: Option<PhysicalDataLayout>,
    kind: LayoutObservationKind,
    carrier: LayoutObservationCarrier,
) -> Result<u64, LayoutObservationError> {
    if physical.is_some_and(|physical| !physical.is_valid()) {
        return Err(LayoutObservationError::InvalidPhysicalParameters);
    }
    let (size, alignment) = match carrier {
        LayoutObservationCarrier::Unit => (0, 1),
        LayoutObservationCarrier::Scalar(scalar) => {
            let geometry =
                crate::reference_storage_geometry::physical_scalar_geometry(scalar, physical)?;
            (geometry.size, geometry.alignment)
        }
        LayoutObservationCarrier::Structural(carrier) => {
            let geometry = crate::reference_storage_geometry::physical_geometry(
                types,
                carrier,
                physical,
                &mut Vec::new(),
            )?;
            (geometry.size, geometry.alignment)
        }
    };
    Ok(match kind {
        LayoutObservationKind::Size => size,
        LayoutObservationKind::Alignment => alignment,
    })
}
