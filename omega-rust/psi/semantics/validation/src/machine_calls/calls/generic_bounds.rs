use crate::declarations::symbols::TopLevelSymbols;
use crate::proof_contracts::properties::{
    declared_property_requirements, referenced_type_parameter, type_satisfies_declared_property,
};
use crate::value_custody::places::declared_place_type;
use crate::value_custody::type_references::type_reference_label;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

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
    self_is_argument: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A claim-free bodyless boundary declaration is a SYMBOL for contracts,
    // not an executable provider.  It has neither checked code nor a `via`
    // realization, so allowing an ordinary body call would turn "introduces
    // no fact" into a hidden runtime implementation hole.  Contract
    // expressions are not body call sites and remain free to name the symbol.
    report_bodyless_boundary_symbol_call(
        program,
        callee_machine,
        callee_state,
        target_name,
        diagnostics,
    );
    validate_type_parameter_instantiation_bounds(
        program,
        symbols,
        callee_machine,
        callee_state,
        target_name,
        arguments,
        current_machine,
        current_state,
        self_is_argument,
        diagnostics,
    );
}

/// Whether a top-level `boundary requirement` may be called directly: public,
/// free of static generic binders, and at most an owned `self`, shared
/// `&self`, or mutable `&mut self` receiver. A receiver-free requirement is
/// called `Owner::name(...)`; a `self`/`&self`/`&mut self` requirement is
/// called through a member receiver `place.name(...)`. An erased lifetime
/// telescope (`Owner::name<'a>(...)`) is admitted: it carries no static
/// application arguments, so the call executes through the same selected
/// provider row a nongeneric requirement uses. A `&self`/`&mut self`
/// borrows the receiver place for the call, so the member call settles
/// through the same forwarding row with `&place`/`&mut place` as the
/// adapter's leading argument; the ordinary receiver-borrow checks already
/// demanded the place be readable or writable for the declared access.
/// Such a call executes
/// only through the selected provider row that selected-dispatch settles
/// after provider planning, which rejects a called requirement with no
/// selected provider; a private or generic requirement keeps the symbol
/// fence. A write-only or qualified `self` receiver (`&write self`,
/// `self in Pending`) keeps it too: receiver custody and obligation
/// transfer are a separate settlement shape.
fn is_directly_callable_top_level_requirement(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
) -> bool {
    callee_machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        && callee_machine.is_public
        && program.machine_type_parameters(callee_machine).is_empty()
        && program.machine_states(callee_machine).len() == 1
        && program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| parameter.is_self)
            .all(|parameter| {
                match program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                {
                    typed_trees::types::TypeReferenceNode::Named { .. } => true,
                    typed_trees::types::TypeReferenceNode::Reference {
                        referee, access, ..
                    } => {
                        matches!(
                            *access,
                            language_core::ReferenceAccess::Shared
                                | language_core::ReferenceAccess::Mutable
                        ) && matches!(
                            program.type_reference_table.type_reference(*referee),
                            typed_trees::types::TypeReferenceNode::Named { .. }
                        )
                    }
                    _ => false,
                }
            })
}

fn report_bodyless_boundary_symbol_call(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let compiler_placed_accessor = callee_machine
        .attached_data
        .as_ref()
        .is_some_and(|attached| attached.as_str().starts_with("PlacedField<"));
    if matches!(
        callee_machine.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::TopLevelRequirement
    ) && !compiler_placed_accessor
        && !callee_machine.body_is_present
        && !is_directly_callable_top_level_requirement(program, callee_machine, callee_state)
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless boundary symbol `{target_name}` has no executable realization; use it only in contracts, or satisfy a boundary requirement via an admitted provider"
        )));
    }
}

/// The instantiation-bound half of `validate_machine_call_type_parameter_bounds`
/// WITHOUT the execution fence. The resolved-target receiver rung adds
/// result-use, arity, and bound checks to calls the name ladder missed; it does
/// not re-decide whether the selected symbol may execute. Resolution admitted
/// those targets before validation ran (a provider closure invokes its own
/// boundary leaf directly, e.g. `ConsoleNativeProvider::write_byte(byte)`), and
/// receiver-place legality stays with the downstream dispatch blockers -- the
/// fence keeps its existing named-rung owners only.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_resolved_target_type_parameter_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    callee_machine: &Machine,
    callee_state: &State,
    target_name: &str,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    current_state: Option<&State>,
    self_is_argument: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_type_parameter_instantiation_bounds(
        program,
        symbols,
        callee_machine,
        callee_state,
        target_name,
        arguments,
        current_machine,
        current_state,
        self_is_argument,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_type_parameter_instantiation_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    callee_machine: &Machine,
    callee_state: &State,
    target_name: &str,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    current_state: Option<&State>,
    self_is_argument: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.machine_type_parameters(callee_machine);
    if type_parameters.is_empty() {
        return;
    }

    let caller_type_parameters = program.machine_type_parameters(current_machine);

    for (argument, parameter) in arguments.iter().zip(
        program
            .state_parameters(callee_state)
            .iter()
            .filter(|parameter| self_is_argument || !parameter.is_self),
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
