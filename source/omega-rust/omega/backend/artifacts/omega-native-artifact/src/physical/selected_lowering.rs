use omega_machine_code::{MachineCodeFunction, MachineCodePlan, SemanticCodeSite};
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{MachineId, StructuralTypeId};
use psi_terminal::TerminalPsiIdentity;
use sha2::{Digest, Sha256};

const SELECTED_LOWERING_BINDING_DOMAIN: &[u8] =
    b"omega.native-artifact.selected-lowering-publication-binding.sha256.v1\0";
const SELECTED_LOWERING_MACHINE_PROJECTION_DOMAIN: &[u8] =
    b"omega.native-artifact.selected-lowering-machine-projection.sha256.v1\0";

/// Exact optimizer lineage and machine plan admitted by the first
/// selected-lowering native-publication cohort.
///
/// This carrier is deliberately narrower than a general physical-pipeline
/// descriptor. It binds one already validated, spill-free, return-only plan;
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
    validate_return_only_machine_plan(input.machine_code)?;
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
    object: &omega_image_emission::ObjectArtifact,
) -> Result<(), &'static str> {
    validate_return_only_object(object)?;
    if machine_projection_from_object(object)? != *binding.machine_projection() {
        return Err(
            "selected-lowering object disagrees with the exact admitted machine projection",
        );
    }
    Ok(())
}

fn validate_return_only_machine_plan(plan: &MachineCodePlan) -> Result<(), &'static str> {
    validate_cohort_target(plan.target)?;
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
        validate_return_only_machine_function(function)?;
    }
    if entry_count != 1 {
        return Err("selected-lowering machine plan does not contain exactly one entry");
    }
    Ok(())
}

fn validate_return_only_machine_function(
    function: &MachineCodeFunction,
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
        || function.bytes.as_slice() != [0xc3]
        || function.fixed_integer_scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || function.structural_call_scalar_return.is_some()
        || function.unit_scalar_abi.is_some()
        || !function.x86_scalar_fma.is_empty()
        || !function.x86_scalar_fma_occurrences.is_empty()
        || function.x86_floating_control.is_some()
        || stack.frame.is_some()
        || stack.aarch64_return_link.is_some()
        || stack.stack_alignment != 16
        || !function.unit_parameter_homes.is_empty()
        || !function.unit_parameters.is_empty()
        || function.scalar_stack.is_some()
        || !function.internal_calls.is_empty()
        || !function.foreign_calls.is_empty()
        || !function.internal_unit_calls.is_empty()
        || !function.internal_unit_scalar_calls.is_empty()
        || !function.installed_provider_unit_scalar_calls.is_empty()
        || !function.dynamic_calls.is_empty()
        || !function.dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_descriptor_calls.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
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

fn validate_return_only_object(
    object: &omega_image_emission::ObjectArtifact,
) -> Result<(), &'static str> {
    validate_cohort_target(object.target())?;
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
        validate_return_only_object_function(object, function)?;
    }
    if entry_count != 1 || expected_text_offset != object.text_bytes().len() {
        return Err(
            "selected-lowering object does not retain one exact entry/function text roster",
        );
    }
    Ok(())
}

fn validate_return_only_object_function(
    object: &omega_image_emission::ObjectArtifact,
    function: &omega_image_emission::ObjectFunction,
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
        || function.bytes(object) != [0xc3]
        || function.fixed_integer_scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || function.structural_call_scalar_return.is_some()
        || function.unit_scalar_abi.is_some()
        || !function.x86_scalar_fma.is_empty()
        || !function.x86_scalar_fma_occurrences.is_empty()
        || function.x86_floating_control.is_some()
        || stack.frame_bytes != 0
        || stack.local_peak_bytes != 0
        || stack.stack_alignment != 16
        || function.scalar_stack.is_some()
        || !function.unit_call_stacks.is_empty()
        || !function.scalar_call_stacks.is_empty()
        || !function.internal_unit_calls.is_empty()
        || !function.internal_unit_scalar_calls.is_empty()
        || !function.installed_provider_unit_scalar_calls.is_empty()
        || !function.dynamic_calls.is_empty()
        || !function.dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_descriptor_calls.is_empty()
        || !function.unit_scalar_homes.is_empty()
        || !function.unit_integer_constants.is_empty()
        || !function.unit_affine_scalar_records.is_empty()
        || !function.unit_structural_scalar_field_stores.is_empty()
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

fn validate_cohort_target(target: NativeTarget) -> Result<(), &'static str> {
    if target.architecture != Architecture::X86_64
        || target.pointer_size != 8
        || target.pointer_alignment != 8
    {
        return Err("selected-lowering return-only publication currently requires x86-64");
    }
    Ok(())
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
    }
    digest.finalize().into()
}

fn machine_projection_from_object(
    object: &omega_image_emission::ObjectArtifact,
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
    use omega_machine_code::{
        MachineCodeFunction, SemanticCodeAttribution, UnitAffineCleanupRecord, UnitStackEvidence,
    };
    use omega_optimization_core::{
        FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
        FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
        PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{EdgeId, MachineId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
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
    fn selected_lowering_object_must_replay_the_exact_machine_projection() {
        let admitted_plan = plan(1);
        let binding =
            derive_selected_lowering_publication_binding(terminal(), input(&admitted_plan, 3))
                .expect("exact return-only binding");
        let matching_object = omega_image_emission::build_object_artifact(&admitted_plan)
            .expect("matching return-only object");
        assert!(validate_selected_lowering_publication_object(&binding, &matching_object).is_ok());

        let different_plan = plan(2);
        let different_object = omega_image_emission::build_object_artifact(&different_plan)
            .expect("different valid return-only object");
        assert_eq!(
            validate_selected_lowering_publication_object(&binding, &different_object),
            Err("selected-lowering object disagrees with the exact admitted machine projection")
        );
    }
}
