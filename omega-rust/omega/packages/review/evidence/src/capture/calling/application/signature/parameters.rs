//! Clone-local instantiation of nested static parameter signatures.

use super::{rejected, types};
use crate::capture::semantics::signatures::parameters::CallingContractScope;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::{
    data::{MachineParameterContract, TypeParameter, TypeParameterKind},
    name::Identifier,
    signature::StateParameter,
    types::TypeReferenceHandle,
};

pub(crate) fn instantiate(
    compilation: &mut CheckedCompilation,
    parameters: &mut [TypeParameter],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[(Identifier, Identifier)],
    containing_lifetime_binders: &[Identifier],
    contract_scopes: &mut Vec<CallingContractScope>,
    depth: usize,
) -> Result<(), Vec<Diagnostic>> {
    if depth >= 64 {
        return Err(rejected("calling static contract exceeds projection depth"));
    }
    for parameter in parameters {
        match &mut parameter.kind {
            TypeParameterKind::Const { type_reference } => {
                *type_reference = types::instantiate(
                    compilation,
                    *type_reference,
                    substitutions,
                    lifetimes,
                    depth + 1,
                )?
            }
            TypeParameterKind::Machine {
                contract: MachineParameterContract::Structural(signature),
            } => {
                let original_signature = signature.clone();
                // A nested declaration introduces new binders even when its
                // authored names shadow an outer telescope. Give its clone
                // distinct names before the shared ordinal projector appends
                // it to that telescope; source names never enter the record.
                let mut nested_binders = containing_lifetime_binders.to_vec();
                let mut nested_lifetimes = Vec::new();
                for (ordinal, name) in signature.lifetime_parameters.iter_mut().enumerate() {
                    if nested_lifetimes.iter().any(|(source, _)| source == name) {
                        return Err(rejected(
                            "calling static contract repeats a lifetime binder",
                        ));
                    }
                    let mut collision = 0;
                    let normalized = loop {
                        let candidate = Identifier::generated(format!(
                            "$calling-nested-lifetime-{depth}-{ordinal}-{collision}"
                        ));
                        if !nested_binders.contains(&candidate)
                            && !lifetimes.iter().any(|(_, target)| *target == candidate)
                        {
                            break candidate;
                        }
                        collision += 1;
                    };
                    nested_lifetimes.push((name.clone(), normalized.clone()));
                    nested_binders.push(normalized.clone());
                    *name = normalized;
                }
                nested_lifetimes.extend_from_slice(lifetimes);
                contract_scopes.push(CallingContractScope {
                    parameter_symbol: parameter.symbol,
                    signature_symbol: signature.symbol,
                    lifetime_substitutions: nested_lifetimes.iter().rev().cloned().collect(),
                    parameters: compilation.state_signature_parameters(signature).to_vec(),
                    original_signature,
                });
                let mut parameters = compilation
                    .state_signature_type_parameters(signature)
                    .to_vec();
                instantiate(
                    compilation,
                    &mut parameters,
                    substitutions,
                    &nested_lifetimes,
                    &nested_binders,
                    contract_scopes,
                    depth + 1,
                )?;
                signature.type_parameters = compilation
                    .typed
                    .data_type_parameters
                    .insert_many(parameters);
                let mut values = compilation.state_signature_parameters(signature).to_vec();
                instantiate_values(
                    compilation,
                    &mut values,
                    substitutions,
                    &nested_lifetimes,
                    depth + 1,
                )?;
                signature.parameters = compilation.typed.state_parameters.insert_many(values);
                signature.return_type = types::instantiate(
                    compilation,
                    signature.return_type,
                    substitutions,
                    &nested_lifetimes,
                    depth + 1,
                )?;
            }
            TypeParameterKind::Proposition { contract } => {
                let mut values = compilation
                    .typed
                    .state_parameters
                    .span_or_empty(contract.parameters)
                    .to_vec();
                instantiate_values(
                    compilation,
                    &mut values,
                    substitutions,
                    lifetimes,
                    depth + 1,
                )?;
                contract.parameters = compilation.typed.state_parameters.insert_many(values);
            }
            _ => {}
        }
    }
    Ok(())
}

fn instantiate_values(
    compilation: &mut CheckedCompilation,
    values: &mut [StateParameter],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[(Identifier, Identifier)],
    depth: usize,
) -> Result<(), Vec<Diagnostic>> {
    for value in values {
        value.type_reference = types::instantiate(
            compilation,
            value.type_reference,
            substitutions,
            lifetimes,
            depth,
        )?;
    }
    Ok(())
}
