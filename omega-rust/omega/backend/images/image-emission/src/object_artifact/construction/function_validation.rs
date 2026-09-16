//! The validating pass over every retained machine-code function: each
//! function's records are replayed against its final bytes before the object
//! is laid out. `validate_functions` walks the plan in canonical order and
//! keeps the validated stacks the later emission phases consume.

use crate::object_artifact::call_sites::{
    validate_foreign_call_floating_control, validate_foreign_call_site,
    validate_foreign_scalar_arguments,
};
use crate::object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
use crate::object_artifact::replay::boundary::completion_receipts::{
    CompletionCustodyError, validate_completion_custody,
};
use crate::object_artifact::replay::boundary::result_placement::boundary_result_is_exact;
use crate::object_artifact::replay::boundary::runtime_scalar_custody::hosted_write_byte_custody_is_exact;
use crate::object_artifact::replay::dynamic::conformance::{
    validate_dynamic_calls, validate_stored_dynamic_calls,
};
use crate::object_artifact::replay::scalar::cleanup_preservation::validate_scalar_cleanup_preservation;
use crate::object_artifact::replay::scalar::conditional_call_paths::{
    conditional_call_path, conditional_paths_are_exclusive,
};
use crate::object_artifact::replay::scalar::control_cleanup::{
    cleanup_for_owner, validate_scalar_control_cleanup_evidence,
};
use crate::object_artifact::replay::scalar::stack::validate_scalar_stack;
use crate::object_artifact::replay::scalar::structural_scalar_field_store::validate_scalar_structural_scalar_field_stores;
use crate::object_artifact::replay::structural::affine_projected_calls::{
    exact_fully_consumed_affine_parameter, exact_partially_consumed_affine_parameter,
};
use crate::object_artifact::replay::structural::return_record::validate_structural_return_record;
use crate::object_artifact::replay::unit::affine_cleanup::validate_unit_affine_cleanup;
use crate::object_artifact::replay::unit::call_custody::{
    structural_result_matches_return, validate_internal_unit_call_custody,
    validate_mixed_structural_scalar_abi, validate_unit_affine_scalar_records,
};
use crate::object_artifact::replay::unit::dynamic_descriptor_join::validate_unit_dynamic_descriptor_join;
use crate::object_artifact::replay::unit::installed_provider_scalar_call::validate_installed_provider_unit_scalar_calls;
use crate::object_artifact::replay::unit::scalar_call_custody::validate_internal_unit_scalar_calls;
use crate::object_artifact::replay::unit::stack::{
    validate_complete_unit_stack_evidence, validate_foreign_unit_call_stack,
    validate_unit_call_stack, validate_unit_function_stack,
};
use crate::object_artifact::replay::unit::structural_scalar_field_store::validate_unit_structural_scalar_field_stores;
use crate::object_artifact::replay::unit::write_only_primitive_store::validate_unit_write_only_primitive_stores;
use crate::object_artifact::{
    ObjectError, ObjectScalarCallStack, ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack,
};
use machine_code::MachineCodeFunction;
use machine_code::{MachineCodePlan, SemanticCodeSite};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget};
use target_operations::{BoundaryRealization, CallSiteOwner};

/// What the validating pass leaves for emission: the total text size and the
/// validated Unit, scalar and foreign-call stacks keyed by function.
pub(super) struct FunctionValidation {
    pub(super) text_size: usize,
    pub(super) validated_unit_stacks:
        std::collections::BTreeMap<MachineId, (ObjectUnitStack, Vec<ObjectUnitCallStack>)>,
    pub(super) validated_scalar_stacks:
        std::collections::BTreeMap<MachineId, (ObjectScalarStack, Vec<ObjectScalarCallStack>)>,
    pub(super) validated_foreign_call_stacks:
        std::collections::BTreeMap<(MachineId, CallSiteOwner), u32>,
}

/// Replay every function's retained records against its bytes, in canonical
/// order: shape and ordering, foreign calls, stack and call custody, semantic
/// attribution, port effects, then boundary settlements.
pub(super) fn validate_functions(
    plan: &MachineCodePlan,
    x86_feature_profile: Option<target::TargetProfile>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<FunctionValidation, ObjectError> {
    let mut previous = None;
    let mut saw_entry = false;
    let mut text_size = 0usize;
    let mut validated_unit_stacks = std::collections::BTreeMap::new();
    let mut validated_scalar_stacks = std::collections::BTreeMap::new();
    let mut validated_foreign_call_stacks = std::collections::BTreeMap::new();
    let attachments = plan
        .functions
        .iter()
        .map(|function| (function.machine, function.attachment))
        .collect::<std::collections::BTreeMap<_, _>>();
    let machine_functions = plan
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    for function in &plan.functions {
        validate_function_shape(
            plan,
            function,
            previous,
            x86_feature_profile,
            x86_scalar_fma_provider,
        )?;
        validate_foreign_calls(plan, function)?;
        let mut validated_function_stack = function
            .unit_stack
            .map(|stack| {
                validate_unit_function_stack(
                    plan.target.architecture,
                    function.machine,
                    &function.bytes,
                    stack,
                    0,
                )
            })
            .transpose()?;
        if function.boundary_settlements.iter().any(|settlement| {
            hosted_write_byte_custody_is_exact(
                plan.target,
                settlement,
                &function.boundary_settlements,
                &function.unit_integer_constants,
                &function.unit_scalar_homes,
                |home, consumer_ordinal, consumer_offset| {
                    crate::object_artifact::replay::unit::scalar_call_custody::exact_preceding_internal_unit_scalar_home_producer_count(
                        &function.internal_unit_scalar_calls,
                        home,
                        consumer_ordinal,
                        consumer_offset,
                    )
                },
                Some(&function.bytes),
            )
        }) {
            let stack = validated_function_stack
                .as_mut()
                .ok_or(ObjectError::UnaccountedTerminalStack(function.machine))?;
            stack.local_peak_bytes = stack
                .frame_bytes
                .checked_add(16)
                .ok_or(ObjectError::UnaccountedTerminalStack(function.machine))?;
        }
        if function.unit_stack.is_some() && function.scalar_stack.is_some() {
            return Err(ObjectError::ConflictingTerminalStackEvidence(
                function.machine,
            ));
        }
        if let Some(returned) = function.structural_call_scalar_return
            && !(function.unit_stack.is_some()
                && function.scalar_stack.is_none()
                && function.provenance.operations.as_slice() == [returned.psi_operation]
                && function.provenance.edges.as_slice() == [returned.psi_edge]
                && matches!(
                    (
                        function.internal_unit_calls.as_slice(),
                        function.semantic_code_attribution.as_slice(),
                        function.unit_affine_cleanup.as_ref(),
                    ),
                    ([call], [call_attribution, return_attribution], Some(cleanup))
                        if call.owner == CallSiteOwner::Operation(returned.psi_operation)
                            && call.target == returned.callee
                            && call.operation_ordinal == 0
                            && call.result == Some(returned.scalar_type)
                            && call.semantic_result.as_ref().is_some_and(|result| {
                                result.value == returned.source_value
                                    && result.scalar_type == returned.scalar_type
                            })
                            && call_attribution.site
                                == SemanticCodeSite::Operation(returned.psi_operation)
                            && call_attribution.operation_ordinal == call.operation_ordinal
                            && call_attribution.code_offset == call.code_offset
                            && call_attribution.byte_count == call.byte_count
                            && return_attribution.site == SemanticCodeSite::Edge(returned.psi_edge)
                            && return_attribution.operation_ordinal == 1
                            && return_attribution.code_offset == cleanup.code_offset
                            && return_attribution.byte_count == cleanup.byte_count
                            && cleanup.psi_edge == returned.psi_edge
                            && cleanup.locals.is_empty()
                            && cleanup.actions.is_empty()
                ))
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        if let Some(returned) = &function.structural_return {
            validate_structural_return_record(
                plan.target,
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                returned,
            )?;
            if function.unit_stack.is_some()
                || function.scalar_stack.is_some()
                || !function.internal_calls.is_empty()
                || !function.port_effects.is_empty()
                || !function.boundary_settlements.is_empty()
            {
                return Err(ObjectError::StructuralReturnEvidenceConflict(
                    function.machine,
                ));
            }
        }
        if let Some(stack) = &function.scalar_stack {
            validate_scalar_cleanup_preservation(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                stack,
                function.scalar_affine_cleanup.as_ref(),
            )?;
            validate_scalar_control_cleanup_evidence(
                plan.target.architecture,
                function.machine,
                &function.provenance,
                &function.bytes,
                stack,
                &function.scalar_control_affine_cleanups,
            )?;
            validated_scalar_stacks.insert(
                function.machine,
                validate_scalar_stack(
                    plan.target.architecture,
                    function.machine,
                    &function.bytes,
                    &function.internal_calls,
                    &function.dynamic_parameter_calls,
                    &function.provenance,
                    &function.semantic_code_attribution,
                    stack,
                    function.scalar_affine_cleanup.as_ref(),
                    &function.scalar_control_affine_cleanups,
                    &function.scalar_structural_parameter_homes,
                )?,
            );
        }
        let mut validated_call_stacks = Vec::new();
        let mut call_owner_paths =
            std::collections::BTreeMap::<CallSiteOwner, Vec<Option<Vec<(usize, bool)>>>>::new();
        for call in &function.internal_calls {
            let owner_in_provenance = match call.owner {
                CallSiteOwner::Operation(operation) => {
                    function.provenance.operations.contains(&operation)
                }
                CallSiteOwner::CleanupAction { edge, .. } => {
                    function.provenance.edges.contains(&edge)
                }
            };
            if !owner_in_provenance {
                return Err(ObjectError::InternalCallOperationNotInProvenance {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            let path = conditional_call_path(
                plan.target.architecture,
                &function.bytes,
                function.scalar_stack.as_ref(),
                call,
            );
            let prior_paths = call_owner_paths.entry(call.owner).or_default();
            if !prior_paths.is_empty()
                && (!matches!(call.owner, CallSiteOwner::Operation(_))
                    || path.as_ref().is_none_or(|path| {
                        prior_paths.iter().any(|prior| {
                            prior
                                .as_ref()
                                .is_none_or(|prior| !conditional_paths_are_exclusive(prior, path))
                        })
                    }))
            {
                return Err(ObjectError::DuplicateInternalCallOperation {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            prior_paths.push(path);
            match (function.unit_stack, call.unit_stack) {
                (Some(_), Some(call_stack)) => {
                    let validated = validate_unit_call_stack(
                        plan.target.architecture,
                        function.machine,
                        &function.bytes,
                        *call,
                        function.unit_stack.expect("Unit stack evidence exists"),
                        validated_function_stack.expect("validated Unit stack exists"),
                        call_stack,
                    )?;
                    let function_stack = validated_function_stack
                        .as_mut()
                        .expect("validated Unit stack exists");
                    function_stack.local_peak_bytes = function_stack
                        .local_peak_bytes
                        .max(validated.caller_live_bytes);
                    validated_call_stacks.push(validated);
                }
                (Some(_), None) => {
                    return Err(ObjectError::MissingUnitCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, Some(_)) => {
                    return Err(ObjectError::UnexpectedUnitCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, None) => {}
            }
            match (function.scalar_stack.as_ref(), call.scalar_stack) {
                (Some(_), Some(_)) => {}
                (Some(_), None) => {
                    return Err(ObjectError::MissingScalarCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, Some(_)) => {
                    return Err(ObjectError::UnexpectedScalarCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, None) => {}
            }
        }
        for call in &function.foreign_calls {
            let Some(stack) = function.unit_stack else {
                return Err(ObjectError::UnexpectedUnitCallStackEvidence {
                    caller: function.machine,
                    owner: call.owner,
                });
            };
            let caller_live_bytes = validate_foreign_unit_call_stack(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                call,
                stack,
                validated_function_stack.expect("validated Unit stack exists"),
            )?;
            let function_stack = validated_function_stack
                .as_mut()
                .expect("validated Unit stack exists");
            let admitted_alignment = call.same_stack_contribution.alignment();
            if admitted_alignment > u64::from(function_stack.stack_alignment) {
                return Err(ObjectError::UnsupportedForeignStackAlignment {
                    caller: function.machine,
                    owner: call.owner,
                    admitted_alignment,
                    physical_alignment: function_stack.stack_alignment,
                });
            }
            function_stack.local_peak_bytes =
                function_stack.local_peak_bytes.max(caller_live_bytes);
            validated_foreign_call_stacks.insert((function.machine, call.owner), caller_live_bytes);
        }
        let is_unit_custody_relocation = |call: &&machine_code::InternalCallRelocation| {
            call.unit_stack.is_some()
                || ((function.scalar_affine_cleanup.is_some()
                    || cleanup_for_owner(&function.scalar_control_affine_cleanups, call.owner)
                        .is_some())
                    && matches!(call.owner, CallSiteOwner::CleanupAction { .. })
                    && call.scalar_stack.is_some())
        };
        let unit_custody_count = function
            .internal_unit_calls
            .len()
            .checked_add(function.internal_unit_scalar_calls.len())
            .and_then(|count| count.checked_add(function.forwarded_dynamic_descriptor_calls.len()))
            .and_then(|count| {
                count.checked_add(
                    function
                        .forwarded_dynamic_parameter_calls
                        .iter()
                        .filter(|call| {
                            matches!(
                                call.call_stack,
                                machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(_)
                            )
                        })
                        .count(),
                )
            })
            .and_then(|count| {
                count.checked_add(function.installed_provider_unit_scalar_calls.len())
            })
            .ok_or(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ))?;
        if unit_custody_count
            != function
                .internal_calls
                .iter()
                .filter(is_unit_custody_relocation)
                .count()
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        let relocation_identities = function
            .internal_calls
            .iter()
            .filter(is_unit_custody_relocation)
            .map(|call| (call.owner, call.target))
            .collect::<std::collections::BTreeSet<_>>();
        let custody_identities = function
            .internal_unit_calls
            .iter()
            .map(|call| (call.owner, call.target))
            .chain(
                function
                    .internal_unit_scalar_calls
                    .iter()
                    .map(|call| (call.owner, call.target)),
            )
            .chain(
                function
                    .forwarded_dynamic_descriptor_calls
                    .iter()
                    .map(|call| (CallSiteOwner::Operation(call.psi_operation), call.callee)),
            )
            .chain(
                function
                    .forwarded_dynamic_parameter_calls
                    .iter()
                    .filter_map(|call| {
                        matches!(
                            call.call_stack,
                            machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(_)
                        )
                        .then_some((CallSiteOwner::Operation(call.psi_operation), call.callee))
                    }),
            )
            .chain(
                function
                    .installed_provider_unit_scalar_calls
                    .iter()
                    .map(|call| (call.owner, call.provider.candidate)),
            )
            .collect::<std::collections::BTreeSet<_>>();
        if custody_identities.len() != unit_custody_count
            || custody_identities != relocation_identities
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        let scalar_cleanup_custody = function.scalar_affine_cleanup.is_some()
            || !function.scalar_control_affine_cleanups.is_empty();
        let scalar_boundary_custody = function.boundary_settlements.iter().any(|settlement| {
            matches!(
                settlement.realization,
                BoundaryRealization::DirectPortReadU8(_)
                    | BoundaryRealization::HostedExitProcessI32(_)
            )
        });
        let scalar_custody = scalar_cleanup_custody
            || scalar_boundary_custody
            || function.mixed_structural_scalar_abi.is_some()
            || !function.scalar_structural_scalar_field_stores.is_empty();
        let parameter_homes = if scalar_custody {
            function.scalar_structural_parameter_homes.as_slice()
        } else {
            function.unit_parameter_homes.as_slice()
        };
        let default_affine_cleanup = if let Some(cleanup) = function.scalar_affine_cleanup.as_ref()
        {
            Some(cleanup)
        } else {
            function.unit_affine_cleanup.as_ref()
        };
        let continuation_discards =
            crate::object_artifact::replay::unit::continuations::validate_function(function)?;
        if !function.unit_continuations.is_empty()
            && crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::validate_shape(
                plan.target,
                parameter_homes,
                function.parameter_abi.as_ref(),
                true,
                validated_function_stack
                    .as_ref()
                    .map_or(0, |stack| stack.frame_bytes),
            )
            .is_none_or(|end| {
                validated_function_stack.as_ref().is_none_or(|stack| {
                    end > stack.frame_bytes
                        || (function
                            .internal_unit_calls
                            .iter()
                            .all(|call| call.structural_result.is_none())
                            && !crate::object_artifact::replay::unit::call_custody::result_home::exact_frame(
                                plan.target,
                                end,
                                stack.frame_bytes,
                                function
                                    .unit_stack
                                    .and_then(|stack| stack.aarch64_return_link)
                                    .map(|link| link.frame_byte_offset),
                            ))
                })
            })
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if !function.unit_continuations.is_empty()
            && !crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::exact_prologue(
                plan.target,
                &function.bytes,
                function.parameter_abi.as_ref(),
                parameter_homes,
                validated_function_stack
                    .as_ref()
                    .map_or(0, |stack| stack.frame_bytes),
                function
                    .semantic_code_attribution
                    .first()
                    .map_or(usize::MAX, |row| row.code_offset),
            )
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        let fully_consumed_affine_parameter = exact_fully_consumed_affine_parameter(
            parameter_homes,
            &function.internal_unit_calls,
            default_affine_cleanup,
        );
        let partially_consumed_affine_parameter = exact_partially_consumed_affine_parameter(
            parameter_homes,
            &function.internal_unit_calls,
            default_affine_cleanup,
        );
        for custody in &function.internal_unit_calls {
            let target_returns_scalar = machine_functions
                .get(&custody.target)
                .copied()
                .is_some_and(|target| {
                    target.scalar_stack.is_some() || target.structural_call_scalar_return.is_some()
                });
            let target_structural_return = machine_functions
                .get(&custody.target)
                .and_then(|target| target.structural_return.as_ref());
            let structural_result_valid =
                match (&custody.structural_result, target_structural_return) {
                    (None, None) => true,
                    (Some(result), Some(target)) => {
                        custody.result.is_none() && structural_result_matches_return(result, target)
                    }
                    _ => false,
                };
            if custody.result.is_some() != target_returns_scalar
                || custody
                    .semantic_result
                    .as_ref()
                    .map(|result| result.scalar_type)
                    != custody.result
                || !structural_result_valid
                || (custody.structural_result.is_some() && target_returns_scalar)
                || machine_functions
                    .get(&custody.target)
                    .is_some_and(|target| {
                        target
                            .structural_call_scalar_return
                            .is_some_and(|returned| custody.result != Some(returned.scalar_type))
                    })
            {
                return Err(ObjectError::InvalidInternalUnitCallEvidence(
                    function.machine,
                ));
            }
            let unit_call_stack = validated_call_stacks
                .iter()
                .find(|call| call.owner == custody.owner && call.target == custody.target);
            let scalar_call_stack =
                validated_scalar_stacks
                    .get(&function.machine)
                    .and_then(|(_, calls)| {
                        calls.iter().find(|call| {
                            call.owner == custody.owner && call.target == custody.target
                        })
                    });
            if unit_call_stack.is_none() == scalar_call_stack.is_none() {
                return Err(ObjectError::InvalidInternalUnitCallEvidence(
                    function.machine,
                ));
            }
            let affine_cleanup =
                cleanup_for_owner(&function.scalar_control_affine_cleanups, custody.owner)
                    .or_else(|| {
                        crate::object_artifact::replay::unit::continuations::cleanup_for_call(
                            &function.unit_continuations,
                            custody.operation_ordinal,
                        )
                    })
                    .or(default_affine_cleanup);
            if !function.unit_continuations.is_empty()
                && custody
                    .arguments
                    .iter()
                    .any(|argument| !argument.path.is_empty())
                && machine_functions.get(&custody.target).is_none_or(|callee| {
                    callee.scalar_abi.is_some()
                        || default_affine_cleanup.is_none_or(|cleanup| {
                            !crate::object_artifact::replay::unit::continuations::exact_projected_callee(
                                custody,
                                &callee.unit_parameters,
                                callee.unit_affine_cleanup.as_ref(),
                                cleanup,
                                &callee.semantic_code_attribution,
                            )
                        })
                })
            {
                return Err(ObjectError::InvalidInternalUnitCallEvidence(
                    function.machine,
                ));
            }
            validate_internal_unit_call_custody(
                plan.target,
                function,
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.internal_calls,
                &function.internal_unit_calls,
                parameter_homes,
                validated_function_stack.as_ref(),
                unit_call_stack,
                scalar_call_stack,
                machine_functions
                    .get(&custody.target)
                    .and_then(|callee| callee.parameter_abi.as_ref()),
                machine_functions
                    .get(&custody.target)
                    .map_or(&[][..], |callee| callee.unit_parameters.as_slice()),
                machine_functions
                    .get(&custody.target)
                    .and_then(|callee| callee.mixed_structural_scalar_abi.as_ref()),
                target_structural_return,
                custody,
                affine_cleanup,
                fully_consumed_affine_parameter
                    || custody
                        .arguments
                        .iter()
                        .any(|argument| continuation_discards.contains(&argument.place)),
            )?;
        }
        validate_unit_affine_scalar_records(function)?;
        validate_internal_unit_scalar_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
            &validated_call_stacks,
        )?;
        validate_installed_provider_unit_scalar_calls(
            plan.target,
            function,
            &machine_functions,
            &validated_call_stacks,
        )?;
        let dynamic_peak = validate_dynamic_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
        )?;
        let stored_dynamic_peak = validate_stored_dynamic_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
        )?;
        if let Some(stack) = validated_function_stack.as_mut() {
            stack.local_peak_bytes = stack
                .local_peak_bytes
                .max(dynamic_peak)
                .max(stored_dynamic_peak);
        }
        validate_unit_write_only_primitive_stores(
            plan.target,
            function,
            validated_function_stack.as_ref(),
        )?;
        validate_unit_structural_scalar_field_stores(
            plan.target,
            function,
            validated_function_stack.as_ref(),
        )?;
        validate_scalar_structural_scalar_field_stores(plan.target, function)?;
        match (&function.unit_stack, &function.unit_affine_cleanup) {
            (Some(_), Some(cleanup)) => validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.unit_parameter_homes,
                &function.internal_unit_calls,
                &function.boundary_settlements,
                &attachments,
                &machine_functions,
                cleanup,
                false,
                fully_consumed_affine_parameter,
                partially_consumed_affine_parameter,
                &continuation_discards,
            )?,
            (None, None) => {}
            _ => {
                return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
        }
        if let Some(cleanup) = &function.scalar_affine_cleanup {
            if function.unit_stack.is_some() || function.scalar_stack.is_none() {
                return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
            validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.scalar_structural_parameter_homes,
                &function.internal_unit_calls,
                &function.boundary_settlements,
                &attachments,
                &machine_functions,
                cleanup,
                true,
                false,
                false,
                &[],
            )?;
        }
        if !function.scalar_control_affine_cleanups.is_empty() {
            if function.unit_stack.is_some()
                || function.scalar_stack.is_none()
                || function.scalar_affine_cleanup.is_some()
            {
                return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
            for record in &function.scalar_control_affine_cleanups {
                let cleanup_end = record
                    .cleanup
                    .code_offset
                    .checked_add(record.cleanup.byte_count)
                    .ok_or(ObjectError::InvalidUnitAffineCleanupEvidence(
                        function.machine,
                    ))?;
                validate_unit_affine_cleanup(
                    function.machine,
                    &function.provenance,
                    function.bytes.get(..cleanup_end).ok_or(
                        ObjectError::InvalidUnitAffineCleanupEvidence(function.machine),
                    )?,
                    &function.semantic_code_attribution,
                    &function.scalar_structural_parameter_homes,
                    &function.internal_unit_calls,
                    &function.boundary_settlements,
                    &attachments,
                    &machine_functions,
                    &record.cleanup,
                    true,
                    false,
                    false,
                    &[],
                )?;
            }
        }
        if function.unit_parameters.len() != function.unit_parameter_homes.len()
            || function
                .unit_parameters
                .iter()
                .zip(&function.unit_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.access != home.access
                        || parameter.shape != home.shape
                })
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if function.scalar_structural_parameters.len()
            != function.scalar_structural_parameter_homes.len()
            || function
                .scalar_structural_parameters
                .iter()
                .zip(&function.scalar_structural_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.access != home.access
                        || parameter.shape != home.shape
                })
            || (!scalar_custody
                && (!function.scalar_structural_parameters.is_empty()
                    || !function.scalar_structural_parameter_homes.is_empty()))
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if let Some(stack) = function.unit_stack {
            let inline_data = function
                .boundary_settlements
                .iter()
                .filter(|settlement| {
                    linux_write_line_custody_is_exact(
                        plan.target,
                        settlement,
                        Some(&function.bytes),
                    )
                })
                .flat_map(|settlement| &settlement.byte_sequence_arguments)
                .filter_map(|argument| {
                    argument
                        .data_offset
                        .checked_add(argument.data_byte_count)
                        .map(|end| argument.data_offset..end)
                })
                .collect::<Vec<_>>();
            validate_complete_unit_stack_evidence(
                plan.target,
                function.machine,
                &function.bytes,
                stack,
                &function.internal_calls,
                &function.foreign_calls,
                &function.dynamic_calls,
                &function.stored_dynamic_calls,
                &function.boundary_settlements,
                &function.unit_integer_constants,
                &function.unit_scalar_homes,
                &function.internal_unit_scalar_calls,
                &inline_data,
            )?;
        }
        if let Some(stack) = validated_function_stack {
            validated_unit_stacks.insert(function.machine, (stack, validated_call_stacks));
        }
        validate_semantic_code_attribution(function)?;
        validate_port_effects(plan, function)?;
        validate_boundary_settlements(plan, function)?;
        previous = Some(function.machine);
        saw_entry |= function.machine == plan.entry;
        text_size = text_size
            .checked_add(function.bytes.len())
            .ok_or(ObjectError::TextSizeOverflow)?;
    }
    if !saw_entry {
        return Err(ObjectError::EntryFunctionMissing(plan.entry));
    }
    Ok(FunctionValidation {
        text_size,
        validated_unit_stacks,
        validated_scalar_stacks,
        validated_foreign_call_stacks,
    })
}

/// Shape and ordering preconditions: admitted pointer custody, ABI joins,
/// canonical function and call order, non-empty bytes, exclusive cleanup
/// evidence, and the x86 scalar FMA seam.
fn validate_function_shape(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
    previous: Option<MachineId>,
    x86_feature_profile: Option<target::TargetProfile>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<(), ObjectError> {
    // Legacy machine plans have only assigned stack homes. Incoming ABI
    // pointer custody is admitted through the independently replayed
    // shared-fragment object boundary, including call-free functions.
    if function
        .unit_parameter_homes
        .iter()
        .chain(&function.scalar_structural_parameter_homes)
        .any(|home| {
            matches!(
                home.location,
                machine_code::StructuralSourceLocation::IncomingIndirectPointer { .. }
                    | machine_code::StructuralSourceLocation::IncomingIndirectStackPointer { .. }
                    | machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
            )
        })
        || function.internal_unit_calls.iter().any(|call| {
            call.arguments.iter().any(|argument| {
                matches!(
                    argument.source_location,
                    machine_code::StructuralSourceLocation::IncomingIndirectStackPointer { .. }
                        | machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                )
            })
        })
    {
        return Err(ObjectError::InvalidInternalUnitCallEvidence(
            function.machine,
        ));
    }
    validate_mixed_structural_scalar_abi(plan.target, function)?;
    validate_unit_dynamic_descriptor_join(plan.target, function)?;
    if let Some(previous) = previous
        && previous >= function.machine
    {
        return Err(ObjectError::NonCanonicalFunctionOrder {
            previous,
            current: function.machine,
        });
    }
    if function.bytes.is_empty() {
        return Err(ObjectError::EmptyFunction(function.machine));
    }
    crate::object_artifact::replay::x86_fma::validate_x86_scalar_fma_function(
        plan.target,
        x86_feature_profile,
        x86_scalar_fma_provider,
        function,
    )?;
    if function
        .internal_calls
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(ObjectError::NonCanonicalInternalCallOrder(function.machine));
    }
    if (function.unit_affine_cleanup.is_some()
        && (function.scalar_affine_cleanup.is_some()
            || !function.scalar_control_affine_cleanups.is_empty()))
        || (function.scalar_affine_cleanup.is_some()
            && !function.scalar_control_affine_cleanups.is_empty())
    {
        return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
            function.machine,
        ));
    }
    if function
        .foreign_calls
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(ObjectError::NonCanonicalForeignCallOrder(function.machine));
    }
    Ok(())
}

/// Every foreign call site: provenance ownership, one call per owner, target
/// agreement, stack-plan agreement, no overlap with internal calls, exact site
/// bytes, and monotone floating-control custody.
fn validate_foreign_calls(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    let mut foreign_owners = std::collections::BTreeSet::new();
    let mut foreign_floating_control_slot = None;
    let mut prior_foreign_floating_control_end = None;
    for call in &function.foreign_calls {
        let owner_in_provenance = match call.owner {
            CallSiteOwner::Operation(operation) => {
                function.provenance.operations.contains(&operation)
            }
            CallSiteOwner::CleanupAction { edge, .. } => function.provenance.edges.contains(&edge),
        };
        if !owner_in_provenance {
            return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                caller: function.machine,
                owner: call.owner,
            });
        }
        if !foreign_owners.insert(call.owner) {
            return Err(ObjectError::DuplicateForeignCallOwner {
                caller: function.machine,
                owner: call.owner,
            });
        }
        if call.locator.target().native_target() != plan.target {
            return Err(ObjectError::ForeignCallTargetMismatch {
                caller: function.machine,
                owner: call.owner,
            });
        }
        if call.same_stack_contribution.provider_plan_report_identity()
            != call.provider_execution.provider_plan_report_identity
        {
            return Err(ObjectError::ForeignStackProviderPlanMismatch {
                caller: function.machine,
                owner: call.owner,
            });
        }
        if function
            .internal_calls
            .iter()
            .any(|internal| internal.offset == call.offset)
        {
            return Err(ObjectError::ForeignCallOverlapsInternalCall {
                caller: function.machine,
                offset: call.offset,
            });
        }
        validate_foreign_call_site(
            plan.target.architecture,
            function.machine,
            &function.bytes,
            call,
        )?;
        validate_foreign_call_floating_control(plan.target, function, call)?;
        let control = match plan.target.architecture {
            Architecture::X86_64 => call.x86_floating_control.map(|control| {
                (
                    control.saved_slot_byte_offset,
                    control.save_offset,
                    control.restore_offset,
                    control.restore_byte_count,
                )
            }),
            Architecture::Aarch64 => call.aarch64_floating_control.map(|control| {
                (
                    control.saved_slot_byte_offset,
                    control.save_offset,
                    control.restore_offset,
                    control.restore_byte_count,
                )
            }),
        };
        if let Some((slot, save_offset, restore_offset, restore_byte_count)) = control {
            if foreign_floating_control_slot
                .replace(slot)
                .is_some_and(|prior| prior != slot)
                || prior_foreign_floating_control_end.is_some_and(|end| end > save_offset)
            {
                return Err(ObjectError::InvalidForeignCallFloatingControl {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            prior_foreign_floating_control_end =
                Some(restore_offset.checked_add(restore_byte_count).ok_or(
                    ObjectError::InvalidForeignCallFloatingControl {
                        caller: function.machine,
                        owner: call.owner,
                    },
                )?);
        }
        validate_foreign_scalar_arguments(plan.target, function, call)?;
    }
    Ok(())
}

/// Semantic code attribution rows are ordered, inside the function, and name
/// each provenance site once.
fn validate_semantic_code_attribution(function: &MachineCodeFunction) -> Result<(), ObjectError> {
    if function.semantic_code_attribution.windows(2).any(|pair| {
        (pair[0].operation_ordinal, pair[0].code_offset)
            >= (pair[1].operation_ordinal, pair[1].code_offset)
    }) {
        return Err(ObjectError::NonCanonicalSemanticCodeAttributionOrder(
            function.machine,
        ));
    }
    let mut attribution_sites = std::collections::BTreeSet::new();
    for attribution in &function.semantic_code_attribution {
        let end = attribution
            .code_offset
            .checked_add(attribution.byte_count)
            .ok_or(ObjectError::SemanticCodeAttributionOutsideFunction(
                function.machine,
            ))?;
        let known = match attribution.site {
            SemanticCodeSite::Operation(operation) => {
                function.provenance.operations.contains(&operation)
            }
            SemanticCodeSite::Edge(edge) => function.provenance.edges.contains(&edge),
        };
        if end > function.bytes.len() || !known || !attribution_sites.insert(attribution.site) {
            return Err(ObjectError::InvalidSemanticCodeAttribution(
                function.machine,
            ));
        }
    }
    Ok(())
}

/// Port effects are ordered, inside the function, owned by provenance, unique
/// per operation, and encode the exact immediate port write.
fn validate_port_effects(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    if function.port_effects.windows(2).any(|pair| {
        (pair[0].code_offset, pair[0].operation_ordinal)
            >= (pair[1].code_offset, pair[1].operation_ordinal)
    }) {
        return Err(ObjectError::NonCanonicalPortEffectOrder(function.machine));
    }
    let mut port_operations = std::collections::BTreeSet::new();
    for effect in &function.port_effects {
        let end = effect.code_offset.checked_add(effect.byte_count).ok_or(
            ObjectError::PortEffectOutsideFunction {
                machine: function.machine,
                operation: effect.psi_operation,
            },
        )?;
        if end > function.bytes.len() || effect.byte_count == 0 {
            return Err(ObjectError::PortEffectOutsideFunction {
                machine: function.machine,
                operation: effect.psi_operation,
            });
        }
        if !function
            .provenance
            .operations
            .contains(&effect.psi_operation)
        {
            return Err(ObjectError::PortEffectOperationNotInProvenance {
                machine: function.machine,
                operation: effect.psi_operation,
            });
        }
        if !port_operations.insert(effect.psi_operation) {
            return Err(ObjectError::DuplicatePortEffectOperation {
                machine: function.machine,
                operation: effect.psi_operation,
            });
        }
        if plan.target.architecture != Architecture::X86_64
            || function.bytes[effect.code_offset..end]
                != x86_encoding::encode_immediate_port_write(effect.port, effect.value)
        {
            return Err(ObjectError::PortEffectBytesMismatch {
                machine: function.machine,
                operation: effect.psi_operation,
            });
        }
    }
    Ok(())
}

/// Boundary settlements are ordered, inside the function, owned by provenance,
/// unique per operation, carry exact completion custody, and realize their
/// declared boundary with the exact bytes and result placement.
fn validate_boundary_settlements(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
) -> Result<(), ObjectError> {
    if function.boundary_settlements.windows(2).any(|pair| {
        (pair[0].code_offset, pair[0].operation_ordinal)
            >= (pair[1].code_offset, pair[1].operation_ordinal)
    }) {
        return Err(ObjectError::NonCanonicalBoundarySettlementOrder(
            function.machine,
        ));
    }
    let mut settlement_operations = std::collections::BTreeSet::new();
    for settlement in &function.boundary_settlements {
        if settlement.code_offset > function.bytes.len() {
            return Err(ObjectError::BoundarySettlementOutsideFunction {
                machine: function.machine,
                operation: settlement.psi_operation,
            });
        }
        if !function
            .provenance
            .operations
            .contains(&settlement.psi_operation)
        {
            return Err(ObjectError::BoundarySettlementOperationNotInProvenance {
                machine: function.machine,
                operation: settlement.psi_operation,
            });
        }
        if !settlement_operations.insert(settlement.psi_operation) {
            return Err(ObjectError::DuplicateBoundarySettlementOperation {
                machine: function.machine,
                operation: settlement.psi_operation,
            });
        }
        if let Err(error) = validate_completion_custody(settlement) {
            return Err(match error {
                CompletionCustodyError::ArgumentPath => {
                    ObjectError::InvalidBoundarySettlementArgumentPath {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    }
                }
                CompletionCustodyError::ReceiptArgumentIndex => {
                    ObjectError::InvalidCompletionReceiptArgumentIndex {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    }
                }
                CompletionCustodyError::ReceiptCustody => {
                    ObjectError::InvalidCompletionReceiptCustody {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    }
                }
                CompletionCustodyError::ProviderCustody => {
                    ObjectError::InvalidCompletionProviderCustody {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    }
                }
            });
        }
        let valid_realization = match settlement.realization {
            BoundaryRealization::MetadataOnlyPort(realization) => {
                settlement.scalar_arguments.is_empty()
                    && settlement.runtime_scalar_arguments.is_empty()
                    && settlement.byte_sequence_arguments.is_empty()
                    && settlement.byte_count == 0
                    && function
                        .port_effects
                        .iter()
                        .filter(|effect| {
                            effect.psi_operation == realization.effect_operation
                                && effect.service == realization.service
                                && effect.port == realization.port
                                && effect.value == realization.value
                                && effect.operation_ordinal.checked_add(1)
                                    == Some(settlement.operation_ordinal)
                                && effect.code_offset.checked_add(effect.byte_count)
                                    == Some(settlement.code_offset)
                        })
                        .count()
                        == 1
            }
            BoundaryRealization::ClaimCompletionOnly(_) => {
                settlement.scalar_arguments.is_empty()
                    && settlement.runtime_scalar_arguments.is_empty()
                    && settlement.byte_sequence_arguments.is_empty()
                    && settlement.native_result.is_unit()
                    && settlement.byte_count == 0
            }
            BoundaryRealization::DirectPortReadU8(realization) => {
                let expected = x86_encoding::encode_immediate_port_read_u8(realization.port);
                let exact_return_edge =
                    settlement.native_result.scalar().is_some_and(|result| {
                        let Some(return_ordinal) = settlement.operation_ordinal.checked_add(1)
                        else {
                            return false;
                        };
                        let Some(return_offset) =
                            settlement.code_offset.checked_add(settlement.byte_count)
                        else {
                            return false;
                        };
                        function
                            .semantic_code_attribution
                            .iter()
                            .filter(|attribution| {
                                attribution.site == SemanticCodeSite::Edge(result.return_edge)
                                    && attribution.operation_ordinal == return_ordinal
                                    && attribution.code_offset == return_offset
                                    && attribution.byte_count == 1
                            })
                            .count()
                            == 1
                            && function.bytes.get(return_offset) == Some(&0xc3)
                    });
                settlement.scalar_arguments.is_empty()
                    && settlement.runtime_scalar_arguments.is_empty()
                    && settlement.byte_sequence_arguments.is_empty()
                    && settlement.byte_count == expected.len()
                    && plan.target.architecture == Architecture::X86_64
                    && settlement
                        .code_offset
                        .checked_add(settlement.byte_count)
                        .and_then(|end| function.bytes.get(settlement.code_offset..end))
                        == Some(expected.as_slice())
                    && function.unit_stack.is_none()
                    && function.scalar_stack.is_some()
                    && exact_return_edge
                    && settlement.arguments.iter().all(|argument| {
                        argument.path.is_empty()
                            && function
                                .scalar_structural_parameters
                                .iter()
                                .any(|parameter| parameter.place == argument.place)
                    })
            }
            BoundaryRealization::LinuxWriteLine(_) => {
                linux_write_line_custody_is_exact(
                    plan.target,
                    settlement,
                    Some(&function.bytes),
                ) && function.unit_stack.is_some()
                    && function.scalar_stack.is_none()
            }
            BoundaryRealization::HostedExitProcessI32(_) => {
                if settlement.runtime_scalar_arguments.iter().any(|argument| matches!(argument.source, machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. })) {
                    crate::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::bytes_are_exact(plan.target, settlement, &function.bytes)
                        && function.scalar_stack.is_none()
                        && settlement.operation_ordinal.checked_add(1).is_some_and(|ordinal| function.semantic_code_attribution.iter().filter(|row| {
                            matches!(row.site, SemanticCodeSite::Edge(_))
                                && row.operation_ordinal == ordinal
                                && row.byte_count == 0
                                && settlement.code_offset.checked_add(settlement.byte_count) == Some(row.code_offset)
                        }).count() == 1)
                } else {
                let [argument] = settlement.scalar_arguments.as_slice() else {
                    return Err(ObjectError::BoundaryRealizationMismatch {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    });
                };
                let i32_type = semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .expect("i32 is valid");
                let value = match (argument.scalar_type, argument.immediate) {
                    (
                        semantic_vocabulary::ScalarType::Integer(actual),
                        semantic_vocabulary::IntegerValue::Signed(value),
                    ) if actual == i32_type => i32::try_from(value).ok(),
                    _ => None,
                };
                let expected_destination =
                    if !target_operations::HostedExitProcessI32Realization::supports_target(
                        plan.target,
                    ) {
                        None
                    } else {
                        match plan.target.architecture {
                            Architecture::X86_64 => {
                                Some(calling_conventions::MachineRegister::X86Rdi)
                            }
                            Architecture::Aarch64 => {
                                Some(calling_conventions::MachineRegister::Aarch64X(0))
                            }
                        }
                    };
                let expected = value.and_then(|value| match plan.target.architecture {
                    Architecture::X86_64 => {
                        Some(isa_x86_64::encode_hosted_exit_process_i32(value))
                    }
                    Architecture::Aarch64 => {
                        isa_aarch64::encode_hosted_exit_process_i32(plan.target, value).ok()
                    }
                });
                let exact_nominal_tail = settlement
                    .operation_ordinal
                    .checked_add(1)
                    .is_some_and(|tail_ordinal| {
                        function
                            .semantic_code_attribution
                            .iter()
                            .filter(|attribution| {
                                matches!(attribution.site, SemanticCodeSite::Edge(_))
                                    && attribution.operation_ordinal == tail_ordinal
                                    && attribution.code_offset
                                        == settlement
                                            .code_offset
                                            .saturating_add(settlement.byte_count)
                                    && (function.unit_stack.is_some()
                                        || (attribution.byte_count == 0
                                            && attribution.code_offset == function.bytes.len()))
                            })
                            .count()
                            == 1
                    });
                expected.is_some_and(|expected| {
                    settlement.byte_count == expected.len()
                        && settlement.byte_count != 0
                        && settlement
                            .code_offset
                            .checked_add(settlement.byte_count)
                            .and_then(|end| function.bytes.get(settlement.code_offset..end))
                            == Some(expected.as_slice())
                }) && expected_destination == Some(argument.destination)
                    && settlement.runtime_scalar_arguments.is_empty()
                    && settlement.arguments.is_empty()
                    && settlement.byte_sequence_arguments.is_empty()
                    && settlement.native_result.is_unit()
                    && function.scalar_stack.is_none()
                    && exact_nominal_tail
                }
            }
            BoundaryRealization::HostedWriteByteI32(_) => {
                hosted_write_byte_custody_is_exact(
                    plan.target,
                    settlement,
                    &function.boundary_settlements,
                    &function.unit_integer_constants,
                    &function.unit_scalar_homes,
                    |home, consumer_ordinal, consumer_offset| {
                        crate::object_artifact::replay::unit::scalar_call_custody::exact_preceding_internal_unit_scalar_home_producer_count(
                            &function.internal_unit_scalar_calls,
                            home,
                            consumer_ordinal,
                            consumer_offset,
                        )
                    },
                    Some(&function.bytes),
                ) && function.unit_stack.is_some()
                    && function.scalar_stack.is_none()
            }
            BoundaryRealization::HostedReadByte(_) => {
                let expected = settlement.native_result.structural().and_then(|result| {
                    let payload = result
                        .home_byte_offset
                        .checked_add(u32::from(result.layout.payload_byte_offset))?;
                    match plan.target {
                        target if target == NativeTarget::linux_x64() => isa_x86_64::encode_linux_read_byte_to_stack(
                            result.home_byte_offset,
                            payload,
                        )
                        .ok(),
                        target if target == NativeTarget::linux_arm64() => isa_aarch64::encode_linux_read_byte_to_stack(
                            result.home_byte_offset,
                            payload,
                        )
                        .ok(),
                        target if target == NativeTarget::macos_arm64() => isa_aarch64::encode_macos_read_byte_to_stack(
                            result.home_byte_offset,
                            payload,
                        )
                        .ok(),
                        _ => None,
                    }
                });
                settlement.scalar_arguments.is_empty()
                    && settlement.runtime_scalar_arguments.is_empty()
                    && settlement.arguments.is_empty()
                    && settlement.byte_sequence_arguments.is_empty()
                    && expected.as_ref().is_some_and(|expected| {
                        settlement.byte_count == expected.len()
                            && settlement
                                .code_offset
                                .checked_add(settlement.byte_count)
                                .and_then(|end| function.bytes.get(settlement.code_offset..end))
                                == Some(expected.as_slice())
                    })
                    && function.unit_stack.is_some()
                    && function.scalar_stack.is_none()
            }
        };
        if !valid_realization
            || !boundary_result_is_exact(
                plan.target,
                settlement.realization,
                &settlement.native_result,
            )
        {
            return Err(ObjectError::BoundaryRealizationMismatch {
                machine: function.machine,
                operation: settlement.psi_operation,
            });
        }
    }
    Ok(())
}
