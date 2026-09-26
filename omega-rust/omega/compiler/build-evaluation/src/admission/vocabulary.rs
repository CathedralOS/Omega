//! Exact toolchain build vocabulary: canonical build machine and prelude facets.

use std::path::Path;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

const BUILD_MACHINE: &str = "build";

/// Whether a machine is the program's canonical free build machine, declared
/// in the exact companion `build.omg` selected by project
/// discovery. Typed symbols retain authored source identity, so neither a
/// filename scan nor a machine-name handoff is authority.
/// A wrong-arity build machine still refuses at evaluation with the arity
/// error (pinned by fail/build/build_machine_wrong_arity).
pub fn is_build_machine(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    build_source_id: Option<source::SourceId>,
) -> bool {
    if machine.name.as_str() != BUILD_MACHINE {
        return false;
    }
    let Some(build_source_id) = build_source_id else {
        return false;
    };
    typed
        .symbols
        .symbol_source_span(machine.symbol)
        .is_some_and(|span| span.source_id == build_source_id)
}

pub(crate) fn has_exact_toolchain_build_facet(typed: &TypedTrees, name: &str) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.name.as_str() == name
            && typed
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
    })
}

fn is_exact_toolchain_build_filesystem_facet_call(
    typed: &TypedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> bool {
    typed.machines().iter().any(|machine| {
        if machine.symbol != target_machine
            || !machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| matches!(attached.as_str(), "BuildSource" | "BuildOutput"))
            || !typed
                .symbols
                .symbol_source_span(machine.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
        {
            return false;
        }
        typed.machine_states(machine).iter().any(|state| {
            state.symbol == target_state
                && matches!(
                    (
                        machine.attached_data.as_ref().map(|name| name.as_str()),
                        state.name.as_str(),
                    ),
                    (Some("BuildSource"), "open" | "read" | "close")
                        | (
                            Some("BuildOutput"),
                            "create" | "write" | "close" | "include_source"
                        )
                )
        })
    })
}

pub(crate) fn build_reaches_filesystem_facet(
    typed: &TypedTrees,
    operational: &flow_effects::OperationalPlan,
    root: SymbolHandle,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some(machine) = operational
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
        else {
            continue;
        };
        for state in operational.states.span_or_empty(machine.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                // Attached calls through the compiler-created private facet
                // value are intentionally unresolved at this typed prepass;
                // the evaluator below still admits only the exact prelude
                // declaration and private activation marker. A same-named
                // ordinary call can at most provision an unused sponsor: it
                // cannot obtain either rooted facet value.
                if is_exact_toolchain_build_filesystem_facet_call(
                    typed,
                    call.target_machine_symbol,
                    call.target_state_symbol,
                ) || (!call.target_state_symbol.is_valid()
                    && matches!(
                        call.target_name.as_str(),
                        "open" | "read" | "close" | "create" | "write" | "include_source"
                    ))
                {
                    return true;
                }
                if call.target_machine_symbol.is_valid() {
                    pending.push(call.target_machine_symbol);
                }
            }
        }
    }
    false
}

pub(crate) fn is_exact_toolchain_build_prelude_data(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    expected_name: &str,
) -> bool {
    typed.data_definitions().iter().any(|definition| {
        definition.symbol == symbol
            && definition.name.as_str() == expected_name
            && typed
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == Path::new("<build-prelude>")
                })
    })
}

fn named_data_symbol(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match typed.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            named_data_symbol(typed, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            named_data_symbol(typed, *base_type)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        _ => None,
    }
}

/// Whether the build machine's selected `Build` declares the ordinary
/// `identifier` field. The toolchain prelude declares it; an authored `Build`
/// opts in by declaring the same member.
pub(crate) fn build_machine_declares_identifier_field(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| typed.state_parameters(state).iter())
        .filter_map(|parameter| named_data_symbol(typed, parameter.type_reference))
        .filter_map(|symbol| {
            typed
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == symbol)
        })
        .filter(|definition| definition.name.as_str() == "Build")
        .flat_map(|definition| typed.data_members(definition).iter())
        .any(|member| {
            matches!(
                member,
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == "identifier"
            )
        })
}
