//! Calls select a source-visible declaration before the root builtin roster.
//! A builtin is the receiverless default, not authority attached to a spelling:
//! choosing it before an authored machine erased that machine's implementation
//! and contracts during intrinsic normalization. Keep ambiguity distinct from
//! absence so competing authored declarations cannot fall back to an intrinsic.
//! The same callable family roster serves executable expressions, contracts,
//! mathematical bodies and qualified namespaces. Selecting a mathematical
//! declaration retains its identity; the calculus owns application formation.

use language_semantics::declaration_selection::BuildOperation;
use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

use crate::symbols::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, diagnostic_path_source_span,
    top_level_symbol_by_kinds, top_level_symbol_for_source,
};
/// Callable declaration identity is independent of executable or proof use.
/// Formation later decides whether the selected declaration can satisfy the
/// demanded role; resolution must retain mathematical targets there as well.
pub(in crate::symbols) const SOURCE_CALL_KINDS: &[SymbolKind] = &[
    SymbolKind::Machine,
    SymbolKind::Operator,
    SymbolKind::Proposition,
    SymbolKind::MathematicalDefinition,
];

use crate::symbols::scope::MachineScope;
use crate::symbols::type_references::call_target_for_type_reference;

pub(in crate::symbols) fn resolve_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    has_receiver: bool,
    receiver_symbol: SymbolHandle,
    target: &symbol_resolved_trees::name::DiagnosticName,
    child_type_references: &arena::Arena<symbol_resolved_trees::types::TypeReference>,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if has_receiver && receiver_symbol.is_valid() {
        // Lexical local identity takes precedence over same-named fields,
        // free machines, and type homes. Missing local methods stay unresolved.
        if matches!(symbols.get(receiver_symbol).kind, SymbolKind::Local) {
            let Some(mut reference) =
                machine
                    .prior_statements
                    .iter()
                    .find_map(|statement| match statement {
                        symbol_resolved_trees::statement::Statement::LocalData(local)
                            if local.symbol == receiver_symbol =>
                        {
                            Some(&local.type_reference)
                        }
                        _ => None,
                    })
            else {
                return SymbolHandle::invalid();
            };
            // A whole collection is not an indexed element receiver. The
            // shared legacy query also serves indexed fields, so only pass a
            // local's declared nominal type through transparent shells here.
            loop {
                use symbol_resolved_trees::types::TypeReference;
                reference = match reference {
                    TypeReference::Reference(reference) => {
                        child_type_references.get(reference.referee)
                    }
                    TypeReference::Constrained(constrained) => {
                        child_type_references.get(constrained.base_type)
                    }
                    TypeReference::FixedArray(_) | TypeReference::Slice(_) => {
                        return SymbolHandle::invalid();
                    }
                    _ => break,
                };
            }
            return call_target_for_type_reference(
                machine,
                symbols,
                child_type_references,
                reference,
                target,
            );
        }
        if let Some(field_type_reference) = machine.field_type_reference(symbols, receiver_symbol) {
            let symbol = call_target_for_type_reference(
                machine,
                symbols,
                child_type_references,
                field_type_reference,
                target,
            );
            return symbol;
        }

        let receiver_kind = symbols.get(receiver_symbol).kind;
        if receiver_kind == SymbolKind::Module {
            // Module children retain their declaration kind: a same-named
            // machine, operator, or other member is the exact selected owner.
            // Only machines rewrite to their entry state; an operator call
            // keeps its declaration identity for named-operator validation.
            let callable =
                child_symbol_by_kinds(symbols, receiver_symbol, SOURCE_CALL_KINDS, target.as_str());
            match symbols.get(callable).kind {
                SymbolKind::Machine => {
                    return child_symbol_by_kinds(symbols, callable, &[SymbolKind::State], "entry");
                }
                SymbolKind::Operator
                | SymbolKind::Proposition
                | SymbolKind::MathematicalDefinition => {
                    return callable;
                }
                _ => return SymbolHandle::invalid(),
            }
        }
        if let Some(parameter) = parameters
            .iter()
            .find(|parameter| parameter.symbol == receiver_symbol)
        {
            let direct = call_target_for_type_reference(
                machine,
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
        // targets the machine attached to that data type. Ordinary migration
        // machines take the old shape and the migration target as ordinary
        // parameters, so they are called through the type, not through a value.
        if matches!(receiver_kind, SymbolKind::Data) {
            let target_symbol = machine.attached_call_target(symbols, receiver_symbol, target);
            if target_symbol.is_valid() {
                return target_symbol;
            }
            // The same qualified spelling may name a boundary operator homed
            // on the carrier (`F32::square_root`), which is a root operator
            // declaration rather than an attached machine. As with a module
            // receiver the call keeps the operator's own declaration identity
            // for typed named-call validation — but only when the qualified
            // name is one exact declaration. A family spelled by several
            // overloads stays unresolved so path-and-arity selection decides
            // it rather than this name-level lookup minting one member.
            if let Some(operator) = symbols
                .lookup_signature_free_top_level_from_source_matching(
                    &format!(
                        "{}::{}",
                        symbols.display_path(receiver_symbol, "::"),
                        target.as_str()
                    ),
                    &[SymbolKind::Operator],
                    target.source_span(),
                    |_| true,
                )
                .unique()
            {
                return operator;
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
        if receiver_kind == SymbolKind::Machine {
            if receiver_symbol == machine.symbol && machine.attached_data_symbol.is_valid() {
                let target_symbol =
                    machine.attached_call_target(symbols, machine.attached_data_symbol, target);
                if target_symbol.is_valid() {
                    return target_symbol;
                }
            }

            // A machine's internal states are jump destinations, not methods.
            // Only transition resolution may select those children.
            return SymbolHandle::invalid();
        }
        if matches!(
            receiver_kind,
            SymbolKind::Trait | SymbolKind::ConformanceParameter
        ) {
            return child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[SymbolKind::State],
                target.as_str(),
            );
        }
    }

    // Everything below selects a receiverless candidate: trait requirements,
    // static machine and proposition parameters, builtins and free machines.
    // A spelled receiver's target
    // belongs to the receiver's declaration instead; when the receiver block
    // above could not resolve it (for example a member receiver whose exact
    // field awaits typed-stage resolution), the call stays unresolved here
    // rather than minting a state owned by a different declaration.
    if has_receiver {
        return SymbolHandle::invalid();
    }

    // Trait requirements share the State symbol representation, but are
    // callable contracts, not a machine's private control-flow destinations.
    // Inside a trait, including a default body instantiated onto a carrier,
    // a requirement name takes precedence over every outer receiverless
    // candidate below.
    let requirement = machine
        .lexical_requirements
        .select(symbols, target.as_str());
    if requirement.is_valid() {
        return requirement;
    }

    // A receiverless call to a static machine parameter (`F(value)`) denotes
    // the authored callable requirement stored on that parameter. Its symbol
    // remains the signature identity until specialization substitutes a
    // concrete entry state.
    // A case owns payload subjects, but callable binders belong to its data.
    let generic_owner = if symbols.get(machine.symbol).kind == SymbolKind::Variant {
        symbols.get(machine.symbol).parent
    } else {
        machine.symbol
    };
    let machine_parameter = machine
        .type_parameters
        .iter()
        .find(|parameter| {
            parameter.name.as_str() == target.as_str()
                && matches!(
                    parameter.kind,
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
                )
        })
        .map(|parameter| parameter.symbol)
        .unwrap_or_else(|| {
            child_symbol_by_kinds(
                symbols,
                generic_owner,
                &[SymbolKind::MachineParameter],
                target.as_str(),
            )
        });
    if machine_parameter.is_valid() {
        return machine_parameter;
    }

    // A proposition-family parameter is callable-shaped only in a proof fact.
    // Typing owns that positional restriction; resolution preserves its
    // distinct target category here instead of confusing it with a machine.
    let proposition_parameter = child_symbol_by_kinds(
        symbols,
        generic_owner,
        &[SymbolKind::PropositionParameter],
        target.as_str(),
    );
    if proposition_parameter.is_valid() {
        return proposition_parameter;
    }

    resolve_free_call_target_symbol(symbols, target)
}

/// A visible authored callable shadows the outermost builtin. Ambiguity and
/// a machine with no entry retain absence rather than selecting another kind.
pub(in crate::symbols) fn resolve_free_call_target_symbol(
    symbols: &SymbolTable,
    target: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
        target.as_str(),
        SOURCE_CALL_KINDS,
        target.source_span(),
        |_| true,
    ) {
        SymbolLookup::Unique(declaration) => {
            if symbols.get(declaration).kind == SymbolKind::Machine {
                machine_entry_symbol(symbols, declaration, target.as_str())
            } else {
                declaration
            }
        }
        SymbolLookup::Ambiguous { .. } => SymbolHandle::invalid(),
        SymbolLookup::NotFound => {
            top_level_symbol_by_kinds(symbols, &[SymbolKind::BuiltinFunction], target.as_str())
        }
    }
}

/// The entry-state symbol of the free top-level machine named `target`, or
/// invalid. A free machine's implicit entry state is named `entry` (the parser
/// generates the name); explicit `entry foo` states are matched by the call
/// target name first.
pub(in crate::symbols) fn resolve_free_machine_entry_state_symbol(
    symbols: &SymbolTable,
    target: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    let machine_symbol = top_level_symbol_for_source(symbols, SymbolKind::Machine, target);
    if !machine_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    machine_entry_symbol(symbols, machine_symbol, target.as_str())
}

fn machine_entry_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    target: &str,
) -> SymbolHandle {
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
    path: &[symbol_resolved_trees::name::DiagnosticName],
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

    let owner = owner.joined_path();
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
    lexical_parameters: &[symbol_resolved_trees::data::TypeParameter],
    argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
    proof_static: bool,
) {
    if argument.type_reference.is_valid()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
    {
        argument.symbol = SymbolHandle::invalid();
    } else if let [name] = argument.path.as_ref()
        && let Some(parameter) = lexical_parameters
            .iter()
            .find(|parameter| parameter.name == *name)
    {
        // A realization uses its owner's telescope without owning another set
        // of symbol children. Resolve static operands in the same lexical
        // scope as its parameter/result types, before looking up declarations.
        // Machine-local shadowing is already settled by declaration_type_scope.
        argument.symbol = parameter.symbol;
    } else {
        argument.symbol = if proof_static {
            resolve_proposition_binder_argument_symbol(symbols, scope_symbol, &argument.path)
        } else {
            resolve_static_machine_argument_symbol(symbols, scope_symbol, &argument.path)
        };
    }
    if let Some(application) = &mut argument.application {
        for nested in &mut application.arguments {
            assign_static_argument_symbols(symbols, scope_symbol, lexical_parameters, nested, true);
        }
    }
}

/// Stamp one single-segment static argument naming a value-scope subject: a
/// state parameter or a `let` local of the enclosing state. Runtime-capable
/// `Value` binders accept such subjects as ordinary runtime arguments, so the
/// name resolves like an executable `Name` instead of remaining an unresolved
/// declaration selection. This runs only after the declaration resolver above
/// has failed: a same-named declaration keeps its static identity.
pub(in crate::symbols) fn assign_runtime_subject_argument_symbol(
    symbols: &SymbolTable,
    state_symbol: SymbolHandle,
    argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
) {
    if argument.type_reference.is_valid()
        || argument.symbol.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
        || !state_symbol.is_valid()
    {
        return;
    }
    let [name] = argument.path.as_ref() else {
        return;
    };
    argument.symbol = child_symbol_by_kinds(
        symbols,
        state_symbol,
        &[SymbolKind::Parameter, SymbolKind::Local],
        name.as_str(),
    );
}

/// The build operation whose static operands name declarations rather than
/// executable static machines, or `None` when a call's operands resolve as
/// ordinary static machine arguments.
///
/// `select_provider<Subject, Product>` and `exclude_service<Service>` name
/// exact declaration paths (see
/// [`assign_provider_selection_argument_symbol`]);
/// `select_representation<Type, Representation>` names declaration identity
/// with an opaque first operand (see
/// [`assign_representation_selection_argument_symbol`]). Provider selection
/// takes this route only while its target is unresolved: a declared machine
/// which happens to be spelled `select_provider` resolves as an ordinary
/// target and keeps ordinary operands. Service exclusion follows the same
/// rule, as does representation selection. The remaining build operations carry no
/// static operands and route nothing here. Assumed binding operands select exact
/// boundary requirement declarations, never executable generic arguments. Their
/// independently checked runtime installation owns compatibility and acceptance.
pub(in crate::symbols) fn build_operand_route(
    target: &str,
    target_symbol: SymbolHandle,
) -> Option<BuildOperation> {
    match BuildOperation::from_call_target(target)? {
        BuildOperation::ProviderSelection
        | BuildOperation::ServiceExclusion
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest
        | BuildOperation::RepresentationSelection
        | BuildOperation::AssumedBindingContract
            if target_symbol.is_valid() =>
        {
            None
        }
        operation @ (BuildOperation::ProviderSelection
        | BuildOperation::ServiceExclusion
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest
        | BuildOperation::RepresentationSelection
        | BuildOperation::AssumedBindingContract) => Some(operation),
        BuildOperation::OptimizationSelection
        | BuildOperation::OptimizationReportRequest
        | BuildOperation::IncludedSourceHandoff
        | BuildOperation::LogWriteLine => None,
    }
}

/// A bound selection argument (already carrying a type reference, evidence,
/// const, or application payload) takes no symbol; unbound ones render their
/// qualified path for name resolution.
fn selection_argument_rendered_path(
    argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
) -> Option<String> {
    if argument.type_reference.is_valid()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
        || argument.application.is_some()
    {
        argument.symbol = SymbolHandle::invalid();
        return None;
    }
    Some(argument.path.joined_path())
}

/// Fallback resolution through descendant paths for selection arguments: the
/// source reference still gates visibility of the found descendant.
fn descendant_selection_symbol(
    symbols: &SymbolTable,
    argument: &symbol_resolved_trees::expression::StaticMachineArgument,
    kinds: &[SymbolKind],
) -> SymbolHandle {
    let reference = argument.path.last().map(|name| name.source_span());
    symbols
        .find_descendant_by_path(
            symbols.root(),
            argument.path.iter().map(|member| member.as_str()),
        )
        .filter(|symbol| kinds.contains(&symbols.get(*symbol).kind))
        .filter(|symbol| {
            reference
                .is_none_or(|reference| symbols.source_reference_can_see_symbol(reference, *symbol))
        })
        .unwrap_or_else(SymbolHandle::invalid)
}

/// Resolve a provider-selection operand as an exact declaration, not an
/// executable static machine. The subject may name a boundary trait, top-level
/// requirement or operator family; the provider may name data or a named map.
/// Both lexical and qualified resolution preserve that declaration category.
/// Harvesting validates the resolved declaration after typed lowering.
pub(in crate::symbols) fn assign_provider_selection_argument_symbol(
    symbols: &SymbolTable,
    argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
    allow_operator_family: bool,
) {
    let Some(rendered) = selection_argument_rendered_path(argument) else {
        return;
    };
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
            SymbolKind::Conformance,
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
        // Qualified declarations obey the same role and visibility rules as
        // top-level operands; a named map is not a hidden receiver data type.
        descendant_selection_symbol(symbols, argument, kinds)
    };
}

/// A relationship names two exact requirement declarations, not callable bodies.
/// Requirement members are State symbols beneath their boundary trait. Product
/// visibility still gates both paths; later typed admission rejects other states.
pub(in crate::symbols) fn assign_assumed_binding_argument_symbol(
    symbols: &SymbolTable,
    argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
) {
    let Some(rendered) = selection_argument_rendered_path(argument) else {
        return;
    };
    let kinds = &[SymbolKind::State, SymbolKind::Machine];
    argument.symbol = argument
        .path
        .last()
        .and_then(|name| {
            symbols.find_top_level_by_name_and_kinds_from_source(
                &rendered,
                kinds,
                name.source_span(),
            )
        })
        .unwrap_or_else(|| descendant_selection_symbol(symbols, argument, kinds));
}

/// Resolve `Build::select_representation` arguments as declaration identity,
/// not as executable static machines.
pub(in crate::symbols) fn assign_representation_selection_argument_symbol(
    symbols: &SymbolTable,
    argument: &mut symbol_resolved_trees::expression::StaticMachineArgument,
    opaque_argument: bool,
) {
    let Some(rendered) = selection_argument_rendered_path(argument) else {
        return;
    };
    let kinds = if opaque_argument {
        &[SymbolKind::Data][..]
    } else {
        &[SymbolKind::Conformance][..]
    };
    let exact = argument.path.last().and_then(|name| {
        symbols.find_top_level_by_name_and_kinds_from_source(&rendered, kinds, name.source_span())
    });
    // A qualified operand (`producer::types::Token`) may name a declaration in
    // the product-scope checked instance of a dependency — the scope a build
    // occurrence cannot reach through `use`. Mirror the provider-selection
    // product bridge with the candidates this stage can enumerate.
    let exact = exact.or_else(|| {
        if argument.path.len() < 2 {
            return None;
        }
        let occurrence = argument.path.last().map(|name| name.source_span())?;
        let candidates = symbols
            .symbols()
            .nodes()
            .iter()
            .filter(|(handle, _)| kinds.contains(&symbols.get(*handle).kind))
            .map(|(handle, _)| (handle, true))
            .collect::<Vec<_>>();
        symbols.find_product_declaration_from_source(&rendered, occurrence, candidates)
    });
    argument.symbol =
        exact.unwrap_or_else(|| descendant_selection_symbol(symbols, argument, kinds));
}

/// Resolve a proof-static proposition-family binder argument. Unlike an
/// executable call's machine selection, proposition arguments may name a
/// lexical type/const binder, a concrete type, or a machine identity. The
/// proposition declaration's typed telescope performs the final category
/// check.
pub(in crate::symbols) fn resolve_proposition_binder_argument_symbol(
    symbols: &SymbolTable,
    scope_symbol: SymbolHandle,
    path: &[symbol_resolved_trees::name::DiagnosticName],
) -> SymbolHandle {
    let [target] = path else {
        return resolve_static_machine_argument_symbol(symbols, scope_symbol, path);
    };
    // A nested callable's proof operand can name a binder in any enclosing
    // telescope. Resolve nearest-first by lexical parentage, never by a
    // top-level spelling fallback that discards the declaration's scope.
    let mut lexical_scope = scope_symbol;
    while lexical_scope.is_valid() {
        let lexical = child_symbol_by_kinds(
            symbols,
            lexical_scope,
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
        lexical_scope = symbols.get(lexical_scope).parent;
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
