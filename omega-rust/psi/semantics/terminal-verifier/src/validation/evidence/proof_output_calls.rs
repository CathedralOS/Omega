//! The proof-output invocations and their evidence bindings.

use super::super::{
    BTreeMap, BTreeSet, EvidenceTermId, MachineId, ModuleError, TerminalMachine, TerminalModule,
};
use super::validate_static_requirement_dispatch;

/// The proof-output invocations: each names a known target, dispatches its
/// static requirement exactly, and binds evidence arguments and outputs to
/// known terms at dense package ordinals.
pub(super) fn validate_proof_output_calls(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    terms: &BTreeMap<EvidenceTermId, &terminal_psi::EvidenceTermDeclaration>,
    used_terms: &mut BTreeSet<EvidenceTermId>,
) -> Result<(), ModuleError> {
    let mut next_package_ordinals = BTreeMap::new();
    for invocation in &module.proof_output_calls {
        let expected = next_package_ordinals
            .entry(invocation.caller)
            .or_insert(0_u32);
        if invocation.ordinal != *expected {
            return Err(ModuleError::NonCanonicalProofOutputCall {
                caller: invocation.caller,
                ordinal: invocation.ordinal,
            });
        }
        *expected = expected
            .checked_add(1)
            .ok_or(ModuleError::NonCanonicalProofOutputCall {
                caller: invocation.caller,
                ordinal: invocation.ordinal,
            })?;
        if !machines.contains_key(&invocation.caller) {
            return Err(ModuleError::UnknownEvidenceContractMachine(
                invocation.caller,
            ));
        }
        if invocation.target_machine_identity.is_empty() || invocation.outputs.is_empty() {
            return Err(ModuleError::InvalidProofOutputCall {
                caller: invocation.caller,
                ordinal: invocation.ordinal,
            });
        }
        validate_static_requirement_dispatch(module, machines, invocation)?;
        match (invocation.runtime_result, invocation.runtime_call) {
            (None, None) => {}
            (Some(terminal_psi::ProofOutputRuntimeResult::Unit), Some(runtime_call)) => {
                let caller = machines
                    .get(&invocation.caller)
                    .expect("the proof-output caller was validated above");
                let mut matching_operations = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| operation.id == runtime_call.operation);
                let Some(operation) = matching_operations.next() else {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                };
                if matching_operations.next().is_some()
                    || !matches!(
                        (&operation.result, &operation.kind),
                        (
                            terminal_psi::OperationResult::Unit,
                            terminal_psi::OperationKind::CallUnit { callee, .. }
                        ) if *callee == runtime_call.callee
                    )
                {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                }
            }
            (
                Some(terminal_psi::ProofOutputRuntimeResult::Scalar(runtime_value)),
                Some(runtime_call),
            ) => {
                let caller = machines
                    .get(&invocation.caller)
                    .expect("the package caller was validated above");
                let mut matching_operations = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| operation.id == runtime_call.operation);
                let Some(operation) = matching_operations.next() else {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                };
                if matching_operations.next().is_some()
                    || !matches!(
                        (&operation.result, &operation.kind),
                        (
                            terminal_psi::OperationResult::Scalar(result),
                            terminal_psi::OperationKind::Call { callee, .. }
                        ) if result.scalar_type == runtime_value && *callee == runtime_call.callee
                    )
                {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                }
            }
            _ => {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
        }
        for (expected_position, argument) in invocation.evidence_arguments.iter().enumerate() {
            let Some(source) = terms.get(&argument.source) else {
                return Err(ModuleError::UnknownEvidenceContractTerm(argument.source));
            };
            let Some(instantiated) = module
                .proposition_applications
                .iter()
                .find(|application| application.id == argument.instantiated_proposition)
            else {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            };
            let callee_application = module
                .proposition_applications
                .iter()
                .find(|application| application.id == argument.callee_proposition)
                .ok_or(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                })?;
            if argument.input_position
                != u32::try_from(expected_position).map_err(|_| {
                    ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    }
                })?
                || source.proposition != argument.instantiated_proposition
                || callee_application.declaration != instantiated.declaration
                || callee_application.binder_arguments != instantiated.binder_arguments
                || callee_application.evidence_interface != instantiated.evidence_interface
                || instantiated.evidence_interface.as_ref() != Some(&source.interface)
            {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
            used_terms.insert(argument.source);
        }
        let mut fields = BTreeSet::new();
        let mut callee_terms = BTreeSet::new();
        let mut output_terms = BTreeSet::new();
        for (expected_position, binding) in invocation.outputs.iter().enumerate() {
            let static_requirement_output = invocation.static_requirement_dispatch.is_some();
            let callee_output = binding
                .callee_output
                .map(|term| {
                    terms
                        .get(&term)
                        .copied()
                        .ok_or(ModuleError::UnknownEvidenceContractTerm(term))
                })
                .transpose()?;
            let Some(instantiated) = module
                .proposition_applications
                .iter()
                .find(|application| application.id == binding.instantiated_proposition)
            else {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            };
            let callee_application = module
                .proposition_applications
                .iter()
                .find(|application| application.id == binding.callee_proposition)
                .ok_or(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                })?;
            let forwarded_source = binding.forwarded_input_position.and_then(|position| {
                invocation
                    .evidence_arguments
                    .get(usize::try_from(position).ok()?)
                    .filter(|argument| argument.input_position == position)
                    .map(|argument| argument.source)
            });
            let valid_source_shape = if static_requirement_output {
                binding.forwarded_input_position.is_none() && binding.callee_output.is_none()
            } else {
                (binding.forwarded_input_position.is_some()
                    && binding.callee_output.is_none()
                    && forwarded_source.is_some())
                    || (binding.forwarded_input_position.is_none() && callee_output.is_some())
            };
            if binding.output_position
                != u32::try_from(expected_position).map_err(|_| {
                    ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    }
                })?
                || binding.output_field.is_empty()
                || binding.output_field == "value"
                || !fields.insert(binding.output_field.as_str())
                || !valid_source_shape
                || binding.callee_output.is_some_and(|callee_output| {
                    !callee_terms.insert(callee_output) || output_terms.contains(&callee_output)
                })
                || callee_application.declaration != instantiated.declaration
                || callee_application.binder_arguments != instantiated.binder_arguments
                || callee_application.evidence_interface != instantiated.evidence_interface
            {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
            if let Some(callee_output) = binding.callee_output {
                used_terms.insert(callee_output);
            }
            if let Some(output_id) = binding.output {
                let Some(output) = terms.get(&output_id) else {
                    return Err(ModuleError::UnknownEvidenceContractTerm(output_id));
                };
                let valid_identity = if static_requirement_output {
                    output_terms.insert(output_id)
                        && !used_terms.contains(&output_id)
                        && !callee_terms.contains(&output_id)
                } else if let Some(forwarded_source) = forwarded_source {
                    output_id == forwarded_source
                } else {
                    binding.callee_output != Some(output_id)
                        && output_terms.insert(output_id)
                        && !callee_terms.contains(&output_id)
                };
                if !valid_identity
                    || binding.instantiated_proposition != output.proposition
                    || instantiated.evidence_interface.as_ref() != Some(&output.interface)
                {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                }
                used_terms.insert(output_id);
            }
        }
    }
    Ok(())
}
