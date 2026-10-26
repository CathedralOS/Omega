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
    IncompatibleFoldedLayout {
        owner: terminal_psi::FoldedLayoutOwner,
        target: String,
    },
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
            Self::IncompatibleFoldedLayout { owner, target } => write!(
                formatter,
                "folded layout requirement for {owner:?} has no matching joint assignment under `{target}`"
            ),
        }
    }
}

/// Admit every independent compatibility row owned by this receiving scope.
/// One alternative must agree in full: individually observed coordinates do
/// not justify a combination the producing judgment never consumed.
pub fn admit_folded_layout_requirements<'types>(
    requirements: &[terminal_psi::FoldedLayoutRequirement],
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    physical: Option<PhysicalDataLayout>,
    target: &str,
    owner: terminal_psi::FoldedLayoutOwner,
) -> Result<(), LayoutObservationError> {
    for requirement in requirements.iter().filter(|row| row.owner == owner) {
        let mut matched = false;
        let mut unavailable = None;
        for alternative in requirement
            .alternatives
            .iter()
            .filter(|row| row.target == target)
        {
            let mut agrees = true;
            for observation in &alternative.observations {
                match observe_layout(
                    types.clone(),
                    physical,
                    observation.kind,
                    observation.carrier,
                ) {
                    Ok(value) if value == observation.value => {}
                    Ok(_) => {
                        agrees = false;
                        break;
                    }
                    Err(reason) => {
                        unavailable.get_or_insert(reason);
                        agrees = false;
                        break;
                    }
                }
            }
            if agrees {
                matched = true;
                break;
            }
        }
        if !matched {
            return Err(unavailable.unwrap_or_else(|| {
                LayoutObservationError::IncompatibleFoldedLayout {
                    owner,
                    target: target.to_owned(),
                }
            }));
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{
        FoldedLayoutAlternative, FoldedLayoutObservation, FoldedLayoutOwner,
        FoldedLayoutRequirement,
    };

    #[test]
    fn folded_observations_require_one_joint_qualified_assignment_per_owner() {
        let owner = FoldedLayoutOwner::Machine(semantic_vocabulary::MachineId::new(1).unwrap());
        let carrier = LayoutObservationCarrier::Scalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::address(64).unwrap(),
        ));
        let alternative = |value| FoldedLayoutAlternative {
            target: "macos_arm64".to_owned(),
            observations: vec![
                FoldedLayoutObservation {
                    kind: LayoutObservationKind::Size,
                    carrier,
                    value,
                },
                FoldedLayoutObservation {
                    kind: LayoutObservationKind::Alignment,
                    carrier,
                    value,
                },
            ],
        };
        let mut requirements = vec![FoldedLayoutRequirement {
            owner,
            alternatives: vec![alternative(4), alternative(8)],
        }];
        let receive = |requirements: &[FoldedLayoutRequirement], size, alignment, target| {
            admit_folded_layout_requirements(
                requirements,
                std::iter::empty(),
                Some(PhysicalDataLayout {
                    address_byte_size: size,
                    address_byte_alignment: alignment,
                }),
                target,
                owner,
            )
        };
        assert_eq!(receive(&requirements, 4, 4, "macos_arm64"), Ok(()));
        assert_eq!(receive(&requirements, 8, 8, "macos_arm64"), Ok(()));
        // Each coordinate matches an alternative, but their combination does not.
        assert!(matches!(
            receive(&requirements, 8, 4, "macos_arm64"),
            Err(LayoutObservationError::IncompatibleFoldedLayout { .. })
        ));
        // Numeric equality does not supply a different evaluation context.
        assert!(receive(&requirements, 8, 8, "linux_arm64").is_err());
        requirements.push(FoldedLayoutRequirement {
            owner,
            alternatives: vec![alternative(4)],
        });
        // Another proof owner's row remains independently owed.
        assert!(receive(&requirements, 8, 8, "macos_arm64").is_err());
        assert_eq!(receive(&requirements, 4, 4, "macos_arm64"), Ok(()));
    }
}
