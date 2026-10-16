//! Link ordinary callback body objects without flattening their Terminal namespaces.
//!
//! Each body keeps its original functions, call custody and replay source. Only
//! section coordinates and object symbol handles cross this join. Resolved
//! internal branches remain valid because a complete body text section moves
//! together; imports deduplicate by their atomic locator, never a local machine
//! or boundary number. Undefined private references allow cyclic code linkage
//! without recursively emitting a registrar. Synchronous stack cycles are a
//! separate resource obligation, not a linker recursion.
//!
//! A non-identity native entry is a separate physical region and symbol calling
//! the namespaced ordinary body entry. Aliasing the body symbol to that bridge
//! would send helpers or recursion back through foreign pointer conversion.

use std::sync::Arc;

use crate::{
    FunctionFragmentObjectArtifactError as Error, ObjectArtifact, ObjectCompilerPrivateFunction,
    PrivateFunctionObject, PrivateObjectSymbolBinding,
};
use function_identity::MachineFunctionIdentity;
use object_file::{
    FunctionSymbolPlan, ObjectPlan, ObjectSymbolHandle, RelocationOrigin, SectionKind, SectionPlan,
    SymbolKind, SymbolPlan, SymbolSection, object_function_reference,
};

/// Links the root's reachable private references to complete body objects.
/// Each body retains its local evidence; only section offsets and symbols are
/// embedded. Missing, duplicate, unreachable or incompatible definitions fail.
pub fn compose_private_function_objects(
    source: ObjectArtifact,
    private_objects: &[PrivateFunctionObject],
) -> Result<ObjectArtifact, Error> {
    if private_objects.is_empty() {
        require_definitions(&source.object)?;
        return Ok(source);
    }
    if source.composition_source.is_some() || !source.private_functions.is_empty() {
        return Err(Error::Mismatch(
            "object composition requires an unlinked root",
        ));
    }
    let source = Arc::new(source);
    let mut linked = source.as_ref().clone();
    linked.composition_source = Some(Arc::clone(&source));
    let mut identities = Vec::with_capacity(private_objects.len());
    let mut demands = Vec::new();
    for (_, reference) in source.object.layout.function_symbols.iter() {
        demands.push(reference.identity);
    }
    let mut next_demand = 0;
    while next_demand < demands.len() {
        let private = private_objects
            .iter()
            .find(|private| private.identity == demands[next_demand])
            .ok_or(Error::Mismatch("missing private function definition"))?;
        for (_, reference) in private.body.object.layout.function_symbols.iter() {
            if !demands.contains(&reference.identity) {
                demands.push(reference.identity);
            }
        }
        next_demand += 1;
    }
    for private in private_objects {
        if identities.contains(&private.identity)
            || !demands.contains(&private.identity)
            || private.identity.callback_thunk_placement_index().is_none()
            || private.body.target != linked.target
            || private.body.hosted_receiver.is_some()
            || private.body.physical_entry.is_some()
            || private.body.composition_source.is_some()
            || !private.body.private_functions.is_empty()
        {
            return Err(Error::Mismatch(
                "private body identity, target or entry custody changed",
            ));
        }
        identities.push(private.identity);
        private_symbol(&mut linked.object, private.identity)?;
    }
    for private in private_objects {
        append_body(&mut linked, private)?;
    }
    require_definitions(&linked.object)?;
    let sections = linked
        .object
        .layout
        .sections
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in sections {
        let section = linked.object.layout.sections.get_mut(handle);
        section.size = match section.kind {
            SectionKind::Text => linked.text_bytes.len(),
            SectionKind::Data => linked.data_bytes.len(),
            _ => return Err(Error::Unsupported("private object section kind")),
        };
    }
    Ok(linked)
}

fn private_symbol(
    object: &mut ObjectPlan,
    identity: MachineFunctionIdentity,
) -> Result<ObjectSymbolHandle, Error> {
    if let Some((symbol, _)) = object_function_reference(object, identity) {
        return Ok(symbol);
    }
    let name = identity
        .private_symbol()
        .ok_or(Error::Mismatch("invalid private function identity"))?;
    if object
        .layout
        .symbols
        .iter()
        .any(|(_, symbol)| symbol.name == name)
    {
        return Err(Error::Mismatch("private function symbol collision"));
    }
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name,
        kind: SymbolKind::Function,
        ..SymbolPlan::default()
    });
    object
        .layout
        .function_symbols
        .insert(FunctionSymbolPlan { identity, symbol });
    Ok(symbol)
}

fn append_body(linked: &mut ObjectArtifact, private: &PrivateFunctionObject) -> Result<(), Error> {
    let body = &private.body;
    let prefix = private
        .identity
        .private_symbol()
        .ok_or(Error::Mismatch("invalid private identity"))?;
    let text_offset = align_section(linked, SectionKind::Text, &prefix)?;
    let data_offset = align_section(linked, SectionKind::Data, &prefix)?;
    let mut bindings = arena::Arena::with_capacity(body.object.layout.symbols.len());
    for (local, symbol) in body.object.layout.symbols.iter() {
        let private_reference = body
            .object
            .layout
            .function_symbols
            .iter()
            .find(|(_, row)| row.symbol == local)
            .map(|(_, row)| row.identity);
        let linked_symbol =
            if local == body.object.layout.entry_symbol && private.entry_bridge.is_none() {
                let handle = private_symbol(&mut linked.object, private.identity)?;
                let destination = linked.object.layout.symbols.get_mut(handle);
                if destination.section != SymbolSection::None {
                    return Err(Error::Mismatch("duplicate private function definition"));
                }
                destination.section = SymbolSection::Section(SectionKind::Text);
                destination.offset = symbol
                    .offset
                    .checked_add(text_offset)
                    .ok_or(Error::Overflow)?;
                destination.size = symbol.size;
                handle
            } else if let Some(identity) = private_reference {
                private_symbol(&mut linked.object, identity)?
            } else if symbol.kind == SymbolKind::Import {
                let locator = body
                    .object
                    .layout
                    .normalized_imports
                    .iter()
                    .find(|import| import.symbol == local)
                    .ok_or(Error::Mismatch("private import has no normalized locator"))?;
                if let Some(import) = linked
                    .object
                    .layout
                    .normalized_imports
                    .iter()
                    .find(|import| import.locator == locator.locator)
                {
                    import.symbol
                } else {
                    let name = object_file::normalized_foreign_import_symbol_name(&locator.locator);
                    if linked
                        .object
                        .layout
                        .symbols
                        .iter()
                        .any(|(_, symbol)| symbol.name == name)
                    {
                        return Err(Error::Mismatch("private import symbol name collision"));
                    }
                    linked.object.layout.insert_import(name, &locator.locator)
                }
            } else {
                let mut relocated = symbol.clone();
                relocated.name = format!("{prefix}_{}", symbol.name);
                relocated.offset = symbol
                    .offset
                    .checked_add(section_offset(symbol.section, text_offset, data_offset)?)
                    .ok_or(Error::Overflow)?;
                if linked
                    .object
                    .layout
                    .symbols
                    .iter()
                    .any(|(_, other)| other.name == relocated.name)
                {
                    return Err(Error::Mismatch("private body symbol collision"));
                }
                linked.object.layout.symbols.insert(relocated)
            };
        bindings.insert(PrivateObjectSymbolBinding {
            local,
            linked: linked_symbol,
        });
    }
    for (_, relocation) in body.relocations.records() {
        let mut relocated = relocation.clone();
        relocated.symbol_handle = remap(&bindings, relocation.symbol_handle)?;
        let origin_symbol = remap(&bindings, relocation.origin.symbol_handle())?;
        relocated.origin = match relocation.origin {
            RelocationOrigin::Instruction {
                selected_instruction_index,
                ..
            } => RelocationOrigin::Instruction {
                function_symbol_handle: origin_symbol,
                selected_instruction_index,
            },
            RelocationOrigin::SemanticOperation {
                operation_identity, ..
            } => RelocationOrigin::SemanticOperation {
                function_symbol_handle: origin_symbol,
                operation_identity,
            },
            RelocationOrigin::SemanticEdge { edge_identity, .. } => {
                RelocationOrigin::SemanticEdge {
                    function_symbol_handle: origin_symbol,
                    edge_identity,
                }
            }
            RelocationOrigin::Materialization { .. } => RelocationOrigin::Materialization {
                object_symbol_handle: origin_symbol,
            },
        };
        relocated.offset = relocation
            .offset
            .checked_add(section_offset(
                SymbolSection::Section(relocation.section),
                text_offset,
                data_offset,
            )?)
            .ok_or(Error::Overflow)?;
        linked.relocations.push_record(relocated);
    }
    linked.text_bytes.extend_from_slice(&body.text_bytes);
    linked.data_bytes.extend_from_slice(&body.data_bytes);
    if !linked.data_bytes.is_empty()
        && !linked
            .object
            .layout
            .sections
            .iter()
            .any(|(_, section)| section.kind == SectionKind::Data)
    {
        linked.object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Data,
            size: linked.data_bytes.len(),
            alignment: 16,
        });
    }
    let function = body
        .function_on_machine(body.entry)
        .ok_or(Error::Mismatch("private body entry function missing"))?;
    let mut function = crate::ObjectPrivateFunctionEntry::from_function(
        function,
        remap(&bindings, function.symbol)?,
        function
            .text_offset
            .checked_add(text_offset)
            .ok_or(Error::Overflow)?,
    );
    if let Some(bridge) = &private.entry_bridge {
        let body_symbol = function.symbol;
        let bridge_offset = align_section(linked, SectionKind::Text, &format!("{prefix}_entry"))?;
        let symbol = private_symbol(&mut linked.object, private.identity)?;
        let destination = linked.object.layout.symbols.get_mut(symbol);
        if destination.section != SymbolSection::None {
            return Err(Error::Mismatch("duplicate private entry definition"));
        }
        destination.section = SymbolSection::Section(SectionKind::Text);
        destination.offset = bridge_offset;
        destination.size = bridge.bytes().len();
        linked.text_bytes.extend_from_slice(bridge.bytes());
        linked
            .relocations
            .push_record(object_file::RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: symbol,
                },
                section: SectionKind::Text,
                offset: bridge_offset
                    .checked_add(bridge.call_field_offset())
                    .ok_or(Error::Overflow)?,
                byte_width: 4,
                symbol_handle: body_symbol,
                addend: 0,
                kind: match linked.target.architecture {
                    target::Architecture::Aarch64 => object_file::RelocationKind::Aarch64Branch26,
                    target::Architecture::X86_64 => object_file::RelocationKind::X86_64Relative32,
                },
            });
        // The scalar ABI summary describes the ordinary body, not this physical
        // entry. Do not publish it as if the descriptor bridge had that ABI.
        function.scalar_abi = None;
        function.symbol = symbol;
        function.text_offset = bridge_offset;
        function.byte_count = bridge.bytes().len();
    }
    linked
        .private_functions
        .push(ObjectCompilerPrivateFunction {
            identity: private.identity,
            source_psi: body.psi,
            function,
            body: Arc::clone(body),
            entry_bridge: private.entry_bridge.clone(),
            text_offset,
            data_offset,
            symbol_bindings: bindings,
        });
    Ok(())
}

fn align_section(
    linked: &mut ObjectArtifact,
    section: SectionKind,
    prefix: &str,
) -> Result<usize, Error> {
    let bytes = match section {
        SectionKind::Text => &mut linked.text_bytes,
        SectionKind::Data => &mut linked.data_bytes,
        _ => return Err(Error::Unsupported("private object section kind")),
    };
    let padding = (16 - bytes.len() % 16) % 16;
    if padding != 0 {
        linked.object.layout.symbols.insert(SymbolPlan {
            name: format!("{prefix}_{section:?}_alignment"),
            section: SymbolSection::Section(section),
            offset: bytes.len(),
            size: padding,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        bytes.resize(bytes.len().checked_add(padding).ok_or(Error::Overflow)?, 0);
    }
    Ok(bytes.len())
}

fn section_offset(section: SymbolSection, text: usize, data: usize) -> Result<usize, Error> {
    match section {
        SymbolSection::Section(SectionKind::Text) => Ok(text),
        SymbolSection::Section(SectionKind::Data) => Ok(data),
        SymbolSection::None => Err(Error::Mismatch(
            "private object has an unresolved non-private symbol",
        )),
        _ => Err(Error::Unsupported("private object section kind")),
    }
}

pub(crate) fn remap(
    bindings: &arena::Arena<PrivateObjectSymbolBinding>,
    local: ObjectSymbolHandle,
) -> Result<ObjectSymbolHandle, Error> {
    bindings
        .iter()
        .find(|(_, binding)| binding.local == local)
        .map(|(_, binding)| binding.linked)
        .ok_or(Error::Mismatch(
            "private object symbol has no composition binding",
        ))
}

pub(crate) fn require_definitions(object: &ObjectPlan) -> Result<(), Error> {
    for (_, row) in object.layout.function_symbols.iter() {
        if !object.layout.symbols.is_valid(row.symbol) {
            return Err(Error::Mismatch("private symbol missing"));
        }
        let symbol = object.layout.symbols.get(row.symbol);
        if symbol.kind != SymbolKind::Function
            || symbol.section != SymbolSection::Section(SectionKind::Text)
        {
            return Err(Error::Mismatch("missing private function definition"));
        }
    }
    Ok(())
}

/// An artifact-local namespace and its section/symbol embedding in the image.
pub(crate) struct ObjectNamespace<'artifact> {
    pub artifact: &'artifact ObjectArtifact,
    pub text_offset: usize,
    pub data_offset: usize,
    bindings: Option<&'artifact arena::Arena<PrivateObjectSymbolBinding>>,
}

impl ObjectNamespace<'_> {
    pub fn text_offset(&self, local: usize) -> Result<usize, diagnostics::Diagnostic> {
        self.text_offset
            .checked_add(local)
            .ok_or_else(|| diagnostics::Diagnostic::error("private text coordinate overflows"))
    }

    pub fn data_offset(&self, local: usize) -> Result<usize, diagnostics::Diagnostic> {
        self.data_offset
            .checked_add(local)
            .ok_or_else(|| diagnostics::Diagnostic::error("private data coordinate overflows"))
    }
    pub fn symbol(&self, local: ObjectSymbolHandle) -> Result<ObjectSymbolHandle, Error> {
        match self.bindings {
            Some(bindings) => remap(bindings, local),
            None => Ok(local),
        }
    }
}

pub(crate) fn namespaces(artifact: &ObjectArtifact) -> impl Iterator<Item = ObjectNamespace<'_>> {
    std::iter::once(ObjectNamespace {
        artifact: artifact.composition_source.as_deref().unwrap_or(artifact),
        text_offset: 0,
        data_offset: 0,
        bindings: None,
    })
    .chain(
        artifact
            .private_functions
            .iter()
            .map(|private| ObjectNamespace {
                artifact: &private.body,
                text_offset: private.text_offset,
                data_offset: private.data_offset,
                bindings: Some(&private.symbol_bindings),
            }),
    )
}

/// Retained-artifact audit; production never replays its newly composed output.
pub(crate) fn validate_composition(artifact: &ObjectArtifact) -> Result<(), Error> {
    let Some(source) = &artifact.composition_source else {
        return Ok(());
    };
    super::function_fragments::validation::validate_function_fragment_object_artifact(
        &source
            .fragment_replay
            .as_ref()
            .ok_or(Error::Mismatch("composition root lacks fragment custody"))?
            .0,
        source,
    )?;
    let mut inputs = Vec::with_capacity(artifact.private_functions.len());
    for private in &artifact.private_functions {
        let replay = private
            .body
            .fragment_replay
            .as_ref()
            .ok_or(Error::Mismatch("private object lacks fragment custody"))?;
        super::function_fragments::validation::validate_function_fragment_object_artifact(
            &replay.0,
            &private.body,
        )?;
        crate::private_entry_transport::validate_entry_transport(private)?;
        inputs.push(PrivateFunctionObject {
            identity: private.identity,
            body: Arc::clone(&private.body),
            entry_bridge: private.entry_bridge.clone(),
        });
    }
    let expected = compose_private_function_objects(source.as_ref().clone(), &inputs)?;
    let mut actual = artifact.clone();
    actual.hosted_receiver = None;
    actual.physical_entry = source.physical_entry.clone();
    if actual != expected {
        return Err(Error::Mismatch(
            "private object composition differs from its artifact-local sources",
        ));
    }
    Ok(())
}
