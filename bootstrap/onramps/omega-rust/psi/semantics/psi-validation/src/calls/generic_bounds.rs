use crate::places::declared_place_type;
use crate::properties::{
    declared_property_requirements, referenced_type_parameter, type_satisfies_declared_property,
};
use crate::symbols::TopLevelSymbols;
use crate::type_references::type_reference_label;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

/// FROZEN DECISION 13 residue -- machine-call monomorphization arguments.
/// A bracket bound on a callee type parameter (`machine copy_it<T [copy]>`)
/// must hold for the concrete type the call instantiates `T` with. There is
/// no explicit type-argument list at call sites today: instantiation is
/// positional inference, so each non-self parameter whose declared type names
/// a bounded callee type parameter (`x: &T`, `x: T`, `[T; N]`, constrained
/// forms) pins `T` to the matching argument's declared place type, and that
/// concrete type must satisfy every bound via the same structural check the
/// data-instantiation path uses (`type_satisfies_declared_property`). An
/// in-scope bounded parameter of the CALLER counts as carrying its bound, so
/// a generic caller may forward its own `U [copy]`.
///
/// FRONTIER (stands down silently, like the wire argument checks): arguments
/// the declared-place scope cannot type (call results, indexed elements,
/// literals, nested member chains), parameters whose type buries `T` inside a
/// generic (`Box<T>`) or slice (`&[T]`).
///
/// Both STATEMENT-position calls (via `validate_call_node`) and VALUE-position
/// calls (via `validate_value_position_calls` + `scan_expression_calls`) now
/// reach this function.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_machine_call_type_parameter_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    callee_machine: &Machine,
    callee_state: &State,
    target_name: &str,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    current_state: Option<&State>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A claim-free bodyless boundary declaration is a SYMBOL for contracts,
    // not an executable provider.  It has neither checked code nor a `via`
    // realization, so allowing an ordinary body call would turn "introduces
    // no fact" into a hidden runtime implementation hole.  Contract
    // expressions are not body call sites and remain free to name the symbol.
    let compiler_placed_accessor = callee_machine
        .attached_data
        .as_ref()
        .is_some_and(|attached| attached.as_str().starts_with("PlacedField<"));
    if callee_machine.supply_mode == psi_language_semantics::MachineSupplyMode::Boundary
        && !compiler_placed_accessor
        && !callee_machine.body_is_present
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless boundary symbol `{target_name}` has no executable realization; use it only in contracts, or satisfy a boundary requirement via an admitted provider"
        )));
    }

    let type_parameters = program.machine_type_parameters(callee_machine);
    if type_parameters.is_empty() {
        return;
    }

    let caller_type_parameters = program.machine_type_parameters(current_machine);

    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| !parameter.is_self),
    ) {
        let Some(type_parameter) =
            referenced_type_parameter(program, type_parameters, parameter.type_reference)
        else {
            continue;
        };
        let bounds = declared_property_requirements(&type_parameter.bounds);
        if bounds.is_empty() {
            continue;
        }
        let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
        let Some(argument_type) =
            declared_place_type(program, current_machine, current_state, *argument)
        else {
            continue;
        };
        for property in bounds {
            if type_satisfies_declared_property(
                program,
                symbols,
                caller_type_parameters,
                argument_type,
                property,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "type parameter `{} [{}]` of machine `{target_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                type_parameter.name,
                bound_labels.join(", "),
                type_reference_label(program, argument_type)
            )));
        }
    }
}
