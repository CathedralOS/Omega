pub use omega_program_contracts::Multiplicity;

use crate::{Access, ValueType};
use omega_resolved::DeclarationId;

impl ValueType {
    /// A tuple has the most restrictive multiplicity of its components. The
    /// caller resolves declarations and generic parameters in its own scope.
    pub fn multiplicity_with(
        &self,
        nominal: &impl Fn(&DeclarationId) -> Option<Multiplicity>,
        parameter: &impl Fn(&DeclarationId, usize) -> Option<Multiplicity>,
    ) -> Option<Multiplicity> {
        use Multiplicity::*;
        match self {
            Self::Array(element, _) | Self::IndexedArray(element, _) | Self::Qualified(element, _) | Self::AtLifetime(element, _) => {
                element.multiplicity_with(nominal, parameter)
            }
            Self::Tuple(parts) => parts.iter().try_fold(Copy, |m, part| Some(m.max(part.multiplicity_with(nominal, parameter)?))),
            Self::Nominal(id) | Self::Applied(id, _) => nominal(id),
            Self::Parameter { owner, index } => parameter(owner, *index),
            Self::Reference(Access::Mut | Access::Write, _) | Self::Witness { .. } => Some(Affine),
            _ => Some(Copy),
        }
    }
}
