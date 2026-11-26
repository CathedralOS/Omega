//! The checked source entry identifies the one published parameter that may borrow the host’s Build.

use crate::AcceptedTerminalSource;
use crate::source_state::{TypedTrees, machine::Machine, types::TypeReferenceNode};
use semantic_vocabulary::MachineId;
use terminal_psi::{
    InvocationCodeScope, StructuralAccess, StructuralParameterDeclaration, TerminalModule,
};

/// Identifies the parameter the host may lend its Build to after source checking.
#[derive(Debug)]
pub struct CheckedBuildReceiver {
    invocation: InvocationCodeScope,
    machine: MachineId,
    receiver: StructuralParameterDeclaration,
    storage_identity: String,
    declaration_identity: String,
}

fn source_receiver(typed: &TypedTrees, machine: &Machine) -> Result<symbols::SymbolHandle, String> {
    let parent = typed.symbols.get(machine.symbol).parent;
    if !parent.is_valid()
        || !matches!(
            typed.symbols.get(parent).kind,
            symbols::SymbolKind::Root | symbols::SymbolKind::Module
        )
        || machine.attached_data.is_some()
    {
        return Err("Build entry must be a free machine in the selected Build source".into());
    }
    let entry = typed
        .machine_states(machine)
        .first()
        .ok_or("Build entry has no source entry state")?;
    let Some([parameter]) = typed.state_parameters(entry).as_array() else {
        return Err("Build activation requires its one original Build parameter".into());
    };
    if parameter.is_const || parameter.relevance.is_erased() {
        return Err("Build entry requires an executable mutable borrow of Build".into());
    }
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = typed
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return Err("Build entry requires an executable mutable borrow of Build".into());
    };
    let TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*referee)
    else {
        return Err("Build entry receiver must name the exact toolchain Build type".into());
    };
    if !typed.is_exact_toolchain_build_prelude_data(*symbol, "Build") {
        return Err("Build entry receiver must name the exact toolchain Build type".into());
    }
    Ok(*symbol)
}

impl AcceptedTerminalSource {
    /// Identify the toolchain Build borrow on this invocation's selected source declaration.
    pub fn build_receiver(
        &self,
        selected: symbols::SymbolHandle,
    ) -> Result<CheckedBuildReceiver, String> {
        let elaboration = self.closure().elaboration();
        let module = &elaboration.module;
        let entry_id = self
            .entry()
            .filter(|&id| self.is_invocation() && elaboration.machines.get(id) == Some(&selected))
            .ok_or("Build entry differs from its accepted invocation root")?;
        let machine = self
            .source()
            .machine(selected)
            .ok_or("prepared Build program lost its selected entry")?;
        let build = source_receiver(self.source(), machine)?;
        let entry = module
            .machine(entry_id)
            .ok_or("Build entry has no accepted machine")?;
        let [parameter] = entry.structural_parameters.as_slice() else {
            return Err("Build activation requires its one original structural receiver".into());
        };
        if !entry.parameters.is_empty()
            || entry.parameter_order != [terminal_psi::BoundaryParameterKind::Structural]
            || parameter.position != 0
            || parameter.access != StructuralAccess::MutableBorrow
        {
            return Err("accepted Build entry lost its one mutable Build receiver".into());
        }
        let mut records = self
            .source_records()
            .iter()
            .filter(|row| row.record == parameter.structural_type);
        let (Some(record), None) = (records.next(), records.next()) else {
            return Err("accepted Build receiver has no unique source record".into());
        };
        if record.declaration != build {
            return Err(
                "accepted Build receiver differs from the exact toolchain Build declaration".into(),
            );
        }
        let storage = terminal_psi::find_structural_type(
            &module.types.structural_types,
            parameter.structural_type,
        )
        .ok_or("accepted Build receiver has no storage declaration")?;
        if module.identities.text(storage.identity) != Some(record.storage_identity.as_str()) {
            return Err("accepted Build receiver lost its source storage identity".into());
        }
        let declaration_identity = entry
            .declaration
            .as_ref()
            .and_then(|declaration| module.identities.text(declaration.semantic_declaration))
            .ok_or("accepted Build entry has no checked semantic declaration")?
            .to_owned();
        Ok(CheckedBuildReceiver {
            invocation: module
                .identities
                .invocation_scope()
                .cloned()
                .ok_or("Build receiver has no accepted invocation owner")?,
            machine: entry.id,
            receiver: parameter.clone(),
            storage_identity: record.storage_identity.clone(),
            declaration_identity,
        })
    }
}

impl CheckedBuildReceiver {
    /// Check the actual invocation before the host lends its Build to this parameter.
    pub fn parameter_for<'module>(
        &self,
        module: &'module TerminalModule,
    ) -> Result<&'module StructuralParameterDeclaration, String> {
        if module.identities.invocation_scope() != Some(&self.invocation)
            || module.entry
                != Some(semantic_vocabulary::TargetMachineCallable::Machine(
                    self.machine,
                ))
        {
            return Err("Build receiver belongs to a different accepted invocation".into());
        }
        let entry = module
            .machine(self.machine)
            .ok_or("published Build entry is absent")?;
        let declaration = entry
            .declaration
            .as_ref()
            .ok_or("published Build entry lost its declaration")?;
        if module.identities.text(declaration.semantic_declaration)
            != Some(self.declaration_identity.as_str())
        {
            return Err(
                "published Build entry differs from its admitted source declaration".into(),
            );
        }
        let [parameter] = entry.structural_parameters.as_slice() else {
            return Err("published Build entry lost its one receiver".into());
        };
        if !entry.parameters.is_empty()
            || entry.parameter_order != [terminal_psi::BoundaryParameterKind::Structural]
            || parameter.position != 0
            || parameter != &self.receiver
            || parameter.access != StructuralAccess::MutableBorrow
        {
            return Err("published Build entry differs from its admitted mutable receiver".into());
        }
        let storage = terminal_psi::find_structural_type(
            &module.types.structural_types,
            self.receiver.structural_type,
        )
        .ok_or("published Build receiver has no storage declaration")?;
        if module.identities.text(storage.identity) != Some(self.storage_identity.as_str()) {
            return Err(
                "published Build receiver differs from its admitted storage identity".into(),
            );
        }
        Ok(parameter)
    }
}
