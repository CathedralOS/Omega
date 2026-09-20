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
use machine_code::NormalizedForeignCallResolutionKind;
use object_file::{
    NormalizedImportPlan, ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, StagedOptimizedRelocationFreeObjectContainer, SymbolKind,
    SymbolPlan, SymbolSection, normalized_foreign_import_symbol_name,
    object_symbol_handle_by_foreign_locator,
};

pub(super) fn publish(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    object: &mut ObjectPlan,
    functions: &[ObjectFunction],
    calls: &[ObjectForeignCall],
) -> Result<RelocationPlan, Error> {
    let fields = &source
        .source()
        .text_section()
        .unresolved_normalized_foreign_calls;
    let mut relocations = RelocationPlan::with_record_capacity(object.target, fields.len());
    for (field, call) in fields.iter().zip(calls) {
        let symbol = if let Some(import) = object
            .layout
            .normalized_imports
            .iter()
            .find(|import| import.locator == call.locator)
        {
            import.symbol
        } else {
            let name = normalized_foreign_import_symbol_name(&call.locator);
            if object
                .layout
                .symbols
                .iter()
                .any(|(_, symbol)| symbol.name == name)
            {
                return Err(Error::Mismatch("fragment import symbol name collision"));
            }
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name,
                section: SymbolSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
                import_library: String::new(),
            });
            object.layout.normalized_imports.push(NormalizedImportPlan {
                symbol,
                locator: call.locator.clone(),
            });
            symbol
        };
        let function = functions
            .iter()
            .find(|function| function.machine == field.caller)
            .ok_or(Error::Mismatch("fragment import caller is absent"))?;
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::SemanticOperation {
                function_symbol_handle: function.symbol,
                operation_identity: field.operation.get(),
            },
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
    let calls = derive_normalized_foreign_call_custody(source)
        .map_err(|_| Error::Mismatch("fragment import lacks selected call custody"))?;
    let fields = &source
        .source()
        .text_section()
        .unresolved_normalized_foreign_calls;
    let layout = &artifact.object.layout;
    if artifact.foreign_calls != calls || artifact.relocations.record_count() != fields.len() {
        return Err(Error::Mismatch("fragment import relocation roster changed"));
    }
    let mut imports = layout.normalized_imports.iter();
    // Import symbols trail the program functions and the compiler-private
    // callback functions; skip both rosters to find the import tail.
    let mut symbols = layout
        .symbols
        .iter()
        .skip(artifact.functions.len() + artifact.private_functions.len());
    for (call_index, call) in calls.iter().enumerate() {
        if calls[..call_index]
            .iter()
            .any(|prior| prior.locator == call.locator)
        {
            continue;
        }
        let (Some(import), Some((symbol_handle, symbol))) = (imports.next(), symbols.next()) else {
            return Err(Error::Mismatch("fragment import is missing"));
        };
        if import.locator != call.locator
            || import.symbol != symbol_handle
            || symbol.name != normalized_foreign_import_symbol_name(&call.locator)
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
    for ((field, call), (_, relocation)) in fields
        .iter()
        .zip(&calls)
        .zip(artifact.relocations.records())
    {
        let function = artifact
            .functions
            .iter()
            .find(|function| function.machine == field.caller)
            .ok_or(Error::Mismatch("fragment import caller is absent"))?;
        let symbol = object_symbol_handle_by_foreign_locator(&artifact.object, &call.locator);
        if !layout.symbols.is_valid(symbol)
            || relocation.origin
                != (RelocationOrigin::SemanticOperation {
                    function_symbol_handle: function.symbol,
                    operation_identity: field.operation.get(),
                })
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

fn relocation_kind(kind: NormalizedForeignCallResolutionKind) -> RelocationKind {
    match kind {
        NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => RelocationKind::X86_64Relative32,
        NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => RelocationKind::Aarch64Branch26,
    }
}
