use super::Error;
use super::ObjectFunctionLocation;
use crate::ObjectForeignCall;
use crate::{
    FunctionSymbolPlan, ObjectPlan, RelocationOrigin, RelocationRecord, SymbolKind, SymbolPlan,
    SymbolSection,
};
use function_identity::MachineFunctionIdentity;
use machine_code::PlacedImageAddress;
use selected_instructions::SelectedImageAddressReference;
use terminal_psi::TerminalPsiIdentity;

fn direct_identity(address: &PlacedImageAddress) -> Option<MachineFunctionIdentity> {
    match address.reference {
        SelectedImageAddressReference::PrivateFunction { identity } => Some(identity),
        _ => None,
    }
}

pub fn identities(
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

pub fn declare(
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
        let name = crate::private_function_symbol_name(*identity).ok_or(Error::Mismatch(
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

pub fn records(
    addresses: &[PlacedImageAddress],
    object: &ObjectPlan,
    functions: &[impl AsRef<ObjectFunctionLocation>],
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
        .map(AsRef::as_ref)
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
