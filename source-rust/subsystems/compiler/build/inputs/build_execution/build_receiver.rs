//! The host lends its Build only to the entry's checked mutable Build borrow.
//! Keep that source/type association through publication before minting a value.

use semantic_vocabulary::MachineId;
use source_compiler::AcceptedTerminalSource;
use source_compiler::source_state::{TypedTrees, machine::Machine, types::TypeReferenceNode};
use terminal_psi::{
    InvocationCodeScope, StructuralAccess, StructuralParameterDeclaration, TerminalModule,
};

#[derive(Debug)]
pub(super) struct BuildReceiver {
    invocation: InvocationCodeScope,
    machine: MachineId,
    receiver: StructuralParameterDeclaration,
    frame: Vec<AdmittedFrameHome>,
    storage_identity: String,
    declaration_identity: String,
}

#[derive(Debug)]
struct AdmittedFrameHome {
    local: source_text::Span,
    parameter: StructuralParameterDeclaration,
}

fn mutable_referent(
    typed: &TypedTrees,
    parameter: &source_compiler::source_state::signature::StateParameter,
) -> Option<symbols::SymbolHandle> {
    if parameter.is_const || parameter.relevance.is_erased() {
        return None;
    }
    let types = &typed.type_reference_table;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = types.type_reference(parameter.type_reference)
    else {
        return None;
    };
    let TypeReferenceNode::Named { symbol, .. } = types.type_reference(*referee) else {
        return None;
    };
    Some(*symbol)
}

fn source_receiver(
    typed: &TypedTrees,
    machine: &Machine,
    stage: &source_compiler::BuildStage,
) -> Result<symbols::SymbolHandle, String> {
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
    let parameters = typed.state_parameters(entry);
    if parameters.len() != 1 + stage.frame_homes().len() {
        return Err("Build activation requires its one original Build parameter".into());
    }
    let entry_span = typed
        .symbols
        .symbol_source_span(machine.symbol)
        .ok_or("Build entry has no source declaration")?;
    for (slot, &home) in parameters.iter().skip(1).zip(stage.frame_homes()) {
        if mutable_referent(typed, slot)
            .and_then(|symbol| typed.symbols.builtin_type_atom(symbol))
            .and_then(symbols::PrimitiveType::from_builtin_atom)
            .is_none()
        {
            return Err("Build frame home requires a mutable borrow of a builtin scalar".into());
        }
        if slot.name.as_str() != source_compiler::BuildStage::frame_parameter_name(home)
            || typed.symbols.symbol_source_span(slot.symbol)
                != Some(source_text::SourceSpan::new(entry_span.source_id, home))
        {
            return Err("Build frame parameter differs from its local declaration".into());
        }
    }
    let symbol = mutable_referent(typed, &parameters[0])
        .ok_or("Build entry requires an executable mutable borrow of Build")?;
    if !super::build_value::is_exact_toolchain_build_prelude_data(typed, symbol, "Build") {
        return Err("Build entry receiver must name the exact toolchain Build type".into());
    }
    Ok(symbol)
}

impl BuildReceiver {
    pub(super) fn admit(
        source: &AcceptedTerminalSource,
        machine: &Machine,
        stage: &source_compiler::BuildStage,
    ) -> Result<Self, String> {
        let build = source_receiver(source.source(), machine, stage)?;
        let elaboration = &source.closure().elaboration();
        let module = &elaboration.module;
        let entry = source
            .entry()
            .and_then(|entry| module.machine(entry))
            .ok_or("Build entry has no accepted machine")?;
        let [parameter, frame @ ..] = entry.structural_parameters.as_slice() else {
            return Err("Build activation requires its one original structural receiver".into());
        };
        if !entry.parameters.is_empty()
            || entry.parameter_order.len() != 1 + frame.len()
            || entry
                .parameter_order
                .iter()
                .any(|kind| *kind != terminal_psi::BoundaryParameterKind::Structural)
            || parameter.position != 0
            || parameter.access != StructuralAccess::MutableBorrow
        {
            return Err("accepted Build entry lost its one mutable Build receiver".into());
        }
        if frame.len() != stage.frame_homes().len()
            || frame.iter().enumerate().any(|(i, slot)| {
                slot.position as usize != i + 1
                    || slot.access != StructuralAccess::MutableBorrow
                    || !slot.qualifications.is_empty()
                    || !slot.projected_qualifications.is_empty()
                    || !module
                        .structural_type(slot.structural_type)
                        .is_some_and(|ty| {
                            matches!(
                                ty.shape,
                                terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                            )
                        })
            })
        {
            return Err("Build continuation requires a checked scalar storage home for each captured declaration".into());
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
        Ok(Self {
            invocation: module
                .identities
                .invocation_scope()
                .cloned()
                .ok_or("Build receiver has no accepted invocation owner")?,
            machine: entry.id,
            receiver: parameter.clone(),
            frame: stage
                .frame_homes()
                .iter()
                .copied()
                .zip(frame.iter().cloned())
                .map(|(local, parameter)| AdmittedFrameHome { local, parameter })
                .collect(),
            storage_identity: record.storage_identity.clone(),
            declaration_identity,
        })
    }

    pub(super) fn has_same_frame(&self, previous: &Self) -> bool {
        self.frame
            .iter()
            .map(|home| home.local)
            .eq(previous.frame.iter().map(|home| home.local))
    }

    pub(super) fn parameters_for<'module>(
        &self,
        module: &'module TerminalModule,
    ) -> Result<&'module [StructuralParameterDeclaration], String> {
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
        let [parameter, frame @ ..] = entry.structural_parameters.as_slice() else {
            return Err("published Build entry lost its one receiver".into());
        };
        if !entry.parameters.is_empty()
            || entry.parameter_order.len() != 1 + frame.len()
            || entry
                .parameter_order
                .iter()
                .any(|kind| *kind != terminal_psi::BoundaryParameterKind::Structural)
            || parameter.position != 0
            || !frame
                .iter()
                .eq(self.frame.iter().map(|home| &home.parameter))
            || parameter != &self.receiver
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
        Ok(&entry.structural_parameters)
    }
}
