//! Prepare family premises in the interface's own resolved telescope.
//!
//! A portable caller must meet every possible member's premises, not those of
//! a lexical prototype. Copying clauses without rebasing their operands would
//! leave them rooted in private member formals. This pass uses positional
//! correspondence and exact symbols; it neither chooses a target nor imports
//! inferred body guarantees. Typing and checking subsequently join the full
//! effect and crash envelope; execution selects a member without reopening
//! a private contract as a portable guarantee.

use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::machine::Machine;
use symbol_resolved_trees::signature::{SignatureContract, SignatureContractKind};
use symbols::SymbolHandle;

pub(crate) fn prepare_premises(program: &mut SymbolResolvedTrees) -> Result<(), Vec<Diagnostic>> {
    let interfaces = program
        .machines
        .iter()
        .enumerate()
        .filter_map(|(position, machine)| {
            (machine.target_family_interface && !machine.target_family_contracts_prepared)
                .then_some(position)
        })
        .collect::<Vec<_>>();
    for position in interfaces {
        let mut interface = program.machines[position].clone();
        let members = program
            .machines
            .iter()
            .filter(|machine| {
                machine.target_family == interface.symbol && machine.authored_target.is_some()
            })
            .cloned()
            .collect::<Vec<_>>();
        if members.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "target-family interface `{}` has no retained implementations",
                interface.name,
            ))]);
        }
        interface.type_parameters = join_property_demands(program, &interface, &members)
            .map_err(|diagnostic| vec![diagnostic])?;
        program.machines[position].type_parameters = interface.type_parameters;
        let mut contracts = Vec::new();
        for member in &members {
            let symbols = telescope_correspondence(program, &interface, member)
                .map_err(|diagnostic| vec![diagnostic])?;
            let mut member_contracts = program.machine_contracts(member).to_vec();
            if let Some(entry) = program.machine_state_handles(member.states).first() {
                member_contracts.extend_from_slice(
                    program.signature_contracts(program.machine_state(*entry).contracts),
                );
            }
            for contract in member_contracts {
                if !matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Crashes { .. }
                ) {
                    continue;
                }
                contracts.push(copy_contract(program, contract, &symbols));
            }
        }
        program.machines[position].contracts = program
            .tables
            .declarations
            .signature_contracts
            .insert_many(contracts);
        program.machines[position].target_family_contracts_prepared = true;
    }
    Ok(())
}

/// Affine/no-carry bounds impose no generic property demand. A portable
/// application must establish every member's non-neutral demand; retaining
/// the first member's weaker bound would check calls the other body cannot
/// accept. The existing property carrier cannot encode conflicting demands.
fn join_property_demands(
    program: &mut SymbolResolvedTrees,
    interface: &Machine,
    members: &[Machine],
) -> Result<HandleSpan<symbol_resolved_trees::data::TypeParameter>, Diagnostic> {
    use language_semantics::Multiplicity;

    let mut parameters = program.machine_type_parameters(interface).to_vec();
    for member in members {
        let member_parameters = program.machine_type_parameters(member);
        if parameters.len() != member_parameters.len() {
            return Err(Diagnostic::error(format!(
                "machine `{}` has an incompatible target-family generic telescope",
                member.name,
            )));
        }
        for (required, actual) in parameters.iter_mut().zip(member_parameters) {
            if actual.bounds.multiplicity != Multiplicity::Affine {
                if required.bounds.multiplicity != Multiplicity::Affine
                    && required.bounds.multiplicity != actual.bounds.multiplicity
                {
                    return Err(Diagnostic::error(format!(
                        "unimplemented: target-family generic multiplicity conjunction for `{}`",
                        interface.name,
                    )));
                }
                required.bounds.multiplicity = actual.bounds.multiplicity;
            }
            if let Some(carry) = actual.bounds.carry {
                if required
                    .bounds
                    .carry
                    .is_some_and(|required| required != carry)
                {
                    return Err(Diagnostic::error(format!(
                        "unimplemented: target-family generic carry conjunction for `{}`",
                        interface.name,
                    )));
                }
                required.bounds.carry = Some(carry);
            }
        }
    }
    Ok(program
        .tables
        .declarations
        .data_type_parameters
        .insert_many(parameters))
}

fn telescope_correspondence(
    program: &SymbolResolvedTrees,
    interface: &Machine,
    member: &Machine,
) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Diagnostic> {
    let interface_entry = program
        .machine_state_handles(interface.states)
        .first()
        .map(|state| program.machine_state(*state));
    let member_entry = program
        .machine_state_handles(member.states)
        .first()
        .map(|state| program.machine_state(*state));
    let (Some(interface_entry), Some(member_entry)) = (interface_entry, member_entry) else {
        return Err(Diagnostic::error(
            "target family has no complete entry telescope",
        ));
    };
    let interface_parameters = program.state_parameters(interface_entry.parameters);
    let member_parameters = program.state_parameters(member_entry.parameters);
    let interface_generics = program.machine_type_parameters(interface);
    let member_generics = program.machine_type_parameters(member);
    if interface_parameters.len() != member_parameters.len()
        || interface_generics.len() != member_generics.len()
        || interface.conformance_bounds.len() != member.conformance_bounds.len()
    {
        return Err(Diagnostic::error(format!(
            "machine `{}` has an incompatible target-family telescope",
            member.name,
        )));
    }
    let mut symbols = member_parameters
        .iter()
        .zip(interface_parameters)
        .map(|(member, interface)| (member.symbol, interface.symbol))
        .chain(
            member_generics
                .iter()
                .zip(interface_generics)
                .map(|(member, interface)| (member.symbol, interface.symbol)),
        )
        .chain(
            member
                .conformance_bounds
                .iter()
                .zip(&interface.conformance_bounds)
                .map(|(member, interface)| (member.binder, interface.binder)),
        )
        .collect::<Vec<_>>();
    symbols.push((member_entry.symbol, interface_entry.symbol));
    // Machine identity is not substituted: a private callable or routed-domain
    // issuer remains the member that authored it, never a newly minted issuer.
    Ok(symbols)
}

fn copy_contract(
    program: &mut SymbolResolvedTrees,
    mut contract: SignatureContract,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> SignatureContract {
    let mut first = arena::Handle::invalid();
    for offset in 0..contract.facts.count() {
        let copied =
            program.copy_proof_fact_with_symbols(contract.facts.handle_at(offset), symbols);
        if !first.is_valid() {
            first = copied;
        }
    }
    // The copier appends one fact per input fact; nested expression/type copies
    // write other arenas. These facts therefore form one contiguous fresh span.
    contract.facts = HandleSpan::from_parts(first, contract.facts.count());
    contract
}
