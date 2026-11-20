//! The host lends its Build only to the entry's checked mutable Build borrow.
//! Keep that source/type association through publication before minting a value.

use semantic_vocabulary::{MachineId, ScalarType, StructuralTypeId};
use source_compiler::publication::AcceptedTerminalSource;
use source_compiler::representations::source_state::{
    TypedTrees, machine::Machine, signature::StateParameter, types::TypeReferenceNode,
};
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
    kind: FrameHomeType,
    saved: bool,
    parameter: StructuralParameterDeclaration,
}

#[derive(Debug, PartialEq, Eq)]
enum FrameHomeType {
    Scalar(ScalarType),
    DependencyRegistration {
        declaration: String,
        storage: String,
    },
}

/// Source record correspondence ties both the nominal declaration and its stored
/// representation to the particular admitted module, whose type numbers are local.
fn record_identity<'a>(
    source: &'a AcceptedTerminalSource,
    module: &TerminalModule,
    id: StructuralTypeId,
    declaration: symbols::SymbolHandle,
) -> Option<&'a str> {
    let mut records = source
        .source_records()
        .iter()
        .filter(|row| row.record == id);
    let record = records.next()?;
    let storage = module.structural_type(id)?;
    (records.next().is_none()
        && record.declaration == declaration
        && module.identities.text(storage.identity) == Some(record.storage_identity.as_str()))
    .then_some(record.storage_identity.as_str())
}

impl FrameHomeType {
    fn admit(
        source: &AcceptedTerminalSource,
        module: &TerminalModule,
        parameter: &StateParameter,
        slot: &StructuralParameterDeclaration,
    ) -> Option<Self> {
        let typed = source.source();
        let symbol = mutable_referent(typed, parameter)?;
        let ty = module.structural_type(slot.structural_type)?;
        if let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) = ty.shape {
            let atom = typed.symbols.builtin_type_atom(symbol)?;
            let expected = match symbols::PrimitiveType::from_builtin_atom(atom)? {
                symbols::PrimitiveType::Bool => ScalarType::Boolean,
                symbols::PrimitiveType::F32 => {
                    ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32)
                }
                symbols::PrimitiveType::F64 => {
                    ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64)
                }
                symbols::PrimitiveType::Integer(carrier) => {
                    ScalarType::Integer(semantic_vocabulary::IntegerType::from_fixed(carrier)?)
                }
                symbols::PrimitiveType::Addr | symbols::PrimitiveType::Selector => return None,
            };
            return (scalar == expected).then_some(Self::Scalar(scalar));
        }
        if !super::build_value::is_exact_toolchain_build_prelude_data(
            typed,
            symbol,
            "DependencyRegistration",
        ) || !registration_shape(ty)
        {
            return None;
        }
        Some(Self::DependencyRegistration {
            declaration: module.identities.text(ty.declaration_identity)?.to_owned(),
            storage: record_identity(source, module, ty.id, symbol)?.to_owned(),
        })
    }

    fn matches_published(&self, module: &TerminalModule, id: StructuralTypeId) -> bool {
        let Some(ty) = module.structural_type(id) else {
            return false;
        };
        match self {
            Self::Scalar(expected) => {
                matches!(ty.shape, terminal_psi::StructuralTypeShape::PrimitiveScalar(actual) if actual == *expected)
            }
            Self::DependencyRegistration {
                declaration,
                storage,
            } => {
                registration_shape(ty)
                    && module.identities.text(ty.declaration_identity) == Some(declaration.as_str())
                    && module.identities.text(ty.identity) == Some(storage.as_str())
            }
        }
    }
}

fn registration_shape(ty: &terminal_psi::StructuralTypeDeclaration) -> bool {
    ty.is_copyable()
        && matches!(&ty.shape, terminal_psi::StructuralTypeShape::Record {
            fields, storage: terminal_psi::StructuralRecordStorage::Packed,
        } if fields.is_empty())
}

fn mutable_referent(
    typed: &TypedTrees,
    parameter: &StateParameter,
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

fn source_receiver<'a>(
    typed: &'a TypedTrees,
    machine: &Machine,
    stage: &source_compiler::BuildStage,
) -> Result<(symbols::SymbolHandle, arena::RootRows<'a, StateParameter>), String> {
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
    Ok((symbol, parameters.slice(1..parameters.len())))
}

impl BuildReceiver {
    pub(super) fn admit(
        source: &AcceptedTerminalSource,
        machine: &Machine,
        stage: &source_compiler::BuildStage,
    ) -> Result<Self, String> {
        let (build, source_slots) = source_receiver(source.source(), machine, stage)?;
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
        if frame.len() != source_slots.len() {
            return Err("accepted Build entry changed its number of continuation homes".into());
        }
        let mut homes = Vec::with_capacity(frame.len());
        for (index, ((slot, source_slot), &local)) in frame
            .iter()
            .zip(source_slots.iter())
            .zip(stage.frame_homes())
            .enumerate()
        {
            if slot.position as usize != index + 1
                || slot.access != StructuralAccess::MutableBorrow
                || !slot.qualifications.is_empty()
                || !slot.projected_qualifications.is_empty()
            {
                return Err("Build continuation requires a mutable storage home for each captured declaration".into());
            }
            let kind = FrameHomeType::admit(source, module, source_slot, slot)
                .ok_or("Build frame home differs from its builtin scalar or toolchain registration declaration")?;
            homes.push(AdmittedFrameHome {
                local,
                saved: stage.saves_frame_home(local),
                kind,
                parameter: slot.clone(),
            });
        }
        let storage_identity = record_identity(source, module, parameter.structural_type, build)
            .ok_or("accepted Build receiver differs from its toolchain declaration or storage identity")?;
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
            frame: homes,
            storage_identity: storage_identity.to_owned(),
            declaration_identity,
        })
    }

    pub(super) fn has_same_frame(&self, previous: &Self) -> bool {
        self.frame
            .iter()
            .map(|home| (home.local, &home.kind))
            .eq(previous.frame.iter().map(|home| (home.local, &home.kind)))
    }

    pub(super) fn registration_frame_positions(&self) -> impl Iterator<Item = u32> + '_ {
        self.frame
            .iter()
            .filter(|home| {
                home.saved && matches!(home.kind, FrameHomeType::DependencyRegistration { .. })
            })
            .map(|home| home.parameter.position)
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
            || frame.len() != self.frame.len()
            || frame.iter().zip(&self.frame).any(|(slot, home)| {
                slot != &home.parameter
                    || !home.kind.matches_published(module, slot.structural_type)
            })
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
