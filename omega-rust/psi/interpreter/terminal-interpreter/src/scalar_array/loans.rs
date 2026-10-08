//! Borrowed fixed arrays pin their owner's activation, never an element copy.
//! The same collection-home descriptor used by live element views decorates
//! the exact fixed-array referent. Its language type and native ABI stay fixed
//! arrays; the companion is interpreter storage custody, not a new source view.

use crate::custody::project_structural_value;
use crate::element_views::{ElementView, ElementViewHome};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::PlaceId;
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape,
};

impl TerminalExecution<'_> {
    pub(crate) fn prepare_scalar_array_loan(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_roots: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError> {
        if !matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types().values(),
            parameter.structural_type,
        )
        .is_none()
        {
            return Ok(None);
        }
        if argument.access != parameter.access
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(malformed());
        }
        let StructuralTypeShape::FixedArray { length, .. } = self
            .program
            .structural_types()
            .get(&parameter.structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        let length = usize::try_from(length).map_err(|_| malformed())?;
        // Validate the source declaration even for empty arrays: no element
        // access will happen there to catch an illicit mutable reborrow.
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let root_type = self.live_structural_type(argument.place)?;
        let source_access = if let Some(source) = machine.structural_parameter(argument.place) {
            if source.structural_type != root_type
                || !source.qualifications.is_empty()
                || !source.projected_qualifications.is_empty()
                || !matches!(
                    source.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                )
                || (source.access != StructuralAccess::Owned
                    && source.multiplicity != StructuralMultiplicity::Unrestricted)
            {
                return Err(malformed());
            }
            source.access
        } else {
            let (operation, declared) = machine
                .producing_operation(argument.place)
                .ok_or_else(malformed)?;
            let source = operation.result.structural().ok_or_else(malformed)?;
            if declared != root_type
                || source.place != argument.place
                || source.structural_type != root_type
                || !matches!(
                    source.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                )
                || !source.claims.is_empty()
                || !source.qualifications.is_empty()
                || !source.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            StructuralAccess::Owned
        };
        if !matches!(
            source_access,
            StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::SharedBorrow
        ) || (parameter.access == StructuralAccess::MutableBorrow
            && source_access == StructuralAccess::SharedBorrow)
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == argument.place && !entry.path.is_empty())
        {
            return Err(malformed());
        }
        if argument.path.is_empty()
            && let Some(loan) = self.element_view_values.get(&argument.place)
        {
            let referent = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(malformed)?;
            if referent.structural_type != parameter.structural_type
                || !loan.matches_descriptor_home(referent)
                || loan.len() != length
            {
                return Err(malformed());
            }
            let loan = loan
                .subslice(0, length, parameter.access)
                .ok_or_else(malformed)?
                .with_descriptor_home(referent.clone());
            return Ok(Some((referent.clone(), loan)));
        }
        let (root, _) =
            self.array_storage_root_staged(argument.place, identity_cursor, local_roots)?;
        let referent = if self.scalar_array_values.contains_key(&argument.place) {
            project_structural_value(&self.program.structural_types(), &root, &argument.path)?
        } else {
            // Reference crossings and selected cases retain their independent
            // access/discriminator checks; a storage home grants neither.
            self.resolve_live_projection(argument.place, &argument.path, argument.access)?
        };
        if referent.structural_type != parameter.structural_type
            || !referent.qualifications.is_empty()
            || !referent.projected_qualifications.is_empty()
        {
            return Err(malformed());
        }
        let loan = ElementView::structural(
            referent.clone(),
            length,
            parameter.access,
            ElementViewHome {
                storage: self.capture_primitive_home(argument.place)?,
                path: argument.path.clone(),
            },
        )
        .with_descriptor_home(referent.clone());
        Ok(Some((referent, loan)))
    }
}
