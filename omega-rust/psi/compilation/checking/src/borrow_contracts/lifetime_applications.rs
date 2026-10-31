//! Resolve lifetime application operands before region comparison. Receiving
//! formals and supplying actuals retain independent declaration coordinates;
//! names never identify slots across two callable scopes.

use symbols::SymbolHandle;
use typed_trees::static_applications::StaticMachineLifetimeBinding;
use typed_trees::type_identity::LifetimeParameterIdentity;

#[derive(Clone, Copy)]
pub struct CallableLifetimeBinding {
    pub parameter: LifetimeParameterIdentity,
    pub argument: LifetimeParameterIdentity,
}

/// One already established enclosing comparison. Each side retains its own
/// source-to-canonical normalization. Reversal swaps sides; captured supplied
/// arguments are not invertible binder permutations.
#[derive(Clone, Default)]
pub(crate) struct CallableLifetimeFrame {
    pub(crate) required: Vec<CallableLifetimeBinding>,
    pub(crate) actual: Vec<CallableLifetimeBinding>,
}

#[derive(Clone, Copy)]
pub enum CallableLifetimeApplication<'scope> {
    /// Captured outer frame; this contract's own fresh binders mask it.
    Enclosing {
        bindings: &'scope [CallableLifetimeBinding],
    },
    /// Operands supplied to this selected callable's own invocation binders.
    Invocation {
        bindings: &'scope [CallableLifetimeBinding],
    },
}

/// Resolve an occurrence's named rows in their exact two scopes. This supplies
/// tuple coordinates only; full callable refinement and backing still owe proof.
pub fn resolve_callable_lifetime_bindings(
    program: &dyn crate::CheckingInput,
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    rows: &[StaticMachineLifetimeBinding],
    preceding: &[CallableLifetimeApplication<'_>],
) -> Result<Vec<CallableLifetimeBinding>, &'static str> {
    let owner = program.lifetime_declaration_owner(declaration);
    let mut resolved: Vec<CallableLifetimeBinding> = Vec::with_capacity(rows.len());
    for row in rows {
        let parameter = typed_trees::type_identity::lifetime_parameter_identity(
            program.typed_program(),
            declaration,
            row.parameter.as_str(),
        )
        .filter(|parameter| parameter.declaration == owner && owner.is_valid())
        .ok_or("callable application lost its exact receiving lifetime telescope")?;
        if resolved.iter().any(|prior| prior.parameter == parameter) {
            return Err("callable application repeats a receiving lifetime parameter");
        }
        let mut argument = typed_trees::type_identity::lifetime_parameter_identity(
            program.typed_program(),
            argument_owner,
            row.argument.as_str(),
        )
        .ok_or("callable application lost its exact supplying lifetime scope")?;
        // Preceding frames already contain canonical values. Resolve this
        // supplying occurrence once, outside the new receiving frame.
        for application in preceding.iter().rev() {
            let bindings = match application {
                CallableLifetimeApplication::Enclosing { bindings }
                | CallableLifetimeApplication::Invocation { bindings } => *bindings,
            };
            if let Some(binding) = bindings
                .iter()
                .rev()
                .find(|binding| binding.parameter == argument)
            {
                argument = binding.argument;
                break;
            }
        }
        resolved.push(CallableLifetimeBinding {
            parameter,
            argument,
        });
    }
    Ok(resolved)
}
