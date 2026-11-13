/*
Each machine of a compilation's module is given the declaration identity
Terminal Psi requires of a source-produced body, so the module can be the
program a product is built from: the program-entry receipt finds the entry by
it, and native admission compares it with the entry the Build bound.

`wiki/spec/terminal-psi/product.md`: "Source-produced machine bodies retain a
portable checked declaration identity: exact-owner callable and initial
state, contract commitment, and optional specialization template/commitment."
`wiki/spec/terminal-psi/encoding.md` gives the row: the exact-owner machine,
initial state and semantic declaration, the parameter types, the callable and
the result type as identity-table entries, the 32-byte contract commitment,
and the published suspension and blocking Booleans.

Every identity is the one the typed trees fix for the declaration's symbol,
copied into the module's identity table in the text form that table holds,
exactly as a record type's is (`RecordType::identity`). Nothing here or in
the checker compares those texts, and none is formed from a name.

The contract commitment is this route's own. The current checker commits to
the contract as it publishes it in checked trees, which this crate may not
read. Here the commitment is a digest of the contract the lowered form
publishes for the machine: its callable identity, each formal's type and
binding mode, its result type, the `requires` and `ensures` propositions and
crash routes of its `MachineContract`, and what its header says about
termination. Two machines with one commitment publish one contract. A
commitment from this route and one from the current checker are never equal,
so a consumer that compares the two refuses instead of pairing them.

A machine this cannot state is left without a declaration and listed in
`Elaboration::undeclared` with the reason. That changes no verdict: the
module is judged as before, and `AcceptedClosure::publish` refuses a module
that lists one, because it would publish a source-produced body with no
declaration identity.
*/

use std::hash::{Hash, Hasher};

use optimization::digest_encoder::DomainDigestEncoder;
use semantic_vocabulary::{IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_psi::{
    CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalIdentityTableBuilder,
    TerminalMachine,
};
use typed_trees::TypedTrees;

use crate::{Elaboration, Signature};

const COMMITMENT_DOMAIN: &[u8] = b"omega.lowered-machine-public-contract.v1\0";

/// Feeds what `Hash` writes into the commitment digest, as the current
/// checker's `ContractHasher` does for its own commitment.
struct Commitment(DomainDigestEncoder);

impl Hasher for Commitment {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }
}

/// States the declaration identity of every machine of `elaboration` that
/// `machines` gives a typed-tree declaration, in machine order.
pub(crate) fn declare(
    typed: &TypedTrees,
    machines: &IdTable<MachineId, SymbolHandle>,
    signatures: &IdTable<MachineId, Signature>,
    elaboration: &mut Elaboration,
) {
    let sources =
        source::source_identity::toolchain_source_identities(typed.symbols.source_files());
    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
    for machine in &mut elaboration.module.machines {
        // A requirement stands for a contract, not a source-produced body.
        if elaboration.requirements.contains(&machine.id) {
            continue;
        }
        let stated = match (
            &sources,
            machines.get(machine.id),
            signatures.get(machine.id),
        ) {
            (Ok(sources), Some(symbol), Some(signature)) => {
                declaration(typed, sources, *symbol, signature, machine, &mut identities)
            }
            (Err(_), _, _) => Err("a toolchain source has no exact identity"),
            _ => Err("the machine has no typed-tree declaration"),
        };
        match stated {
            Ok(declaration) => machine.declaration = Some(declaration),
            Err(reason) => elaboration.undeclared.push((machine.id, reason)),
        }
    }
    elaboration.module.identities = identities.build();
}

fn declaration(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
    signature: &Signature,
    machine: &TerminalMachine,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<CheckedMachineDeclaration, &'static str> {
    let declared = typed
        .machine_by_symbol(symbol)
        .ok_or("the machine has no typed-tree declaration")?;
    let [state] = typed
        .machine_states
        .span(declared.states)
        .ok_or("the machine's state span does not resolve")?
    else {
        return Err("the machine does not have exactly one state");
    };
    // A machine stage 04 generated for a static application also names its
    // template and the application's commitment, which this route does not
    // state yet.
    if typed.executable_application_template(symbol)?.is_some() {
        return Err("the machine is a generated application of a generic machine");
    }
    let formals = typed
        .state_parameters
        .span(state.parameters)
        .ok_or("the machine's parameter span does not resolve")?;
    let callable = typed
        .machine_callable_identity_text(declared)
        .filter(|callable| !callable.is_empty())
        .ok_or("the machine has no callable identity")?;
    let semantic = match invocation_local(typed, symbol, identities)? {
        Some(identity) => identity,
        None => identities.intern(
            &typed
                .normalized_hermetic_symbol_identity(symbol)
                .map_err(|_| "the machine has no hermetic declaration identity")?,
        ),
    };
    let type_text = |type_reference| {
        typed
            .type_identity_text(typed.package_qualified_type_identity(type_reference))
            .filter(|text| !text.is_empty())
            .ok_or("a signature type has no identity")
    };
    let parameter_types = formals
        .iter()
        .map(|formal| type_text(formal.type_reference))
        .collect::<Result<Vec<_>, _>>()?;
    let result_type = type_text(state.return_type)?;
    let machine_identity = nominal(typed, sources, symbol, identities)?;
    let state_identity = nominal(typed, sources, state.symbol, identities)?;

    let mut commitment = Commitment(DomainDigestEncoder::new(COMMITMENT_DOMAIN));
    callable.hash(&mut commitment);
    for (formal, type_text) in formals.iter().zip(&parameter_types) {
        (
            formal.is_mutable,
            formal.is_const,
            formal.relevance,
            type_text,
        )
            .hash(&mut commitment);
    }
    result_type.hash(&mut commitment);
    machine.contract.requires.hash(&mut commitment);
    for clause in &machine.contract.ensures {
        clause.proposition.hash(&mut commitment);
    }
    machine.contract.crash_routes.hash(&mut commitment);
    signature.termination.hash(&mut commitment);

    Ok(CheckedMachineDeclaration {
        machine: NominalDeclarationIdentity::new(machine_identity),
        state: NominalDeclarationIdentity::new(state_identity),
        semantic_declaration: semantic,
        parameter_types: parameter_types
            .iter()
            .map(|text| identities.intern(text))
            .collect(),
        callable: identities.intern(&callable),
        result_type: identities.intern(&result_type),
        contract: commitment.0.finish(),
        // The reader refuses a machine that declares `suspends` or `blocks`
        // (`read/machine.rs`, `machine_shape`), so every machine here
        // publishes neither.
        published_may_suspend: false,
        published_may_block: false,
        specialization: None,
    })
}

/// The exact-owner identity of `symbol`: its package's, or its toolchain
/// source's. A declaration neither owns has no portable identity.
fn nominal(
    typed: &TypedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<terminal_psi::TerminalIdentityIndex, &'static str> {
    if let Some(identity) = invocation_local(typed, symbol, identities)? {
        return Ok(identity);
    }
    let text = typed
        .exact_owner_nominal_identity(symbol, sources)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|text| !text.is_empty())
        .ok_or("the declaration has no exact package or toolchain owner")?;
    Ok(identities.intern(&text))
}

/// Anonymous source and generated invocation declarations belong to this one
/// activation. A generated call over concrete arguments must not acquire a
/// portable identity from its temporary name. Its exact retained derivation
/// must still end at a captured source token before local identity is minted.
fn invocation_local(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<Option<terminal_psi::TerminalIdentityIndex>, &'static str> {
    if !symbol.is_valid() || !identities.is_invocation_scoped() {
        return Ok(None);
    }
    if !typed.symbols.symbols().nodes().is_valid(symbol) {
        return Err("an invocation declaration is foreign to its retained symbols");
    }
    if typed.symbols.symbol_source_span(symbol).is_none() {
        let mut origin = symbol;
        loop {
            let previous = typed.symbols.get(origin).generated_from;
            if !previous.is_valid() {
                return Err("a generated invocation declaration has no derivation origin");
            }
            if !typed.symbols.symbols().nodes().is_valid(previous) {
                return Err("a generated invocation declaration has a foreign derivation origin");
            }
            if previous.arena_index() >= origin.arena_index() {
                return Err(
                    "a generated invocation declaration has a nonpreceding derivation origin",
                );
            }
            origin = previous;
            let Some(span) = typed.symbols.symbol_source_span(origin) else {
                continue;
            };
            let source = typed
                .symbols
                .source_file(span)
                .ok_or("a generated invocation declaration names a foreign source")?;
            if span.span.start >= span.span.end
                || source.source.get(span.span.start..span.span.end).is_none()
            {
                return Err("a generated invocation declaration has an invalid source token");
            }
            break;
        }
    } else if typed.symbols.symbol_package_identity(symbol).is_some()
        || typed.symbols.symbol_source_origin(symbol) != Some(source::SourceOrigin::User)
    {
        return Ok(None);
    }
    identities
        .intern_invocation_local(&format!(
            "invocation-declaration({}:{})",
            symbol.arena_index(),
            symbol.generation()
        ))
        .map(Some)
}
