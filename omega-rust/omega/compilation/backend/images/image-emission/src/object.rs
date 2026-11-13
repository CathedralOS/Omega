/*
We coordinate the object-level composition and component-container lanes:
`private_object_composition` embeds private callback function objects into a
root `ObjectArtifact` without colliding local symbol namespaces, while
`component_object` and `component_shared_image` prepare and emit ELF, PE, and
Mach-O shared-component images.
*/

pub(crate) mod component_object {
    use crate::entries::callable_entry_transport::native_entry_request;
    use diagnostics::Diagnostic;
    use image::{
        FinalImage, FinalImageInput, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
    };
    use machine_emission::native_entry_bridge::NativeEntryBridge;
    use object_file::{RelocationKind, SymbolKind};

    pub struct ComponentFunctionImage {
        pub(crate) body: crate::ObjectArtifact,
        pub(crate) entry_bridge: Option<NativeEntryBridge>,
        pub(crate) image: FinalImage,
        pub(crate) export_symbol: image::FinalImageSymbolHandle,
        pub(crate) slot_commitment: [u8; 32],
    }

    impl ComponentFunctionImage {
        pub const fn image(&self) -> &FinalImage {
            &self.image
        }
        pub const fn slot_commitment(&self) -> &[u8; 32] {
            &self.slot_commitment
        }
    }

    pub fn prepare_component_function_image(
        body: crate::ObjectArtifact,
        transport: crate::CallableEntryTransport,
        slot_commitment: [u8; 32],
    ) -> Result<ComponentFunctionImage, Diagnostic> {
        let invalid =
            |reason: &str| Diagnostic::error(format!("component function image: {reason}"));
        if slot_commitment == [0; 32]
            || body.hosted_receiver_binding().is_some()
            || body.physical_entry_binding().is_some()
            || !body.private_functions().is_empty()
        {
            return Err(invalid(
                "export lacks an exact slot or carries another entry's installation",
            ));
        }
        crate::entries::callable_entry_transport::validate_callable_transport(&body, &transport)
            .map_err(|error| invalid(&error.to_string()))?;
        let entry_bridge = transport.bridge;
        let entry_effects = transport.effects;
        let mut image = image::build_final_image(FinalImageInput {
            target: body.target(),
            object: body.object(),
            relocations: body.relocations(),
            text_bytes: body.text_bytes(),
            data_bytes: body.data_bytes(),
        });
        let body_symbol = image.symbol_table.entry_symbol;
        let export_symbol = match &entry_bridge {
            None => body_symbol,
            Some(bridge) => {
                if bridge.request().target != body.target() {
                    return Err(invalid("entry bridge and body targets differ"));
                }
                let ordinals = bridge
                    .request()
                    .parameters
                    .iter()
                    .map(|(_, parameter)| parameter.inbound_ordinal)
                    .collect::<Vec<_>>();
                let expected = native_entry_request(&body, &bridge.request().inbound, &ordinals)
                    .map_err(|error| invalid(&error.to_string()))?;
                if expected != *bridge.request() {
                    return Err(invalid(
                        "entry bridge belongs to a different ordinary body ABI",
                    ));
                }
                machine_emission::native_entry_bridge::validate_native_entry_bridge(bridge)
                    .map_err(|error| invalid(&error.to_string()))?;
                let offset = image
                    .memory
                    .text
                    .len()
                    .checked_add(15)
                    .ok_or_else(|| invalid("entry bridge offset overflows"))?
                    & !15;
                image.memory.text.resize(offset, 0);
                image.memory.text.extend_from_slice(bridge.bytes());
                let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
                    name: component_export_symbol(&slot_commitment),
                    section: FinalImageSection::Text,
                    offset,
                    size: bridge.bytes().len(),
                    kind: SymbolKind::Function,
                });
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset: offset
                            .checked_add(bridge.call_field_offset())
                            .ok_or_else(|| invalid("entry bridge call offset overflows"))?,
                        byte_width: 4,
                        symbol_handle: body_symbol,
                        addend: 0,
                        kind: match body.target().architecture {
                            target::Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
                            target::Architecture::X86_64 => RelocationKind::X86_64Relative32,
                        },
                    });
                symbol
            }
        };
        let export = image.symbol_table.symbols.get(export_symbol);
        let offset = export.offset;
        let size = export.size;
        if let Some(region) = image
            .executable_regions
            .iter_mut()
            .find(|region| region.section_offset == offset && region.byte_count == size)
        {
            region.entry_effects = Some(entry_effects.clone());
        } else if let Some(bridge) = &entry_bridge {
            image.executable_regions.push(image::FinalExecutableRegion {
                origin: image::FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: offset,
                byte_count: size,
                symbol: export.name.clone(),
                footprint: Some(bridge.generated().effects.footprint().clone()),
                entry_effects: Some(entry_effects.clone()),
            });
        } else {
            return Err(invalid("identity entry has no exact body region"));
        }
        Ok(ComponentFunctionImage {
            body,
            entry_bridge,
            image,
            export_symbol,
            slot_commitment,
        })
    }

    pub fn component_export_symbol(commitment: &[u8; 32]) -> String {
        use std::fmt::Write;
        let mut symbol = String::from("__omega_slot_");
        for byte in commitment {
            write!(&mut symbol, "{byte:02x}").expect("writing a component symbol to a String");
        }
        symbol
    }
}

pub(crate) mod component_shared_image {
    use diagnostics::Diagnostic;
    use machine_emission::native_entry_bridge::NativeEntryBridge;

    #[derive(Debug)]
    pub enum ComponentSharedContainer {
        Elf(image_elf::ElfSharedComponentImage),
        MachO(image_macho::MachoSharedComponentImage),
        Pe(image_pe::PeSharedComponentImage),
    }

    impl ComponentSharedContainer {
        pub const fn image(&self) -> &image::FinalImage {
            match self {
                Self::Elf(image) => image.image(),
                Self::MachO(image) => image.image(),
                Self::Pe(image) => image.image(),
            }
        }
        pub fn final_text_bytes(&self) -> &[u8] {
            match self {
                Self::Elf(image) => image.final_text_bytes(),
                Self::MachO(image) => image.final_text_bytes(),
                Self::Pe(image) => image.final_text_bytes(),
            }
        }
        pub fn contents(&self) -> &[u8] {
            match self {
                Self::Elf(image) => image.contents(),
                Self::MachO(image) => image.contents(),
                Self::Pe(image) => image.contents(),
            }
        }
    }

    pub struct ComponentSharedImage {
        body: crate::ObjectArtifact,
        entry_bridge: Option<NativeEntryBridge>,
        image: ComponentSharedContainer,
        slot_commitment: [u8; 32],
    }

    impl ComponentSharedImage {
        pub const fn body(&self) -> &crate::ObjectArtifact {
            &self.body
        }
        pub const fn entry_bridge(&self) -> Option<&NativeEntryBridge> {
            self.entry_bridge.as_ref()
        }
        pub const fn image(&self) -> &ComponentSharedContainer {
            &self.image
        }
        pub const fn slot_commitment(&self) -> &[u8; 32] {
            &self.slot_commitment
        }

        pub fn stack_demand(&self) -> Result<crate::StackDemand, Diagnostic> {
            let mut demand = crate::derive_stack_demand(&self.body, self.body.entry())
                .map_err(|error| Diagnostic::error(format!("component stack demand: {error:?}")))?;
            if let Some(bridge) = &self.entry_bridge {
                demand.stack_alignment = demand
                    .stack_alignment
                    .max(u32::from(bridge.request().inbound.call.stack_alignment))
                    .max(u32::from(bridge.request().internal.stack_alignment));
                demand.ceiling_bytes = demand
                    .ceiling_bytes
                    .checked_add(u64::from(bridge.frame_bytes()))
                    .and_then(|bytes| bytes.checked_add(u64::from(bridge.call_stack_bias())))
                    .ok_or_else(|| Diagnostic::error("component entry stack demand overflows"))?;
            }
            Ok(demand)
        }
    }

    pub struct ComponentSharedImageEmissionError {
        pub function: crate::ComponentFunctionImage,
        pub diagnostic: Diagnostic,
    }

    pub fn emit_component_shared_image(
        function: crate::ComponentFunctionImage,
    ) -> Result<ComponentSharedImage, Box<ComponentSharedImageEmissionError>> {
        let crate::ComponentFunctionImage {
            body,
            entry_bridge,
            image,
            export_symbol,
            slot_commitment,
        } = function;
        let emitted = emit_shared_function_container(
            image,
            export_symbol,
            slot_commitment,
            crate::component_export_symbol(&slot_commitment).into_bytes(),
        );
        let emitted = emitted.and_then(|image| {
            let validation = validate_component_entry_effects(
                &image,
                &body,
                export_symbol,
                entry_bridge.as_ref(),
            );
            match validation {
                Ok(()) => Ok(image),
                Err(diagnostic) => Err((image.image().clone(), diagnostic)),
            }
        });
        match emitted {
            Ok(image) => Ok(ComponentSharedImage {
                body,
                entry_bridge,
                image,
                slot_commitment,
            }),
            Err((image, diagnostic)) => Err(Box::new(ComponentSharedImageEmissionError {
                function: crate::ComponentFunctionImage {
                    body,
                    entry_bridge,
                    image,
                    export_symbol,
                    slot_commitment,
                },
                diagnostic,
            })),
        }
    }

    pub(crate) fn emit_shared_function_container(
        image: image::FinalImage,
        export_symbol: image::FinalImageSymbolHandle,
        slot_commitment: [u8; 32],
        name: Vec<u8>,
    ) -> Result<ComponentSharedContainer, (image::FinalImage, Diagnostic)> {
        match image.target.object_format {
            target::ObjectFormat::Elf => image_elf::emit_elf_shared_component(
                image,
                vec![image_elf::ElfComponentExport {
                    symbol: export_symbol,
                    name,
                    slot_commitment,
                }],
            )
            .map(ComponentSharedContainer::Elf)
            .map_err(|error| {
                let (image, _, diagnostic) = error.into_parts();
                (image, diagnostic)
            }),
            target::ObjectFormat::MachO => image_macho::emit_macho_shared_component(
                image,
                vec![image_macho::MachoComponentExport {
                    symbol: export_symbol,
                    name,
                    slot_commitment,
                }],
            )
            .map(ComponentSharedContainer::MachO)
            .map_err(|error| {
                let (image, _, diagnostic) = error.into_parts();
                (image, diagnostic)
            }),
            target::ObjectFormat::Coff if image.target == target::NativeTarget::windows_x64() => {
                image_pe::emit_pe_shared_component(
                    image,
                    vec![image_pe::PeComponentExport {
                        symbol: export_symbol,
                        name,
                        slot_commitment,
                    }],
                )
                .map(ComponentSharedContainer::Pe)
                .map_err(|error| {
                    let (image, _, diagnostic) = error.into_parts();
                    (image, diagnostic)
                })
            }
            _ => Err((
                image,
                Diagnostic::error(
                    "shared component requires an admitted ELF, Mach-O, or Windows PE container",
                ),
            )),
        }
    }

    fn validate_component_entry_effects(
        container: &ComponentSharedContainer,
        body: &crate::ObjectArtifact,
        export: image::FinalImageSymbolHandle,
        bridge: Option<&NativeEntryBridge>,
    ) -> Result<(), Diagnostic> {
        let image = container.image();
        let symbol = image.symbol_table.symbols.get(export);
        let region = image
            .executable_regions
            .iter()
            .find(|region| {
                region.section_offset == symbol.offset && region.byte_count == symbol.size
            })
            .ok_or_else(|| {
                Diagnostic::error("component entry has no exact state-evidence region")
            })?;
        let complete = region
            .entry_effects
            .as_ref()
            .ok_or_else(|| Diagnostic::error("component entry lost its complete state evidence"))?;
        if let Some(bridge) = bridge {
            let function = body.entry_function();
            let body_handle = arena::Handle::from_parts(
                function.symbol.arena_index(),
                function.symbol.generation(),
            );
            if image.symbol_table.entry_symbol != body_handle
                || !image.symbol_table.symbols.is_valid(body_handle)
            {
                return Err(Diagnostic::error(
                    "component entry lost its exact body symbol",
                ));
            }
            let body_symbol = image.symbol_table.symbols.get(body_handle);
            if body_symbol.section != image::FinalImageSection::Text
                || body_symbol.kind != object_file::SymbolKind::Function
                || body_symbol.offset != function.text_offset
                || body_symbol.size != function.byte_count
            {
                return Err(Diagnostic::error(
                    "component entry changed its retained body placement",
                ));
            }
            crate::image::entry_effects::validate_body_call_target(
                bridge.request().target.architecture,
                container.final_text_bytes(),
                symbol.offset,
                bridge.call_field_offset(),
                body_symbol.offset,
            )?;
            if complete != bridge.complete_effects() {
                return Err(Diagnostic::error(
                    "component entry state differs from its checked closure",
                ));
            }
            let end = symbol
                .offset
                .checked_add(symbol.size)
                .ok_or_else(|| Diagnostic::error("component entry byte span overflows"))?;
            let bytes = container
                .final_text_bytes()
                .get(symbol.offset..end)
                .ok_or_else(|| Diagnostic::error("component entry byte span is stale"))?;
            let call = Some(
                u32::try_from(bridge.call_field_offset())
                    .map_err(|_| Diagnostic::error("component entry call exceeds u32"))?,
            );
            let replay = match image.target.architecture {
                target::Architecture::X86_64 => {
                    isa_x86_64::decode_x86_64_generated_entry(bytes, call)
                }
                target::Architecture::Aarch64 => {
                    isa_aarch64::decode_aarch64_generated_entry(bytes, call)
                }
            }
            .map_err(Diagnostic::error)?;
            if replay != *bridge.generated()
                || region.footprint.as_ref() != Some(replay.effects.footprint())
            {
                return Err(Diagnostic::error(
                    "component entry instructions or effects fail final target replay",
                ));
            }
        }
        Ok(())
    }
}

pub(crate) use crate::function_fragments;
pub(crate) use crate::object_artifact;
pub(crate) mod private_object_composition {
    use std::sync::Arc;

    use crate::object::function_fragments::refuse_symbol_collision;
    use crate::object::function_fragments::validation::validate_function_fragment_object_artifact;
    use crate::{
        FunctionFragmentObjectArtifactError as Error, ObjectArtifact,
        ObjectCompilerPrivateFunction, PrivateFunctionObject, PrivateObjectSymbolBinding,
    };
    use function_identity::MachineFunctionIdentity;
    use object_file::{
        FunctionSymbolPlan, ObjectPlan, ObjectSymbolHandle, RelocationOrigin, SectionKind,
        SectionPlan, SymbolKind, SymbolPlan, SymbolSection, object_function_reference,
    };

    pub fn compose_private_function_objects(
        source: ObjectArtifact,
        private_objects: &[PrivateFunctionObject],
    ) -> Result<ObjectArtifact, Error> {
        crate::object::object_artifact::installed_stack::validate_installed_stack_requirements(
            &source,
            private_objects,
        )?;
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
                || private.body.profile != linked.profile
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
        refuse_symbol_collision(object, &name, "private function symbol collision")?;
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

    fn append_body(
        linked: &mut ObjectArtifact,
        private: &PrivateFunctionObject,
    ) -> Result<(), Error> {
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
            let linked_symbol = if local == body.object.layout.entry_symbol
                && private.entry_bridge.is_none()
            {
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
            } else if local == body.retained_entry_storage {
                if !linked.retained_entry_storage.is_valid() {
                    linked.retained_entry_storage =
                        crate::object::function_fragments::image_residence::declare_symbol(
                            &mut linked.object,
                        )?;
                }
                linked.retained_entry_storage
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
                    let name = object_file::normalized_import_symbol_name(locator.locator.as_ref());
                    if linked
                        .object
                        .layout
                        .symbols
                        .iter()
                        .any(|(_, symbol)| symbol.name == name)
                    {
                        return Err(Error::Mismatch("private import symbol name collision"));
                    }
                    linked
                        .object
                        .layout
                        .insert_normalized_import(name, locator.locator.as_ref())
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
            let bridge_offset =
                align_section(linked, SectionKind::Text, &format!("{prefix}_entry"))?;
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
                        target::Architecture::Aarch64 => {
                            object_file::RelocationKind::Aarch64Branch26
                        }
                        target::Architecture::X86_64 => {
                            object_file::RelocationKind::X86_64Relative32
                        }
                    },
                });
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
                entry_effects: private.entry_effects.clone(),
                inbound: private.inbound.clone(),
                inbound_ordinals: private.inbound_ordinals.clone(),
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

    pub(crate) struct ObjectNamespace<'artifact> {
        pub artifact: &'artifact ObjectArtifact,
        pub private_function: Option<MachineFunctionIdentity>,
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

    pub(crate) fn namespaces(
        artifact: &ObjectArtifact,
    ) -> impl Iterator<Item = ObjectNamespace<'_>> {
        std::iter::once(ObjectNamespace {
            artifact: artifact.composition_source.as_deref().unwrap_or(artifact),
            private_function: None,
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
                    private_function: Some(private.identity),
                    text_offset: private.text_offset,
                    data_offset: private.data_offset,
                    bindings: Some(&private.symbol_bindings),
                }),
        )
    }

    pub(crate) fn validate_composition(artifact: &ObjectArtifact) -> Result<(), Error> {
        let Some(source) = &artifact.composition_source else {
            return Ok(());
        };
        validate_function_fragment_object_artifact(
            &source
                .fragment_replay
                .as_ref()
                .ok_or(Error::Mismatch("composition root lacks fragment custody"))?
                .0,
            source,
        )?;
        let origins = crate::entries::private_entry_origin::EntryOrigins::new(artifact)?;
        let mut inputs = Vec::with_capacity(artifact.private_functions.len());
        for private in &artifact.private_functions {
            let replay = private
                .body
                .fragment_replay
                .as_ref()
                .ok_or(Error::Mismatch("private object lacks fragment custody"))?;
            validate_function_fragment_object_artifact(&replay.0, &private.body)?;
            origins.verify(private)?;
            crate::entries::callable_entry_transport::validate_entry_transport(private)?;
            inputs.push(PrivateFunctionObject {
                identity: private.identity,
                body: Arc::clone(&private.body),
                entry_bridge: private.entry_bridge.clone(),
                entry_effects: private.entry_effects.clone(),
                inbound: private.inbound.clone(),
                inbound_ordinals: private.inbound_ordinals.clone(),
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
}
