use super::{
    Error, attribution, host, source, validation::validate_function_fragment_object_artifact,
};
use crate::{
    ObjectArtifact, ObjectCodeAttribution, ObjectFunction, ObjectScalarCallStack,
    ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack,
};
use object_file::{
    ObjectPlan, RelocationPlan, SectionKind, SectionPlan,
    StagedOptimizedRelocationFreeObjectContainer, SymbolKind, SymbolPlan, SymbolSection,
    entry_symbol_name,
};
use std::sync::Arc;

pub fn build_function_fragment_object_artifact(
    replay: Arc<StagedOptimizedRelocationFreeObjectContainer>,
) -> Result<ObjectArtifact, Error> {
    let source = replay.as_ref();
    source::admit(source)?;
    let text = source.source().text_section();
    let fragments = source::fragments(source);
    let mut object = ObjectPlan::with_capacity(text.target, 1, text.functions.len());
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text.bytes.len(),
        alignment: host(text.section_alignment)?,
    });
    let mut functions = Vec::new();
    let mut semantic_code_attribution = Vec::new();
    let mut requires_graph_storage_replay = false;
    for placed in &text.functions {
        let (attachment, provenance) = source::fragment_metadata(source, placed.machine)?;
        let (abstracted, targeted) = source::function(source, placed.machine)?;
        requires_graph_storage_replay |=
            source::requires_graph_storage_replay(&abstracted.operations);
        let offset = host(placed.section_offset)?;
        let length = host(placed.byte_count)?;
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: if placed.machine == text.semantic_entry {
                entry_symbol_name(text.target)
            } else {
                format!("omega_terminal_machine_{}", placed.machine.get())
            },
            section: SymbolSection::Section(SectionKind::Text),
            offset,
            size: length,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        if placed.machine == text.semantic_entry {
            object.layout.entry_symbol = symbol;
        }
        let frame = source::frame(source, placed.machine)?;
        let frame_bytes = u32::try_from(frame.map_or(0, |frame| frame.frame_size_bytes))
            .map_err(|_| Error::Overflow)?;
        let alignment = frame.map_or(16, |frame| u32::from(frame.abi_stack_alignment_bytes));
        let linkage = match text.target.architecture {
            target::Architecture::X86_64 => 8,
            target::Architecture::Aarch64 => 0,
        };
        let unit = matches!(
            abstracted.result,
            abstract_operations::AbstractFunctionResult::Unit
        );
        let mut unit_call_stacks = Vec::new();
        let mut scalar_call_stacks = Vec::new();
        let mut local_peak = frame_bytes;
        for call in text
            .resolved_internal_machine_calls
            .iter()
            .filter(|call| call.caller == placed.machine)
        {
            let caller_live_bytes = frame_bytes.checked_add(linkage).ok_or(Error::Overflow)?;
            local_peak = local_peak.max(caller_live_bytes);
            let owner = target_operations::CallSiteOwner::Operation(call.operation);
            let text_offset = host(call.field_section_offset)?;
            if unit {
                unit_call_stacks.push(ObjectUnitCallStack {
                    owner,
                    target: call.callee,
                    text_offset,
                    active_frame_bytes: frame_bytes,
                    transient_bytes: linkage,
                    caller_live_bytes,
                });
            } else {
                scalar_call_stacks.push(ObjectScalarCallStack {
                    owner,
                    target: call.callee,
                    text_offset,
                    caller_live_bytes,
                });
            }
        }
        functions.push(ObjectFunction {
            machine: placed.machine,
            attachment,
            scalar_abi: targeted.scalar_abi.clone(),
            mixed_structural_scalar_abi: targeted.mixed_structural_scalar_abi.clone(),
            structural_call_scalar_return: None,
            parameter_abi: source::parameter_abi(targeted).map(|(call_plan, parameters, _)| {
                machine_code::ParameterFunctionAbiRecord {
                    call_plan: call_plan.clone(),
                    parameters: parameters.to_vec(),
                    entry_register_spills: Vec::new(),
                }
            }),
            provenance: provenance.clone(),
            symbol,
            text_offset: offset,
            byte_count: length,
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: unit.then_some(ObjectUnitStack {
                frame_bytes,
                local_peak_bytes: local_peak,
                stack_alignment: alignment,
            }),
            scalar_stack: (!unit).then_some(ObjectScalarStack {
                local_peak_bytes: local_peak,
                stack_alignment: alignment,
            }),
            unit_call_stacks,
            scalar_call_stacks,
            internal_unit_calls: Vec::new(),
            internal_unit_scalar_calls: Vec::new(),
            installed_provider_unit_scalar_calls: Vec::new(),
            dynamic_calls: Vec::new(),
            stored_dynamic_calls: Vec::new(),
            dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: Vec::new(),
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_write_only_primitive_stores: Vec::new(),
            scalar_structural_scalar_field_stores: Vec::new(),
            unit_parameters: Vec::new(),
            unit_parameter_homes: Vec::new(),
            unit_continuations: Vec::new(),
            unit_affine_cleanup: None,
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        });
        let fragment = fragments
            .functions
            .iter()
            .find(|row| row.machine == placed.machine)
            .ok_or(Error::Mismatch("missing current fragment"))?;
        let mut rows = attribution::produce(fragment, abstracted)?;
        rows.extend(super::structural::settlement_attributions(
            source,
            placed.machine,
        )?);
        rows.sort_by_key(|row| (row.operation_ordinal, row.code_offset, row.byte_count));
        super::structural::populate(
            source,
            functions
                .last_mut()
                .ok_or(Error::Mismatch("missing inserted function"))?,
            &rows,
        )?;
        for attribution in rows {
            semantic_code_attribution.push(ObjectCodeAttribution {
                machine: placed.machine,
                text_offset: offset
                    .checked_add(attribution.code_offset)
                    .ok_or(Error::Overflow)?,
                attribution,
            });
        }
    }
    let artifact = ObjectArtifact {
        requires_graph_storage_replay,
        fragment_replay: Some(super::replay::FragmentReplay(Arc::clone(&replay))),
        psi: text.psi,
        target: text.target,
        x86_feature_profile: None,
        x86_scalar_fma_provider: None,
        entry: text.semantic_entry,
        object,
        relocations: RelocationPlan::with_target(text.target),
        text_bytes: text.bytes.clone(),
        data_bytes: Vec::new(),
        dynamic_conformance_tables: Vec::new(),
        forwarded_dynamic_descriptor_adapters: Vec::new(),
        forwarded_dynamic_descriptor_tables: Vec::new(),
        functions,
        private_functions: Vec::new(),
        semantic_code_attribution,
        port_effects: Vec::new(),
        boundary_settlements: super::structural::settlements(source)?,
        foreign_calls: Vec::new(),
    };
    validate_function_fragment_object_artifact(source, &artifact)?;
    Ok(artifact)
}
