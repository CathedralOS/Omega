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

use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::machine::Machine;
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolKind};
use syntax_trees::operator_spelling::OperatorSpelling;

/// Reject every direct machine whose operand tuple omits its semantic home,
/// then every one whose fixed token, owner, and normalized operand shape
/// repeat an earlier declaration's binding. Each rejection reports at its own
/// declaration; a duplicate names the binding it repeats.
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
            machine,
            spelling,
            owner: binding_owner(program, machine),
            operand_shape: operand_shape(program, machine),
        };
        if let Some(diagnostic) = binding.missing_semantic_home(program) {
            diagnostics.push(diagnostic);
            continue;
        }
        if let Some(earlier) = bindings.iter().find(|earlier| earlier.repeats(&binding)) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "`{}` binds the fixed operator token `{}` already bound by `{}` for the \
                     same owner and operand shape ({}); a second spelling needs a distinct \
                     operand shape or a separate owner",
                    machine.name,
                    spelling.symbol(),
                    earlier.machine.name,
                    binding.operand_shape
                ))
                .with_source_span(machine.name.source_span()),
            );
            continue;
        }
        bindings.push(binding);
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

struct TokenBinding<'program> {
    machine: &'program Machine,
    spelling: OperatorSpelling,
    owner: BindingOwner,
    operand_shape: String,
}

impl TokenBinding<'_> {
    fn repeats(&self, other: &Self) -> bool {
        self.spelling == other.spelling
            && self.owner == other.owner
            && self.operand_shape == other.operand_shape
    }

    /// The rejection for a binding whose operand tuple carries no semantic
    /// home, or `None` when some operand names it.
    fn missing_semantic_home(&self, program: &SymbolResolvedTrees) -> Option<Diagnostic> {
        let operand_types = entry_operand_types(program, self.machine);
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
                    self.machine.name,
                    self.spelling.symbol(),
                    self.machine
                        .attached_data
                        .as_ref()
                        .map_or("", |attached| attached.as_str()),
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
                    self.machine.name,
                    self.spelling.symbol(),
                    self.machine
                        .attached_data
                        .as_ref()
                        .map_or("", |attached| attached.as_str()),
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
                    self.machine.name,
                    self.spelling.symbol(),
                    self.operand_shape
                )
            }
        };
        Some(Diagnostic::error(message).with_source_span(self.machine.name.source_span()))
    }
}

/// The entry state's parameter types in telescope order.
fn entry_operand_types<'program>(
    program: &'program SymbolResolvedTrees,
    machine: &Machine,
) -> Vec<&'program TypeReference> {
    let Some(entry) = program.machine_state_handles(machine.states).first() else {
        return Vec::new();
    };
    let entry = program.machine_state(*entry);
    program
        .state_parameters(entry.parameters)
        .iter()
        .map(|parameter| &parameter.type_reference)
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

/// The complete operand telescope of the machine's entry state, rendered by
/// resolved identity: symbols where resolution assigned them, authored
/// spelling only for the remaining unsymbolled leaves. Reference lifetimes
/// are borrow-region tags and never distinguish operand shapes.
fn operand_shape(program: &SymbolResolvedTrees, machine: &Machine) -> String {
    let Some(entry) = program.machine_state_handles(machine.states).first() else {
        return String::new();
    };
    let entry = program.machine_state(*entry);
    program
        .state_parameters(entry.parameters)
        .iter()
        .map(|parameter| parameter_shape(program, parameter))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parameter_shape(program: &SymbolResolvedTrees, parameter: &StateParameter) -> String {
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
    shape.push_str(&type_shape(program, &parameter.type_reference));
    shape
}

fn type_shape(program: &SymbolResolvedTrees, type_reference: &TypeReference) -> String {
    match type_reference {
        TypeReference::Reference(reference) => {
            let access = match reference.access {
                ReferenceAccess::Shared => "&",
                ReferenceAccess::Mutable => "&mut ",
                ReferenceAccess::WriteOnly => "&write ",
            };
            format!(
                "{access}{}",
                type_shape(program, program.child_type_reference(reference.referee))
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
                            .map(|argument| type_shape(program, argument))
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
                type_shape(program, program.child_type_reference(constrained.base_type))
            )
        }
        TypeReference::FixedArray(fixed_array) => format!(
            "[{}; {}]",
            type_shape(
                program,
                program.child_type_reference(fixed_array.element_type)
            ),
            fixed_array.length
        ),
        TypeReference::Slice(slice) => format!(
            "[{}]",
            type_shape(program, program.child_type_reference(slice.element_type))
        ),
        TypeReference::Generic(generic) => {
            let arguments = program
                .child_type_references(generic.arguments)
                .iter()
                .map(|argument| type_shape(program, argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}<{arguments}>",
                symbol_or_spelling(generic.base_symbol, generic.base_name.as_str())
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
                symbol_or_spelling(*symbol, name.as_str()),
                conformance.arena_index()
            ),
            None => format!("dyn {}", symbol_or_spelling(*symbol, name.as_str())),
        },
        TypeReference::Named { symbol, name } => symbol_or_spelling(*symbol, name.as_str()),
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
