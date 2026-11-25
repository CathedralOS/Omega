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
the forms `declared_type` reads) is
recorded on the declaration that holds it (`crate::source_state::deferred_refusals`)
and its entries stay without a result type. Bundled libraries are typed with
every program, so such a run must not stop a program that never reaches it.
*/

use crate::declaration_selection::AuthoredDeclarationSelectionKind;
use crate::source_state::TypedTrees;
use crate::source_state::deferred_refusals::DeferredRequest;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, TableCastExpression};
use crate::source_state::types::{
    DomainConstraint, DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};
use crate::tables::expression_table;
use crate::type_reference::domain_constraints::{
    normalize_domain_constraints_for_type, select_domain_candidates_where,
};
use crate::typing_queries::declared_expression_type as declared_type;
use diagnostics::Diagnostic;
use language_core::cast_form::{CastEntry, CastEntryKind};
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;

#[cfg(test)]
#[path = "domain_cast_accounting_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Atom {    Declared(SymbolHandle, semantic_vocabulary::SemanticDomainId),
    Policy(ArithmeticDomain),
}

type Spelled = (Atom, String, Option<TypeConstraintNode>);

struct Named {
    handle: ExpressionHandle,
    entry: CastEntry,
    selection: Option<(
        SymbolHandle,
        semantic_vocabulary::SemanticDomainId,
        arena::HandleSpan<TypeReferenceHandle>,
    )>,
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

fn read_run(program: &TypedTrees, outermost: ExpressionHandle) -> Run {
    let mut run = Vec::new();
    let mut cursor = outermost;
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
    run
}

/// A domain-only cast names its complete input set with `=` and `-`.
/// Use that set to select an operand call, without establishing any domain
/// or borrowing the post-cast domains from its receiving declaration.
pub(crate) fn call_operand_destination(
    source: &crate::source_resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    outermost: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Result<(ExpressionHandle, TypeReferenceHandle), Diagnostic> {
    let run = read_run(program, outermost);
    let Some((_, first, _)) = run.first() else {
        return Err(refuse(
            program,
            outermost,
            "call destination has no domain cast entries".to_owned(),
        ));
    };
    let operand = first.value;
    let (carrier, view) = match program
        .type_reference_table
        .type_reference(destination)
        .clone()
    {
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => (flatten(program, referee).0, Some((access, lifetime))),
        _ => (flatten(program, destination).0, None),
    };
    let named = named_domains(source, program, &run, carrier)?;
    let input: Vec<_> = named
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.entry.kind,
                CastEntryKind::Keep | CastEntryKind::Remove
            )
        })
        .flat_map(|entry| entry.atoms)
        .collect();
    let before = spell(source, program, carrier, &input)?;
    let before = if let Some((access, lifetime)) = view {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: before,
                access,
                lifetime,
            })
    } else {
        before
    };
    Ok((operand, before))
}

pub(crate) struct PreparedCasts {
    runs: Vec<Run>,
    initializer_owners: Vec<(SymbolHandle, SymbolHandle)>,
}

impl PreparedCasts {
    pub(crate) fn new(program: &TypedTrees, frontier: usize) -> Result<Self, Diagnostic> {
        let sites = entry_sites(program, frontier);
        if sites.is_empty() {
            return Ok(Self {
                runs: Vec::new(),
                initializer_owners: Vec::new(),
            });
        }
        // A non-first entry's operand is the previous entry of the same run.
        let inner: Vec<ExpressionHandle> = sites
            .iter()
            .filter(|(_, cast)| !cast.form.entry().is_some_and(|entry| entry.first))
            .map(|(_, cast)| cast.value)
            .collect();
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
            runs.push(read_run(program, *outermost));
        }

        Ok(Self {
            runs,
            initializer_owners,
        })
    }

    pub(crate) fn sites(&self) -> impl Iterator<Item = (ExpressionHandle, usize)> + '_ {
        self.runs
            .iter()
            .enumerate()
            .map(|(position, run)| (run[0].0, position))
    }

    pub(crate) fn account(
        &self,
        position: usize,
        source: &crate::source_resolved::SymbolResolvedTrees,
        program: &mut TypedTrees,
        owners: &crate::source_state::references::ProgramOperandOwners,
        cast_destinations: &[TypeReferenceHandle],
    ) -> Result<(), Diagnostic> {
        let run = &self.runs[position];
        let symbol_key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
        let outermost = run[run.len() - 1].0;
        let mut holders = owners
            .expression_owners(outermost)
            .filter(|owner| owner.is_valid())
            .map(|owner| {
                self.initializer_owners
                    .binary_search_by_key(&symbol_key(owner), |(entry, _)| symbol_key(*entry))
                    .map_or(owner, |position| self.initializer_owners[position].1)
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
        let request = match account_run(source, program, owner, run, cast_destinations) {
            Ok(()) => return Ok(()),
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

        Ok(())
    }
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

/// Follow the declaration's own parent membership at the child's exact application.
/// Before preparation only its index expressions need substitution here.
fn ancestors(
    program: &mut TypedTrees,
    carrier: TypeReferenceHandle,
    mut child: Atom,
) -> Result<Vec<Spelled>, Unaccounted> {
    use crate::source_state::domain::refinement_parent;
    let unavailable = || Unaccounted::Deferred(DeferredRequest::DomainCastUnnamedConstraint);
    let mut ancestors = Vec::new();
    while let Atom::Declared(symbol, identity) = child {
        let prepared = program.domain_definition_for_instance(symbol, identity);
        let Some(declaration) = prepared.or_else(|| program.domain_definition(symbol)) else {
            return Err(unavailable());
        };
        let Some(fact) = refinement_parent(program, declaration) else {
            break;
        };
        let parent = fact.domain_symbol;
        let mut parent_id = fact.semantic_domain;
        let mut arguments = program
            .type_reference_table
            .type_reference_handles(fact.domain_arguments)
            .to_vec();
        if prepared.is_none() {
            let bindings = program
                .domain_arguments_for_instance(symbol, identity)
                .ok_or_else(unavailable)?;
            let bindings = bindings.to_vec();
            if !crate::type_reference::carrier_qualifications::substitute_indices(program, symbol, &bindings, &mut arguments) {
                return Err(unavailable());
            }
            parent_id = program.retain_domain_application(parent, carrier, &arguments)?;
        }
        let definition = program.domain_definition(parent).ok_or_else(unavailable)?;
        child = Atom::Declared(parent, parent_id);
        ancestors.push((
            child,
            domain_label(program, definition),
            Some(TypeConstraintNode::Domain(DomainConstraint {
                name: definition.name.clone(),
                symbol: parent,
                semantic_id: parent_id,
                arguments,
                ..DomainConstraint::default()
            })),
        ));
    }
    Ok(ancestors)
}

/* A domain as a cast entry spells it: its path under the carrier it is declared on. */
fn domain_label(
    program: &TypedTrees,
    definition: &crate::source_state::domain::DomainDefinition,
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
    let Atom::Declared(symbol, semantic_id) = atom else {
        return None;
    };
    let definition = program.domain_definition(symbol)?;
    Some(TypeConstraintNode::Domain(DomainConstraint {
        name: definition.name.clone(),
        symbol,
        semantic_id,
        ..DomainConstraint::default()
    }))
}

fn is_anonymous_literal(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    crate::source_state::closed_numeric::has_anonymous_numeric_results(program, expression)
}

fn authored_span(
    program: &TypedTrees,
    handle: ExpressionHandle,
) -> Option<source_text::SourceSpan> {
    expression_table(program)
        .authored_selection_occurrences(handle)
        .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
        .find(|selection| selection.kind() == AuthoredDeclarationSelectionKind::DomainMembership)
        .map(|selection| selection.source_span())
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
    source: &crate::source_resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    owner: Option<SymbolHandle>,
    run: &Run,
    cast_destinations: &[TypeReferenceHandle],
) -> Result<TypeReferenceHandle, Unaccounted> {
    let (first, first_cast, _) = run[0];
    let operand = first_cast.value;
    let label = entry_label(program, &first_cast);
    let destination = cast_destinations
        .get(run[run.len() - 1].0.arena_index() as usize)
        .copied()
        .filter(|destination| destination.is_valid());
    // A quoted or array literal received by a fixed array is that array's
    // plain carrier: `"000" as +Utf8` into `[u8; 3] in Utf8` is the 3-byte
    // array the literal spells, which the entries then give the domain
    // (domains.md: "`[u8; N] in D` is the same exact `N`-byte fixed-array
    // carrier as `[u8; N]`"). The literal is typed against that carrier as
    // any literal stored into a `[u8; 3]` is.
    if matches!(
        program.expression_table.expression(operand),
        ExpressionNode::String(_) | ExpressionNode::ArrayLiteral(_)
    ) && let Some(destination) = destination
    {
        let plain = flatten(program, destination).0;
        if matches!(
            program.type_reference_table.type_reference(plain),
            TypeReferenceNode::FixedArray { .. }
        ) {
            return Ok(plain);
        }
    }
    if let Some(reference) = owner.and_then(|owner| declared_type(program, owner, operand)) {
        return Ok(reference);
    }
    // A quoted literal is a shared view of its bytes in no domain
    // (domains.md, "Byte containers and encoding domains": "Quoted literals
    // supply raw bytes"). With a receiving view it is that view with the
    // domains its referent displays left off, which the entries then add.
    // It keeps no lifetime the destination names: that binder belongs to
    // the destination's declaration (a record's field), and a literal takes
    // the lifetime its use requires (lifetimes.md, "Caller-chosen result
    // lifetimes"), which the reader decides from that use.
    if matches!(
        program.expression_table.expression(operand),
        ExpressionNode::String(_)
    ) {
        let Some(TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared,
            ..
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
                lifetime: None,
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
            |domain| !crate::source_state::domain::has_generic_carrier(program, domain),
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
    program: &mut TypedTrees,
    carrier: TypeReferenceHandle,
    constraints: &[TypeConstraintNode],
) -> Result<Vec<Spelled>, Unaccounted> {
    let mut members: Vec<Spelled> = Vec::new();
    for constraint in constraints {
        let (atom, label) = match constraint {
            TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact) => continue,
            TypeConstraintNode::ArithmeticDomain(policy) => {
                (Atom::Policy(*policy), policy.name().to_owned())
            }
            TypeConstraintNode::Domain(domain)
                if domain.subject == DomainConstraintSubject::Declared
                    && domain.symbol.is_valid() =>
            {
                (
                    Atom::Declared(domain.symbol, domain.semantic_id),
                    program.domain_definition(domain.symbol).map_or_else(
                        || domain.name.as_str().to_owned(),
                        |definition| domain_label(program, definition),
                    ),
                )
            }
            TypeConstraintNode::Domain(_) | TypeConstraintNode::Named(_) => {
                return Err(Unaccounted::Deferred(
                    DeferredRequest::DomainCastUnnamedConstraint,
                ));
            }
        };
        let written = (atom, label, Some(constraint.clone()));
        if let Some(existing) = members.iter_mut().find(|entry| entry.0 == atom) {
            *existing = written;
        } else {
            members.push(written);
        }
        for parent in ancestors(program, carrier, atom)? {
            if !members.iter().any(|entry| entry.0 == parent.0) {
                members.push(parent);
            }
        }
    }
    Ok(members)
}

/* Which domains the entries name, each at most once. */
fn named_domains(
    source: &crate::source_resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    run: &Run,
    carrier: TypeReferenceHandle,
) -> Result<Vec<Named>, Diagnostic> {
    let mut named: Vec<Named> = Vec::new();
    let mut before = None;
    let mut after = None;
    for (handle, cast, entry) in run {
        let label = entry_label(program, cast);
        let mut selection = None;
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
            let mut candidates = select_domain_candidates_where(
                source,
                program,
                &name,
                authored_span(program, *handle),
                SymbolHandle::invalid(),
                |domain| {
                    crate::source_state::domain::domain_accepts_underlying_carrier(
                        program, domain, carrier, &arguments,
                    )
                },
            );
            let competing = candidates.len() > 1;
            if competing {
                use crate::type_reference::carrier_qualifications::{CarrierQualifications, DomainUse};
                for (excluded, side) in [(CastEntryKind::Add, &mut before), (CastEntryKind::Remove, &mut after)] {
                    if entry.kind == excluded { continue; }
                    let available = side.get_or_insert_with(|| {
                        let mut available = CarrierQualifications::default();
                        for (other, cast, entry) in run {
                            if entry.kind == excluded { continue; }
                            if cast.semantic_domain.is_empty() { available.include_policy(cast.domain); continue; }
                            let name = expression_table(program).name_path_string(cast.semantic_domain);
                            let arguments = program.type_reference_table.type_reference_handles(cast.semantic_domain_arguments).to_vec();
                            available.include(source, program, carrier, DomainUse {
                                name: &name, arguments: &arguments, reference: authored_span(program, *other), retained: SymbolHandle::invalid()
                            });
                        }
                        available
                    });
                    available.retain_possible(source, program, &mut candidates, &arguments);
                }
            }
            let [domain] = candidates.as_slice() else {
                let owner = program.display_type_reference(carrier);
                let reason = match candidates.as_slice() {
                    [] if competing => format!("names no eligible domain `{name}` on the qualifications of `{owner}`"),
                    [] => format!("names no visible domain `{name}` of `{owner}`"),
                    [first, second, ..] => format!("names more than one visible domain `{name}` of `{owner}`: `{}` and `{}`",
                        source.symbols.display_path(first.symbol, "::"), source.symbols.display_path(second.symbol, "::")),
                    [_] => unreachable!(),
                };
                return Err(refuse(
                    program,
                    *handle,
                    format!(
                        "cast entry `{label}` {reason}. A `+` or `-` after a cast always begins a domain entry; for arithmetic on the cast result, parenthesise the cast"
                    ),
                ));
            };
            let (semantic_id, supplied) = crate::type_reference::selected_domain_application(
                program,
                domain.symbol,
                carrier,
                cast.semantic_domain_arguments,
            )?;
            let arguments = program
                .type_reference_table
                .type_reference_handles(supplied)
                .to_vec();
            selection = Some((domain.symbol, semantic_id, supplied));
            if domain.alias.is_some() {
                crate::source_state::domain::expanded_alias_members(program, domain.symbol)
                    .map_err(|reason| {
                        refuse(program, *handle, format!("cannot expand cast entry `{label}` ({reason})"))
                    })?
                    .into_iter()
                    .map(|(member, identity)| {
                        (
                            Atom::Declared(member.symbol, identity),
                            domain_label(program, member),
                            None,
                        )
                    })
                    .collect()
            } else {
                vec![(
                    Atom::Declared(domain.symbol, semantic_id),
                    domain_label(program, domain),
                    Some(TypeConstraintNode::Domain(DomainConstraint {
                        name: domain.name.clone(),
                        symbol: domain.symbol,
                        semantic_id,
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
            selection,
            atoms,
            label,
        });
    }
    Ok(named)
}

fn account_run(
    source: &crate::source_resolved::SymbolResolvedTrees,
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
    // Account for the referent's domains while retaining the complete loan.
    // Lowering and checking still owe every removed domain at handback.
    let view = match program.type_reference_table.type_reference(operand_type) {
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => Some((*referee, *access, lifetime.clone())),
        _ => None,
    };
    let accounted = view
        .as_ref()
        .map_or(operand_type, |(referee, _, _)| *referee);
    let (carrier, constraints) = flatten(program, accounted);
    let operand_display = program.display_type_reference_with_constraints(operand_type);
    let members = carried(program, carrier, &constraints)?;
    let named = named_domains(source, program, run, carrier)?;

    // Each entry against what the operand carries.
    for entry in &named {
        for (atom, domain_label, _) in &entry.atoms {
            let carried = members.iter().any(|member| member.0 == *atom);
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
    // Only an addition implicitly accounts for its ancestors. An explicitly
    // named ancestor keeps its written position, in either order.
    let mut inherited: Vec<(ExpressionHandle, Spelled)> = Vec::new();
    for entry in &named {
        if entry.entry.kind == CastEntryKind::Remove {
            continue;
        }
        for (atom, label, _) in &entry.atoms {
            for parent in ancestors(program, carrier, *atom)? {
                match kind_of(parent.0) {
                    Some(CastEntryKind::Remove) => {
                        return Err(refuse(
                            program,
                            entry.handle,
                            format!(
                                "domain cast would leave `{label}` without its parent `{}`",
                                parent.1,
                            ),
                        )
                        .into());
                    }
                    None if entry.entry.kind == CastEntryKind::Add
                        && !inherited.iter().any(|(_, member)| member.0 == parent.0) =>
                    {
                        inherited.push((entry.handle, parent));
                    }
                    _ => {}
                }
            }
        }
    }
    for (atom, label, _) in &members {
        if kind_of(*atom).is_none() && !inherited.iter().any(|(_, member)| member.0 == *atom) {
            return Err(refuse(
                program,
                outermost,
                format!(
                    "domain cast leaves attached domain `{label}` unaccounted for: the operand's type `{operand_display}` carries `{label}`, so the cast names it as `={label}` to keep it or `-{label}` to remove it",
                ),
            ).into());
        }
    }
    let policies = named
        .iter()
        .filter(|entry| entry.entry.kind != CastEntryKind::Remove)
        .flat_map(|entry| &entry.atoms)
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
    // Intermediate entries carry the exact set accounted for so far.
    // Only the whole simultaneous run needs a closed refinement chain.
    let mut running = members;
    for entry in &named {
        let before = spell(source, program, carrier, &running)?;
        match entry.entry.kind {
            CastEntryKind::Keep => {}
            CastEntryKind::Remove => {
                running.retain(|member| !entry.atoms.iter().any(|removed| removed.0 == member.0))
            }
            CastEntryKind::Add => {
                running.extend(entry.atoms.iter().cloned());
                for (_, parent) in inherited.iter().filter(|(site, _)| *site == entry.handle) {
                    if !running.iter().any(|member| member.0 == parent.0) {
                        running.push(parent.clone());
                    }
                }
            }
        }
        let after = spell(source, program, carrier, &running)?;
        // A view's entry carries the same reference to each referent type.
        let (before, after) = match &view {
            Some((_, access, lifetime)) => {
                let mut lent = |referee| {
                    // Changing the displayed domains does not move a named
                    // lifetime into the caller's declaration.
                    let occurrence = program.type_reference_table.fresh_occurrence(operand_type);
                    program.type_reference_table.substitute_node(
                        occurrence,
                        TypeReferenceNode::Reference {
                            referee,
                            access: *access,
                            lifetime: lifetime.clone(),
                        },
                    );
                    occurrence
                };
                (lent(before), lent(after))
            }
            None => (before, after),
        };
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(entry.handle)
        else {
            continue;
        };
        if let Some((symbol, identity, arguments)) = entry.selection {
            cast.semantic_domain_arguments = arguments;
            cast.semantic_domain_symbol = symbol;
            cast.semantic_domain_id = identity;
        }
        match entry.entry.kind {
            CastEntryKind::Add => {
                cast.target_type = before;
                cast.result_type = after;
            }
            CastEntryKind::Keep | CastEntryKind::Remove => {
                cast.target_type = after;
                cast.result_type = after;
                // Keep the written policy as the entry's identity. The
                // settled result type already records whether it survives;
                // source capture still distinguishes it from a named domain.
            }
        }
    }
    Ok(())
}

fn spell(
    source: &crate::source_resolved::SymbolResolvedTrees,
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
