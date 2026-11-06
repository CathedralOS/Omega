//! Declare private code once whether a foreign argument or an installation
//! takes its address. Composition supplies the body; ordinary image-address
//! relocations supply its placement. Neither step grants installation authority.

use super::Error;
use crate::{ObjectArtifact, ObjectForeignCall, ObjectFunction};
use function_identity::MachineFunctionIdentity;
use machine_code::PlacedImageAddress;
use object_file::{
    FunctionSymbolPlan, ObjectPlan, RelocationOrigin, RelocationRecord, SymbolKind, SymbolPlan,
    SymbolSection,
};
use selected_instructions::SelectedImageAddressReference;
use terminal_psi::TerminalPsiIdentity;

fn direct_identity(address: &PlacedImageAddress) -> Option<MachineFunctionIdentity> {
    match address.reference {
        SelectedImageAddressReference::PrivateFunction { identity } => Some(identity),
        _ => None,
    }
}

/// Preserve first-reference order, including the established foreign-callback
/// prefix. Sorting an auxiliary roster avoids rescanning all prior references.
pub(super) fn identities(
    calls: &[ObjectForeignCall],
    addresses: &[PlacedImageAddress],
    psi: TerminalPsiIdentity,
) -> Result<Vec<MachineFunctionIdentity>, Error> {
    unique_identities(
        calls
            .iter()
            .flat_map(|call| &call.callback_addresses)
            .map(|row| row.target.callback_function)
            .chain(addresses.iter().filter_map(direct_identity)),
        psi,
    )
}

fn unique_identities(
    identities: impl Iterator<Item = MachineFunctionIdentity>,
    psi: TerminalPsiIdentity,
) -> Result<Vec<MachineFunctionIdentity>, Error> {
    let mut rows = identities
        .enumerate()
        .map(|(position, identity)| {
            if !identity.is_valid() || identity.caller() != psi {
                return Err(Error::Mismatch(
                    "private function reference escaped its exact caller namespace",
                ));
            }
            Ok((identity.to_bytes(), position, identity))
        })
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_unstable_by_key(|(key, position, _)| (*key, *position));
    rows.dedup_by_key(|(key, _, _)| *key);
    rows.sort_unstable_by_key(|(_, position, _)| *position);
    Ok(rows.into_iter().map(|(_, _, identity)| identity).collect())
}

pub(super) fn declare(
    identities: &[MachineFunctionIdentity],
    object: &mut ObjectPlan,
) -> Result<(), Error> {
    if !object.layout.function_symbols.is_empty() {
        return Err(Error::Mismatch(
            "private function symbols were already declared",
        ));
    }
    let mut existing_names = object
        .layout
        .symbols
        .iter()
        .map(|(_, row)| row.name.clone())
        .collect::<Vec<_>>();
    existing_names.sort_unstable();
    for identity in identities {
        let name = identity.private_symbol().ok_or(Error::Mismatch(
            "private reference has no function identity",
        ))?;
        if existing_names.binary_search(&name).is_ok() {
            return Err(Error::Mismatch("private function symbol name collision"));
        }
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name,
            kind: SymbolKind::Function,
            ..Default::default()
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: *identity,
            symbol,
        });
    }
    Ok(())
}

/// One prepared identity/symbol join and one owner/function join per object.
/// Each observation retains the source operation, including repeated address
/// loads of one private function. Those loads are not duplicate definitions.
pub(super) fn records(
    addresses: &[PlacedImageAddress],
    object: &ObjectPlan,
    functions: &[ObjectFunction],
    psi: TerminalPsiIdentity,
) -> Result<Vec<RelocationRecord>, Error> {
    if !addresses.iter().any(|row| direct_identity(row).is_some()) {
        return Ok(Vec::new());
    }
    let mut symbols = object
        .layout
        .function_symbols
        .iter()
        .map(|(_, row)| (row.identity.to_bytes(), row.symbol))
        .collect::<Vec<_>>();
    symbols.sort_unstable_by_key(|(key, _)| *key);
    if symbols.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Mismatch(
            "private function symbol identity is duplicated",
        ));
    }
    let mut owners = functions
        .iter()
        .map(|row| (row.machine, row.symbol))
        .collect::<Vec<_>>();
    owners.sort_unstable_by_key(|(key, _)| *key);
    if owners.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Mismatch(
            "private address has duplicate caller functions",
        ));
    }
    let mut records = Vec::new();
    for address in addresses {
        let Some(identity) = direct_identity(address) else {
            continue;
        };
        if !identity.is_valid() || identity.caller() != psi {
            return Err(Error::Mismatch(
                "private address changed its caller namespace",
            ));
        }
        let target = symbols
            .binary_search_by_key(&identity.to_bytes(), |(key, _)| *key)
            .ok()
            .map(|position| symbols[position].1)
            .ok_or(Error::Mismatch(
                "private address has no declared function symbol",
            ))?;
        let owner = owners
            .binary_search_by_key(&address.machine, |(key, _)| *key)
            .ok()
            .map(|position| owners[position].1)
            .ok_or(Error::Mismatch(
                "private address has no exact observing function",
            ))?;
        if !object.layout.symbols.is_valid(target) || !object.layout.symbols.is_valid(owner) {
            return Err(Error::Mismatch(
                "private address has an invalid object symbol",
            ));
        }
        records.extend(super::image_addresses::records(
            address.fields,
            RelocationOrigin::SemanticOperation {
                function_symbol_handle: owner,
                operation_identity: address.operation.get(),
            },
            target,
            0,
        )?);
    }
    Ok(records)
}

pub(super) fn validate(
    identities: &[MachineFunctionIdentity],
    addresses: &[PlacedImageAddress],
    artifact: &ObjectArtifact,
) -> Result<usize, Error> {
    validate_symbols(identities, &artifact.object, artifact.functions.len())?;
    let expected = records(
        addresses,
        &artifact.object,
        &artifact.functions,
        artifact.psi,
    )?;
    validate_relocation_tail(&artifact.relocations, &expected)?;
    Ok(expected.len())
}

fn validate_symbols(
    identities: &[MachineFunctionIdentity],
    object: &ObjectPlan,
    ordinary_functions: usize,
) -> Result<(), Error> {
    let layout = &object.layout;
    if identities.len() != layout.function_symbols.len() {
        return Err(Error::Mismatch("private function reference roster changed"));
    }
    let mut symbols = layout.symbols.iter().skip(ordinary_functions);
    for (identity, (_, reference)) in identities.iter().zip(layout.function_symbols.iter()) {
        let (handle, symbol) = symbols.next().ok_or(Error::Mismatch(
            "private function reference symbol is absent",
        ))?;
        if reference.identity != *identity
            || reference.symbol != handle
            || identity.private_symbol().as_deref() != Some(symbol.name.as_str())
            || symbol.kind != SymbolKind::Function
            || symbol.section != SymbolSection::None
            || symbol.offset != 0
            || symbol.size != 0
            || !symbol.import_library.is_empty()
        {
            return Err(Error::Mismatch(
                "private function reference differs from its retained identity",
            ));
        }
    }
    Ok(())
}

fn validate_relocation_tail(
    relocations: &object_file::RelocationPlan,
    expected: &[RelocationRecord],
) -> Result<(), Error> {
    // These observations follow the existing import/pool/residence rows.
    // Comparing their exact tail avoids confusing a foreign callback argument
    // with a direct address of the same function at another source operation.
    let start = relocations
        .record_count()
        .checked_sub(expected.len())
        .ok_or(Error::Mismatch("private address relocation is absent"))?;
    if !relocations
        .records()
        .skip(start)
        .map(|(_, row)| row)
        .eq(expected.iter())
    {
        return Err(Error::Mismatch(
            "private address relocations differ from retained observations",
        ));
    }
    Ok(())
}
