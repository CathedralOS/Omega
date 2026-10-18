//! Places only an ordinary body establishes for itself: trivial affine
//! locals. Scalar locals, scalar arrays, primitive locals and structural
//! values emit through the shared `operation_frame`.

use super::MachineEmission;
use crate::unit::{CheckedUnitEffectOperationPlan, LoweringError, OperationKind};

impl MachineEmission<'_> {
    pub(super) fn establish_trivial_affine_local(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine {
                declaration_ordinal,
                statement_index,
                ..
            },
        ) = operation
        else {
            unreachable!("dispatched establish_trivial_affine_local")
        };
        let local = self
            .local_places
            .get(
                usize::try_from(*declaration_ordinal)
                    .map_err(|_| LoweringError::Unsupported("Unit local ordinal exceeds usize"))?,
            )
            .ok_or(LoweringError::Unsupported(
                "Unit local ordinal is not dense",
            ))?;
        self.operations
            .structural_declarations
            .push((local.id, *statement_index));
        Ok(Some(OperationKind::EstablishTrivialAffineLocal {
            destination: local.id,
        }))
    }
}
