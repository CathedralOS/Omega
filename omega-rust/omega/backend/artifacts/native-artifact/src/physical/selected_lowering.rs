use machine_code::{
    Aarch64ReturnLinkEvidence, MachineCodeFunction, MachineCodePlan, SemanticCodeSite,
    StackAdjustmentPair, UnitStackEvidence,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::TerminalPsiProvenance;
use terminal_psi::TerminalPsiIdentity;

mod scalar;

const SELECTED_LOWERING_BINDING_DOMAIN: &[u8] =
    b"omega.native-artifact.selected-lowering-publication-binding.sha256.v1\0";
const SELECTED_LOWERING_MACHINE_PROJECTION_DOMAIN: &[u8] =
    b"omega.native-artifact.selected-lowering-machine-projection.sha256.v1\0";

/// Exact optimizer lineage and machine plan admitted by selected-lowering
/// native publication.
///
/// This carrier is deliberately narrower than a general physical-pipeline
/// descriptor. It binds validated Unit returns and fixed-integer scalar leaves;
/// it cannot carry policy callbacks or claim support for calls and providers.
#[derive(Debug, Clone, Copy)]
pub struct SelectedLoweringNativePublicationInput<'plan> {
    selections: OptimizationSelectionIdentity,
    selected_lowering_completion: SelectedLoweringOptimizationCompletionIdentity,
    pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_relative_realization_manifest: FunctionRelativeOptimizationRealizationManifestIdentity,
    function_fragment_emission: FunctionFragmentEmissionIdentity,
    function_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    machine_code: &'plan MachineCodePlan,
}

impl<'plan> SelectedLoweringNativePublicationInput<'plan> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        selections: OptimizationSelectionIdentity,
        selected_lowering_completion: SelectedLoweringOptimizationCompletionIdentity,
        pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        function_relative_realization_manifest: FunctionRelativeOptimizationRealizationManifestIdentity,
        function_fragment_emission: FunctionFragmentEmissionIdentity,
        function_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
        machine_code: &'plan MachineCodePlan,
    ) -> Self {
        Self {
            selections,
            selected_lowering_completion,
            pre_physical_manifest,
            post_allocation_manifest,
            function_relative_realization_manifest,
            function_fragment_emission,
            function_fragment_manifest,
            machine_code,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedLoweringNativePublicationBinding {
    machine_projection: [u8; 32],
    identity: [u8; 32],
}

impl SelectedLoweringNativePublicationBinding {
    pub(super) const fn machine_projection(&self) -> &[u8; 32] {
        &self.machine_projection
    }

    pub(super) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

pub(crate) fn derive_selected_lowering_publication_binding(
    terminal: TerminalPsiIdentity,
    input: SelectedLoweringNativePublicationInput<'_>,
) -> Result<SelectedLoweringNativePublicationBinding, &'static str> {
    if input.machine_code.psi != terminal {
        return Err("selected-lowering machine plan is detached from its Terminal identity");
    }
    validate_leaf_machine_plan(input.machine_code)?;
    let machine_projection = machine_projection_from_plan(input.machine_code);

    let mut digest = Sha256::new();
    digest.update(SELECTED_LOWERING_BINDING_DOMAIN);
    digest.update(input.selections.bytes());
    digest.update(input.selected_lowering_completion.bytes());
    digest.update(input.pre_physical_manifest.bytes());
    digest.update(input.post_allocation_manifest.bytes());
    digest.update(input.function_relative_realization_manifest.bytes());
    digest.update(input.function_fragment_emission.bytes());
    digest.update(input.function_fragment_manifest.bytes());
    digest.update(machine_projection);
    Ok(SelectedLoweringNativePublicationBinding {
        machine_projection,
        identity: digest.finalize().into(),
    })
}

pub(crate) fn validate_selected_lowering_publication_object(
    binding: &SelectedLoweringNativePublicationBinding,
    object: &image_emission::ObjectArtifact,
) -> Result<(), &'static str> {
    validate_leaf_object(object)?;
    if machine_projection_from_object(object)? != *binding.machine_projection() {
        return Err(
            "selected-lowering object disagrees with the exact admitted machine projection",
        );
    }
    Ok(())
}

fn validate_leaf_machine_plan(plan: &MachineCodePlan) -> Result<(), &'static str> {
    let shape = cohort_return_shape(plan.target)?;
    if plan.functions.is_empty() {
        return Err("selected-lowering publication requires at least one machine function");
    }
    let mut previous = None;
    let mut entry_count = 0usize;
    for function in &plan.functions {
        if previous.is_some_and(|previous| previous >= function.machine) {
            return Err("selected-lowering machine functions are not in canonical order");
        }
        previous = Some(function.machine);
        entry_count += usize::from(function.machine == plan.entry);
        if function.scalar_stack.is_some() {
            scalar::validate_machine_function(function, plan.target)?;
        } else {
            validate_return_only_machine_function(function, &shape)?;
        }
    }
    if entry_count != 1 {
        return Err("selected-lowering machine plan does not contain exactly one entry");
    }
    if plan
        .functions
        .iter()
        .any(|function| function.scalar_stack.is_some())
    {
        // Object admission independently decodes the existing machine bytes
        // and replays their exact stack mutations. It does not emit them again.
        let object = image_emission::build_object_artifact(plan).map_err(
            |_| "selected-lowering scalar machine bytes failed independent object replay",
        )?;
        validate_leaf_object(&object)?;
    }
    Ok(())
}

fn validate_return_only_machine_function(
    function: &MachineCodeFunction,
    shape: &ReturnOnlyShape,
) -> Result<(), &'static str> {
    let [return_edge] = function.provenance.edges.as_slice() else {
        return Err("selected-lowering return-only function requires one Terminal return edge");
    };
    let [attribution] = function.semantic_code_attribution.as_slice() else {
        return Err("selected-lowering return-only function requires one edge attribution");
    };
    let Some(stack) = function.unit_stack else {
        return Err("selected-lowering return-only function requires Unit stack evidence");
    };
    let Some(cleanup) = function.unit_affine_cleanup.as_ref() else {
        return Err("selected-lowering return-only function requires Unit cleanup custody");
    };
    if !function.provenance.operations.is_empty()
        || function.bytes.as_slice() != shape.bytes
        || function.fixed_integer_scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || function.structural_call_scalar_return.is_some()
        || function.unit_scalar_abi.is_some()
        || !function.x86_scalar_fma.is_empty()
        || !function.x86_scalar_fma_occurrences.is_empty()
        || function.x86_floating_control.is_some()
        || stack != shape.stack
        || !function.unit_parameter_homes.is_empty()
        || !function.unit_parameters.is_empty()
        || function.scalar_stack.is_some()
        || !function.internal_calls.is_empty()
        || !function.foreign_calls.is_empty()
        || !function.internal_unit_calls.is_empty()
        || !function.internal_unit_scalar_calls.is_empty()
        || !function.installed_provider_unit_scalar_calls.is_empty()
        || !function.dynamic_calls.is_empty()
        || !function.stored_dynamic_calls.is_empty()
        || !function.dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_descriptor_calls.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
        || !function.unit_write_only_primitive_stores.is_empty()
        || !function.scalar_structural_scalar_field_stores.is_empty()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || !function.scalar_structural_parameters.is_empty()
        || !function.scalar_structural_parameter_homes.is_empty()
        || function.ranked_u32_countdown.is_some()
        || !function.port_effects.is_empty()
        || !function.boundary_settlements.is_empty()
        || function.structural_return.is_some()
        || cleanup.psi_edge != *return_edge
        || !cleanup.structural_types.is_empty()
        || !cleanup.locals.is_empty()
        || !cleanup.actions.is_empty()
        || cleanup.code_offset != 0
        || cleanup.byte_count != function.bytes.len()
        || attribution.site != SemanticCodeSite::Edge(*return_edge)
        || attribution.operation_ordinal != 0
        || attribution.code_offset != 0
        || attribution.byte_count != function.bytes.len()
    {
        return Err("selected-lowering machine function is outside the exact return-only cohort");
    }
    Ok(())
}

fn validate_leaf_object(object: &image_emission::ObjectArtifact) -> Result<(), &'static str> {
    let shape = cohort_return_shape(object.target())?;
    if object.functions().is_empty()
        || !object.private_functions().is_empty()
        || object.relocations().record_count() != 0
        || !object.data_bytes().is_empty()
        || !object.dynamic_conformance_tables().is_empty()
        || !object.forwarded_dynamic_descriptor_adapters().is_empty()
        || !object.forwarded_dynamic_descriptor_tables().is_empty()
        || !object.port_effects().is_empty()
        || !object.boundary_settlements().is_empty()
        || !object.foreign_calls().is_empty()
        || object.x86_feature_profile().is_some()
        || object.x86_scalar_fma_provider().is_some()
    {
        return Err("selected-lowering object is outside the exact return-only cohort");
    }

    let mut previous = None;
    let mut entry_count = 0usize;
    let mut expected_text_offset = 0usize;
    for function in object.functions() {
        if previous.is_some_and(|previous| previous >= function.machine) {
            return Err("selected-lowering object functions are not in canonical order");
        }
        previous = Some(function.machine);
        entry_count += usize::from(function.machine == object.entry());
        if function.text_offset != expected_text_offset {
            return Err("selected-lowering object functions are not contiguously ordered");
        }
        expected_text_offset = expected_text_offset
            .checked_add(function.byte_count)
            .ok_or("selected-lowering object text size overflows")?;
        if function.scalar_stack.is_some() {
            scalar::validate_object_function(object, function)?;
        } else {
            validate_return_only_object_function(object, function, &shape)?;
        }
    }
    if entry_count != 1 || expected_text_offset != object.text_bytes().len() {
        return Err(
            "selected-lowering object does not retain one exact entry/function text roster",
        );
    }
    Ok(())
}

fn validate_return_only_object_function(
    object: &image_emission::ObjectArtifact,
    function: &image_emission::ObjectFunction,
    shape: &ReturnOnlyShape,
) -> Result<(), &'static str> {
    let [return_edge] = function.provenance.edges.as_slice() else {
        return Err("selected-lowering object function requires one Terminal return edge");
    };
    let Some(stack) = function.unit_stack else {
        return Err("selected-lowering object function requires Unit stack evidence");
    };
    let Some(cleanup) = function.unit_affine_cleanup.as_ref() else {
        return Err("selected-lowering object function requires Unit cleanup custody");
    };
    let attributions = object
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| attribution.machine == function.machine)
        .collect::<Vec<_>>();
    let [attribution] = attributions.as_slice() else {
        return Err("selected-lowering object function requires one edge attribution");
    };
    if !function.provenance.operations.is_empty()
        || function.bytes(object) != shape.bytes
        || function.fixed_integer_scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || function.structural_call_scalar_return.is_some()
        || function.unit_scalar_abi.is_some()
        || !function.x86_scalar_fma.is_empty()
        || !function.x86_scalar_fma_occurrences.is_empty()
        || function.x86_floating_control.is_some()
        || stack.frame_bytes != shape.frame_bytes()
        || stack.local_peak_bytes != shape.frame_bytes()
        || stack.stack_alignment != 16
        || function.scalar_stack.is_some()
        || !function.unit_call_stacks.is_empty()
        || !function.scalar_call_stacks.is_empty()
        || !function.internal_unit_calls.is_empty()
        || !function.internal_unit_scalar_calls.is_empty()
        || !function.installed_provider_unit_scalar_calls.is_empty()
        || !function.dynamic_calls.is_empty()
        || !function.stored_dynamic_calls.is_empty()
        || !function.dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_descriptor_calls.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
        || !function.unit_write_only_primitive_stores.is_empty()
        || !function.scalar_structural_scalar_field_stores.is_empty()
        || !function.unit_parameters.is_empty()
        || !function.unit_parameter_homes.is_empty()
        || function.scalar_affine_cleanup.is_some()
        || !function.scalar_control_affine_cleanups.is_empty()
        || !function.scalar_structural_parameters.is_empty()
        || !function.scalar_structural_parameter_homes.is_empty()
        || function.ranked_u32_countdown.is_some()
        || function.structural_return.is_some()
        || cleanup.psi_edge != *return_edge
        || !cleanup.structural_types.is_empty()
        || !cleanup.locals.is_empty()
        || !cleanup.actions.is_empty()
        || cleanup.code_offset != 0
        || cleanup.byte_count != function.byte_count
        || attribution.attribution.site != SemanticCodeSite::Edge(*return_edge)
        || attribution.attribution.operation_ordinal != 0
        || attribution.attribution.code_offset != 0
        || attribution.attribution.byte_count != function.byte_count
        || attribution.text_offset != function.text_offset
    {
        return Err("selected-lowering object function is outside the exact return-only cohort");
    }
    Ok(())
}

/// Independent exact-byte and stack check for return-only publication. AArch64
/// uses the same sixteen-byte saved-link frame as ordinary Unit emission;
/// selected execution cannot change the calling convention to a bare return.
struct ReturnOnlyShape {
    bytes: &'static [u8],
    stack: UnitStackEvidence,
}

impl ReturnOnlyShape {
    fn frame_bytes(&self) -> u32 {
        self.stack.frame.map_or(0, |frame| frame.byte_size)
    }
}

fn cohort_return_shape(target: NativeTarget) -> Result<ReturnOnlyShape, &'static str> {
    if target.pointer_size != 8 || target.pointer_alignment != 8 {
        return Err(
            "selected-lowering return-only publication requires an eight-byte pointer target",
        );
    }
    Ok(match target.architecture {
        Architecture::X86_64 => ReturnOnlyShape {
            bytes: &[0xc3],
            stack: UnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            },
        },
        Architecture::Aarch64 => ReturnOnlyShape {
            // sub sp, sp, #16; str x30, [sp]; ldr x30, [sp];
            // add sp, sp, #16; ret
            bytes: &[
                0xff, 0x43, 0x00, 0xd1, 0xfe, 0x03, 0x00, 0xf9, 0xfe, 0x03, 0x40, 0xf9, 0xff, 0x43,
                0x00, 0x91, 0xc0, 0x03, 0x5f, 0xd6,
            ],
            stack: UnitStackEvidence {
                frame: Some(StackAdjustmentPair {
                    byte_size: 16,
                    allocation_offset: 0,
                    allocation_byte_count: 4,
                    release_offset: 12,
                    release_byte_count: 4,
                }),
                aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                    frame_byte_offset: 0,
                    store_offset: 4,
                    load_offset: 8,
                }),
                stack_alignment: 16,
            },
        },
    })
}

fn machine_projection_from_plan(plan: &MachineCodePlan) -> [u8; 32] {
    let mut digest =
        machine_projection_hasher(plan.psi, plan.target, plan.entry, plan.functions.len());
    for function in &plan.functions {
        hash_function_projection(
            &mut digest,
            function.machine,
            function.attachment,
            &function.provenance,
            &function.bytes,
        );
        if let Some(abi) = &function.fixed_integer_scalar_abi {
            scalar::hash_projection(&mut digest, abi, function.semantic_code_attribution.iter());
        }
    }
    digest.finalize().into()
}

fn machine_projection_from_object(
    object: &image_emission::ObjectArtifact,
) -> Result<[u8; 32], &'static str> {
    let mut digest = machine_projection_hasher(
        object.psi(),
        object.target(),
        object.entry(),
        object.functions().len(),
    );
    for function in object.functions() {
        hash_function_projection(
            &mut digest,
            function.machine,
            function.attachment,
            &function.provenance,
            function.bytes(object),
        );
        if let Some(abi) = &function.fixed_integer_scalar_abi {
            scalar::hash_projection(
                &mut digest,
                abi,
                object
                    .semantic_code_attribution()
                    .iter()
                    .filter(|row| row.machine == function.machine)
                    .map(|row| &row.attribution),
            );
        }
    }
    Ok(digest.finalize().into())
}

fn machine_projection_hasher(
    terminal: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    function_count: usize,
) -> Sha256 {
    let mut digest = Sha256::new();
    digest.update(SELECTED_LOWERING_MACHINE_PROJECTION_DOMAIN);
    digest.update(terminal.vocabulary_marker.get().to_le_bytes());
    digest.update(terminal.program_fingerprint.as_bytes());
    hash_target(&mut digest, target);
    digest.update(entry.get().to_le_bytes());
    digest.update(canonical_usize(function_count));
    digest
}

fn hash_function_projection(
    digest: &mut Sha256,
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
) {
    digest.update(machine.get().to_le_bytes());
    match attachment {
        None => digest.update([0]),
        Some(attachment) => {
            digest.update([1]);
            digest.update(attachment.get().to_le_bytes());
        }
    }
    digest.update(canonical_usize(provenance.operations.len()));
    for operation in &provenance.operations {
        digest.update(operation.get().to_le_bytes());
    }
    digest.update(canonical_usize(provenance.edges.len()));
    for edge in &provenance.edges {
        digest.update(edge.get().to_le_bytes());
    }
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
}

fn hash_target(digest: &mut Sha256, target: NativeTarget) {
    digest.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    digest.update(canonical_usize(target.pointer_size));
    digest.update(canonical_usize(target.pointer_alignment));
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("selected-lowering native projection length fits u64")
        .to_le_bytes()
}

#[cfg(test)]
mod tests {
    use machine_code::{
        MachineCodeFunction, SemanticCodeAttribution, UnitAffineCleanupRecord, UnitStackEvidence,
    };
    use optimization_core::{
        FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
        FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
        PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use semantic_vocabulary::{EdgeId, MachineId};
    use target_operations::TerminalPsiProvenance;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;

    fn terminal() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
        }
    }

    fn plan(machine_raw: u64) -> MachineCodePlan {
        let machine = MachineId::new(machine_raw).expect("nonzero machine");
        let return_edge = EdgeId::new(7).expect("nonzero edge");
        MachineCodePlan {
            psi: terminal(),
            target: NativeTarget::linux_x64(),
            entry: machine,
            functions: vec![MachineCodeFunction {
                machine,
                attachment: None,
                fixed_integer_scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                unit_scalar_abi: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![return_edge],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: None,
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
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
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    psi_edge: return_edge,
                    structural_types: Vec::new(),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 0,
                    byte_count: 1,
                }),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                ranked_u32_countdown: None,
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(return_edge),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                structural_return: None,
            }],
        }
    }

    fn input<'plan>(
        plan: &'plan MachineCodePlan,
        selection_marker: u8,
    ) -> SelectedLoweringNativePublicationInput<'plan> {
        SelectedLoweringNativePublicationInput::new(
            OptimizationSelectionIdentity::from_bytes([selection_marker; 32]),
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"completion"),
            PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pre-physical"),
            PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"post-allocation"),
            FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                b"function-relative",
            ),
            FunctionFragmentEmissionIdentity::from_canonical_bytes(b"fragments"),
            FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"fragment manifest"),
            plan,
        )
    }

    #[test]
    fn selected_lowering_binding_identity_changes_with_lineage() {
        let plan = plan(1);
        let baseline = derive_selected_lowering_publication_binding(terminal(), input(&plan, 3))
            .expect("exact return-only binding");
        let changed = derive_selected_lowering_publication_binding(terminal(), input(&plan, 5))
            .expect("changed-lineage return-only binding");

        assert_ne!(baseline.identity(), changed.identity());
        assert_eq!(baseline.machine_projection(), changed.machine_projection());
    }

    #[test]
    fn aarch64_publication_requires_exact_saved_link_bytes_and_custody() {
        let mut admitted = plan(1);
        admitted.target = NativeTarget::linux_arm64();
        let shape = cohort_return_shape(admitted.target).unwrap();
        let function = &mut admitted.functions[0];
        function.bytes = shape.bytes.to_vec();
        function.unit_stack = Some(shape.stack);
        function.unit_affine_cleanup.as_mut().unwrap().byte_count = shape.bytes.len();
        function.semantic_code_attribution[0].byte_count = shape.bytes.len();
        let binding = derive_selected_lowering_publication_binding(terminal(), input(&admitted, 3))
            .expect("saved-link return publication");
        let object = image_emission::build_object_artifact(&admitted).unwrap();
        validate_selected_lowering_publication_object(&binding, &object).unwrap();

        for corruption in 0..7 {
            let mut changed = admitted.clone();
            let function = &mut changed.functions[0];
            let stack = function.unit_stack.as_mut().unwrap();
            match corruption {
                0 => stack.frame = None,
                1 => stack.aarch64_return_link = None,
                2 => stack.frame.as_mut().unwrap().byte_size = 32,
                3 => stack.frame.as_mut().unwrap().release_offset = 8,
                4 => {
                    stack
                        .aarch64_return_link
                        .as_mut()
                        .unwrap()
                        .frame_byte_offset = 8
                }
                5 => function.bytes[4] ^= 1,
                6 => function.bytes[12] ^= 1,
                _ => unreachable!(),
            }
            assert!(
                derive_selected_lowering_publication_binding(terminal(), input(&changed, 3))
                    .is_err(),
                "frame corruption {corruption} must reject before publication"
            );
            assert!(
                image_emission::build_object_artifact(&changed).is_err(),
                "object replay must independently reject frame corruption {corruption}"
            );
        }
    }

    #[test]
    fn selected_lowering_object_must_replay_the_exact_machine_projection() {
        let admitted_plan = plan(1);
        let binding =
            derive_selected_lowering_publication_binding(terminal(), input(&admitted_plan, 3))
                .expect("exact return-only binding");
        let matching_object = image_emission::build_object_artifact(&admitted_plan)
            .expect("matching return-only object");
        assert!(validate_selected_lowering_publication_object(&binding, &matching_object).is_ok());

        let different_plan = plan(2);
        let different_object = image_emission::build_object_artifact(&different_plan)
            .expect("different valid return-only object");
        assert_eq!(
            validate_selected_lowering_publication_object(&binding, &different_object),
            Err("selected-lowering object disagrees with the exact admitted machine projection")
        );
    }

    fn scalar_plan() -> MachineCodePlan {
        use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
        use machine_code::{ScalarControlFlowEvidence, ScalarStackEvidence};
        use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ValueId};
        use target_operations::{FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi};

        let mut plan = plan(1);
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(plan.target),
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let function = &mut plan.functions[0];
        function.fixed_integer_scalar_abi = Some(FixedIntegerScalarFunctionAbi {
            parameters: Vec::new(),
            result: FixedIntegerScalarAbiValue {
                value: ValueId::new(9).unwrap(),
                scalar_type,
                placement: call_plan.result.clone().unwrap(),
            },
            call_plan,
        });
        // mov eax, 7; ret -- writing EAX zero-extends the U64 ABI result.
        function.bytes = vec![0xb8, 7, 0, 0, 0, 0xc3];
        function.unit_stack = None;
        function.unit_affine_cleanup = None;
        function.scalar_stack = Some(ScalarStackEvidence {
            mutations: Vec::new(),
            control_flow: ScalarControlFlowEvidence::Linear,
            stack_alignment: 16,
            cleanup_preservation: None,
        });
        let operation = OperationId::new(11).unwrap();
        function.provenance.operations = vec![operation];
        function.semantic_code_attribution = vec![
            SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(operation),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 5,
            },
            SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(function.provenance.edges[0]),
                operation_ordinal: 1,
                code_offset: 5,
                byte_count: 1,
            },
        ];
        plan
    }

    #[test]
    fn scalar_leaf_publication_binds_abi_values_attribution_and_bytes() {
        let plan = scalar_plan();
        let binding =
            derive_selected_lowering_publication_binding(terminal(), input(&plan, 3)).unwrap();
        let object = image_emission::build_object_artifact(&plan).unwrap();
        validate_selected_lowering_publication_object(&binding, &object).unwrap();

        for corruption in 0..4 {
            let mut changed = plan.clone();
            let function = &mut changed.functions[0];
            match corruption {
                0 => {
                    function
                        .fixed_integer_scalar_abi
                        .as_mut()
                        .unwrap()
                        .result
                        .value = semantic_vocabulary::ValueId::new(10).unwrap()
                }
                1 => function.semantic_code_attribution[1].operation_ordinal = 2,
                2 => function.semantic_code_attribution[0].byte_count = 4,
                3 => function.bytes[1] = 8,
                _ => unreachable!(),
            }
            let changed_binding =
                derive_selected_lowering_publication_binding(terminal(), input(&changed, 3))
                    .unwrap();
            assert_ne!(
                binding.machine_projection(),
                changed_binding.machine_projection(),
                "scalar coordinate {corruption}"
            );
            let changed_object = image_emission::build_object_artifact(&changed).unwrap();
            assert!(
                validate_selected_lowering_publication_object(&binding, &changed_object).is_err(),
                "scalar object substitution {corruption}"
            );
        }
    }

    #[test]
    fn scalar_leaf_publication_rejects_noncanonical_abi_and_stack_custody() {
        let plan = scalar_plan();
        for corruption in 0..6 {
            let mut changed = plan.clone();
            let function = &mut changed.functions[0];
            match corruption {
                0 => function
                    .fixed_integer_scalar_abi
                    .as_mut()
                    .unwrap()
                    .result
                    .placement
                    .locations
                    .clear(),
                1 => {
                    function
                        .fixed_integer_scalar_abi
                        .as_mut()
                        .unwrap()
                        .call_plan
                        .result = None
                }
                2 => function.scalar_stack.as_mut().unwrap().stack_alignment = 8,
                3 => function.bytes.pop().map(|_| ()).unwrap(),
                4 => function.bytes = vec![0x50, 0xb8, 7, 0, 0, 0, 0xc3],
                5 => {
                    function.semantic_code_attribution[1].site =
                        SemanticCodeSite::Edge(semantic_vocabulary::EdgeId::new(99).unwrap())
                }
                _ => unreachable!(),
            }
            assert!(
                derive_selected_lowering_publication_binding(terminal(), input(&changed, 3))
                    .is_err(),
                "scalar corruption {corruption}"
            );
        }
    }

    #[test]
    fn scalar_leaf_publication_accepts_both_instruction_architectures() {
        use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};

        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let mut plan = scalar_plan();
            plan.target = target;
            let function = &mut plan.functions[0];
            let abi = function.fixed_integer_scalar_abi.as_mut().unwrap();
            abi.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: Vec::new(),
                    result: Some(ValueShape::integer(8, 8)),
                },
            )
            .unwrap();
            abi.result.placement = abi.call_plan.result.clone().unwrap();
            if target.architecture == Architecture::Aarch64 {
                // movz x0, #7; ret -- a scalar leaf preserves incoming x30.
                function.bytes = vec![0xe0, 0x00, 0x80, 0xd2, 0xc0, 0x03, 0x5f, 0xd6];
                function.semantic_code_attribution[0].byte_count = 4;
                function.semantic_code_attribution[1].code_offset = 4;
                function.semantic_code_attribution[1].byte_count = 4;
            }
            let binding =
                derive_selected_lowering_publication_binding(terminal(), input(&plan, 3)).unwrap();
            let object = image_emission::build_object_artifact(&plan).unwrap();
            validate_selected_lowering_publication_object(&binding, &object).unwrap();
        }
    }

    #[test]
    fn unit_machine_projection_retains_its_original_identity_coordinates() {
        let plan = plan(1);
        let function = &plan.functions[0];
        let mut original =
            machine_projection_hasher(plan.psi, plan.target, plan.entry, plan.functions.len());
        hash_function_projection(
            &mut original,
            function.machine,
            function.attachment,
            &function.provenance,
            &function.bytes,
        );
        let original: [u8; 32] = original.finalize().into();
        assert_eq!(machine_projection_from_plan(&plan), original);
    }
}
