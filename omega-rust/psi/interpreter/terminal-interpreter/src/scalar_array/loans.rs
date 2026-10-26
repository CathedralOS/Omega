//! Borrowed fixed arrays pin their owner's activation, never an element copy.
//! The same collection-home descriptor used by live element views decorates
//! the exact fixed-array referent. Its language type and native ABI stay fixed
//! arrays; the companion is interpreter storage custody, not a new source view.

use crate::custody::project_structural_value;
use crate::element_views::{ElementView, ElementViewHome};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::runtime_rows::RuntimeRows;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeShape,
};

/// Captured backing retains its actual memberships before a receiving contract
/// is applied. Resident aggregate storage needs no activation companion.
struct CapturedArrayLoan {
    value: TerminalStructuralValue,
    companion: Option<ElementView>,
}

impl CapturedArrayLoan {
    fn into_companion(self) -> Option<(TerminalStructuralValue, ElementView)> {
        self.companion.map(|companion| (self.value, companion))
    }
}

impl TerminalExecution<'_> {
    /// Packed arrays and forwarded collection loans need an activation home.
    /// Ordinary structural arrays already have stable resident identity and
    /// keep the reference owner's normal storage projection. This selects a
    /// physical backing owner, not a different source reference operation.
    pub(crate) fn prepare_scalar_array_reference(
        &self,
        referent_type: StructuralTypeId,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_roots: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError> {
        if self.lent_collection_type(referent_type, argument).is_none() {
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
        // Reference formation owns its authenticated current observations. A
        // backing capture cannot require those fresh facts as standing source
        // memberships, nor publish them back onto the owner's storage.
        Ok(self
            .capture_scalar_array_loan(referent_type, argument, identity_cursor, local_roots)?
            .and_then(CapturedArrayLoan::into_companion))
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
        local_roots: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ElementView)>, TerminalInterpretError> {
        if parameter.access == StructuralAccess::Owned {
            return Ok(None);
        }
        if self
            .lent_collection_type(parameter.structural_type, argument)
            .is_none()
        {
            return Ok(None);
        }
        let types = self.program.structural_types();
        let fixed_array_formal = matches!(
            types.get(&parameter.structural_type).map(|row| &row.shape),
            Some(StructuralTypeShape::FixedArray { .. })
        );
        // Fixed arrays preserve their carrier. The receiving contract is
        // checked against captured standing memberships below; a view
        // conversion cannot acquire the array's qualifications.
        let supported_memberships = fixed_array_formal
            || parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty();
        if argument.access != parameter.access
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !supported_memberships
        {
            return Err(malformed());
        }
        let Some(captured) = self.capture_scalar_array_loan(
            parameter.structural_type,
            argument,
            identity_cursor,
            local_roots,
        )?
        else {
            return Ok(None);
        };
        // Calls and state arrivals transport standing memberships. Unlike
        // reference formation, they cannot add a predicate by capturing a home.
        if fixed_array_formal && !borrowed_array_memberships_match(&captured.value, parameter) {
            return Err(malformed());
        }
        Ok(captured.into_companion())
    }

    fn capture_scalar_array_loan(
        &self,
        structural_type: StructuralTypeId,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_roots: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<CapturedArrayLoan>, TerminalInterpretError> {
        if argument.access == StructuralAccess::Owned {
            return Ok(None);
        }
        let Some(collection_type) = self.lent_collection_type(structural_type, argument) else {
            return Ok(None);
        };
        let types = self.program.structural_types();
        let fixed_array_formal = matches!(
            types.get(&structural_type).map(|row| &row.shape),
            Some(StructuralTypeShape::FixedArray { .. })
        );
        // A fixed-array loan preserves the exact carrier and its standing
        // memberships, checked against the selected source below. Pinning
        // backing neither reads bytes nor needs a byte-predicate theory.
        // An element-view formal changes carrier; it keeps the distinct
        // qualification-free descriptor contract rather than relabelling the
        // source array's memberships as view memberships.
        let preserves_membership_carrier = |whole: &[_], projected: &[_]| {
            fixed_array_formal || whole.is_empty() && projected.is_empty()
        };
        if crate::custody::place_is_claimed(&self.live_claims, argument.place) {
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
            {
                return Err(malformed());
            }
            StructuralAccess::Owned
        };
        // Root declarations authenticate access and custody. Only the selected
        // array's memberships classify this loan; unrelated containing-record
        // invariants remain with checked formation and ancestor preservation.
        // Capturing a loan pins backing; it never observes its contents. The
        // requested access must still fit the actual source, including when
        // forwarding a write-only loan or attenuating mutable access.
        if !argument.access.admits(source_access)
            || (!argument
                .path
                .contains(&terminal_psi::StructuralPathSegment::Referent)
                && self
                    .live_affine_frontier
                    .iter()
                    .any(|entry| entry.place == argument.place && !entry.path.is_empty()))
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
                || !preserves_membership_carrier(
                    &referent.qualifications,
                    &referent.projected_qualifications,
                )
                || !loan.matches_descriptor_home(referent)
                || loan.len() != length
            {
                return Err(malformed());
            }
            let mut presented = referent.clone();
            presented.structural_type = structural_type;
            let loan = loan
                .subslice(0, length, argument.access)
                .ok_or_else(malformed)?
                .with_descriptor_home(presented.clone());
            return Ok(Some(CapturedArrayLoan {
                value: presented,
                companion: Some(loan),
            }));
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
            || !preserves_membership_carrier(
                &referent.qualifications,
                &referent.projected_qualifications,
            )
        {
            return Err(malformed());
        }
        // A resident fixed-array reference binds the already-loaded opaque
        // referent through ordinary aggregate transport. Pinning the argument's
        // descriptor path here would replay a replaceable pointer after the
        // call or state edge. Only packed storage and existing collection loans
        // need an activation-home companion. A real element-view formal still
        // forms its explicit descriptor through the collection path below.
        if fixed_array_formal
            && captured_home.is_none()
            && !self.scalar_array_values.contains_key(&argument.place)
            && !self.element_view_values.contains_key(&argument.place)
        {
            return Ok(Some(CapturedArrayLoan {
                value: referent,
                companion: None,
            }));
        }
        let mut presented = referent.clone();
        presented.structural_type = structural_type;
        let home = match captured_home {
            Some(home) => Some(home),
            None if let Some(loan) = self.element_view_values.get(&argument.place) => {
                // A selected row of a fixed-array formal has no descriptor
                // crossing in its path, but still borrows the formal's original
                // home. Pinning this forwarding activation would leave returned
                // rows pointing at a frame that has already retired.
                if !loan.matches_descriptor_home(&root)
                    || !argument.access.admits_referent(loan.access())
                {
                    return Err(malformed());
                }
                loan.captured_storage_home(&self.program.structural_types(), &argument.path)?
            }
            None => Some(ElementViewHome {
                storage: self.capture_primitive_home(argument.place)?,
                path: argument.path.clone(),
            }),
        };
        let loan = ElementView::structural(referent, length, argument.access, home, argument.place)
            .with_descriptor_home(presented.clone());
        Ok(Some(CapturedArrayLoan {
            value: presented,
            companion: Some(loan),
        }))
    }

    /// The fixed-array type a borrowed formal lends: the formal's own
    /// scalar-array type, or, for an element-view formal, the argument's
    /// fixed array when its element is the view's element. Any other pairing
    /// (a forwarded view, a byte carrier) is not this loan.
    fn lent_collection_type(
        &self,
        structural_type: StructuralTypeId,
        argument: &StructuralArgument,
    ) -> Option<StructuralTypeId> {
        let types = self.program.structural_types();
        match types.get(&structural_type)?.shape {
            StructuralTypeShape::FixedArray { .. } => {
                terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)?;
                Some(structural_type)
            }
            StructuralTypeShape::ElementView { element, .. } => {
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

/// Ordinary array loans preserve the selected standing contract without reading
/// its bytes. Shared access may forget facts; exclusive access retains every
/// receiving obligation. This compares existing membership identities; neither
/// pinning the array nor matching its contract establishes a new theory.
pub(crate) fn borrowed_array_memberships_match(
    value: &TerminalStructuralValue,
    parameter: &StructuralParameterDeclaration,
) -> bool {
    parameter
        .qualifications
        .iter()
        .all(|domain| value.qualifications.contains(domain))
        && parameter
            .projected_qualifications
            .iter()
            .all(|row| value.projected_qualifications.contains(row))
        && (parameter.access == StructuralAccess::SharedBorrow
            || value
                .qualifications
                .iter()
                .all(|domain| parameter.qualifications.contains(domain))
                && value
                    .projected_qualifications
                    .iter()
                    .all(|row| parameter.projected_qualifications.contains(row)))
}
