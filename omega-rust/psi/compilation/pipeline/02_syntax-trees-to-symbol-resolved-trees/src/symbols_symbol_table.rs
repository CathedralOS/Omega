/*
    Symbol table construction from every declaration: `children` seeds the
    child symbols each declaration kind owns, and `names` derives the parameter,
    evidence and machine seeds those children are inserted from.
*/

mod children {
    /*
        Child symbols for each kind of top-level declaration.

        `build_symbol_table` and `extend_symbol_table` (in `symbol_table`)
        insert one root symbol per declaration, then call the matching
        `insert_*_symbol_children` function to add that declaration's parameters,
        fields, states and other named children beneath it. Machine children are
        inserted later, by `insert_selected_machine_children`, once all root
        identities exist. A qualified machine does not inherit its carrier's fields.

        The child modules hold the insertion for builtin types, data, machines,
        mathematical definitions, domains and operators (both in `operators`),
        propositions and traits. This file holds the two small cases, measures and
        named conformances, and `insert_machine_parameter_signature_children`,
        which the conformance, data, machine and trait insertions use for
        machine-parameter signatures.

        `symbols::top_level` matches these children to declarations by position,
        walking them in the order they are inserted here, so the insertion order
        and the assignment order must agree.
    */

    mod builtin {
        use symbols::{
            SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder,
            builtin_type_member_symbols,
        };

        pub(in crate::symbols::symbol_table) fn insert_builtin_type_symbol_children(
            builder: &mut SymbolTableBuilder,
            builtin_symbol: SymbolHandle,
            builtin_type: (SymbolKind, SymbolNameRef<'static>),
        ) {
            let SymbolNameRef::Static(name) = builtin_type.1 else {
                return;
            };
            let Some(builtin_type) = crate::symbols::builtin_type_atom_by_symbol_name(name) else {
                return;
            };

            builder.insert_children(builtin_symbol, builtin_type_member_symbols(builtin_type));
        }
    }
    mod data {
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

        use super::insert_generic_parameter_children;
        use crate::symbols::symbol_table::names::{symbol_seed, type_parameter_seeds};

        pub(in crate::symbols::symbol_table) fn insert_data_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            data_symbol: SymbolHandle,
            data_definition: &symbol_resolved_trees::data::DataDefinition,
            has_sources: bool,
        ) {
            let data_children = builder.insert_children(
                data_symbol,
                type_parameter_seeds(
                    program.data_type_parameters(data_definition.type_parameters),
                    false,
                    has_sources,
                )
                .chain(
                    program.data_members(data_definition.members).iter().map(
                        |member| match member {
                            symbol_resolved_trees::data::DataMember::Field(field) => {
                                symbol_seed(SymbolKind::Field, &field.name, has_sources)
                            }
                            symbol_resolved_trees::data::DataMember::Variant(variant) => {
                                symbol_seed(SymbolKind::Variant, &variant.name, has_sources)
                            }
                        },
                    ),
                )
                .chain((!data_definition.requires_facts.is_empty()).then_some((
                    SymbolKind::Parameter,
                    symbols::SymbolNameRef::Borrowed("$predicate_subject"),
                ))),
            );

            let mut data_children = SymbolTableBuilder::child_handles(data_children);
            insert_generic_parameter_children(
                builder,
                program,
                program.data_type_parameters(data_definition.type_parameters),
                &data_definition.conformance_bounds,
                &mut data_children,
                has_sources,
            );
            for member in program.data_members(data_definition.members) {
                let Some(member_symbol) = data_children.next() else {
                    break;
                };
                let symbol_resolved_trees::data::DataMember::Variant(variant) = member else {
                    continue;
                };
                builder.insert_children(
                    member_symbol,
                    program
                        .data_payload_fields(variant.payload)
                        .iter()
                        .map(|field| symbol_seed(SymbolKind::Field, &field.name, has_sources)),
                );
            }
        }
    }
    mod evidence {
        /* Explicit evidence namespaces shared by declaration and callable telescopes. */

        use crate::symbols::symbol_table::names::evidence_requirement_seed;
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolTableAppender};

        pub(super) fn insert_evidence_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            binder: SymbolHandle,
            bound: &symbol_resolved_trees::machine::GenericConformanceBound,
            sources: Option<&source::SourceMap>,
        ) {
            builder.insert_children(
                binder,
                crate::symbols::requirement_namespaces::visible_requirements(
                    program, bound, sources,
                )
                .into_iter()
                .map(|requirement| evidence_requirement_seed(&requirement.name)),
            );
        }
    }
    mod machines {
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

        use super::insert_generic_parameter_children;
        use crate::symbols::symbol_table::names::{
            parameter_seeds, symbol_seed, type_parameter_seeds,
        };

        pub(in crate::symbols::symbol_table) fn insert_machine_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            machine_symbol: SymbolHandle,
            machine: &symbol_resolved_trees::machine::Machine,
            has_sources: bool,
            sources: Option<&source::SourceMap>,
        ) {
            let machine_children = builder.insert_children(
                machine_symbol,
                type_parameter_seeds(program.machine_type_parameters(machine), false, has_sources)
                    .chain(machine.conformance_bounds.iter().map(|bound| {
                        symbol_seed(
                            SymbolKind::ConformanceParameter,
                            &bound.binder_name,
                            has_sources,
                        )
                    }))
                    .chain(program.machine_owned_data(machine.owned_data).iter().map(
                        |owned_data| symbol_seed(SymbolKind::Field, &owned_data.name, has_sources),
                    ))
                    .chain(
                        program
                            .machine_state_handles(machine.states)
                            .iter()
                            .map(|state| program.machine_state(*state))
                            .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
                    ),
            );
            let mut machine_children = SymbolTableBuilder::child_handles(machine_children);

            insert_generic_parameter_children(
                builder,
                program,
                program.machine_type_parameters(machine),
                &machine.conformance_bounds,
                &mut machine_children,
                has_sources,
            );
            for bound in machine.conformance_bounds.iter() {
                let Some(binder_symbol) = machine_children.next() else {
                    break;
                };
                super::evidence::insert_evidence_children(
                    builder,
                    program,
                    binder_symbol,
                    bound,
                    sources,
                );
            }
            for _ in program.machine_owned_data(machine.owned_data) {
                let _ = machine_children.next();
            }
            for state in program.machine_state_handles(machine.states) {
                if let Some(state_symbol) = machine_children.next() {
                    let state = program.machine_state(*state);
                    builder.insert_children(
                        state_symbol,
                        parameter_seeds(program.state_parameters(state.parameters), has_sources),
                    );
                }
            }
        }
    }
    mod mathematical {
        use crate::tables::resolved_declarations;
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender};

        use super::super::names::{symbol_seed, type_parameter_seeds};

        /*
            Children of a mathematical `let`/`boundary let`: its generic binders in
            authored order, then its ordinary telescope parameters. Universe-shaped
            binders (`u: core::Level`) register as `TypeParameter` like every other
            non-machine binder — their carrier's resolved declaration is what makes
            them levels, a classification the elaboration leg owns.
        */
        pub(in crate::symbols) fn insert_mathematical_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            definition_symbol: SymbolHandle,
            definition: &symbol_resolved_trees::mathematical::MathematicalDefinition,
            has_sources: bool,
        ) {
            builder.insert_children(
                definition_symbol,
                type_parameter_seeds(
                    resolved_declarations(program)
                        .data_type_parameters
                        .span_or_empty(definition.binders),
                    false,
                    has_sources,
                )
                .chain(
                    resolved_declarations(program)
                        .mathematical_parameters
                        .span_or_empty(definition.parameters)
                        .iter()
                        .map(|parameter| {
                            symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                        }),
                ),
            );
        }
    }
    mod operators {
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

        use crate::symbols::symbol_table::names::{
            operator_symbol_name, operator_symbol_seed, parameter_seeds, symbol_seed,
        };

        pub(in crate::symbols::symbol_table) fn insert_domain_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            domain_symbol: SymbolHandle,
            domain: &symbol_resolved_trees::domain::DomainDefinition,
            has_sources: bool,
        ) {
            let operator_names = program
                .operator_definitions(domain.operators)
                .iter()
                .map(|operator| operator_symbol_name(program, operator))
                .collect::<Vec<_>>();
            let domain_children = builder.insert_children(
                domain_symbol,
                program
                    .data_type_parameters(domain.type_parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources)
                    })
                    .chain(std::iter::once(symbol_seed(
                        SymbolKind::Parameter,
                        &domain.subject_name,
                        has_sources,
                    )))
                    .chain(
                        operator_names
                            .iter()
                            .zip(program.operator_definitions(domain.operators).iter())
                            .map(|(name, operator)| {
                                operator_symbol_seed(program, operator, name, has_sources)
                            }),
                    ),
            );

            for (operator_symbol, operator) in SymbolTableBuilder::child_handles(domain_children)
                .skip(domain.type_parameters.len() + 1)
                .zip(program.operator_definitions(domain.operators).iter())
            {
                insert_operator_symbol_children(
                    builder,
                    program,
                    operator_symbol,
                    operator,
                    has_sources,
                );
            }
        }

        pub(in crate::symbols::symbol_table) fn insert_operator_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            operator_symbol: SymbolHandle,
            operator: &symbol_resolved_trees::operator::OperatorDefinition,
            has_sources: bool,
        ) {
            builder.insert_children(
                operator_symbol,
                program
                    .data_type_parameters(operator.type_parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources)
                    })
                    .chain(parameter_seeds(
                        program.state_parameters(operator.parameters),
                        has_sources,
                    )),
            );
        }
    }
    mod propositions {
        use crate::tables::resolved_declarations;
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender};

        use super::super::names::{parameter_seeds, symbol_seed};
        use symbol_resolved_trees::proposition::PropositionBinderKind;

        pub(in crate::symbols) fn insert_proposition_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            proposition_symbol: SymbolHandle,
            proposition: &symbol_resolved_trees::proposition::PropositionDefinition,
            has_sources: bool,
        ) {
            builder.insert_children(
                proposition_symbol,
                resolved_declarations(program)
                    .proposition_binders
                    .span_or_empty(proposition.binders)
                    .iter()
                    .map(|binder| {
                        let kind = match binder.kind {
                            PropositionBinderKind::Machine => {
                                SymbolKind::PropositionMachineParameter
                            }
                            _ => SymbolKind::TypeParameter,
                        };
                        symbol_seed(kind, &binder.name, has_sources)
                    })
                    .chain(parameter_seeds(
                        program.state_parameters(proposition.parameters),
                        has_sources,
                    )),
            );
        }
    }
    mod traits {
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

        use crate::symbols::symbol_table::names::symbol_seed;
        use symbol_resolved_trees::data::TypeParameterKind;
        use symbol_resolved_trees::trait_definition::TraitDefinition;

        pub(in crate::symbols::symbol_table) fn insert_trait_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            trait_symbol: SymbolHandle,
            trait_definition: &TraitDefinition,
            has_sources: bool,
        ) {
            let trait_children = builder.insert_children(
                trait_symbol,
                program
                    .trait_type_parameters(trait_definition)
                    .iter()
                    .map(|parameter| {
                        let kind = match parameter.kind {
                            TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
                            TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                            TypeParameterKind::Proposition { .. } => {
                                SymbolKind::PropositionParameter
                            }
                            _ => SymbolKind::TypeParameter,
                        };
                        symbol_seed(kind, &parameter.name, has_sources)
                    })
                    .chain(
                        program
                            .trait_machine_signatures(trait_definition.machines)
                            .iter()
                            .chain(
                                trait_definition
                                    .refinement_clauses
                                    .iter()
                                    .map(|clause| &clause.signature),
                            )
                            .map(|machine| {
                                symbol_seed(SymbolKind::State, &machine.name, has_sources)
                            }),
                    ),
            );

            let mut trait_children = SymbolTableBuilder::child_handles(trait_children);
            super::insert_generic_parameter_children(
                builder,
                program,
                program.trait_type_parameters(trait_definition),
                &trait_definition.conformance_bounds,
                &mut trait_children,
                has_sources,
            );

            for (machine_symbol, machine) in trait_children.zip(
                program
                    .trait_machine_signatures(trait_definition.machines)
                    .iter()
                    .chain(
                        trait_definition
                            .refinement_clauses
                            .iter()
                            .map(|clause| &clause.signature),
                    ),
            ) {
                super::insert_machine_parameter_signature_children(
                    builder,
                    program,
                    machine_symbol,
                    machine,
                    has_sources,
                );
            }
        }
    }
    mod type_aliases {
        /* Alias parameters belong to the source declaration, without runtime children. */

        use super::insert_machine_parameter_signature_children;
        use crate::symbols::symbol_table::names::{parameter_seeds, type_parameter_seeds};
        use symbol_resolved_trees::SymbolResolvedTrees;
        use symbol_resolved_trees::data::TypeParameterKind;
        use symbol_resolved_trees::type_aliases::TypeAliasDefinition;
        use symbols::{SymbolHandle, SymbolTableAppender, SymbolTableBuilder};

        pub(in crate::symbols::symbol_table) fn insert_type_alias_symbol_children(
            builder: &mut impl SymbolTableAppender,
            program: &SymbolResolvedTrees,
            alias_symbol: SymbolHandle,
            alias: &TypeAliasDefinition,
            has_sources: bool,
        ) {
            let parameters = program.data_type_parameters(alias.type_parameters);
            let children = builder.insert_children(
                alias_symbol,
                type_parameter_seeds(parameters, true, has_sources),
            );
            for (parameter, symbol) in parameters
                .iter()
                .zip(SymbolTableBuilder::child_handles(children))
            {
                match &parameter.kind {
                    TypeParameterKind::Machine { contract } => {
                        if let Some(contract) = contract.structural() {
                            insert_machine_parameter_signature_children(
                                builder,
                                program,
                                symbol,
                                contract,
                                has_sources,
                            );
                        }
                    }
                    TypeParameterKind::Proposition { contract } => {
                        builder.insert_children(
                            symbol,
                            parameter_seeds(
                                program.state_parameters(contract.parameters),
                                has_sources,
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    pub(super) use builtin::insert_builtin_type_symbol_children;
    pub(super) use data::insert_data_symbol_children;
    pub(super) use machines::insert_machine_symbol_children;
    pub(super) use mathematical::insert_mathematical_symbol_children;
    pub(super) use operators::{insert_domain_symbol_children, insert_operator_symbol_children};
    pub(super) use propositions::insert_proposition_symbol_children;
    pub(super) use traits::insert_trait_symbol_children;
    pub(super) use type_aliases::insert_type_alias_symbol_children;

    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolTableAppender, SymbolTableBuilder};

    use super::names::{parameter_seeds, type_parameter_seeds};
    use symbol_resolved_trees::data::TypeParameterKind;

    pub(super) fn insert_measure_symbol_children(
        builder: &mut impl SymbolTableAppender,
        measure_symbol: SymbolHandle,
        measure: &symbol_resolved_trees::measure::MeasureDefinition,
        has_sources: bool,
    ) {
        builder.insert_children(
            measure_symbol,
            parameter_seeds(measure.parameter.iter(), has_sources),
        );
    }

    /*
        Insert the lexical children owned by a machine-parameter signature. Its
        generic parameters precede its value parameters, mirroring ordinary
        machine symbol layout. Nested machine parameters recursively own their
        own signature children.
        Every declaration's generic parameters own their nested callable,
        proposition or explicit evidence namespace through this same walk.
    */
    fn insert_generic_parameter_children(
        builder: &mut impl SymbolTableAppender,
        program: &SymbolResolvedTrees,
        parameters: &[symbol_resolved_trees::data::TypeParameter],
        bounds: &[symbol_resolved_trees::machine::GenericConformanceBound],
        children: &mut impl Iterator<Item = SymbolHandle>,
        has_sources: bool,
    ) {
        for parameter in parameters {
            let Some(parameter_symbol) = children.next() else {
                break;
            };
            match &parameter.kind {
                TypeParameterKind::Machine { contract } => {
                    if let Some(contract) = contract.structural() {
                        insert_machine_parameter_signature_children(
                            builder,
                            program,
                            parameter_symbol,
                            contract,
                            has_sources,
                        );
                    }
                }
                TypeParameterKind::Implementation => {
                    if let Some(bound) = bounds
                        .iter()
                        .find(|bound| bound.binder_name == parameter.name)
                    {
                        let sources = builder.source_map();
                        evidence::insert_evidence_children(
                            builder,
                            program,
                            parameter_symbol,
                            bound,
                            sources.as_deref(),
                        );
                    }
                }
                TypeParameterKind::Proposition { contract } => {
                    builder.insert_children(
                        parameter_symbol,
                        parameter_seeds(program.state_parameters(contract.parameters), has_sources),
                    );
                }
                _ => {}
            }
        }
    }

    fn insert_machine_parameter_signature_children(
        builder: &mut impl SymbolTableAppender,
        program: &SymbolResolvedTrees,
        owner_symbol: SymbolHandle,
        signature: &symbol_resolved_trees::signature::StateSignature,
        has_sources: bool,
    ) {
        let children = builder.insert_children(
            owner_symbol,
            type_parameter_seeds(
                program.data_type_parameters(signature.type_parameters),
                false,
                has_sources,
            )
            .chain(parameter_seeds(
                program.state_parameters(signature.parameters),
                has_sources,
            )),
        );

        let mut children = SymbolTableBuilder::child_handles(children);
        insert_generic_parameter_children(
            builder,
            program,
            program.data_type_parameters(signature.type_parameters),
            &signature.conformance_bounds,
            &mut children,
            has_sources,
        );
    }

    pub(in crate::symbols::symbol_table) fn insert_conformance_symbol_children(
        builder: &mut impl SymbolTableAppender,
        program: &SymbolResolvedTrees,
        conformance_symbol: SymbolHandle,
        conformance: &symbol_resolved_trees::trait_definition::Conformance,
        has_sources: bool,
    ) {
        let children = builder.insert_children(
            conformance_symbol,
            type_parameter_seeds(
                program.data_type_parameters(conformance.type_parameters),
                true,
                has_sources,
            ),
        );

        let mut children = SymbolTableBuilder::child_handles(children);
        for parameter in program.data_type_parameters(conformance.type_parameters) {
            let Some(parameter_symbol) = children.next() else {
                break;
            };
            match &parameter.kind {
                TypeParameterKind::Machine { contract } => {
                    if let Some(contract) = contract.structural() {
                        insert_machine_parameter_signature_children(
                            builder,
                            program,
                            parameter_symbol,
                            contract,
                            has_sources,
                        );
                    }
                }
                TypeParameterKind::Proposition { contract } => {
                    builder.insert_children(
                        parameter_symbol,
                        parameter_seeds(program.state_parameters(contract.parameters), has_sources),
                    );
                }
                _ => {}
            }
        }
    }
}
mod names {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbols::{JoinedPath, SymbolKind, SymbolNameRef};

    pub(super) type SymbolSeed<'name> = (SymbolKind, SymbolNameRef<'name>);

    pub(super) fn symbol_seed<'name>(
        kind: SymbolKind,
        name: &'name symbol_resolved_trees::name::DiagnosticName,
        has_sources: bool,
    ) -> SymbolSeed<'name> {
        if has_sources && name.is_source_backed() {
            (
                kind,
                SymbolNameRef::OwnedSource {
                    value: name.as_str(),
                    source_span: name.source_span(),
                },
            )
        } else {
            (kind, SymbolNameRef::Borrowed(name.as_str()))
        }
    }

    /*
        A bound requirement is a child of the local evidence binder, rather than
        another declaration at the requirement's original source occurrence.
        Keep its spelling while inheriting provenance from that exact binder.
    */
    pub(in crate::symbols::symbol_table) fn evidence_requirement_seed(
        name: &symbol_resolved_trees::name::DiagnosticName,
    ) -> SymbolSeed<'_> {
        (SymbolKind::State, SymbolNameRef::Borrowed(name.as_str()))
    }

    /*
        The value-parameter child stream of one callable signature: every
        declaration kind seeds its parameter children, in order, from the same
        declared names.
    */
    pub(in crate::symbols::symbol_table) fn parameter_seeds<'name>(
        parameters: impl IntoIterator<Item = &'name symbol_resolved_trees::signature::StateParameter>,
        has_sources: bool,
    ) -> impl Iterator<Item = SymbolSeed<'name>> {
        parameters
            .into_iter()
            .map(move |parameter| symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources))
    }

    /*
        The type-parameter children of one declaration, in declaration order.
        `proposition_parameters` is true only where the declaring family admits
        proposition binders.
    */
    pub(in crate::symbols::symbol_table) fn type_parameter_seeds<'name>(
        parameters: impl IntoIterator<Item = &'name symbol_resolved_trees::data::TypeParameter>,
        proposition_parameters: bool,
        has_sources: bool,
    ) -> impl Iterator<Item = SymbolSeed<'name>> {
        parameters.into_iter().map(move |parameter| {
            type_parameter_seed(parameter, proposition_parameters, has_sources)
        })
    }

    /*
        Seed a type parameter's child symbol: `Machine` binders always get their
        own kind; `Proposition` binders get theirs only where the declaring family
        admits proposition parameters.
    */
    pub(in crate::symbols::symbol_table) fn type_parameter_seed<'name>(
        parameter: &'name symbol_resolved_trees::data::TypeParameter,
        proposition_parameters: bool,
        has_sources: bool,
    ) -> SymbolSeed<'name> {
        let kind = match parameter.kind {
            TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
            TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
            TypeParameterKind::Proposition { .. } if proposition_parameters => {
                SymbolKind::PropositionParameter
            }
            _ => SymbolKind::TypeParameter,
        };
        symbol_seed(kind, &parameter.name, has_sources)
    }

    /*
        A machine declaration's seed. Compiler-synthesized machines (trait
        defaults, equatable realizations) carry a generated name, so the authored
        carrier occurrence in the conformance supplies the declaration's exact
        provenance: that occurrence pins the conforming package's identity even
        when the carrier declaration itself lives in another package.
    */
    pub(super) fn machine_symbol_seed<'name>(
        machine: &'name symbol_resolved_trees::machine::Machine,
        has_sources: bool,
    ) -> SymbolSeed<'name> {
        if machine.target.is_some() {
            return symbol_seed(
                SymbolKind::Machine,
                &machine.target_symbol_name,
                has_sources,
            );
        }
        if has_sources
            && !machine.name.is_source_backed()
            && let Some(attached) = machine
                .attached_data
                .as_ref()
                .filter(|attached| attached.is_source_backed())
        {
            return (
                SymbolKind::Machine,
                SymbolNameRef::OwnedSource {
                    value: machine.name.as_str(),
                    source_span: attached.source_span(),
                },
            );
        }
        symbol_seed(SymbolKind::Machine, &machine.name, has_sources)
    }

    pub(super) fn operator_symbol_name(
        program: &SymbolResolvedTrees,
        operator: &symbol_resolved_trees::operator::OperatorDefinition,
    ) -> String {
        program.operator_path_string(operator.name)
    }

    pub(super) fn measure_symbol_name(
        program: &SymbolResolvedTrees,
        measure: &symbol_resolved_trees::measure::MeasureDefinition,
    ) -> String {
        program.measure_path_members(measure.name).joined_path()
    }

    pub(super) fn measure_symbol_seed<'name>(
        program: &SymbolResolvedTrees,
        measure: &symbol_resolved_trees::measure::MeasureDefinition,
        canonical_name: &'name str,
        has_sources: bool,
    ) -> SymbolSeed<'name> {
        match program.measure_path_members(measure.name).last() {
            Some(name) if has_sources && name.is_source_backed() => (
                SymbolKind::Measure,
                SymbolNameRef::OwnedSource {
                    value: canonical_name,
                    source_span: name.source_span(),
                },
            ),
            _ => (SymbolKind::Measure, SymbolNameRef::Borrowed(canonical_name)),
        }
    }

    pub(super) fn operator_symbol_seed<'name>(
        program: &SymbolResolvedTrees,
        operator: &symbol_resolved_trees::operator::OperatorDefinition,
        canonical_name: &'name str,
        has_sources: bool,
    ) -> SymbolSeed<'name> {
        let source_name = program.operator_path_members(operator.name).last();
        if has_sources && source_name.is_some_and(|name| name.is_source_backed()) {
            (
                SymbolKind::Operator,
                SymbolNameRef::OwnedSource {
                    value: canonical_name,
                    source_span: source_name
                        .expect("source-backed operator path member")
                        .source_span(),
                },
            )
        } else {
            (
                SymbolKind::Operator,
                SymbolNameRef::Borrowed(canonical_name),
            )
        }
    }
}

use std::sync::Arc;

use source::SourceMap;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{
    FixedCoreDeclaration, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder,
    builtin_function_symbols, builtin_type_symbols, fixed_core_declaration_symbols,
};

use crate::symbols::symbol_table::children::{
    insert_builtin_type_symbol_children, insert_conformance_symbol_children,
    insert_data_symbol_children, insert_domain_symbol_children, insert_machine_symbol_children,
    insert_mathematical_symbol_children, insert_measure_symbol_children,
    insert_operator_symbol_children, insert_proposition_symbol_children,
    insert_trait_symbol_children, insert_type_alias_symbol_children,
};
use crate::symbols::symbol_table::names::{
    machine_symbol_seed, measure_symbol_name, measure_symbol_seed, operator_symbol_name,
    operator_symbol_seed, symbol_seed,
};

pub(super) fn extend_symbol_table(
    program: &mut SymbolResolvedTrees,
    sources: Arc<SourceMap>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    roots: crate::resolution::lowerer::RootWatermarks,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
) {
    let has_sources = true;
    let mut extension = std::mem::take(&mut program.symbols)
        .begin_extension(Some(sources.clone()), source_scoped_top_level_bindings);

    macro_rules! extend_root_family {
        ($roster:ident, $kind:ident, $insert:path $(, $argument:expr)*) => {
            for index in roots.$roster..program.$roster.len() {
                let definition = program.$roster[index].clone();
                let symbol = extension.insert_top_level([symbol_seed(
                    SymbolKind::$kind,
                    &definition.name,
                    has_sources,
                )])[0];
                $insert(&mut extension, program, symbol, &definition, has_sources $(, $argument)*);
                program.$roster[index].symbol = symbol;
            }
        };
    }
    extend_root_family!(domain_definitions, Domain, insert_domain_symbol_children);
    extend_root_family!(data_definitions, Data, insert_data_symbol_children);
    extend_root_family!(type_aliases, TypeAlias, insert_type_alias_symbol_children);
    for index in roots.conformances..program.conformances.len() {
        let conformance = program.conformances[index].clone();
        let Some(alias) = &conformance.alias else {
            continue;
        };
        let symbol =
            extension.insert_top_level([symbol_seed(SymbolKind::Conformance, alias, has_sources)])
                [0];
        insert_conformance_symbol_children(
            &mut extension,
            program,
            symbol,
            &conformance,
            has_sources,
        );
        program.conformances[index].symbol = symbol;
    }
    for index in roots.machines..program.machines.len() {
        let machine = program.machines[index].clone();
        if machine.lexical_parent.is_valid() {
            continue;
        }
        let symbol = extension.insert_top_level([machine_symbol_seed(&machine, has_sources)])[0];
        program.machines[index].symbol = symbol;
    }
    extend_root_family!(
        propositions,
        Proposition,
        insert_proposition_symbol_children
    );
    extend_root_family!(
        mathematical_definitions,
        MathematicalDefinition,
        insert_mathematical_symbol_children
    );
    for index in roots.operators..program.operators.len() {
        let operator = program.operators[index].clone();
        let name = operator_symbol_name(program, &operator);
        let symbol = extension.insert_top_level([operator_symbol_seed(
            program,
            &operator,
            &name,
            has_sources,
        )])[0];
        insert_operator_symbol_children(&mut extension, program, symbol, &operator, has_sources);
        program.operators[index].symbol = symbol;
    }
    for index in roots.measures..program.measures.len() {
        let measure = program.measures[index].clone();
        let name = measure_symbol_name(program, &measure);
        let symbol = extension.insert_top_level([measure_symbol_seed(
            program,
            &measure,
            &name,
            has_sources,
        )])[0];
        insert_measure_symbol_children(&mut extension, symbol, &measure, has_sources);
        program.measures[index].symbol = symbol;
    }
    extend_root_family!(traits, Trait, insert_trait_symbol_children);
    for index in roots.wire_schemas..program.wire_schemas.len() {
        let schema = program.wire_schemas[index].clone();
        let symbol = extension.insert_top_level([symbol_seed(
            SymbolKind::WireSchema,
            &schema.name,
            has_sources,
        )])[0];
        program.wire_schemas[index].symbol = symbol;
    }
    for declaration in const_declarations {
        let symbol = extension.insert_top_level([(
            SymbolKind::Const,
            SymbolNameRef::OwnedSource {
                value: declaration.semantic_name.as_str(),
                source_span: declaration.source_span,
            },
        )])[0];
        program
            .const_declarations
            .get_mut(declaration.declaration)
            .symbol = symbol;
    }

    program.symbols = extension.finish();
}

pub(super) fn build_symbol_table(
    program: &mut SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
    has_authored_modules: bool,
) -> SymbolTable {
    let has_sources = sources.is_some() || has_authored_modules;
    let root_operator_names = program
        .operators
        .iter()
        .map(|operator| operator_symbol_name(program, operator))
        .collect::<Vec<_>>();
    let measure_names = program
        .measures
        .iter()
        .map(|measure| measure_symbol_name(program, measure))
        .collect::<Vec<_>>();
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        sources.clone(),
        source_scoped_top_level_bindings,
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let root_children = builder.insert_children(
        root,
        builtin_type_symbols()
            .into_iter()
            .chain(builtin_function_symbols())
            .chain(fixed_core_declaration_symbols())
            .chain(
                program
                    .domain_definitions
                    .iter()
                    .map(|domain| symbol_seed(SymbolKind::Domain, &domain.name, has_sources)),
            )
            .chain(
                program
                    .data_definitions
                    .iter()
                    .map(|data| symbol_seed(SymbolKind::Data, &data.name, has_sources)),
            )
            .chain(
                program
                    .type_aliases
                    .iter()
                    .map(|alias| symbol_seed(SymbolKind::TypeAlias, &alias.name, has_sources)),
            )
            .chain(program.conformances.iter().filter_map(|conformance| {
                conformance
                    .alias
                    .as_ref()
                    .map(|alias| symbol_seed(SymbolKind::Conformance, alias, has_sources))
            }))
            .chain(
                program
                    .machines
                    .iter()
                    .filter(|machine| !machine.lexical_parent.is_valid())
                    .map(|machine| machine_symbol_seed(machine, has_sources)),
            )
            .chain(program.propositions.iter().map(|proposition| {
                symbol_seed(SymbolKind::Proposition, &proposition.name, has_sources)
            }))
            .chain(program.mathematical_definitions.iter().map(|definition| {
                symbol_seed(
                    SymbolKind::MathematicalDefinition,
                    &definition.name,
                    has_sources,
                )
            }))
            .chain(
                root_operator_names
                    .iter()
                    .zip(program.operators.iter())
                    .map(|(name, operator)| {
                        operator_symbol_seed(program, operator, name, has_sources)
                    }),
            )
            .chain(
                measure_names
                    .iter()
                    .zip(program.measures.iter())
                    .map(|(name, measure)| {
                        measure_symbol_seed(program, measure, name, has_sources)
                    }),
            )
            .chain(program.traits.iter().map(|trait_definition| {
                symbol_seed(SymbolKind::Trait, &trait_definition.name, has_sources)
            }))
            .chain(program.wire_schemas.iter().map(|wire_schema| {
                symbol_seed(SymbolKind::WireSchema, &wire_schema.name, has_sources)
            }))
            .chain(const_declarations.iter().map(|declaration| {
                (
                    SymbolKind::Const,
                    SymbolNameRef::OwnedSource {
                        value: declaration.semantic_name.as_str(),
                        source_span: declaration.source_span,
                    },
                )
            })),
    );
    let mut root_children = SymbolTableBuilder::child_handles(root_children);

    for builtin_type in builtin_type_symbols() {
        if let Some(builtin_symbol) = root_children.next() {
            insert_builtin_type_symbol_children(&mut builder, builtin_symbol, builtin_type);
        }
    }
    for _ in 0..builtin_function_symbols().len() {
        let _ = root_children.next();
    }
    for _ in 0..FixedCoreDeclaration::COUNT {
        let _ = root_children.next();
    }
    macro_rules! install_root_family {
        ($roster:ident, $insert:path $(, $argument:expr)*) => {
            for ordinal in 0..program.$roster.len() {
                let item = &program.$roster[ordinal];
                if let Some(symbol) = root_children.next() {
                    $insert(&mut builder, program, symbol, item, has_sources $(, $argument)*);
                    program.$roster[ordinal].symbol = symbol;
                }
            }
        };
    }
    install_root_family!(domain_definitions, insert_domain_symbol_children);
    install_root_family!(data_definitions, insert_data_symbol_children);
    install_root_family!(type_aliases, insert_type_alias_symbol_children);
    for ordinal in 0..program.conformances.len() {
        let conformance = &program.conformances[ordinal];
        if conformance.alias.is_some()
            && let Some(conformance_symbol) = root_children.next()
        {
            insert_conformance_symbol_children(
                &mut builder,
                program,
                conformance_symbol,
                conformance,
                has_sources,
            );
            program.conformances[ordinal].symbol = conformance_symbol;
        }
    }
    for index in 0..program.machines.len() {
        if !program.machines[index].lexical_parent.is_valid() {
            program.machines[index].symbol = root_children.next().unwrap_or_default();
        }
    }
    install_root_family!(propositions, insert_proposition_symbol_children);
    install_root_family!(
        mathematical_definitions,
        insert_mathematical_symbol_children
    );
    install_root_family!(operators, insert_operator_symbol_children);
    for ordinal in 0..program.measures.len() {
        let measure = &program.measures[ordinal];
        if let Some(measure_symbol) = root_children.next() {
            insert_measure_symbol_children(&mut builder, measure_symbol, measure, has_sources);
            program.measures[ordinal].symbol = measure_symbol;
        }
    }
    install_root_family!(traits, insert_trait_symbol_children);
    program.wire_schemas.for_each_mut(|schema| {
        schema.symbol = root_children.next().unwrap_or_default();
    });
    for declaration in const_declarations {
        program
            .const_declarations
            .get_mut(declaration.declaration)
            .symbol = root_children.next().unwrap_or_default();
    }

    builder.finish()
}

/*
    Add the selected declaration cohort's own children after root identities
    exist. Carrier qualification supplies neither field aliases nor storage.
*/
pub(super) fn insert_selected_machine_children(
    program: &mut SymbolResolvedTrees,
    table: SymbolTable,
    sources: Option<Arc<SourceMap>>,
    first_machine: usize,
    has_sources: bool,
) -> SymbolTable {
    let mut extension = table.begin_extension(sources.clone(), Vec::new());
    for index in first_machine..program.machines.len() {
        let machine = program.machines[index].clone();
        let symbol = if machine.lexical_parent.is_valid() {
            let parent = program.machines.get(machine.lexical_parent).symbol;
            let children =
                extension.append_children(parent, [machine_symbol_seed(&machine, has_sources)]);
            program.machines[index].symbol = children.start();
            children.start()
        } else {
            machine.symbol
        };
        insert_machine_symbol_children(
            &mut extension,
            program,
            symbol,
            &machine,
            has_sources,
            sources.as_deref(),
        );
    }
    extension.finish()
}

/*
    Append implementation-owned children after the whole declaration cohort
    has identities. A later completion uses this same operation with a selected
    cohort: existing state/parameter/local handles are never assigned again.
*/
pub(super) fn append_body_symbols(
    program: &mut SymbolResolvedTrees,
    selected_machines: &[symbols::SymbolHandle],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    if selected_machines.iter().any(|selected| {
        !selected.is_valid()
            || !program
                .machines
                .iter()
                .any(|machine| machine.symbol == *selected)
    }) {
        return Err(vec![diagnostics::Diagnostic::error(
            "body symbol completion requires an existing machine declaration",
        )]);
    }
    let has_sources = program.symbols.source_files().next().is_some();
    let mut new_states = Vec::new();
    let declarations = &mut program.tables.declarations;
    for machine in program.roots.machines.iter() {
        if !selected_machines.contains(&machine.symbol) {
            continue;
        }
        for state_handle in declarations
            .machine_state_handles
            .span_or_empty(machine.states)
        {
            let state = declarations.machine_states.get_mut(*state_handle);
            if !state.symbol.is_valid() {
                let inserted = program.symbols.append_children(
                    machine.symbol,
                    [symbol_seed(SymbolKind::State, &state.name, has_sources)],
                );
                state.symbol = inserted.start();
                new_states.push((machine.type_parameters, *state_handle));
            }
            let parameters = declarations
                .state_parameters
                .span_mut_or_empty(state.parameters);
            let inserted = program.symbols.append_children(
                state.symbol,
                parameters
                    .iter()
                    .filter(|parameter| !parameter.symbol.is_valid())
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            );
            for (parameter, symbol) in parameters
                .iter_mut()
                .filter(|parameter| !parameter.symbol.is_valid())
                .zip(SymbolTableBuilder::child_handles(inserted))
            {
                parameter.symbol = symbol;
            }
            let statements = declarations
                .state_statements
                .span_mut_or_empty(state.statements);
            append_local_symbols(&mut program.symbols, state.symbol, statements);
            let graphs: Vec<_> = statements
                .iter()
                .filter_map(|statement| {
                    if let symbol_resolved_trees::statement::Statement::Assembly(handle) = statement
                    {
                        Some(*handle)
                    } else {
                        None
                    }
                })
                .collect();
            for handle in graphs {
                let graph = declarations.assembly_graphs.get(handle);
                for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                    append_local_symbols(
                        &mut program.symbols,
                        state.symbol,
                        declarations
                            .state_statements
                            .span_mut_or_empty(block.instructions),
                    );
                }
            }
        }
    }
    for (type_parameters, state_handle) in new_states {
        let state = declarations.machine_states.get_mut(state_handle);
        let type_parameters = declarations
            .data_type_parameters
            .span_or_empty(type_parameters);
        for parameter in declarations
            .state_parameters
            .span_mut_or_empty(state.parameters)
        {
            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                &program.symbols,
                &mut declarations.child_type_references,
                &program.tables.types.constraints,
                type_parameters,
                &mut parameter.type_reference,
            );
        }
        if let Some(return_type) = &mut state.return_type {
            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                &program.symbols,
                &mut declarations.child_type_references,
                &program.tables.types.constraints,
                type_parameters,
                return_type,
            );
        }
    }
    Ok(())
}

/*
    The same append-only declaration allocation serves ordinary bodies and
    one-shot lexical input preparation. Already indexed locals keep identity.
*/
pub(super) fn append_local_symbols(
    symbols: &mut SymbolTable,
    state: symbols::SymbolHandle,
    statements: &mut [symbol_resolved_trees::statement::Statement],
) {
    use symbol_resolved_trees::statement::Statement;
    let has_sources = symbols.source_files().next().is_some();
    let inserted = symbols.append_children(
        state,
        statements.iter().filter_map(|statement| match statement {
            Statement::LocalData(local) if !local.symbol.is_valid() => {
                Some(symbol_seed(SymbolKind::Local, &local.name, has_sources))
            }
            _ => None,
        }),
    );
    let locals = statements
        .iter_mut()
        .filter_map(|statement| match statement {
            Statement::LocalData(local) if !local.symbol.is_valid() => Some(local),
            _ => None,
        });
    for (local, symbol) in locals.zip(SymbolTableBuilder::child_handles(inserted)) {
        local.symbol = symbol;
    }
}
