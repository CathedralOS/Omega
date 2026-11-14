/*
We account for domain casts, `value as =Keep -Remove +Add` (domains.md,
"Domain casts"): `account` reads each run of entries as the one simultaneous
cast the source wrote and leaves every entry with its target and result type.
*/

/*
The parser leaves one entry node per named domain, innermost first. Typing's
part of the cast is what can be read from declarations: which domain each
entry names, that the entries account for every domain the operand's declared
type carries, each exactly once, and that the result is a well-formed set (a
child with its parents, one arithmetic policy). Each entry then carries the
type before it and after it, and each named domain keeps its declaration on the
node. Whether an addition is allowed (predicates, issuance) and what a removal
must retain are judged where the declaration is checked, not here.

A run typing cannot account for yet (its operand's declared type is not one of
the forms `declared_type` reads, or the cast applies through a reference) is
recorded on the declaration that holds it (`typed_trees::deferred_refusals`)
and its entries stay without a result type. Bundled libraries are typed with
every program, so such a run must not stop a program that never reaches it.
*/

use crate::tables::expression_table;
use crate::type_reference::domain_constraints::{
    normalize_domain_constraints_for_type, select_domain_candidates, select_domain_candidates_where,
};
use diagnostics::Diagnostic;
use language_core::cast_form::{CastEntry, CastEntryKind};
use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use typed_trees::types::{
    DomainConstraint, DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};

#[cfg(test)]
#[path = "domain_cast_accounting_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Atom {
    Declared(SymbolHandle, language_semantics::SemanticDomainId),
    Policy(ArithmeticDomain),
}

type Spelled = (Atom, String, Option<TypeConstraintNode>);

struct Member {
    atom: Atom,
    label: String,
    node: Option<TypeConstraintNode>, // The operand type's own constraint, when it spells this domain.
    attached: bool,                   // Listed by the type itself, not only implied as an ancestor.
}

struct Named {
    handle: ExpressionHandle,
    entry: CastEntry,
    atoms: Vec<Spelled>,
    label: String,
}

type Run = Vec<(ExpressionHandle, TableCastExpression, CastEntry)>;

/// Why a run is not accounted: a source error, or a form typing does not
/// serve yet.
enum Unaccounted {
    Rejected(Diagnostic),
    Deferred(DeferredRequest),
}

impl From<Diagnostic> for Unaccounted {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Rejected(diagnostic)
    }
}

fn is_new(handle: ExpressionHandle, frontier: usize) -> bool {
    !usize::try_from(handle.arena_index()).is_ok_and(|index| index <= frontier)
}

fn entry_sites(
    program: &TypedTrees,
    frontier: usize,
) -> Vec<(ExpressionHandle, TableCastExpression)> {
    expression_table(program)
        .expression_entries()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            (is_new(handle, frontier) && cast.form.entry().is_some()).then_some((handle, *cast))
        })
        .collect()
}

pub(crate) fn account(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    frontier: usize,
    cast_destinations: &[TypeReferenceHandle],
) -> Result<(), Diagnostic> {
    let sites = entry_sites(program, frontier);
    if sites.is_empty() {
        return Ok(());
    }
    // A non-first entry's operand is the previous entry of the same run.
    let inner: Vec<ExpressionHandle> = sites
        .iter()
        .filter(|(_, cast)| !cast.form.entry().is_some_and(|entry| entry.first))
        .map(|(_, cast)| cast.value)
        .collect();
    let owners = typed_trees::references::collect_program_operand_owners(program);
    // A retained constant and its exact initializer entry share source
    // expressions. They are one source owner, unlike two unrelated callables
    // that happen to retain the same expression handle.
    let symbol_key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut initializer_owners = program
        .const_declarations()
        .iter()
        .filter(|declaration| {
            program
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
        })
        .map(|declaration| {
            (
                program
                    .tables
                    .machines
                    .get(declaration.evaluation_entry)
                    .symbol,
                declaration.symbol,
            )
        })
        .collect::<Vec<_>>();
    initializer_owners.sort_unstable_by_key(|(entry, _)| symbol_key(*entry));
    if initializer_owners
        .windows(2)
        .any(|pair| pair[0].0 == pair[1].0)
    {
        return Err(Diagnostic::error(
            "domain cast initializer entry has more than one constant owner",
        ));
    }
    let mut runs: Vec<Run> = Vec::new();
    for (outermost, _) in sites.iter().filter(|(handle, _)| !inner.contains(handle)) {
        let mut run = Vec::new();
        let mut cursor = *outermost;
        while let Some(cast) = program.expression_table.cast_node(cursor).copied() {
            let Some(entry) = cast.form.entry() else {
                break;
            };
            run.push((cursor, cast, entry));
            if entry.first {
                break;
            }
            cursor = cast.value;
        }
        run.reverse();
        runs.push(run);
    }
    // Operands are allocated before the casts over them, so inner runs settle first.
    runs.sort_by_key(|run| run[0].0.arena_index());
    for run in runs {
        let outermost = run[run.len() - 1].0;
        let mut holders = owners
            .expression_owners(outermost)
            .filter(|owner| owner.is_valid())
            .map(|owner| {
                initializer_owners
                    .binary_search_by_key(&symbol_key(owner), |(entry, _)| symbol_key(*entry))
                    .map_or(owner, |position| initializer_owners[position].1)
            })
            .collect::<Vec<_>>();
        holders.sort_unstable_by_key(|owner| symbol_key(*owner));
        holders.dedup();
        let owner = match holders.as_slice() {
            [owner] => Some(*owner),
            [] => None,
            _ => {
                return Err(refuse(
                    program,
                    outermost,
                    "domain cast has more than one exact lexical owner".into(),
                ));
            }
        };
        let request = match account_run(source, program, owner, &run, cast_destinations) {
            Ok(()) => continue,
            Err(Unaccounted::Rejected(diagnostic)) => return Err(diagnostic),
            Err(Unaccounted::Deferred(request)) => request,
        };
        let source_span = program.expression_table.source_span(run[0].0);
        if holders.is_empty() {
            return Err(Diagnostic::error(format!(
                "no product route yet: an expression outside every declaration is one {}",
                request.construct()
            ))
            .with_source_span(source_span));
        }
        for holder in holders {
            program.record_deferred_refusal(holder, request, source_span);
        }
    }
    Ok(())
}

fn refuse(program: &TypedTrees, handle: ExpressionHandle, message: String) -> Diagnostic {
    Diagnostic::error(message).with_source_span(expression_table(program).source_span(handle))
}

fn flatten(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> (TypeReferenceHandle, Vec<TypeConstraintNode>) {
    let mut layers = Vec::new();
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        layers.push(
            program
                .type_reference_table
                .constraints(*constraints)
                .to_vec(),
        );
        reference = *base_type;
    }
    layers.reverse();
    (reference, layers.into_iter().flatten().collect())
}

/* The domain whose path this domain's own path extends by one member. */
fn parent(program: &TypedTrees, domain: SymbolHandle) -> Option<SymbolHandle> {
    let definition = program.domain_definition(domain)?;
    typed_trees::domain::subject_membership_facts(program, definition)
        .map(|membership| membership.domain_symbol)
        .find(|candidate| {
            *candidate != domain
                && program.domain_definition(*candidate).is_some_and(|parent| {
                    definition
                        .name
                        .as_str()
                        .strip_prefix(parent.name.as_str())
                        .and_then(|rest| rest.strip_prefix("::"))
                        .is_some_and(|leaf| !leaf.contains("::"))
                })
        })
}

fn is_ancestor(program: &TypedTrees, ancestor: SymbolHandle, mut child: SymbolHandle) -> bool {
    while let Some(next) = parent(program, child) {
        if next == ancestor {
            return true;
        }
        child = next;
    }
    false
}

fn implies(program: &TypedTrees, child: Atom, ancestor: Atom) -> bool {
    matches!((child, ancestor), (Atom::Declared(child, _), Atom::Declared(ancestor, _))
            if child != ancestor && is_ancestor(program, ancestor, child))
}

fn declared_atom(program: &TypedTrees, symbol: SymbolHandle) -> Option<(Atom, String)> {
    let definition = program.domain_definition(symbol)?;
    Some((
        Atom::Declared(symbol, definition.semantic_id),
        domain_label(program, definition),
    ))
}

/* A domain as a cast entry spells it: its path under the carrier it is declared on. */
fn domain_label(
    program: &TypedTrees,
    definition: &typed_trees::domain::DomainDefinition,
) -> String {
    let name = definition.name.as_str();
    name.strip_prefix(
        program
            .display_type_reference(definition.target_type)
            .as_str(),
    )
    .and_then(|rest| rest.strip_prefix("::"))
    .unwrap_or(name)
    .to_owned()
}

fn declared_node(program: &TypedTrees, atom: Atom) -> Option<TypeConstraintNode> {
    let Atom::Declared(symbol, _) = atom else {
        return None;
    };
    let definition = program.domain_definition(symbol)?;
    Some(TypeConstraintNode::Domain(DomainConstraint {
        name: definition.name.clone(),
        symbol,
        ..DomainConstraint::default()
    }))
}

fn is_anonymous_literal(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    typed_trees::closed_numeric::has_anonymous_numeric_results(program, expression)
}

fn authored_span(program: &TypedTrees, handle: ExpressionHandle) -> Option<source::SourceSpan> {
    expression_table(program)
        .authored_selection_occurrences(handle)
        .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
        .find(|selection| selection.kind() == AuthoredDeclarationSelectionKind::DomainMembership)
        .map(|selection| selection.source_span())
}

/*
    The declared type of an operand, for the forms whose type is written in a
    declaration or follows from one: a place (a parameter, an annotated local,
    a field or element of one), a cast, and builtin arithmetic over plain
    operands. `None` is "typing does not read this form", never "no domains":
    the caller defers the run. Calls, matches and unannotated locals are not
    read here; the checker of the lowered form types every expression and
    re-checks each entry against the operand it finds.
*/
fn declared_type(
    program: &TypedTrees,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let table = &program.expression_table;
    if !table.expression_is_valid(expression) {
        return None;
    }
    match table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            crate::typing_queries::declared_callable_value_type(program, owner, expression).ok()
        }
        // An accounted entry and a settled cast carry their result. A plain
        // carrier cast results in the carrier it names.
        ExpressionNode::Cast(cast) if cast.result_type.is_valid() => Some(cast.result_type),
        ExpressionNode::Cast(cast)
            if cast.form == language_core::CastForm::Value
                && cast.domain == ArithmeticDomain::Exact
                && cast.semantic_domain.is_empty()
                && cast.target_type.is_valid() =>
        {
            Some(cast.target_type)
        }
        // domains.md, "Operands": builtin operators take unqualified operands
        // only, so over builtin carriers with no domain the result is that
        // plain carrier. An operand that carries a domain, or a data type,
        // selects a declared operator, whose result typing does not read.
        ExpressionNode::Binary(binary)
            if binary.operator.is_arithmetic() && binary.policy == ArithmeticDomain::Exact =>
        {
            let mut result = None;
            for operand in [binary.left, binary.right] {
                if is_anonymous_literal(program, operand) {
                    continue;
                }
                let reference = declared_type(program, owner, operand)?;
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                if program.symbols.get(*symbol).kind != symbols::SymbolKind::BuiltinType
                    || result.is_some_and(|earlier| earlier != *symbol)
                {
                    return None;
                }
                result = Some(*symbol);
            }
            let operand = [binary.left, binary.right]
                .into_iter()
                .find(|operand| !is_anonymous_literal(program, *operand))?;
            result.and_then(|_| declared_type(program, owner, operand))
        }
        _ => None,
    }
}

/*
    `value as T +D`: the carrier cast in front of the entries takes an
    operand with no domains (domains.md, "Carrier casts"). Without this
    check the conversion would drop whatever `value` carried without any
    entry naming it, which is the silent removal entries exist to end.
*/
fn refuse_qualified_carrier_operand(
    program: &TypedTrees,
    owner: Option<SymbolHandle>,
    run: &Run,
) -> Result<(), Unaccounted> {
    let (first, first_cast, entry) = run[0];
    if !entry.after_carrier {
        return Ok(());
    }
    let Some(carrier_cast) = program.expression_table.cast_node(first_cast.value) else {
        return Ok(());
    };
    let target = program.display_type_reference(carrier_cast.target_type);
    let converted = owner.and_then(|owner| declared_type(program, owner, carrier_cast.value));
    let Some(converted) = converted else {
        if is_anonymous_literal(program, carrier_cast.value) {
            return Ok(());
        }
        return Err(Unaccounted::Deferred(
            DeferredRequest::DomainCastOperandType,
        ));
    };
    let (_, constraints) = flatten(program, converted);
    let carried = constraints
        .iter()
        .filter_map(|constraint| match constraint {
            TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact) => None,
            TypeConstraintNode::ArithmeticDomain(policy) => Some(policy.name().to_owned()),
            TypeConstraintNode::Domain(domain) => {
                Some(program.domain_definition(domain.symbol).map_or_else(
                    || domain.name.as_str().to_owned(),
                    |definition| domain_label(program, definition),
                ))
            }
            TypeConstraintNode::Named(name) => Some(name.as_str().to_owned()),
        })
        .collect::<Vec<_>>();
    if carried.is_empty() {
        return Ok(());
    }
    let removal = carried
        .iter()
        .map(|label| format!("-{label}"))
        .collect::<Vec<_>>()
        .join(" ");
    Err(refuse(
        program,
        first,
        format!(
            "carrier cast `as {target}` converts a value whose type `{}` carries domain `{}`; a carrier cast takes an operand with no domains, so remove them by name first: `value as {removal} as {target} {}`",
            program.display_type_reference_with_constraints(converted),
            carried.join("`, `"),
            entry_label(program, &first_cast),
        ),
    )
    .into())
}

fn entry_label(program: &TypedTrees, cast: &TableCastExpression) -> String {
    expression_table(program)
        .name_path_string(cast.target_label)
        .trim()
        .to_owned()
}

/* An anonymous operand takes its receiving carrier, or a carrier fixed
by a named domain when no receiving type is available. */
fn operand_type(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    owner: Option<SymbolHandle>,
    run: &Run,
    cast_destinations: &[TypeReferenceHandle],
) -> Result<TypeReferenceHandle, Unaccounted> {
    let (first, first_cast, _) = run[0];
    let operand = first_cast.value;
    if let Some(reference) = owner.and_then(|owner| declared_type(program, owner, operand)) {
        return Ok(reference);
    }
    let label = entry_label(program, &first_cast);
    // A quoted literal is a shared view of its bytes in no domain
    // (domains.md, "Byte containers and encoding domains": "Quoted literals
    // supply raw bytes"). With a receiving view it is that view with the
    // domains its referent displays left off, which the entries then add.
    if matches!(
        program.expression_table.expression(operand),
        ExpressionNode::String(_)
    ) {
        let destination = cast_destinations
            .get(run[run.len() - 1].0.arena_index() as usize)
            .copied()
            .filter(|destination| destination.is_valid());
        let Some(TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared,
            lifetime,
        }) = destination.map(|destination| {
            program
                .type_reference_table
                .type_reference(destination)
                .clone()
        })
        else {
            return Err(Unaccounted::Deferred(
                DeferredRequest::DomainCastOperandType,
            ));
        };
        let referee = flatten(program, referee).0;
        return Ok(program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                lifetime,
            }));
    }
    if !is_anonymous_literal(program, operand) {
        return Err(Unaccounted::Deferred(
            DeferredRequest::DomainCastOperandType,
        ));
    }
    if let Some(destination) = cast_destinations
        .get(run[run.len() - 1].0.arena_index() as usize)
        .filter(|destination| destination.is_valid())
    {
        // The anonymous operand starts unqualified. Taking the full
        // expected type here would silently insert its domains before
        // the authored entries have accounted for them.
        return Ok(flatten(program, *destination).0);
    }
    for (handle, cast, _) in run {
        if cast.semantic_domain.is_empty() {
            continue;
        }
        let name = expression_table(program).name_path_string(cast.semantic_domain);
        let candidates = select_domain_candidates_where(
            source,
            program,
            &name,
            authored_span(program, *handle),
            SymbolHandle::invalid(),
            |domain| !typed_trees::domain::has_generic_carrier(program, domain),
        );
        if let [domain] = candidates.as_slice() {
            return Ok(domain.target_type);
        }
    }
    Err(refuse(
        program,
        first,
        format!(
            "the literal in `as {label}` has no carrier that the named domain fixes; write the carrier first, as in `5 as i32 +Km`",
        ),
    )
    .into())
}

/* What the operand's type carries: its own constraints and each one's ancestors. */
fn carried(
    program: &TypedTrees,
    constraints: &[TypeConstraintNode],
) -> Result<Vec<Member>, Unaccounted> {
    let mut members: Vec<Member> = Vec::new();
    for constraint in constraints {
        match constraint {
            TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact) => {}
            TypeConstraintNode::ArithmeticDomain(policy) => members.push(Member {
                atom: Atom::Policy(*policy),
                label: policy.name().to_owned(),
                node: Some(constraint.clone()),
                attached: true,
            }),
            TypeConstraintNode::Domain(domain)
                if domain.subject == DomainConstraintSubject::Declared
                    && domain.symbol.is_valid() =>
            {
                let atom = Atom::Declared(domain.symbol, domain.semantic_id);
                match members.iter_mut().find(|member| member.atom == atom) {
                    Some(member) => {
                        member.attached = true;
                        member.node = Some(constraint.clone());
                    }
                    None => members.push(Member {
                        atom,
                        label: program.domain_definition(domain.symbol).map_or_else(
                            || domain.name.as_str().to_owned(),
                            |definition| domain_label(program, definition),
                        ),
                        node: Some(constraint.clone()),
                        attached: true,
                    }),
                }
                let mut ancestor = parent(program, domain.symbol);
                while let Some(symbol) = ancestor {
                    if let Some((atom, label)) = declared_atom(program, symbol)
                        && !members.iter().any(|member| member.atom == atom)
                    {
                        members.push(Member {
                            atom,
                            label,
                            node: None,
                            attached: false,
                        });
                    }
                    ancestor = parent(program, symbol);
                }
            }
            // A compiler-owned domain or a constraint no declaration
            // answered: nothing here says how an entry names it.
            TypeConstraintNode::Domain(_) | TypeConstraintNode::Named(_) => {
                return Err(Unaccounted::Deferred(
                    DeferredRequest::DomainCastUnnamedConstraint,
                ));
            }
        }
    }
    Ok(members)
}

/* Which domains the entries name, each at most once. */
fn named_domains(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    run: &Run,
    carrier: TypeReferenceHandle,
) -> Result<Vec<Named>, Diagnostic> {
    let mut named: Vec<Named> = Vec::new();
    for (handle, cast, entry) in run {
        let label = entry_label(program, cast);
        let atoms: Vec<Spelled> = if cast.semantic_domain.is_empty() {
            vec![(
                Atom::Policy(cast.domain),
                cast.domain.name().to_owned(),
                Some(TypeConstraintNode::ArithmeticDomain(cast.domain)),
            )]
        } else {
            let name = expression_table(program).name_path_string(cast.semantic_domain);
            let arguments = program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments)
                .to_vec();
            let candidates = select_domain_candidates(
                source,
                program,
                carrier,
                &name,
                authored_span(program, *handle),
                SymbolHandle::invalid(),
                &arguments,
            );
            let [domain] = candidates.as_slice() else {
                let reason = if candidates.is_empty() {
                    "names no visible domain"
                } else {
                    "names more than one visible domain"
                };
                return Err(refuse(
                    program,
                    *handle,
                    format!(
                        "cast entry `{label}` {reason} `{name}` of `{}`. A `+` or `-` after a cast always begins a domain entry; for arithmetic on the cast result, parenthesise the cast",
                        program.display_type_reference(carrier),
                    ),
                ));
            };
            if let Some(alias) = &domain.alias {
                // Naming an alias names its expansion.
                alias
                    .constituents
                    .iter()
                    .filter_map(|constituent| declared_atom(program, constituent.domain_symbol))
                    .map(|(atom, label)| (atom, label, None))
                    .collect()
            } else {
                let semantic_id = if arguments.is_empty() {
                    domain.semantic_id
                } else {
                    let instance = typed_trees::domain::indexed_domain_instance_name(
                        program,
                        domain,
                        typed_trees::domain::index_parameters(program, domain),
                        &arguments,
                    )?;
                    program.semantic_domains.intern(&instance)
                };
                vec![(
                    Atom::Declared(domain.symbol, semantic_id),
                    domain_label(program, domain),
                    Some(TypeConstraintNode::Domain(DomainConstraint {
                        name: domain.name.clone(),
                        symbol: domain.symbol,
                        arguments,
                        ..DomainConstraint::default()
                    })),
                )]
            }
        };
        for (atom, domain_label, _) in &atoms {
            if named
                .iter()
                .any(|earlier| earlier.atoms.iter().any(|(seen, _, _)| seen == atom))
            {
                return Err(refuse(
                    program,
                    *handle,
                    format!(
                        "cast entry `{label}` names domain `{domain_label}` a second time in one cast; each domain appears exactly once, as `=`, `-` or `+`",
                    ),
                ));
            }
        }
        named.push(Named {
            handle: *handle,
            entry: *entry,
            atoms,
            label,
        });
    }
    Ok(named)
}

fn account_run(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    owner: Option<SymbolHandle>,
    run: &Run,
    cast_destinations: &[TypeReferenceHandle],
) -> Result<(), Unaccounted> {
    let outermost = run[run.len() - 1].0;
    refuse_qualified_carrier_operand(program, owner, run)?;
    let operand_type = operand_type(source, program, owner, run, cast_destinations)?;
    // domains.md, "Mutable views": "A cast applies to a reference by
    // accounting for the domains displayed on its referent, with the same
    // syntax ... The cast preserves the referent, lifetime and access mode".
    // Through a shared reference "any removal a by-value cast permits is
    // accepted and nothing is owed", so the accounting is the referent's and
    // each type an entry carries is that reference again. A mutable or
    // write-only view may owe a removed domain back, which this does not
    // account for yet.
    let view = match program.type_reference_table.type_reference(operand_type) {
        TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared,
            lifetime,
        } => Some((*referee, lifetime.clone())),
        TypeReferenceNode::Reference { .. } => {
            return Err(Unaccounted::Deferred(
                DeferredRequest::DomainCastThroughReference,
            ));
        }
        _ => None,
    };
    let accounted = view.as_ref().map_or(operand_type, |(referee, _)| *referee);
    let (carrier, constraints) = flatten(program, accounted);
    let operand_display = program.display_type_reference_with_constraints(operand_type);
    let members = carried(program, &constraints)?;
    let named = named_domains(source, program, run, carrier)?;

    // Each entry against what the operand carries.
    for entry in &named {
        for (atom, domain_label, _) in &entry.atoms {
            let carried = members.iter().any(|member| member.atom == *atom);
            match entry.entry.kind {
                CastEntryKind::Add if carried => {
                    return Err(refuse(
                        program,
                        entry.handle,
                        format!(
                            "cast entry `{}` adds domain `{domain_label}`, but the operand's type `{operand_display}` already carries `{domain_label}`; write `={domain_label}` to keep it",
                            entry.label,
                        ),
                    )
                    .into());
                }
                CastEntryKind::Keep | CastEntryKind::Remove if !carried => {
                    return Err(refuse(
                        program,
                        entry.handle,
                        format!(
                            "cast entry `{}` names domain `{domain_label}`, but the operand's type `{operand_display}` does not carry `{domain_label}`; only `+{domain_label}` can name a domain the operand lacks",
                            entry.label,
                        ),
                    )
                    .into());
                }
                _ => {}
            }
        }
    }
    let kind_of = |atom: Atom| {
        named
            .iter()
            .find(|entry| entry.atoms.iter().any(|(named, _, _)| *named == atom))
            .map(|entry| entry.entry.kind)
    };
    for member in &members {
        if kind_of(member.atom).is_none() {
            return Err(refuse(
                program,
                outermost,
                format!(
                    "domain cast leaves attached domain `{label}` unaccounted for: the operand's type `{operand_display}` carries `{label}`, so the cast names it as `={label}` to keep it or `-{label}` to remove it",
                    label = member.label,
                ),
            )
            .into());
        }
    }

    // The result set: a child never outlives its parent.
    let mut result: Vec<Spelled> = members
        .iter()
        .filter(|member| kind_of(member.atom) == Some(CastEntryKind::Keep))
        .map(|member| (member.atom, member.label.clone(), member.node.clone()))
        .collect();
    for entry in &named {
        if entry.entry.kind == CastEntryKind::Add {
            result.extend(entry.atoms.iter().cloned());
        }
    }
    // One arithmetic policy per type: `Wrapping & Saturating` has no single meaning for `+`.
    let policies = result
        .iter()
        .filter(|(atom, _, _)| matches!(atom, Atom::Policy(_)))
        .map(|(_, label, _)| format!("`{label}`"))
        .collect::<Vec<_>>();
    if policies.len() > 1 {
        return Err(refuse(
            program,
            outermost,
            format!(
                "domain cast would leave competing arithmetic policies {} on one type; remove one of them in the same cast",
                policies.join(" and "),
            ),
        )
        .into());
    }
    for (atom, label, _) in &result {
        let Atom::Declared(symbol, _) = atom else {
            continue;
        };
        if let Some(parent) = parent(program, *symbol)
            && let Some((parent_atom, parent_label)) = declared_atom(program, parent)
            && !result.iter().any(|(member, _, _)| *member == parent_atom)
        {
            return Err(refuse(
                program,
                outermost,
                format!(
                    "domain cast would leave `{label}` without its parent `{parent_label}`; a child domain travels with its ancestors, so keep or add `{parent_label}` as well, or remove `{label}`",
                ),
            )
            .into());
        }
    }

    /*
        Give each entry the type before it and after it. The entries are one
        simultaneous accounting; the running set only spells the types the
        nodes carry, in the order written. It starts at what the operand's
        type lists and changes as each entry is applied. A type spells a
        child without its ancestors, so an ancestor is spelled only while no
        listed domain implies it.
    */
    let mut running: Vec<Spelled> = members
        .iter()
        .filter(|member| member.attached)
        .map(|member| (member.atom, member.label.clone(), member.node.clone()))
        .collect();
    for entry in &named {
        let before = spell(source, program, carrier, &running)?;
        match entry.entry.kind {
            CastEntryKind::Keep => {}
            CastEntryKind::Remove => {
                for (atom, _, _) in &entry.atoms {
                    running.retain(|(member, _, _)| member != atom);
                }
                for member in &members {
                    if kind_of(member.atom) == Some(CastEntryKind::Keep)
                        && !running.iter().any(|(listed, _, _)| {
                            *listed == member.atom || implies(program, *listed, member.atom)
                        })
                    {
                        running.push((member.atom, member.label.clone(), member.node.clone()));
                    }
                }
            }
            CastEntryKind::Add => {
                for (atom, label, node) in &entry.atoms {
                    running.retain(|(listed, _, _)| !implies(program, *atom, *listed));
                    if !running
                        .iter()
                        .any(|(listed, _, _)| implies(program, *listed, *atom))
                    {
                        running.push((*atom, label.clone(), node.clone()));
                    }
                }
            }
        }
        let after = spell(source, program, carrier, &running)?;
        // A view's entry carries the same reference to each referent type.
        let (before, after) = match &view {
            Some((_, lifetime)) => {
                let mut lent = |referee| {
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Reference {
                            referee,
                            access: language_core::ReferenceAccess::Shared,
                            lifetime: lifetime.clone(),
                        })
                };
                (lent(before), lent(after))
            }
            None => (before, after),
        };
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(entry.handle)
        else {
            continue;
        };
        match entry.entry.kind {
            CastEntryKind::Add => {
                cast.target_type = before;
                cast.result_type = after;
            }
            CastEntryKind::Keep | CastEntryKind::Remove => {
                cast.target_type = after;
                cast.result_type = after;
                cast.domain = ArithmeticDomain::Exact;
            }
        }
    }
    Ok(())
}

fn spell(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    carrier: TypeReferenceHandle,
    members: &[Spelled],
) -> Result<TypeReferenceHandle, Diagnostic> {
    if members.is_empty() {
        return Ok(carrier);
    }
    if let [(Atom::Policy(policy), _, _)] = members
        && let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(carrier)
        && let Some(retained) = program
            .type_reference_table
            .find_arithmetic_result_type_reference(*symbol, *policy)
    {
        return Ok(retained);
    }
    let constraints = members
        .iter()
        .filter_map(|(atom, _, node)| node.clone().or_else(|| declared_node(program, *atom)))
        .collect::<Vec<_>>();
    let constraints = program.type_reference_table.insert_constraints(constraints);
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: carrier,
            constraints,
        });
    normalize_domain_constraints_for_type(source, program, reference)?;
    Ok(reference)
}
