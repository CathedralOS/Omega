//! Keep the caller's exact registrar and timer-slot selections beside the call.
//!
//! Ordinary generic argument checking still owns every remaining argument.
//! This only recognizes the canonical library entrance; later admission must
//! establish its complete callback, storage and authority associations.

use crate::expressions::expression::lower_static_machine_argument;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
use symbol_resolved_trees as resolved;
use symbols::SymbolKind;
use typed_trees::expression::InstalledInterruptOperationRequest;

pub(crate) fn lower_request(
    program: Option<&resolved::SymbolResolvedTrees>,
    target_trees: &mut typed_trees::TypedTrees,
    exposure: Option<SelectionExposure>,
    occurrences: &[language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId],
    target_symbol: symbols::SymbolHandle,
    machine_arguments: &[resolved::expression::StaticMachineArgument],
) -> Result<Option<InstalledInterruptOperationRequest>, Diagnostic> {
    let Some(program) = program else {
        return Ok(None);
    };
    if !target_symbol.is_valid() {
        return Ok(None);
    }
    let target = program.symbols.get(target_symbol);
    if target.kind != SymbolKind::State {
        return Ok(None);
    }
    let Some(owner) = program.machines.unique_by_symbol(target.parent) else {
        return Ok(None);
    };
    if program.symbols.name(target_symbol) != "run"
        || !owner.attached_data_symbol.is_valid()
        || program.symbols.name(owner.attached_data_symbol) != "X86StaticInterruptScope"
        || program.symbols.symbol_source_origin(target_symbol)
            != Some(source::SourceOrigin::Toolchain)
        || program
            .symbols
            .symbol_source_origin(owner.attached_data_symbol)
            != Some(source::SourceOrigin::Toolchain)
    {
        // Same-spelled authored declarations remain ordinary calls. Only
        // the toolchain-owned declaration can retain installation evidence.
        return Ok(None);
    }
    if !program
        .machine_state_handles(owner.states)
        .iter()
        .any(|state| program.machine_state(*state).symbol == target_symbol)
    {
        return Err(Diagnostic::error(
            "interrupt installation lost its exact declaration's entry",
        ));
    }
    let [registrar, timer_slot, ..] = machine_arguments.as_ref() else {
        return Err(Diagnostic::error(
            "interrupt installation needs its exact registrar application and timer slot",
        ));
    };
    if registrar.const_literal.is_some() || registrar.evidence_projection.is_some() {
        return Err(Diagnostic::error(
            "interrupt installation requires a registrar declaration, not a value or evidence projection",
        ));
    }
    let mut declaration_path = registrar.path.to_vec();
    let mut continuation = registrar;
    while let Some(application) = &continuation.application
        && application.member.is_valid()
    {
        continuation = crate::tables::resolved_expressions(program)
            .static_member(application.member)
            .ok_or_else(|| {
                Diagnostic::error("interrupt registrar member has no owning resolved record")
            })?;
        declaration_path.extend_from_slice(&continuation.path);
    }
    // The requirement's own token may be a qualified member continuation.
    // Rejoin only occurrences already attached to this source call, never a
    // same-spelled selection elsewhere in the package.
    let registrar_selection = if let Some((first, last)) =
        continuation.path.first().zip(continuation.path.last())
    {
        let span = source::SourceSpan::new(
            first.source_span().source_id,
            source::Span::new(first.source_span().span.start, last.source_span().span.end),
        );
        let mut matches = occurrences.iter().copied().filter(|occurrence| {
            program.authored_declaration_selections().get(*occurrence).is_some_and(|selection| {
                selection.source_span() == span
                    && selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticArgument
            })
        });
        let selected = matches.next();
        if matches.next().is_some() || (first.is_source_backed() && selected.is_none()) {
            return Err(Diagnostic::error(
                "interrupt registrar lost its unique authored requirement selection",
            )
            .with_source_span(span));
        }
        selected
    } else {
        None
    };
    let selected = syntax_trees_to_symbol_resolved_trees::declaration_selection::resolve_signature_free_requirement(program, &declaration_path)
            .map_err(|reason| Diagnostic::error(format!("interrupt installation registrar declaration cannot be selected: {reason:?}")))?;
    let registrar_trait = selected.trait_definition.symbol;
    let mut registrar =
        lower_static_machine_argument(Some(program), target_trees, exposure, registrar)?;
    let mut member_handle = registrar
        .application
        .as_ref()
        .map(|application| application.member)
        .unwrap_or_default();
    if !member_handle.is_valid() {
        registrar.symbol = selected.requirement.symbol;
    } else {
        loop {
            let member = target_trees
                .expression_table
                .static_member_mut(member_handle)
                .ok_or_else(|| {
                    Diagnostic::error("interrupt registrar member has no owning typed record")
                })?;
            let next = member
                .application
                .as_ref()
                .map(|application| application.member)
                .unwrap_or_default();
            if !next.is_valid() {
                member.symbol = selected.requirement.symbol;
                break;
            }
            member_handle = next;
        }
    }
    if !timer_slot.symbol.is_valid()
        || program.symbols.get(timer_slot.symbol).kind != SymbolKind::Conformance
        || timer_slot.const_literal.is_some()
        || timer_slot.evidence_projection.is_some()
    {
        return Err(Diagnostic::error(
            "interrupt installation must identify one exact private callback slot conformance",
        ));
    }
    Ok(Some(InstalledInterruptOperationRequest {
        registrar_application: Default::default(),
        registrar_target: Default::default(),
        registrar_receiving_scope: Default::default(),
        registrar_selection,
        registrar_trait,
        registrar,
        timer_slot: lower_static_machine_argument(
            Some(program),
            target_trees,
            exposure,
            timer_slot,
        )?,
    }))
}
