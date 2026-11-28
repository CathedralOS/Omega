use super::{Error, ObjectFunctionLocation};
use crate::{ObjectForeignCall, ObjectPublication};
pub(super) fn placed_calls<'a>(
    source: &'a ObjectPublication,
    functions: &[impl AsRef<ObjectFunctionLocation>],
    calls: &'a [ObjectForeignCall],
) -> Result<Vec<crate::PlacedImportCall<'a>>, Error> {
    let import_calls = calls
        .iter()
        .filter_map(|call| call.binding.import_locator().map(|locator| (call, locator)))
        .collect::<Vec<_>>();
    let fields = &source.object().unresolved_normalized_foreign_calls;
    if fields.len() != import_calls.len() {
        return Err(Error::Mismatch("fragment import field roster changed"));
    }
    fields
        .iter()
        .map(|row| {
            let field = &row.resolution;
            let (call, locator) = call_for_import_field(&import_calls, field)?;
            let function = super::find_object_function(functions, field.caller)
                .ok_or(Error::Mismatch("fragment import caller is absent"))?;
            Ok(crate::PlacedImportCall {
                field,
                caller_symbol: function.symbol,
                locator,
                callbacks: &call.callback_addresses,
            })
        })
        .collect()
}

pub fn call_for_import_field<'a>(
    calls: &[(
        &'a ObjectForeignCall,
        target::NormalizedImportLocatorRef<'a>,
    )],
    field: &machine_code::PlacedNormalizedForeignCallResolution,
) -> Result<
    (
        &'a ObjectForeignCall,
        target::NormalizedImportLocatorRef<'a>,
    ),
    Error,
> {
    let mut matches = calls.iter().copied().filter(|(call, _)| {
        call.machine == field.caller
            && call.owner == target_vocabulary::CallSiteOwner::from(field.invocation_site)
            && u64::try_from(call.text_offset) == Ok(field.field_section_offset)
    });
    match (matches.next(), matches.next()) {
        (Some(call), None) => Ok(call),
        _ => Err(Error::Mismatch(
            "fragment import field lacks unique call custody",
        )),
    }
}

/// Read the import and callback patch fields directly from the relocation
/// table. The encoder emits these before literal, descriptor and private-address
/// relocations. Resolve the expected fields independently from physical calls.
pub fn readback(
    source: &ObjectPublication,
    object: &crate::ObjectPlan,
    relocations: &crate::RelocationPlan,
    functions: &[impl AsRef<ObjectFunctionLocation>],
) -> Result<Vec<ObjectForeignCall>, Error> {
    use super::host;
    use crate::{
        RelocationKind, SectionKind, invocation_relocation_origin, object_function_reference,
        object_symbol_handle_by_import_locator,
    };
    use machine_code::CallbackAddressEncoding;
    let calls = crate::derive_normalized_foreign_call_custody(
        source
            .emission_facts()
            .physical
            .foreign_calls
            .as_ref()
            .map_err(|error| Error::ForeignCalls(error.clone().into()))?,
        source.text(),
    )
    .map_err(Error::ForeignCalls)?;
    let fields = &source.object().unresolved_normalized_foreign_calls;
    let import_calls = calls
        .iter()
        .filter_map(|call| call.binding.import_locator().map(|locator| (call, locator)))
        .collect::<Vec<_>>();
    if import_calls.len() != fields.len() {
        return Err(Error::Mismatch("fragment import field roster changed"));
    }
    let mut records = relocations.records();
    for row in fields {
        let field = &row.resolution;
        let (call, locator) = call_for_import_field(&import_calls, field)?;
        let function = super::find_object_function(functions, field.caller)
            .ok_or(Error::Mismatch("fragment import caller is absent"))?;
        let origin = invocation_relocation_origin(function.symbol, field.invocation_site);
        for callback in &call.callback_addresses {
            let Some((callback_symbol, _)) =
                object_function_reference(object, callback.target.callback_function)
            else {
                return Err(Error::Mismatch(
                    "fragment import callback private function is missing",
                ));
            };
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
        let symbol = object_symbol_handle_by_import_locator(object, locator);
        let Some((_, relocation)) = records.next() else {
            return Err(Error::Mismatch("fragment import relocation roster changed"));
        };
        if !object.layout.symbols.is_valid(symbol)
            || relocation.origin != origin
            || relocation.section != SectionKind::Text
            || relocation.offset != host(field.field_section_offset)?
            || relocation.byte_width != usize::from(field.field_byte_width)
            || relocation.addend != field.addend
            || relocation.kind != RelocationKind::from(field.kind)
            || relocation.symbol_handle != symbol
        {
            return Err(Error::Mismatch(
                "fragment import relocation differs from source call",
            ));
        }
    }
    Ok(calls)
}
