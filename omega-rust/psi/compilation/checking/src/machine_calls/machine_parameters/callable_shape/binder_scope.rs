//! One callable comparison's positional static binders and enclosing region
//! frames. Contravariant descent reverses both together; sibling refinement
//! borrows the same parent frames without consuming their captures.

use super::BinderBinding;
use crate::borrow_contracts::CallableLifetimeBinding;
use symbols::SymbolHandle;
use typed_trees::{name::Identifier, type_identity::LifetimeParameterIdentity};

#[derive(Clone, Default)]
pub(super) struct CallableBinderScope {
    pub(super) static_bindings: Vec<BinderBinding>,
    pub(super) lifetime_frames: Vec<Vec<CallableLifetimeBinding>>,
}

impl CallableBinderScope {
    pub(super) fn push_callable(
        &mut self,
        program: &dyn crate::CheckingInput,
        required: SymbolHandle,
        required_parameters: &[Identifier],
        actual: SymbolHandle,
        actual_parameters: &[Identifier],
    ) -> Result<(), &'static str> {
        let (required, complete_required) = callable_parameters(program, required)?;
        let (actual, complete_actual) = callable_parameters(program, actual)?;
        if complete_required != required_parameters || complete_actual != actual_parameters {
            return Err(
                "enclosing callable lifetime correspondence lost its complete own declarations",
            );
        }
        self.push_frame(required, complete_required, actual, complete_actual)
    }

    pub(super) fn push_declarations(
        &mut self,
        program: &dyn crate::CheckingInput,
        required: SymbolHandle,
        actual: SymbolHandle,
    ) -> Result<(), &'static str> {
        let (required, required_parameters) = callable_parameters(program, required)?;
        let (actual, actual_parameters) = callable_parameters(program, actual)?;
        self.push_frame(required, required_parameters, actual, actual_parameters)
    }

    fn push_frame(
        &mut self,
        required: SymbolHandle,
        required_parameters: &[Identifier],
        actual: SymbolHandle,
        actual_parameters: &[Identifier],
    ) -> Result<(), &'static str> {
        if required_parameters.len() != actual_parameters.len() {
            return Err("enclosing callable lifetime telescopes have different arity");
        }
        if required_parameters.is_empty() {
            return Ok(());
        }
        // The actual declaration is compared in the required declaration's
        // arbitrary enclosing environment. Current invocation binders are
        // introduced separately by the ordinary region judgment.
        self.lifetime_frames.push(
            (0..required_parameters.len())
                .map(|ordinal| CallableLifetimeBinding {
                    parameter: LifetimeParameterIdentity {
                        declaration: actual,
                        ordinal,
                    },
                    argument: LifetimeParameterIdentity {
                        declaration: required,
                        ordinal,
                    },
                })
                .collect(),
        );
        Ok(())
    }

    pub(super) fn reversed(&self) -> Self {
        Self {
            static_bindings: self
                .static_bindings
                .iter()
                .map(|binding| BinderBinding {
                    required: binding.actual,
                    actual: binding.required,
                })
                .collect(),
            lifetime_frames: self
                .lifetime_frames
                .iter()
                .map(|frame| {
                    frame
                        .iter()
                        .map(|binding| CallableLifetimeBinding {
                            parameter: binding.argument,
                            argument: binding.parameter,
                        })
                        .collect()
                })
                .collect(),
        }
    }
}

/// Resolve only published callable ownership. A missing telescope is distinct
/// from a retained callable that declares zero own lifetime slots.
fn callable_parameters(
    program: &dyn crate::CheckingInput,
    declaration: SymbolHandle,
) -> Result<(SymbolHandle, &[Identifier]), &'static str> {
    let declaration = program.lifetime_declaration_owner(declaration);
    let owner = if let Some(machine) = program
        .machine_by_symbol(declaration)
        .or_else(|| program.machine_holding_state(declaration))
    {
        machine.symbol
    } else if program.trait_machine_signature(declaration).is_some()
        || program
            .retained_static_machine_contract(declaration)
            .is_some()
    {
        declaration
    } else if let Some((_, signature)) = program.machine_parameter_signature(declaration) {
        signature.symbol
    } else {
        return Err("enclosing callable has no retained declaration telescope");
    };
    let owner = program.lifetime_declaration_owner(owner);
    let parameters =
        typed_trees::type_identity::declaration_lifetime_parameters(program.typed_program(), owner)
            .ok_or("enclosing callable lost its declaration-owned lifetime telescope")?;
    Ok((owner, parameters))
}
