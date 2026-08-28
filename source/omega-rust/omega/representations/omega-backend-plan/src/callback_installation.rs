//! Sealed, address-free evidence for callback entries that may later be
//! attributed to one installed code occurrence.

use crate::{
    BackendPlan, CallbackPlacementBindingIdentity, CallbackPrivateObjectStoreRequest,
    callback_thunk_placement_identity_fingerprint, replay_callback_private_object_store_requests,
};
use omega_calling_conventions::PlanDiagnostic;
use omega_control_flow::MachineFunctionIdentity;
use omega_machine_bytes::{
    CompilerInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
};
use omega_object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SymbolKind, SymbolPlan, SymbolSection, object_function_symbol,
};
use omega_target::{Architecture, NativeTarget};
use psi_arena::{Handle, HandleSpan};
use psi_layout_plans::EntryStubId;

/// One exact callback installation entry, still free of a runtime address or
/// installation authority.
///
/// The entry retains the complete placement identity rather than relying on
/// its compact compiler-issued `EntryStubId`. It also retains the exact
/// encoded store and relocation rows that connect the callback function to its
/// registrar destination. Private fields prevent callers from assembling a
/// convincing subset.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackInstallationEntry {
    placement_index: usize,
    placement_identity: CallbackPlacementBindingIdentity,
    object_store: CallbackPrivateObjectStoreRequest,
    assigned_binding_index: usize,
    assigned_binding: crate::CallbackRegistrarAssignedOperandBinding,
    requirement_identity: String,
    function_identity: MachineFunctionIdentity,
    target: NativeTarget,
    entry: EntryStubId,
    function_symbol: ObjectSymbolHandle,
    function_symbol_plan: SymbolPlan,
    text_offset: usize,
    text_byte_count: usize,
    storage_region: omega_target_operations::RuntimeStorageRegion,
    storage_offset: usize,
    storage_symbol: ObjectSymbolHandle,
    storage_symbol_plan: SymbolPlan,
    entry_machine_name: String,
    storage_byte_size: usize,
    storage_alignment: usize,
    store_function_identity: MachineFunctionIdentity,
    store_function_symbol: ObjectSymbolHandle,
    store_selected_instruction_index: u32,
    abstract_store_instruction: Handle<omega_abstract_operations::AbstractOperation>,
    target_store_instruction: Handle<omega_target_operations::TargetOperation>,
    assigned_store_instruction: Handle<omega_assigned_target_operations::AssignedOperation>,
    encoded_store: EncodedMachineInstruction,
    relocations: Vec<(Handle<RelocationRecord>, RelocationRecord)>,
}

impl CallbackInstallationEntry {
    pub const fn placement_index(&self) -> usize {
        self.placement_index
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn function_identity(&self) -> MachineFunctionIdentity {
        self.function_identity
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn text_offset(&self) -> usize {
        self.text_offset
    }

    pub const fn text_byte_count(&self) -> usize {
        self.text_byte_count
    }
}

/// Complete ordered callback-entry catalog retained beside one emitted native
/// artifact. This carrier is deliberately non-clonable and grants no loading,
/// registration, invocation, lease, capacity, or publication authority.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackInstallationManifest {
    target: NativeTarget,
    placement_identity_fingerprint: u64,
    entries: Vec<CallbackInstallationEntry>,
}

impl CallbackInstallationManifest {
    /// Construct the only valid empty manifest. This creates no entry evidence
    /// and is used by retained native artifacts with no callback placements.
    pub fn empty_for_target(target: NativeTarget) -> Self {
        Self {
            target,
            placement_identity_fingerprint: callback_thunk_placement_identity_fingerprint(&[]),
            entries: Vec::new(),
        }
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn placement_identity_fingerprint(&self) -> u64 {
        self.placement_identity_fingerprint
    }

    pub fn entries(&self) -> &[CallbackInstallationEntry] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<CallbackInstallationEntry> {
        self.entries
    }

    /// Independently replay the retained object, encoded-instruction, and
    /// relocation evidence against an emitted artifact snapshot.
    pub fn replay_artifact(
        &self,
        target: NativeTarget,
        placement_identity_fingerprint: u64,
        object: &ObjectPlan,
        relocations: &RelocationPlan,
        encoded: &EncodedMachineCode,
    ) -> Result<(), PlanDiagnostic> {
        if self.target != target
            || object.target != target
            || relocations.target != target
            || self.placement_identity_fingerprint != placement_identity_fingerprint
        {
            return Err(manifest_error("target or placement fingerprint"));
        }
        let mut previous_placement = None;
        for entry in &self.entries {
            if previous_placement.is_some_and(|previous| previous >= entry.placement_index) {
                return Err(manifest_error("ordered unique placement identity"));
            }
            previous_placement = Some(entry.placement_index);
            replay_entry(target, object, relocations, encoded, entry)?;
            let expected_entry = callback_entry_id(
                entry.placement_index,
                &entry.placement_identity,
                entry.function_identity,
            )?;
            if entry.entry != expected_entry
                || entry.function_identity.callback_thunk_placement_index()
                    != Some(entry.placement_index)
                || entry.placement_identity.private_materialization.is_none()
                || entry.requirement_identity
                    != entry.placement_identity.canonical_requirement_overload
            {
                return Err(manifest_error("domain-separated callback entry identity"));
            }
        }
        for (index, left) in self.entries.iter().enumerate() {
            if self.entries[index + 1..]
                .iter()
                .any(|right| right.entry == left.entry)
            {
                return Err(manifest_error("unique callback entry identity"));
            }
        }
        Ok(())
    }
}

/// Build the exact ordered callback installation-entry manifest from a fully
/// replayed backend plan.
pub fn build_callback_installation_manifest(
    plan: &BackendPlan,
) -> Result<CallbackInstallationManifest, PlanDiagnostic> {
    replay_callback_private_object_store_requests(
        plan.target,
        &plan.callback_placements,
        &plan.callback_thunks,
        &plan.callback_private_relocations,
        &plan.host_calls,
        &plan.abstract_operations.semantics.boundaries,
        &plan.callback_registrar_arguments,
        &plan.layouts,
        &plan.callback_registrar_destinations,
        &plan.abstract_operations,
        &plan.target_operations,
        &plan.assigned_target_operations,
        &plan.callback_registrar_assigned_operands,
        &plan.object,
        plan.entry_machine_name(),
        &plan.callback_private_object_stores,
    )?;
    let placement_identity_fingerprint =
        callback_thunk_placement_identity_fingerprint(&plan.callback_thunks);
    let mut entries = Vec::with_capacity(plan.callback_private_object_stores.len());
    for request in plan.callback_private_object_stores.iter() {
        let placement_index = request
            .function_identity
            .callback_thunk_placement_index()
            .ok_or_else(|| manifest_error("callback function role"))?;
        let placement = plan
            .callback_placements
            .get(placement_index)
            .ok_or_else(|| manifest_error("placement row"))?;
        let thunk = plan
            .callback_thunks
            .get(placement_index)
            .ok_or_else(|| manifest_error("thunk row"))?;
        if thunk.placement_index != placement_index
            || thunk.placement_identity != crate::callback_placement_binding_identity(placement)
            || request.function_identity != thunk.function_identity
        {
            return Err(manifest_error("placement/function join"));
        }

        let store_selected_instruction_index = request.assigned_store_instruction.arena_index();
        let matches = plan
            .encoded_machine
            .code
            .instructions
            .iter()
            .filter(|(_, instruction)| {
                instruction.selected_instruction_index == store_selected_instruction_index
            })
            .collect::<Vec<_>>();
        let [(store_handle, encoded_store)] = matches.as_slice() else {
            return Err(manifest_error("one encoded callback store"));
        };
        let owners = plan
            .encoded_machine
            .code
            .functions
            .iter()
            .filter(|(_, function)| handle_in_span(*store_handle, function.instructions))
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        let [store_function] = owners.as_slice() else {
            return Err(manifest_error("one encoded registrar function"));
        };
        let (store_function_symbol, _) =
            object_function_symbol(&plan.object, store_function.identity)
                .ok_or_else(|| manifest_error("registrar function symbol"))?;
        let relocation_rows = exact_store_relocations(
            plan.target,
            &plan.relocations,
            store_function_symbol,
            store_selected_instruction_index,
            encoded_store.bytes,
            request.function_symbol,
            request.storage_symbol,
        )?;
        let entry = CallbackInstallationEntry {
            placement_index,
            placement_identity: thunk.placement_identity.clone(),
            object_store: request.clone(),
            assigned_binding_index: request.assigned_binding_index,
            assigned_binding: request.assigned_binding.clone(),
            requirement_identity: placement.canonical_requirement_overload.clone(),
            function_identity: request.function_identity,
            target: plan.target,
            entry: callback_entry_id(
                placement_index,
                &thunk.placement_identity,
                request.function_identity,
            )?,
            function_symbol: request.function_symbol,
            function_symbol_plan: request.function_symbol_plan.clone(),
            text_offset: request.function_symbol_plan.offset,
            text_byte_count: request.function_symbol_plan.size,
            storage_region: request.storage_region,
            storage_offset: request.destination_offset,
            storage_symbol: request.storage_symbol,
            storage_symbol_plan: request.storage_symbol_plan.clone(),
            entry_machine_name: plan.entry_machine_name().to_owned(),
            storage_byte_size: request.byte_size,
            storage_alignment: request.alignment,
            store_function_identity: store_function.identity,
            store_function_symbol,
            store_selected_instruction_index,
            abstract_store_instruction: request.abstract_store_instruction,
            target_store_instruction: request.target_store_instruction,
            assigned_store_instruction: request.assigned_store_instruction,
            encoded_store: (*encoded_store).clone(),
            relocations: relocation_rows,
        };
        entries.push(entry);
    }
    let manifest = CallbackInstallationManifest {
        target: plan.target,
        placement_identity_fingerprint,
        entries,
    };
    manifest.replay_artifact(
        plan.target,
        placement_identity_fingerprint,
        &plan.object,
        &plan.relocations,
        &plan.encoded_machine.code,
    )?;
    Ok(manifest)
}

fn replay_entry(
    target: NativeTarget,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    encoded: &EncodedMachineCode,
    entry: &CallbackInstallationEntry,
) -> Result<(), PlanDiagnostic> {
    let Some((function_symbol, function_plan)) =
        object_function_symbol(object, entry.function_identity)
    else {
        return Err(manifest_error("callback function symbol"));
    };
    let demand = &entry.assigned_binding.destination.binding.demand;
    let exact_geometry = match (
        &entry.assigned_binding.destination.kind,
        &entry.assigned_binding.target_operand.kind,
    ) {
        (
            crate::CallbackRegistrarPhysicalDestinationKind::Field { layout_demand, .. },
            omega_target_operations::TargetInstructionOperandKind::RuntimeStorageAddress {
                region,
                byte_offset,
            },
        ) => {
            *region == entry.storage_region
                && *byte_offset == entry.object_store.storage_base_offset
                && layout_demand.offset == entry.object_store.slot_offset
                && layout_demand.byte_size == entry.storage_byte_size
                && layout_demand.alignment == entry.storage_alignment
                && byte_offset.checked_add(layout_demand.offset) == Some(entry.storage_offset)
        }
        _ => false,
    };
    if entry.object_store.assigned_binding_index != entry.assigned_binding_index
        || entry.object_store.assigned_binding != entry.assigned_binding
        || entry.object_store.storage_region != entry.storage_region
        || entry.object_store.destination_offset != entry.storage_offset
        || entry.object_store.byte_size != entry.storage_byte_size
        || entry.object_store.alignment != entry.storage_alignment
        || entry.object_store.storage_symbol != entry.storage_symbol
        || entry.object_store.storage_symbol_plan != entry.storage_symbol_plan
        || entry.object_store.function_identity != entry.function_identity
        || entry.object_store.function_symbol != entry.function_symbol
        || entry.object_store.function_symbol_plan != entry.function_symbol_plan
        || entry.object_store.abstract_store_instruction != entry.abstract_store_instruction
        || entry.object_store.target_store_instruction != entry.target_store_instruction
        || entry.object_store.assigned_store_instruction != entry.assigned_store_instruction
        || !exact_geometry
        || demand.placement_index != entry.placement_index
        || demand.placement_identity != entry.placement_identity
        || demand.function_identity != entry.function_identity
        || entry.assigned_binding_index != entry.assigned_binding.destination_index
        || entry.assigned_store_instruction.arena_index() != entry.store_selected_instruction_index
    {
        return Err(manifest_error(
            "complete private object-store request identity",
        ));
    }
    let encoded_functions = encoded
        .functions
        .iter()
        .filter(|(_, function)| function.identity == entry.function_identity)
        .map(|(_, function)| function)
        .collect::<Vec<_>>();
    let [encoded_function] = encoded_functions.as_slice() else {
        return Err(manifest_error("one encoded callback function"));
    };
    if entry.function_symbol != function_symbol
        || entry.target != target
        || entry.function_symbol_plan != *function_plan
        || function_plan.kind != SymbolKind::Function
        || function_plan.section != SymbolSection::Section(SectionKind::Text)
        || entry.text_offset != function_plan.offset
        || entry.text_byte_count != function_plan.size
        || encoded_function.byte_offset != entry.text_offset
        || encoded_function.byte_count != entry.text_byte_count
    {
        return Err(manifest_error("callback text interval"));
    }
    if !object.layout.symbols.is_valid(entry.storage_symbol)
        || object.layout.symbols.get(entry.storage_symbol) != &entry.storage_symbol_plan
        || entry.storage_symbol_plan.kind != SymbolKind::Object
        || entry.storage_symbol_plan.section != SymbolSection::Section(SectionKind::Bss)
        || entry.storage_symbol_plan.name
            != omega_object_file::storage_region_symbol_name(
                entry.storage_region,
                &entry.entry_machine_name,
            )
        || entry
            .storage_offset
            .checked_add(entry.storage_byte_size)
            .is_none_or(|end| end > entry.storage_symbol_plan.size)
        || entry.storage_byte_size != std::mem::size_of::<u64>()
        || entry.storage_alignment != std::mem::align_of::<u64>()
        || !entry.storage_offset.is_multiple_of(entry.storage_alignment)
    {
        return Err(manifest_error("canonical BSS storage symbol and geometry"));
    }
    let store_functions = encoded
        .functions
        .iter()
        .filter(|(_, function)| function.identity == entry.store_function_identity)
        .map(|(_, function)| function)
        .collect::<Vec<_>>();
    let [store_function] = store_functions.as_slice() else {
        return Err(manifest_error("one encoded registrar function"));
    };
    let (store_symbol, _) = object_function_symbol(object, entry.store_function_identity)
        .ok_or_else(|| manifest_error("registrar function symbol"))?;
    let store_rows = encoded
        .instructions
        .iter()
        .filter(|(handle, instruction)| {
            handle_in_span(*handle, store_function.instructions)
                && instruction.selected_instruction_index == entry.store_selected_instruction_index
        })
        .collect::<Vec<_>>();
    let [(_, encoded_store)] = store_rows.as_slice() else {
        return Err(manifest_error("one encoded callback store"));
    };
    if entry.store_function_symbol != store_symbol
        || entry.encoded_store != **encoded_store
        || !matches!(
            encoded_store.compiler_validation_kind,
            Some(CompilerInstructionValidationKind::CompilerBodyFunctionAddressStore {
                function,
                target_region,
                target_offset,
            }) if function == entry.function_identity
                && target_region == entry.storage_region
                && target_offset == entry.storage_offset
        )
    {
        return Err(manifest_error("encoded callback store identity"));
    }
    let expected = exact_store_relocations(
        target,
        relocations,
        store_symbol,
        entry.store_selected_instruction_index,
        encoded_store.bytes,
        entry.function_symbol,
        entry.storage_symbol,
    )?;
    if entry.relocations != expected {
        return Err(manifest_error("callback store relocation snapshot"));
    }
    Ok(())
}

fn exact_store_relocations(
    target: NativeTarget,
    relocations: &RelocationPlan,
    store_function_symbol: ObjectSymbolHandle,
    selected_instruction_index: u32,
    instruction_bytes: HandleSpan<u8>,
    function_symbol: ObjectSymbolHandle,
    storage_symbol: ObjectSymbolHandle,
) -> Result<Vec<(Handle<RelocationRecord>, RelocationRecord)>, PlanDiagnostic> {
    if instruction_bytes.is_empty() {
        return Err(manifest_error("encoded callback store byte interval"));
    }
    let instruction_offset = usize::try_from(instruction_bytes.start().arena_index())
        .map_err(|_| manifest_error("encoded callback store byte offset"))?;
    let expected = match target.architecture {
        Architecture::X86_64 => vec![
            (2usize, 8usize, RelocationKind::Absolute64, function_symbol),
            (12, 8, RelocationKind::Absolute64, storage_symbol),
        ],
        Architecture::Aarch64 => vec![
            (
                0usize,
                4usize,
                RelocationKind::Aarch64Page21,
                function_symbol,
            ),
            (4, 4, RelocationKind::Aarch64PageOffset12, function_symbol),
            (8, 4, RelocationKind::Aarch64Page21, storage_symbol),
            (12, 4, RelocationKind::Aarch64PageOffset12, storage_symbol),
        ],
    };
    let rows = relocations
        .records()
        .filter(|(_, record)| {
            record.origin
                == RelocationOrigin::Instruction {
                    function_symbol_handle: store_function_symbol,
                    selected_instruction_index,
                }
        })
        .map(|(handle, record)| (handle, record.clone()))
        .collect::<Vec<_>>();
    // Relocation-plan insertion order is part of the existing final-replay
    // contract: source materialization precedes destination materialization,
    // and each AArch64 page row precedes its page-offset row. Reordering is
    // therefore identity drift rather than an equivalent representation.
    if rows.len() != expected.len() {
        return Err(manifest_error("callback store relocation cardinality"));
    }
    for ((_, actual), (relative, width, kind, symbol)) in rows.iter().zip(expected) {
        if actual.section != SectionKind::Text
            || actual.offset != instruction_offset.saturating_add(relative)
            || actual.byte_width != width
            || actual.kind != kind
            || actual.symbol_handle != symbol
            || actual.addend != 0
        {
            return Err(PlanDiagnostic(format!(
                "callback installation manifest lost its exact callback store relocation identity: actual={actual:?}, expected offset={}, width={width}, kind={kind:?}, symbol={symbol:?}",
                instruction_offset.saturating_add(relative)
            )));
        }
    }
    Ok(rows)
}

fn callback_entry_id(
    placement_index: usize,
    placement_identity: &CallbackPlacementBindingIdentity,
    function_identity: MachineFunctionIdentity,
) -> Result<EntryStubId, PlanDiagnostic> {
    let synthetic_thunk_fingerprint = {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        fingerprint_into(&mut hash, b"omega.callback-installed-entry.v2");
        fingerprint_into(
            &mut hash,
            &crate::callback_placements::callback_placement_identity_row_fingerprint(
                b"omega.callback-installed-entry-placement.v2",
                placement_index,
                placement_identity,
            )
            .to_le_bytes(),
        );
        let continuation = function_identity.associated_source_continuation();
        fingerprint_into(
            &mut hash,
            &u64::from(continuation.machine.arena_index()).to_le_bytes(),
        );
        fingerprint_into(
            &mut hash,
            &u64::from(continuation.machine.generation()).to_le_bytes(),
        );
        fingerprint_into(
            &mut hash,
            &u64::from(continuation.state.arena_index()).to_le_bytes(),
        );
        fingerprint_into(
            &mut hash,
            &u64::from(continuation.state.generation()).to_le_bytes(),
        );
        fingerprint_into(
            &mut hash,
            &(continuation.segment_index as u64).to_le_bytes(),
        );
        hash
    };
    EntryStubId::from_normalized_identity(synthetic_thunk_fingerprint)
        .map_err(|_| manifest_error("nonzero callback entry identity"))
}

fn fingerprint_into(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn handle_in_span<T>(handle: Handle<T>, span: HandleSpan<T>) -> bool {
    !span.is_empty()
        && handle.generation() == span.start().generation()
        && handle.arena_index() >= span.start().arena_index()
        && handle.arena_index() < span.start().arena_index().saturating_add(span.count())
}

fn manifest_error(identity: &str) -> PlanDiagnostic {
    PlanDiagnostic(format!(
        "callback installation manifest lost its exact {identity}"
    ))
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;
    use crate::BoundCallbackPrivateMaterialization;
    use omega_calling_conventions::{
        CallSignature, CallbackMaterializationContext, CallingPolicy, NativePlace,
        StaticMachineBinderId, callback_layout_plan_id, callback_layout_slot_id,
        callback_native_parameter_id, callback_requirement_id,
        evaluate_ordinary_boundary_entry_plan,
    };
    use omega_machine_bytes::{EncodedMachineFunction, EncodedMachinePlan};
    use omega_object_file::{FunctionSymbolPlan, ObjectPlan, RelocationRecord};
    use omega_target_operations::RuntimeStorageRegion;
    use psi_checked_trees::NominalMachineUseSite;
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    fn key(machine: u32, state: u32) -> omega_control_flow::StateKey {
        omega_control_flow::StateKey {
            machine: SymbolHandle::from_arena_index(machine),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    fn resource_receipt(
        machine: SymbolHandle,
        entry: SymbolHandle,
    ) -> psi_checked_trees::CheckedCallbackResourceReceipt {
        psi_checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
            &psi_checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
                machine, entry, 0xfeed,
            ),
        )
        .expect("canonical checked callback resource receipt")
    }

    fn placement_identity() -> CallbackPlacementBindingIdentity {
        let requirement_name = "Registrar::callback";
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty fixture boundary");
        let parameter = callback_native_parameter_id(requirement_name, 0);
        let layout = callback_layout_plan_id(41, 8, 8);
        let destination = NativePlace::Field {
            parameter,
            layout,
            field_path: vec![callback_layout_slot_id(layout, "handler")],
        };
        let selected_machine = SymbolHandle::from_arena_index(9);
        let selected_entry = SymbolHandle::from_arena_index(10);
        CallbackPlacementBindingIdentity {
            site: NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_arena_index(7),
            ),
            registration_operation: SymbolHandle::from_arena_index(8),
            static_machine_ordinal: 0,
            selected_machine,
            selected_entry,
            satisfaction_trait: SymbolHandle::from_arena_index(11),
            satisfaction_requirement: SymbolHandle::from_arena_index(12),
            canonical_requirement_overload: requirement_name.into(),
            boundary_calling_plan_fingerprint: boundary.contract_fingerprint(),
            resource_receipt: resource_receipt(selected_machine, selected_entry),
            private_materialization: Some(BoundCallbackPrivateMaterialization {
                binder: StaticMachineBinderId::new(13).unwrap(),
                destination,
                requirement: callback_requirement_id(requirement_name),
                registrar_boundary_entry_plan: boundary.plan().clone(),
                registrar_calling_plan_fingerprint: boundary.contract_fingerprint(),
                context: CallbackMaterializationContext::default(),
            }),
        }
    }

    pub fn callback_installation_test_fixture(
        target: NativeTarget,
    ) -> (
        CallbackInstallationManifest,
        ObjectPlan,
        RelocationPlan,
        EncodedMachineCode,
    ) {
        let placement_index = 0;
        let placement_identity = placement_identity();
        let callback_identity = MachineFunctionIdentity::callback_thunk(key(9, 10), 0).unwrap();
        let store_identity = MachineFunctionIdentity::source(key(1, 2));
        let mut object = ObjectPlan::with_capacities(target, 2, 3, 2);
        object
            .layout
            .sections
            .insert(omega_object_file::SectionPlan {
                kind: SectionKind::Text,
                size: 128,
                alignment: 16,
            });
        object
            .layout
            .sections
            .insert(omega_object_file::SectionPlan {
                kind: SectionKind::Bss,
                size: 128,
                alignment: 8,
            });
        let store_function_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "registrar".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 0,
            size: 32,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: store_identity,
            symbol: store_function_symbol,
        });
        let function_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "__omega_callback_private".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 64,
            size: 8,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: callback_identity,
            symbol: function_symbol,
        });
        let storage_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::storage_region_symbol_name(
                RuntimeStorageRegion::Machine,
                "Main",
            ),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 128,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });

        let width = match target.architecture {
            Architecture::X86_64 => 27,
            Architecture::Aarch64 => 20,
        };
        let mut encoded = EncodedMachinePlan::with_capacity(target, 2, 1, 128);
        for _ in 0..16 {
            encoded.code.bytes.insert(0);
        }
        let byte_span = encoded.code.bytes.insert_many(vec![0; width]);
        while encoded.code.bytes.len() < 72 {
            encoded.code.bytes.insert(0);
        }
        encoded.code.byte_count = encoded.code.bytes.len();
        let encoded_store = EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: byte_span,
            compiler_validation_kind: Some(
                CompilerInstructionValidationKind::CompilerBodyFunctionAddressStore {
                    function: callback_identity,
                    target_region: RuntimeStorageRegion::Machine,
                    target_offset: 40,
                },
            ),
            ..Default::default()
        };
        let store_handle = encoded.code.instructions.insert(encoded_store.clone());
        encoded.code.functions.insert(EncodedMachineFunction {
            symbol: "registrar".into(),
            identity: store_identity,
            byte_offset: 0,
            byte_count: 32,
            instructions: HandleSpan::from_parts(store_handle, 1),
        });
        encoded.code.functions.insert(EncodedMachineFunction {
            symbol: "__omega_callback_private".into(),
            identity: callback_identity,
            byte_offset: 64,
            byte_count: 8,
            instructions: HandleSpan::empty(),
        });

        let store_offset = usize::try_from(byte_span.start().arena_index()).unwrap();
        let expected = match target.architecture {
            Architecture::X86_64 => vec![
                (
                    store_offset + 2,
                    8usize,
                    RelocationKind::Absolute64,
                    function_symbol,
                ),
                (
                    store_offset + 12,
                    8,
                    RelocationKind::Absolute64,
                    storage_symbol,
                ),
            ],
            Architecture::Aarch64 => vec![
                (
                    store_offset,
                    4usize,
                    RelocationKind::Aarch64Page21,
                    function_symbol,
                ),
                (
                    store_offset + 4,
                    4,
                    RelocationKind::Aarch64PageOffset12,
                    function_symbol,
                ),
                (
                    store_offset + 8,
                    4,
                    RelocationKind::Aarch64Page21,
                    storage_symbol,
                ),
                (
                    store_offset + 12,
                    4,
                    RelocationKind::Aarch64PageOffset12,
                    storage_symbol,
                ),
            ],
        };
        let mut relocations = RelocationPlan::with_target(target);
        let mut retained_relocations = Vec::new();
        for (offset, byte_width, kind, symbol_handle) in expected {
            let record = RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: store_function_symbol,
                    selected_instruction_index: 4,
                },
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle,
                addend: 0,
                kind,
            };
            let handle = relocations.push_record(record.clone());
            retained_relocations.push((handle, record));
        }
        let abstract_store_instruction = Handle::from_parts(4, 1);
        let target_store_instruction = Handle::from_parts(4, 1);
        let assigned_store_instruction = Handle::from_parts(4, 1);
        let binder = StaticMachineBinderId::new(13).unwrap();
        let demand = crate::CallbackPrivateRelocationDemand {
            placement_index,
            placement_identity: placement_identity.clone(),
            binder,
            destination: placement_identity
                .private_materialization
                .as_ref()
                .unwrap()
                .destination
                .clone(),
            requirement: callback_requirement_id("Registrar::callback"),
            function_identity: callback_identity,
            private_symbol: Arc::from("__omega_callback_private"),
        };
        let argument_binding = crate::CallbackRegistrarArgumentBinding {
            demand_index: 0,
            demand,
            host_call: Handle::invalid(),
            native_argument: Handle::invalid(),
        };
        let parameter = callback_native_parameter_id("Registrar::callback", 0);
        let NativePlace::Field {
            layout, field_path, ..
        } = &placement_identity
            .private_materialization
            .as_ref()
            .unwrap()
            .destination
        else {
            unreachable!()
        };
        let layout_demand = omega_layout::TargetClosedPrivateCallbackDemand {
            data_symbol: SymbolHandle::from_arena_index(14),
            slot_identity: Arc::from("handler"),
            layout_subject_identity: Arc::from("Registrar"),
            callback_requirement_identity: Arc::from("Registrar::callback"),
            layout: *layout,
            slot: field_path[0],
            requirement: callback_requirement_id("Registrar::callback"),
            offset: 8,
            byte_size: 8,
            alignment: 8,
        };
        let destination = crate::CallbackRegistrarPhysicalDestination {
            binding_index: 0,
            binding: argument_binding,
            formal_ordinal: 0,
            parameter_placement: omega_calling_conventions::ValuePlacement {
                shape: omega_calling_conventions::ValueShape::integer(8, 8),
                locations: Vec::new(),
            },
            kind: crate::CallbackRegistrarPhysicalDestinationKind::Field {
                layout_demand_index: 0,
                layout_demand,
            },
        };
        let abstract_operand = Handle::invalid();
        let target_operand = Handle::invalid();
        let assigned_binding = crate::CallbackRegistrarAssignedOperandBinding {
            destination_index: 0,
            destination,
            abstract_instruction: Handle::invalid(),
            target_instruction: Handle::invalid(),
            assigned_instruction: Handle::invalid(),
            abstract_provenance: omega_abstract_operations::AbstractHostOperationProvenance {
                source_call_index: 1,
                source_call_generation: 1,
                call_ordinal: 0,
                operation_ordinal: 0,
                formal_operands: Arc::from([]),
            },
            provenance: omega_target_operations::TargetHostOperationProvenance {
                occurrence: Handle::invalid(),
                boundary_edge: Handle::invalid(),
                call_ordinal: 0,
                operation_ordinal: 0,
                formal_operands: Arc::from([]),
            },
            formal_operand: omega_target_operations::TargetHostFormalOperandBinding {
                native_argument: Handle::invalid(),
                formal_ordinal: 0,
                native_parameter: parameter,
                abstract_operand,
                abstract_operand_kind:
                    omega_abstract_operations::InstructionOperandKind::ImmediateInteger(0),
                operand: target_operand,
            },
            target_operand: omega_target_operations::TargetInstructionOperand {
                kind:
                    omega_target_operations::TargetInstructionOperandKind::RuntimeStorageAddress {
                        region: RuntimeStorageRegion::Machine,
                        byte_offset: 32,
                    },
            },
            assigned_operand: Handle::invalid(),
        };
        let object_store = CallbackPrivateObjectStoreRequest {
            assigned_binding_index: 0,
            assigned_binding: assigned_binding.clone(),
            storage_region: RuntimeStorageRegion::Machine,
            storage_base_offset: 32,
            slot_offset: 8,
            destination_offset: 40,
            byte_size: 8,
            alignment: 8,
            storage_symbol,
            storage_symbol_plan: object.layout.symbols.get(storage_symbol).clone(),
            function_identity: callback_identity,
            function_symbol,
            function_symbol_plan: object.layout.symbols.get(function_symbol).clone(),
            abstract_store_instruction,
            target_store_instruction,
            assigned_store_instruction,
        };
        let entry = CallbackInstallationEntry {
            placement_index,
            placement_identity: placement_identity.clone(),
            object_store,
            assigned_binding_index: 0,
            assigned_binding,
            requirement_identity: placement_identity.canonical_requirement_overload.clone(),
            function_identity: callback_identity,
            target,
            entry: callback_entry_id(placement_index, &placement_identity, callback_identity)
                .unwrap(),
            function_symbol,
            function_symbol_plan: object.layout.symbols.get(function_symbol).clone(),
            text_offset: 64,
            text_byte_count: 8,
            storage_region: RuntimeStorageRegion::Machine,
            storage_offset: 40,
            storage_symbol,
            storage_symbol_plan: object.layout.symbols.get(storage_symbol).clone(),
            entry_machine_name: "Main".into(),
            storage_byte_size: 8,
            storage_alignment: 8,
            store_function_identity: store_identity,
            store_function_symbol,
            store_selected_instruction_index: 4,
            abstract_store_instruction,
            target_store_instruction,
            assigned_store_instruction,
            encoded_store,
            relocations: retained_relocations,
        };
        let manifest = CallbackInstallationManifest {
            target,
            placement_identity_fingerprint: 73,
            entries: vec![entry],
        };
        (manifest, object, relocations, encoded.code)
    }

    #[cfg(test)]
    fn replay(
        manifest: &CallbackInstallationManifest,
        object: &ObjectPlan,
        relocations: &RelocationPlan,
        encoded: &EncodedMachineCode,
    ) -> Result<(), PlanDiagnostic> {
        manifest.replay_artifact(
            manifest.target,
            manifest.placement_identity_fingerprint,
            object,
            relocations,
            encoded,
        )
    }

    #[cfg(test)]
    #[test]
    fn replays_exact_callback_entries_for_both_native_instruction_families() {
        for target in [NativeTarget::windows_x64(), NativeTarget::linux_arm64()] {
            let (manifest, object, relocations, encoded) =
                callback_installation_test_fixture(target);
            replay(&manifest, &object, &relocations, &encoded)
                .unwrap_or_else(|error| panic!("{target:?}: {error:?}"));
            assert_eq!(manifest.entries()[0].text_offset(), 64);
            assert_eq!(manifest.entries()[0].text_byte_count(), 8);
        }
    }

    #[cfg(test)]
    #[test]
    fn rejects_symbol_interval_encoded_kind_and_identity_drift() {
        let target = NativeTarget::windows_x64();
        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].text_offset += 1;
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].function_symbol_plan.kind = SymbolKind::Object;
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].storage_symbol_plan.section = SymbolSection::Section(SectionKind::Data);
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].storage_symbol_plan.kind = SymbolKind::Function;
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0]
            .storage_symbol_plan
            .name
            .push_str("_drift");
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].storage_offset += 8;
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].encoded_store.compiler_validation_kind = None;
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].object_store.storage_base_offset += 8;
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].object_store.abstract_store_instruction = Handle::invalid();
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].requirement_identity.push_str("::drift");
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (mut manifest, object, relocations, encoded) =
            callback_installation_test_fixture(target);
        manifest.entries[0].entry = EntryStubId::from_normalized_identity(1).unwrap();
        assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

        let (manifest, object, relocations, encoded) = callback_installation_test_fixture(target);
        assert!(
            manifest
                .replay_artifact(
                    target,
                    manifest.placement_identity_fingerprint ^ 1,
                    &object,
                    &relocations,
                    &encoded
                )
                .is_err()
        );
    }

    #[cfg(test)]
    #[test]
    fn rejects_missing_duplicate_reordered_and_drifted_relocations_on_both_targets() {
        for target in [NativeTarget::windows_x64(), NativeTarget::linux_arm64()] {
            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            relocations.record_set.records = psi_arena::Arena::new();
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            let duplicate = relocations.records().next().unwrap().1.clone();
            relocations.push_record(duplicate);
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            let rows = relocations
                .records()
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>();
            relocations.record_set.records = psi_arena::Arena::new();
            for row in rows.into_iter().rev() {
                relocations.push_record(row);
            }
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            let handle = relocations.records().next().unwrap().0;
            relocations.record_set.records.get_mut(handle).addend = 1;
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            let handle = relocations.records().next().unwrap().0;
            relocations.record_set.records.get_mut(handle).symbol_handle =
                manifest.entries[0].storage_symbol;
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            let handle = relocations.records().next().unwrap().0;
            relocations.record_set.records.get_mut(handle).kind = RelocationKind::Aarch64Branch26;
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());

            let (manifest, object, mut relocations, encoded) =
                callback_installation_test_fixture(target);
            let handle = relocations.records().next().unwrap().0;
            relocations.record_set.records.get_mut(handle).origin =
                RelocationOrigin::Materialization {
                    object_symbol_handle: manifest.entries[0].function_symbol,
                };
            assert!(replay(&manifest, &object, &relocations, &encoded).is_err());
        }
    }
}
