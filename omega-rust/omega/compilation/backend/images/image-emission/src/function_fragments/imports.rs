//! Bind already validated fragment call fields to the ordinary image writer's
//! import symbols. Internal calls are resolved during text placement; external
//! calls must remain relocations until the target image writer binds its import
//! stubs. Publishing their zero placeholders without these rows produces a
//! well-formed but nonfunctional image.
//!
//! Full normalized locators determine deduplication, not boundary ordinals (which
//! are local to a selected function) or diagnostic symbol fingerprints. Replay
//! independently joins each relocation to the retained source call and rejects
//! missing, extra, substituted, or reordered imports and call fields.

use super::{Error, host};
use crate::{
    ObjectArtifact, ObjectForeignCall, ObjectFunction, derive_normalized_foreign_call_custody,
};

pub(super) fn declare_callbacks(
    calls: &[ObjectForeignCall],
    object: &mut ObjectPlan,
    psi: terminal_psi::TerminalPsiIdentity,
) -> Result<(), Error> {
    for callback in calls.iter().flat_map(|call| &call.callback_addresses) {
        let identity = callback.target.callback_function;
        if identity.caller() != psi {
            return Err(Error::Mismatch(
                "callback reference escaped its caller namespace",
            ));
        }
        if object_function_reference(object, identity).is_some() {
            continue;
        }
        let name = identity.private_symbol().ok_or(Error::Mismatch(
            "callback reference has no caller-scoped function identity",
        ))?;
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name,
            kind: SymbolKind::Function,
            ..SymbolPlan::default()
        });
        object
            .layout
            .function_symbols
            .insert(object_file::FunctionSymbolPlan { identity, symbol });
    }
    Ok(())
}
use machine_code::{CallbackAddressEncoding, NormalizedForeignCallResolutionKind};
use object_file::{
    ObjectPlan, RelocationKind, RelocationPlan, RelocationRecord, SectionKind,
    StagedOptimizedRelocationFreeObjectContainer, SymbolKind, SymbolPlan, SymbolSection,
    boundary_relocation_origin, normalized_foreign_import_symbol_name, object_function_reference,
    object_symbol_handle_by_foreign_locator,
};

pub(super) fn publish(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    object: &mut ObjectPlan,
    functions: &[ObjectFunction],
    calls: &[ObjectForeignCall],
) -> Result<RelocationPlan, Error> {
    let import_calls = calls
        .iter()
        .filter_map(|call| call.binding.import().map(|(locator, _)| (call, locator)))
        .collect::<Vec<_>>();
    let fields = &source
        .source()
        .text_section()
        .unresolved_normalized_foreign_calls;
    if fields.len() != import_calls.len() {
        return Err(Error::Mismatch("fragment import field roster changed"));
    }
    let callback_record_count = calls
        .iter()
        .flat_map(|call| call.callback_addresses.iter())
        .map(|callback| match callback.encoding {
            CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
            CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let mut relocations =
        RelocationPlan::with_record_capacity(object.target, fields.len() + callback_record_count);
    for field in fields {
        let (call, locator) = call_for_import_field(&import_calls, field)?;
        let function = crate::object_artifact::find_object_function(functions, field.caller)
            .ok_or(Error::Mismatch("fragment import caller is absent"))?;
        let origin = boundary_relocation_origin(function.symbol, field.invocation_site);
        // A callback argument's address is a compiler-private relocation: it
        // binds the private thunk's symbol, not the import tail.
        for callback in &call.callback_addresses {
            let Some((callback_symbol, _)) =
                object_function_reference(object, callback.target.callback_function)
            else {
                return Err(Error::Mismatch(
                    "fragment import lacks callback private function",
                ));
            };
            let mut push_callback_relocation =
                |relocation_offset: usize, kind: RelocationKind| -> Result<(), Error> {
                    let offset = function
                        .text_offset
                        .checked_add(relocation_offset)
                        .ok_or(Error::Overflow)?;
                    relocations.push_record(RelocationRecord {
                        origin,
                        section: SectionKind::Text,
                        offset,
                        byte_width: 4,
                        symbol_handle: callback_symbol,
                        addend: 0,
                        kind,
                    });
                    Ok(())
                };
            match callback.encoding {
                CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                    push_callback_relocation(relocation_offset, RelocationKind::X86_64Relative32)?
                }
                CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    push_callback_relocation(
                        page_relocation_offset,
                        RelocationKind::Aarch64Page21,
                    )?;
                    push_callback_relocation(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        let symbol = if let Some(import) = object
            .layout
            .normalized_imports
            .iter()
            .find(|import| import.locator == *locator)
        {
            import.symbol
        } else {
            let name = normalized_foreign_import_symbol_name(locator);
            if object
                .layout
                .symbols
                .iter()
                .any(|(_, symbol)| symbol.name == name)
            {
                return Err(Error::Mismatch("fragment import symbol name collision"));
            }
            object.layout.insert_import(name, locator)
        };
        relocations.push_record(RelocationRecord {
            origin,
            section: SectionKind::Text,
            offset: host(field.field_section_offset)?,
            byte_width: usize::from(field.field_byte_width),
            symbol_handle: symbol,
            addend: field.addend,
            kind: relocation_kind(field.kind),
        });
    }
    Ok(relocations)
}

pub(super) fn validate(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    artifact: &ObjectArtifact,
) -> Result<(), Error> {
    let calls = derive_normalized_foreign_call_custody(source).map_err(Error::ForeignCalls)?;
    let import_calls = calls
        .iter()
        .filter_map(|call| call.binding.import().map(|(locator, _)| (call, locator)))
        .collect::<Vec<_>>();
    let fields = &source
        .source()
        .text_section()
        .unresolved_normalized_foreign_calls;
    let layout = &artifact.object.layout;
    let expected_records = fields.len()
        + calls
            .iter()
            .flat_map(|call| call.callback_addresses.iter())
            .map(|callback| match callback.encoding {
                CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
                CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
            })
            .sum::<usize>();
    // Byte-literal records trail the import rows; their own replay owns them.
    let expected_records = expected_records
        + super::byte_literals::relocation_count(artifact.byte_literal_pool.as_ref())
        + super::implementation_tables::relocation_count(
            artifact.implementation_table_pool.as_ref(),
        );
    if artifact.foreign_calls != calls
        || artifact.relocations.record_count() != expected_records
        || fields.len() != import_calls.len()
    {
        return Err(Error::Mismatch("fragment import relocation roster changed"));
    }
    let mut imports = layout.normalized_imports.iter();
    // Import symbols trail the program functions, the compiler-private
    // callback functions and the byte-literal pool; skip those rosters to
    // find the import tail.
    let mut symbols = layout.symbols.iter().skip(
        artifact.functions.len()
            + layout.function_symbols.len()
            + usize::from(artifact.byte_literal_pool.is_some())
            + artifact
                .implementation_table_pool
                .as_ref()
                .map_or(0, |pool| pool.tables.len()),
    );
    let mut prior_locators = Vec::new();
    for field in fields {
        let (_, locator) = call_for_import_field(&import_calls, field)?;
        if prior_locators.contains(&locator) {
            continue;
        }
        prior_locators.push(locator);
        let (Some(import), Some((symbol_handle, symbol))) = (imports.next(), symbols.next()) else {
            return Err(Error::Mismatch("fragment import is missing"));
        };
        if import.locator != *locator
            || import.symbol != symbol_handle
            || symbol.name != normalized_foreign_import_symbol_name(locator)
            || symbol.kind != SymbolKind::Import
            || symbol.section != SymbolSection::None
            || symbol.offset != 0
            || symbol.size != 0
            || !symbol.import_library.is_empty()
            || layout
                .symbols
                .iter()
                .filter(|(_, other)| other.name == symbol.name)
                .count()
                != 1
        {
            return Err(Error::Mismatch(
                "fragment import differs from selected locator",
            ));
        }
    }
    if imports.next().is_some() || symbols.next().is_some() {
        return Err(Error::Mismatch("fragment import roster has extra rows"));
    }
    let mut records = artifact.relocations.records();
    for field in fields {
        let (call, locator) = call_for_import_field(&import_calls, field)?;
        let function = artifact
            .function_on_machine(field.caller)
            .ok_or(Error::Mismatch("fragment import caller is absent"))?;
        let origin = boundary_relocation_origin(function.symbol, field.invocation_site);
        for callback in &call.callback_addresses {
            let Some((callback_symbol, callback_plan)) =
                object_function_reference(&artifact.object, callback.target.callback_function)
            else {
                return Err(Error::Mismatch(
                    "fragment import callback private function is missing",
                ));
            };
            let mut carriers = artifact
                .private_functions
                .iter()
                .filter(|private| private.identity == callback.target.callback_function);
            match (carriers.next(), carriers.next()) {
                (Some(private), None)
                    if private.function.symbol == callback_symbol
                        && private.function.text_offset == callback_plan.offset
                        && private.function.byte_count == callback_plan.size => {}
                (None, None)
                    if callback_plan.section == SymbolSection::None
                        && callback_plan.kind == SymbolKind::Function
                        && callback_plan.offset == 0
                        && callback_plan.size == 0
                        && callback
                            .target
                            .callback_function
                            .private_symbol()
                            .as_deref()
                            == Some(callback_plan.name.as_str()) => {}
                _ => {
                    return Err(Error::Mismatch(
                        "fragment callback definition or reference differs",
                    ));
                }
            }
            let mut replay_callback_relocation =
                |relocation_offset: usize, kind: RelocationKind| -> Result<(), Error> {
                    let offset = function
                        .text_offset
                        .checked_add(relocation_offset)
                        .ok_or(Error::Overflow)?;
                    let Some((_, relocation)) = records.next() else {
                        return Err(Error::Mismatch("fragment import relocation roster changed"));
                    };
                    if relocation.origin != origin
                        || relocation.section != SectionKind::Text
                        || relocation.offset != offset
                        || relocation.byte_width != 4
                        || relocation.symbol_handle != callback_symbol
                        || relocation.addend != 0
                        || relocation.kind != kind
                    {
                        return Err(Error::Mismatch(
                            "fragment import callback relocation differs",
                        ));
                    }
                    Ok(())
                };
            match callback.encoding {
                CallbackAddressEncoding::X86_64Relative32 { relocation_offset } => {
                    replay_callback_relocation(relocation_offset, RelocationKind::X86_64Relative32)?
                }
                CallbackAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    replay_callback_relocation(
                        page_relocation_offset,
                        RelocationKind::Aarch64Page21,
                    )?;
                    replay_callback_relocation(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        let symbol = object_symbol_handle_by_foreign_locator(&artifact.object, locator);
        let Some((_, relocation)) = records.next() else {
            return Err(Error::Mismatch("fragment import relocation roster changed"));
        };
        if !layout.symbols.is_valid(symbol)
            || relocation.origin != origin
            || relocation.section != SectionKind::Text
            || relocation.offset != host(field.field_section_offset)?
            || relocation.byte_width != usize::from(field.field_byte_width)
            || relocation.addend != field.addend
            || relocation.kind != relocation_kind(field.kind)
            || relocation.symbol_handle != symbol
        {
            return Err(Error::Mismatch(
                "fragment import relocation differs from source call",
            ));
        }
    }
    Ok(())
}

/// Selected roster order need not equal placed instruction order. Join by
/// semantic owner and exact field offset, never by a positional zip.
fn call_for_import_field<'a>(
    calls: &[(&'a ObjectForeignCall, &'a target::NormalizedForeignLocator)],
    field: &machine_code::PlacedNormalizedForeignCallResolution,
) -> Result<(&'a ObjectForeignCall, &'a target::NormalizedForeignLocator), Error> {
    let mut matches = calls.iter().copied().filter(|(call, _)| {
        call.machine == field.caller
            && call.owner == target_operations::CallSiteOwner::from(field.invocation_site)
            && u64::try_from(call.text_offset) == Ok(field.field_section_offset)
    });
    match (matches.next(), matches.next()) {
        (Some(call), None) => Ok(call),
        _ => Err(Error::Mismatch(
            "fragment import field lacks unique call custody",
        )),
    }
}

fn relocation_kind(kind: NormalizedForeignCallResolutionKind) -> RelocationKind {
    match kind {
        NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => RelocationKind::X86_64Relative32,
        NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => RelocationKind::Aarch64Branch26,
    }
}
