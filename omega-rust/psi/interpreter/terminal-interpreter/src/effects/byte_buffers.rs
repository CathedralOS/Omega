//! Prepare ordinary boundary loans, then stage exact external byte writeback.
mod writeback;

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::calls::arguments::StructuralCallArguments;
use crate::custody::bind_structural_arguments;
use crate::custody::resolve_structural_arguments;
use crate::effects::storage_loans::BoundaryStorageLoans;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::structural_identities::LocalStructuralIdentities;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalStructuralValue;
use std::collections::BTreeMap;
use terminal_psi::ByteSequenceCarrier;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeShape;
pub use writeback::TerminalBoundaryByteBuffer;
use writeback::{BoundaryByteBufferBinding, BoundaryByteWriteScope};

pub(crate) struct BoundaryArguments {
    local_identity_cursor: Option<u64>,
    pub(crate) values: Vec<TerminalStructuralValue>,
    pub(crate) bytes: Vec<Option<Vec<u8>>>,
    pub(crate) buffers: Vec<TerminalBoundaryByteBuffer>,
    pub(crate) storage_loans: BoundaryStorageLoans,
    bindings: Vec<BoundaryByteBufferBinding>,
    byte_sequences: Vec<Option<ByteSequenceBinding>>,
    primitive_loans: Vec<Option<crate::primitive_storage::PrimitiveLoan>>,
    element_views: Vec<Option<crate::element_views::ElementView>>,
}

impl BoundaryArguments {
    pub(crate) fn reserve_host_result(
        &self,
        identities: &mut LocalStructuralIdentities,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        identities.reserve_host_at_cursor(value, self.local_identity_cursor)
    }

    /// Provider bodies borrow the live array/field binding, not the staged external
    /// buffers. Nested boundary calls commit backing; provider return must not
    /// restore the pre-call snapshot.
    pub(crate) fn into_call_arguments(
        mut self,
        parameters: &[StructuralParameterDeclaration],
    ) -> Result<StructuralCallArguments, TerminalInterpretError> {
        if parameters.len() != self.byte_sequences.len() || parameters.len() != self.values.len() {
            return Err(malformed());
        }
        // An owned byte copy was checked against every domain its formal
        // names, so the callee holds the copy under those domains, exactly
        // as an ordinary call's owned copy.
        for ((parameter, value), binding) in parameters
            .iter()
            .zip(&mut self.values)
            .zip(&self.byte_sequences)
        {
            if parameter.access == StructuralAccess::Owned && binding.is_some() {
                value.absorb_qualifications(&parameter.qualifications);
            }
        }
        let values = bind_structural_arguments(parameters, &self.values)?;
        let mut byte_sequences = BTreeMap::new();
        let mut primitive_loans = BTreeMap::new();
        for (parameter, loan) in parameters.iter().zip(self.primitive_loans) {
            if let Some(loan) = loan {
                if values.get(&parameter.place) != Some(&loan.referent)
                    || loan.access != parameter.access
                    || primitive_loans.insert(parameter.place, loan).is_some()
                {
                    return Err(malformed());
                }
            }
        }
        let mut element_views = BTreeMap::new();
        for (parameter, view) in parameters.iter().zip(self.element_views) {
            let Some(view) = view else {
                continue;
            };
            if !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) || parameter.access != view.access()
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || element_views.insert(parameter.place, view).is_some()
            {
                return Err(malformed());
            }
        }
        for (parameter, binding) in parameters.iter().zip(self.byte_sequences) {
            let Some(binding) = binding else {
                continue;
            };
            let access = match &binding {
                // A detached copy is the owned formal's own value.
                ByteSequenceBinding::Immutable(view)
                    if parameter.access == StructuralAccess::Owned
                        && view.matches_descriptor_home(
                            values.get(&parameter.place).ok_or_else(malformed)?,
                        ) =>
                {
                    StructuralAccess::Owned
                }
                ByteSequenceBinding::Immutable(_) => StructuralAccess::SharedBorrow,
                ByteSequenceBinding::MutableField { .. }
                | ByteSequenceBinding::MutableOwner { .. }
                    if parameter.access == StructuralAccess::WriteOnlyBorrow =>
                {
                    StructuralAccess::WriteOnlyBorrow
                }
                ByteSequenceBinding::MutableField { .. }
                | ByteSequenceBinding::MutableOwner { .. }
                | ByteSequenceBinding::MutableArray { .. } => StructuralAccess::MutableBorrow,
            };
            if parameter.access != access
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
                || match &binding {
                    ByteSequenceBinding::MutableField { referent, .. }
                    | ByteSequenceBinding::MutableOwner { referent, .. } => {
                        referent.qualifications != parameter.qualifications
                    }
                    ByteSequenceBinding::Immutable(_) => false,
                    ByteSequenceBinding::MutableArray { .. } => {
                        !parameter.qualifications.is_empty()
                    }
                }
                || byte_sequences.insert(parameter.place, binding).is_some()
            {
                return Err(malformed());
            }
        }
        Ok(StructuralCallArguments {
            local_identity_cursor: self.local_identity_cursor,
            descriptor_parameters: parameters.to_vec(),
            values,
            byte_sequences,
            primitive_loans,
            scalar_arrays: BTreeMap::new(),
            element_views,
        })
    }

    /// A handler can reorder its slice, but cannot thereby retarget writeback.
    /// Check every binding before committing either fields or the result.
    pub(crate) fn validate_writeback(
        &mut self,
        execution: &TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        if self.buffers.len() != self.bindings.len() {
            return Err(malformed());
        }
        for (buffer, binding) in self.buffers.iter().zip(&self.bindings) {
            binding.validate(execution, buffer)?;
        }
        self.storage_loans.validate_writeback(execution)
    }

    pub(crate) fn commit(
        self,
        execution: &mut TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        for (buffer, binding) in self.buffers.into_iter().zip(self.bindings) {
            binding.commit(execution, buffer)?;
        }
        self.storage_loans.commit(execution)
    }

    /// After input validation, publish the staged allocation cursor before a
    /// host scalar-case result allocates its original home. Input writeback
    /// remains deferred; resetting this cursor after result publication would
    /// collide with that newly allocated result.
    pub(crate) fn commit_identity_cursor(&self, execution: &mut TerminalExecution) {
        execution
            .local_structural_identities
            .commit_cursor(self.local_identity_cursor);
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn resolve_boundary_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
        storage_arguments: &[usize],
    ) -> Result<BoundaryArguments, TerminalInterpretError> {
        if parameters.len() != arguments.len() {
            return Err(malformed());
        }
        // Primitive loans use the general scalar backing owner rather than
        // the byte-view presentation protocol. Prepare all other arguments
        // together so shared array presentations keep their invocation-local
        // identity association, then restore the original structural order.
        let mut ordinary_parameters = Vec::new();
        let mut ordinary_arguments = Vec::new();
        let mut scalar_indices = Vec::new();
        for (argument_index, (parameter, argument)) in parameters.iter().zip(arguments).enumerate()
        {
            let scalar_loan = matches!(
                self.program
                    .structural_types()
                    .get(&parameter.structural_type)
                    .ok_or_else(malformed)?
                    .shape,
                StructuralTypeShape::PrimitiveScalar(_)
            ) && parameter.access != StructuralAccess::Owned;
            if scalar_loan {
                scalar_indices.push(argument_index);
            } else {
                ordinary_parameters.push(parameter.clone());
                ordinary_arguments.push(argument.clone());
            }
        }
        let mut resolved =
            self.prepare_boundary_arguments(&ordinary_parameters, &ordinary_arguments)?;
        for argument_index in scalar_indices {
            let value = resolved.storage_loans.capture_scalar(
                self,
                &parameters[argument_index],
                &arguments[argument_index],
                argument_index,
                &mut resolved.local_identity_cursor,
            )?;
            resolved.values.insert(argument_index, value);
            resolved.byte_sequences.insert(argument_index, None);
            resolved.primitive_loans.insert(argument_index, None);
            resolved.element_views.insert(argument_index, None);
        }
        let mut requested = std::collections::BTreeSet::new();
        for &argument_index in storage_arguments {
            if !requested.insert(argument_index) {
                return Err(malformed());
            }
            let parameter = parameters.get(argument_index).ok_or_else(malformed)?;
            let argument = arguments.get(argument_index).ok_or_else(malformed)?;
            if !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) {
                return Err(malformed());
            }
            if !matches!(
                self.program
                    .structural_types()
                    .get(&parameter.structural_type)
                    .ok_or_else(malformed)?
                    .shape,
                StructuralTypeShape::PrimitiveScalar(_)
            ) {
                resolved.storage_loans.capture_fields(
                    self,
                    parameter,
                    argument,
                    &resolved.values[argument_index],
                    resolved.element_views[argument_index].as_ref(),
                    argument_index,
                )?;
            }
        }
        for (argument_index, binding) in resolved.byte_sequences.iter().enumerate() {
            match binding {
                None => resolved.bytes.push(None),
                Some(ByteSequenceBinding::Immutable(view)) => {
                    resolved.bytes.push(Some(view.bytes().to_vec()));
                }
                Some(
                    binding @ (ByteSequenceBinding::MutableArray { .. }
                    | ByteSequenceBinding::MutableField { .. }
                    | ByteSequenceBinding::MutableOwner { .. }),
                ) => {
                    let parameter = &parameters[argument_index];
                    let declaration = self
                        .program
                        .structural_types()
                        .get(&parameter.structural_type)
                        .ok_or_else(malformed)?;
                    let scope = match declaration.shape {
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            ..
                        }) => BoundaryByteWriteScope::View,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                            ..
                        }) => BoundaryByteWriteScope::Owner,
                        _ => return Err(malformed()),
                    };
                    let (binding, buffer) =
                        BoundaryByteBufferBinding::capture(self, argument_index, binding, scope)?;
                    resolved.bytes.push(Some(buffer.bytes().to_vec()));
                    resolved.buffers.push(buffer);
                    resolved.bindings.push(binding);
                }
            }
        }
        Ok(resolved)
    }

    pub(crate) fn prepare_boundary_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
    ) -> Result<BoundaryArguments, TerminalInterpretError> {
        if parameters.len() != arguments.len() {
            return Err(malformed());
        }
        let mut resolved = BoundaryArguments {
            local_identity_cursor: self.local_structural_identities.cursor(),
            values: Vec::with_capacity(arguments.len()),
            bytes: Vec::with_capacity(arguments.len()),
            buffers: Vec::new(),
            storage_loans: BoundaryStorageLoans::default(),
            bindings: Vec::new(),
            byte_sequences: Vec::with_capacity(arguments.len()),
            primitive_loans: Vec::with_capacity(arguments.len()),
            element_views: Vec::with_capacity(arguments.len()),
        };
        let mut local_array_roots = BTreeMap::new();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if matches!(
                self.program
                    .structural_types()
                    .get(&parameter.structural_type)
                    .ok_or_else(malformed)?
                    .shape,
                StructuralTypeShape::PrimitiveScalar(_)
            ) && parameter.access != StructuralAccess::Owned
            {
                let loan = self.capture_primitive_loan(
                    parameter,
                    argument,
                    &mut resolved.local_identity_cursor,
                )?;
                resolved.values.push(loan.referent.clone());
                resolved.primitive_loans.push(Some(loan));
                resolved.byte_sequences.push(None);
                resolved.element_views.push(None);
                continue;
            }
            resolved.primitive_loans.push(None);
            if let Some((referent, loan)) = self.prepare_scalar_array_loan(
                parameter,
                argument,
                &mut resolved.local_identity_cursor,
                &mut local_array_roots,
            )? {
                resolved.values.push(referent);
                resolved.byte_sequences.push(None);
                resolved.element_views.push(Some(loan));
                continue;
            }
            if let Some((referent, binding)) = self.prepare_byte_sequence_loan(
                parameter,
                argument,
                &mut resolved.local_identity_cursor,
                &mut local_array_roots,
            )? {
                resolved.values.push(referent);
                resolved.byte_sequences.push(Some(binding));
                resolved.element_views.push(None);
                continue;
            }
            let values = resolve_structural_arguments(
                &self.program.structural_types(),
                &self.structural_values,
                std::slice::from_ref(argument),
            )?;
            let mut bytes = self.bind_byte_sequence_arguments(
                std::slice::from_ref(parameter),
                std::slice::from_ref(argument),
                &values,
            )?;
            let binding = bytes.remove(&parameter.place);
            resolved.byte_sequences.push(binding);
            let mut element_views = self.bind_element_view_arguments(
                std::slice::from_ref(parameter),
                std::slice::from_ref(argument),
                &values,
            )?;
            resolved
                .element_views
                .push(element_views.remove(&parameter.place));
            resolved.values.extend(values);
        }
        Ok(resolved)
    }

    /// One borrowed byte argument's exact referent and binding: a shared
    /// descriptor, an array presented as a byte view, a bounded owner lent
    /// shared or exclusively, or a view field. Calls, checked providers and
    /// state edges lend through this same preparation. `None` leaves the
    /// argument to the direct descriptor rebinding every binder applies last.
    pub(crate) fn prepare_byte_sequence_loan(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_array_roots: &mut BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        if let Some(prepared) = self.prepare_shared_descriptor_argument(parameter, argument)? {
            return Ok(Some(prepared));
        }
        if let Some(prepared) = self.prepare_owner_view_argument(parameter, argument)? {
            return Ok(Some(prepared));
        }
        // Reuse the ordinary call's exact initialized-array binding. It can
        // enter a checked provider and be forwarded again. External staging
        // lends its exact range, never field-replacement authority.
        if let Some(prepared) = self.prepare_array_view_argument_staged(
            parameter,
            argument,
            identity_cursor,
            local_array_roots,
        )? {
            return Ok(Some(prepared));
        }
        let declaration = self
            .program
            .structural_types()
            .get(&parameter.structural_type)
            .ok_or_else(malformed)?;
        if declaration.shape.is_bounded_owned_byte_sequence()
            && matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
        {
            if argument.access != parameter.access
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            let binding = self.resolve_mutable_boundary_binding(parameter, argument)?;
            let referent = binding.owner_loan_referent().ok_or_else(malformed)?.clone();
            return Ok(Some((referent, binding)));
        }
        if parameter.access == StructuralAccess::SharedBorrow
            && declaration.shape.is_bounded_owned_byte_sequence()
        {
            if argument.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            return self
                .resolve_shared_owner_binding(parameter, argument)
                .map(Some);
        }
        if parameter.access == StructuralAccess::SharedBorrow
            && declaration.shape.is_borrowed_byte_view()
            && !argument.path.is_empty()
        {
            if argument.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            let (referent, binding) = self.resolve_shared_boundary_binding(parameter, argument)?;
            // A lent view field must satisfy every domain the formal
            // names (`text: &[u8] in Utf8` from `main.name`). As for a
            // whole shared descriptor, execution checks the bytes the
            // formal observes rather than the field's declaration.
            if !terminal_semantics::byte_literal_qualifications_hold_in(
                self.program.catalogs.structural_domains.iter(),
                parameter.structural_type,
                &parameter.qualifications,
                binding.immutable()?.bytes(),
            ) {
                return Err(malformed());
            }
            return Ok(Some((referent, binding)));
        }
        if parameter.access == StructuralAccess::MutableBorrow
            && declaration.shape.is_borrowed_byte_view()
        {
            if argument.access != StructuralAccess::MutableBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            let binding = self.resolve_mutable_boundary_binding(parameter, argument)?;
            let ByteSequenceBinding::MutableField {
                field: destination,
                capacity,
                referent,
                ..
            } = &binding
            else {
                return Err(malformed());
            };
            let bytes = self
                .structural_byte_sequence_fields
                .get(destination)
                .ok_or_else(malformed)?
                .bytes();
            if bytes.len() as u128 > u128::from(*capacity) {
                return Err(malformed());
            }
            return Ok(Some((referent.clone(), binding)));
        }
        Ok(None)
    }

    /// A whole bounded owner lent to a plain shared view (`observe(line)` for
    /// `bytes: &[u8]`) presents its live bytes under the view's declaration.
    /// The owner keeps its identity for alias checks; the view formal's
    /// domains must hold for the bytes it observes.
    fn prepare_owner_view_argument(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        let types = self.program.structural_types();
        if parameter.access != StructuralAccess::SharedBorrow
            || !argument.path.is_empty()
            || !types
                .get(&parameter.structural_type)
                .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
        {
            return Ok(None);
        }
        let Some(owner) = self.structural_values.get(&argument.place) else {
            return Ok(None);
        };
        if !types
            .get(&owner.structural_type)
            .is_some_and(|declaration| declaration.is_bounded_owned_byte_sequence())
        {
            return Ok(None);
        }
        if argument.access != StructuralAccess::SharedBorrow
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(malformed());
        }
        let binding = self
            .byte_sequence_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        let ByteSequenceBinding::Immutable(bytes) =
            self.reborrow_byte_sequence_binding(binding, owner, StructuralAccess::SharedBorrow)?
        else {
            return Err(malformed());
        };
        if !terminal_semantics::byte_literal_qualifications_hold_in(
            self.program.catalogs.structural_domains.iter(),
            parameter.structural_type,
            &parameter.qualifications,
            bytes.bytes(),
        ) {
            return Err(malformed());
        }
        let mut view = owner.clone();
        view.structural_type = parameter.structural_type;
        view.qualifications = parameter.qualifications.clone();
        let bytes = bytes.with_descriptor_home(view.clone());
        Ok(Some((view, ByteSequenceBinding::Immutable(bytes))))
    }

    fn resolve_mutable_boundary_binding(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        if argument.path.is_empty() {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(malformed)?;
            if value.structural_type != parameter.structural_type {
                return Err(malformed());
            }
            let binding = self
                .byte_sequence_values
                .get(&argument.place)
                .ok_or_else(malformed)?;
            if let ByteSequenceBinding::Immutable(_) = binding {
                return self.lend_frame_owner(parameter, argument.place, value);
            }
            binding.validate_mutable_referent(&self.program.structural_types(), value)?;
            return Ok(binding.clone());
        }
        let (mut parent, identity, field) = self.structural_field_binding_frame(argument)?;
        let Some(ByteSequenceCarrier::BoundedOwned { capacity }) =
            field.field_type.byte_sequence_carrier()
        else {
            return Err(malformed());
        };
        let destination = StructuralByteSequenceRuntimeField {
            parent: StructuralRuntimePlace::from(&parent),
            field: field.id,
        };

        let parent_type = parent.structural_type;
        // Present the callee parameter type, retaining the original
        // referent and full field path for alias/custody checks.
        parent.structural_type = parameter.structural_type;
        parent.qualifications = parameter.qualifications.clone();
        parent
            .path
            .push(StructuralPathSegment::Field(identity.clone()));

        let binding = ByteSequenceBinding::MutableField {
            field: destination,
            capacity,
            parent_type,
            referent: parent,
        };
        binding.validate_mutable_referent(
            &self.program.structural_types(),
            binding.owner_loan_referent().ok_or_else(malformed)?,
        )?;
        Ok(binding)
    }

    /// A frame-local bounded owner lent exclusively stays in its activation:
    /// the loan pins that home under the owner's own identity, and every
    /// write through it lands in the owner. The formal must name exactly the
    /// owner's domains, since writes through it re-prove only those.
    fn lend_frame_owner(
        &self,
        parameter: &StructuralParameterDeclaration,
        place: semantic_vocabulary::PlaceId,
        owner: &TerminalStructuralValue,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        let (crate::byte_sequences::write::MutableByteSequenceStorage::Owned(home), _) =
            self.mutable_byte_sequence_storage(place)?
        else {
            return Err(malformed());
        };
        let Some(ByteSequenceCarrier::BoundedOwned { capacity }) = self
            .program
            .structural_types()
            .get(&owner.structural_type)
            .and_then(|declaration| match &declaration.shape {
                StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                _ => None,
            })
        else {
            return Err(malformed());
        };
        if home.place != place || owner.qualifications != parameter.qualifications {
            return Err(malformed());
        }
        let binding = ByteSequenceBinding::MutableOwner {
            home,
            capacity,
            referent: owner.clone(),
        };
        binding.validate_mutable_referent(&self.program.structural_types(), owner)?;
        Ok(binding)
    }

    /// Shared owner arguments snapshot the live prefix while retaining the
    /// original referent and exact qualifications for alias checks and forwarding.
    fn resolve_shared_owner_binding(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError> {
        if !argument.path.is_empty() {
            return self.resolve_shared_boundary_binding(parameter, argument);
        }
        let value = self
            .structural_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        if value.structural_type != parameter.structural_type
            || parameter
                .qualifications
                .iter()
                .any(|domain| !value.qualifications.contains(domain))
        {
            return Err(malformed());
        }
        let binding = self
            .byte_sequence_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        let bytes = match binding {
            ByteSequenceBinding::Immutable(bytes) => bytes.clone(),
            ByteSequenceBinding::MutableField { field, .. } => {
                binding.validate_mutable_referent(&self.program.structural_types(), value)?;
                self.structural_byte_sequence_fields
                    .get(field)
                    .ok_or_else(malformed)?
                    .clone()
            }
            ByteSequenceBinding::MutableOwner { home, .. } => {
                binding.validate_mutable_referent(&self.program.structural_types(), value)?;
                self.byte_owner_bytes(*home)?.clone()
            }
            ByteSequenceBinding::MutableArray { .. } => {
                return Err(malformed());
            }
        };
        Ok((
            value.clone(),
            ByteSequenceBinding::Immutable(bytes.with_descriptor_home(value.clone())),
        ))
    }

    /// Bind a shared byte-view argument to one initialized inline byte field.
    /// The view observes the field's live bytes; the shared loan grants no
    /// writeback, so no staged buffer or field coordinate is retained.
    fn resolve_shared_boundary_binding(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError> {
        let (mut parent, identity, field) = self.structural_field_binding_frame(argument)?;
        let carrier = field
            .field_type
            .byte_sequence_carrier()
            .ok_or_else(malformed)?;
        if !carrier.supports_result_qualifications() {
            return Err(malformed());
        }
        let source = StructuralByteSequenceRuntimeField {
            parent: StructuralRuntimePlace::from(&parent),
            field: field.id,
        };
        let bytes = self
            .structural_byte_sequence_fields
            .get(&source)
            .ok_or_else(malformed)?;
        let bytes = match carrier {
            ByteSequenceCarrier::BorrowedView { .. } => bytes.clone(),
            ByteSequenceCarrier::BoundedOwned { .. } => {
                ByteSequenceView::new(bytes.bytes().to_vec())
            }
            ByteSequenceCarrier::BoundedView { .. } => {
                return Err(malformed());
            }
        };
        parent.structural_type = parameter.structural_type;
        parent.qualifications = parameter.qualifications.clone();
        parent
            .path
            .push(StructuralPathSegment::Field(identity.clone()));
        let bytes = bytes.with_descriptor_home(parent.clone());
        Ok((parent, ByteSequenceBinding::Immutable(bytes)))
    }

    /// Resolves `argument`'s last-field parent record: the shared preamble
    /// the mutable and shared boundary bindings walk before their carrier
    /// policies diverge. Returns the resolved parent value, the last
    /// segment's field name, and the field declaration.
    fn structural_field_binding_frame<'a>(
        &'a self,
        argument: &'a StructuralArgument,
    ) -> Result<
        (
            TerminalStructuralValue,
            &'a String,
            &'a terminal_psi::StructuralFieldDeclaration,
        ),
        TerminalInterpretError,
    > {
        let Some((StructuralPathSegment::Field(identity), prefix)) = argument.path.split_last()
        else {
            return Err(malformed());
        };
        let parent = resolve_structural_arguments(
            &self.program.structural_types(),
            &self.structural_values,
            &[StructuralArgument {
                place: argument.place,
                path: prefix.to_vec(),
                access: argument.access,
            }],
        )?
        .pop()
        .ok_or_else(malformed)?;
        let Some(fields) = self.program.record_fields(&parent.structural_type) else {
            return Err(malformed());
        };
        let field = terminal_psi::live_field_by_identity(fields, identity).ok_or_else(malformed)?;
        Ok((parent, identity, field))
    }
}
