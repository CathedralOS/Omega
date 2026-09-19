//! Semantic-home and owner-local duplicate checks for direct machine token
//! bindings.
//!
//! One declaration binds at most one fixed token, and distinct operand shapes
//! may share a token; a second direct binding of the same token for the same
//! owner and operand shape rejects at that second declaration
//! ([expressions: machine token binding](../../../../../../../wiki/spec/language/expressions.md)).
//! The owner is the attached data declaration, or the declaring module for a
//! free machine. Shapes compare by resolved symbol identity, so the check runs
//! after every selection has settled and a same-leaf type from another module
//! never collides.
//!
//! A closed direct family is published only by its semantic-home owner
//! ([operator families](../../../../../../../wiki/spec/language/expressions.md#operator-families)).
//! Use sites select by operand types alone, so every binding that survives
//! this module must carry its home in its operand tuple: an attached binding
//! (`machine + Vec2::add`) must take `Vec2` somewhere in its telescope, and a
//! free binding must name at least one declared type or domain. A telescope
//! of bare compiler-owned primitives has no declaration-owned home, so
//! `machine + add(left: u8, right: u8)` would inject into `u8`'s closed
//! family and rejects here rather than becoming a candidate at every `u8 +`.
//! Which declared type is the home of a free binding, and whether that
//! declaration's package may publish it, are typed-stage and package-graph
//! questions and remain open in OPERATOR-MACHINE-SUPPLY.
//!
//! A binding attached to a domain (`machine + Quantity::Additive::add`) is a
//! domain-family meaning ([domains: semantic roles and operators](../../../../../../../wiki/spec/language/domains.md#semantic-roles-and-operators)):
//! it participates only where an operand binding selects that domain, so its
//! semantic home is the domain's carrier (the data or builtin type the domain
//! classifies), not the domain symbol itself. The domain is its owner for the
//! duplicate check, and declaring such a binding gives the domain its
//! denotation role exactly as a domain-homed `operator` declaration did, so
//! implicit weakening and result-dispatch keep treating it as semantic.

use std::collections::HashMap;

use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::TypeParameter;
use symbol_resolved_trees::machine::Machine;
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::operator_spelling::OperatorSpelling;

/// Reject every direct machine whose operand tuple omits its semantic home,
/// then every spelling-bearing declaration whose fixed token, owner, and
/// normalized operand shape repeat an earlier direct machine binding. Each
/// rejection reports at its own declaration; a duplicate names the binding it
/// repeats.
///
/// `machine <token>` and `operator <token>` are one binding space while both
/// introducers parse: the second declaration of an already-bound
/// token/owner/shape triple rejects regardless of which introducer spelled
/// it. Operator declarations are checked only against machine bindings here;
/// an `operator` repeating another `operator` still rejects at the typed
/// stage, which owns the same-name and spelling-overlap rules for the family
/// it keeps while the introducer is being retired. `boundary` signatures are
/// requirement slots, not supply, but they occupy the same token space at a
/// use site, so a boundary row repeating a machine binding rejects here too.
///
/// The semantic-home rule stays machine-only: attached `operator` spells like
/// `Slice::index<T>(items: &[T], ...)` attach to a namespace that never joins
/// the operand tuple, and trait-homed operators participate through
/// conformance, not operand selection. Whether the operator form keeps that
/// looser admission or retires behind the machine rule is an
/// OPERATOR-MACHINE-SUPPLY question, not one this check settles.
pub(crate) fn reject_duplicate_direct_token_bindings(
    program: &SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut bindings: Vec<TokenBinding<'_>> = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines.iter() {
        let Some(spelling) = machine.spelling else {
            continue;
        };
        let binding = TokenBinding {
            machine: Some(machine),
            name: machine.name.as_str().to_owned(),
            name_span: machine.name.source_span(),
            owner_spelling: machine
                .attached_data
                .as_ref()
                .map_or_else(String::new, |attached| attached.as_str().to_owned()),
            spelling,
            owner: binding_owner(program, machine),
            operand_shape: operand_shape(
                program,
                entry_parameters(program, machine),
                &own_binders(program, machine.type_parameters),
            ),
        };
        if let Some(diagnostic) = binding.missing_semantic_home(program) {
            diagnostics.push(diagnostic);
            continue;
        }
        if let Some(earlier) = bindings.iter().find(|earlier| earlier.repeats(&binding)) {
            diagnostics.push(duplicate_diagnostic(&binding, earlier));
            continue;
        }
        bindings.push(binding);
    }
    for operator in program.operators.iter() {
        let Some(spelling) = operator.spelling else {
            continue;
        };
        let binding = operator_token_binding(program, operator, None, spelling);
        if let Some(earlier) = bindings.iter().find(|earlier| earlier.repeats(&binding)) {
            diagnostics.push(duplicate_diagnostic(&binding, earlier));
        }
    }
    for domain in program.domain_definitions.iter() {
        for operator in program.operator_definitions(domain.operators) {
            let Some(spelling) = operator.spelling else {
                continue;
            };
            let binding = operator_token_binding(program, operator, Some(domain), spelling);
            if let Some(earlier) = bindings.iter().find(|earlier| earlier.repeats(&binding)) {
                diagnostics.push(duplicate_diagnostic(&binding, earlier));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
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
        binding.operand_shape
    ))
    .with_source_span(binding.name_span)
}

struct TokenBinding<'program> {
    /// The declaring `machine` when this binding came from that introducer;
    /// `operator`-form bindings carry `None` and skip the semantic-home rule
    /// per the module contract above.
    machine: Option<&'program Machine>,
    /// The declaration's display path (`Vec2::add`, or the leaf for a
    /// domain-homed operator).
    name: String,
    name_span: SourceSpan,
    /// The authored owner path (`Vec2`, `Quantity::Additive`, empty for a
    /// free binding). Breaks the tie between two owners whose paths resolved
    /// to no declaration symbol.
    owner_spelling: String,
    spelling: OperatorSpelling,
    owner: BindingOwner,
    operand_shape: String,
}

impl TokenBinding<'_> {
    fn repeats(&self, other: &Self) -> bool {
        self.spelling == other.spelling
            && self.same_owner(other)
            && self.operand_shape == other.operand_shape
    }

    fn same_owner(&self, other: &Self) -> bool {
        match (self.owner, other.owner) {
            (BindingOwner::AttachedData(left), BindingOwner::AttachedData(right))
                if !(left.is_valid() && right.is_valid()) =>
            {
                // An owner path that resolved to no data declaration leaves an
                // invalid symbol; invalid handles all compare equal, so the
                // authored owner spelling separates unrelated attachments.
                left == right && self.owner_spelling == other.owner_spelling
            }
            _ => self.owner == other.owner,
        }
    }

    /// The rejection for a binding whose operand tuple carries no semantic
    /// home, or `None` when some operand names it. Only `machine <token>`
    /// declarations answer this rule; `machine` is `None` on an
    /// `operator`-form binding, which is never checked for a home.
    fn missing_semantic_home(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
        let machine = self.machine?;
        let operand_types = entry_operand_types(program, machine);
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
                    self.operand_shape
                )
            }
            BindingOwner::Domain { domain, carrier } => {
                // A generic domain (`domain<T, const U: Unit> T::Quantity<U>`)
                // classifies an open carrier, so an operand participates by
                // being qualified with the domain itself.
                let domain_name = program
                    .domain_definitions
                    .iter()
                    .find(|definition| definition.symbol == domain)
                    .map(|definition| definition.name.as_str().to_owned())
                    .unwrap_or_default();
                if operand_types.iter().any(|type_reference| {
                    (carrier.is_valid() && names_symbol(program, type_reference, carrier))
                        || qualified_by_domain(program, type_reference, &domain_name)
                }) {
                    return None;
                }
                format!(
                    "`{}` binds the fixed operator token `{}` but no operand names the carrier of \
                     its home domain `{}` ({}); a domain-family binding participates only through \
                     operands the domain classifies",
                    self.name,
                    self.spelling.symbol(),
                    self.owner_spelling,
                    self.operand_shape
                )
            }
            BindingOwner::Module(_) => {
                if operand_types
                    .iter()
                    .any(|type_reference| names_declaration(program, type_reference))
                {
                    return None;
                }
                format!(
                    "`{}` binds the fixed operator token `{}` over operands that name no declared \
                     type or domain ({}); compiler-owned primitive families accept no direct \
                     token bindings",
                    self.name,
                    self.spelling.symbol(),
                    self.operand_shape
                )
            }
        };
        Some(Diagnostic::error(message).with_source_span(self.name_span))
    }
}

/// The entry state's parameter types in telescope order.
fn entry_operand_types<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> Vec<&'program TypeReference> {
    entry_parameters(program, machine)
        .iter()
        .map(|parameter| &parameter.type_reference)
        .collect()
}

/// The entry state's parameters, empty when the machine has no entry state.
fn entry_parameters<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> &'program [StateParameter] {
    let Some(entry) = program.machine_state_handles(machine.states).first() else {
        return &[];
    };
    program.state_parameters(program.machine_state(*entry).parameters)
}

/// Build the binding identity of a spelling-bearing `operator`-form
/// declaration: a classic `operator`, a `boundary operator`/`boundary
/// machine` requirement slot (both lower to the same operator record), or —
/// when `domain` is given — a member already homed into that domain's
/// operator family by `normalize_domain_operator_homes`.
///
/// The owner mirrors `binding_owner` for machines: a domain member takes its
/// domain (and carrier) as owner; a root `operator` with a qualified name
/// resolves its `Owner::` prefix through the same source-visible top-level
/// lookup a machine's `attached_data` uses, and a bare `operator + f` is a
/// free binding owned by its declaring module. Root operators whose owner
/// path names a domain were moved by `normalize_domain_operator_homes`
/// before this check runs, so the data-kind lookup is the whole residual
/// rule: trait and builtin owners, like unresolvable ones, stay invalid
/// symbols compared by their authored spelling.
fn operator_token_binding<'program>(
    program: &'program SymbolResolvedTrees,
    operator: &'program symbol_resolved_trees::operator::OperatorDefinition,
    domain: Option<&'program symbol_resolved_trees::domain::DomainDefinition>,
    spelling: OperatorSpelling,
) -> TokenBinding<'program> {
    let members = program.operator_path_members(operator.name);
    let leaf = members.last();
    let name = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let name_span = members.first().map_or_else(SourceSpan::default, |first| {
        let first = first.source_span();
        let last = leaf.expect("nonempty members").source_span();
        SourceSpan::new(
            first.source_id,
            source::Span::new(first.span.start, last.span.end),
        )
    });
    let (owner_spelling, owner) = match domain {
        Some(domain) => (
            domain.name.as_str().to_owned(),
            BindingOwner::Domain {
                domain: domain.symbol,
                carrier: carrier_symbol(program, &domain.target_type),
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
                let owner_spelling = owner_members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
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
        name,
        name_span,
        owner_spelling,
        spelling,
        owner,
        operand_shape: operand_shape(
            program,
            program.state_parameters(operator.parameters),
            &own_binders(program, operator.type_parameters),
        ),
    }
}

/// The declaration's own generic binders, mapped to their telescope
/// position, so `operator + Vec2::f<T>` and `operator + Vec2::g<U>` render
/// one operand shape. Binders owned by another declaration — the attached
/// data's `T` inside `Vec2<T>` — are not in this map and keep their symbol
/// identity, so `Vec2<T>` and `Vec2<U>` under one owner stay distinct.
fn own_binders(
    program: &SymbolResolvedTrees,
    type_parameters: HandleSpan<TypeParameter>,
) -> HashMap<SymbolHandle, usize> {
    program
        .data_type_parameters(type_parameters)
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, ordinal))
        .collect()
}

/// Whether `type_reference` names the declaration `home` anywhere in its
/// structure: directly, behind a reference, as a generic base, as the
/// constrained carrier, or as an array or slice element.
fn names_symbol(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    home: SymbolHandle,
) -> bool {
    match type_reference {
        TypeReference::Named { symbol, .. } => *symbol == home,
        TypeReference::SelfType { symbol } => {
            // `Self` inside an attached machine's signature resolves to the
            // machine's own symbol; the operand participates as the attached
            // data, so `&self` on `machine [] Buffer::index` names `Buffer`.
            *symbol == home
                || program.machines.iter().any(|machine| {
                    machine.symbol == *symbol && machine.attached_data_symbol == home
                })
        }
        TypeReference::Generic(generic) => {
            generic.base_symbol == home
                || program
                    .child_type_references(generic.arguments)
                    .iter()
                    .any(|argument| names_symbol(program, argument, home))
        }
        TypeReference::DynamicTrait { symbol, .. } => *symbol == home,
        TypeReference::Reference(reference) => names_symbol(
            program,
            program.child_type_reference(reference.referee),
            home,
        ),
        TypeReference::Constrained(constrained) => names_symbol(
            program,
            program.child_type_reference(constrained.base_type),
            home,
        ),
        TypeReference::FixedArray(fixed_array) => names_symbol(
            program,
            program.child_type_reference(fixed_array.element_type),
            home,
        ),
        TypeReference::Slice(slice) => names_symbol(
            program,
            program.child_type_reference(slice.element_type),
            home,
        ),
        TypeReference::ConstExpression(_) | TypeReference::Unit => false,
    }
}

/// Whether `type_reference` carries a domain constraint spelled as `domain_name`
/// or its leaf (`i32 in Degrees` selects `i32::Degrees`), behind references
/// and elements. Constraints are still names at this stage; the typed stage
/// settles their symbols.
fn qualified_by_domain(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    domain_name: &str,
) -> bool {
    let leaf = domain_name.rsplit("::").next().unwrap_or(domain_name);
    match type_reference {
        TypeReference::Constrained(constrained) => {
            program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
                .iter()
                .any(|constraint| {
                    matches!(
                        constraint,
                        symbol_resolved_trees::types::TypeConstraint::Domain(domain)
                            if domain.name.as_str() == domain_name || domain.name.as_str() == leaf
                    )
                })
                || qualified_by_domain(
                    program,
                    program.child_type_reference(constrained.base_type),
                    domain_name,
                )
        }
        TypeReference::Reference(reference) => qualified_by_domain(
            program,
            program.child_type_reference(reference.referee),
            domain_name,
        ),
        TypeReference::FixedArray(fixed_array) => qualified_by_domain(
            program,
            program.child_type_reference(fixed_array.element_type),
            domain_name,
        ),
        TypeReference::Slice(slice) => qualified_by_domain(
            program,
            program.child_type_reference(slice.element_type),
            domain_name,
        ),
        TypeReference::Named { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Generic(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::ConstExpression(_)
        | TypeReference::Unit => false,
    }
}

/// Whether `type_reference` names any authored declaration or a declared
/// domain constraint. Builtin primitives resolve to `BuiltinType` symbols and
/// generic binders to `TypeParameter` symbols; neither is a declaration that
/// can own a family, so a telescope of only those has no home.
fn names_declaration(program: &SymbolResolvedTrees, type_reference: &TypeReference) -> bool {
    let is_declaration = |symbol: &SymbolHandle| {
        symbol.is_valid()
            && matches!(
                program.symbols.get(*symbol).kind,
                SymbolKind::Data | SymbolKind::Domain | SymbolKind::Trait
            )
    };
    match type_reference {
        TypeReference::Named { symbol, .. }
        | TypeReference::SelfType { symbol }
        | TypeReference::DynamicTrait { symbol, .. } => is_declaration(symbol),
        TypeReference::Generic(generic) => {
            is_declaration(&generic.base_symbol)
                || program
                    .child_type_references(generic.arguments)
                    .iter()
                    .any(|argument| names_declaration(program, argument))
        }
        TypeReference::Reference(reference) => {
            names_declaration(program, program.child_type_reference(reference.referee))
        }
        TypeReference::Constrained(constrained) => {
            program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
                .iter()
                .any(|constraint| {
                    matches!(
                        constraint,
                        symbol_resolved_trees::types::TypeConstraint::Domain(_)
                    )
                })
                || names_declaration(program, program.child_type_reference(constrained.base_type))
        }
        TypeReference::FixedArray(fixed_array) => names_declaration(
            program,
            program.child_type_reference(fixed_array.element_type),
        ),
        TypeReference::Slice(slice) => {
            names_declaration(program, program.child_type_reference(slice.element_type))
        }
        TypeReference::ConstExpression(_) | TypeReference::Unit => false,
    }
}

/// The declaration set a direct binding is validated against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingOwner {
    /// `machine + Vec2::add(...)`: the exact attached data declaration.
    AttachedData(SymbolHandle),
    /// `machine + Quantity::Additive::add(...)`: the attached domain, whose
    /// carrier type is the binding's semantic home.
    Domain {
        domain: SymbolHandle,
        carrier: SymbolHandle,
    },
    /// A free machine: its declaring module (invalid for root scope).
    Module(SymbolHandle),
}

fn binding_owner(program: &SymbolResolvedTrees, machine: &Machine) -> BindingOwner {
    if machine.attached_data.is_none() {
        return BindingOwner::Module(program.symbols.symbol_module(machine.symbol));
    }
    if let Some(domain) = attached_domain(program, machine) {
        return BindingOwner::Domain {
            domain: domain.symbol,
            carrier: carrier_symbol(program, &domain.target_type),
        };
    }
    BindingOwner::AttachedData(machine.attached_data_symbol)
}

/// The domain a machine's attached path names, if any. Attachment symbols are
/// assigned against data declarations only, so a domain-attached machine
/// carries an invalid attachment symbol and is matched by its exact declared
/// path; a path naming more than one domain is no home.
fn attached_domain<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> Option<&'program symbol_resolved_trees::domain::DomainDefinition> {
    if machine.attached_data_symbol.is_valid() {
        return None;
    }
    let attached = machine.attached_data.as_ref()?.as_str();
    let mut matches = program
        .domain_definitions
        .iter()
        .filter(|domain| domain.name.as_str() == attached);
    let domain = matches.next()?;
    matches.next().is_none().then_some(domain)
}

/// The declaration a domain classifies: the named base of its target type,
/// behind references and generic applications.
fn carrier_symbol(program: &SymbolResolvedTrees, type_reference: &TypeReference) -> SymbolHandle {
    match type_reference {
        TypeReference::Named { symbol, .. } | TypeReference::SelfType { symbol } => *symbol,
        TypeReference::Generic(generic) => generic.base_symbol,
        TypeReference::Reference(reference) => {
            carrier_symbol(program, program.child_type_reference(reference.referee))
        }
        TypeReference::Constrained(constrained) => {
            carrier_symbol(program, program.child_type_reference(constrained.base_type))
        }
        TypeReference::DynamicTrait { .. }
        | TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::Unit => SymbolHandle::invalid(),
    }
}

/// Give every domain that owns a token-bearing machine its denotation role,
/// as `normalize_domain_operator_homes` does for domain-homed `operator`
/// declarations. Runs once attached symbols are assigned and before the
/// selections that classify predicate-only domains read the roles.
pub(crate) fn mark_token_bound_domain_homes(program: &mut SymbolResolvedTrees) {
    let homes = program
        .machines
        .iter()
        .filter(|machine| machine.spelling.is_some())
        .filter_map(|machine| attached_domain(program, machine).map(|domain| domain.symbol))
        .collect::<Vec<_>>();
    if homes.is_empty() {
        return;
    }
    program.domain_definitions.for_each_mut(|domain| {
        if homes.contains(&domain.symbol) {
            domain.semantic_roles.denotation_dimension = Some(domain.semantic_id);
        }
    });
}

/// The complete operand telescope rendered by resolved identity: symbols
/// where resolution assigned them, authored spelling only for the remaining
/// unsymbolled leaves, and the declaration's own generic binders as
/// telescope-position ordinals. Reference lifetimes are borrow-region tags
/// and never distinguish operand shapes.
fn operand_shape(
    program: &SymbolResolvedTrees,
    parameters: &[StateParameter],
    binders: &HashMap<SymbolHandle, usize>,
) -> String {
    parameters
        .iter()
        .map(|parameter| parameter_shape(program, binders, parameter))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parameter_shape(
    program: &SymbolResolvedTrees,
    binders: &HashMap<SymbolHandle, usize>,
    parameter: &StateParameter,
) -> String {
    let mut shape = String::new();
    if parameter.is_self {
        shape.push_str("self ");
    }
    if parameter.is_mutable {
        shape.push_str("mut ");
    }
    if parameter.is_const {
        shape.push_str("const ");
    }
    shape.push_str(&type_shape(program, binders, &parameter.type_reference));
    shape
}

/// Render `symbol` as its binder ordinal when the declaring signature owns
/// it, else by resolved identity or authored spelling.
fn binder_or_symbol(
    binders: &HashMap<SymbolHandle, usize>,
    symbol: SymbolHandle,
    spelling: &str,
) -> String {
    if let Some(ordinal) = binders.get(&symbol) {
        return format!("binder{ordinal}");
    }
    symbol_or_spelling(symbol, spelling)
}

fn type_shape(
    program: &SymbolResolvedTrees,
    binders: &HashMap<SymbolHandle, usize>,
    type_reference: &TypeReference,
) -> String {
    match type_reference {
        TypeReference::Reference(reference) => {
            let access = match reference.access {
                ReferenceAccess::Shared => "&",
                ReferenceAccess::Mutable => "&mut ",
                ReferenceAccess::WriteOnly => "&write ",
            };
            format!(
                "{access}{}",
                type_shape(
                    program,
                    binders,
                    program.child_type_reference(reference.referee)
                )
            )
        }
        TypeReference::Constrained(constrained) => {
            // An indexed domain application is part of the shape: `Quantity<
            // Units::METER>` and `Quantity<Units::KILOMETER>` are distinct
            // operand shapes for one token under one domain owner.
            let constraints = program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
                .iter()
                .map(|constraint| match constraint {
                    symbol_resolved_trees::types::TypeConstraint::Domain(domain)
                        if !domain.arguments.is_empty() =>
                    {
                        let arguments = program
                            .child_type_references(domain.arguments)
                            .iter()
                            .map(|argument| type_shape(program, binders, argument))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("{}<{arguments}>", domain.name.as_str())
                    }
                    _ => constraint.display_name(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}[{constraints}]",
                type_shape(
                    program,
                    binders,
                    program.child_type_reference(constrained.base_type)
                )
            )
        }
        TypeReference::FixedArray(fixed_array) => format!(
            "[{}; {}]",
            type_shape(
                program,
                binders,
                program.child_type_reference(fixed_array.element_type)
            ),
            fixed_array.length
        ),
        TypeReference::Slice(slice) => format!(
            "[{}]",
            type_shape(
                program,
                binders,
                program.child_type_reference(slice.element_type)
            )
        ),
        TypeReference::Generic(generic) => {
            let arguments = program
                .child_type_references(generic.arguments)
                .iter()
                .map(|argument| type_shape(program, binders, argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}<{arguments}>",
                binder_or_symbol(binders, generic.base_symbol, generic.base_name.as_str())
            )
        }
        TypeReference::ConstExpression(expression) => format!(
            "const({})",
            program.tables.bodies.expressions.display_name(*expression)
        ),
        TypeReference::DynamicTrait {
            symbol,
            name,
            conformance,
            ..
        } => match conformance {
            Some(conformance) => format!(
                "dyn {} via #{}",
                binder_or_symbol(binders, *symbol, name.as_str()),
                conformance.arena_index()
            ),
            None => format!("dyn {}", binder_or_symbol(binders, *symbol, name.as_str())),
        },
        TypeReference::Named { symbol, name } => binder_or_symbol(binders, *symbol, name.as_str()),
        TypeReference::SelfType { symbol } => {
            // `Self` inside an attached machine's signature resolves to that
            // machine's own symbol. The operand shape is the attached data's,
            // so `&self` bindings on one owner collide as duplicate shapes
            // instead of each spelling their own machine symbol.
            let attached = program
                .machines
                .iter()
                .find(|machine| machine.symbol == *symbol)
                .filter(|machine| machine.attached_data_symbol.is_valid());
            attached.map_or_else(
                || symbol_or_spelling(*symbol, "Self"),
                |machine| {
                    let name = machine
                        .attached_data
                        .as_ref()
                        .map_or("Self", |name| name.as_str());
                    symbol_or_spelling(machine.attached_data_symbol, name)
                },
            )
        }
        TypeReference::Unit => "()".to_owned(),
    }
}

fn symbol_or_spelling(symbol: SymbolHandle, spelling: &str) -> String {
    if symbol.is_valid() {
        format!("{spelling}#{}", symbol.arena_index())
    } else {
        spelling.to_owned()
    }
}

#[cfg(test)]
mod tests;
