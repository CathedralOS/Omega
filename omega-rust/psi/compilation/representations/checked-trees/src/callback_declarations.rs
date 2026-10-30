//! Project nominal-use declaration facts without target or provider decisions.

use crate::{CheckedNominalMachineUse, CheckedTrees};
use language_semantics::callback_declarations::{
    CallbackDeclaration, CallbackRegistrationDeclaration,
};

impl CheckedTrees {
    /// Borrow the original occurrence's checked entry and requirement. A missing
    /// registration remains a deferred per-use error, not an early batch refusal.
    pub fn callback_declaration<'usage>(
        &self,
        usage: &'usage CheckedNominalMachineUse,
    ) -> CallbackDeclaration<'usage> {
        CallbackDeclaration {
            registration: self.callback_registration_declaration(
                usage.registration_operation,
                usage.registration_requirement_trait,
            ),
            satisfaction_is_boundary: self
                .typed
                .trait_definition_by_symbol(usage.satisfaction_trait)
                .is_some_and(|definition| definition.is_boundary),
            has_checked_entry: usage.callback_entry.is_some(),
            requirement_overload: &usage.canonical_requirement_overload,
        }
    }

    fn callback_registration_declaration(
        &self,
        operation: symbols::SymbolHandle,
        requirement_trait: Option<symbols::SymbolHandle>,
    ) -> Result<CallbackRegistrationDeclaration, String> {
        let typed = &self.typed;
        let identity = if let Some((owner, signature)) = typed.trait_machine_signature(operation) {
            if !owner.is_boundary {
                return Ok(CallbackRegistrationDeclaration::CheckedSource);
            }
            typed.trait_requirement_callable_identity_text(owner, signature)
        } else if let Some(machine) = typed.machine_by_own_or_state_symbol(operation) {
            if machine.supplies_checked_body() {
                return Ok(CallbackRegistrationDeclaration::CheckedSource);
            }
            typed
                .machine_callable_identity_text(machine)
                .ok_or_else(|| {
                    "nominal machine registration lost its complete callable identity".to_owned()
                })?
        } else if typed.machine_parameter_signature(operation).is_some() {
            // A symbolic forwarding edge names its binder. Concrete selection
            // owns any eventual external materialization.
            return Ok(CallbackRegistrationDeclaration::CheckedSource);
        } else if requirement_trait
            .and_then(|owner| typed.trait_definition_by_symbol(owner))
            .is_some_and(|declaration| !declaration.is_boundary)
        {
            return Ok(CallbackRegistrationDeclaration::CheckedSource);
        } else {
            return Err(format!(
                "nominal machine use lost its exact registration declaration: {} ({operation:?})",
                typed.symbols.display_path(operation, "::"),
            ));
        };
        Ok(CallbackRegistrationDeclaration::External(identity))
    }
}
