//! Roster validation for compiler-private callback functions carried
//! beside the semantic program functions of one object.
use crate::object_artifact::construction::build_object_artifact_with_x86_feature_profile;
use crate::{ObjectError, ObjectFunction};
use machine_code::{CompilerPrivateMachineCodeFunction, MachineCodePlan};
use target::NativeTarget;

pub(crate) struct ValidatedPrivateFunction<'plan> {
    pub(crate) machine: &'plan CompilerPrivateMachineCodeFunction,
    pub(crate) function: ObjectFunction,
}

pub(crate) fn validate_private_functions<'plan>(
    target: NativeTarget,
    private_functions: &'plan [CompilerPrivateMachineCodeFunction],
) -> Result<Vec<ValidatedPrivateFunction<'plan>>, ObjectError> {
    // A registrar materializes one private function per placement; the roster
    // is unbounded but every member must carry a distinct callback-thunk
    // identity and a distinct private symbol.
    let mut identities = std::collections::HashSet::new();
    let mut symbols = std::collections::BTreeSet::new();
    let mut validated = Vec::with_capacity(private_functions.len());
    for private in private_functions {
        let Some(_placement_index) = private.identity.callback_thunk_placement_index() else {
            return Err(ObjectError::InvalidPrivateFunctionIdentity);
        };
        if !private.identity.is_valid() || !identities.insert(private.identity) {
            return Err(ObjectError::InvalidPrivateFunctionIdentity);
        }
        if private.private_symbol.is_empty() {
            return Err(ObjectError::EmptyPrivateFunctionSymbol);
        }
        if !symbols.insert(private.private_symbol.as_ref()) {
            return Err(ObjectError::PrivateFunctionSymbolCollision);
        }
        if private.function.scalar_abi.is_none() {
            return Err(ObjectError::InvalidPrivateFunctionAbi);
        }
        if !private.function.internal_calls.is_empty()
            || !private.function.foreign_calls.is_empty()
            || !private.function.internal_unit_calls.is_empty()
            || !private.function.internal_unit_scalar_calls.is_empty()
            || !private
                .function
                .installed_provider_unit_scalar_calls
                .is_empty()
            || private.function.parameter_abi.is_some()
            || !private.function.dynamic_calls.is_empty()
            || !private.function.stored_dynamic_calls.is_empty()
            || !private.function.dynamic_parameter_calls.is_empty()
            || !private
                .function
                .forwarded_dynamic_descriptor_calls
                .is_empty()
            || !private.function.x86_scalar_fma.is_empty()
            || !private.function.x86_scalar_fma_occurrences.is_empty()
            || private.function.x86_floating_control.is_some()
            || !private.function.port_effects.is_empty()
            || !private.function.boundary_settlements.is_empty()
            || private.function.structural_return.is_some()
        {
            return Err(ObjectError::UnsupportedPrivateFunctionBody);
        }
        let standalone = MachineCodePlan {
            psi: private.source_psi,
            target,
            entry: private.function.machine,
            functions: vec![private.function.clone()],
        };
        let replayed = build_object_artifact_with_x86_feature_profile(&standalone, &[], None, None)
            .map_err(|_| ObjectError::InvalidPrivateFunctionBody)?;
        let [function] = replayed.functions.as_slice() else {
            return Err(ObjectError::InvalidPrivateFunctionBody);
        };
        if !replayed.object.layout.normalized_imports.is_empty()
            || replayed.relocations.record_count() != 0
            || replayed.text_bytes != private.function.bytes
            || !replayed.data_bytes.is_empty()
        {
            return Err(ObjectError::InvalidPrivateFunctionBody);
        }
        validated.push(ValidatedPrivateFunction {
            machine: private,
            function: function.clone(),
        });
    }
    Ok(validated)
}
