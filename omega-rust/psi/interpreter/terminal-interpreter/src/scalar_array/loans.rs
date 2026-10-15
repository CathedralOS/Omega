//! Borrowed fixed arrays pin their owner's activation, never an element copy.
//! The same collection-home descriptor used by live element views decorates
//! the exact fixed-array referent. Its language type and native ABI stay fixed
//! arrays; the companion is interpreter storage custody, not a new source view.

use crate::custody::project_structural_value;
use crate::element_views::{ElementView, ElementViewHome};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape,
};

impl TerminalExecution<'_> {
    /// Packed arrays and forwarded collection loans need an activation home.
    /// Ordinary structural arrays already have stable resident identity and
    /// keep the reference owner's normal storage projection. This selects a
    /// physical backing owner, not a different source reference operation.
    pub(crate) fn prepare_scalar_array_reference(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_roots: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError> {
        if self.lent_collection_type(parameter, argument).is_none() {
            return Ok(None);
        }
        if !self.scalar_array_values.contains_key(&argument.place)
            && !self.element_view_values.contains_key(&argument.place)
            && self
                .resolve_live_storage_projection(argument.place, &argument.path, argument.access)?
                .collection_home
                .is_none()
        {
            return Ok(None);
        }
        self.prepare_scalar_array_loan(parameter, argument, identity_cursor, local_roots)
    }

    /// A fixed array lent to a fixed-array formal, or lent whole to an
    /// element-view formal of its own element type (`walk(&items)` for
    /// `values: &[i32]`), borrows the same live collection home. The view
    /// formal sees that home under its declared view type, exactly the
    /// `WholeCollection` view `EstablishElementView` forms, so writes through
    /// a mutable loan land in the owner.
    pub(crate) fn prepare_scalar_array_loan(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_roots: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError> {
        if parameter.access == StructuralAccess::Owned {
            return Ok(None);
        }
        let Some(collection_type) = self.lent_collection_type(parameter, argument) else {
            return Ok(None);
        };
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
            .get(&collection_type)
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
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
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
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                || !source.claims.is_empty()
                || !source.qualifications.is_empty()
                || !source.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            StructuralAccess::Owned
        };
        // Capturing a loan pins backing; it never observes its contents. The
        // requested access must still fit the actual source, including when
        // forwarding a write-only loan or attenuating mutable access.
        if !parameter.access.admits(source_access)
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
            if referent.structural_type != collection_type
                || !loan.matches_descriptor_home(referent)
                || loan.len() != length
            {
                return Err(malformed());
            }
            let mut presented = referent.clone();
            presented.structural_type = parameter.structural_type;
            let loan = loan
                .subslice(0, length, parameter.access)
                .ok_or_else(malformed)?
                .with_descriptor_home(presented.clone());
            return Ok(Some((presented, loan)));
        }
        let (root, _) =
            self.array_storage_root_staged(argument.place, identity_cursor, local_roots)?;
        let (referent, captured_home) = if self.scalar_array_values.contains_key(&argument.place) {
            (
                project_structural_value(&self.program.structural_types(), &root, &argument.path)?,
                None,
            )
        } else {
            // Reference crossings and selected cases retain their independent
            // access/discriminator checks; a storage home grants neither.
            let projected = self.resolve_live_storage_projection(
                argument.place,
                &argument.path,
                argument.access,
            )?;
            (projected.value, projected.collection_home)
        };
        if referent.structural_type != collection_type
            || !referent.qualifications.is_empty()
            || !referent.projected_qualifications.is_empty()
        {
            return Err(malformed());
        }
        let mut presented = referent.clone();
        presented.structural_type = parameter.structural_type;
        let home = match captured_home {
            Some(home) => home,
            None if let Some(loan) = self.element_view_values.get(&argument.place) => {
                // A selected row of a fixed-array formal has no descriptor
                // crossing in its path, but still borrows the formal's original
                // home. Pinning this forwarding activation would leave returned
                // rows pointing at a frame that has already retired.
                if !loan.matches_descriptor_home(&root)
                    || !parameter.access.admits_referent(loan.access())
                {
                    return Err(malformed());
                }
                loan.captured_storage_home(&self.program.structural_types(), &argument.path)?
            }
            None => ElementViewHome {
                storage: self.capture_primitive_home(argument.place)?,
                path: argument.path.clone(),
            },
        };
        let loan = ElementView::structural(referent, length, parameter.access, home)
            .with_descriptor_home(presented.clone());
        Ok(Some((presented, loan)))
    }

    /// The fixed-array type a borrowed formal lends: the formal's own
    /// scalar-array type, or, for an element-view formal, the argument's
    /// fixed array when its element is the view's element. Any other pairing
    /// (a forwarded view, a byte carrier) is not this loan.
    fn lent_collection_type(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Option<StructuralTypeId> {
        let types = self.program.structural_types();
        match types.get(&parameter.structural_type)?.shape {
            StructuralTypeShape::FixedArray { .. } => {
                terminal_semantics::scalar_array_leaf_shape(
                    types.values(),
                    parameter.structural_type,
                )?;
                Some(parameter.structural_type)
            }
            StructuralTypeShape::ElementView { element } => {
                let root_type = self.live_structural_type(argument.place).ok()?;
                let collection_type =
                    crate::custody::resolve_structural_path_type(&types, root_type, &argument.path)
                        .ok()?;
                matches!(
                    types.get(&collection_type)?.shape,
                    StructuralTypeShape::FixedArray { element: source, .. } if source == element
                )
                .then_some(collection_type)
            }
            _ => None,
        }
    }
}
