//! A paused entry keeps its storage. The next publication must give every
//! stored path, type and membership a checked meaning before resuming its execution.

use crate::boundary::values::{StructuralFieldNamespace, StructuralRuntimePlace};
use crate::storage::byte_sequences::binding::ByteSequenceBinding;
use crate::storage::primitive_storage::{PrimitiveLoan, PrimitiveStorageEndpoint};
use crate::storage::reference::ReferenceReferent;
use crate::{TerminalEffect, TerminalExecution, TerminalStructuralValue};
use semantic_vocabulary::{ScalarType, StructuralDomainId, StructuralFieldId, StructuralTypeId};
use terminal_psi::StructuralPathSegment;
use terminal_semantics::continuation::CheckedPrefix;

// The active machine and its suspended callers hold the same register tables.
macro_rules! registers {
    ($frame:ident, $field:ident) => {
        $frame
                                    .call_stack
                                    .iter()
                                    .map(|caller| &caller.$field)
                                    .chain(std::iter::once(&$frame.$field))
    };
    (mut $frame:ident, $field:ident) => {
        $frame
                                    .call_stack
                                    .iter_mut()
                                    .map(|caller| &mut caller.$field)
                                    .chain(std::iter::once(&mut $frame.$field))
    };
}

impl TerminalExecution<'_> {
    fn relocate_suspended_callers(
        &mut self,
        coordinates: &Coordinates<'_, '_>,
        successor: &terminal_psi::TerminalModule,
    ) -> Result<(), String> {
        use crate::engine::execution::SuspendedCallResult;
        use terminal_psi::OperationResult;

        for (caller, &(before, after)) in self.call_stack.iter_mut().zip(coordinates.prefix.stack())
        {
            let old_machine = self
                .program
                .catalogs
                .machine(before.machine)
                .ok_or("continuation lost its caller")?;
            let new_machine = successor
                .machine(after.machine)
                .ok_or("continuation lost its successor caller")?;
            let original = old_machine
                .operation(before.operation)
                .ok_or("continuation lost its call")?;
            let replacement = new_machine
                .operation(after.operation)
                .ok_or("continuation lost its successor call")?;
            match (&mut caller.result, &original.result, &replacement.result) {
                (SuspendedCallResult::Unit, OperationResult::Unit, OperationResult::Unit) => {}
                (
                    SuspendedCallResult::Scalar(id),
                    OperationResult::Scalar(a),
                    OperationResult::Scalar(b),
                ) if *id == a.id && a.id == b.id => {}
                (
                    SuspendedCallResult::Structural {
                        result,
                        expected_reference_backings,
                        preserved_projected_qualifications,
                        profile_completion,
                        ..
                    },
                    OperationResult::Structural(a),
                    OperationResult::Structural(b),
                ) if result == a && profile_completion.is_none() => {
                    for backing in expected_reference_backings.iter_mut().flatten() {
                        backing.relocate_checked_entry(coordinates)?;
                    }
                    *result = b.clone();
                    coordinates
                        .projected(Some(a.structural_type), preserved_projected_qualifications)?;
                }
                _ => {
                    return Err(
                        "continuation cannot transfer this caller's return obligations".into(),
                    );
                }
            }
            if !caller.loan_completions.iter().eq(old_machine
                .loan_completions
                .iter()
                .filter(|row| row.operation == before.operation))
            {
                return Err("continuation cannot authenticate a pending loan completion".into());
            }
            caller.loan_completions = new_machine
                .loan_completions
                .iter()
                .filter(|row| row.operation == after.operation)
                .cloned()
                .collect();
            caller.current_machine = after.machine;
            caller.current = after.block;
            caller.next_operation = after.index + 1;
        }
        Ok(())
    }

    pub(crate) fn relocate_continuation_storage(
        &mut self,
        prefix: &CheckedPrefix<'_>,
        old: &terminal_psi::TerminalModule,
        successor: &terminal_psi::TerminalModule,
    ) -> Result<(), String> {
        if self
            .exposed_address_selectors
            .iter()
            .any(|selector| !selector.unchanged(prefix))
        {
            return Err("continuation changes a selector encoded in an exposed address".into());
        }
        if self.entry_reference_backings.is_some()
            || !self.original_storage_roots.is_empty()
            || !self.original_byte_cells.is_empty()
            || registers!(self, access_view_occurrences).any(|rows| !rows.is_empty())
            || !self.component_lent_host_atoms.is_empty()
            || !self.component_borrowed_addresses.is_empty()
            || !self.established_erased_fields.is_empty()
            || self.effects.iter().any(|effect| {
                !matches!(
                    effect,
                    TerminalEffect::BoundaryCall { .. } | TerminalEffect::ComponentCall { .. }
                )
            })
        {
            return Err("continuation retains an artifact-bound address, authority or fact".into());
        }
        if registers!(self, values)
            .flat_map(|rows| rows.values())
            .any(|value| value.literal().is_none())
            || !self.structural_primitive_storage.has_only_literal_cells()
            || !self.structural_scalar_fields.has_only_literal_cells()
            || self
                .structural_byte_storage
                .values()
                .any(|array| array.has_residents())
            || registers!(self, scalar_array_values)
                .flat_map(|rows| rows.backings())
                .any(|array| array.cells().iter().any(|cell| !cell.is_local_literal()))
        {
            return Err("continuation retains a callable selection, host address or resident scalar backing".into());
        }
        // A host argument can carry additional qualifications. Inspect the
        // tags in the retained values, including handles owned by a loan.
        let mut held = HeldCoordinates {
            prefix,
            frame: self,
            domains: Vec::new(),
            complete_layout: None,
            scalar_fields: Vec::new(),
            roots: Vec::new(),
            projections: Vec::new(),
        };
        for caller in &self.call_stack {
            if let crate::engine::execution::SuspendedCallResult::Structural {
                expected_reference_backings,
                ..
            } = &caller.result
                && expected_reference_backings
                    .iter()
                    .flatten()
                    .any(|backing| !backing.portable_for_continuation(&mut held))
            {
                return Err("continuation cannot transfer a borrowed result's storage".into());
            }
        }
        if registers!(self, structural_values)
            .flat_map(|rows| rows.values())
            .chain(registers!(self, linear_homes).flat_map(|rows| rows.values()))
            .chain(
                self.borrowed_entry_storage
                    .iter()
                    .map(|(_, _, value)| value),
            )
            .any(|value| !held.value(value))
            || registers!(self, retired_structural_homes)
                .flatten()
                .any(|(_, value, reference)| {
                    !held.value(value)
                        || reference
                            .as_ref()
                            .is_some_and(|reference| !held.reference(reference))
                })
            || registers!(self, live_affine_frontier)
                .flatten()
                .any(|discard| !held.ty(discard.structural_type) || !held.path(&discard.path))
            || registers!(self, live_claims)
                .flat_map(|rows| rows.values())
                .any(|claim| !held.path(&claim.path))
        {
            return Err(
                "continuation retains a value or projection outside the checked declarations"
                    .into(),
            );
        }
        if registers!(self, element_view_values).any(|rows| !rows.is_empty())
            || !self.structural_element_view_fields.is_empty()
            || registers!(self, primitive_loan_values)
                .flat_map(|rows| rows.values())
                .any(|loan| !held.loan(loan))
            || self
                .reference_referents
                .values()
                .any(|reference| !held.reference(reference))
            || registers!(self, aggregate_argument_backings)
                .flat_map(|rows| rows.values())
                .any(|backing| {
                    !held.value(&backing.value)
                        || backing
                            .region
                            .is_some_and(|region| !held.ty(region.structural_type))
                        || backing.placed.is_some()
                        || backing.collection_home.is_some()
                        || backing.array_loan.is_some()
                        || backing
                            .byte_region
                            .as_ref()
                            .is_some_and(|region| !held.endpoint(&region.endpoint))
                })
            || registers!(self, byte_sequence_values)
                .flat_map(|rows| rows.values())
                .chain(self.structural_byte_sequence_fields.values())
                .chain(self.structural_byte_descriptors.values())
                .any(|binding| !held.binding(binding))
        {
            return Err("continuation retains a view of storage outside the checked entry".into());
        }
        for (home, case) in &self.structural_cases {
            if !held.typed_place(home, case.structural_type) {
                return Err("continuation changes the declaration of a stored case".into());
            }
        }
        for (home, array) in &self.empty_array_storage.rows {
            if !held.typed_place(home, array.structural_type) {
                return Err("continuation changes the declaration of an empty array".into());
            }
        }
        for (home, array) in &self.structural_byte_storage {
            if !held.typed_place(home, array.structural_type) {
                return Err("continuation changes a retained byte array type".into());
            }
            if let Some(bytes) = array.local_view()
                && !held.bytes(bytes)
            {
                return Err("continuation retains a byte array with an external backing".into());
            }
            if let Some(packed) = array.packed() {
                if !held.ty(packed.structural_type) {
                    return Err("continuation changes a packed byte array type".into());
                }
                held.qualifications(&packed.qualifications);
            }
        }
        for packed in registers!(self, scalar_array_values).flat_map(|rows| rows.backings()) {
            if !held.ty(packed.structural_type) {
                return Err("continuation changes the type of a packed array".into());
            }
            held.qualifications(&packed.qualifications);
        }
        held.roots.sort_unstable();
        held.roots.dedup();
        let projections = std::mem::take(&mut held.projections);
        let scalar_fields = std::mem::take(&mut held.scalar_fields);
        if projections.iter().any(|place| !held.covered_place(place))
            || scalar_fields
                .iter()
                .any(|field| !held.covered_field(&field.parent, field.namespace, field.field))
            || self
                .structural_scalar_fields
                .keys()
                .any(|key| !held.covered_field(&key.parent, key.namespace, key.field))
            || self
                .structural_byte_sequence_fields
                .keys()
                .any(|key| !held.covered_field(&key.parent, key.namespace, key.field))
        {
            return Err("continuation cannot establish the declaration of a retained field".into());
        }
        if self
            .structural_primitive_entry_places
            .values()
            .chain(self.retired_descriptor_homes.iter())
            .chain(self.structural_primitive_storage.keys())
            .chain(self.structural_byte_descriptors.keys())
            .chain(self.structural_byte_storage.keys())
            .chain(self.empty_array_storage.rows.keys())
            .chain(self.structural_cases.keys())
            .chain(self.structural_host_atoms.keys())
            .chain(self.reference_referents.keys())
            .any(|place| !held.covered_place(place))
        {
            return Err("continuation cannot establish the type of a retained storage path".into());
        }
        for domain in held.domains {
            if prefix.structural_domain(domain).is_none() {
                return Err(format!(
                    "continuation cannot establish stored membership {domain:?}"
                ));
            }
        }
        let roots = held.roots;
        let coordinates = Coordinates {
            prefix,
            old,
            roots: &roots,
        };
        coordinates.relocate_frame(self)?;
        self.relocate_suspended_callers(&coordinates, successor)
    }
}

pub(crate) struct HeldCoordinates<'a, 'code, 'artifact> {
    prefix: &'a CheckedPrefix<'code>,
    frame: &'a TerminalExecution<'artifact>,
    domains: Vec<StructuralDomainId>,
    complete_layout: Option<bool>,
    scalar_fields: Vec<crate::boundary::values::StructuralScalarRuntimeField>,
    roots: Vec<(u64, StructuralTypeId)>,
    projections: Vec<StructuralRuntimePlace>,
}

impl HeldCoordinates<'_, '_, '_> {
    // A translated type proves that its nominal declaration and storage layout agree.
    pub(crate) fn ty(&self, ty: StructuralTypeId) -> bool {
        self.prefix.structural_type(ty).is_some()
    }

    fn typed_place(&mut self, home: &StructuralRuntimePlace, ty: StructuralTypeId) -> bool {
        if !self.ty(ty) {
            return false;
        }
        if home.path.is_empty() {
            self.roots.push((home.opaque_identity, ty));
        }
        true
    }

    // A row left by a completed call may outlive the value that identifies its
    // owner. Only that case requires a correspondence for every possible layout.
    fn complete_layout(&mut self) -> bool {
        *self.complete_layout.get_or_insert_with(|| {
            self.frame
                .program
                .catalogs
                .types
                .structural_types
                .iter()
                .all(|row| self.prefix.structural_type(row.id).is_some())
        })
    }

    fn covered_place(&mut self, place: &StructuralRuntimePlace) -> bool {
        self.path(&place.path) && (self.covered_type(place).is_some() || self.complete_layout())
    }

    fn covered_type(
        &self,
        place: &StructuralRuntimePlace,
    ) -> Option<&terminal_psi::StructuralTypeDeclaration> {
        let ty = root_type(&self.roots, place.opaque_identity)?;
        terminal_semantics::runtime_structural_path_tip(
            self.frame.program.catalogs.types.structural_types.iter(),
            ty,
            &place.path,
        )
    }

    fn covered_field(
        &mut self,
        place: &StructuralRuntimePlace,
        namespace: StructuralFieldNamespace,
        id: StructuralFieldId,
    ) -> bool {
        if !self.path(&place.path) {
            return false;
        }
        let Some(parent) = self.covered_type(place) else {
            return self.complete_layout();
        };
        match namespace {
            StructuralFieldNamespace::Member => parent
                .shape
                .direct_fields()
                .is_some_and(|fields| fields.iter().any(|field| field.id == id)),
            StructuralFieldNamespace::CasePayload => parent
                .shape
                .cases()
                .iter()
                .any(|case| case.fields.iter().any(|field| field.id == id)),
        }
    }

    fn qualifications(&mut self, domains: &[StructuralDomainId]) {
        for &domain in domains {
            if let Err(position) = self.domains.binary_search(&domain) {
                self.domains.insert(position, domain);
            }
        }
    }

    pub(crate) fn path(&mut self, path: &[StructuralPathSegment]) -> bool {
        path.iter().all(|segment| match segment {
            StructuralPathSegment::Field(field) => self.prefix.field_identity(*field).is_some(),
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => {
                !matches!(stored_scalar, Some(ScalarType::Callable(_))) && self.ty(*structural_type)
            }
            StructuralPathSegment::RuntimeIndex { .. } => false,
            StructuralPathSegment::AllElements
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::FixedIndex(_)
            | StructuralPathSegment::FixedRange { .. }
            | StructuralPathSegment::Referent => true,
        })
    }

    pub(crate) fn value(&mut self, value: &TerminalStructuralValue) -> bool {
        self.qualifications(&value.qualifications);
        for qualification in &value.projected_qualifications {
            self.qualifications(std::slice::from_ref(&qualification.domain));
            if !self.path(&qualification.path) {
                return false;
            }
        }
        if !self.ty(value.structural_type) || !self.path(&value.path) {
            return false;
        }
        if value.path.is_empty() {
            self.roots
                .push((value.opaque_identity, value.structural_type));
        } else {
            self.projections.push(StructuralRuntimePlace::from(value));
        }
        true
    }

    fn reference(&mut self, reference: &ReferenceReferent) -> bool {
        self.value(&reference.view)
            && reference
                .aggregate_view
                .is_none_or(|region| self.ty(region.structural_type))
            && reference.placed.is_none()
            && reference.collection_loan.is_none()
            && reference
                .primitive_loan
                .as_ref()
                .is_none_or(|loan| self.loan(loan))
            && reference
                .byte_owner
                .as_ref()
                .is_none_or(|binding| self.binding(binding))
            && reference
                .byte_region
                .as_ref()
                .is_none_or(|region| self.endpoint(&region.endpoint))
    }

    fn loan(&mut self, loan: &PrimitiveLoan) -> bool {
        !matches!(loan.scalar_type, ScalarType::Callable(_))
            && loan.storage.has_literal_scalar_encoding()
            && self.value(&loan.referent)
            && self.endpoint(&loan.storage.endpoint)
    }

    pub(crate) fn endpoint(&mut self, endpoint: &PrimitiveStorageEndpoint) -> bool {
        match endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => {
                self.projections.push(place.clone());
                true
            }
            PrimitiveStorageEndpoint::ScalarField(field) => {
                self.scalar_fields.push(field.clone());
                true
            }
            PrimitiveStorageEndpoint::Projection {
                fragments,
                logical,
                stored,
            } => {
                !matches!(logical, ScalarType::Callable(_))
                    && !matches!(stored, ScalarType::Callable(_))
                    && fragments.iter().all(|fragment| {
                        fragment.has_literal_scalar_types()
                            && self.endpoint(&fragment.storage.endpoint)
                    })
            }
            PrimitiveStorageEndpoint::ViewByte { bytes, .. } => self.bytes(bytes),
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                self.frame.retains_entry_array_element(home, *index)
            }
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                self.path(&array.referent.path)
                    && self
                        .frame
                        .stored_byte_array_length(array)
                        .is_ok_and(|length| *index < length)
            }
            PrimitiveStorageEndpoint::RepresentationByte(_) => false,
        }
    }

    pub(crate) fn binding(&mut self, binding: &ByteSequenceBinding) -> bool {
        match binding {
            ByteSequenceBinding::Immutable(bytes) => self.bytes(bytes),
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset,
                length,
                referent,
                descriptor_home,
            } => {
                self.path(&array.referent.path)
                    && self.ty(*array_type)
                    && self.value(referent)
                    && descriptor_home.as_ref().is_none_or(|home| self.value(home))
                    && self
                        .frame
                        .stored_byte_array_length(array)
                        .is_ok_and(|size| {
                            offset
                                .checked_add(*length)
                                .is_some_and(|end| u128::from(end) <= size as u128)
                        })
            }
            ByteSequenceBinding::Representation(_)
            | ByteSequenceBinding::MutableField { .. }
            | ByteSequenceBinding::MutableOwner { .. } => false,
        }
    }

    fn bytes(&mut self, bytes: &crate::storage::byte_sequences::ByteSequenceView) -> bool {
        bytes.original_array().is_none_or(|window| {
            self.path(&window.array.referent.path)
                && self.ty(window.array_type)
                && self
                    .frame
                    .stored_byte_array_length(&window.array)
                    .is_ok_and(|size| {
                        window
                            .offset
                            .checked_add(window.length)
                            .is_some_and(|end| end <= size)
                    })
        }) && bytes.descriptor_home().is_none_or(|home| self.value(home))
    }
}

/// Translates publication-local coordinates only. All allocation identities and
/// payload storage stay owned by the invocation that created them.
pub(crate) struct Coordinates<'a, 'module> {
    prefix: &'a CheckedPrefix<'module>,
    old: &'a terminal_psi::TerminalModule,
    roots: &'a [(u64, StructuralTypeId)],
}

fn root_type(roots: &[(u64, StructuralTypeId)], identity: u64) -> Option<StructuralTypeId> {
    let index = roots.partition_point(|&(candidate, _)| candidate < identity);
    let &(candidate, ty) = roots.get(index)?;
    (candidate == identity
        && roots
            .get(index + 1)
            .is_none_or(|&(next, _)| next != identity))
    .then_some(ty)
}

impl Coordinates<'_, '_> {
    pub(crate) fn machine(
        &self,
        depth: usize,
        old: semantic_vocabulary::MachineId,
    ) -> Result<semantic_vocabulary::MachineId, String> {
        self.prefix
            .stack()
            .get(depth)
            .filter(|(a, _)| a.machine == old)
            .map(|(_, b)| b.machine)
            .ok_or_else(|| "continuation loan refers to another frame".into())
    }
    pub(crate) fn ty(&self, ty: &mut StructuralTypeId) -> Result<(), String> {
        *ty = self
            .prefix
            .structural_type(*ty)
            .ok_or_else(|| format!("continuation cannot translate type {ty:?}"))?;
        Ok(())
    }
    fn memberships(&self, domains: &mut [StructuralDomainId]) -> Result<(), String> {
        for domain in &mut *domains {
            *domain = self
                .prefix
                .structural_domain(*domain)
                .ok_or_else(|| format!("continuation cannot translate membership {domain:?}"))?;
        }
        domains.sort_unstable();
        Ok(())
    }
    pub(crate) fn path(&self, path: &mut [StructuralPathSegment]) -> Result<(), String> {
        self.typed_path(None, path)
    }

    fn typed_path(
        &self,
        root: Option<StructuralTypeId>,
        path: &mut [StructuralPathSegment],
    ) -> Result<(), String> {
        // Walk from the leaf so the prefix used to identify each parent
        // still consists entirely of coordinates in the old publication.
        for index in (0..path.len()).rev() {
            let (part, parent_path) = path[..=index]
                .split_last_mut()
                .expect("selected path segment");
            match part {
                StructuralPathSegment::Field(identity) => {
                    *identity = self.prefix.field_identity(*identity).ok_or_else(|| {
                        format!("continuation cannot translate field {identity:?}")
                    })?
                }
                StructuralPathSegment::RepresentationView {
                    structural_type,
                    stored_scalar,
                } => {
                    if matches!(stored_scalar, Some(ScalarType::Callable(_))) {
                        return Err("continuation retains an artifact-bound callable view".into());
                    }
                    self.ty(structural_type)?;
                }
                StructuralPathSegment::RuntimeIndex { .. } => {
                    return Err("continuation retains an unresolved runtime index".into());
                }
                StructuralPathSegment::Case(case) => {
                    let owner = root.and_then(|ty| self.parent_type(ty, parent_path));
                    *case = match owner {
                        Some(parent) => self.prefix.case_id(parent, *case),
                        None => self.prefix.unambiguous_case_id(*case),
                    }
                    .ok_or_else(|| format!("continuation cannot translate case {case:?}"))?;
                }
                StructuralPathSegment::AllElements
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::FixedRange { .. }
                | StructuralPathSegment::Referent => {}
            }
        }
        Ok(())
    }
    pub(crate) fn value(&self, value: &mut TerminalStructuralValue) -> Result<(), String> {
        let original = value.structural_type;
        self.typed_path(
            root_type(self.roots, value.opaque_identity),
            &mut value.path,
        )?;
        self.ty(&mut value.structural_type)?;
        self.memberships(&mut value.qualifications)?;
        self.projected(Some(original), &mut value.projected_qualifications)
    }
    fn projected(
        &self,
        parent: Option<StructuralTypeId>,
        qualifications: &mut [terminal_psi::StructuralPathQualification],
    ) -> Result<(), String> {
        for row in &mut *qualifications {
            self.typed_path(parent, &mut row.path)?;
            self.memberships(std::slice::from_mut(&mut row.domain))?;
        }
        qualifications.sort_unstable();
        Ok(())
    }

    fn parent_type(
        &self,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        terminal_semantics::runtime_structural_path_tip(
            self.old.types.structural_types.iter(),
            root,
            path,
        )
        .map(|declaration| declaration.id)
    }

    fn place(&self, place: &mut StructuralRuntimePlace) -> Result<(), String> {
        self.typed_path(
            root_type(self.roots, place.opaque_identity),
            &mut place.path,
        )
    }

    fn field(
        &self,
        parent: &StructuralRuntimePlace,
        field: StructuralFieldId,
    ) -> Result<StructuralFieldId, String> {
        let path = crate::boundary::values::structural_storage_path(&parent.path);
        let owner = root_type(self.roots, parent.opaque_identity)
            .and_then(|root| self.parent_type(root, path));
        match owner {
            Some(ty) => self.prefix.field_id(ty, field),
            None => self.prefix.unambiguous_field_id(field),
        }
        .ok_or_else(|| format!("continuation cannot translate field {field:?} in its allocation"))
    }

    fn key(&self, old: &StructuralRuntimePlace) -> Result<Option<StructuralRuntimePlace>, String> {
        if old.path.is_empty() {
            return Ok(None);
        }
        let mut new = old.clone();
        self.place(&mut new)?;
        Ok((new != *old).then_some(new))
    }

    pub(crate) fn array(
        &self,
        array: &mut crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage,
    ) -> Result<(), String> {
        use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorageHome;
        self.place(&mut array.referent)?;
        if let ArrayStorageHome::Frame {
            depth,
            structural_type,
            ..
        } = &mut array.home
        {
            if *depth >= self.prefix.stack().len() {
                return Err("continuation array resides in another call frame".into());
            }
            self.ty(structural_type)?;
        }
        Ok(())
    }

    pub(crate) fn packed(
        &self,
        array: &mut crate::storage::scalar_array::PackedScalarArrayValue,
    ) -> Result<(), String> {
        self.ty(&mut array.structural_type)?;
        self.memberships(&mut array.qualifications)
    }

    pub(crate) fn binding(&self, binding: &mut ByteSequenceBinding) -> Result<(), String> {
        match binding {
            ByteSequenceBinding::Immutable(bytes) => bytes.relocate_checked_entry(self),
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                referent,
                descriptor_home,
                ..
            } => {
                self.array(array)?;
                self.ty(array_type)?;
                self.value(referent)?;
                if let Some(home) = descriptor_home {
                    self.value(home)?;
                }
                Ok(())
            }
            _ => Err("continuation retains an unsupported byte descriptor".into()),
        }
    }

    pub(crate) fn endpoint(&self, endpoint: &mut PrimitiveStorageEndpoint) -> Result<(), String> {
        match endpoint {
            PrimitiveStorageEndpoint::Scalar(place) => self.place(place),
            PrimitiveStorageEndpoint::ScalarField(field) => {
                field.field = self.field(&field.parent, field.field)?;
                self.place(&mut field.parent)
            }
            PrimitiveStorageEndpoint::ArrayElement { array, .. } => self.array(array),
            PrimitiveStorageEndpoint::OwnedArrayElement { home, .. } => {
                home.relocate_checked_entry(self)
            }
            PrimitiveStorageEndpoint::ViewByte { bytes, .. } => bytes.relocate_checked_entry(self),
            PrimitiveStorageEndpoint::Projection { fragments, .. } => {
                for fragment in fragments {
                    self.endpoint(&mut fragment.storage.endpoint)?;
                }
                Ok(())
            }
            PrimitiveStorageEndpoint::RepresentationByte(_) => {
                Err("continuation retains an external representation view".into())
            }
        }
    }

    fn loan(&self, loan: &mut PrimitiveLoan) -> Result<(), String> {
        self.value(&mut loan.referent)?;
        self.endpoint(&mut loan.storage.endpoint)
    }

    fn reference(&self, reference: &mut ReferenceReferent) -> Result<(), String> {
        self.value(&mut reference.view)?;
        if let Some(region) = &mut reference.aggregate_view {
            self.ty(&mut region.structural_type)?;
        }
        if let Some(loan) = &mut reference.primitive_loan {
            self.loan(loan)?;
        }
        if let Some(binding) = &mut reference.byte_owner {
            self.binding(binding)?;
        }
        if let Some(region) = &mut reference.byte_region {
            self.endpoint(&mut region.endpoint)?;
        }
        Ok(())
    }

    fn relocate_frame(&self, frame: &mut TerminalExecution<'_>) -> Result<(), String> {
        for (_, value) in registers!(mut frame, structural_values).flat_map(|rows| rows.iter_mut())
        {
            self.value(value)?;
        }
        for (_, value) in registers!(mut frame, linear_homes).flat_map(|rows| rows.iter_mut()) {
            self.value(value)?;
        }
        for (_, _, value) in &mut frame.borrowed_entry_storage {
            self.value(value)?;
        }
        for (_, value, reference) in registers!(mut frame, retired_structural_homes).flatten() {
            self.value(value)?;
            if let Some(reference) = reference {
                self.reference(reference)?;
            }
        }
        for frontier in registers!(mut frame, live_affine_frontier) {
            for mut discard in std::mem::take(frontier) {
                self.path(&mut discard.path)?;
                self.ty(&mut discard.structural_type)?;
                if !frontier.insert(discard) {
                    return Err("continuation aliases distinct affine obligations".into());
                }
            }
        }
        for claim in registers!(mut frame, live_claims).flat_map(|rows| rows.values_mut()) {
            self.path(&mut claim.path)?;
        }
        for (_, loan) in
            registers!(mut frame, primitive_loan_values).flat_map(|rows| rows.iter_mut())
        {
            self.loan(loan)?;
        }
        for (_, reference) in frame.reference_referents.iter_mut() {
            self.reference(reference)?;
        }
        for (_, backing) in
            registers!(mut frame, aggregate_argument_backings).flat_map(|rows| rows.iter_mut())
        {
            self.value(&mut backing.value)?;
            if let Some(region) = &mut backing.region {
                self.ty(&mut region.structural_type)?;
            }
            if let Some(region) = &mut backing.byte_region {
                self.endpoint(&mut region.endpoint)?;
            }
        }
        for binding in registers!(mut frame, byte_sequence_values)
            .flat_map(|rows| rows.iter_mut().map(|(_, binding)| binding))
            .chain(
                frame
                    .structural_byte_sequence_fields
                    .iter_mut()
                    .map(|(_, binding)| binding),
            )
            .chain(
                frame
                    .structural_byte_descriptors
                    .iter_mut()
                    .map(|(_, binding)| binding),
            )
        {
            self.binding(binding)?;
        }
        for (_, case) in frame.structural_cases.iter_mut() {
            case.case = self
                .prefix
                .case_id(case.structural_type, case.case)
                .ok_or_else(|| {
                    format!("continuation cannot translate stored case {:?}", case.case)
                })?;
            self.ty(&mut case.structural_type)?;
        }
        for (_, array) in frame.empty_array_storage.rows.iter_mut() {
            self.ty(&mut array.structural_type)?;
        }
        for (_, array) in frame.structural_byte_storage.iter_mut() {
            array.relocate_checked_entry(self)?;
        }
        for packed in
            registers!(mut frame, scalar_array_values).flat_map(|rows| rows.backings_mut())
        {
            self.packed(packed)?;
        }

        for place in frame
            .structural_primitive_entry_places
            .iter_mut()
            .map(|(_, place)| place)
            .chain(&mut frame.retired_descriptor_homes)
        {
            self.place(place)?;
        }
        frame.structural_scalar_fields.relocate_keys(|key| {
            let field = self.field(&key.parent, key.field)?;
            let parent = self.key(&key.parent)?;
            if field == key.field && parent.is_none() {
                return Ok(None);
            }
            let mut new = key.relocated(parent.unwrap_or_else(|| key.parent.clone()));
            new.field = field;
            Ok(Some(new))
        })?;
        frame.structural_byte_sequence_fields.relocate_keys(|key| {
            let field = self.field(&key.parent, key.field)?;
            let parent = self.key(&key.parent)?;
            if field == key.field && parent.is_none() {
                return Ok(None);
            }
            let mut new = key.relocated(parent.unwrap_or_else(|| key.parent.clone()));
            new.field = field;
            Ok(Some(new))
        })?;
        frame
            .structural_primitive_storage
            .relocate_keys(|key| self.key(key))?;
        frame
            .structural_byte_descriptors
            .relocate_keys(|key| self.key(key))?;
        frame
            .structural_byte_storage
            .relocate_keys(|key| self.key(key))?;
        frame
            .empty_array_storage
            .rows
            .relocate_keys(|key| self.key(key))?;
        frame.structural_cases.relocate_keys(|key| self.key(key))?;
        frame
            .structural_host_atoms
            .relocate_keys(|key| self.key(key))?;
        frame
            .reference_referents
            .relocate_keys(|key| self.key(key))?;
        Ok(())
    }
}
