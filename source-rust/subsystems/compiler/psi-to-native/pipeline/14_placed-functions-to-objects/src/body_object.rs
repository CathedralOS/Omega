//! Sections, symbols and relocations for one placed body. All offsets in this
//! artifact are object-relative; image composition rebases them as a unit.
use crate::{
    ForeignCallPlacementError, ObjectForeignCall, ObjectPlan, ObjectPublication,
    ObjectSymbolHandle, RelocationPlan, SectionKind, SectionPlan, SymbolKind, SymbolPlan,
    SymbolSection, entry_symbol_name,
};
use semantic_vocabulary::MachineId;
pub mod byte_literals;
pub mod code_descriptors;
mod image_addresses;
pub mod image_residence;
pub mod imports;
pub mod native_code_layouts;
pub mod private_addresses;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectByteLiteralPool {
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
    pub addresses: Vec<ObjectByteLiteralAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectByteLiteralAddress {
    pub machine: MachineId,
    pub literal: machine_code::ByteLiteralReference,
    pub fields: machine_code::ImageAddressFields,
    pub pool_offset: usize,
    pub byte_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectFunctionLocation {
    pub machine: MachineId,
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
}
impl AsRef<Self> for ObjectFunctionLocation {
    fn as_ref(&self) -> &Self {
        self
    }
}
fn find_object_function(
    functions: &[impl AsRef<ObjectFunctionLocation>],
    machine: MachineId,
) -> Option<&ObjectFunctionLocation> {
    functions
        .iter()
        .map(AsRef::as_ref)
        .find(|row| row.machine == machine)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Mismatch(&'static str),
    Unsupported(&'static str),
    ForeignCalls(ForeignCallPlacementError),
    NativeCodeLayout(machine_code::code_layout::NativeCodeLayoutError),
    Overflow,
}
fn host(value: u64) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| Error::Overflow)
}

pub(crate) fn refuse_symbol_collision(
    object: &crate::ObjectPlan,
    name: &str,
    reason: &'static str,
) -> Result<(), Error> {
    if object.layout.symbols.iter().any(|(_, s)| s.name == name) {
        return Err(Error::Mismatch(reason));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionObjectSections {
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
    pub text_bytes: Vec<u8>,
    pub data_bytes: Vec<u8>,
    pub functions: Vec<ObjectFunctionLocation>,
    pub foreign_calls: Vec<ObjectForeignCall>,
    pub retained_entry_storage: ObjectSymbolHandle,
    pub byte_literal_pool: Option<ObjectByteLiteralPool>,
    pub code_descriptor_pool: Option<code_descriptors::ObjectCodeDescriptorPool>,
    pub native_code_layout_pool: Option<native_code_layouts::NativeCodeLayoutPool>,
}
impl FunctionObjectSections {
    pub(crate) fn assemble(source: &ObjectPublication) -> Result<Self, Error> {
        let text = source.text();
        let mut object = ObjectPlan::with_capacity(text.target, 1, text.functions.len());
        let mut text_bytes = text.bytes.clone();
        let mut functions = Vec::with_capacity(text.functions.len());
        for placed in &text.functions {
            let offset = host(placed.section_offset)?;
            let length = host(placed.byte_count)?;
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: if placed.machine == text.semantic_entry {
                    entry_symbol_name(text.target)
                } else {
                    format!("omega_terminal_machine_{}", placed.machine.get())
                },
                section: SymbolSection::Section(SectionKind::Text),
                offset,
                size: length,
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            if placed.machine == text.semantic_entry {
                object.layout.entry_symbol = symbol;
            }
            functions.push(ObjectFunctionLocation {
                machine: placed.machine,
                symbol,
                text_offset: offset,
                byte_count: length,
            });
        }
        object.layout.function_unwinds = function_unwinds(source, &functions)?;
        let profile = source.emission_facts().physical.contracts.profile();
        let catalogs = source
            .emission_facts()
            .physical
            .contracts
            .publication()
            .code_catalogs();
        let foreign_calls = crate::derive_normalized_foreign_call_custody(
            source
                .emission_facts()
                .physical
                .foreign_calls
                .as_ref()
                .map_err(|error| Error::ForeignCalls(error.clone().into()))?,
            source.text(),
        )
        .map_err(Error::ForeignCalls)?;
        let private_identities = private_addresses::identities(
            &foreign_calls,
            &text.unresolved_image_addresses,
            text.psi,
        )?;
        private_addresses::declare(&private_identities, &mut object)?;
        let byte_literal_pool = byte_literals::place(source, &mut object, &mut text_bytes)?;
        let (code_descriptor_pool, mut data_bytes) = code_descriptors::place(
            catalogs,
            &source.text().unresolved_image_addresses,
            &mut object,
        )?;
        let native_code_layout_pool = native_code_layouts::place(
            source
                .native_code_layouts()
                .map_err(Error::NativeCodeLayout)?
                .as_ref(),
            &source.text().unresolved_image_addresses,
            &mut object,
            &mut data_bytes,
        )?;
        let retained_entry_storage = image_residence::declare(
            &source.text().unresolved_image_addresses,
            &mut object,
            profile,
        )?;
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Text,
            size: text_bytes.len(),
            alignment: host(text.section_alignment)?,
        });
        let calls = imports::placed_calls(source, &functions, &foreign_calls)?;
        let (object, mut relocations) =
            object
                .with_import_relocations(&calls)
                .map_err(|error| match error {
                    crate::ImportRelocationError::Mismatch(reason) => Error::Mismatch(reason),
                    crate::ImportRelocationError::Overflow => Error::Overflow,
                })?;
        byte_literals::publish_relocations(
            byte_literal_pool.as_ref(),
            &functions,
            &mut relocations,
        )?;
        code_descriptors::publish_relocations(
            code_descriptor_pool.as_ref(),
            &functions,
            &mut relocations,
        )?;
        native_code_layouts::publish_relocations(
            native_code_layout_pool.as_ref(),
            &functions,
            &mut relocations,
        )?;
        image_residence::publish_relocations(
            &source.text().unresolved_image_addresses,
            retained_entry_storage,
            &functions,
            &mut relocations,
        )?;
        for record in private_addresses::records(
            &text.unresolved_image_addresses,
            &object,
            &functions,
            text.psi,
        )? {
            relocations.push_record(record);
        }
        let bindings = source.bindings();
        object.check_bindings(bindings).map_err(Error::Mismatch)?;
        check_windows_internet_companion(
            object
                .layout
                .normalized_imports
                .iter()
                .map(|symbol| symbol.locator.as_ref()),
            bindings.windows_internet.is_some(),
        )?;
        Ok(Self {
            object,
            relocations,
            text_bytes,
            data_bytes,
            functions,
            foreign_calls,
            retained_entry_storage,
            byte_literal_pool,
            code_descriptor_pool,
            native_code_layout_pool,
        })
    }
}

fn check_windows_internet_companion<'a>(
    imports: impl IntoIterator<Item = target::NormalizedImportLocatorRef<'a>>,
    supplied: bool,
) -> Result<(), Error> {
    if runtime_abi::needs_windows_internet_companion(imports) == supplied {
        Ok(())
    } else {
        Err(Error::Mismatch(
            "Windows helper code differs from the object's import symbols",
        ))
    }
}

#[cfg(test)]
#[path = "../../../tests/fixtures/windows-internet-provider/import_rosters.rs"]
mod windows_import_rosters;

#[test]
fn checks_windows_companion_against_the_objects_imports() {
    windows_import_rosters::check(|label, imports, expected| {
        assert!(
            check_windows_internet_companion(imports.iter().copied(), expected).is_ok(),
            "{label}"
        );
        assert!(
            check_windows_internet_companion(imports.iter().copied(), !expected).is_err(),
            "{label}"
        );
    });
}

/// Carry each framed function's prologue and epilogue steps, as encoded in
/// stage 11 and positioned by frame application in stage 13, onto its symbol.
/// A frameless function gets no row: it moves neither the stack pointer nor a
/// preserved register, so an unwinder needs nothing to undo.
fn function_unwinds(
    source: &ObjectPublication,
    functions: &[ObjectFunctionLocation],
) -> Result<Vec<crate::FunctionUnwindPlan>, Error> {
    let code = source.body().code();
    let protocol = &code.protocol;
    let application = &code.frame_application;
    let mut unwinds = Vec::new();
    for function in functions {
        let mut rows = protocol
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let (Some(row), None) = (rows.next(), rows.next()) else {
            return Err(Error::Mismatch(
                "placed function has no single frame protocol row",
            ));
        };
        let steps = |span: machine_code::FrameProtocolStepSpan| {
            span.steps(&protocol.steps)
                .map(<[_]>::to_vec)
                .ok_or(Error::Mismatch("frame protocol step span is out of range"))
        };
        let mut frame = calling_conventions::frame_unwind::FunctionFrameUnwind {
            prologue: steps(row.prologue_steps)?,
            epilogue: steps(row.epilogue_steps)?,
            epilogue_offsets: Vec::new(),
        };
        if frame.is_frameless() {
            continue;
        }
        let applied = application
            .functions
            .iter()
            .find(|applied| applied.machine == function.machine)
            .ok_or(Error::Mismatch("framed function has no frame application"))?;
        let byte_count = u64::try_from(function.byte_count).map_err(|_| Error::Overflow)?;
        if applied.prologue_function_offset != 0
            || applied.prologue_byte_count != u64::from(frame.prologue_byte_count())
            || applied.prologue_byte_count > byte_count
        {
            return Err(Error::Mismatch(
                "frame application moved the prologue away from the function start",
            ));
        }
        frame.epilogue_offsets.reserve(applied.epilogues.len());
        for epilogue in &applied.epilogues {
            let end = epilogue
                .function_offset
                .checked_add(epilogue.byte_count)
                .ok_or(Error::Overflow)?;
            if epilogue.byte_count != u64::from(frame.epilogue_byte_count()) || end > byte_count {
                return Err(Error::Mismatch(
                    "frame application placed an epilogue outside its function",
                ));
            }
            frame
                .epilogue_offsets
                .push(u32::try_from(epilogue.function_offset).map_err(|_| Error::Overflow)?);
        }
        frame.epilogue_offsets.sort_unstable();
        unwinds.push(crate::FunctionUnwindPlan {
            symbol: function.symbol,
            frame,
        });
    }
    Ok(unwinds)
}
