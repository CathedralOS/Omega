//! Match an authored Build preparation to its checked call in the entry body.

use checked_program_to_psi::AcceptedTerminalSource;
use semantic_vocabulary::InvocationSite;
use semantic_vocabulary::{MachineId, OperationId};
use source_text::Span;
use terminal_psi::TerminalModule;
use terminal_semantics::continuation::{CheckedPrefix, PrefixCall};

/// Find the selected wait in its own admitted source declaration. A helper
/// can reside in a different source file or acquire a different machine ID.
pub fn locate_build_suspending_call(
    accepted: &AcceptedTerminalSource,
    owner: &source_admission::RetainedDeclaration,
    call: Span,
) -> Result<(MachineId, OperationId), &'static str> {
    let symbols = &accepted.source().symbols;
    let mut found = None;
    for machine in &accepted.module().machines {
        let Ok(symbol) = source_machine(accepted, machine.id) else {
            continue;
        };
        if symbols.get(symbol).kind != owner.kind()
            || !symbols
                .symbol_source_span(symbol)
                .is_some_and(|s| s.span == owner.span())
            || crate::source_symbols::rejoin_declaration(owner, symbols, [symbol]).is_err()
        {
            continue;
        }
        if let Ok(id) = preparation_in_machine(accepted, machine.id, call)
            && found.replace((machine.id, id)).is_some()
        {
            return Err("the retained preparation occurs in multiple checked machines");
        }
    }
    found.ok_or("the retained Build declaration has no matching checked preparation")
}

fn preparation_in_machine(
    accepted: &AcceptedTerminalSource,
    entry: MachineId,
    span: Span,
) -> Result<OperationId, &'static str> {
    let lowered = accepted.closure().elaboration();
    let origins = lowered
        .sources
        .get(entry)
        .ok_or("Build entry lacks source coordinates")?;
    let owner = lowered
        .machines
        .get(entry)
        .ok_or("Build entry lacks its source declaration")?;
    let file = accepted
        .source()
        .symbols
        .symbol_provenance_source_span(*owner)
        .ok_or("Build entry lacks its authored source file")?
        .source_id;
    let machine = accepted
        .module()
        .machine(entry)
        .ok_or("Build entry is missing")?;
    let mut matched = accepted.emitted_source_calls().iter().filter_map(|call| {
        let InvocationSite::Operation(id) = call.site else {
            return None;
        };
        if call.machine != entry || call.owner != *owner || call.source_member != entry {
            return None;
        }
        let op = machine.operation(id)?;
        if op.suspension_crossing.is_none() || op.declared_invocation().is_none() {
            return None;
        }
        let origin = origins.operations.get(id)?.span()?;
        (origin.source_id == file && origin.span.start <= span.start && span.end <= origin.span.end)
            .then_some(id)
    });
    let operation = matched
        .next()
        .ok_or("no emitted Build preparation matches the authored call")?;
    if matched.next().is_some() {
        return Err("the authored Build preparation has multiple emitted calls");
    }
    Ok(operation)
}

/// Bind this comparison to the exact two source judgments that produced the
/// admitted programs. A retained declaration includes the captured source bytes,
/// package and dependency scope as well as its occurrence in the file.
pub fn checked_build_stack<'a>(
    old: &AcceptedTerminalSource,
    old_module: &'a TerminalModule,
    stack: &[PrefixCall],
    new: &AcceptedTerminalSource,
    new_module: &'a TerminalModule,
    new_call: (MachineId, OperationId),
) -> Result<CheckedPrefix<'a>, &'static str> {
    admitted_modules(old, old_module, new, new_module)?;
    bind_source(
        CheckedPrefix::check_stack(old_module, stack, new_module, new_call)?,
        old,
        new,
    )
}

fn admitted_modules(
    old: &AcceptedTerminalSource,
    old_module: &TerminalModule,
    new: &AcceptedTerminalSource,
    new_module: &TerminalModule,
) -> Result<(), &'static str> {
    if old.is_invocation()
        && new.is_invocation()
        && old.module() == old_module
        && new.module() == new_module
    {
        Ok(())
    } else {
        Err("Build continuation requires the admitted modules of both source judgments")
    }
}

fn source_machine(
    source: &AcceptedTerminalSource,
    id: MachineId,
) -> Result<symbols::SymbolHandle, &'static str> {
    let symbols = &source.source().symbols;
    let owner = *source
        .closure()
        .elaboration()
        .machines
        .get(id)
        .ok_or("missing reached machine source")?;
    let mut candidate = symbols.authored_origin(owner);
    while candidate.is_valid() {
        if symbols.symbol_source_span(candidate).is_some() {
            return Ok(candidate);
        }
        candidate = symbols.get(candidate).parent;
    }
    Err("reached machine has no authored declaration")
}

fn bind_source<'a>(
    certificate: CheckedPrefix<'a>,
    old: &AcceptedTerminalSource,
    new: &AcceptedTerminalSource,
) -> Result<CheckedPrefix<'a>, &'static str> {
    let mut rejoined_files = Vec::new();
    for &(a, b) in certificate.machine_pairs() {
        let previous = source_machine(old, a)?;
        let next = source_machine(new, b)?;
        let retained = crate::source_symbols::retain_declaration(&old.source().symbols, previous)?;
        let target = crate::source_symbols::retain_declaration(&new.source().symbols, next)?;
        if retained.span() != target.span() || retained.kind() != target.kind() {
            return Err("the reached machine changed its authored declaration");
        }
        // The two accepted source maps are immutable. Their first declaration
        // pair checks the complete files and package scopes; later declarations
        // in the same files still check their own exact spans and roles.
        let files = (retained.source().source_id, target.source().source_id);
        if !rejoined_files.contains(&files) {
            crate::source_symbols::rejoin_declaration(&retained, &new.source().symbols, [next])?;
            rejoined_files.push(files);
        }
    }
    Ok(certificate)
}
