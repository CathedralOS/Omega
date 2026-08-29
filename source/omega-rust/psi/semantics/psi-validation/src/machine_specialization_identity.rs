//! Independent checked-carrier replay for generic machine specialization identity.
//!
//! Typed-to-checked lowering constructs the original commitment. Later stages
//! enter here through [`recompute_checked_machine_specialization_commitment`]
//! so they validate retained checked custody without depending back on that
//! earlier pipeline stage.

use psi_checked_trees::CheckedTrees;
use psi_language_semantics::MachineSupplyMode;
use psi_symbols::SymbolHandle;
use sha2::{Digest, Sha256};

/// Recompute one retained specialization commitment from checked custody.
///
/// The returned digest is deliberately representation-neutral. Callers compare
/// it with the commitment retained beside the checked specialization; they do
/// not acquire typed-to-checked production authority through this validator.
pub fn recompute_checked_machine_specialization_commitment(
    checked: &CheckedTrees,
    instance: SymbolHandle,
) -> Result<[u8; 32], &'static str> {
    let mut matches = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == instance);
    let specialization = matches
        .next()
        .ok_or("checked machine has no retained specialization")?;
    if matches.next().is_some() {
        return Err("checked machine has ambiguous retained specializations");
    }

    let template = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.template)
        .ok_or("checked specialization lost its template machine")?;
    match (
        template.supply_mode,
        &specialization.accepted_template_commitment,
    ) {
        (MachineSupplyMode::Accepted, None) => {
            return Err("accepted checked specialization lost its template commitment");
        }
        (MachineSupplyMode::Accepted, Some(commitment)) if template.name.as_str() != commitment => {
            return Err("accepted checked specialization mismatches its template commitment");
        }
        (MachineSupplyMode::Accepted, Some(_)) => {}
        (_, Some(_)) => {
            return Err(
                "checked specialization attached an accepted commitment to a checked template",
            );
        }
        _ => {}
    }

    if specialization.template_contract_report_fingerprint == 0
        || specialization.canonical_template_contract_bytes.is_empty()
        || fnv1a_report_fingerprint(&specialization.canonical_template_contract_bytes)
            != specialization.template_contract_report_fingerprint
    {
        return Err("checked specialization mismatches its canonical template contract");
    }
    let expected_template_commitment =
        machine_template_commitment(&specialization.canonical_template_contract_bytes);
    if specialization.template_contract_commitment.is_zero()
        || specialization.template_contract_commitment.as_bytes() != expected_template_commitment
    {
        return Err("checked specialization mismatches its authoritative template commitment");
    }
    if specialization.normalized_template_identity.is_empty() {
        return Err("checked specialization lost its normalized template identity");
    }

    let concrete = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.instance)
        .ok_or("checked specialization lost its concrete machine")?;
    let concrete_identity = normalized_machine_identity(checked, concrete)
        .ok_or("checked specialization has no normalized concrete identity")?;

    let mut machine_owners = Vec::with_capacity(specialization.machine_arguments.len());
    let mut machine_commitments = Vec::with_capacity(specialization.machine_arguments.len());
    for state_symbol in &specialization.machine_arguments {
        let owner = checked
            .typed
            .machines()
            .iter()
            .find(|machine| {
                checked
                    .typed
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == *state_symbol)
            })
            .ok_or("checked specialization references a machine state without an owner")?;
        let contract = checked
            .facts
            .contract_plans
            .for_machine(owner.symbol)
            .ok_or("checked specialization selected a machine without a contract")?;
        if contract.commitment.is_zero() {
            return Err("checked specialization selected an empty machine contract commitment");
        }
        let owner_identity = normalized_machine_identity(checked, owner)
            .ok_or("checked specialization selected a machine without a normalized identity")?;
        let state_identity = symbol_identity(checked, *state_symbol);
        machine_owners.push(format!("{owner_identity}|selected={state_identity}"));
        machine_commitments.push(contract.commitment.as_bytes());
    }

    let mut conformance_commitments =
        Vec::with_capacity(specialization.conformance_applications.len());
    for application in &specialization.conformance_applications {
        let conformance = checked
            .typed
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == application.declaration)
            .ok_or("checked specialization references a missing conformance")?;
        if checked.typed.closed_conformance_rows(conformance).is_none() {
            return Err("checked specialization selected a conformance that is not closed");
        }
        if application.commitment.is_zero() {
            return Err("checked specialization selected an empty conformance commitment");
        }
        conformance_commitments.push(application.commitment.as_bytes());
    }

    let mut bytes = Vec::new();
    encode_bytes(
        &specialization.canonical_template_contract_bytes,
        &mut bytes,
    );
    bytes.extend(specialization.template_contract_commitment.as_bytes());
    encode_text(&specialization.normalized_template_identity, &mut bytes);
    encode_text(&concrete_identity, &mut bytes);
    encode_texts(&specialization.type_argument_identities, &mut bytes);
    encode_texts(&specialization.const_argument_identities, &mut bytes);
    bytes.extend((machine_owners.len() as u64).to_le_bytes());
    for (owner, commitment) in machine_owners.iter().zip(machine_commitments.iter()) {
        encode_text(owner, &mut bytes);
        bytes.extend(commitment);
    }
    bytes.extend((conformance_commitments.len() as u64).to_le_bytes());
    for commitment in &conformance_commitments {
        bytes.extend(commitment);
    }
    match &specialization.accepted_template_commitment {
        Some(commitment) => {
            bytes.push(1);
            encode_text(commitment, &mut bytes);
        }
        None => bytes.push(0),
    }

    let mut strong = Sha256::new();
    strong.update(b"omega.machine-specialization.v1\0");
    strong.update(bytes);
    Ok(strong.finalize().into())
}

fn symbol_identity(checked: &CheckedTrees, symbol: SymbolHandle) -> String {
    let path = checked.typed.symbols.display_path(symbol, "::");
    let Some(package) = checked.typed.symbols.symbol_package_identity(symbol) else {
        return format!("unmanaged::{path}");
    };
    let mut owner = String::with_capacity(package.digest().len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in package.digest() {
        owner.push(char::from(HEX[usize::from(byte >> 4)]));
        owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    format!("package:{owner}::{path}")
}

fn normalized_machine_identity(
    checked: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
) -> Option<String> {
    let declaration = symbol_identity(checked, machine.symbol);
    let overload = checked
        .typed
        .normalized_machine_overload_identity(machine)?
        .identity();
    Some(format!("{declaration}|{overload}"))
}

fn encode_bytes(value: &[u8], bytes: &mut Vec<u8>) {
    bytes.extend((value.len() as u64).to_le_bytes());
    bytes.extend(value);
}

fn encode_text(value: &str, bytes: &mut Vec<u8>) {
    encode_bytes(value.as_bytes(), bytes);
}

fn encode_texts(values: &[String], bytes: &mut Vec<u8>) {
    bytes.extend((values.len() as u64).to_le_bytes());
    for value in values {
        encode_text(value, bytes);
    }
}

fn fnv1a_report_fingerprint(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    bytes.iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

fn machine_template_commitment(canonical_template_contract_bytes: &[u8]) -> [u8; 32] {
    let mut strong = Sha256::new();
    strong.update(b"omega.machine-template.v1\0");
    strong.update(canonical_template_contract_bytes);
    strong.finalize().into()
}
