use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, diagnostic_path_source_span,
    top_level_symbol_by_kinds, top_level_symbol_for_source,
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
                target,
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
                target,
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
                target.source_span(),
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
            let direct = call_target_for_attached_data(
                symbols,
                owner,
                target.as_str(),
                target.source_span(),
            );
            if direct.is_valid() {
                return direct;
            }
            if let Some(leaf) = owner.rsplit("::").next() {
                let leaf = call_target_for_attached_data(
                    symbols,
                    leaf,
                    target.as_str(),
                    target.source_span(),
                );
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
                let target_symbol = call_target_for_attached_data(
                    symbols,
                    attached_data.as_str(),
                    target.as_str(),
                    target.source_span(),
                );
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

    let proposition = top_level_symbol_for_source(symbols, SymbolKind::Proposition, target);
    if proposition.is_valid() {
        return proposition;
    }

    // A receiverless call to a FREE top-level machine (`machine compute(item:
    // &Item) -> i32 { ... }`, called as `compute(item)`): resolve to the free
    // machine's entry state so downstream passes (contract call obligations,
    // state-call planning) see a resolved target instead of an invalid symbol.
    resolve_free_machine_entry_state_symbol(symbols, target)
}

/// The entry-state symbol of the free top-level machine named `target`, or
/// invalid. A free machine's implicit entry state is named `entry` (the parser
/// generates the name); explicit `entry foo` states are matched by the call
/// target name first.
pub(in crate::symbols) fn resolve_free_machine_entry_state_symbol(
    symbols: &SymbolTable,
    target: &psi_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    let machine_symbol = top_level_symbol_for_source(symbols, SymbolKind::Machine, target);
    if !machine_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    let named = child_symbol_by_kinds(
        symbols,
        machine_symbol,
        &[SymbolKind::State],
        target.as_str(),
    );
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
    let reference = diagnostic_path_source_span(path);
    if owner.is_empty() {
        // A generic machine may forward one of its own compile-time static
        // parameters. Keep lexical bindings distinct from same-named concrete
        // declarations; the selected callee telescope validates the category.
        let type_or_const_parameter = child_symbol_by_kinds(
            symbols,
            machine_symbol,
            &[SymbolKind::TypeParameter],
            target.as_str(),
        );
        if type_or_const_parameter.is_valid() {
            return type_or_const_parameter;
        }
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
        let conformance = symbols
            .find_top_level_by_name_and_kinds_from_source(
                target.as_str(),
                &[SymbolKind::Conformance],
                reference,
            )
            .unwrap_or_else(SymbolHandle::invalid);
        if conformance.is_valid() {
            return conformance;
        }
        let constant = symbols
            .find_top_level_by_name_and_kinds_from_source(
                target.as_str(),
                &[SymbolKind::Const],
                reference,
            )
            .unwrap_or_else(SymbolHandle::invalid);
        if constant.is_valid() {
            return constant;
        }
        let concrete_type = symbols
            .find_top_level_by_name_and_kinds_from_source(
                target.as_str(),
                &[SymbolKind::BuiltinType, SymbolKind::Data],
                reference,
            )
            .unwrap_or_else(SymbolHandle::invalid);
        if concrete_type.is_valid() {
            return concrete_type;
        }
        return resolve_free_machine_entry_state_symbol(symbols, target);
    }

    let owner = owner
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let rendered = format!("{owner}::{}", target.as_str());
    for kinds in [
        &[SymbolKind::Const][..],
        &[SymbolKind::Conformance][..],
        &[SymbolKind::BuiltinType, SymbolKind::Data][..],
    ] {
        let declaration = symbols
            .find_top_level_by_name_and_kinds_from_source(&rendered, kinds, reference)
            .unwrap_or_else(SymbolHandle::invalid);
        if declaration.is_valid() {
            return declaration;
        }
    }
    call_target_for_attached_data(symbols, &owner, target.as_str(), reference)
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
/// The marker's static arguments are exact declaration paths, not executable
/// machine selections. The first may denote one boundary trait, one explicit
/// top-level boundary requirement, or an entire same-path boundary-operator
/// family; build harvesting reifies and validates the exact declaration after
/// typed lowering.
pub(in crate::symbols) fn assign_provider_selection_argument_symbol(
    symbols: &SymbolTable,
    argument: &mut psi_symbol_resolved_trees::expression::StaticMachineArgument,
    allow_operator_family: bool,
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
    let kinds: &[SymbolKind] = if allow_operator_family {
        &[
            SymbolKind::Trait,
            SymbolKind::Machine,
            SymbolKind::Operator,
            SymbolKind::Data,
            SymbolKind::BuiltinType,
        ]
    } else {
        &[
            SymbolKind::Trait,
            SymbolKind::Operator,
            SymbolKind::Data,
            SymbolKind::BuiltinType,
        ]
    };
    let exact = argument.path.last().and_then(|name| {
        if allow_operator_family {
            symbols.find_top_level_declaration_or_operator_family_from_source(
                &rendered,
                kinds,
                name.source_span(),
            )
        } else {
            symbols.find_top_level_by_name_and_kinds_from_source(
                &rendered,
                kinds,
                name.source_span(),
            )
        }
    });
    let exact = exact.unwrap_or_else(SymbolHandle::invalid);
    argument.symbol = if exact.is_valid() {
        exact
    } else {
        let reference = argument.path.last().map(|name| name.source_span());
        symbols
            .find_descendant_by_path(
                symbols.root(),
                argument.path.iter().map(|member| member.as_str()),
            )
            .filter(|symbol| {
                matches!(
                    symbols.get(*symbol).kind,
                    SymbolKind::Trait
                        | SymbolKind::Operator
                        | SymbolKind::Data
                        | SymbolKind::BuiltinType
                )
            })
            .filter(|symbol| {
                reference.is_none_or(|reference| {
                    symbols.source_reference_can_see_symbol(reference, *symbol)
                })
            })
            .unwrap_or_else(SymbolHandle::invalid)
    };
}

/// Resolve `Build::select_representation` arguments as declaration identity,
/// not as executable static machines.
pub(in crate::symbols) fn assign_representation_selection_argument_symbol(
    symbols: &SymbolTable,
    argument: &mut psi_symbol_resolved_trees::expression::StaticMachineArgument,
    opaque_argument: bool,
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
    let kinds = if opaque_argument {
        &[SymbolKind::Data][..]
    } else {
        &[SymbolKind::Conformance][..]
    };
    let exact = argument.path.last().and_then(|name| {
        symbols.find_top_level_by_name_and_kinds_from_source(&rendered, kinds, name.source_span())
    });
    argument.symbol = exact.unwrap_or_else(|| {
        let reference = argument.path.last().map(|name| name.source_span());
        symbols
            .find_descendant_by_path(
                symbols.root(),
                argument.path.iter().map(|member| member.as_str()),
            )
            .filter(|symbol| kinds.contains(&symbols.get(*symbol).kind))
            .filter(|symbol| {
                reference.is_none_or(|reference| {
                    symbols.source_reference_can_see_symbol(reference, *symbol)
                })
            })
            .unwrap_or_else(SymbolHandle::invalid)
    });
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
    let concrete_type = symbols
        .find_top_level_by_name_and_kinds_from_source(
            target.as_str(),
            &[SymbolKind::BuiltinType, SymbolKind::Data],
            target.source_span(),
        )
        .unwrap_or_else(SymbolHandle::invalid);
    if concrete_type.is_valid() {
        return concrete_type;
    }
    resolve_static_machine_argument_symbol(symbols, scope_symbol, path)
}
