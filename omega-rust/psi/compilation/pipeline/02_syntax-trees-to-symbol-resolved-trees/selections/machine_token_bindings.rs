/*
    Semantic-home and owner-local duplicate checks for direct machine token
    bindings.

    One declaration binds at most one fixed token, and distinct operand shapes
    may share a token; a second direct binding of the same token for the same
    owner and operand shape rejects at that second declaration
    ([expressions: machine token binding](../../../../../../wiki/spec/language/expressions.md)).
    The owner is the attached data declaration, or the declaring module for a
    free machine. Shapes compare by resolved symbol identity, so the check runs
    after every selection has settled and a same-leaf type from another module
    never collides.

    A closed direct family is published only by its semantic-home owner
    ([operator families](../../../../../../wiki/spec/language/expressions.md#operator-families)).
    Use sites select by operand types alone, so every binding that survives
    this module must carry its home in its operand tuple: an attached binding
    (`machine + Vec2::add`) must take `Vec2` somewhere in its telescope, and a
    free binding must name at least one declared type or domain. A telescope
    of bare compiler-owned primitives has no declaration-owned home, so
    `machine + add(left: u8, right: u8)` would inject into `u8`'s closed
    family and rejects here rather than becoming a candidate at every `u8 +`.
    That primitive-family rule binds every introducer -- direct machines,
    `operator` declarations and `boundary` requirement slots -- because each
    becomes a candidate for the token at a use site, and a provider-supplied
    slot replaces the primitive meaning just as a checked body would. Only
    the core semantic-home owner, the toolchain sources, publishes primitive
    operator meanings; compiler policy qualifiers such as `Wrapping` are not
    declared domains and supply no home.

    Ownership is package-scoped, not merely program-local: a closed direct
    family is published only by its semantic-home owner's package
    ([operator families](../../../../../../wiki/spec/language/expressions.md#operator-families)),
    so a `machine <token>` binding whose home is owned by another package
    rejects at its own declaration rather than entering that family's
    candidate set to collide at a use. The home for an attached binding is
    the attached declaration; for a domain-homed binding the domain; for a
    free binding every declared type or domain the operand tuple names.
    Package identity comes from each declaration's checked source owner, so
    symbols without one (toolchain, source-free, or unmanaged inputs) cannot
    prove a foreign owner and keep the owner-local rule.

    A binding attached to a domain (`machine + Quantity::Additive::add`) is a
    domain-family meaning ([domains: semantic roles and operators](../../../../../../wiki/spec/language/domains.md#semantic-roles-and-operators)):
    it participates only where an operand binding selects that domain, so its
    semantic home is the exact domain. At least one operand must carry that
    qualification. Association changes neither the domain's independent
    membership/access rules nor the machine's lexical body context.
*/

use crate::tables::body_expressions;
use std::collections::{HashMap, HashSet};

use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use source::SourceSpan;
use source::Span;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::TypeParameter;
use symbol_resolved_trees::machine::Machine;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::types::TypeConstraint;
use symbol_resolved_trees::types::TypeReference;
use symbols::{JoinedPath, SymbolHandle, SymbolKind};
use syntax_trees::operator_spelling::OperatorSpelling;

/*
    Reject every direct machine whose operand tuple omits its semantic home,
    then every spelling-bearing declaration whose fixed token, owner, and
    normalized operand shape repeat an earlier direct machine binding. Each
    rejection reports at its own declaration; a duplicate names the binding it
    repeats.

    `machine <token>` and `operator <token>` are one binding space while both
    introducers parse: the second declaration of an already-bound
    token/owner/shape triple rejects regardless of which introducer spelled
    it. Operator declarations are checked only against machine bindings here;
    an `operator` repeating another `operator` still rejects at the typed
    stage, which owns the same-name and spelling-overlap rules for the family
    it keeps while the introducer is being retired. `boundary` signatures are
    requirement slots, not supply, but they occupy the same token space at a
    use site, so a boundary row repeating a machine binding rejects here too.

    The semantic-home rule stays machine-only: attached `operator` spells like
    `Slice::index<T>(items: &[T], ...)` attach to a namespace that never joins
    the operand tuple, and trait-homed operators participate through
    conformance, not operand selection. Whether the operator form keeps that
    looser admission or retires behind the machine rule is an
    OPERATOR-MACHINE-SUPPLY question, not one this check settles. The one
    home rule every form answers is the primitive-family rule
    ([`TokenBinding::primitive_family_binding`]): no authored declaration of
    any introducer may bind a token over operands that name no declared type
    or domain.
*/
pub(crate) fn reject_duplicate_direct_token_bindings(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    // A retained forest also contains declarations excluded from this
    // activation. Only the selected frontier supplies token bindings; an
    // inactive Build declaration cannot reacquire a home in Product scope.
    let machine_bindings = program
        .machines
        .iter()
        .filter(|machine| program.symbols.symbol_source_is_selected(machine.symbol))
        .filter_map(|machine| {
            machine
                .spelling
                .map(|spelling| machine_token_binding(program, machine, spelling))
        })
        .collect::<Vec<_>>();
    let mut operator_bindings = program
        .operators
        .iter()
        .filter(|operator| program.symbols.symbol_source_is_selected(operator.symbol))
        .filter_map(|operator| {
            operator
                .spelling
                .map(|spelling| operator_token_binding(program, operator, None, spelling))
        })
        .collect::<Vec<_>>();
    for domain in program
        .domain_definitions
        .iter()
        .filter(|domain| program.symbols.symbol_source_is_selected(domain.symbol))
    {
        operator_bindings.extend(
            program
                .operator_definitions(domain.operators)
                .iter()
                .filter(|operator| program.symbols.symbol_source_is_selected(operator.symbol))
                .filter_map(|operator| {
                    operator.spelling.map(|spelling| {
                        operator_token_binding(program, operator, Some(domain), spelling)
                    })
                }),
        );
    }
    let mut bindings: Vec<TokenBinding<'_>> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut pending = Vec::new();
    for binding in machine_bindings {
        if let Some(diagnostic) = binding.missing_semantic_home(program) {
            diagnostics.push(diagnostic);
            continue;
        }
        if let Some(diagnostic) = binding.foreign_semantic_home(program) {
            diagnostics.push(diagnostic);
            continue;
        }
        if let Some(diagnostic) = binding.primitive_family_binding(program) {
            diagnostics.push(diagnostic);
            continue;
        }
        pending.extend(
            bindings
                .iter()
                .filter(|earlier| earlier.needs_typed_comparison(program, &binding))
                .map(
                    |earlier| symbol_resolved_trees::PendingTokenShapeComparison {
                        earlier: earlier.symbol,
                        later: binding.symbol,
                    },
                ),
        );
        if let Some(earlier) = bindings
            .iter()
            .find(|earlier| earlier.repeats(program, &binding))
        {
            diagnostics.push(duplicate_diagnostic(&binding, earlier));
            continue;
        }
        bindings.push(binding);
    }
    for binding in &operator_bindings {
        if let Some(diagnostic) = binding.primitive_family_binding(program) {
            diagnostics.push(diagnostic);
            continue;
        }
        pending.extend(
            bindings
                .iter()
                .filter(|earlier| earlier.needs_typed_comparison(program, binding))
                .map(
                    |earlier| symbol_resolved_trees::PendingTokenShapeComparison {
                        earlier: earlier.symbol,
                        later: binding.symbol,
                    },
                ),
        );
        if let Some(earlier) = bindings
            .iter()
            .find(|earlier| earlier.repeats(program, binding))
        {
            diagnostics.push(duplicate_diagnostic(binding, earlier));
        }
    }
    if diagnostics.is_empty() {
        program.pending_token_shape_comparisons.extend(pending);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/* Build the binding identity of a `machine <token>` declaration. */
fn machine_token_binding<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &'program Machine,
    spelling: OperatorSpelling,
) -> TokenBinding<'program> {
    TokenBinding {
        machine: Some(machine),
        symbol: machine.symbol,
        operand_types: entry_operand_types(program, machine),
        name: machine.name.as_str().to_owned(),
        name_span: machine.name.source_span(),
        owner_spelling: machine
            .attached_data
            .as_ref()
            .map_or_else(String::new, |attached| attached.as_str().to_owned()),
        spelling,
        owner: binding_owner(program, machine),
        operand_shape: ShapeNormalizer::new(program, machine.type_parameters)
            .operand_shape(entry_parameters(program, machine)),
        operand_display: ShapeNormalizer::for_display(program, machine.type_parameters)
            .operand_display(entry_parameters(program, machine)),
    }
}

fn duplicate_diagnostic(binding: &TokenBinding, earlier: &TokenBinding) -> Diagnostic {
    Diagnostic::error(format!(
        "`{}` binds the fixed operator token `{}` already bound by `{}` for the \
     same owner and operand shape ({}); a second spelling needs a distinct \
     operand shape or a separate owner",
        binding.name,
        binding.spelling.symbol(),
        earlier.name,
        binding.operand_display
    ))
    .with_source_span(binding.name_span)
}

struct TokenBinding<'program> {
    /*
        The declaring `machine` when this binding came from that introducer;
        `operator`-form bindings carry `None` and skip the semantic-home rule
        per the module contract above.
    */
    machine: Option<&'program Machine>,
    /*
        The declaring machine or operator symbol, whose source custody decides
        whether it may publish a primitive operator meaning.
    */
    symbol: SymbolHandle,
    /* The declaration's operand types in telescope order. */
    operand_types: Vec<&'program TypeReference>,
    /*
        The declaration's display path (`Vec2::add`, or the leaf for a
        domain-homed operator).
    */
    name: String,
    name_span: SourceSpan,
    /*
        The authored owner path (`Vec2`, `Quantity::Additive`, empty for a
        free binding). Breaks the tie between two owners whose paths resolved
        to no declaration symbol.
    */
    owner_spelling: String,
    spelling: OperatorSpelling,
    owner: BindingOwner,
    /*
        Duplicate-detection key: resolved symbol identities and binder
        positions, never shown to the author.
    */
    operand_shape: Option<String>,
    /* The same operand shape in authored spellings, for diagnostics. */
    operand_display: String,
}

impl TokenBinding<'_> {
    fn repeats(&self, program: &SymbolResolvedTrees, other: &Self) -> bool {
        self.spelling == other.spelling
            && self.same_owner(program, other)
            && self.operand_shape.is_some()
            && self.operand_shape == other.operand_shape
    }

    fn needs_typed_comparison(&self, program: &SymbolResolvedTrees, other: &Self) -> bool {
        self.spelling == other.spelling
            && self.same_owner(program, other)
            && (self.operand_shape.is_none() || other.operand_shape.is_none())
    }

    fn same_owner(&self, program: &SymbolResolvedTrees, other: &Self) -> bool {
        match (self.owner, other.owner) {
            (BindingOwner::AttachedData(left), BindingOwner::AttachedData(right))
                if !(left.is_valid() && right.is_valid()) =>
            {
                left == right
                    && self.owner_spelling == other.owner_spelling
                    && program
                        .symbols
                        .same_source_package(self.name_span, other.name_span)
            }
            _ => self.owner == other.owner,
        }
    }

    /*
        Whether the declaration comes from a toolchain source; `None` for
        source-free trees, which cannot prove custody either way.
    */
    fn is_toolchain_declaration(&self, program: &SymbolResolvedTrees) -> Option<bool> {
        program
            .symbols
            .symbol_source_origin(self.symbol)
            .map(|origin| origin == source::SourceOrigin::Toolchain)
    }

    /*
        The rejection for an authored binding whose operands name no declared
        type or domain, or `None` otherwise. Such a binding joins a primitive
        operator family (`u32 -`, `i32 ==`), which only the core semantic-home
        owner publishes; its spelled path cannot supply a home, since use sites
        select by operand types alone. A domain-homed binding participates only
        through operands its domain qualifies and keeps its own carrier rule.
        Toolchain declarations are the core owner; source-free trees cannot
        prove custody and are left to the remaining rules.
    */
    fn primitive_family_binding(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
        if matches!(self.owner, BindingOwner::Domain { .. })
            || self.is_toolchain_declaration(program)?
        {
            return None;
        }
        if self
            .operand_types
            .iter()
            .any(|type_reference| names_declaration(program, type_reference))
        {
            return None;
        }
        Some(
            Diagnostic::error(format!(
                "`{}` binds the fixed operator token `{}` over operands that name no declared \
             type or domain ({}); that joins a primitive operator family, which only the \
             core semantic-home owner may publish -- give the binding a data or domain \
             home among its operands",
                self.name,
                self.spelling.symbol(),
                self.operand_display,
            ))
            .with_source_span(self.name_span),
        )
    }

    /*
        The rejection for a binding whose operand tuple carries no semantic
        home, or `None` when some operand names it. Only `machine <token>`
        declarations answer this rule; `machine` is `None` on an
        `operator`-form binding, which is never checked for a home.
    */
    fn missing_semantic_home(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
        let machine = self.machine?;
        if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement {
            return None;
        }
        let operand_types = &self.operand_types;
        let message = match self.owner {
            BindingOwner::AttachedData(home) => {
                if operand_types
                    .iter()
                    .any(|type_reference| names_symbol(program, type_reference, home))
                {
                    return None;
                }
                format!(
                    "`{}` binds the fixed operator token `{}` but no operand names its semantic \
                 home `{}` ({}); a closed family's direct token bindings belong to the owner \
                 of a participating operand",
                    self.name,
                    self.spelling.symbol(),
                    self.owner_spelling,
                    self.operand_display
                )
            }
            BindingOwner::Domain { domain, .. } => {
                if operand_types
                    .iter()
                    .any(|type_reference| qualified_by_domain(program, type_reference, domain))
                {
                    return None;
                }
                format!(
                    "`{}` binds the fixed operator token `{}` but no operand carries \
                 its home domain `{}` ({}); a domain-family binding participates only through \
                 operands qualified by that domain",
                    self.name,
                    self.spelling.symbol(),
                    self.owner_spelling,
                    self.operand_display
                )
            }
            BindingOwner::Module(_) => return None,
        };
        Some(Diagnostic::error(message).with_source_span(self.name_span))
    }

    /*
        The rejection for a binding whose semantic home is owned by another
        package, or `None` when every home resolves inside the binding's own
        package. Only `machine <token>` declarations answer this rule (the
        same machine-only scope as [`TokenBinding::missing_semantic_home`]):
        `operator`-form and `boundary` slots occupy the token space but the
        introducer's retirement owns their admission change.

        A home owned by another package rejects the whole binding -- a
        package that owns one operand still injects the binding into the
        foreign operand's closed family. Declarations without a reconciled
        package identity cannot prove a foreign owner and are left to the
        owner-local rules.
    */
    fn foreign_semantic_home(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
        let machine = self.machine?;
        if machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement {
            return None;
        }
        let binding_package = program.symbols.symbol_package_identity(machine.symbol)?;
        let foreign = |home: SymbolHandle| {
            program
                .symbols
                .symbol_package_identity(home)
                .is_some_and(|owner| owner != binding_package)
        };
        let (home, participant) = match self.owner {
            BindingOwner::AttachedData(home) => {
                (home.is_valid() && foreign(home)).then_some((home, "attached declaration"))
            }
            BindingOwner::Domain { domain, .. } => {
                (domain.is_valid() && foreign(domain)).then_some((domain, "home domain"))
            }
            BindingOwner::Module(_) => self
                .operand_types
                .iter()
                .flat_map(|type_reference| declared_operand_symbols(program, type_reference))
                .find(|symbol| foreign(*symbol))
                .map(|symbol| (symbol, "operand declaration")),
        }?;
        Some(
            Diagnostic::error(format!(
                "`{}` binds the fixed operator token `{}` into a closed family through the \
             {participant} `{}`, which another package owns; only the semantic-home owner's \
             package may publish a closed direct family's token bindings",
                self.name,
                self.spelling.symbol(),
                program.symbols.display_path(home, "::"),
            ))
            .with_source_span(self.name_span),
        )
    }
}

/* The entry state's parameter types in telescope order. */
fn entry_operand_types<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> Vec<&'program TypeReference> {
    entry_parameters(program, machine)
        .iter()
        .map(|parameter| &parameter.type_reference)
        .collect()
}

/* The entry state's parameters, empty when the machine has no entry state. */
fn entry_parameters<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> &'program [StateParameter] {
    let Some(entry) = program.machine_state_handles(machine.states).first() else {
        return &[];
    };
    program.state_parameters(program.machine_state(*entry).parameters)
}

/*
    Build the binding identity of a spelling-bearing `operator`-form
    declaration: a classic `operator`, a `boundary operator`/`boundary
    machine` requirement slot (both lower to the same operator record), or —
    when `domain` is given — a member already homed into that domain's
    operator family by `normalize_domain_operator_homes`.

    The owner mirrors `binding_owner` for machines: a domain member takes its
    domain (and carrier) as owner; a root `operator` with a qualified name
    resolves its `Owner::` prefix through the same source-visible top-level
    lookup a machine's `attached_data` uses, and a bare `operator + f` is a
    free binding owned by its declaring module. Root operators whose owner
    path names a domain were moved by `normalize_domain_operator_homes`
    before this check runs, so the data-kind lookup is the whole residual
    rule: trait and builtin owners, like unresolvable ones, stay invalid
    symbols compared by their authored spelling.
*/
fn operator_token_binding<'program>(
    program: &'program SymbolResolvedTrees,
    operator: &'program symbol_resolved_trees::operator::OperatorDefinition,
    domain: Option<&'program symbol_resolved_trees::domain::DomainDefinition>,
    spelling: OperatorSpelling,
) -> TokenBinding<'program> {
    let members = program.operator_path_members(operator.name);
    let leaf = members.last();
    let name = members.joined_path();
    let name_span = members.first().map_or_else(SourceSpan::default, |first| {
        let first = first.source_span();
        let last = leaf.expect("nonempty members").source_span();
        SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end))
    });
    let (owner_spelling, owner) = match domain {
        Some(domain) => (
            domain.name.as_str().to_owned(),
            BindingOwner::Domain {
                domain: domain.symbol,
            },
        ),
        None => {
            let owner_members = &members[..members.len().saturating_sub(1)];
            if owner_members.is_empty() {
                (
                    String::new(),
                    BindingOwner::Module(program.symbols.symbol_module(operator.symbol)),
                )
            } else {
                let owner_spelling = owner_members.joined_path();
                let owner = program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &owner_spelling,
                        &[SymbolKind::Data],
                        owner_members
                            .first()
                            .map_or(name_span, |member| member.source_span()),
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
                (owner_spelling, BindingOwner::AttachedData(owner))
            }
        }
    };
    TokenBinding {
        machine: None,
        symbol: operator.symbol,
        operand_types: program
            .state_parameters(operator.parameters)
            .iter()
            .map(|parameter| &parameter.type_reference)
            .collect(),
        name,
        name_span,
        owner_spelling,
        spelling,
        owner,
        operand_shape: ShapeNormalizer::new(program, operator.type_parameters)
            .operand_shape(program.state_parameters(operator.parameters)),
        operand_display: ShapeNormalizer::for_display(program, operator.type_parameters)
            .operand_display(program.state_parameters(operator.parameters)),
    }
}

/*
    The declaration's own generic binder symbols. Each renders as its
    first-occurrence position in the operand telescope (see
    [`ShapeNormalizer`]), so renaming or reordering binders without changing
    their occurrence relationships — `index<T>(&[T], u64) -> T` and
    `index<U>(&[U], u64) -> U`, or `combine<T,U>(T,U)` and
    `combine<A,B>(B,A)` — renders one shape and the declarations collide as
    duplicates. Binders owned by another declaration — the attached data's
    `T` inside `Vec2<T>` — are not in this set and keep their symbol
    identity, so `Vec2<T>` and `Vec2<U>` under one owner stay distinct.
*/
fn own_binders(
    program: &SymbolResolvedTrees,
    type_parameters: HandleSpan<TypeParameter>,
) -> HashSet<SymbolHandle> {
    program
        .data_type_parameters(type_parameters)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect()
}

/*
    One name position of an operand's resolved type reference.

    [`name_positions`] owns the inventory; a rule states which positions it
    reads and how deep its reading reaches.
*/
enum TypePosition<'a> {
    /* A `Named` type or a generic application's base (`Vec2` in `Vec2<T>`). */
    Declaration { symbol: SymbolHandle },
    /*
        A domain constraint's spelling (`i32 in Degrees`). A constraint is
        still a name here; [`domain_named`] is the one place that joins a
        spelling to the declarations it selects.
    */
    Domain { name: &'a DiagnosticName },
}

/*
    How deep a rule's reading reaches. The operand-telescope rules ask
    different questions, so their reading depths genuinely differ.
*/
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadingScope {
    /* The complete generic argument telescope. */
    EveryArgument,
    /*
        The reference, constrained-base, array and slice chain, with no
        application arguments.
    */
    Chain,
}

/*
    Every name position of one operand's resolved type reference, in operand
    order.

    Every operand-telescope rule in this module reads the same closed
    inventory: a `Named` slot, a generic application's base and arguments,
    a domain constraint's spelling, and the
    reference, constrained-base, array and slice chains. This is the one owner
    of that inventory; a new position is one edit here rather than five
    recursions kept in step by comment. Two facts are structural, not scope: a
    constraint reports its spelling without resolving it, and a constraint's
    own arguments parameterize the indexed family rather than joining the
    operand tuple, so they are never positions.
*/
fn name_positions<'program>(
    program: &'program SymbolResolvedTrees,
    reference: &'program TypeReference,
    scope: ReadingScope,
) -> Vec<TypePosition<'program>> {
    let mut positions = Vec::new();
    extend_name_positions(program, reference, scope, &mut positions);
    positions
}

/* Append every position `scope` reaches, in operand order. */
fn extend_name_positions<'program>(
    program: &'program SymbolResolvedTrees,
    reference: &'program TypeReference,
    scope: ReadingScope,
    positions: &mut Vec<TypePosition<'program>>,
) {
    match reference {
        TypeReference::AliasUse { expansion, .. } => {
            extend_name_positions(program, expansion, scope, positions);
        }
        TypeReference::Named { symbol, .. } => {
            positions.push(TypePosition::Declaration { symbol: *symbol });
        }
        TypeReference::Generic(generic) => {
            positions.push(TypePosition::Declaration {
                symbol: generic.base_symbol,
            });
            if scope == ReadingScope::EveryArgument {
                for argument in program.child_type_references(generic.arguments) {
                    extend_name_positions(program, argument, scope, positions);
                }
            }
        }
        TypeReference::Callable { contract, .. } => {
            if scope == ReadingScope::EveryArgument {
                let signature = &program
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .get(*contract)
                    .signature;
                for parameter in program.state_parameters(signature.parameters) {
                    extend_name_positions(program, &parameter.type_reference, scope, positions);
                }
                if let Some(result) = &signature.return_type {
                    extend_name_positions(program, result, scope, positions);
                }
            }
        }
        TypeReference::Reference(inner) => extend_name_positions(
            program,
            program.child_type_reference(inner.referee),
            scope,
            positions,
        ),
        TypeReference::Constrained(constrained) => {
            for constraint in program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                if let TypeConstraint::Domain(domain) = constraint {
                    positions.push(TypePosition::Domain { name: &domain.name });
                }
            }
            extend_name_positions(
                program,
                program.child_type_reference(constrained.base_type),
                scope,
                positions,
            );
        }
        TypeReference::FixedArray(fixed_array) => {
            extend_name_positions(
                program,
                program.child_type_reference(fixed_array.element_type),
                scope,
                positions,
            );
        }
        TypeReference::Slice(slice) => extend_name_positions(
            program,
            program.child_type_reference(slice.element_type),
            scope,
            positions,
        ),
        TypeReference::ConstExpression(_) | TypeReference::Unit => {}
    }
}

/*
    Whether `symbol` is an authored declaration that can own a closed
    family.
*/
fn is_declaration(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Data | SymbolKind::Domain | SymbolKind::Trait
        )
}

/*
    Whether `type_reference` names the declaration `home` anywhere in its
    structure: directly, behind a reference, as a generic base, as the
    constrained carrier, or as an array or slice element. Every argument
    position is read, including an instantiated `dyn` requirement's.
*/
fn names_symbol(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    home: SymbolHandle,
) -> bool {
    name_positions(program, type_reference, ReadingScope::EveryArgument)
        .into_iter()
        .any(|position| match position {
            TypePosition::Declaration { symbol } => symbol == home,
            TypePosition::Domain { .. } => false,
        })
}

/*
    Whether a constraint selects the exact home domain, behind references,
    constrained bases, arrays and slices. A same-spelled constraint in another
    checked package or scope does not participate in this family's home.
    An application's arguments are not constraint positions.
*/
fn qualified_by_domain(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    domain: SymbolHandle,
) -> bool {
    name_positions(program, type_reference, ReadingScope::Chain)
        .into_iter()
        .any(|position| match position {
            TypePosition::Domain { name } => domain_named(program, name) == Some(domain),
            TypePosition::Declaration { .. } => false,
        })
}

/*
    Whether `type_reference` names any authored declaration or a declared
    domain constraint. Builtin primitives resolve to `BuiltinType` symbols and
    generic binders to `TypeParameter` symbols; neither is a declaration that
    can own a family, so a telescope of only those has no home. Generic
    application arguments are read.
*/
fn names_declaration(program: &SymbolResolvedTrees, type_reference: &TypeReference) -> bool {
    name_positions(program, type_reference, ReadingScope::EveryArgument)
        .into_iter()
        .any(|position| match position {
            TypePosition::Declaration { symbol } => is_declaration(program, symbol),
            TypePosition::Domain { name } => domain_named(program, name).is_some(),
        })
}

/*
    Every declaration-owned symbol a type reference participates in, over the
    walk [`names_declaration`] reads: named data/domain/trait declarations,
    the declarations behind references, generic bases and arguments, array and
    slice elements, constrained bases, and the declared domains named by
    domain constraints. Domain constraint arguments parameterize the indexed
    family rather than joining it, so they are not homes.
*/
fn declared_operand_symbols(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Vec<SymbolHandle> {
    let mut symbols = Vec::new();
    for position in name_positions(program, type_reference, ReadingScope::EveryArgument) {
        match position {
            TypePosition::Declaration { symbol } => {
                if is_declaration(program, symbol) {
                    symbols.push(symbol);
                }
            }
            TypePosition::Domain { name } => symbols.extend(domain_named(program, name)),
        }
    }
    symbols
}

/*
    Resolve a token's domain home with the membership pool. A signature-free
    unique hit does not return before the exposure scan. The same
    symbol reached by lookup and by the exposure scan is one home. Two
    different symbols stay unresolved even when they print the same.
*/
fn domain_named(program: &SymbolResolvedTrees, name: &DiagnosticName) -> Option<SymbolHandle> {
    match crate::names::membership_domain_lookup(
        &program.symbols,
        name.as_str(),
        name.source_span(),
    ) {
        crate::names::DomainMembership::Unique(symbol) => Some(symbol),
        crate::names::DomainMembership::NotFound
        | crate::names::DomainMembership::Ambiguous { .. }
        | crate::names::DomainMembership::Contested => None,
    }
}

/*
    Two unsourced `Quantity` rows are different symbols. A shared display
    path does not choose one of them.
*/
#[test]
fn domain_home_lookup_keeps_unsourced_ambiguity() {
    use symbols::{SymbolNameRef, SymbolTableBuilder};

    for count in [1, 2] {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let domains = builder.insert_children(
            root,
            (0..count).map(|_| (SymbolKind::Domain, SymbolNameRef::Static("Quantity"))),
        );
        let program = SymbolResolvedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        assert_eq!(
            domain_named(&program, &DiagnosticName::generated_static("Quantity")),
            (count == 1).then_some(domains.start()),
        );
    }
}

/*
    Ordinary lookup and the exposure scan can both name one domain. That
    repeated handle is one candidate.
*/
#[test]
fn one_domain_symbol_reached_through_two_routes_is_one_candidate() {
    use std::path::PathBuf;
    use std::sync::Arc;

    use source::{SourceMap, SourceSpan, Span};
    use symbols::{SymbolKind, SymbolLookup, SymbolNameRef, SymbolTableBuilder};

    let mut sources = SourceMap::default();
    let local = sources
        .add(PathBuf::from("pkg/local.omg"), "Quantity".to_owned())
        .source_id;
    let sources = Arc::new(sources);
    let reference = SourceSpan::new(local, Span::new(0, 8));
    let mut builder = SymbolTableBuilder::with_sources(Some(sources));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let quantity = builder.insert_children(
        root,
        [(
            SymbolKind::Domain,
            SymbolNameRef::OwnedSource {
                value: "Quantity",
                source_span: reference,
            },
        )],
    );
    let quantity = SymbolTableBuilder::child_handles(quantity)
        .next()
        .expect("one domain");
    let mut symbols = builder.finish();
    symbols
        .register_source_module(local, [("here", reference)])
        .expect("one module");
    assert_eq!(
        symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
            "Quantity",
            &[SymbolKind::Domain],
            reference,
            |_| true,
        ),
        SymbolLookup::Unique(quantity),
    );
    assert!(symbols.domain_name_reaches(quantity, "Quantity", reference));
    assert!(matches!(
        crate::names::membership_domain_lookup(&symbols, "Quantity", reference),
        crate::names::DomainMembership::Unique(symbol) if symbol == quantity
    ));
}

/*
    A signature-free lookup can see one exact `Quantity` and stop. The
    membership lookup still scans the module and also reaches the local leaf
    `Widget::Quantity`. Two visible domains compete, and neither the local
    module nor the signature-free hit wins, so the token home stays
    unresolved (`wiki/spec/language/modules.md#resolution`).
*/
#[test]
fn signature_free_unique_domain_does_not_hide_a_competing_local_domain() {
    use std::path::PathBuf;
    use std::sync::Arc;

    use source::{SourceMap, SourceSpan, Span};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    let mut sources = SourceMap::default();
    let local = sources
        .add(PathBuf::from("pkg/local.omg"), "Quantity".to_owned())
        .source_id;
    let sources = Arc::new(sources);
    let reference = SourceSpan::new(local, Span::new(0, 8));
    let mut builder = SymbolTableBuilder::with_sources(Some(sources));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let mut handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::Domain, SymbolNameRef::Static("Quantity")),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Widget::Quantity",
                    source_span: reference,
                },
            ),
        ],
    ));
    let signature_free = handles.next().expect("exact Quantity");
    let module_local = handles.next().expect("module-local leaf");
    let mut symbols = builder.finish();
    symbols
        .register_source_module(local, [("here", reference)])
        .expect("one module");
    let program = SymbolResolvedTrees {
        symbols,
        ..Default::default()
    };
    let selected = domain_named(&program, &DiagnosticName::from_str("Quantity", reference));
    assert_eq!(
        selected.map(|symbol| program.symbols.display_path(symbol, "::")),
        None,
        "competing {} and {} must not select a home",
        program.symbols.display_path(signature_free, "::"),
        program.symbols.display_path(module_local, "::"),
    );
}

/* The declaration set a direct binding is validated against. */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingOwner {
    /* `machine + Vec2::add(...)`: the exact attached data declaration. */
    AttachedData(SymbolHandle),
    /* `machine + Quantity::Additive::add(...)`: the exact domain owner. */
    Domain { domain: SymbolHandle },
    /* A free machine: its declaring module (invalid for root scope). */
    Module(SymbolHandle),
}

fn binding_owner(program: &SymbolResolvedTrees, machine: &Machine) -> BindingOwner {
    if machine.attached_data.is_none() {
        return BindingOwner::Module(program.symbols.symbol_module(machine.symbol));
    }
    if let Some(domain) = attached_domain(program, machine) {
        return BindingOwner::Domain {
            domain: domain.symbol,
        };
    }
    BindingOwner::AttachedData(machine.attached_data_symbol)
}

/*
    The domain resolved in the machine's authored source context. A semantic
    association is separate from data attachment and from lexical scope.
*/
fn attached_domain<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> Option<&'program symbol_resolved_trees::domain::DomainDefinition> {
    if !machine.operator_home_domain.is_valid() {
        return None;
    }
    program.domain_definition_by_symbol(machine.operator_home_domain)
}

/*
    Resolve semantic association once in the declaration's own source scope.
    A token binding remains a lexical machine; its domain association grants
    no imports, denotation role, membership or access authority.
*/
pub(crate) fn resolve_token_bound_domain_homes(program: &mut SymbolResolvedTrees) {
    let homes = program
        .machines
        .iter()
        .enumerate()
        .filter(|(_, machine)| {
            machine.spelling.is_some() && program.symbols.symbol_source_is_selected(machine.symbol)
        })
        .map(|(position, machine)| {
            let home = machine
                .attached_data
                .as_ref()
                .filter(|_| !machine.attached_data_symbol.is_valid())
                .and_then(|name| domain_named(program, name))
                .unwrap_or_else(SymbolHandle::invalid);
            (position, home)
        })
        .collect::<Vec<_>>();
    for (position, home) in homes {
        program.machines[position].operator_home_domain = home;
    }
}

/*
    Renders the complete operand telescope by resolved identity: symbols
    where resolution assigned them, authored spelling only for the remaining
    unsymbolled leaves, and the declaration's own generic binders as
    first-occurrence positions across the telescope. Reference lifetimes are
    borrow-region tags and never distinguish operand shapes.
*/
struct ShapeNormalizer<'program> {
    program: &'program SymbolResolvedTrees,
    /*
        The declaration's own binder symbols; every other symbol keeps its
        resolved identity.
    */
    own_binders: HashSet<SymbolHandle>,
    /*
        Own binder symbol → its first-occurrence position in the telescope,
        assigned during the single rendering pass.
    */
    ordinals: HashMap<SymbolHandle, usize>,
    /*
        Render authored spellings instead of identities: a diagnostic names
        `Wrapped`, not the arena slot that tells two same-spelled types apart.
    */
    display: bool,
    callable_depth: usize,
    has_dependent_callable_operand: bool,
}

impl<'program> ShapeNormalizer<'program> {
    fn new(
        program: &'program SymbolResolvedTrees,
        type_parameters: HandleSpan<TypeParameter>,
    ) -> Self {
        Self {
            program,
            own_binders: own_binders(program, type_parameters),
            ordinals: HashMap::new(),
            display: false,
            callable_depth: 0,
            has_dependent_callable_operand: false,
        }
    }

    fn for_display(
        program: &'program SymbolResolvedTrees,
        type_parameters: HandleSpan<TypeParameter>,
    ) -> Self {
        Self {
            display: true,
            ..Self::new(program, type_parameters)
        }
    }

    fn operand_shape(&mut self, parameters: &[StateParameter]) -> Option<String> {
        let shape = self.operand_display(parameters);
        (!self.has_dependent_callable_operand).then_some(shape)
    }

    fn operand_display(&mut self, parameters: &[StateParameter]) -> String {
        parameters
            .iter()
            .map(|parameter| self.parameter_shape(parameter))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn parameter_shape(&mut self, parameter: &StateParameter) -> String {
        let mut shape = String::new();
        if parameter.is_mutable {
            shape.push_str("mut ");
        }
        if parameter.is_const {
            shape.push_str("const ");
        }
        shape.push_str(&self.type_shape(&parameter.type_reference));
        shape
    }

    /*
        Render `symbol` as the position where it first occurs in the
        telescope when the declaring signature owns it, else by resolved
        identity or authored spelling.
    */
    fn binder_or_symbol(&mut self, symbol: SymbolHandle, spelling: &str) -> String {
        if self.display {
            return spelling.to_owned();
        }
        if self.own_binders.contains(&symbol) {
            let ordinal = self.ordinals.len();
            return format!("binder{}", *self.ordinals.entry(symbol).or_insert(ordinal));
        }
        symbol_or_spelling(symbol, spelling)
    }

    fn callable_shape(
        &mut self,
        signature: &symbol_resolved_trees::signature::StateSignature,
    ) -> String {
        // This key distinguishes operand shapes, so the nested callable's
        // parameters and result participate. Predicates and effects remain
        // in its contract; they cannot distinguish enclosing token overloads.
        let outer_binders = self.own_binders.clone();
        let outer_ordinals = self.ordinals.clone();
        self.own_binders
            .extend(own_binders(self.program, signature.type_parameters));
        self.callable_depth += 1;
        let parameters = self
            .program
            .data_type_parameters(signature.type_parameters)
            .to_vec();
        let binders = parameters
            .iter()
            .map(|parameter| {
                use symbol_resolved_trees::data::{MachineParameterContract, TypeParameterKind};
                let binder = self.binder_or_symbol(parameter.symbol, parameter.name.as_str());
                let kind = match &parameter.kind {
                    TypeParameterKind::Type => "type".to_owned(),
                    TypeParameterKind::Implementation => "implementation".to_owned(),
                    TypeParameterKind::Const { type_reference } => {
                        format!("const {}", self.type_shape(type_reference))
                    }
                    TypeParameterKind::Value { type_reference } => {
                        format!("value {}", self.type_shape(type_reference))
                    }
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(signature),
                    } => self.callable_shape(signature),
                    TypeParameterKind::Machine {
                        contract:
                            MachineParameterContract::Nominal {
                                requirement,
                                trait_arguments,
                                ..
                            },
                    } => {
                        let arguments = self
                            .program
                            .child_type_references(*trait_arguments)
                            .to_vec();
                        format!(
                            "nominal#{}<{}>",
                            requirement.arena_index(),
                            arguments
                                .iter()
                                .map(|argument| self.type_shape(argument))
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                    TypeParameterKind::Machine { .. } => {
                        self.has_dependent_callable_operand = true;
                        "pending machine".to_owned()
                    }
                    TypeParameterKind::Proposition { contract } => {
                        let parameters =
                            self.program.state_parameters(contract.parameters).to_vec();
                        format!("proposition({})", self.operand_display(&parameters))
                    }
                };
                format!("{binder}:{kind}")
            })
            .collect::<Vec<_>>()
            .join(",");
        let parameters = self.program.state_parameters(signature.parameters).to_vec();
        let operands = self.operand_display(&parameters);
        let result = signature
            .return_type
            .as_ref()
            .map_or_else(|| "()".to_owned(), |result| self.type_shape(result));
        self.callable_depth -= 1;
        self.own_binders = outer_binders;
        self.ordinals = outer_ordinals;
        format!("machine<{binders}>({operands})->{result}")
    }

    fn type_shape(&mut self, type_reference: &TypeReference) -> String {
        match type_reference {
            TypeReference::AliasUse { expansion, .. } => self.type_shape(expansion),
            TypeReference::Callable { contract, .. } => {
                let signature = self
                    .program
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .get(*contract)
                    .signature
                    .clone();
                self.callable_shape(&signature)
            }
            TypeReference::Reference(reference) => {
                let access = match reference.access {
                    ReferenceAccess::Shared => "&",
                    ReferenceAccess::Mutable => "&mut ",
                    ReferenceAccess::WriteOnly => "&write ",
                };
                let referee = self.program.child_type_reference(reference.referee);
                format!("{access}{}", self.type_shape(referee))
            }
            TypeReference::Constrained(constrained) => {
                let constraints = self
                    .program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                    .iter()
                    .map(|constraint| match constraint {
                        TypeConstraint::Domain(domain) if !domain.arguments.is_empty() => {
                            let arguments = self
                                .program
                                .child_type_references(domain.arguments)
                                .iter()
                                .map(|argument| self.type_shape(argument))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{}<{arguments}>", domain.name.as_str())
                        }
                        _ => constraint.display_name(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let base_type = self.program.child_type_reference(constrained.base_type);
                format!("{}[{constraints}]", self.type_shape(base_type))
            }
            TypeReference::FixedArray(fixed_array) => {
                let element_type = self.program.child_type_reference(fixed_array.element_type);
                let length = match &fixed_array.length {
                    symbol_resolved_trees::types::FixedArrayLength::Literal(value) => {
                        value.to_string()
                    }
                    symbol_resolved_trees::types::FixedArrayLength::ConstParameter {
                        symbol,
                        name,
                    } => self.binder_or_symbol(*symbol, name.as_str()),
                    symbol_resolved_trees::types::FixedArrayLength::ConstCall { machine, name } => {
                        self.binder_or_symbol(*machine, name.as_str())
                    }
                    symbol_resolved_trees::types::FixedArrayLength::Expression(reference) => {
                        self.type_shape(self.program.child_type_reference(*reference))
                    }
                };
                format!("[{}; {length}]", self.type_shape(element_type))
            }
            TypeReference::Slice(slice) => {
                let element_type = self.program.child_type_reference(slice.element_type);
                format!("[{}]", self.type_shape(element_type))
            }
            TypeReference::Generic(generic) => {
                let arguments = self
                    .program
                    .child_type_references(generic.arguments)
                    .iter()
                    .map(|argument| self.type_shape(argument))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{}<{arguments}>",
                    self.binder_or_symbol(generic.base_symbol, generic.base_name.as_str())
                )
            }
            TypeReference::ConstExpression(expression) => {
                if self.callable_depth != 0 && !self.display {
                    // No source rendering or placeholder is a dependent type
                    // identity. The stored pair must be checked after lowering.
                    self.has_dependent_callable_operand = true;
                }
                format!(
                    "const({})",
                    body_expressions(self.program).display_name(expression.expression)
                )
            }
            TypeReference::Named { symbol, name, .. } => {
                self.binder_or_symbol(*symbol, name.as_str())
            }
            TypeReference::Unit => "()".to_owned(),
        }
    }
}

fn symbol_or_spelling(symbol: SymbolHandle, spelling: &str) -> String {
    if symbol.is_valid() {
        format!("{spelling}#{}", symbol.arena_index())
    } else {
        spelling.to_owned()
    }
}
