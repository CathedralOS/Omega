//! Source parameter declarations shared by typing and contract readers.
//! Selection retains the exact callable or signature owner; missing and
//! ambiguous declarations supply no parameter roster.

use super::{TypedTrees, signature::StateParameter, types::TypeReferenceHandle};
use symbols::SymbolHandle;

pub struct CallableValueTypes<'a> {
    pub parameters: arena::RootRows<'a, StateParameter>,
    pub result: TypeReferenceHandle,
}

impl TypedTrees {
    pub fn callable_parameters(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<arena::RootRows<'_, StateParameter>>, String> {
        Ok(self
            .callable_value_types(target)?
            .map(|types| types.parameters))
    }

    pub fn callable_value_types(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<CallableValueTypes<'_>>, String> {
        if !self.symbols.symbols().nodes().is_valid(target) {
            return Ok(None);
        }
        if let Some(application) = self.closed_requirement_application(target) {
            return Ok(Some(CallableValueTypes {
                parameters: self.state_signature_parameters(application.signature),
                result: application.signature.return_type,
            }));
        }
        if let Some(state) = self.state_by_symbol(target) {
            return Ok(Some(CallableValueTypes {
                parameters: self.state_parameters(state),
                result: state.return_type,
            }));
        }
        if let Some(machine) = self.machine_by_symbol(target) {
            return Ok(self
                .machine_states(machine)
                .first()
                .map(|state| CallableValueTypes {
                    parameters: self.state_parameters(state),
                    result: state.return_type,
                }));
        }
        if let Some((_, signature)) = self.machine_parameter_signature(target) {
            return Ok(Some(CallableValueTypes {
                parameters: self.state_signature_parameters(signature),
                result: signature.return_type,
            }));
        }
        if let Some(operator) = self.operator_definition_by_symbol(target) {
            return Ok(Some(CallableValueTypes {
                parameters: self.operator_parameters(operator),
                result: operator.return_type,
            }));
        }
        let mut signatures = self
            .traits()
            .iter()
            .flat_map(|declaration| self.trait_machine_signatures(declaration))
            .filter(|signature| signature.symbol == target);
        if let Some(signature) = signatures.next()
            && signatures.next().is_none()
        {
            return Ok(Some(CallableValueTypes {
                parameters: self.state_signature_parameters(signature),
                result: signature.return_type,
            }));
        }
        if let Some(signature) = self.evidence_requirement_declaration(target)? {
            return Ok(Some(CallableValueTypes {
                parameters: self.state_signature_parameters(signature),
                result: signature.return_type,
            }));
        }
        Ok(None)
    }
}
