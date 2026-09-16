//! Owner-local duplicate checks for direct machine token bindings.
//!
//! One declaration binds at most one fixed token, and distinct operand shapes
//! may share a token; a second direct binding of the same token for the same
//! owner and operand shape rejects at that second declaration
//! ([expressions: machine token binding](../../../../../../../wiki/spec/language/expressions.md)).
//! The owner is the attached data declaration, or the declaring module for a
//! free machine. Shapes compare by resolved symbol identity, so the check runs
//! after every selection has settled and a same-leaf type from another module
//! never collides.

use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::machine::Machine;
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolHandle;
use syntax_trees::operator_spelling::OperatorSpelling;

/// Reject every direct machine whose fixed token, owner, and normalized operand
/// shape repeat an earlier declaration's binding. Each duplicate reports at
/// its own declaration and names the binding it repeats.
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
}

/// The declaration set a direct binding is validated against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingOwner {
    /// `machine + Vec2::add(...)`: the exact attached data declaration.
    AttachedData(SymbolHandle),
    /// A free machine: its declaring module (invalid for root scope).
    Module(SymbolHandle),
}

fn binding_owner(program: &SymbolResolvedTrees, machine: &Machine) -> BindingOwner {
    if machine.attached_data.is_some() {
        BindingOwner::AttachedData(machine.attached_data_symbol)
    } else {
        BindingOwner::Module(program.symbols.symbol_module(machine.symbol))
    }
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
            let constraints = program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
                .iter()
                .map(|constraint| constraint.display_name())
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
        TypeReference::SelfType { symbol } => symbol_or_spelling(*symbol, "Self"),
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
