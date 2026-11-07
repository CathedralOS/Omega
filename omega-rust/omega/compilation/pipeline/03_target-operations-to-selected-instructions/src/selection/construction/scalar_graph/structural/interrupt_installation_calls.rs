//! The installer invokes checked bodies with ordinary ABI transport. Each call
//! keeps its role under the original Scope operation; neither invents a source
//! call or a semantic result for the validator's internal Boolean.

use super::{Builder, LegalizedScalarFunction, LegalizedScalarInstruction, VirtualRegisterId};
use crate::SelectedInstructionError;
use legalized_operations::LegalizedScalarCall;
use target_operations::{CallSiteOwner, InstallationCallRole};

struct Invocation<'a> {
    row: &'a LegalizedScalarInstruction,
    call: &'a LegalizedScalarCall,
}
impl super::super::call_arguments::InvocationArgumentSource for Invocation<'_> {
    fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
        semantic_vocabulary::InvocationSite::Operation(self.row.operation)
    }
    fn invocation_call_plan(&self) -> Option<&calling_conventions::CallPlan> {
        Some(&self.call.call_plan)
    }
    fn invocation_ownership(&self) -> &[optimization_unit::OwnershipEvent] {
        &[]
    }
    fn invocation_work(&self) -> &[optimization_unit::WorkSettlement] {
        &[]
    }
    fn invocation_argument_place(&self, position: usize) -> Option<semantic_vocabulary::PlaceId> {
        match self.call.arguments.get(position)? {
            legalized_operations::LegalizedScalarArgument::Structural { semantic, .. } => {
                Some(semantic.place)
            }
            _ => None,
        }
    }
}

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    role: InstallationCallRole,
    call: &LegalizedScalarCall,
    builder: &mut Builder<'_>,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let invalid = || {
        SelectedInstructionError::custody_because("installed helper lost its checked ordinary call")
    };
    let owner = CallSiteOwner::Installation {
        operation: row.operation,
        role,
    };
    let environment = builder.environment;
    let key = match role {
        InstallationCallRole::Validate => {
            crate::selection::scalar_call_abi::unit_key(call, environment)
        }
        InstallationCallRole::Work => crate::selection::scalar_call_abi::no_normal_key(
            call,
            &abstract_operations::AbstractFunctionResult::Unit,
            environment,
        ),
    }
    .ok_or_else(invalid)?;
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        owner,
        key,
        super::super::row(builder.catalog, key)?,
        environment,
    )?;
    let invocation = Invocation { row, call };
    let mut operands =
        super::super::call_arguments::marshal(source, &invocation, call, builder, invalid)?;
    let (kind, result) = match role {
        InstallationCallRole::Validate => {
            let constraint = super::super::row(builder.catalog, key)?;
            let output = constraint.operands.last().ok_or_else(invalid)?;
            let result = super::super::no_normal::scratch(
                builder,
                output.class,
                semantic_vocabulary::ScalarType::Boolean,
                operands.len(),
            )?;
            operands.push(result);
            (
                selected_instructions::SelectedInstructionKind::CallScalar {
                    target: selected_instructions::SelectedCallTarget::from_native(call.target),
                },
                Some(result),
            )
        }
        InstallationCallRole::Work => {
            call.validate_no_normal_shape().map_err(|_| invalid())?;
            (
                selected_instructions::SelectedInstructionKind::InvokeNoNormal {
                    target: selected_instructions::SelectedCallTarget::from_native(call.target),
                },
                None,
            )
        }
    };
    super::super::call_arguments::emit_call(
        builder,
        owner,
        call,
        row.effect,
        Vec::new(),
        kind,
        key,
        &operands,
        super::provenance(&invocation),
    )?;
    if role == InstallationCallRole::Work {
        super::super::operation_control::finish_installation_work(builder, row)?;
    }
    Ok(result)
}
