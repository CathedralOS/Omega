/*
    Symbol assignment for call, transition and static-namespace targets.
    A call selects a source-visible declaration before the builtin roster, and
    compile-time machine and value arguments are stamped at their exact slots.
*/

mod calls {
    /*
        Calls select a source-visible declaration before the root builtin roster.
        A builtin is the receiverless default, not authority attached to a spelling:
        choosing it before an authored machine erased that machine's implementation
        and contracts during intrinsic normalization. Keep ambiguity distinct from
        absence so competing authored declarations cannot fall back to an intrinsic.
        The same callable family roster serves executable expressions, contracts,
        mathematical bodies and qualified namespaces. Selecting a mathematical
        declaration retains its identity; the calculus owns application formation.
    */

    use language_semantics::declaration_selection::BuildOperation;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

    use crate::symbols::lookup::{
        call_target_for_attached_data, child_symbol_by_kinds, diagnostic_path_source_span,
        top_level_symbol_by_kinds, top_level_symbol_for_source,
    };
    /*
        Callable declaration identity is independent of executable or proof use.
        Formation later decides whether the selected declaration can satisfy the
        demanded role; resolution must retain mathematical targets there as well.
    */
    pub(in crate::symbols) const SOURCE_CALL_KINDS: &[SymbolKind] = &[
        SymbolKind::Machine,
        SymbolKind::Operator,
        SymbolKind::Proposition,
        SymbolKind::MathematicalDefinition,
    ];

    use crate::symbols::scope::MachineScope;
    use crate::symbols::type_references::call_target_for_type_reference;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::expression::StaticMachineArgument;
    use symbol_resolved_trees::statement::Statement;

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
            if matches!(symbols.get(receiver_symbol).kind, SymbolKind::Local) {
                let Some(mut reference) =
                    machine
                        .prior_statements
                        .iter()
                        .find_map(|statement| match statement {
                            Statement::LocalData(local) if local.symbol == receiver_symbol => {
                                Some(&local.type_reference)
                            }
                            _ => None,
                        })
                else {
                    return SymbolHandle::invalid();
                };
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
            if let Some(field_type_reference) =
                machine.field_type_reference(symbols, receiver_symbol)
            {
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
                /*
                    The same declaration the static pass selects. A child whose
                    stored name is only the leaf misses `machine kit::work`,
                    whose name is the qualified spelling and whose state is
                    `work`; asking that miss for `entry` was a third answer.
                */
                let qualified = format!(
                    "{}::{}",
                    symbols.display_path(receiver_symbol, "::"),
                    target.as_str()
                );
                let callable = symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &qualified,
                        SOURCE_CALL_KINDS,
                        target.source_span(),
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
                match symbols.get(callable).kind {
                    SymbolKind::Machine => {
                        return machine_entry_symbol(symbols, callable, target.as_str());
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
            if matches!(receiver_kind, SymbolKind::Data) {
                let target_symbol = machine.attached_call_target(symbols, receiver_symbol, target);
                if target_symbol.is_valid() {
                    return target_symbol;
                }
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

        if has_receiver {
            return SymbolHandle::invalid();
        }

        let requirement = machine
            .lexical_requirements
            .select(symbols, target.as_str());
        if requirement.is_valid() {
            return requirement;
        }

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
                    && matches!(parameter.kind, TypeParameterKind::Machine { .. })
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

        let proposition_parameter = child_symbol_by_kinds(
            symbols,
            generic_owner,
            &[SymbolKind::PropositionParameter],
            target.as_str(),
        );
        if proposition_parameter.is_valid() {
            return proposition_parameter;
        }

        match crate::symbols::lookup::lexical_declaration_lookup(
            symbols,
            machine.symbol,
            target.as_str(),
            &[SymbolKind::Machine],
        ) {
            SymbolLookup::Unique(declaration) => {
                machine_entry_symbol(symbols, declaration, target.as_str())
            }
            SymbolLookup::Ambiguous { .. } => SymbolHandle::invalid(),
            SymbolLookup::NotFound => resolve_free_call_target_symbol(symbols, target),
        }
    }

    /*
        A visible authored callable shadows the outermost builtin. Ambiguity and
        a machine with no entry retain absence rather than selecting another kind.
    */
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

    /*
        The entry-state symbol of the free top-level machine named `target`, or
        invalid. A free machine's implicit entry state is named `entry` (the parser
        generates the name); explicit `entry foo` states are matched by the call
        target name first.
    */
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

    pub(super) fn machine_entry_symbol(
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

    /*
        Resolve a compile-time machine-symbol argument to its concrete entry state.
        A free machine is spelled `work`; an attached machine is spelled
        `Card::power`. The argument denotes no runtime value.
    */
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
            for kind in [
                SymbolKind::TypeParameter,
                SymbolKind::MachineParameter,
                SymbolKind::ConformanceParameter,
            ] {
                let binding =
                    child_symbol_by_kinds(symbols, machine_symbol, &[kind], target.as_str());
                if binding.is_valid() {
                    return binding;
                }
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
            return match crate::symbols::lookup::lexical_declaration_lookup(
                symbols,
                machine_symbol,
                target.as_str(),
                &[SymbolKind::Machine],
            ) {
                SymbolLookup::Unique(declaration) => {
                    machine_entry_symbol(symbols, declaration, target.as_str())
                }
                SymbolLookup::Ambiguous { .. } => SymbolHandle::invalid(),
                SymbolLookup::NotFound => resolve_free_machine_entry_state_symbol(symbols, target),
            };
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
        symbols
            .find_top_level_by_name_and_kinds_from_source(
                &rendered,
                &[SymbolKind::Machine],
                reference,
            )
            .map_or_else(SymbolHandle::invalid, |declaration| {
                machine_entry_symbol(symbols, declaration, target.as_str())
            })
    }

    /*
        Stamp one static argument and every declaration-owned nested application.
        Nested application slots may be types, consts, or machines, so they use the
        proof-static resolver; their final category is checked against the selected
        declaration's own telescope after typed lowering.
    */
    pub(in crate::symbols) fn assign_static_argument_symbols(
        symbols: &SymbolTable,
        scope_symbol: SymbolHandle,
        lexical_parameters: &[symbol_resolved_trees::data::TypeParameter],
        argument: &mut StaticMachineArgument,
        proof_static: bool,
    ) {
        assign_static_argument_symbol(
            symbols,
            scope_symbol,
            lexical_parameters,
            argument,
            proof_static,
        );
        if let Some(application) = &mut argument.application {
            for nested in &mut application.arguments {
                assign_static_argument_symbols(
                    symbols,
                    scope_symbol,
                    lexical_parameters,
                    nested,
                    true,
                );
            }
        }
    }

    /// Bind one static operand in its exact lexical telescope. Context owners
    /// that also resolve structural type references walk their own typed operand
    /// graph; the ordinary recursive walker uses the same binding operation.
    pub(in crate::symbols) fn assign_static_argument_symbol(
        symbols: &SymbolTable,
        scope_symbol: SymbolHandle,
        lexical_parameters: &[symbol_resolved_trees::data::TypeParameter],
        argument: &mut StaticMachineArgument,
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
            argument.symbol = parameter.symbol;
        } else {
            argument.symbol = if proof_static {
                resolve_proposition_binder_argument_symbol(symbols, scope_symbol, &argument.path)
            } else {
                resolve_static_machine_argument_symbol(symbols, scope_symbol, &argument.path)
            };
        }
    }

    /*
        Stamp one single-segment static argument naming a value-scope subject: a
        state parameter or a `let` local of the enclosing state. Runtime-capable
        `Value` binders accept such subjects as ordinary runtime arguments, so the
        name resolves like an executable `Name` instead of remaining an unresolved
        declaration selection. This runs only after the declaration resolver above
        has failed: a same-named declaration keeps its static identity.
    */
    pub(in crate::symbols) fn assign_runtime_subject_argument_symbol(
        symbols: &SymbolTable,
        state_symbol: SymbolHandle,
        argument: &mut StaticMachineArgument,
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

    /*
        Whether a call target names a proof-position declaration: propositions,
        their binders, and mathematical definitions take proof-static operands
        rather than executable static-machine arguments.
    */
    pub(in crate::symbols) fn proof_static_call_target(
        symbols: &SymbolTable,
        target_symbol: SymbolHandle,
    ) -> bool {
        target_symbol.is_valid()
            && matches!(
                symbols.get(target_symbol).kind,
                SymbolKind::Proposition
                    | SymbolKind::PropositionParameter
                    | SymbolKind::MathematicalDefinition
            )
    }

    /*
        Route every machine argument of one call through the operand route its
        already-selected target implies: build-operation operands take their own
        exact declaration resolvers, ordinary operands of a proof-position target
        take the proof-static resolver, and the rest take the static-machine
        resolver with the state-scope subject fallback. Body expression calls and
        statement calls share this one owner.
    */
    pub(in crate::symbols) fn assign_call_machine_argument_symbols(
        symbols: &SymbolTable,
        machine: &super::super::scope::MachineScope<'_>,
        state_symbol: SymbolHandle,
        target: &symbol_resolved_trees::name::DiagnosticName,
        target_symbol: SymbolHandle,
        machine_arguments: &mut [StaticMachineArgument],
    ) {
        let operand_route = build_operand_route(target.as_str(), target_symbol);
        let proof_static = proof_static_call_target(symbols, target_symbol);
        for (index, argument) in machine_arguments.iter_mut().enumerate() {
            match operand_route {
                Some(BuildOperation::ProviderReadiness) if index == 0 => {
                    let mut application = argument.application.take();
                    assign_provider_selection_argument_symbol(symbols, argument, true);
                    if let Some(application) = &mut application {
                        for operand in &mut application.arguments {
                            assign_static_argument_symbols(
                                symbols,
                                machine.symbol,
                                machine.type_parameters,
                                operand,
                                true,
                            );
                        }
                    }
                    argument.application = application;
                }
                Some(
                    BuildOperation::AssumedBindingContract | BuildOperation::TestGroupRegistration,
                ) => {
                    assign_assumed_binding_argument_symbol(symbols, argument);
                }
                // The group's requirement is a machine; the slot a boundary trait.
                Some(BuildOperation::TestGroupService) if index == 0 => {
                    assign_assumed_binding_argument_symbol(symbols, argument);
                }
                Some(BuildOperation::RepresentationSelection) => {
                    assign_representation_selection_argument_symbol(symbols, argument, index == 0);
                }
                Some(operation) => assign_provider_selection_argument_symbol(
                    symbols,
                    argument,
                    matches!(
                        operation,
                        BuildOperation::ProviderSelection
                            | BuildOperation::ProviderReadiness
                            | BuildOperation::BoundaryAcceptance
                    ) && index == 0,
                ),
                None => {
                    assign_static_argument_symbols(
                        symbols,
                        machine.symbol,
                        machine.type_parameters,
                        argument,
                        proof_static,
                    );
                    if !proof_static {
                        assign_runtime_subject_argument_symbol(symbols, state_symbol, argument);
                    }
                }
            }
        }
    }

    /*
        The build operation whose static operands name declarations rather than
        executable static machines, or `None` when a call's operands resolve as
        ordinary static machine arguments.

        `select_provider<Subject, Product>` and `exclude_service<Service>` name
        exact declaration paths (see
        [`assign_provider_selection_argument_symbol`]);
        `select_representation<Type, Representation>` names declaration identity
        with an opaque first operand (see
        [`assign_representation_selection_argument_symbol`]). Provider selection
        takes this route only while its target is unresolved: a declared machine
        which happens to be spelled `select_provider` resolves as an ordinary
        target and keeps ordinary operands. Service exclusion follows the same
        rule, as does representation selection. The remaining build operations carry no
        static operands and route nothing here. Assumed binding operands select exact
        boundary requirement declarations, never executable generic arguments. Their
        independently checked runtime installation owns compatibility and acceptance.
    */
    pub(in crate::symbols) fn build_operand_route(
        target: &str,
        target_symbol: SymbolHandle,
    ) -> Option<BuildOperation> {
        let operation = BuildOperation::from_call_target(target)?;
        (!target_symbol.is_valid() && operation.has_designated_operands()).then_some(operation)
    }

    /*
        A bound selection argument (already carrying a type reference, evidence,
        const, or application payload) takes no symbol; unbound ones render their
        qualified path for name resolution.
    */
    fn selection_argument_rendered_path(argument: &mut StaticMachineArgument) -> Option<String> {
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

    /*
        Fallback resolution through descendant paths for selection arguments: the
        source reference still gates visibility of the found descendant.
    */
    fn descendant_selection_symbol(
        symbols: &SymbolTable,
        argument: &StaticMachineArgument,
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
                reference.is_none_or(|reference| {
                    symbols.source_reference_can_see_symbol(reference, *symbol)
                })
            })
            .unwrap_or_else(SymbolHandle::invalid)
    }

    /*
        Resolve a provider-selection operand as an exact declaration, not an
        executable static machine. The subject may name a boundary trait, top-level
        requirement or operator family; the provider may name data or a named map.
        Both lexical and qualified resolution preserve that declaration category.
        Harvesting validates the resolved declaration after typed lowering.
    */
    pub(in crate::symbols) fn assign_provider_selection_argument_symbol(
        symbols: &SymbolTable,
        argument: &mut StaticMachineArgument,
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
            descendant_selection_symbol(symbols, argument, kinds)
        };
    }

    /*
        A relationship names two exact requirement declarations, not callable bodies.
        Requirement members are State symbols beneath their boundary trait. Product
        visibility still gates both paths; later typed admission rejects other states.
    */
    pub(in crate::symbols) fn assign_assumed_binding_argument_symbol(
        symbols: &SymbolTable,
        argument: &mut StaticMachineArgument,
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

    /*
        Resolve `Build::select_representation` arguments as declaration identity,
        not as executable static machines.
    */
    pub(in crate::symbols) fn assign_representation_selection_argument_symbol(
        symbols: &SymbolTable,
        argument: &mut StaticMachineArgument,
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
        let candidates = symbols
            .symbols()
            .nodes()
            .iter()
            .filter(|(handle, _)| kinds.contains(&symbols.get(*handle).kind))
            .map(|(handle, _)| (handle, true))
            .collect::<Vec<_>>();
        argument.symbol = argument
            .path
            .last()
            .and_then(|name| {
                symbols.find_product_declaration_from_source(
                    &rendered,
                    name.source_span(),
                    candidates,
                )
            })
            .unwrap_or_else(SymbolHandle::invalid);
    }

    /*
        Resolve a proof-static proposition-family binder argument. Unlike an
        executable call's machine selection, proposition arguments may name a
        lexical type/const binder, a concrete type, or a machine identity. The
        proposition declaration's typed telescope performs the final category
        check.
    */
    pub(in crate::symbols) fn resolve_proposition_binder_argument_symbol(
        symbols: &SymbolTable,
        scope_symbol: SymbolHandle,
        path: &[symbol_resolved_trees::name::DiagnosticName],
    ) -> SymbolHandle {
        let [target] = path else {
            return resolve_static_machine_argument_symbol(symbols, scope_symbol, path);
        };
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
}
mod static_namespaces {
    /*
        Source-certified static namespaces for every invocation carrier.

        Ordinary expressions, statement calls and terminal targets share declaration
        selection, lexical shadowing and receiver erasure. Qualification retains its
        source authority and selected callable; it contributes no runtime argument.
    */

    use crate::symbols::lookup;
    use crate::tables::body_expressions;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::statement::TransitionTarget;
    use symbols::{SymbolHandle, SymbolKind};

    /*
        Resolve an authored static path through ordinary source-owned selection.
        A package alias need not have a Module symbol: its loader-certified import
        already selects the exact declaration owner. A machine stands for the
        state `machine_entry_symbol` selects: the child spelled like the call
        when one exists, and `entry` otherwise. An unresolved member still
        claims the call with an invalid target so later passes see a selection,
        not a spelling.
        Every invocation carrier applies this one law: keeping the
        lookup in one place is what stops the two carriers from drifting apart.
    */
    fn static_call_target(
        symbols: &symbols::SymbolTable,
        receiver_name: &str,
        name: &str,
        reference: source::SourceSpan,
        lexical_head: SymbolHandle,
    ) -> Option<SymbolHandle> {
        let fixed = symbols::FixedCoreDeclaration::from_name(name)
            .and_then(|declaration| symbols.fixed_core_declaration_symbol(declaration));
        if let Some(fixed) = fixed.filter(|symbol| symbol.is_valid()) {
            return Some(fixed);
        }
        if lexical_head.is_valid() && symbols.get(lexical_head).kind != SymbolKind::Module {
            return None;
        }
        let namespace = symbols.find_top_level_by_name_and_kinds_from_source(
            receiver_name,
            &[SymbolKind::Module],
            reference,
        );
        if !namespace.is_some_and(|symbol| symbols.get(symbol).kind == SymbolKind::Module)
            && !symbols.is_source_package_qualifier(reference, receiver_name)
        {
            return None;
        }
        Some(
            symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name,
                    super::SOURCE_CALL_KINDS,
                    reference,
                )
                .map(|symbol| {
                    if symbols.get(symbol).kind == SymbolKind::Machine {
                        let call_target = name
                            .strip_prefix(receiver_name)
                            .and_then(|suffix| suffix.strip_prefix("::"))
                            .unwrap_or(name);
                        super::calls::machine_entry_symbol(symbols, symbol, call_target)
                    } else {
                        symbol
                    }
                })
                .unwrap_or_else(SymbolHandle::invalid),
        )
    }

    pub(crate) fn normalize_static_module_calls(
        program: &mut SymbolResolvedTrees,
        calls: &[(ExpressionHandle, Vec<DiagnosticName>)],
        statement_calls: &[(source::SourceSpan, Vec<DiagnosticName>)],
    ) {
        use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
        use symbols::JoinedPath;
        for (expression, path) in calls {
            let Some((target, receiver)) = path.split_last() else {
                continue;
            };
            let receiver_name = receiver.joined_path();
            let reference = lookup::diagnostic_path_source_span(path);
            let name = format!("{receiver_name}::{}", target.as_str());
            let original_receiver = match program.tables.bodies.expressions.expression(*expression)
            {
                ExpressionNode::Call(call) => call.receiver,
                _ => continue,
            };
            let lexical_head = if original_receiver.is_valid() {
                body_expressions(program)
                    .name_path_node(original_receiver)
                    .map(|path| path.head_symbol)
                    .unwrap_or_default()
            } else {
                SymbolHandle::invalid()
            };
            let Some(selected) = static_call_target(
                &program.symbols,
                &receiver_name,
                &name,
                reference,
                lexical_head,
            ) else {
                continue;
            };
            if original_receiver.is_valid()
                && let ExpressionNode::Name(original_path) = body_expressions(program)
                    .expression(original_receiver)
                    .clone()
            {
                let mut prefix = String::new();
                for (offset, member) in receiver.iter().enumerate() {
                    if !prefix.is_empty() {
                        prefix.push_str("::");
                    }
                    prefix.push_str(member.as_str());
                    let selected_module = program
                        .symbols
                        .find_top_level_by_name_and_kinds_from_source(
                            &prefix,
                            &[SymbolKind::Module],
                            reference,
                        )
                        .unwrap_or_else(SymbolHandle::invalid);
                    program
                        .tables
                        .bodies
                        .expressions
                        .set_name_path_member_symbol_at_offset(
                            original_path.member_symbols,
                            offset as u32,
                            selected_module,
                        );
                }
            }
            if let ExpressionNode::Call(call) = program
                .tables
                .bodies
                .expressions
                .expression_mut(*expression)
            {
                call.receiver = ExpressionHandle::invalid();
                call.target = DiagnosticName::from_str(&name, reference);
                call.target_symbol = selected;
            }
        }
        let symbols = &program.symbols;
        let declarations = &mut program.tables.declarations;
        let members = &mut declarations.statement_path_members;
        declarations
            .state_statements
            .for_each_mut(|_, statement| match statement {
                Statement::Call(call) => {
                    let Some((_, path)) = statement_calls
                        .iter()
                        .find(|(span, _)| *span == call.target.source_span())
                    else {
                        return;
                    };
                    let Some((target, receiver)) = path.split_last() else {
                        return;
                    };
                    let receiver_name = receiver.joined_path();
                    let reference = lookup::diagnostic_path_source_span(path);
                    let name = format!("{receiver_name}::{}", target.as_str());
                    let Some(selected) = static_call_target(
                        symbols,
                        &receiver_name,
                        &name,
                        reference,
                        call.receiver_root_symbol,
                    ) else {
                        return;
                    };
                    call.receiver = arena::HandleSpan::empty();
                    call.receiver_symbol = SymbolHandle::invalid();
                    call.receiver_root_symbol = SymbolHandle::invalid();
                    call.target = DiagnosticName::from_str(&name, reference);
                    call.target_symbol = selected;
                }
                Statement::Transition(transition) => {
                    normalize_terminal_namespace(symbols, members, &mut transition.target);
                    if let Some(continuation) = &mut transition.continuation {
                        normalize_terminal_namespace(symbols, members, continuation);
                    }
                }
                _ => {}
            });
    }

    /*
        Static qualification is settled before typed call construction. Keep the
        original authored occurrence and source span; the runtime path has only its
        selected callable coordinate, just as an ordinary normalized call does.
    */
    fn normalize_terminal_namespace(
        symbols: &symbols::SymbolTable,
        members: &mut arena::Arena<DiagnosticName>,
        target: &mut TransitionTarget,
    ) {
        use symbol_resolved_trees::statement::TransitionTarget;
        use symbols::JoinedPath;
        let TransitionTarget::Named(named) = target else {
            return;
        };
        let path = members.span_or_empty(named.path);
        let Some((target, receiver)) = path
            .split_last()
            .filter(|(_, receiver)| !receiver.is_empty())
        else {
            return;
        };
        let receiver_name = receiver.joined_path();
        let reference = lookup::diagnostic_path_source_span(path);
        let name = format!("{receiver_name}::{}", target.as_str());
        let Some(selected) =
            static_call_target(symbols, &receiver_name, &name, reference, named.head_symbol)
        else {
            return;
        };
        let invocation = super::transitions::terminal_call_target(symbols, selected);
        let mut canonical = arena::HandleSpan::empty();
        members.append_to_span(&mut canonical, DiagnosticName::from_str(&name, reference));
        named.path = canonical;
        named.symbol = invocation;
        named.head_symbol = invocation;
        named.receiver_symbol = SymbolHandle::invalid();
    }
}
mod transitions {
    use arena::Arena;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::expressions::{
        assign_expression_span_symbols, assign_statement_expression_symbols,
    };
    use crate::symbols::scope::MachineScope;
    use crate::symbols::scoped_paths::resolve_state_scoped_members;

    pub(in crate::symbols) fn assign_transition_target_symbols(
        machine: &MachineScope<'_>,
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
        statement_path_members: &mut Arena<symbol_resolved_trees::name::DiagnosticName>,
        target: &mut symbol_resolved_trees::statement::TransitionTarget,
        symbols: &SymbolTable,
    ) {
        let symbol_resolved_trees::statement::TransitionTarget::Named(named) = target else {
            if let symbol_resolved_trees::statement::TransitionTarget::Value(expression) = target {
                assign_statement_expression_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    *expression,
                );
            }
            return;
        };

        named.receiver_symbol = SymbolHandle::invalid();
        named.head_symbol = SymbolHandle::invalid();
        named.symbol = SymbolHandle::invalid();
        assign_expression_span_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            named.arguments,
        );

        let path = statement_path_members.span_or_empty(named.path);
        let target_name = path.last().cloned();

        if path.len() == 1
            && let Some(target_name) = target_name.as_ref()
        {
            let target_symbol = super::calls::resolve_call_target_symbol(
                machine,
                parameters,
                false,
                SymbolHandle::invalid(),
                target_name,
                child_type_references,
                symbols,
            );
            if target_symbol.is_valid() {
                let invocation = terminal_call_target(symbols, target_symbol);
                named.head_symbol = invocation;
                named.symbol = invocation;
                return;
            }
        }

        let (head_symbol, symbol) =
            resolve_state_scoped_members(symbols, machine.symbol, state_symbol, path, None);
        named.head_symbol = head_symbol;
        if path.len() >= 2 {
            let (receiver_head, receiver) = resolve_state_scoped_members(
                symbols,
                machine.symbol,
                state_symbol,
                &path[..path.len() - 1],
                None,
            );
            named.head_symbol = receiver_head;
            named.receiver_symbol = receiver;
            if let Some(leaf) = target_name.as_ref() {
                let selected = super::calls::resolve_call_target_symbol(
                    machine,
                    parameters,
                    true,
                    receiver,
                    leaf,
                    child_type_references,
                    symbols,
                );
                if selected.is_valid() {
                    named.symbol = terminal_call_target(symbols, selected);
                    return;
                }
            }
        }
        let body_coordinate = symbols.get(symbol).kind == SymbolKind::State
            && symbols.get(symbols.get(symbol).parent).kind == SymbolKind::Machine;
        if symbol.is_valid() && !body_coordinate {
            named.symbol = symbol;
            return;
        }
        if body_coordinate && named.head_symbol == symbol {
            named.head_symbol = SymbolHandle::invalid();
        }
    }

    /*
        Callable entry coordinates supply a signature, while the owning machine
        supplies invocation identity. This does not make a body name source-visible.
    */
    pub(super) fn terminal_call_target(
        symbols: &SymbolTable,
        target: SymbolHandle,
    ) -> SymbolHandle {
        if symbols.get(target).kind == SymbolKind::State
            && symbols.get(symbols.get(target).parent).kind == SymbolKind::Machine
        {
            symbols.get(target).parent
        } else {
            target
        }
    }
}

pub(super) use calls::{
    SOURCE_CALL_KINDS, assign_call_machine_argument_symbols, assign_static_argument_symbol,
    assign_static_argument_symbols, proof_static_call_target, resolve_call_target_symbol,
    resolve_free_call_target_symbol, resolve_free_machine_entry_state_symbol,
};
pub(super) use transitions::assign_transition_target_symbols;

pub(crate) use static_namespaces::normalize_static_module_calls;

#[cfg(test)]
mod tests {
    /*
        `machine kit::work` names its callable state `work`. The static pass
        used to ask only for a child named `entry` and left `kit::work()`
        with no target. The call keeps that `work` state.
    */

    use crate::resolution::{ResolutionRequest, resolve};
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbols::SymbolKind;

    #[test]
    fn static_module_call_keeps_the_state_named_the_call_target() {
        let text = r#"
            module kit;
            machine kit::work() -> i32 { 1 }
            machine caller() -> i32 { kit::work() }
        "#;
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("kit/main.omg".into(), text.to_owned())
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
        let program = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .expect("resolve");
        let call = program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str() == "kit::work" => Some(call),
                _ => None,
            })
            .expect("the qualified call");
        assert!(
            call.target_symbol.is_valid(),
            "the static pass dropped the state named the call target"
        );
        assert_eq!(
            program.symbols.get(call.target_symbol).kind,
            SymbolKind::State
        );
        assert_eq!(program.symbols.name(call.target_symbol), "work");
        assert_eq!(
            program
                .symbols
                .name(program.symbols.get(call.target_symbol).parent),
            "kit::work"
        );
    }

    /*
        `kit.work()` resolves its receiver to the module. The old arm looked
        for a child stored as `work`, then a state named `entry`.
        `machine kit::work` is stored as `kit::work`, so the call had no
        target, while `kit::work()` kept the state `work`. Both calls now
        keep that state.
    */
    #[test]
    fn module_receiver_call_keeps_the_state_named_the_call_target() {
        let text = r#"
            module kit;
            machine kit::work() -> i32 { 1 }
            machine caller() -> i32 { kit.work() }
        "#;
        let call = resolved_call(text, "work");
        assert!(
            call.target_symbol.is_valid(),
            "the module receiver dropped the state named the call target"
        );
        assert_eq!(call.symbols_kind, SymbolKind::State);
        assert_eq!(call.state_name, "work");
        assert_eq!(call.machine_name, "kit::work");
    }

    /*
        A one-segment machine in the same module still has the generated
        `entry` state. The module receiver and the static pass both keep it.
    */
    #[test]
    fn module_receiver_call_keeps_the_generated_entry_state() {
        let text = r#"
            module kit;
            machine work() -> i32 { 1 }
            machine caller() -> i32 { kit.work() }
        "#;
        let call = resolved_call(text, "work");
        assert_eq!(call.symbols_kind, SymbolKind::State);
        assert_eq!(call.state_name, "entry");
        assert_eq!(call.machine_name, "work");

        let text = r#"
            module kit;
            machine work() -> i32 { 1 }
            machine caller() -> i32 { kit::work() }
        "#;
        let call = resolved_call(text, "kit::work");
        assert_eq!(call.symbols_kind, SymbolKind::State);
        assert_eq!(call.state_name, "entry");
        assert_eq!(call.machine_name, "work");
    }

    /*
        A statement call uses the same module receiver as an expression call.
        `kit.work();` keeps the state named `work`.
    */
    #[test]
    fn module_receiver_statement_matches_the_static_pass() {
        let text = r#"
            module kit;
            machine kit::work() -> i32 { 1 }
            machine caller() { kit.work(); }
        "#;
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("kit/main.omg".into(), text.to_owned())
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
        let program = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .expect("resolve");
        let call = program
            .tables
            .declarations
            .state_statements
            .iter()
            .find_map(|(_, statement)| match statement {
                symbol_resolved_trees::statement::Statement::Call(call)
                    if call.target.as_str() == "work" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("the statement call");
        assert!(call.target_symbol.is_valid());
        assert_eq!(
            program.symbols.get(call.target_symbol).kind,
            SymbolKind::State
        );
        assert_eq!(program.symbols.name(call.target_symbol), "work");
        assert_eq!(
            program
                .symbols
                .name(program.symbols.get(call.target_symbol).parent),
            "kit::work"
        );
    }

    struct ResolvedCall {
        symbols_kind: SymbolKind,
        state_name: String,
        machine_name: String,
        target_symbol: symbols::SymbolHandle,
    }

    fn resolved_call(text: &str, target: &str) -> ResolvedCall {
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("kit/main.omg".into(), text.to_owned())
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
        let program = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .expect("resolve");
        let call = program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str() == target => Some(call.clone()),
                _ => None,
            })
            .expect("the call");
        let target_symbol = call.target_symbol;
        ResolvedCall {
            symbols_kind: program.symbols.get(target_symbol).kind,
            state_name: program.symbols.name(target_symbol).to_owned(),
            machine_name: program
                .symbols
                .name(program.symbols.get(target_symbol).parent)
                .to_owned(),
            target_symbol,
        }
    }
}
