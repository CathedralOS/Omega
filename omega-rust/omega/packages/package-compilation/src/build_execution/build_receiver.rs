//! The host lends its Build only to the entry's checked mutable Build borrow.
//! Keep that source/type association through publication before minting a value.

use psi_compiler::AcceptedTerminalSource;
use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
use terminal_psi::{
    InvocationCodeScope, StructuralAccess, StructuralParameterDeclaration, TerminalModule,
};
use typed_trees::{TypedTrees, machine::Machine, types::TypeReferenceNode};

#[derive(Debug)]
pub(super) struct BuildReceiver {
    invocation: InvocationCodeScope,
    machine: MachineId,
    place: PlaceId,
    structural_type: StructuralTypeId,
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
    let [parameter] = typed.state_parameters(entry) else {
        return Err("Build activation requires its one original Build parameter".into());
    };
    if parameter.is_const || parameter.relevance.is_erased() {
        return Err("Build entry requires an executable mutable borrow of Build".into());
    }
    let TypeReferenceNode::Reference {
        referee,
        access: language_semantics::ReferenceAccess::Mutable,
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
    if !build_time_evaluation::is_exact_toolchain_build_prelude_data(typed, *symbol, "Build") {
        return Err("Build entry receiver must name the exact toolchain Build type".into());
    }
    Ok(*symbol)
}

impl BuildReceiver {
    pub(super) fn admit(
        source: &AcceptedTerminalSource,
        machine: &Machine,
    ) -> Result<Self, String> {
        let build = source_receiver(source.source(), machine)?;
        let elaboration = &source.closure().elaboration;
        let module = &elaboration.module;
        let entry = source
            .entry()
            .and_then(|entry| module.machine(entry))
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
        let mut records = source
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
        let storage =
            terminal_psi::find_structural_type(&module.structural_types, parameter.structural_type)
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
        Ok(Self {
            invocation: module
                .identities
                .invocation_scope()
                .cloned()
                .ok_or("Build receiver has no accepted invocation owner")?,
            machine: entry.id,
            place: parameter.place,
            structural_type: parameter.structural_type,
            storage_identity: record.storage_identity.clone(),
            declaration_identity,
        })
    }

    pub(super) fn parameter_for<'module>(
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
            || parameter.place != self.place
            || parameter.structural_type != self.structural_type
            || parameter.access != StructuralAccess::MutableBorrow
        {
            return Err("published Build entry differs from its admitted mutable receiver".into());
        }
        let storage =
            terminal_psi::find_structural_type(&module.structural_types, self.structural_type)
                .ok_or("published Build receiver has no storage declaration")?;
        if module.identities.text(storage.identity) != Some(self.storage_identity.as_str()) {
            return Err(
                "published Build receiver differs from its admitted storage identity".into(),
            );
        }
        Ok(parameter)
    }
}
