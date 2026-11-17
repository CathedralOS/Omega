//! Resolve a signature's own lifetime telescope under the exact supplying
//! declaration and preceding applications. This is source formation metadata.

use crate::static_applications::StaticMachineLifetimeBinding;
use crate::type_identity::LifetimeParameterIdentity;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub struct CallableLifetimeBinding {
    pub parameter: LifetimeParameterIdentity,
    pub argument: LifetimeParameterIdentity,
}

#[derive(Clone, Default)]
pub struct CallableLifetimeFrame {
    pub required: Vec<CallableLifetimeBinding>,
    pub actual: Vec<CallableLifetimeBinding>,
}

#[derive(Clone, Copy)]
pub enum CallableLifetimeApplication<'scope> {
    Enclosing {
        bindings: &'scope [CallableLifetimeBinding],
    },
    Invocation {
        bindings: &'scope [CallableLifetimeBinding],
    },
}

pub fn resolve_callable_lifetime_bindings(
    program: &crate::TypedTrees,
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    rows: &[StaticMachineLifetimeBinding],
    preceding: &[CallableLifetimeApplication<'_>],
) -> Result<Vec<CallableLifetimeBinding>, &'static str> {
    let owner = program.lifetime_declaration_owner(declaration);
    let mut resolved: Vec<CallableLifetimeBinding> = Vec::with_capacity(rows.len());
    for row in rows {
        let parameter = crate::type_identity::lifetime_parameter_identity(
            program,
            declaration,
            row.parameter.as_str(),
        )
        .filter(|parameter| parameter.declaration == owner && owner.is_valid())
        .ok_or("callable application lost its exact receiving lifetime telescope")?;
        if resolved.iter().any(|prior| prior.parameter == parameter) {
            return Err("callable application repeats a receiving lifetime parameter");
        }
        let mut argument = crate::type_identity::lifetime_parameter_identity(
            program,
            argument_owner,
            row.argument.as_str(),
        )
        .ok_or("callable application lost its exact supplying lifetime scope")?;
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
