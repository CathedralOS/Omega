use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, top_level_symbol_by_kinds,
};
use crate::symbols::scope::MachineScope;
use crate::symbols::type_references::call_target_for_type_reference;

pub(in crate::symbols) fn resolve_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    has_receiver: bool,
    receiver_symbol: SymbolHandle,
    target: &psi_symbol_resolved_trees::name::DiagnosticName,
    child_type_references: &psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if has_receiver && receiver_symbol.is_valid() {
        if let Some(field_type_reference) = machine.field_type_reference(symbols, receiver_symbol) {
            let symbol = call_target_for_type_reference(
                symbols,
                child_type_references,
                field_type_reference,
                target.as_str(),
            );
            return symbol;
        }

        let receiver_kind = symbols.get(receiver_symbol).kind;
        if let Some(parameter) = parameters
            .iter()
            .find(|parameter| parameter.symbol == receiver_symbol)
        {
            let direct = call_target_for_type_reference(
                symbols,
                child_type_references,
                &parameter.type_reference,
                target.as_str(),
            );
            if direct.is_valid() {
                return direct;
            }
        }
        if matches!(receiver_kind, SymbolKind::BuiltinType) {
            return child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[SymbolKind::BuiltinFunction],
                target.as_str(),
            );
        }
        // A call through a DATA TYPE name (`Counter::from_v1(old, &mut current)`)
        // targets the machine attached to that data type: chapter 21's migration
        // machines take the old shape and the migration target as ordinary
        // parameters, so they are called through the type, not through a value.
        if matches!(receiver_kind, SymbolKind::Data) {
            let target_symbol = call_target_for_attached_data(
                symbols,
                symbols.name(receiver_symbol),
                target.as_str(),
            );
            if target_symbol.is_valid() {
                return target_symbol;
            }
        }
        // A domain-owned proof machine is called by its exact authored home
        // in contracts (`Granted::content(&value)`). Domains are not runtime
        // receivers; later validation admits only the owner-unique content
        // projection in proof position and rejects runtime use.
        if matches!(receiver_kind, SymbolKind::Domain) {
            let owner = symbols.name(receiver_symbol);
            let direct = call_target_for_attached_data(symbols, owner, target.as_str());
            if direct.is_valid() {
                return direct;
            }
            if let Some(leaf) = owner.rsplit("::").next() {
                let leaf = call_target_for_attached_data(symbols, leaf, target.as_str());
                if leaf.is_valid() {
                    return leaf;
                }
            }
        }
        if matches!(
            receiver_kind,
            SymbolKind::Machine | SymbolKind::Trait | SymbolKind::ConformanceParameter
        ) {
            if receiver_symbol == machine.symbol
                && let Some(attached_data) = machine.attached_data
            {
                let target_symbol =
                    call_target_for_attached_data(symbols, attached_data.as_str(), target.as_str());
                if target_symbol.is_valid() {
                    return target_symbol;
                }
            }

            return child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[SymbolKind::State],
                target.as_str(),
            );
        }
    }

    let machine_state = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        target.as_str(),
    );
    if machine_state.is_valid() {
        return machine_state;
    }

    // A receiverless call to a static machine parameter (`F(value)`) denotes
    // the authored callable requirement stored on that parameter. Its symbol
    // remains the signature identity until specialization substitutes a
    // concrete entry state.
    let machine_parameter = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::MachineParameter],
        target.as_str(),
    );
    if machine_parameter.is_valid() {
        return machine_parameter;
    }

    // A proposition-family parameter is callable-shaped only in a proof fact.
    // Typing owns that positional restriction; resolution preserves its
    // distinct target category here instead of confusing it with a machine.
    let proposition_parameter = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::PropositionParameter],
        target.as_str(),
    );
    if proposition_parameter.is_valid() {
        return proposition_parameter;
    }

    let builtin =
        top_level_symbol_by_kinds(symbols, &[SymbolKind::BuiltinFunction], target.as_str());
    if builtin.is_valid() {
        return builtin;
    }

    let proposition =
        top_level_symbol_by_kinds(symbols, &[SymbolKind::Proposition], target.as_str());
    if proposition.is_valid() {
        return proposition;
    }

    // A receiverless call to a FREE top-level machine (`machine compute(item:
    // &Item) -> i32 { ... }`, called as `compute(item)`): resolve to the free
    // machine's entry state so downstream passes (contract call obligations,
    // state-call planning) see a resolved target instead of an invalid symbol.
    resolve_free_machine_entry_state_symbol(symbols, target.as_str())
}

/// The entry-state symbol of the free top-level machine named `target`, or
/// invalid. A free machine's implicit entry state is named `entry` (the parser
/// generates the name); explicit `entry foo` states are matched by the call
/// target name first.
pub(in crate::symbols) fn resolve_free_machine_entry_state_symbol(
    symbols: &SymbolTable,
    target: &str,
) -> SymbolHandle {
    let machine_symbol = top_level_symbol_by_kinds(symbols, &[SymbolKind::Machine], target);
    if !machine_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    let named = child_symbol_by_kinds(symbols, machine_symbol, &[SymbolKind::State], target);
    if named.is_valid() {
        return named;
    }

    child_symbol_by_kinds(symbols, machine_symbol, &[SymbolKind::State], "entry")
}

/// Resolve a compile-time machine-symbol argument to its concrete entry state.
/// A free machine is spelled `work`; an attached machine is spelled
/// `Card::power`. The argument denotes no runtime value.
pub(in crate::symbols) fn resolve_static_machine_argument_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    path: &[psi_symbol_resolved_trees::name::DiagnosticName],
) -> SymbolHandle {
    let Some((target, owner)) = path.split_last() else {
        return SymbolHandle::invalid();
    };
    if owner.is_empty() {
        // A generic machine may forward one of its own compile-time machine
        // parameters (`map<F>(tail)`). Keep that lexical binding distinct
        // from a same-named free machine; specialization substitutes it with
        // the concrete selected entry before checked lowering.
        let parameter = child_symbol_by_kinds(
            symbols,
            machine_symbol,
            &[SymbolKind::MachineParameter],
            target.as_str(),
        );
        if parameter.is_valid() {
            return parameter;
        }
        let evidence_parameter = child_symbol_by_kinds(
            symbols,
            machine_symbol,
            &[SymbolKind::ConformanceParameter],
            target.as_str(),
        );
        if evidence_parameter.is_valid() {
            return evidence_parameter;
        }
        let conformance =
            top_level_symbol_by_kinds(symbols, &[SymbolKind::Conformance], target.as_str());
        if conformance.is_valid() {
            return conformance;
        }
        let concrete_type = top_level_symbol_by_kinds(
            symbols,
            &[SymbolKind::BuiltinType, SymbolKind::Data],
            target.as_str(),
        );
        if concrete_type.is_valid() {
            return concrete_type;
        }
        return resolve_free_machine_entry_state_symbol(symbols, target.as_str());
    }

    let owner = owner
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    call_target_for_attached_data(symbols, &owner, target.as_str())
}

/// Stamp one static argument and every declaration-owned nested application.
/// Nested application slots may be types, consts, or machines, so they use the
/// proof-static resolver; their final category is checked against the selected
/// declaration's own telescope after typed lowering.
pub(in crate::symbols) fn assign_static_argument_symbols(
    symbols: &SymbolTable,
    scope_symbol: SymbolHandle,
    argument: &mut psi_symbol_resolved_trees::expression::StaticMachineArgument,
    proof_static: bool,
) {
    if argument.evidence_projection.is_some() || argument.const_literal.is_some() {
        argument.symbol = SymbolHandle::invalid();
    } else {
        argument.symbol = if proof_static {
            resolve_proposition_binder_argument_symbol(symbols, scope_symbol, &argument.path)
        } else {
            resolve_static_machine_argument_symbol(symbols, scope_symbol, &argument.path)
        };
    }
    if let Some(application) = &mut argument.application {
        for nested in &mut application.arguments {
            assign_static_argument_symbols(symbols, scope_symbol, nested, true);
        }
    }
}

/// Resolve one `Build::select_provider` path as an exact declaration identity.
/// The marker's two static arguments are type paths, not executable machine
/// selections; their kind is checked by build harvesting after typed lowering.
pub(in crate::symbols) fn assign_provider_selection_argument_symbol(
    symbols: &SymbolTable,
    argument: &mut psi_symbol_resolved_trees::expression::StaticMachineArgument,
) {
    if argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
        || argument.application.is_some()
    {
        argument.symbol = SymbolHandle::invalid();
        return;
    }
    let rendered = argument
        .path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let exact = top_level_symbol_by_kinds(
        symbols,
        &[SymbolKind::Trait, SymbolKind::Data, SymbolKind::BuiltinType],
        &rendered,
    );
    argument.symbol = if exact.is_valid() {
        exact
    } else {
        symbols
            .find_descendant_by_path(
                symbols.root(),
                argument.path.iter().map(|member| member.as_str()),
            )
            .filter(|symbol| {
                matches!(
                    symbols.get(*symbol).kind,
                    SymbolKind::Trait | SymbolKind::Data | SymbolKind::BuiltinType
                )
            })
            .unwrap_or_else(SymbolHandle::invalid)
    };
}

/// Resolve a proof-static proposition-family binder argument. Unlike an
/// executable call's machine selection, proposition arguments may name a
/// lexical type/const binder, a concrete type, or a machine identity. The
/// proposition declaration's typed telescope performs the final category
/// check.
pub(in crate::symbols) fn resolve_proposition_binder_argument_symbol(
    symbols: &SymbolTable,
    scope_symbol: SymbolHandle,
    path: &[psi_symbol_resolved_trees::name::DiagnosticName],
) -> SymbolHandle {
    let [target] = path else {
        return resolve_static_machine_argument_symbol(symbols, scope_symbol, path);
    };
    let lexical = child_symbol_by_kinds(
        symbols,
        scope_symbol,
        &[
            SymbolKind::TypeParameter,
            SymbolKind::MachineParameter,
            SymbolKind::PropositionMachineParameter,
        ],
        target.as_str(),
    );
    if lexical.is_valid() {
        return lexical;
    }
    let concrete_type = top_level_symbol_by_kinds(
        symbols,
        &[SymbolKind::BuiltinType, SymbolKind::Data],
        target.as_str(),
    );
    if concrete_type.is_valid() {
        return concrete_type;
    }
    resolve_static_machine_argument_symbol(symbols, scope_symbol, path)
}
