//! Separate a carrier's current declaration from the formations it transports.
//! A block parameter is a real receiving occurrence, not a made-up operation
//! result. All incoming formations survive control-flow selection, even when
//! their descriptor bytes and backing happen to agree.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in super::super) enum ReferenceCarrierContract {
    Operation {
        operation: OperationId,
        result: StructuralOperationResult,
    },
    Parameter(terminal_psi::StructuralParameterDeclaration),
}

impl ReferenceCarrierContract {
    pub(in super::super) fn structural_type(&self) -> StructuralTypeId {
        match self {
            Self::Operation { result, .. } => result.structural_type,
            Self::Parameter(parameter) => parameter.structural_type,
        }
    }

    pub(in super::super) fn multiplicity(&self) -> StructuralMultiplicity {
        match self {
            Self::Operation { result, .. } => result.multiplicity,
            Self::Parameter(parameter) => parameter.multiplicity,
        }
    }

    pub(in super::super) fn matches_home(
        &self,
        home: &target_operations::TargetStructuralHomeRequirement,
    ) -> bool {
        match (self, &home.origin) {
            (
                Self::Operation { operation, result },
                target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation: actual_operation,
                    result: actual,
                },
            ) => operation == actual_operation && result == actual,
            (
                Self::Parameter(parameter),
                target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    declaration: actual,
                    ..
                },
            ) => parameter == actual,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in super::super) struct ReferenceFormation {
    pub(in super::super) identity: (PlaceId, Vec<StructuralPathSegment>),
    pub(in super::super) contract: ReferenceCarrierContract,
}

impl ReferenceCustody {
    pub(in super::super) fn reform(
        &mut self,
        operation: OperationId,
        result: &StructuralOperationResult,
    ) {
        self.contract = ReferenceCarrierContract::Operation {
            operation,
            result: result.clone(),
        };
        self.formations = vec![ReferenceFormation {
            identity: self.identity.clone(),
            contract: self.contract.clone(),
        }];
    }
}
