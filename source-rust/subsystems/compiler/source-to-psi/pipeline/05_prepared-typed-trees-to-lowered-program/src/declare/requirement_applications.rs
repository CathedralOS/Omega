//! Publish the exact ordered static operands of a closed boundary requirement.
//! Selected callback bodies contribute their own completed declaration contracts.

use super::{NominalDeclarationIdentity, SymbolHandle, TerminalIdentityTableBuilder, TypedTrees};
use application_identity::conformance_identity::MachineOperandSelectionIdentity;

#[derive(Debug, Clone)]
enum RequirementArgumentSource {
    Type(String),
    Const(String),
    Machine(SymbolHandle),
}

/// Normalize an application once while its typed source is available. The
/// publishing route interns these same identities into its own module table.
#[derive(Debug, Clone)]
pub(super) struct RequirementApplicationSource {
    callable: String,
    arguments: Vec<RequirementArgumentSource>,
    commitment: [u8; 32],
}

impl RequirementApplicationSource {
    pub(super) fn read(
        typed: &TypedTrees,
        closed: &crate::source_state::ClosedRequirementApplication<'_>,
    ) -> Result<Self, &'static str> {
        use crate::source_state::static_applications::StaticMachineBindingValue;
        let (owner, template) = typed
            .trait_machine_signature(closed.template)
            .filter(|(owner, _)| owner.symbol == closed.declaring_trait)
            .ok_or("a closed requirement's requirement has no typed-tree declaration")?;
        let callable = typed.trait_requirement_callable_identity_text(owner, template);
        if callable.is_empty() {
            return Err("a closed requirement's requirement has no callable identity");
        }
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(closed.application);
        // Declaring-trait arguments belong to the closed callable identity
        // and ABI's declaring_application. This tuple holds only the
        // method's own binders, so it is empty for Trait<T>::value().
        let parameters = typed.state_signature_type_parameters(template);
        if bindings.len() != parameters.len()
            || bindings.len() != closed.portable_local_arguments.len()
            || bindings
                .iter()
                .zip(parameters)
                .any(|(binding, parameter)| binding.parameter != parameter.symbol)
        {
            return Err("a closed requirement binds other than one argument per parameter");
        }
        let mut selected = Vec::with_capacity(bindings.len());
        let mut arguments = Vec::with_capacity(bindings.len());
        for (binding, identity) in bindings.iter().zip(closed.portable_local_arguments) {
            let text = typed
                .type_identity_text(*identity)
                .filter(|text| !text.is_empty())
                .ok_or("a closed requirement's argument has no identity")?;
            match &binding.value {
                StaticMachineBindingValue::Type(_) => {
                    selected.push(MachineOperandSelectionIdentity::Type(text.clone()));
                    arguments.push(RequirementArgumentSource::Type(text));
                }
                StaticMachineBindingValue::Const(_) => {
                    selected.push(MachineOperandSelectionIdentity::Const(text.clone()));
                    arguments.push(RequirementArgumentSource::Const(text));
                }
                StaticMachineBindingValue::Machine(argument) => {
                    if argument.application.is_some()
                        || argument.authored_machine_application.is_valid()
                    {
                        return Err(
                            "an applied requirement callback needs one closed, selected machine",
                        );
                    }
                    let machine = typed
                        .machine_by_symbol(argument.symbol)
                        .or_else(|| {
                            typed
                                .unique_machine_by_entry_state_symbol(argument.symbol)
                                .map(|(machine, _)| machine)
                        })
                        .ok_or("an applied requirement callback has no selected machine")?;
                    let identity = typed
                        .normalized_hermetic_symbol_identity(machine.symbol)
                        .map_err(
                            |_| "the selected callback has no portable declaration identity",
                        )?;
                    selected.push(MachineOperandSelectionIdentity::Machine(identity));
                    arguments.push(RequirementArgumentSource::Machine(machine.symbol));
                }
                StaticMachineBindingValue::Conformance(_)
                | StaticMachineBindingValue::RuntimeValue { .. }
                | StaticMachineBindingValue::Proof(_) => {
                    return Err("a closed requirement binds an unsupported static operand");
                }
            }
        }
        let commitment =
            application_identity::conformance_identity::requirement_application_commitment(
                &callable, &selected,
            )?;
        Ok(Self {
            callable,
            arguments,
            commitment,
        })
    }

    /// Type and constant arguments are self-contained. A machine argument
    /// additionally names a body contract, assigned after the body headers.
    pub(super) fn header_arguments(
        &self,
        identities: &mut TerminalIdentityTableBuilder,
    ) -> Vec<terminal_psi::ClosedReachArgument> {
        self.publish_arguments(identities, |_| None)
            .unwrap_or_default()
    }

    fn publish_arguments(
        &self,
        identities: &mut TerminalIdentityTableBuilder,
        mut callable: impl FnMut(
            SymbolHandle,
        ) -> Option<(terminal_psi::TerminalIdentityIndex, [u8; 32])>,
    ) -> Option<Vec<terminal_psi::ClosedReachArgument>> {
        self.arguments
            .iter()
            .map(|argument| {
                Some(match argument {
                    RequirementArgumentSource::Type(text) => {
                        terminal_psi::ClosedReachArgument::Type(identities.intern(text))
                    }
                    RequirementArgumentSource::Const(text) => {
                        terminal_psi::ClosedReachArgument::Const(identities.intern(text))
                    }
                    RequirementArgumentSource::Machine(symbol) => {
                        let (identity, contract_commitment) = callable(*symbol)?;
                        terminal_psi::ClosedReachArgument::Machine {
                            identity,
                            contract_commitment,
                        }
                    }
                })
            })
            .collect()
    }

    pub(super) fn publish_specialization(
        &self,
        template: terminal_psi::TerminalIdentityIndex,
        semantic: terminal_psi::TerminalIdentityIndex,
        identities: &mut TerminalIdentityTableBuilder,
    ) -> terminal_psi::CheckedMachineSpecialization {
        terminal_psi::CheckedMachineSpecialization {
            capture: None,
            template_entry: NominalDeclarationIdentity::new(template),
            template: NominalDeclarationIdentity::new(template),
            template_callable: identities.intern(&self.callable),
            template_semantic_declaration: semantic,
            commitment: self.commitment,
        }
    }
}

pub(super) fn complete_callbacks(
    program: &crate::read::Program<'_>,
    module: &mut terminal_psi::TerminalModule,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<(), &'static str> {
    let selected_application = |id| {
        let applied = program
            .boundaries
            .rows
            .get(id)?
            .declaration
            .application
            .as_ref()?;
        applied
            .application
            .arguments
            .iter()
            .any(|argument| matches!(argument, RequirementArgumentSource::Machine(_)))
            .then_some(&applied.application)
    };
    if !module
        .interfaces
        .boundary_machines
        .iter()
        .any(|row| selected_application(row.id).is_some())
    {
        return Ok(());
    }
    let mut contracts: Vec<_> = module
        .machines
        .iter()
        .filter_map(|machine| {
            Some((
                program.symbols.get(machine.id)?.arena_index(),
                machine.declaration.as_ref()?,
            ))
        })
        .collect();
    contracts.sort_unstable_by_key(|(symbol, _)| *symbol);
    for boundary in &mut module.interfaces.boundary_machines {
        let Some(application) = selected_application(boundary.id) else {
            continue;
        };
        let arguments = application
            .publish_arguments(identities, |symbol| {
                let key = symbol.arena_index();
                let start = contracts.partition_point(|(id, _)| *id < key);
                let (id, declaration) = *contracts.get(start)?;
                if id != key || contracts.get(start + 1).is_some_and(|(id, _)| *id == key) {
                    return None;
                }
                Some((declaration.semantic_declaration, declaration.contract))
            })
            .ok_or("a requirement callback lost its exact checked declaration and contract")?;
        boundary
            .declaration
            .as_mut()
            .ok_or("a requirement callback lost its receiving boundary")?
            .method_arguments = arguments;
    }
    Ok(())
}
