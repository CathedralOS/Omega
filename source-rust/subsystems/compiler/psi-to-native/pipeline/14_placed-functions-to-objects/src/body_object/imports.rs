use super::{Error, ObjectFunctionLocation};
use crate::{ObjectForeignCall, ObjectPublication};
pub fn placed_calls<'a>(
    source: &'a ObjectPublication,
    functions: &[impl AsRef<ObjectFunctionLocation>],
    calls: &'a [ObjectForeignCall],
) -> Result<Vec<crate::PlacedImportCall<'a>>, Error> {
    let import_calls = calls
        .iter()
        .filter_map(|call| call.binding.import_locator().map(|locator| (call, locator)))
        .collect::<Vec<_>>();
    let fields = &source.text().unresolved_normalized_foreign_calls;
    if fields.len() != import_calls.len() {
        return Err(Error::Mismatch("fragment import field roster changed"));
    }
    fields
        .iter()
        .map(|field| {
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
