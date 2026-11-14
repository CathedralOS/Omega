/*
    Host-supplied structural entry inputs and their pre-execution validation
    and binding into Terminal Psi runtime storage.

    We bind complete structural contents, scalar and boolean fields, fixed byte
    arrays, case memberships, and established access-view referents before
    committing invocation custody.
*/

pub(crate) mod access_views {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::{MachineId, StructuralDomainId, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{StructuralAccess, TerminalAccessViewInput, TerminalModule};

    pub use terminal_installation::TerminalAccessViewEstablishment;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct AccessViewOccurrence {
        pub(crate) ram: Option<crate::storage::reference::AccessReference>,
        pub(crate) view: StructuralRuntimePlace,
        pub(crate) referent: StructuralRuntimePlace,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) qualifications: Vec<StructuralDomainId>,
    }

    const fn exclusive(access: StructuralAccess) -> bool {
        matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())
    }

    fn overlaps(left: &StructuralRuntimePlace, right: &StructuralRuntimePlace) -> bool {
        left.opaque_identity == right.opaque_identity
            && (left.path.starts_with(&right.path) || right.path.starts_with(&left.path))
    }

    fn input_parameter<'module>(
        module: &'module TerminalModule,
        row: &TerminalAccessViewInput,
    ) -> Result<&'module terminal_psi::StructuralParameterDeclaration, TerminalInterpretError> {
        terminal_semantics::access_view_input_parameter(module, row)
            .map_err(TerminalInterpretError::AccessViewInput)
    }

    pub(crate) fn establish_access_view_inputs(
        module: &TerminalModule,
        entry: MachineId,
        establishments: &[TerminalAccessViewEstablishment],
        structural_arguments: &[TerminalStructuralValue],
    ) -> Result<BTreeMap<TerminalAccessViewInput, AccessViewOccurrence>, TerminalInterpretError>
    {
        let entry_rows = module
            .access_view_inputs
            .iter()
            .filter(|row| row.machine == entry)
            .map(|row| input_parameter(module, row).map(|_| row))
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = std::collections::BTreeSet::new();
        for establishment in establishments {
            if !entry_rows.contains(&&establishment.input) {
                return Err(
                    TerminalInterpretError::AccessViewInputEstablishmentUnexpected {
                        machine: establishment.input.machine,
                        position: establishment.input.position,
                    },
                );
            }
            if !seen.insert(&establishment.input) {
                return Err(
                    TerminalInterpretError::AccessViewInputEstablishmentDuplicate {
                        machine: establishment.input.machine,
                        position: establishment.input.position,
                    },
                );
            }
            let referent = &establishment.referent;
            terminal_semantics::validate_access_view_referent(
                module,
                referent.structural_type,
                &referent.path,
                &referent.qualifications,
            )
            .map_err(|error| match error {
                terminal_semantics::AccessViewReferentError::QualificationsNonCanonical => {
                    TerminalInterpretError::StructuralQualificationsNonCanonical
                }
                error => TerminalInterpretError::AccessViewReferent(error),
            })?;
        }
        let mut occurrences: BTreeMap<TerminalAccessViewInput, AccessViewOccurrence> =
            BTreeMap::new();
        for row in entry_rows {
            let Some(establishment) = establishments
                .iter()
                .find(|establishment| establishment.input == *row)
            else {
                return Err(TerminalInterpretError::AccessViewInputsRequireCustody);
            };
            let referent = StructuralRuntimePlace::from(&establishment.referent);
            for (previous_row, previous) in &occurrences {
                if (exclusive(row.access) || exclusive(previous_row.access))
                    && overlaps(&referent, &previous.referent)
                {
                    return Err(
                        TerminalInterpretError::AccessViewInputEstablishmentAliasing(
                            establishment.referent.opaque_identity,
                        ),
                    );
                }
            }
            let parameter = input_parameter(module, row)?;
            let view = structural_arguments
                .get(row.runtime_position as usize)
                .filter(|view| view.structural_type == parameter.structural_type)
                .ok_or_else(malformed)?;
            if occurrences.iter().any(|(previous_row, previous)| {
                previous.view == StructuralRuntimePlace::from(view)
                    && (previous.referent != referent
                        || previous.structural_type != establishment.referent.structural_type
                        || previous.qualifications != establishment.referent.qualifications
                        || previous_row.view_identity != row.view_identity
                        || previous_row.policy_identity != row.policy_identity
                        || previous_row.policy_plan_machine_identity
                            != row.policy_plan_machine_identity
                        || previous_row.schema_identity != row.schema_identity
                        || previous_row.placement_commitment != row.placement_commitment
                        || previous_row.placement_report_fingerprint
                            != row.placement_report_fingerprint)
            }) {
                return Err(
                    TerminalInterpretError::AccessViewInputEstablishmentAliasing(
                        view.opaque_identity,
                    ),
                );
            }
            if structural_arguments
                .iter()
                .zip(
                    &module
                        .machine(entry)
                        .ok_or_else(malformed)?
                        .structural_parameters,
                )
                .enumerate()
                .filter(|(position, _)| *position != row.runtime_position as usize)
                .any(|(_, (argument, formal))| {
                    (exclusive(row.access)
                        || exclusive(formal.access)
                        || formal.access == StructuralAccess::Owned)
                        && overlaps(&referent, &StructuralRuntimePlace::from(argument))
                })
            {
                return Err(
                    TerminalInterpretError::AccessViewInputEstablishmentAliasing(
                        establishment.referent.opaque_identity,
                    ),
                );
            }
            if occurrences
                .insert(
                    row.clone(),
                    AccessViewOccurrence {
                        ram: None,
                        view: StructuralRuntimePlace::from(view),
                        referent,
                        structural_type: establishment.referent.structural_type,
                        qualifications: establishment.referent.qualifications.clone(),
                    },
                )
                .is_some()
            {
                return Err(malformed());
            }
        }
        Ok(occurrences)
    }

    impl crate::engine::execution::TerminalExecution<'_> {
        pub(crate) fn bind_call_access_views(
            &self,
            callee: MachineId,
            arguments: &[terminal_psi::StructuralArgument],
            values: &RuntimeRows<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
        ) -> Result<BTreeMap<TerminalAccessViewInput, AccessViewOccurrence>, TerminalInterpretError>
        {
            let module = &self.program.catalogs;
            let mut bound: BTreeMap<TerminalAccessViewInput, AccessViewOccurrence> =
                BTreeMap::new();
            for row in module
                .access_view_inputs
                .iter()
                .filter(|row| row.machine == callee)
            {
                let parameter = input_parameter(module, row)?;
                let argument = arguments
                    .get(row.runtime_position as usize)
                    .ok_or_else(malformed)?;
                let (resolved, ram) = self.resolve_live_access_projection(argument)?;
                let presented = values.get(&parameter.place).ok_or_else(malformed)?;
                if argument.access != row.access
                    || presented.structural_type != parameter.structural_type
                {
                    return Err(malformed());
                }
                let occurrence = if let Some(mut ram) = ram {
                    self.validate_access_backing(&ram)?;
                    let plan = module
                        .access_view_plans
                        .iter()
                        .find(|plan| plan.carrier == ram.carrier())
                        .ok_or_else(malformed)?;
                    if ram.selected_field.is_some()
                        || ram.presentation != resolved
                        || ram.carrier() != parameter.structural_type
                        || !crate::storage::reference::access_covers(ram.access, row.access)
                        || (
                            row.view_identity,
                            row.policy_identity,
                            row.policy_plan_machine_identity,
                            row.schema_identity,
                            row.placement_commitment,
                            row.placement_report_fingerprint,
                        ) != (
                            plan.view_identity,
                            plan.policy_identity,
                            plan.policy_plan_machine_identity,
                            plan.schema_identity,
                            plan.placement.content_interpretation().commitment(),
                            plan.placement.identity().compatibility_fingerprint(),
                        )
                    {
                        return Err(malformed());
                    }
                    ram.access = row.access;
                    ram.presentation = presented.clone();
                    AccessViewOccurrence {
                        view: StructuralRuntimePlace::from(presented),
                        referent: StructuralRuntimePlace::from(&ram.source),
                        structural_type: ram.source.structural_type,
                        qualifications: ram.source.qualifications.clone(),
                        ram: Some(ram),
                    }
                } else {
                    let source = self
                        .access_view_occurrences
                        .iter()
                        .find(|(source, occurrence)| {
                            occurrence.view == StructuralRuntimePlace::from(&resolved)
                                && source.view_identity == row.view_identity
                                && source.policy_identity == row.policy_identity
                                && source.policy_plan_machine_identity
                                    == row.policy_plan_machine_identity
                                && source.schema_identity == row.schema_identity
                                && source.placement_commitment == row.placement_commitment
                                && source.placement_report_fingerprint
                                    == row.placement_report_fingerprint
                                && match (source.access, row.access) {
                                    (
                                        StructuralAccess::MutableBorrow,
                                        StructuralAccess::SharedBorrow
                                        | StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow,
                                    ) => true,
                                    (left, right) => left == right,
                                }
                        })
                        .ok_or(TerminalInterpretError::AccessViewInputsRequireCustody)?;
                    let mut occurrence = source.1.clone();
                    occurrence.view = StructuralRuntimePlace::from(presented);
                    if let Some(ram) = &mut occurrence.ram {
                        self.validate_access_backing(ram)?;
                        if !crate::storage::reference::access_covers(ram.access, row.access) {
                            return Err(malformed());
                        }
                        ram.access = row.access;
                        ram.presentation = presented.clone();
                    }
                    occurrence
                };
                for (other_row, other) in &bound {
                    if (exclusive(row.access) || exclusive(other_row.access))
                        && overlaps(&occurrence.referent, &other.referent)
                    {
                        return Err(
                            TerminalInterpretError::AccessViewInputEstablishmentAliasing(
                                occurrence.referent.opaque_identity,
                            ),
                        );
                    }
                }
                if values.iter().any(|(place, value)| {
                    *place != parameter.place
                        && module.machine(callee).is_some_and(|machine| {
                            machine
                                .structural_parameters
                                .iter()
                                .find(|formal| formal.place == *place)
                                .is_some_and(|formal| {
                                    (exclusive(row.access)
                                        || exclusive(formal.access)
                                        || formal.access == StructuralAccess::Owned)
                                        && overlaps(
                                            &occurrence.referent,
                                            &StructuralRuntimePlace::from(value),
                                        )
                                })
                        })
                }) {
                    return Err(
                        TerminalInterpretError::AccessViewInputEstablishmentAliasing(
                            occurrence.referent.opaque_identity,
                        ),
                    );
                }
                bound.insert(row.clone(), occurrence);
            }
            Ok(bound)
        }
    }
}
pub(crate) mod byte_arrays {
    pub(crate) mod storage {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{StructuralRuntimePlace, TerminalScalarValue};
        use crate::engine::custody::fixed_index_step;
        use crate::engine::execution::TerminalExecution;
        use crate::engine::execution::frame_field_mut;
        use crate::storage::byte_sequences::ByteSequenceView;
        use crate::storage::byte_sequences::storage::ByteSequenceStorage;
        use crate::storage::scalar_array::PackedScalarArrayValue;
        use crate::storage::scalar_storage::{ScalarCell, TerminalScalarBacking};
        use evaluation_accounting::EvaluationInvocationAccount;
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralTypeId,
        };
        use std::rc::Rc;

        #[derive(Clone)]
        pub(crate) struct StructuralByteArrayStorage {
            pub(crate) structural_type: StructuralTypeId,
            backing: StructuralByteArrayBacking,
            account: Option<EvaluationInvocationAccount>,
        }

        #[derive(Clone)]
        enum StructuralByteArrayBacking {
            Local(ByteSequenceView),
            Cells(PackedScalarArrayValue),
        }

        pub(crate) struct PreparedStructuralByteArrayReplacement {
            retained: StructuralByteArrayStorage,
            values: ByteSequenceView,
        }

        impl PreparedStructuralByteArrayReplacement {
            pub(crate) fn commit(
                mut self,
            ) -> Result<StructuralByteArrayStorage, TerminalInterpretError> {
                for (index, byte) in self.values.bytes().iter().enumerate() {
                    self.retained.replace_byte(index, *byte)?;
                }
                Ok(self.retained)
            }
        }

        impl StructuralByteArrayStorage {
            pub(crate) fn new(
                structural_type: StructuralTypeId,
                bytes: ByteSequenceView,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Self {
                Self {
                    structural_type,
                    backing: StructuralByteArrayBacking::Local(bytes),
                    account: account.cloned(),
                }
            }

            pub(crate) fn len(&self) -> usize {
                match &self.backing {
                    StructuralByteArrayBacking::Local(bytes) => bytes.len(),
                    StructuralByteArrayBacking::Cells(cells) => cells.len(),
                }
            }

            pub(crate) fn local_view(&self) -> Option<&ByteSequenceView> {
                match &self.backing {
                    StructuralByteArrayBacking::Local(bytes) => Some(bytes),
                    _ => None,
                }
            }

            pub(crate) fn local_view_mut(&mut self) -> Option<&mut ByteSequenceView> {
                match &mut self.backing {
                    StructuralByteArrayBacking::Local(bytes) => Some(bytes),
                    _ => None,
                }
            }

            pub(crate) fn packed(&self) -> Option<&PackedScalarArrayValue> {
                match &self.backing {
                    StructuralByteArrayBacking::Cells(cells) => Some(cells),
                    _ => None,
                }
            }

            pub(crate) fn measure_result_custody<'storage>(
                &'storage self,
                custody: &mut crate::boundary::result_custody::ResultCustody<'storage>,
            ) -> Result<(), TerminalInterpretError> {
                match &self.backing {
                    StructuralByteArrayBacking::Local(bytes) => custody.bytes(bytes),
                    StructuralByteArrayBacking::Cells(cells) => custody.add_cells(cells.len()),
                }
            }

            pub(crate) fn has_residents(&self) -> bool {
                self.packed().is_some_and(|cells| {
                    cells
                        .cells()
                        .iter()
                        .any(|cell| matches!(cell, ScalarCell::Resident { .. }))
                })
            }

            pub(crate) fn prepare_replacement(
                &self,
                incoming: &Self,
            ) -> Result<Option<PreparedStructuralByteArrayReplacement>, TerminalInterpretError>
            {
                if !self.has_residents() {
                    return Ok(None);
                }
                if self.structural_type != incoming.structural_type || self.len() != incoming.len()
                {
                    return Err(malformed());
                }
                let (index, resident) = self
                    .packed()
                    .ok_or_else(malformed)?
                    .cells()
                    .iter()
                    .enumerate()
                    .find(|(_, cell)| matches!(cell, ScalarCell::Resident { .. }))
                    .ok_or_else(malformed)?;
                let resident = resident.clone();
                let mut retained = self.clone();
                let account = retained.account.clone();
                retained.prepare_resident_install(index, &resident, account.as_ref())?;
                Ok(Some(PreparedStructuralByteArrayReplacement {
                    retained,
                    values: incoming.copy_snapshot()?,
                }))
            }

            pub(crate) fn read_view(
                &self,
            ) -> Result<crate::TerminalByteRead<'_>, TerminalInterpretError> {
                self.read_window(0, self.len())
            }

            pub(crate) fn read_window(
                &self,
                start: usize,
                end: usize,
            ) -> Result<crate::TerminalByteRead<'_>, TerminalInterpretError> {
                match &self.backing {
                    StructuralByteArrayBacking::Local(bytes) => {
                        Ok(crate::TerminalByteRead::contiguous(
                            bytes.bytes().get(start..end).ok_or_else(malformed)?,
                        ))
                    }
                    StructuralByteArrayBacking::Cells(cells) => crate::TerminalByteRead::cells(
                        cells.cells().get(start..end).ok_or_else(malformed)?,
                    ),
                }
            }

            pub(crate) fn read(&self, index: usize) -> Result<u8, TerminalInterpretError> {
                match &self.backing {
                    StructuralByteArrayBacking::Local(bytes) => {
                        bytes.get(index).copied().ok_or_else(malformed)
                    }
                    StructuralByteArrayBacking::Cells(cells) => {
                        scalar_byte(&cells.element(index)?.ok_or_else(malformed)?)
                    }
                }
            }

            pub(crate) fn copy_snapshot(&self) -> Result<ByteSequenceView, TerminalInterpretError> {
                Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
                    self.len(),
                    (0..self.len()).map(|index| self.read(index)),
                    self.account.as_ref(),
                )?))
            }

            pub(crate) fn relocated(&self, transfer: bool) -> Result<Self, TerminalInterpretError> {
                if transfer {
                    return Ok(self.clone());
                }
                Ok(Self::new(
                    self.structural_type,
                    self.copy_snapshot()?,
                    self.account.as_ref(),
                ))
            }

            pub(crate) fn replace_byte(
                &mut self,
                index: usize,
                byte: u8,
            ) -> Result<(), TerminalInterpretError> {
                match &mut self.backing {
                    StructuralByteArrayBacking::Local(bytes) => bytes.replace_byte(index, byte),
                    StructuralByteArrayBacking::Cells(cells) => {
                        cells.set_element(index, byte_scalar(byte)?)
                    }
                }
            }

            pub(crate) fn capture_scalar(
                &self,
                index: usize,
            ) -> Result<
                (TerminalScalarValue, Option<Rc<dyn TerminalScalarBacking>>),
                TerminalInterpretError,
            > {
                match &self.backing {
                    StructuralByteArrayBacking::Local(bytes) => {
                        Ok((byte_scalar(*bytes.get(index).ok_or_else(malformed)?)?, None))
                    }
                    StructuralByteArrayBacking::Cells(cells) => {
                        let cell = cells.cell(index).ok_or_else(malformed)?;
                        Ok((
                            cell.read()?,
                            match cell {
                                ScalarCell::Local(_) => None,
                                ScalarCell::Resident { backing, .. } => Some(backing.clone()),
                            },
                        ))
                    }
                }
            }

            pub(crate) fn prepare_resident_install(
                &mut self,
                index: usize,
                resident: &ScalarCell,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<(), TerminalInterpretError> {
                match &mut self.backing {
                    StructuralByteArrayBacking::Local(bytes) => {
                        let mut cells = PackedScalarArrayValue::from_bytes(
                            self.structural_type,
                            bytes.bytes(),
                            account,
                        )?;
                        cells.prepare_resident_install(index, resident)?;
                        self.backing = StructuralByteArrayBacking::Cells(cells);
                        self.account = account.cloned();
                        Ok(())
                    }
                    StructuralByteArrayBacking::Cells(cells) => {
                        cells.prepare_resident_install(index, resident)
                    }
                }
            }

            pub(crate) fn install_resident(
                &mut self,
                index: usize,
                resident: ScalarCell,
            ) -> Result<(), TerminalInterpretError> {
                let StructuralByteArrayBacking::Cells(cells) = &mut self.backing else {
                    return Err(malformed());
                };
                cells.install_resident(index, resident)
            }
        }

        fn byte_scalar(byte: u8) -> Result<TerminalScalarValue, TerminalInterpretError> {
            Ok(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                value: IntegerValue::Unsigned(u128::from(byte)),
            })
        }

        #[derive(Clone, PartialEq, Eq)]
        pub(crate) enum ArrayStorageHome {
            Runtime,
            Frame {
                depth: usize,
                place: PlaceId,
                structural_type: StructuralTypeId,
            },
        }

        #[derive(Clone, PartialEq, Eq)]
        pub(crate) struct ArrayStorage {
            pub(crate) referent: StructuralRuntimePlace,
            pub(crate) home: ArrayStorageHome,
        }

        impl From<StructuralRuntimePlace> for ArrayStorage {
            fn from(referent: StructuralRuntimePlace) -> Self {
                Self {
                    referent,
                    home: ArrayStorageHome::Runtime,
                }
            }
        }

        impl TerminalExecution<'_> {
            pub(crate) fn stored_byte_array_read_view(
                &self,
                storage: &ArrayStorage,
                start: usize,
                end: usize,
            ) -> Result<
                crate::storage::byte_sequences::read::TerminalByteRead<'_>,
                TerminalInterpretError,
            > {
                use crate::storage::byte_sequences::read::TerminalByteRead;
                match storage.home {
                    ArrayStorageHome::Runtime => {
                        let Some(bytes) = self.structural_byte_storage.get(&storage.referent)
                        else {
                            if start == 0
                                && end == 0
                                && self.initialized_empty_byte_array(&storage.referent)?
                            {
                                return Ok(TerminalByteRead::contiguous(&[]));
                            }
                            return Err(malformed());
                        };
                        bytes.read_window(start, end)
                    }
                    ArrayStorageHome::Frame { depth, place, .. } => {
                        let (base, limit) = self.frame_byte_array_window(storage)?;
                        if start > end || end > limit - base {
                            return Err(malformed());
                        }
                        let values = self.frame_array(depth, place).ok_or_else(malformed)?;
                        TerminalByteRead::cells(&values.cells()[base + start..base + end])
                    }
                }
            }

            pub(crate) fn frame_array(
                &self,
                depth: usize,
                place: PlaceId,
            ) -> Option<&PackedScalarArrayValue> {
                if depth == self.call_stack.len() {
                    self.scalar_array_values.backing(&place)
                } else {
                    self.call_stack
                        .get(depth)?
                        .scalar_array_values
                        .backing(&place)
                }
            }

            pub(crate) fn frame_byte_array_window(
                &self,
                storage: &ArrayStorage,
            ) -> Result<(usize, usize), TerminalInterpretError> {
                let ArrayStorageHome::Frame {
                    depth,
                    place,
                    structural_type,
                } = storage.home
                else {
                    return Err(malformed());
                };
                let array = self
                    .frame_array(depth, place)
                    .ok_or_else(|| malformed().in_reference_formation("frame byte array home"))?;
                let types = self.structural_types();
                let (scalar, count) =
                    terminal_semantics::scalar_array_leaf_shape(types.values(), structural_type)
                        .ok_or_else(|| {
                            malformed().in_reference_formation("frame byte array scalar shape")
                        })?;
                if array.structural_type != structural_type
                    || u64::try_from(array.len()).ok() != Some(count)
                    || !matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.bits() == Some(8) && !integer.is_address() && integer.sign() == IntegerSign::Unsigned)
                {
                    return Err(
                        malformed().in_reference_formation("frame byte array type or extent")
                    );
                }
                let mut selected = structural_type;
                let mut offset = 0usize;
                for segment in &storage.referent.path {
                    let (element, index, _) =
                        fixed_index_step(&types, selected, segment).map_err(|error| {
                            error.in_reference_formation("frame byte array projection")
                        })?;
                    let child_count = match types.declaration(&element)?.shape {
                        terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => 1,
                        _ => {
                            terminal_semantics::scalar_array_leaf_shape(types.values(), element)
                                .ok_or_else(malformed)?
                                .1
                        }
                    };
                    offset = offset
                        .checked_add(
                            usize::try_from(index.checked_mul(child_count).ok_or_else(malformed)?)
                                .map_err(|_| malformed())?,
                        )
                        .ok_or_else(malformed)?;
                    selected = element;
                }
                let length =
                    usize::try_from(super::byte_array_length(&types, selected).ok_or_else(
                        || malformed().in_reference_formation("frame byte array selected shape"),
                    )?)
                    .map_err(|_| malformed())?;
                let end = offset
                    .checked_add(length)
                    .filter(|end| *end <= array.len())
                    .ok_or_else(malformed)?;
                Ok((offset, end))
            }

            pub(crate) fn stored_byte_array_length(
                &self,
                storage: &ArrayStorage,
            ) -> Result<usize, TerminalInterpretError> {
                match storage.home {
                    ArrayStorageHome::Runtime => {
                        if let Some(bytes) = self.structural_byte_storage.get(&storage.referent) {
                            Ok(bytes.len())
                        } else if self.initialized_empty_byte_array(&storage.referent)? {
                            Ok(0)
                        } else {
                            Err(malformed()
                                .in_reference_formation("missing runtime byte array storage"))
                        }
                    }
                    ArrayStorageHome::Frame { .. } => self
                        .frame_byte_array_window(storage)
                        .map(|(start, end)| end - start),
                }
            }

            pub(crate) fn stored_byte_array_read(
                &self,
                storage: &ArrayStorage,
                index: usize,
            ) -> Result<u8, TerminalInterpretError> {
                match storage.home {
                    ArrayStorageHome::Runtime => self
                        .structural_byte_storage
                        .require(&storage.referent)?
                        .read(index),
                    ArrayStorageHome::Frame { depth, place, .. } => {
                        let (start, end) = self.frame_byte_array_window(storage)?;
                        let absolute = start
                            .checked_add(index)
                            .filter(|index| *index < end)
                            .ok_or_else(malformed)?;
                        let value = self
                            .frame_array(depth, place)
                            .ok_or_else(malformed)?
                            .element(absolute)?
                            .ok_or_else(malformed)?;
                        scalar_byte(&value)
                    }
                }
            }

            pub(crate) fn stored_byte_array_write(
                &mut self,
                storage: &ArrayStorage,
                index: usize,
                byte: u8,
            ) -> Result<(), TerminalInterpretError> {
                let result = match storage.home {
                    ArrayStorageHome::Runtime => self
                        .structural_byte_storage
                        .get_mut(&storage.referent)
                        .ok_or_else(malformed)?
                        .replace_byte(index, byte),
                    ArrayStorageHome::Frame { depth, place, .. } => {
                        let (start, end) = self.frame_byte_array_window(storage)?;
                        let absolute = start
                            .checked_add(index)
                            .filter(|index| *index < end)
                            .ok_or_else(malformed)?;
                        let array = frame_field_mut!(self, depth, scalar_array_values)
                            .backing_mut(&place)
                            .ok_or_else(malformed)?;
                        let scalar_type =
                            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                        array.set_element(
                            absolute,
                            TerminalScalarValue::Integer {
                                scalar_type,
                                value: IntegerValue::Unsigned(u128::from(byte)),
                            },
                        )?;
                        Ok(())
                    }
                };
                if result.is_ok() {
                    self.invalidate_stored_provenance(&storage.referent);
                }
                result
            }

            pub(crate) fn stored_byte_array_snapshot(
                &self,
                storage: &ArrayStorage,
                start: usize,
                end: usize,
            ) -> Result<ByteSequenceView, TerminalInterpretError> {
                if let ArrayStorageHome::Runtime = storage.home {
                    if !self.structural_byte_storage.contains_key(&storage.referent)
                        && start == 0
                        && end == 0
                        && self.initialized_empty_byte_array(&storage.referent)?
                    {
                        return Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
                            0,
                            std::iter::empty(),
                            self.account.as_ref(),
                        )?));
                    }
                    let array = self.structural_byte_storage.require(&storage.referent)?;
                    if start > end || end > array.len() {
                        return Err(malformed());
                    }
                    return Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
                        end.checked_sub(start).ok_or_else(malformed)?,
                        (start..end).map(|index| array.read(index)),
                        self.account.as_ref(),
                    )?));
                }
                if start > end || end > self.stored_byte_array_length(storage)? {
                    return Err(malformed());
                }
                let ArrayStorageHome::Frame { depth, place, .. } = storage.home else {
                    return Err(malformed());
                };
                let (base, _) = self.frame_byte_array_window(storage)?;
                let values = self
                    .frame_array(depth, place)
                    .and_then(|array| array.cells().get(base + start..base + end))
                    .ok_or_else(malformed)?
                    .iter()
                    .map(|cell| scalar_byte(&cell.read()?));
                Ok(ByteSequenceView::new(ByteSequenceStorage::from_values(
                    end - start,
                    values,
                    self.account.as_ref(),
                )?))
            }

            pub(crate) fn initialized_empty_byte_array(
                &self,
                home: &StructuralRuntimePlace,
            ) -> Result<bool, TerminalInterpretError> {
                self.empty_array_storage.initialized_byte_array(
                    &self.structural_types(),
                    &self.structural_byte_storage,
                    home,
                )
            }
        }

        pub(crate) fn scalar_byte(
            value: &TerminalScalarValue,
        ) -> Result<u8, TerminalInterpretError> {
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(byte),
            } = value
            else {
                return Err(malformed());
            };
            if scalar_type.sign() != IntegerSign::Unsigned
                || scalar_type.bits() != Some(8)
                || scalar_type.is_address()
            {
                return Err(malformed());
            }
            u8::try_from(*byte).map_err(|_| malformed())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::boundary::effects::TerminalEffectRejection;
            use std::cell::Cell;

            #[derive(Debug)]
            struct OriginalByte(Cell<TerminalScalarValue>);
            impl TerminalScalarBacking for OriginalByte {
                fn read(&self) -> Result<TerminalScalarValue, TerminalEffectRejection> {
                    Ok(self.0.get())
                }
                fn write(&self, value: TerminalScalarValue) -> Result<(), TerminalEffectRejection> {
                    self.0.set(value);
                    Ok(())
                }
            }

            #[test]
            fn native_promotion_and_whole_replacement_preserve_cells_but_guest_copies_do_not_alias()
            {
                let array_type = StructuralTypeId::new(1).unwrap();
                let make = |values: &[u8]| {
                    StructuralByteArrayStorage::new(
                        array_type,
                        ByteSequenceView::new(ByteSequenceStorage::copy(values, None).unwrap()),
                        None,
                    )
                };
                let mut owner = make(&[7, 23]);
                let original = Rc::new(OriginalByte(Cell::new(byte_scalar(7).unwrap())));
                let backing: Rc<dyn TerminalScalarBacking> = original.clone();
                let resident =
                    ScalarCell::resident(byte_scalar(7).unwrap(), backing.clone()).unwrap();
                owner.prepare_resident_install(0, &resident, None).unwrap();
                assert!(
                    owner.capture_scalar(0).unwrap().1.is_none(),
                    "preflight grants no resident yet"
                );
                owner.install_resident(0, resident).unwrap();
                let snapshot = owner.copy_snapshot().unwrap();
                let incoming = make(&[31, 41]);
                let prepared = owner.prepare_replacement(&incoming).unwrap().unwrap();
                assert_eq!(
                    original.0.get(),
                    byte_scalar(7).unwrap(),
                    "staging cannot publish writes"
                );
                let replaced = prepared.commit().unwrap();
                assert!(Rc::ptr_eq(
                    &replaced.capture_scalar(0).unwrap().1.unwrap(),
                    &backing
                ));
                assert_eq!(original.0.get(), byte_scalar(31).unwrap());
                assert_eq!(replaced.read(0).unwrap(), 31);
                assert_eq!(replaced.read(1).unwrap(), 41);
                assert_eq!(owner.read(0).unwrap(), 31);
                assert_eq!(owner.read(1).unwrap(), 23);
                assert_eq!(snapshot.bytes(), &[7, 23]);
                original.0.set(byte_scalar(47).unwrap());
                assert_eq!(replaced.read_view().unwrap().read(0).unwrap(), 47);
                assert_eq!(owner.read(0).unwrap(), 47);
                assert_eq!(snapshot.bytes(), &[7, 23]);
            }
        }
    }

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::custody::resolve_single_structural_argument;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::ByteSequenceView;
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use crate::storage::byte_sequences::storage::ByteSequenceStorage;
    use crate::storage::element_views::CollectionStorage;
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::ScalarType;
    use semantic_vocabulary::StructuralTypeId;
    use storage::{ArrayStorage, ArrayStorageHome};
    use terminal_psi::StructuralAccess;
    use terminal_psi::StructuralArgument;
    use terminal_psi::StructuralParameterDeclaration;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralTypeDeclaration;
    use terminal_psi::StructuralTypeShape;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalStructuralByteArrayValue {
        pub argument_index: u32,
        pub path: Vec<StructuralPathSegment>,
        pub bytes: Vec<u8>,
    }

    pub(crate) fn byte_array_length(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        array: StructuralTypeId,
    ) -> Option<u64> {
        if let StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedView {
            capacity,
            ..
        }) = types.get(&array)?.shape
        {
            return Some(capacity);
        }
        let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = types.get(&array)?.shape
        else {
            return None;
        };
        let StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(element)) =
            types.get(&element)?.shape
        else {
            return None;
        };
        (!element.is_address()
            && element.sign() == semantic_vocabulary::IntegerSign::Unsigned
            && element.bits() == Some(8))
        .then_some(length)
    }

    pub(crate) fn array_path_type(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        terminal_semantics::runtime_structural_path_tip(types.values(), structural_type, path)
            .map(|declaration| declaration.id)
    }

    impl TerminalExecution<'_> {
        fn check_byte_view_handback(
            &self,
            value: &TerminalStructuralValue,
            argument: &StructuralArgument,
            parameter: &StructuralParameterDeclaration,
        ) -> Result<(), TerminalInterpretError> {
            if !value.projected_qualifications.is_empty() {
                return Err(malformed());
            }
            let domains = self.program.catalogs.structural_domains.iter();
            let original = terminal_semantics::byte_qualification_predicates_in(
                domains.clone(),
                value.structural_type,
                &value.qualifications,
            )
            .ok_or_else(malformed)?;
            let required = terminal_semantics::byte_parameter_predicates_in(domains, parameter)
                .ok_or_else(malformed)?;
            if !original.implies(required)
                || (argument.access == StructuralAccess::MutableBorrow
                    && !required.implies(original))
            {
                return Err(malformed());
            }
            Ok(())
        }

        pub(crate) fn bind_byte_arrays(
            &mut self,
            arguments: &[TerminalStructuralByteArrayValue],
        ) -> Result<(), TerminalInterpretError> {
            let machine = self.program.machine(self.current_machine)?;
            let mut storage = RuntimeRows::new(self.account.as_ref());
            let mut descriptors = RuntimeRows::new(self.account.as_ref());
            for argument in arguments {
                if !argument.path.iter().all(|segment| {
                    matches!(
                        segment,
                        StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                    )
                }) {
                    return Err(malformed());
                }
                let parameter = machine
                    .structural_parameters
                    .get(argument.argument_index as usize)
                    .ok_or_else(malformed)?;
                if !matches!(
                    parameter.access,
                    StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                ) || !matches!(
                    parameter.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                ) || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || crate::engine::custody::place_is_claimed(&self.live_claims, parameter.place)
                {
                    return Err(malformed());
                }
                let root = self.structural_value(parameter.place)?;
                if !root.qualifications.is_empty()
                    || root.structural_type != parameter.structural_type
                {
                    return Err(malformed());
                }
                let array_type = array_path_type(
                    &self.structural_types(),
                    root.structural_type,
                    &argument.path,
                )
                .ok_or_else(malformed)?;
                let length = byte_array_length(&self.structural_types(), array_type)
                    .ok_or_else(malformed)?;
                if argument.bytes.len() as u128 != u128::from(length) {
                    return Err(malformed());
                }
                let mut array = StructuralRuntimePlace::from(root);
                array.path.extend_from_slice(&argument.path);
                let bytes = ByteSequenceView::new(ByteSequenceStorage::copy(
                    &argument.bytes,
                    self.account.as_ref(),
                )?);
                if argument.path.is_empty()
                    && matches!(
                        self.structural_types()
                            .get(&array_type)
                            .map(|row| &row.shape),
                        Some(StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                        ))
                    )
                    && matches!(
                        parameter.access,
                        terminal_psi::reading_borrow_pair_pattern!()
                    )
                {
                    let binding = if parameter.access == StructuralAccess::MutableBorrow {
                        ByteSequenceBinding::MutableArray {
                            array: array.clone().into(),
                            array_type,
                            offset: 0,
                            length,
                            referent: root.clone(),
                            descriptor_home: None,
                        }
                    } else {
                        ByteSequenceBinding::Immutable(bytes.clone())
                    };
                    if self.byte_sequence_values.contains_key(&parameter.place)
                        || descriptors.insert(parameter.place, binding)?.is_some()
                    {
                        return Err(malformed());
                    }
                }
                if self.structural_byte_storage.contains_key(&array)
                    || storage
                        .insert(
                            array,
                            storage::StructuralByteArrayStorage::new(
                                array_type,
                                bytes,
                                self.account.as_ref(),
                            ),
                        )?
                        .is_some()
                {
                    return Err(malformed());
                }
            }
            self.structural_byte_storage.validate_append(&storage)?;
            self.byte_sequence_values.validate_append(&descriptors)?;
            self.structural_byte_storage.append(&mut storage)?;
            self.byte_sequence_values.append(&mut descriptors)?;
            Ok(())
        }

        pub(crate) fn array_storage_root_staged(
            &self,
            place: semantic_vocabulary::PlaceId,
            identity_cursor: &mut Option<u64>,
            local_array_roots: &mut RuntimeRows<
                semantic_vocabulary::PlaceId,
                TerminalStructuralValue,
            >,
        ) -> Result<(TerminalStructuralValue, ArrayStorageHome), TerminalInterpretError> {
            let root = if let Some(array) = self.scalar_array_values.get(&place) {
                if self.structural_values.contains_key(&place) {
                    return Err(malformed());
                }
                let root = match local_array_roots.get(&place) {
                    Some(root) => root.clone(),
                    None => {
                        let root = TerminalStructuralValue::minted(
                            self.local_structural_identities
                                .allocate_staged(identity_cursor)?,
                            array.structural_type,
                            array.qualifications.clone(),
                        );
                        local_array_roots.insert(place, root.clone())?;
                        root
                    }
                };
                (
                    root,
                    ArrayStorageHome::Frame {
                        depth: self.call_stack.len(),
                        place,
                        structural_type: array.structural_type,
                    },
                )
            } else if let Some(home) = self.loaned_byte_array_frame(place) {
                (self.structural_value(place)?.clone(), home)
            } else {
                (
                    self.structural_value(place)?.clone(),
                    ArrayStorageHome::Runtime,
                )
            };
            Ok(root)
        }

        fn byte_array_storage_staged(
            &self,
            source: &StructuralArgument,
            identity_cursor: &mut Option<u64>,
            local_array_roots: &mut RuntimeRows<
                semantic_vocabulary::PlaceId,
                TerminalStructuralValue,
            >,
        ) -> Result<(TerminalStructuralValue, ArrayStorage), TerminalInterpretError> {
            let CollectionStorage {
                collection: selected,
                home,
                ..
            } = self.element_collection_storage(source)?;
            let referent = match selected {
                Some(selected) => selected,
                None => {
                    let (root, _) = self.array_storage_root_staged(
                        source.place,
                        identity_cursor,
                        local_array_roots,
                    )?;
                    let values =
                        RuntimeRows::from_rows(self.account.as_ref(), [(source.place, root)])?;
                    resolve_single_structural_argument(&self.structural_types(), &values, source)?
                }
            };
            let mut storage_referent = StructuralRuntimePlace::from(&referent);
            let home = match home {
                Some(home) => {
                    let (depth, place, structural_type) = home.storage.scalar_array_frame();
                    storage_referent.path = home.path;
                    ArrayStorageHome::Frame {
                        depth,
                        place,
                        structural_type,
                    }
                }
                None => ArrayStorageHome::Runtime,
            };
            Ok((
                referent,
                ArrayStorage {
                    referent: storage_referent,
                    home,
                },
            ))
        }

        fn loaned_byte_array_frame(
            &self,
            place: semantic_vocabulary::PlaceId,
        ) -> Option<ArrayStorageHome> {
            let loan = self.element_view_values.get(&place)?;
            let (depth, frame_place, structural_type) =
                loan.complete_scalar_array_frame(&self.structural_types())?;
            let array = self.frame_array(depth, frame_place)?;
            if array.structural_type != structural_type
                || !loan.matches_descriptor_home(self.structural_values.get(&place)?)
                || terminal_semantics::scalar_array_leaf_shape(
                    self.structural_types().values(),
                    structural_type,
                )?
                .1 != u64::try_from(array.len()).ok()?
            {
                return None;
            }
            Some(ArrayStorageHome::Frame {
                depth,
                place: frame_place,
                structural_type,
            })
        }

        pub(crate) fn establish_byte_array_binding(
            &self,
            source: &StructuralArgument,
            presentation: &TerminalStructuralValue,
            collection: &terminal_semantics::CollectionBacking,
            identity_cursor: &mut Option<u64>,
        ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError>
        {
            if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!()) {
                return Err(malformed().in_reference_formation("byte view source access"));
            }
            if collection.storage == terminal_semantics::CollectionStorage::Borrowed {
                let value = self
                    .resolve_live_projection(source.place, &source.path, source.access)
                    .map_err(|error| error.in_reference_formation("byte view borrowed source"))?;
                let binding = self
                    .byte_descriptor_binding(source, &value)
                    .map_err(|error| error.in_reference_formation("byte view source binding"))?;
                let length = self
                    .byte_descriptor_length(&binding, &value)
                    .map_err(|error| error.in_reference_formation("byte view source length"))?;
                if (source.access == StructuralAccess::MutableBorrow
                    && !value.qualifications.is_empty())
                    || match collection.extent {
                        terminal_semantics::CollectionExtent::Fixed(capacity) => length != capacity,
                        terminal_semantics::CollectionExtent::StoredLength => false,
                        _ => true,
                    }
                {
                    return Err(malformed()
                        .in_reference_formation("byte view source qualifications or extent"));
                }
                let mut binding = self
                    .reborrow_byte_sequence_binding(&binding, &value, source.access)
                    .map_err(|error| error.in_reference_formation("byte view reborrow"))?;
                if source.access == StructuralAccess::SharedBorrow {
                    return Ok((
                        presentation.clone(),
                        binding.with_descriptor_home(presentation.clone()),
                    ));
                }
                let mut value = value;
                value.structural_type = presentation.structural_type;
                match &mut binding {
                    ByteSequenceBinding::MutableArray {
                        referent,
                        descriptor_home,
                        ..
                    } => {
                        if descriptor_home.is_some() {
                            *descriptor_home = Some(value.clone());
                            referent.structural_type = presentation.structural_type;
                        } else {
                            *referent = value.clone();
                        }
                        binding
                            .validate_mutable_referent(self, &value)
                            .map_err(|error| {
                                error.in_reference_formation("byte view representation")
                            })?;
                    }
                    ByteSequenceBinding::Immutable(bytes) => {
                        *bytes = bytes.clone().with_descriptor_home(value.clone());
                    }
                    ByteSequenceBinding::MutableOwner {
                        descriptor_home: descriptor_home @ Some(_),
                        ..
                    } => {
                        *descriptor_home = Some(value.clone());
                        binding
                            .validate_mutable_referent(self, &value)
                            .map_err(|error| {
                                error.in_reference_formation("byte view representation")
                            })?;
                    }
                    ByteSequenceBinding::Representation(_)
                    | ByteSequenceBinding::MutableField { .. }
                    | ByteSequenceBinding::MutableOwner { .. } => {
                        return Err(malformed());
                    }
                }
                return Ok((value, binding));
            }
            if source.access == StructuralAccess::SharedBorrow
                && matches!(
                    collection.extent,
                    terminal_semantics::CollectionExtent::LiveLength { .. }
                )
            {
                let (_, bytes) = self.bounded_owner_snapshot(source)?;
                let bytes = bytes.with_descriptor_home(presentation.clone());
                return Ok((presentation.clone(), ByteSequenceBinding::Immutable(bytes)));
            }
            let (mut referent, array) = self
                .byte_array_storage_staged(
                    source,
                    identity_cursor,
                    &mut RuntimeRows::new(self.account.as_ref()),
                )
                .map_err(|error| error.in_reference_formation("byte view backing storage"))?;
            let array_type = referent.structural_type;
            let length =
                byte_array_length(&self.structural_types(), array_type).ok_or_else(|| {
                    malformed().in_reference_formation("byte view backing array type")
                })?;
            if (source.access == StructuralAccess::MutableBorrow
                && !referent.qualifications.is_empty())
                || collection.extent != terminal_semantics::CollectionExtent::Fixed(length)
                || self.stored_byte_array_length(&array).map_err(|error| {
                    error.in_reference_formation("byte view backing array length")
                })? as u128
                    != u128::from(length)
            {
                return Err(malformed()
                    .in_reference_formation("byte view backing qualifications or extent"));
            }
            if source.access == StructuralAccess::SharedBorrow {
                let length = usize::try_from(length).map_err(|_| malformed())?;
                let bytes = self
                    .stored_byte_array_snapshot(&array, 0, length)?
                    .with_original_array(crate::storage::byte_sequences::OriginalByteArrayWindow {
                        array,
                        array_type,
                        offset: 0,
                        length,
                    })
                    .with_descriptor_home(presentation.clone());
                return Ok((presentation.clone(), ByteSequenceBinding::Immutable(bytes)));
            }
            referent.structural_type = presentation.structural_type;
            let binding = ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset: 0,
                length,
                referent: referent.clone(),
                descriptor_home: None,
            };
            binding
                .validate_mutable_referent(self, &referent)
                .map_err(|error| {
                    error.in_reference_formation("byte view installed array binding")
                })?;
            Ok((referent, binding))
        }

        pub(crate) fn prepare_array_view_argument_staged(
            &self,
            parameter: &StructuralParameterDeclaration,
            argument: &StructuralArgument,
            identity_cursor: &mut Option<u64>,
            local_array_roots: &mut RuntimeRows<
                semantic_vocabulary::PlaceId,
                TerminalStructuralValue,
            >,
        ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
        {
            if !matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            ) || !self
                .structural_types()
                .get(&parameter.structural_type)
                .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
            {
                return Ok(None);
            }
            if !terminal_semantics::view_descriptor_parameter(
                parameter,
                self.structural_types().values(),
            ) || argument.access != parameter.access
                || crate::engine::custody::place_is_claimed(&self.live_claims, argument.place)
            {
                return Err(malformed());
            }
            if argument.path.is_empty()
                && let Some(binding @ ByteSequenceBinding::Immutable(_)) =
                    self.byte_sequence_values.get(&argument.place)
            {
                let value = self.structural_value(argument.place)?;
                if parameter.access != StructuralAccess::SharedBorrow {
                    return Err(malformed());
                }
                self.check_byte_view_handback(value, argument, parameter)?;
                let binding =
                    self.reborrow_byte_sequence_binding(binding, value, parameter.access)?;
                if value.structural_type != parameter.structural_type {
                    let machine = self.program.verified_machine(self.current_machine)?;
                    let source = machine
                        .structural_parameters
                        .iter()
                        .chain(machine.block_structural_parameters())
                        .find(|source| source.place == argument.place)
                        .ok_or_else(malformed)?;
                    let types = self.structural_types();
                    if source.structural_type != value.structural_type
                        || source.access != StructuralAccess::SharedBorrow
                        || !matches!(
                            types.declaration(&source.structural_type)?.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                            )
                        )
                        || !matches!(
                            types.declaration(&parameter.structural_type)?.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            )
                        )
                    {
                        return Err(malformed());
                    }
                    let window = terminal_semantics::fixed_byte_array_window(
                        types.values(),
                        source,
                        argument,
                        parameter,
                    )
                    .ok_or_else(malformed)?;
                    let ByteSequenceBinding::Immutable(bytes) = binding else {
                        return Err(malformed());
                    };
                    if !window.backing_path.is_empty()
                        || window.offset != 0
                        || bytes.len() as u128 != u128::from(window.length)
                    {
                        return Err(malformed());
                    }
                    let mut presented = value.clone();
                    presented.structural_type = parameter.structural_type;
                    presented.qualifications = parameter.qualifications.clone();
                    let binding = ByteSequenceBinding::Immutable(
                        bytes.with_descriptor_home(presented.clone()),
                    );
                    return Ok(Some((presented, binding)));
                }
                return Ok(Some((value.clone(), binding)));
            }
            if argument.path.is_empty()
                && let Some(binding @ ByteSequenceBinding::MutableArray { .. }) =
                    self.byte_sequence_values.get(&argument.place)
            {
                let value = self.structural_value(argument.place)?;
                self.check_byte_view_handback(value, argument, parameter)?;
                let borrowed =
                    self.reborrow_byte_sequence_binding(binding, value, parameter.access)?;
                if value.structural_type != parameter.structural_type {
                    let machine = self.program.verified_machine(self.current_machine)?;
                    let source = machine
                        .structural_parameters
                        .iter()
                        .chain(machine.block_structural_parameters())
                        .find(|source| source.place == argument.place)
                        .ok_or_else(malformed)?;
                    let types = self.structural_types();
                    if source.structural_type != value.structural_type
                        || source.access != StructuralAccess::MutableBorrow
                        || parameter.access != StructuralAccess::SharedBorrow
                        || ![source.structural_type, parameter.structural_type]
                            .iter()
                            .all(|identity| {
                                types.get(identity).is_some_and(|declaration| {
                                    matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                        )
                                    )
                                })
                            })
                    {
                        return Err(malformed());
                    }
                    let window = terminal_semantics::fixed_byte_array_window(
                        types.values(),
                        source,
                        argument,
                        parameter,
                    )
                    .ok_or_else(malformed)?;
                    let ByteSequenceBinding::Immutable(bytes) = borrowed else {
                        return Err(malformed());
                    };
                    if !window.backing_path.is_empty()
                        || window.offset != 0
                        || bytes.len() as u128 != u128::from(window.length)
                        || !value.projected_qualifications.is_empty()
                    {
                        return Err(malformed());
                    }
                    let mut presented = value.clone();
                    presented.structural_type = parameter.structural_type;
                    presented.qualifications = parameter.qualifications.clone();
                    return Ok(Some((
                        presented.clone(),
                        ByteSequenceBinding::Immutable(bytes.with_descriptor_home(presented)),
                    )));
                }
                let presented = if let ByteSequenceBinding::MutableArray {
                    referent,
                    descriptor_home: Some(_),
                    ..
                } = binding
                {
                    let mut presented = value.clone();
                    presented.opaque_identity = referent.opaque_identity;
                    presented.path = referent.path.clone();
                    presented
                } else {
                    value.clone()
                };
                let borrowed = borrowed.with_descriptor_home(presented.clone());
                return Ok(Some((presented, borrowed)));
            }
            let root_type = self.live_structural_type(argument.place)?;
            let backing_path = match argument.path.split_last() {
                Some((StructuralPathSegment::FixedRange { .. }, backing)) => backing,
                _ => &argument.path,
            };
            let Some(array_type) =
                array_path_type(&self.structural_types(), root_type, backing_path)
            else {
                return Ok(None);
            };
            let Some(length) = byte_array_length(&self.structural_types(), array_type) else {
                return Ok(None);
            };
            let machine = self.program.machine(self.current_machine)?;
            let source = machine
                .structural_parameters
                .iter()
                .chain(machine.block_structural_parameters())
                .find(|source| source.place == argument.place)
                .cloned()
                .or_else(|| {
                    let place = machine.structural_place(argument.place)?;
                    let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } = place.kind
                    else {
                        return None;
                    };
                    let result = machine.operation(producer)?.result.structural()?;
                    if result.place != argument.place
                        || result.structural_type != structural_type
                        || !matches!(
                            result.multiplicity,
                            terminal_psi::nonlinear_multiplicity_pair_pattern!()
                        )
                        || !result.claims.is_empty()
                    {
                        return None;
                    }
                    Some(StructuralParameterDeclaration {
                        place: result.place,
                        position: 0,
                        structural_type,
                        multiplicity: result.multiplicity,
                        access: StructuralAccess::Owned,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                        additional_input_qualifications: result
                            .additional_input_qualifications
                            .clone(),
                    })
                })
                .ok_or_else(malformed)?;
            if source.structural_type != root_type {
                return Err(malformed());
            }
            let window = terminal_semantics::fixed_byte_array_loan(
                self.structural_types().values(),
                self.program.catalogs.structural_domains.iter(),
                &source,
                argument,
                parameter,
            )
            .ok_or_else(malformed)?;
            let backing_argument = StructuralArgument {
                place: argument.place,
                path: window.backing_path.to_vec(),
                access: argument.access,
            };
            let (mut referent, array) = self.byte_array_storage_staged(
                &backing_argument,
                identity_cursor,
                local_array_roots,
            )?;
            if self.stored_byte_array_length(&array)? as u128 != u128::from(length) {
                return Err(malformed());
            }
            referent.structural_type = parameter.structural_type;
            referent.qualifications = parameter.qualifications.clone();
            if let Some(segment @ StructuralPathSegment::FixedRange { .. }) = argument.path.last() {
                referent.path.push(segment.clone());
            }
            if parameter.access == StructuralAccess::SharedBorrow {
                let start = usize::try_from(window.offset).map_err(|_| malformed())?;
                let end = usize::try_from(
                    window
                        .offset
                        .checked_add(window.length)
                        .ok_or_else(malformed)?,
                )
                .map_err(|_| malformed())?;
                let descriptor = self
                    .stored_byte_array_snapshot(&array, start, end)?
                    .with_original_array(crate::storage::byte_sequences::OriginalByteArrayWindow {
                        array: array.clone(),
                        array_type,
                        offset: start,
                        length: end - start,
                    })
                    .with_descriptor_home(referent.clone());
                return Ok(Some((referent, ByteSequenceBinding::Immutable(descriptor))));
            }
            Ok(Some((
                referent.clone(),
                ByteSequenceBinding::MutableArray {
                    array,
                    array_type,
                    offset: window.offset,
                    length: window.length,
                    referent,
                    descriptor_home: None,
                },
            )))
        }
    }
}
pub(crate) mod case_membership {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{
        StructuralFieldNamespace, StructuralRuntimePlace, TerminalStructuralValue,
    };
    use crate::engine::custody::{project_structural_value, resolve_structural_path_type};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, PlaceId, StructuralCaseId, StructuralTypeId,
    };
    use std::collections::BTreeMap;
    use terminal_psi::{
        Operation, StructuralAccess, StructuralFieldDeclaration, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape, field_by_identity,
    };

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct StructuralCaseContents {
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) case: StructuralCaseId,
    }

    pub(crate) fn stored_case_at_projection(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        root: &TerminalStructuralValue,
        cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralCaseId> {
        let selected = project_structural_value(types, root, path).ok()?;
        let contents = cases.get(&StructuralRuntimePlace::from(&selected))?;
        let declaration = types.get(&selected.structural_type)?;
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            &declaration.shape
        else {
            return None;
        };
        (contents.structural_type == selected.structural_type
            && terminal_psi::find_structural_case(cases, contents.case).is_some())
        .then_some(contents.case)
    }

    impl TerminalExecution<'_> {
        pub(crate) fn active_field_declarations(
            &self,
            parent_type: StructuralTypeId,
            parent: &StructuralRuntimePlace,
            case: Option<StructuralCaseId>,
        ) -> Result<(&[StructuralFieldDeclaration], StructuralFieldNamespace), TerminalInterpretError>
        {
            let declaration = self.structural_declaration(parent_type)?;
            if let Some(case) = case {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return Err(malformed());
                };
                let contents = self.structural_cases.require(parent)?;
                if contents.structural_type != parent_type || contents.case != case {
                    return Err(malformed());
                }
                let selected =
                    terminal_psi::find_structural_case(cases, case).ok_or_else(malformed)?;
                Ok((&selected.fields, StructuralFieldNamespace::CasePayload))
            } else {
                Ok((
                    declaration.shape.direct_fields().ok_or_else(malformed)?,
                    StructuralFieldNamespace::Member,
                ))
            }
        }

        pub(crate) fn active_structural_claims(
            &self,
            source: PlaceId,
            catalogue: &[terminal_psi::StructuralResultClaimBinding],
        ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, TerminalInterpretError>
        {
            if catalogue.is_empty() {
                return Ok(Vec::new());
            }
            let root =
                crate::engine::custody::verified_structural_place(&self.structural_values, source)?;
            self.active_claims_in_value(root, catalogue)
        }

        pub(crate) fn active_claims_in_value(
            &self,
            root: &TerminalStructuralValue,
            catalogue: &[terminal_psi::StructuralResultClaimBinding],
        ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, TerminalInterpretError>
        {
            catalogue
                .iter()
                .filter_map(|binding| {
                    match case_projection_in_value_is_active(
                        &self.structural_types(),
                        root,
                        self.structural_cases.as_map(),
                        &binding.path,
                    ) {
                        Ok(true) => Some(Ok(binding.clone())),
                        Ok(false) => None,
                        Err(error) => Some(Err(error)),
                    }
                })
                .collect()
        }

        pub(crate) fn actual_storage_type_from_owner(
            &self,
            target: &StructuralRuntimePlace,
        ) -> Result<StructuralTypeId, TerminalInterpretError> {
            let mut actual = None;
            if let Some(owner) = self.activation_storage.owners.get(target) {
                return Ok(owner.structural_type);
            }
            for owner in self.live_structural_values().chain(
                self.borrowed_entry_storage
                    .iter()
                    .map(|(_, _, value)| value),
            ) {
                if owner.opaque_identity != target.opaque_identity
                    || !target.path.starts_with(&owner.path)
                {
                    continue;
                }
                let candidate = self.actual_storage_type(owner, target)?;
                if actual.is_some_and(|previous| previous != candidate) {
                    return Err(malformed());
                }
                actual = Some(candidate);
            }
            actual.ok_or_else(malformed)
        }

        pub(crate) fn actual_storage_type(
            &self,
            root: &TerminalStructuralValue,
            target: &StructuralRuntimePlace,
        ) -> Result<StructuralTypeId, TerminalInterpretError> {
            self.walk_actual_storage_path(root, target, |_| {})
        }

        pub(crate) fn actual_storage_projection(
            &self,
            root: &TerminalStructuralValue,
            target: &StructuralRuntimePlace,
        ) -> Result<(StructuralTypeId, Vec<CanonicalStructuralPathSegment>), TerminalInterpretError>
        {
            let mut path = Vec::new();
            let selected =
                self.walk_actual_storage_path(root, target, |segment| path.push(segment))?;
            Ok((selected, path))
        }

        fn walk_actual_storage_path(
            &self,
            root: &TerminalStructuralValue,
            target: &StructuralRuntimePlace,
            mut emit: impl FnMut(CanonicalStructuralPathSegment),
        ) -> Result<StructuralTypeId, TerminalInterpretError> {
            if target.opaque_identity != root.opaque_identity
                || !target.path.starts_with(&root.path)
            {
                return Err(malformed());
            }
            let mut structural_type = root.structural_type;
            let mut parent = StructuralRuntimePlace::from(root);
            let mut selected_case = None;
            for segment in &target.path[root.path.len()..] {
                let declaration = self.structural_declaration(structural_type)?;
                match segment {
                    StructuralPathSegment::Case(case) => {
                        if selected_case.is_some() {
                            return Err(malformed());
                        }
                        let (StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                        else {
                            return Err(malformed());
                        };
                        let actual = self.structural_cases.require(&parent)?;
                        if actual.structural_type != structural_type || actual.case != *case {
                            return Err(malformed());
                        }
                        selected_case = Some(
                            terminal_psi::find_structural_case(cases, *case)
                                .ok_or_else(malformed)?,
                        );
                        emit(CanonicalStructuralPathSegment::Case(*case));
                    }
                    StructuralPathSegment::Field(identity) => {
                        let fields = match selected_case.take() {
                            Some(case) => case.fields.as_slice(),
                            None => declaration.shape.direct_fields().ok_or_else(malformed)?,
                        };
                        let field = field_by_identity(fields, *identity).ok_or_else(malformed)?;
                        let Some(child) = field.field_type.as_structural() else {
                            return Err(malformed());
                        };
                        emit(CanonicalStructuralPathSegment::Field(field.id));
                        structural_type = child;
                    }
                    StructuralPathSegment::FixedIndex(index) => {
                        if selected_case.is_some() {
                            return Err(malformed());
                        }
                        let StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } = declaration.shape
                        else {
                            return Err(malformed());
                        };
                        if *index >= length {
                            return Err(malformed());
                        }
                        emit(CanonicalStructuralPathSegment::FixedIndex(*index));
                        structural_type = element;
                    }
                    _ => return Err(malformed()),
                }
                parent.path.push(segment.clone());
            }
            if selected_case.is_some() {
                return Err(malformed());
            }
            Ok(structural_type)
        }

        pub(crate) fn validate_case_projections(
            &self,
            operation: &Operation,
        ) -> Result<(), TerminalInterpretError> {
            for projection in operation.kind.structural_projections() {
                for (position, segment) in projection.path.iter().enumerate() {
                    let StructuralPathSegment::Case(case) = segment else {
                        continue;
                    };
                    let carrier = self.resolve_case_guard_carrier(
                        projection.root,
                        &projection.path[..position],
                    )?;
                    self.active_field_declarations(
                        carrier.structural_type,
                        &StructuralRuntimePlace::from(&carrier),
                        Some(*case),
                    )?;
                }
            }
            Ok(())
        }

        pub(crate) fn case_payload_projection(
            &self,
            source: PlaceId,
            path: &[CanonicalStructuralPathSegment],
        ) -> Result<(TerminalStructuralValue, Vec<StructuralPathSegment>), TerminalInterpretError>
        {
            let mut projection = Vec::with_capacity(path.len());
            let mut leaf =
                crate::engine::custody::verified_structural_place(&self.structural_values, source)
                    .cloned()?;
            let mut case_fields = None;
            let mut saw_case = false;
            for segment in path {
                let declaration = self.structural_declaration(leaf.structural_type)?;
                match segment {
                    CanonicalStructuralPathSegment::Referent => {
                        if case_fields.is_some() {
                            return Err(malformed());
                        }
                        projection.push(StructuralPathSegment::Referent);
                        leaf = self.resolve_live_projection(
                            source,
                            &projection,
                            StructuralAccess::SharedBorrow,
                        )?;
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        if case_fields.is_some() {
                            return Err(malformed());
                        }
                        let (StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                        else {
                            return Err(malformed());
                        };
                        let contents = self
                            .structural_cases
                            .get(&StructuralRuntimePlace::from(&leaf))
                            .ok_or_else(malformed)?;
                        if contents.structural_type != leaf.structural_type
                            || contents.case != *case
                        {
                            return Err(malformed());
                        }
                        case_fields = Some((
                            *case,
                            cases
                                .iter()
                                .find(|candidate| candidate.id == *case)
                                .ok_or_else(malformed)?
                                .fields
                                .as_slice(),
                        ));
                        projection.push(StructuralPathSegment::Case(*case));
                        saw_case = true;
                    }
                    CanonicalStructuralPathSegment::Field(field) => {
                        let fields = match case_fields.take() {
                            Some((case, fields)) => {
                                leaf.path.push(StructuralPathSegment::Case(case));
                                fields
                            }
                            None => match &declaration.shape {
                                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                                _ => return Err(malformed()),
                            },
                        };
                        let field = fields
                            .iter()
                            .find(|candidate| {
                                candidate.id == *field && !candidate.relevance.is_erased()
                            })
                            .ok_or_else(malformed)?;
                        let Some(child) = field.field_type.as_structural() else {
                            return Err(malformed());
                        };
                        leaf.path.push(StructuralPathSegment::Field(field.identity));
                        projection.push(StructuralPathSegment::Field(field.identity));
                        leaf.structural_type = child;
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        let StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } = declaration.shape
                        else {
                            return Err(malformed());
                        };
                        if case_fields.is_some() || *index >= length {
                            return Err(malformed());
                        }
                        leaf.path.push(StructuralPathSegment::FixedIndex(*index));
                        projection.push(StructuralPathSegment::FixedIndex(*index));
                        leaf.structural_type = element;
                    }
                }
            }
            if !saw_case || case_fields.is_some() {
                return Err(malformed());
            }
            Ok((leaf, projection))
        }

        pub(crate) fn observe_structural_case(
            &self,
            source: PlaceId,
            path: &[StructuralPathSegment],
        ) -> Result<StructuralCaseId, TerminalInterpretError> {
            if !self.structural_values.contains_key(&source) {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    source,
                ));
            }
            let selected =
                self.resolve_live_projection(source, path, StructuralAccess::SharedBorrow)?;
            let contents = self
                .structural_cases
                .get(&StructuralRuntimePlace::from(&selected))
                .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    source,
                ))?;
            if contents.structural_type != selected.structural_type {
                return Err(malformed());
            }
            Ok(contents.case)
        }
    }

    pub(crate) fn validate_case_projection(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
        cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        if case_projection_is_active(types, values, cases, source, path)? {
            Ok(())
        } else {
            Err(malformed())
        }
    }

    pub(crate) fn case_projection_is_active(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
        cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<bool, TerminalInterpretError> {
        if !path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
        {
            return Ok(true);
        }
        let root = crate::engine::custody::verified_structural_place(values, source)?;
        case_projection_in_value_is_active(types, root, cases, path)
    }

    pub(crate) fn case_projection_in_value_is_active(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        root: &TerminalStructuralValue,
        cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
        path: &[StructuralPathSegment],
    ) -> Result<bool, TerminalInterpretError> {
        case_projection_with_membership_is_active(types, root.structural_type, path, &|prefix| {
            stored_case_at_projection(types, root, cases, prefix)
        })
    }

    pub(crate) fn active_projected_qualifications(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        root: &TerminalStructuralValue,
        cases: &BTreeMap<StructuralRuntimePlace, StructuralCaseContents>,
        rows: &[terminal_psi::StructuralPathQualification],
    ) -> Result<Vec<terminal_psi::StructuralPathQualification>, TerminalInterpretError> {
        let mut active = Vec::new();
        for row in rows {
            if case_projection_in_value_is_active(types, root, cases, &row.path)? {
                active.push(row.clone());
            }
        }
        Ok(active)
    }

    pub(crate) fn case_projection_with_membership_is_active(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        membership: &impl Fn(&[StructuralPathSegment]) -> Option<StructuralCaseId>,
    ) -> Result<bool, TerminalInterpretError> {
        case_projection_with_checked_membership_is_active(types, structural_type, path, &|prefix| {
            Ok(membership(prefix))
        })
    }

    /// Resolve each conditional discriminator through its checked captured storage.
    /// A failed crossing remains an error; absence is never an inactive payload.
    pub(crate) fn case_projection_with_checked_membership_is_active(
        types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        membership: &impl Fn(
            &[StructuralPathSegment],
        ) -> Result<Option<StructuralCaseId>, TerminalInterpretError>,
    ) -> Result<bool, TerminalInterpretError> {
        let mut selected = path
            .iter()
            .enumerate()
            .filter_map(|(position, segment)| {
                if let StructuralPathSegment::Case(case) = segment {
                    Some((position, *case))
                } else {
                    None
                }
            })
            .peekable();
        if selected.peek().is_none() {
            return Ok(true);
        }
        for (position, case) in selected {
            let prefix = &path[..position];
            let selected_type = resolve_structural_path_type(types, structural_type, prefix)?;
            let stored = membership(prefix)?.ok_or_else(malformed)?;
            let declaration = types.declaration(&selected_type)?;
            let (StructuralTypeShape::Sum {
                cases: declared_cases,
            }
            | StructuralTypeShape::Mixed {
                cases: declared_cases,
                ..
            }) = &declaration.shape
            else {
                return Err(malformed());
            };
            if terminal_psi::find_structural_case(declared_cases, stored).is_none() {
                return Err(malformed());
            }
            if stored != case {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
pub(crate) mod contents {
    use crate::boundary::errors::TerminalInterpretError;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::record::storage::ConstructorStorage;
    use crate::storage::structural_storage::contents::{self, StorageLeafKind};
    use crate::{TerminalBoundaryStorageValue, TerminalStorageContents};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalStructuralContents {
        pub argument_index: u32,
        pub values: Vec<TerminalBoundaryStorageValue>,
    }

    impl TerminalExecution<'_> {
        pub(crate) fn bind_structural_contents(
            &mut self,
            arguments: &[TerminalStructuralContents],
        ) -> Result<(), TerminalInterpretError> {
            if arguments.is_empty() {
                return Ok(());
            }
            let mut staged = ConstructorStorage::new(self.account.as_ref());
            for argument in arguments {
                let invalid = |reason| TerminalInterpretError::StructuralContentsArgumentInvalid {
                    argument_index: argument.argument_index,
                    reason,
                };
                if argument
                    .values
                    .windows(2)
                    .any(|pair| pair[0].path >= pair[1].path)
                {
                    return Err(invalid(
                        "contents must have unique paths in canonical order",
                    ));
                }
                let parameter = self
                    .program
                    .machine(self.current_machine)?
                    .structural_parameters
                    .get(argument.argument_index as usize)
                    .ok_or_else(|| invalid("contents name no structural argument"))?;
                let root = self.structural_value(parameter.place)?.clone();
                let description = contents::describe(
                    self,
                    &root,
                    terminal_psi::StructuralAccess::MutableBorrow,
                    Some(&argument.values),
                )
                .map_err(|error| {
                    error.in_argument_preparation("describe complete entry contents")
                })?;
                if description.requires_erased_establishment {
                    return Err(invalid(
                        "scalar/case contents cannot establish erased bindings",
                    ));
                }
                let leaves = description.leaves;
                let has_bytes = argument
                    .values
                    .iter()
                    .any(|value| matches!(&value.contents, TerminalStorageContents::Bytes(_)));
                if !has_bytes
                    && leaves.iter().any(|leaf| {
                        matches!(
                            leaf.kind,
                            StorageLeafKind::Opaque { .. } | StorageLeafKind::ByteField { .. }
                        )
                    })
                {
                    return Err(invalid(
                        "scalar/case contents cannot establish opaque or reference backing",
                    ));
                }
                let mut visible = leaves
                    .iter()
                    .filter(|leaf| {
                        matches!(
                            leaf.kind,
                            StorageLeafKind::Scalar { .. }
                                | StorageLeafKind::Case { .. }
                                | StorageLeafKind::ByteField { .. }
                                | StorageLeafKind::Opaque { .. }
                        )
                    })
                    .collect::<Vec<_>>();
                let byte_arrays = leaves
                    .iter()
                    .filter(|leaf| matches!(leaf.kind, StorageLeafKind::ByteArray { .. }))
                    .collect::<Vec<_>>();
                visible.sort_unstable_by(|left, right| left.path.cmp(&right.path));
                if visible.len() != argument.values.len() {
                    return Err(invalid(
                        "contents must supply every active scalar and case exactly once",
                    ));
                }
                for (leaf, value) in visible.into_iter().zip(&argument.values) {
                    if let TerminalStorageContents::Bytes(bytes) = &value.contents {
                        if leaf.path != value.path {
                            return Err(invalid(
                                "byte contents do not name the complete active path",
                            ));
                        }
                        self.stage_immutable_contents(&root, leaf, bytes, &mut staged)
                            .map_err(|error| {
                                error.in_argument_preparation("bind immutable byte contents")
                            })?;
                        continue;
                    }
                    leaf.validate_value(value).map_err(invalid)?;
                    match (&leaf.kind, value.contents.clone()) {
                        (
                            StorageLeafKind::Scalar { destination, .. },
                            TerminalStorageContents::Scalar { value, .. },
                        ) => {
                            if !byte_arrays
                                .iter()
                                .any(|array| leaf.path.starts_with(&array.path))
                            {
                                staged.stage_scalar_endpoint(destination, value)?;
                            }
                        }
                        (
                            StorageLeafKind::Case {
                                destination,
                                structural_type,
                                case,
                            },
                            TerminalStorageContents::Case(_),
                        ) => {
                            staged.stage_case(destination.clone(), *structural_type, *case)?;
                        }
                        _ => return Err(invalid("contents do not select a scalar or case")),
                    }
                }
                for leaf in &leaves {
                    match &leaf.kind {
                        StorageLeafKind::ByteArray {
                            destination,
                            structural_type,
                            ..
                        } => {
                            let values = argument
                                .values
                                .iter()
                                .skip(
                                    argument
                                        .values
                                        .partition_point(|value| value.path < leaf.path),
                                )
                                .take_while(|value| value.path.starts_with(&leaf.path))
                                .map(|value| match value.contents.clone() {
                                    TerminalStorageContents::Scalar { value, .. } => Ok(value),
                                    _ => Err(invalid("byte array element is not a scalar")),
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            staged.stage_array(self, *structural_type, &values, destination)?;
                        }
                        StorageLeafKind::EmptyScalarArray {
                            destination,
                            structural_type,
                        } => {
                            staged.stage_array(self, *structural_type, &[], destination)?;
                        }
                        _ => {}
                    }
                }
            }
            staged.publish(self)
        }

        fn stage_immutable_contents(
            &self,
            root: &crate::TerminalStructuralValue,
            leaf: &crate::storage::structural_storage::contents::StorageLeaf,
            bytes: &[u8],
            staged: &mut ConstructorStorage,
        ) -> Result<(), TerminalInterpretError> {
            use crate::boundary::errors::malformed;
            use crate::boundary::values::{
                StructuralByteSequenceRuntimeField, StructuralFieldNamespace,
            };
            use crate::storage::byte_sequences::{
                ByteSequenceView, binding::ByteSequenceBinding, storage::ByteSequenceStorage,
            };
            if !root.qualifications.is_empty() || !root.projected_qualifications.is_empty() {
                return Err(malformed()
                    .in_argument_preparation("byte contents cannot establish qualifications"));
            }
            match &leaf.kind {
                StorageLeafKind::Opaque {
                    destination,
                    declaration,
                } => {
                    let declaration = self.structural_declaration(*declaration)?;
                    if !declaration.shape.is_shared_readable_byte_view()
                        || declaration.owns_linear_claim
                    {
                        return Err(malformed().in_argument_preparation(
                            "byte contents require an immutable byte declaration",
                        ));
                    }
                    let mut home = root.clone();
                    home.path = destination.path.clone();
                    home.structural_type = declaration.id;
                    let view = ByteSequenceView::new(ByteSequenceStorage::copy(
                        bytes,
                        self.account.as_ref(),
                    )?)
                    .with_descriptor_home(home);
                    staged.stage_byte_descriptor(
                        destination.clone(),
                        ByteSequenceBinding::Immutable(view),
                    )
                }
                StorageLeafKind::ByteField {
                    parent,
                    field,
                    declaration,
                } => {
                    if !declaration
                        .byte_sequence_carrier()
                        .is_some_and(|carrier| carrier.is_shared_readable_view())
                        || !declaration.byte_predicates().holds_for(bytes)
                    {
                        return Err(malformed().in_argument_preparation(
                            "byte field contents differ from their access or predicates",
                        ));
                    }
                    let namespace = if matches!(
                        leaf.path.iter().rev().nth(1),
                        Some(semantic_vocabulary::CanonicalStructuralPathSegment::Case(_))
                    ) {
                        StructuralFieldNamespace::CasePayload
                    } else {
                        StructuralFieldNamespace::Member
                    };
                    staged.stage_byte_field(
                        StructuralByteSequenceRuntimeField {
                            parent: parent.clone(),
                            namespace,
                            field: *field,
                        },
                        ByteSequenceBinding::Immutable(ByteSequenceView::new(
                            ByteSequenceStorage::copy(bytes, self.account.as_ref())?,
                        )),
                    )
                }
                _ => Err(malformed()
                    .in_argument_preparation("byte contents name scalar or non-byte storage")),
            }
        }
    }
}
pub(crate) mod initialization {
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::structural_inputs::TerminalStructuralInputs;
    use terminal_semantics::structural_paths_may_overlap;

    pub(crate) fn validate(
        inputs: TerminalStructuralInputs<'_>,
    ) -> Result<(), TerminalInterpretError> {
        for field in inputs.established_erased_fields {
            if !inputs
                .zero_initialized_arguments
                .contains(&field.argument_index)
                || field.field_path.is_empty()
                || field.carrier_type_identity.is_empty()
            {
                return Err(TerminalInterpretError::ZeroInitializedArgumentInvalid {
                    argument_index: field.argument_index,
                });
            }
        }
        let initializers = || {
            inputs
                .zero_initialized_arguments
                .iter()
                .copied()
                .map(|argument_index| (argument_index, true))
                .chain(
                    inputs
                        .contents
                        .iter()
                        .map(|contents| (contents.argument_index, false)),
                )
        };
        for (position, (argument_index, zeroed)) in initializers().enumerate() {
            let invalid = || {
                if zeroed {
                    TerminalInterpretError::ZeroInitializedArgumentInvalid { argument_index }
                } else {
                    TerminalInterpretError::StructuralContentsArgumentInvalid {
                        argument_index,
                        reason: "complete contents overlap another initializer or name no argument",
                    }
                }
            };
            let root = inputs
                .arguments
                .get(argument_index as usize)
                .ok_or_else(invalid)?;
            let overlaps = |other_index: u32, path: &[terminal_psi::StructuralPathSegment]| {
                inputs
                    .arguments
                    .get(other_index as usize)
                    .is_some_and(|other| {
                        if root.opaque_identity != other.opaque_identity {
                            return false;
                        }
                        let mut other_path = other.path.clone();
                        other_path.extend_from_slice(path);
                        structural_paths_may_overlap(&root.path, &other_path)
                    })
            };
            if initializers()
                .take(position)
                .any(|(other, _)| overlaps(other, &[]))
                || inputs
                    .scalar_fields
                    .iter()
                    .any(|field| overlaps(field.argument_index, &field.path))
                || inputs
                    .boolean_fields
                    .iter()
                    .any(|field| overlaps(field.argument_index, &field.path))
                || inputs
                    .primitive_values
                    .iter()
                    .any(|value| overlaps(value.argument_index, &[]))
                || inputs
                    .byte_arrays
                    .iter()
                    .any(|array| overlaps(array.argument_index, &array.path))
                || inputs
                    .access_view_establishments
                    .iter()
                    .any(|establishment| {
                        root.opaque_identity == establishment.referent.opaque_identity
                            && structural_paths_may_overlap(
                                &root.path,
                                &establishment.referent.path,
                            )
                    })
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
pub(crate) mod scalar_fields {
    use crate::storage::runtime_rows::RuntimeRows;

    use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{OperationKind, StructuralPathSegment, StructuralTypeDeclaration};

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralRuntimePlace;
    use crate::boundary::values::StructuralScalarRuntimeField;
    use crate::boundary::values::{
        TerminalScalarValue, TerminalStructuralBooleanFieldValue, TerminalStructuralValue,
    };
    use crate::engine::custody::direct_scalar_field_type;
    use crate::engine::custody::resolve_structural_path_type;
    use terminal_psi::TerminalMachine;

    pub(crate) mod entry {
        use crate::boundary::errors::malformed;
        use crate::boundary::structural_inputs::scalar_fields::{
            PlaceId, StructuralScalarRuntimeField, StructuralTypeDeclaration, StructuralTypeId,
            TerminalInterpretError, TerminalMachine, TerminalStructuralValue,
        };
        use crate::boundary::values::StructuralRuntimePlace;
        use crate::storage::runtime_rows::RuntimeRows;
        use std::collections::BTreeSet;
        use terminal_psi::StructuralPathSegment;
        use terminal_psi::{StructuralFieldType, StructuralTypeShape};
        use terminal_semantics::scalar_field_requires_evidence;

        pub(crate) fn validate(
            machine: &TerminalMachine,
            types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
            roots: &RuntimeRows<PlaceId, TerminalStructuralValue>,
            values: &crate::storage::scalar_storage::ScalarStorageRows<
                StructuralScalarRuntimeField,
            >,
            zero_initialized_arguments: &[u32],
        ) -> Result<(), TerminalInterpretError> {
            for (position, parameter) in machine.structural_parameters.iter().enumerate() {
                if zero_initialized_arguments.contains(&(position as u32)) {
                    continue;
                }
                let root =
                    crate::engine::custody::verified_structural_place(roots, parameter.place)?;
                validate_type(
                    types,
                    root.structural_type,
                    &mut StructuralRuntimePlace::from(root),
                    values,
                    u32::try_from(position).map_err(|_| malformed())?,
                    &mut Vec::new(),
                )?;
            }
            Ok(())
        }

        fn validate_type(
            types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
            current: StructuralTypeId,
            parent: &mut StructuralRuntimePlace,
            values: &crate::storage::scalar_storage::ScalarStorageRows<
                StructuralScalarRuntimeField,
            >,
            argument_index: u32,
            active: &mut Vec<StructuralTypeId>,
        ) -> Result<(), TerminalInterpretError> {
            if !requires_contents(types, current) {
                return Ok(());
            }
            if active.contains(&current) {
                return Err(malformed());
            }
            active.push(current);
            let declaration = types.declaration(&current)?;
            match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => {
                    for field in fields {
                        if field.relevance.is_erased() {
                            continue;
                        }
                        match &field.field_type {
                            field_type if scalar_field_requires_evidence(field_type) => {
                                let key =
                                    StructuralScalarRuntimeField::member(parent.clone(), field.id);
                                if !values
                                    .get(&key)
                                    .map(crate::storage::scalar_storage::ScalarCell::read)
                                    .transpose()?
                                    .is_some_and(|value| value.satisfies_field(field_type))
                                {
                                    return Err(
                                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                                    argument_index,
                                    field: field.id,
                                },
                            );
                                }
                            }
                            StructuralFieldType::Structural(child) => {
                                parent
                                    .path
                                    .push(StructuralPathSegment::Field(field.identity));
                                validate_type(
                                    types,
                                    *child,
                                    parent,
                                    values,
                                    argument_index,
                                    active,
                                )?;
                                parent.path.pop();
                            }
                            _ => {}
                        }
                    }
                }
                StructuralTypeShape::FixedArray { element, length } => {
                    let length = length.literal().ok_or(malformed())?;
                    for position in 0..length {
                        parent
                            .path
                            .push(StructuralPathSegment::FixedIndex(position));
                        validate_type(types, *element, parent, values, argument_index, active)?;
                        parent.path.pop();
                    }
                }
                terminal_psi::sum_or_mixed_shape_pattern!()
                | StructuralTypeShape::Reference { .. }
                | StructuralTypeShape::AtomicCell(_) => {
                    return Err(malformed());
                }
                StructuralTypeShape::Opaque
                | StructuralTypeShape::PrimitiveScalar(_)
                | StructuralTypeShape::ByteSequence(_)
                | StructuralTypeShape::ElementView { .. } => {}
            }
            active.pop();
            Ok(())
        }

        fn requires_contents(
            types: &crate::embedding::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
            root: StructuralTypeId,
        ) -> bool {
            let mut pending = vec![root];
            let mut visited = BTreeSet::new();
            while let Some(current) = pending.pop() {
                if !visited.insert(current) {
                    continue;
                }
                let Some(declaration) = types.get(&current) else {
                    return true;
                };
                let mut fields = Vec::new();
                match &declaration.shape {
                    StructuralTypeShape::Reference { .. } | StructuralTypeShape::AtomicCell(_) => {
                        return true;
                    }
                    StructuralTypeShape::Opaque
                    | StructuralTypeShape::PrimitiveScalar(_)
                    | StructuralTypeShape::ByteSequence(_) => {
                        continue;
                    }
                    StructuralTypeShape::FixedArray { element, length } => {
                        if length.literal() != Some(0) {
                            pending.push(*element);
                        }
                        continue;
                    }
                    StructuralTypeShape::ElementView { element, .. } => {
                        pending.push(*element);
                        continue;
                    }
                    StructuralTypeShape::Record {
                        fields: members, ..
                    } => fields.extend(members),
                    StructuralTypeShape::Sum { cases } => {
                        fields.extend(cases.iter().flat_map(|case| &case.fields));
                    }
                    StructuralTypeShape::Mixed {
                        fields: members,
                        cases,
                    } => {
                        fields.extend(members);
                        fields.extend(cases.iter().flat_map(|case| &case.fields));
                    }
                }
                for field in fields
                    .into_iter()
                    .filter(|field| !field.relevance.is_erased())
                {
                    match field.field_type {
                        StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::RefinedScalar(_) => {
                            return true;
                        }
                        StructuralFieldType::Structural(child) => pending.push(child),
                        _ => {}
                    }
                }
            }
            false
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalStructuralScalarFieldValue {
        pub argument_index: u32,
        pub path: Vec<StructuralPathSegment>,
        pub field: StructuralFieldId,
        pub value: TerminalScalarValue,
    }

    impl From<&TerminalStructuralBooleanFieldValue> for TerminalStructuralScalarFieldValue {
        fn from(field: &TerminalStructuralBooleanFieldValue) -> Self {
            Self {
                argument_index: field.argument_index,
                path: field.path.clone(),
                field: field.field,
                value: TerminalScalarValue::Boolean(field.value),
            }
        }
    }

    pub(crate) fn bind(
        machine: &TerminalMachine,
        structural_types: &crate::embedding::program::ProgramDeclarations<
            '_,
            StructuralTypeDeclaration,
        >,
        structural_values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
        arguments: &[TerminalStructuralScalarFieldValue],
        mut values: crate::storage::scalar_storage::ScalarStorageRows<StructuralScalarRuntimeField>,
        zero_initialized_arguments: &[u32],
    ) -> Result<
        crate::storage::scalar_storage::ScalarStorageRows<StructuralScalarRuntimeField>,
        TerminalInterpretError,
    > {
        for argument in arguments {
            let invalid = || TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                argument_index: argument.argument_index,
                field: argument.field,
            };
            let parameter = machine
                .structural_parameters
                .get(argument.argument_index as usize)
                .ok_or_else(invalid)?;
            let root = crate::engine::custody::verified_structural_place(
                structural_values,
                parameter.place,
            )?;
            let parent_type = resolve_structural_path_type(
                structural_types,
                root.structural_type,
                &argument.path,
            )
            .map_err(|_| invalid())?;
            if direct_scalar_field_type(structural_types, parent_type, argument.field)
                != Some(argument.value.scalar_type())
                || matches!(argument.value, TerminalScalarValue::Integer { scalar_type, value } if !scalar_type.admits(value))
            {
                return Err(invalid());
            }
            let mut parent = StructuralRuntimePlace::from(root);
            parent.path.extend_from_slice(&argument.path);
            if values
                .insert(
                    StructuralScalarRuntimeField::member(parent, argument.field),
                    argument.value,
                )?
                .is_some()
            {
                return Err(invalid());
            }
        }
        entry::validate(
            machine,
            structural_types,
            structural_values,
            &values,
            zero_initialized_arguments,
        )?;
        for operation in machine.operations() {
            let (source, path, field) = match operation.kind {
                OperationKind::BooleanStructuralField {
                    source,
                    ref path,
                    field,
                } => (source, path, field),
                _ => continue,
            };
            if !machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == source)
            {
                continue;
            }
            let root =
                crate::engine::custody::verified_structural_place(structural_values, source)?;
            if terminal_semantics::case_payload_carrier(
                structural_types.values(),
                root.structural_type,
                path,
            )
            .is_some()
            {
                continue;
            }
            let carrier = terminal_semantics::record_field_carrier(
                structural_types.values(),
                root.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            let mut parent = StructuralRuntimePlace::from(root);
            parent.path.extend(carrier.path);
            if !values.contains_key(&StructuralScalarRuntimeField::member(parent, field)) {
                return Err(TerminalInterpretError::StructuralBooleanFieldMissing {
                    source,
                    field,
                });
            }
        }
        Ok(values)
    }
}

use crate::boundary::structural_inputs::access_views::TerminalAccessViewEstablishment;
use crate::boundary::structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
use crate::boundary::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
use crate::boundary::values::{
    TerminalStructuralBooleanFieldValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEstablishedErasedField {
    pub argument_index: u32,
    pub field_path: Vec<String>,
    pub carrier_type_identity: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalStructuralInputs<'input> {
    pub arguments: &'input [TerminalStructuralValue],
    pub contents: &'input [contents::TerminalStructuralContents],
    pub zero_initialized_arguments: &'input [u32],
    pub established_erased_fields: &'input [TerminalEstablishedErasedField],
    pub scalar_fields: &'input [TerminalStructuralScalarFieldValue],
    pub boolean_fields: &'input [TerminalStructuralBooleanFieldValue],
    pub primitive_values: &'input [TerminalStructuralPrimitiveValue],
    pub byte_arrays: &'input [TerminalStructuralByteArrayValue],
    pub access_view_establishments: &'input [TerminalAccessViewEstablishment],
    pub private_entry_ram: &'input [terminal_installation::CheckedPrivateEntryRamRecipe],
}
