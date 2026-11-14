/*
Written type-reference lowering (`type_reference`), domain alias expansion,
domain constraint normalization, closed const-argument applications, and
generic origin validation.
*/

pub(crate) mod domain_aliases {
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use symbols::JoinedPath;

    #[derive(Debug, Clone)]
    pub(crate) struct ExpandedDomainReference {
        pub(crate) symbol: symbols::SymbolHandle,
        pub(crate) path: Vec<resolved::name::DiagnosticName>,
    }

    pub(crate) fn expand_domain_reference(
        program: &resolved::SymbolResolvedTrees,
        symbol: symbols::SymbolHandle,
        path: Vec<resolved::name::DiagnosticName>,
    ) -> Result<Vec<ExpandedDomainReference>, Diagnostic> {
        fn expand(
            program: &resolved::SymbolResolvedTrees,
            symbol: symbols::SymbolHandle,
            path: Vec<resolved::name::DiagnosticName>,
            stack: &mut Vec<symbols::SymbolHandle>,
        ) -> Result<Vec<ExpandedDomainReference>, Diagnostic> {
            let name = path.joined_path();
            if name == "Carry::Portable" {
                return Ok(language_semantics::CarryPermission::ALL
                    .into_iter()
                    .map(|permission| {
                        let [namespace, member] = permission
                            .name()
                            .split("::")
                            .collect::<Vec<_>>()
                            .try_into()
                            .expect("compiler carry permission has two path members");
                        ExpandedDomainReference {
                            symbol: symbols::SymbolHandle::invalid(),
                            path: vec![
                                resolved::name::DiagnosticName::generated_static(namespace),
                                resolved::name::DiagnosticName::generated_static(member),
                            ],
                        }
                    })
                    .collect());
            }
            let Some(domain) = program.domain_definition_by_symbol(symbol) else {
                return Ok(vec![ExpandedDomainReference { symbol, path }]);
            };
            let Some(alias) = domain.alias.as_ref() else {
                return Ok(vec![ExpandedDomainReference { symbol, path }]);
            };
            if let Some(cycle_start) = stack.iter().position(|candidate| *candidate == symbol) {
                let cycle = stack[cycle_start..]
                    .iter()
                    .copied()
                    .chain(std::iter::once(symbol))
                    .filter_map(|candidate| program.domain_definition_by_symbol(candidate))
                    .map(|domain| domain.name.to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return Err(Diagnostic::error(format!("domain alias cycle: {cycle}")));
            }
            if alias.constituents.is_empty() {
                return Err(Diagnostic::error(format!(
                    "domain alias `{}` must name at least one constituent",
                    domain.name
                )));
            }

            stack.push(symbol);
            let mut expanded = Vec::new();
            for constituent in &alias.constituents {
                let constituent_path = program.domain_path_members(constituent.domain).to_vec();
                expanded.extend(expand(
                    program,
                    constituent.domain_symbol,
                    constituent_path,
                    stack,
                )?);
            }
            stack.pop();
            Ok(expanded)
        }

        expand(program, symbol, path, &mut Vec::new())
    }
}
pub(crate) mod domain_constraints {
    use crate::type_reference::domain_aliases::expand_domain_reference;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::JoinedPath;
    use typed_trees::TypedTrees;
    use typed_trees::domain::{DomainDefinition, domain_by_symbol};
    use typed_trees::name::Identifier;
    use typed_trees::types::{DomainConstraint, DomainConstraintSubject, TypeConstraintNode};

    pub(crate) fn normalize_domain_constraints(
        source: &SymbolResolvedTrees,
        program: &mut TypedTrees,
        type_reference_frontier: usize,
    ) -> Result<(), Diagnostic> {
        let sites = program
            .type_reference_table
            .constrained_type_reference_sites()
            .into_iter()
            .filter(|(site, _, _)| {
                usize::try_from(site.arena_index())
                    .is_ok_and(|index| index > type_reference_frontier)
            })
            .collect::<Vec<_>>();

        for (site, carrier, constraints) in sites {
            normalize_constraint_span(source, program, site, carrier, constraints)?;
        }
        Ok(())
    }

    pub(crate) fn normalize_domain_constraints_for_type(
        source: &SymbolResolvedTrees,
        program: &mut TypedTrees,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Result<(), Diagnostic> {
        match program
            .type_reference_table
            .type_reference(type_reference)
            .clone()
        {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                normalize_domain_constraints_for_type(source, program, referee)?;
            }
            typed_trees::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                normalize_constraint_span(source, program, type_reference, base_type, constraints)?
            }
            _ => {}
        }
        Ok(())
    }

    fn normalize_constraint_span(
        source: &SymbolResolvedTrees,
        program: &mut TypedTrees,
        site: typed_trees::types::TypeReferenceHandle,
        carrier: typed_trees::types::TypeReferenceHandle,
        constraints: arena::HandleSpan<TypeConstraintNode>,
    ) -> Result<(), Diagnostic> {
        let authored = program
            .type_reference_table
            .constraints(constraints)
            .to_vec();
        let mut normalized = Vec::new();
        let mut expanded_alias = false;

        for constraint in authored {
            let TypeConstraintNode::Domain(domain_constraint) = constraint else {
                normalized.push(constraint);
                continue;
            };
            let authored_name = domain_constraint.name.as_str();
            let reference = domain_constraint
                .source_selection
                .map(|selection| selection.source_span)
                .filter(|span| span.span.start != span.span.end);
            let matches = select_domain_candidates(
                source,
                program,
                carrier,
                authored_name,
                reference,
                domain_constraint.symbol,
                &domain_constraint.arguments,
            );
            let [domain] = matches.as_slice() else {
                normalized.push(TypeConstraintNode::Domain(domain_constraint));
                continue;
            };

            if let Some(authored) = domain_constraint.source_selection
                && let Some(exposure) = authored.authored_exposure
            {
                program
                .record_resolved_authored_declaration_selection_once(
                    authored.source_span,
                    exposure,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainMembership,
                    domain.symbol,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain authored domain-constraint selection: {error:?}"
                    ))
                    .with_source_span(authored.source_span)
                })?;
            }

            let index_parameters = typed_trees::domain::index_parameters(program, domain);
            if domain_constraint.arguments.len() != index_parameters.len() {
                return Err(Diagnostic::error(format!(
                    "domain family `{}` requires {} closed index argument(s), but {} were supplied",
                    domain.name,
                    index_parameters.len(),
                    domain_constraint.arguments.len()
                )));
            }
            let instance_name = typed_trees::domain::indexed_domain_instance_name(
                program,
                domain,
                index_parameters,
                &domain_constraint.arguments,
            )?;
            let semantic_id = if domain_constraint.arguments.is_empty() {
                domain.semantic_id
            } else {
                program.semantic_domains.intern(&instance_name)
            };
            let semantic_roles = language_semantics::DomainSemanticRoles {
                denotation_dimension: domain
                    .semantic_roles
                    .denotation_dimension
                    .map(|_| semantic_id),
                arithmetic_policy: domain.semantic_roles.arithmetic_policy.map(|_| semantic_id),
            };

            if domain.alias.is_none() {
                normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                    name: domain_constraint.name,
                    arguments: domain_constraint.arguments,
                    subject: DomainConstraintSubject::Declared,
                    symbol: domain.symbol,
                    semantic_id,
                    classification: domain.classification,
                    predicate_presence: domain.predicate_presence,
                    semantic_roles,
                    establishment_routes: domain.establishment_routes.clone(),
                    source_selection: None,
                }));
                continue;
            }

            expanded_alias = true;
            let source_domain = source
                .domain_definition_by_symbol(domain.symbol)
                .expect("typed alias declaration must have a resolved source");
            for atom in
                expand_domain_reference(source, domain.symbol, vec![source_domain.name.clone()])?
            {
                let name = if atom.symbol.is_valid() {
                    atom.path
                        .last()
                        .map(crate::lowerer::name::lower_name)
                        .unwrap_or_else(|| Identifier::generated(""))
                } else {
                    Identifier::generated(atom.path.joined_path())
                };
                let declaration = domain_by_symbol(program, atom.symbol);
                let subject = if atom.symbol.is_valid() {
                    DomainConstraintSubject::Declared
                } else {
                    language_semantics::CarryPermission::from_name(name.as_str())
                        .map(DomainConstraintSubject::Carry)
                        .unwrap_or_default()
                };
                normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                    name,
                    arguments: Vec::new(),
                    subject,
                    symbol: atom.symbol,
                    semantic_id: declaration
                        .map(|domain| domain.semantic_id)
                        .unwrap_or_default(),
                    classification: declaration.and_then(|domain| domain.classification),
                    predicate_presence: declaration
                        .map(|domain| domain.predicate_presence)
                        .unwrap_or_default(),
                    semantic_roles: declaration
                        .map(|domain| domain.semantic_roles)
                        .unwrap_or_default(),
                    establishment_routes: declaration
                        .map(|domain| domain.establishment_routes.clone())
                        .unwrap_or_default(),
                    source_selection: None,
                }));
            }
        }

        if expanded_alias {
            let normalized = program.type_reference_table.insert_constraints(normalized);
            program
                .type_reference_table
                .set_constraint_span(site, normalized);
        } else {
            for (target, value) in program
                .type_reference_table
                .constraints_mut(constraints)
                .iter_mut()
                .zip(normalized)
            {
                *target = value;
            }
        }
        Ok(())
    }

    fn spell_domain_range(
        source: &SymbolResolvedTrees,
        definitions: &[DomainDefinition],
        index: &mut typed_trees::declaration_index::DomainCandidateIndex,
        range: std::ops::Range<usize>,
    ) {
        for position in range {
            let domain = &definitions[position];
            let qualified = source.symbols.display_path(domain.symbol, "::");
            index
                .by_qualified
                .entry(qualified.into_boxed_str())
                .or_default()
                .push(position as u32);
            let local = domain.name.as_str();
            let mut spellings = vec![local.to_owned()];
            if let Some((_, members)) = local.split_once("::") {
                spellings.push(members.to_owned());
            }
            let leaf = local.rsplit("::").next().unwrap_or(local);
            if leaf != local {
                spellings.push(leaf.to_owned());
            }
            spellings.sort_unstable();
            spellings.dedup();
            for spelling in spellings {
                index
                    .by_relative
                    .entry(spelling.into_boxed_str())
                    .or_default()
                    .push(position as u32);
            }
        }
    }

    fn with_domain_candidate_index<R>(
        source: &SymbolResolvedTrees,
        program: &TypedTrees,
        reader: impl FnOnce(&typed_trees::declaration_index::DomainCandidateIndex) -> R,
    ) -> R {
        let definitions = program.domain_definitions();
        program.with_domain_candidate_index(
            |index| spell_domain_range(source, definitions, index, 0..definitions.len()),
            reader,
        )
    }

    pub(crate) fn select_domain_candidates(
        source: &SymbolResolvedTrees,
        program: &TypedTrees,
        carrier: typed_trees::types::TypeReferenceHandle,
        authored_name: &str,
        reference: Option<source::SourceSpan>,
        retained: symbols::SymbolHandle,
        arguments: &[typed_trees::types::TypeReferenceHandle],
    ) -> Vec<typed_trees::domain::DomainDefinition> {
        select_domain_candidates_where(
            source,
            program,
            authored_name,
            reference,
            retained,
            |domain| {
                typed_trees::domain::domain_accepts_carrier(program, domain, carrier, arguments)
            },
        )
    }

    /*
        The same visible-name selection with the carrier test supplied by the
        caller. A domain cast on an anonymous literal has no operand carrier to
        test against: there the one visible domain of that name supplies it.
    */
    pub(crate) fn select_domain_candidates_where(
        source: &SymbolResolvedTrees,
        program: &TypedTrees,
        authored_name: &str,
        reference: Option<source::SourceSpan>,
        retained: symbols::SymbolHandle,
        accepts: impl Fn(&DomainDefinition) -> bool,
    ) -> Vec<typed_trees::domain::DomainDefinition> {
        let selected = reference
            .and_then(|span| {
                source.symbols.find_top_level_by_name_and_kinds_from_source(
                    authored_name,
                    &[symbols::SymbolKind::Domain],
                    span,
                )
            })
            .or_else(|| (reference.is_none() && retained.is_valid()).then_some(retained));
        let definitions = program.domain_definitions();
        let visible = |domain: &DomainDefinition| {
            source
                .symbols
                .source_reference_can_see_symbol(reference.unwrap_or_default(), domain.symbol)
        };
        let matches = if let Some(symbol) = selected {
            domain_by_symbol(program, symbol)
                .filter(|domain| visible(domain) && accepts(domain))
                .cloned()
                .into_iter()
                .collect()
        } else {
            with_domain_candidate_index(source, program, |index| {
                let mut merged = Vec::new();
                if let Some(bucket) = index.by_qualified.get(authored_name) {
                    merged.extend_from_slice(bucket);
                }
                if let Some(bucket) = index.by_relative.get(authored_name) {
                    merged.extend_from_slice(bucket);
                }
                merged.sort_unstable();
                merged.dedup();
                merged
                    .into_iter()
                    .filter_map(|position| {
                        let domain = &definitions[position as usize];
                        (visible(domain)
                            && reference.is_some_and(|reference| {
                                source.symbols.domain_name_reaches(
                                    domain.symbol,
                                    authored_name,
                                    reference,
                                )
                            })
                            && accepts(domain))
                        .then(|| domain.clone())
                    })
                    .collect()
            })
        };
        prefer_local_domain(source, matches, reference)
    }

    fn prefer_local_domain(
        source: &SymbolResolvedTrees,
        candidates: Vec<typed_trees::domain::DomainDefinition>,
        reference: Option<source::SourceSpan>,
    ) -> Vec<typed_trees::domain::DomainDefinition> {
        if candidates.len() <= 1 {
            return candidates;
        }
        let Some(reference) = reference else {
            return candidates;
        };
        let reference_module = source.symbols.source_module(reference.source_id);
        if reference_module.is_valid() {
            let local = candidates
                .iter()
                .filter(|domain| source.symbols.symbol_module(domain.symbol) == reference_module)
                .cloned()
                .collect::<Vec<_>>();
            if !local.is_empty() {
                return local;
            }
        }
        let local = candidates
            .iter()
            .filter(|domain| {
                source
                    .symbols
                    .symbol_provenance_source_span(domain.symbol)
                    .is_some_and(|span| source.symbols.same_source_package(reference, span))
            })
            .cloned()
            .collect::<Vec<_>>();
        if local.is_empty() { candidates } else { local }
    }
}
#[cfg(test)]
#[path = "type_reference_domain_exposure_tests.rs"]
mod domain_exposure_tests;

use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
use symbol_resolved_trees as resolved;
use symbols::SymbolHandle;
use symbols::SymbolKind;
use typed_trees as typed;

pub(crate) mod alias_uses {
    use diagnostics::Diagnostic;
    use source::SourceSpan;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    pub(crate) fn retain_application(
        source: &resolved::SymbolResolvedTrees,
        target: &mut typed::TypedTrees,
        expansion: typed::types::TypeReferenceHandle,
        application: typed::types::TypeReferenceHandle,
        declaration_source: SourceSpan,
        reference: SourceSpan,
    ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
        use typed::types::TypeReferenceNode;
        let (declaration, arguments, lifetime_arguments) =
            match target.type_reference_table.type_reference(application) {
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    lifetime_arguments,
                    ..
                } => (*base_symbol, *arguments, lifetime_arguments.clone()),
                TypeReferenceNode::Named { symbol, .. } => {
                    (*symbol, arena::HandleSpan::empty(), Vec::new())
                }
                _ => {
                    return Err(Diagnostic::error(
                        "type alias application lost its declaration head",
                    ));
                }
            };
        if source.symbols.get(declaration).kind != symbols::SymbolKind::TypeAlias
            || source.symbols.symbol_source_span(declaration) != Some(declaration_source)
        {
            return Err(Diagnostic::error(
                "type alias application does not rejoin its selected source declaration",
            )
            .with_source_span(reference));
        }
        let supplied_operand_owner = target
            .type_reference_table
            .operand_lifetime_owner(application);
        let instance = target.type_reference_table.fresh_occurrence(expansion);
        target.type_reference_table.retain_type_alias_application(
            typed::type_aliases::TypeAliasApplication {
                instance,
                application,
                declaration,
                declaration_source,
                reference,
                arguments,
                lifetime_arguments,
                supplied_operand_owner,
            },
        );
        Ok(instance)
    }
}
pub(crate) mod arithmetic_results {
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::BuiltinTypeAtom;
    use typed_trees::TypedTrees;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn retain_arithmetic_result_type(
        source_symbols: &symbols::SymbolTable,
        program: &mut TypedTrees,
        reference: TypeReferenceHandle,
        cast_domain: Option<ArithmeticDomain>,
    ) {
        let mut current = reference;
        let mut domain = None;
        let mut visited = Vec::new();
        let carrier = loop {
            if !program
                .type_reference_table
                .contains_type_reference(current)
                || visited.contains(&current)
            {
                return;
            }
            visited.push(current);
            match program.type_reference_table.type_reference(current) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    let Some(constraints) =
                        program.type_reference_table.constraint_span(*constraints)
                    else {
                        return;
                    };
                    for constraint in constraints {
                        match constraint {
                            TypeConstraintNode::ArithmeticDomain(policy) if domain.is_none() => {
                                domain = Some(*policy);
                            }
                            _ => return,
                        }
                    }
                    current = *base_type;
                }
                TypeReferenceNode::Named { symbol, .. } => break *symbol,
                _ => return,
            }
        };
        if !matches!(
            source_symbols.builtin_type_atom(carrier),
            Some(BuiltinTypeAtom::Integer(_) | BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64)
        ) {
            return;
        }
        if let Some(cast_policy) = cast_domain
            && cast_policy != ArithmeticDomain::Exact
            && domain.is_none()
            && reference != current
            && program
                .type_reference_table
                .find_policy_qualified_type_reference(reference, cast_policy)
                .is_none()
        {
            let constraints = program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::ArithmeticDomain(cast_policy)]);
            program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: reference,
                    constraints,
                });
        }
        let domain = cast_domain.unwrap_or(domain.unwrap_or(ArithmeticDomain::Exact));
        if program
            .type_reference_table
            .find_arithmetic_result_type_reference(carrier, domain)
            .is_some()
        {
            return;
        }
        if domain != ArithmeticDomain::Exact {
            let constraints = program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::ArithmeticDomain(domain)]);
            program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: current,
                    constraints,
                });
        }
    }
}
pub(crate) mod computed_results {
    use typed_trees::TypedTrees;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn retain_computed_result_types(
        source_symbols: &symbols::SymbolTable,
        program: &mut TypedTrees,
        reference: TypeReferenceHandle,
        cast_domain: Option<numerics::arithmetic::ArithmeticDomain>,
    ) {
        super::retain_arithmetic_result_type(source_symbols, program, reference, cast_domain);
        program
            .type_reference_table
            .retain_collection_result_types(source_symbols, reference);
    }
}
pub(crate) mod const_arguments {
    use diagnostics::Diagnostic;
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::name::Identifier;
    use typed_trees::typed_trees::canonical_const_declaration_value;
    use typed_trees::types::TypeReferenceNode;

    pub(crate) mod closed_applications {
        use diagnostics::Diagnostic;
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use typed_trees::TypedTrees;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::name::Identifier;
        use typed_trees::types::TypeReferenceNode;

        pub(crate) fn normalize(
            program: &mut TypedTrees,
            frontier: usize,
        ) -> Result<(), Diagnostic> {
            let mut applications = program
                .type_reference_table
                .type_reference_rows()
                .filter_map(|(reference, node)| {
                    if reference.arena_index() as usize <= frontier {
                        return None;
                    }
                    let TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } = node
                    else {
                        return None;
                    };
                    Some((
                        *base_symbol,
                        program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                            .to_vec(),
                        program
                            .type_reference_table
                            .operand_lifetime_owner(reference),
                    ))
                })
                .collect::<Vec<_>>();
            applications.extend(program.conformances().iter().map(|declaration| {
                (
                    declaration.trait_symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(declaration.arguments)
                        .to_vec(),
                    declaration.symbol,
                )
            }));
            for machine in program.machines() {
                applications.extend(program.machine_trait_conformances(machine).iter().map(
                    |edge| {
                        (
                            edge.symbol,
                            program
                                .type_reference_table
                                .type_reference_handles(edge.arguments)
                                .to_vec(),
                            machine.symbol,
                        )
                    },
                ));
            }
            for declaration in program.traits() {
                applications.extend(
                    program
                        .trait_requirements(declaration)
                        .iter()
                        .map(|parent| {
                            (
                                parent.symbol,
                                program
                                    .type_reference_table
                                    .type_reference_handles(parent.arguments)
                                    .to_vec(),
                                declaration.symbol,
                            )
                        }),
                );
                if let Some(base) = &declaration.refines {
                    applications.push((
                        base.symbol,
                        program
                            .type_reference_table
                            .type_reference_handles(base.arguments)
                            .to_vec(),
                        declaration.symbol,
                    ));
                }
            }
            let bounds = program
                .machines()
                .iter()
                .map(|item| (item.symbol, &item.conformance_bounds))
                .chain(
                    program
                        .traits()
                        .iter()
                        .map(|item| (item.symbol, &item.conformance_bounds)),
                )
                .chain(
                    program
                        .data_definitions()
                        .iter()
                        .map(|item| (item.symbol, &item.conformance_bounds)),
                )
                .chain(
                    program
                        .type_aliases()
                        .iter()
                        .map(|item| (item.symbol, &item.conformance_bounds)),
                );
            for (owner, bounds) in bounds {
                applications.extend(bounds.iter().map(|bound| {
                    let owner = if bound.binder.is_valid() {
                        program.symbols.get(bound.binder).parent
                    } else {
                        owner
                    };
                    (bound.trait_symbol, bound.arguments.clone(), owner)
                }));
            }
            for (declaration, arguments, owner) in applications {
                if let Some(sort) = universe_sort(program, declaration) {
                    // `core::Type<u>` and `core::Strict<u>` apply one universe
                    // level, which is not a constant argument: it has no
                    // carrier and no value to admit. Typing owns the arity;
                    // the level grammar and its constraints belong to sort
                    // formation in the proof kernel.
                    if arguments.len() != 1 {
                        return Err(Diagnostic::error(format!(
                            "`{}` takes exactly one universe level, found {} arguments",
                            sort.name(),
                            arguments.len()
                        )));
                    }
                    continue;
                }
                let has_closed_demand = arguments.iter().any(|argument| {
                    argument.arena_index() as usize > frontier
                        && needs_admission(program, *argument)
                });
                let Some(parameters) = parameter_declarations(program, declaration)
                    .filter(|parameters| parameters.len() == arguments.len())
                else {
                    if has_closed_demand {
                        return Err(crate::unavailable_judgment(
                            "closed const argument declaration custody",
                        ));
                    }
                    continue;
                };
                // This is retained supplying scope, not const admission. It
                // also belongs to ordinary type arguments with no judgment.
                for argument in &arguments {
                    if argument.arena_index() as usize > frontier
                        && owner.is_valid()
                        && !program
                            .type_reference_table
                            .operand_lifetime_owner(*argument)
                            .is_valid()
                    {
                        program
                            .type_reference_table
                            .retain_operand_lifetime_owner(*argument, owner)
                            .map_err(Diagnostic::error)?;
                    }
                }
                for (parameter, argument) in parameters.iter().zip(arguments.iter().copied()) {
                    if argument.arena_index() as usize <= frontier {
                        continue;
                    }
                    let TypeParameterKind::Const { type_reference } = parameter.kind else {
                        continue;
                    };
                    let TypeReferenceNode::ConstExpression(expression) =
                        *program.type_reference_table.type_reference(argument)
                    else {
                        if needs_admission(program, argument) {
                            // A record or case value: whether it is a value
                            // of the declared data type is typing's question,
                            // and the check that answered it is not rebuilt.
                            return Err(Diagnostic::error(
                                "no product route yet: a record or case const argument is not \
                                 typed against its declared data type",
                            ));
                        }
                        continue;
                    };
                    if !needs_admission(program, argument) {
                        continue;
                    }
                    let Some(carrier) =
                        builtin_carrier(program, type_reference, &parameters, &arguments)
                    else {
                        continue;
                    };
                    let Some(spelling) =
                        literal_in_carrier(program, declaration, parameter, expression, carrier)?
                    else {
                        continue;
                    };
                    program.type_reference_table.substitute_node(
                        argument,
                        TypeReferenceNode::Named {
                            symbol: symbols::SymbolHandle::invalid(),
                            name: Identifier::generated(spelling),
                        },
                    );
                }
            }
            Ok(())
        }

        /// The builtin scalar a const parameter is declared in.
        enum Carrier {
            Integer(numerics::literals::LandedIntegerType),
            Boolean,
        }

        /// The builtin integer or `bool` that `carrier` names, directly or as
        /// the type this application supplies for a type parameter of the
        /// same header (`<T, const N: T>` applied at `<u8, 4>`).
        ///
        /// `None` for every other carrier. A carrier with a domain or an
        /// arithmetic policy asks whether the value is a member, which is a
        /// proof question, and an alias or data carrier asks for a structured
        /// value; neither is answered by reading a literal, so the argument
        /// stays as written.
        fn builtin_carrier(
            program: &TypedTrees,
            carrier: typed_trees::types::TypeReferenceHandle,
            parameters: &[typed_trees::data::TypeParameter],
            arguments: &[typed_trees::types::TypeReferenceHandle],
        ) -> Option<Carrier> {
            use numerics::literals::LandedIntegerType as Landed;
            use symbols::BuiltinTypeAtom as Atom;
            let named = |reference| match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => Some(*symbol),
                _ => None,
            };
            let mut symbol = named(carrier)?;
            if let Some(supplied) = parameters.iter().position(|parameter| {
                parameter.symbol == symbol && matches!(parameter.kind, TypeParameterKind::Type)
            }) {
                symbol = named(arguments[supplied])?;
            }
            Some(match program.symbols.builtin_type_atom(symbol)? {
                Atom::Bool => Carrier::Boolean,
                Atom::Integer(carrier) => Carrier::Integer(Landed::Fixed(carrier)),
                _ => return None,
            })
        }

        /// The canonical spelling of a literal argument that has the
        /// parameter's declared type, an error when the literal has another
        /// type or lies outside the carrier, and `None` when the argument is
        /// not a literal.
        ///
        /// Resolution folds a closed scalar argument to its literal, so a
        /// remaining operator, name or call is work for an evaluator and is
        /// left as written for it. numeric_values.md: an anonymous literal
        /// lands in the expected carrier when its exact value is in range,
        /// and a typed literal keeps the carrier it names.
        fn literal_in_carrier(
            program: &TypedTrees,
            declaration: symbols::SymbolHandle,
            parameter: &typed_trees::data::TypeParameter,
            expression: typed_trees::expression::ExpressionHandle,
            carrier: Carrier,
        ) -> Result<Option<String>, Diagnostic> {
            use typed_trees::expression::ExpressionNode;
            let refuse = |found: String| {
                let expected = match carrier {
                    Carrier::Integer(landed) => landed.name(),
                    Carrier::Boolean => "bool".to_owned(),
                };
                Diagnostic::error(format!(
                    "const argument for `{}` of `{}` must be a `{expected}`, found {found}",
                    parameter.name.as_str(),
                    program.symbols.name(declaration),
                ))
                .with_source_span(program.expression_table.source_span(expression))
            };
            match (program.expression_table.expression(expression), &carrier) {
                (ExpressionNode::Integer(literal), Carrier::Integer(landed)) => {
                    if let Some(landing) = literal.landing() {
                        if landing.landed_type != *landed {
                            return Err(refuse(format!(
                                "a `{}` literal",
                                landing.landed_type.name()
                            )));
                        }
                        if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                            return Ok(None);
                        }
                    }
                    let value = literal
                        .value_i64()
                        .map(i128::from)
                        .or_else(|| literal.value_u64().map(i128::from));
                    match value {
                        Some(value) if landed.admits_i128(value) => Ok(Some(value.to_string())),
                        _ => Err(refuse(format!(
                            "`{}`, which is outside that type",
                            literal.canonical_spelling().as_str()
                        ))),
                    }
                }
                (ExpressionNode::Boolean(value), Carrier::Boolean) => Ok(Some(value.to_string())),
                (ExpressionNode::Integer(_), Carrier::Boolean) => {
                    Err(refuse("an integer literal".to_owned()))
                }
                (ExpressionNode::Boolean(_), Carrier::Integer(_)) => {
                    Err(refuse("a Boolean literal".to_owned()))
                }
                _ => Ok(None),
            }
        }

        /// The fixed sort `declaration` is, when it is one of the two that
        /// apply a universe level. Only the compiler-installed slot counts.
        fn universe_sort(
            program: &TypedTrees,
            declaration: symbols::SymbolHandle,
        ) -> Option<symbols::FixedCoreDeclaration> {
            program
                .symbols
                .fixed_core_declaration_for_symbol(declaration)
                .filter(|sort| {
                    matches!(
                        sort,
                        symbols::FixedCoreDeclaration::Type | symbols::FixedCoreDeclaration::Strict
                    )
                })
        }

        fn parameter_declarations(
            program: &TypedTrees,
            declaration: symbols::SymbolHandle,
        ) -> Option<Vec<typed_trees::data::TypeParameter>> {
            fn unique<T>(mut rows: impl Iterator<Item = T>) -> Option<T> {
                let row = rows.next()?;
                rows.next().is_none().then_some(row)
            }
            if !declaration.is_valid() {
                return None;
            }
            match program.symbols.get(declaration).kind {
                symbols::SymbolKind::Data => {
                    let data = program.unique_data_definition_by_symbol(declaration)?;
                    Some(program.data_type_parameters(data).to_vec())
                }
                symbols::SymbolKind::Trait => {
                    let interface = unique(
                        program
                            .traits()
                            .iter()
                            .filter(|row| row.symbol == declaration),
                    )?;
                    Some(program.trait_type_parameters(interface).to_vec())
                }
                symbols::SymbolKind::Conformance => {
                    let implementation = unique(
                        program
                            .conformances()
                            .iter()
                            .filter(|row| row.symbol == declaration),
                    )?;
                    Some(program.conformance_type_parameters(implementation).to_vec())
                }
                symbols::SymbolKind::TypeAlias => {
                    let alias = unique(
                        program
                            .type_aliases()
                            .iter()
                            .filter(|row| row.symbol == declaration),
                    )?;
                    program
                        .tables
                        .data_type_parameters
                        .span(alias.type_parameters)
                        .map(<[_]>::to_vec)
                }
                _ => None,
            }
        }

        fn needs_admission(
            program: &TypedTrees,
            argument: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            match program.type_reference_table.type_reference(argument) {
                TypeReferenceNode::ConstExpression(expression) => {
                    !program.pending_const_range_endpoints.contains(expression)
                        && typed_trees::domain::index_argument_is_closed(program, argument)
                }
                TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                    CanonicalConstValue::from_atom(name.as_str()).is_some_and(|value| {
                        matches!(
                            value.decode_encoding(),
                            Some(
                                DecodedCanonicalConstValue::Record { .. }
                                    | DecodedCanonicalConstValue::Variant { .. }
                            )
                        )
                    })
                }
                _ => false,
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use numerics::{
                arithmetic::ArithmeticDomain,
                literals::{IntegerLanding, IntegerLiteral, LandedIntegerType},
            };
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::{
                constant::ConstDeclaration,
                data::TypeParameter,
                expression::{
                    BinaryOperator, Expression, ExpressionNode, NamePath, TableBinaryExpression,
                },
                trait_definition::Conformance,
                type_aliases::TypeAliasDefinition,
            };

            #[test]
            fn empty_const_admission_needs_no_unavailable_judgment() {
                normalize(&mut TypedTrees::default(), 0).unwrap();
            }

            /// A program holding `base<level, ...>` with `count` closed
            /// integer arguments, where `base` is either the installed
            /// `core::Strict` slot or an authored definition spelled the same.
            fn sort_application(
                installed: bool,
                count: usize,
            ) -> (TypedTrees, Vec<typed_trees::types::TypeReferenceHandle>) {
                let mut builder = SymbolTableBuilder::new();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                builder.insert_children(
                    root,
                    symbols::builtin_type_symbols()
                        .into_iter()
                        .chain(symbols::builtin_function_symbols())
                        .chain(symbols::fixed_core_declaration_symbols())
                        .chain([(
                            SymbolKind::MathematicalDefinition,
                            SymbolNameRef::Static("core::Strict"),
                        )]),
                );
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let base = if installed {
                    program
                        .symbols
                        .fixed_core_declaration_symbol(symbols::FixedCoreDeclaration::Strict)
                        .unwrap()
                } else {
                    program
                        .symbols
                        .child_handles(program.symbols.root())
                        .unwrap()
                        .last()
                        .unwrap()
                };
                let levels = (0..count)
                    .map(|_| {
                        let level = program
                            .expression_table
                            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::ConstExpression(level))
                    })
                    .collect::<Vec<_>>();
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles(levels.iter().copied());
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: base,
                        base_name: Identifier::generated("core::Strict"),
                        arguments,
                        lifetime_arguments: Vec::new(),
                    });
                (program, levels)
            }

            #[test]
            fn a_universe_level_is_not_a_constant_argument_to_admit() {
                let (mut program, levels) = sort_application(true, 1);
                let authored = program.clone();
                normalize(&mut program, 0).unwrap();
                assert!(matches!(
                    program.type_reference_table.type_reference(levels[0]),
                    TypeReferenceNode::ConstExpression(_)
                ));
                assert_eq!(program, authored);
            }

            #[test]
            fn a_sort_applied_to_no_level_or_two_is_a_typing_error() {
                for count in [0, 2] {
                    let (mut program, _) = sort_application(true, count);
                    let error = normalize(&mut program, 0).unwrap_err();
                    assert!(
                        error
                            .message
                            .contains("`core::Strict` takes exactly one universe level"),
                        "{}",
                        error.message
                    );
                }
            }

            #[test]
            fn a_definition_spelled_like_a_sort_keeps_its_refusal() {
                let (mut program, _) = sort_application(false, 1);
                let error = normalize(&mut program, 0).unwrap_err();
                assert!(error.message.contains("no product route yet"));
            }

            #[test]
            fn ordinary_type_arguments_keep_their_exact_supplier_scope() {
                let mut builder = SymbolTableBuilder::new();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let children = builder.insert_children(
                    root,
                    symbols::builtin_type_symbols().into_iter().chain([
                        (SymbolKind::TypeAlias, SymbolNameRef::Static("Alias")),
                        (SymbolKind::Machine, SymbolNameRef::Static("supplier")),
                    ]),
                );
                let alias = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
                let supplier = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
                let formal = builder
                    .insert_children(
                        alias,
                        [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
                    )
                    .start();
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let argument = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32),
                        name: Identifier::generated("u64"),
                    });
                let parameters = program.data_type_parameters.insert_many([TypeParameter {
                    symbol: formal,
                    name: Identifier::generated("T"),
                    kind: TypeParameterKind::Type,
                    ..Default::default()
                }]);
                program.push_type_alias_definition(TypeAliasDefinition {
                    symbol: alias,
                    name: Identifier::generated("Alias"),
                    type_parameters: parameters,
                    expansion: argument,
                    ..Default::default()
                });
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles([argument]);
                let application = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: alias,
                        base_name: Identifier::generated("Alias"),
                        arguments,
                        lifetime_arguments: Vec::new(),
                    });
                program
                    .type_reference_table
                    .retain_operand_lifetime_owner(application, supplier)
                    .unwrap();
                normalize(&mut program, 0).unwrap();
                assert_eq!(
                    program
                        .type_reference_table
                        .operand_lifetime_owner(argument),
                    supplier
                );
                assert!(
                    matches!(program.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. } if *symbol == children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32))
                );
            }

            #[test]
            fn a_literal_of_the_declared_type_is_admitted_and_the_rest_wait_or_refuse() {
                for (kind, header_count, argument_scope) in [
                    (SymbolKind::TypeAlias, 0, false),
                    (SymbolKind::TypeAlias, 1, false),
                    (SymbolKind::TypeAlias, 2, false),
                    (SymbolKind::TypeAlias, 1, true),
                    (SymbolKind::Conformance, 0, true),
                    (SymbolKind::Conformance, 1, true),
                    (SymbolKind::Conformance, 2, true),
                ] {
                    let mut builder = SymbolTableBuilder::default();
                    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                    let children = builder.insert_children(
                        root,
                        symbols::builtin_type_symbols().into_iter().chain([
                            (kind, SymbolNameRef::Static("Alias")),
                            (SymbolKind::Machine, SymbolNameRef::Static("supplier")),
                            (SymbolKind::Const, SymbolNameRef::Static("ZERO")),
                        ]),
                    );
                    let u64_symbol =
                        children.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32);
                    let alias = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
                    let supplier = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
                    let zero = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
                    let parameter = builder
                        .insert_children(
                            alias,
                            [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
                        )
                        .start();
                    let mut program = TypedTrees {
                        symbols: builder.finish(),
                        ..Default::default()
                    };
                    let carrier = program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol: u64_symbol,
                            name: Identifier::generated("u64"),
                        });
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol: children
                                .handle_at(symbols::BuiltinTypeAtom::U8.ordinal() as u32),
                            name: Identifier::generated("u8"),
                        });
                    let parameters = program.data_type_parameters.insert_many([TypeParameter {
                        symbol: parameter,
                        name: Identifier::generated("N"),
                        kind: TypeParameterKind::Const {
                            type_reference: carrier,
                        },
                        ..Default::default()
                    }]);
                    for _ in 0..header_count {
                        if kind == SymbolKind::Conformance {
                            program.push_conformance(Conformance {
                                symbol: alias,
                                type_parameters: parameters,
                                ..Default::default()
                            });
                        } else {
                            program.push_type_alias_definition(TypeAliasDefinition {
                                symbol: alias,
                                type_parameters: parameters,
                                ..Default::default()
                            });
                        }
                    }
                    let value = program.expression_table.insert(ExpressionNode::Integer(
                        IntegerLiteral::from_value(1).with_landing(IntegerLanding {
                            landed_type: LandedIntegerType::U64,
                            domain: ArithmeticDomain::Exact,
                        }),
                    ));
                    let value = if argument_scope {
                        let materialized_initializer =
                            program.expression_table.insert(ExpressionNode::Integer(
                                IntegerLiteral::from_value(0).with_landing(IntegerLanding {
                                    landed_type: LandedIntegerType::U64,
                                    domain: ArithmeticDomain::Exact,
                                }),
                            ));
                        program.push_const_declaration(ConstDeclaration {
                        symbol: zero,
                        declared_type: carrier,
                        materialized_initializer,
                        evaluated_value: Some(
                            language_core::constant_value::EvaluatedConstant::Encoded(
                                language_semantics::const_value::CanonicalConstIdentity::integer(
                                    "u64", 0,
                                )
                                .encoding,
                            ),
                        ),
                        ..Default::default()
                    });
                        let left = program.expression_table.insert_tree(&Expression::Name(
                            NamePath::resolved(vec![Identifier::generated("ZERO")], zero, zero),
                        ));
                        program.expression_table.insert(ExpressionNode::Binary(
                            TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left,
                                operator: BinaryOperator::Add,
                                right: value,
                            },
                        ))
                    } else {
                        value
                    };
                    let argument = program
                        .type_reference_table
                        .insert(TypeReferenceNode::ConstExpression(value));
                    let arguments = program
                        .type_reference_table
                        .insert_type_reference_handles([argument]);
                    let application =
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Generic {
                                base_symbol: alias,
                                base_name: Identifier::generated("Alias"),
                                arguments,
                                lifetime_arguments: Vec::new(),
                            });
                    program
                        .type_reference_table
                        .retain_operand_lifetime_owner(
                            if argument_scope {
                                argument
                            } else {
                                application
                            },
                            supplier,
                        )
                        .unwrap();
                    let outcome = normalize(&mut program, 0);
                    if header_count == 1 {
                        outcome.unwrap();
                    } else {
                        // With no header or two, typing cannot say which
                        // parameter the argument answers.
                        assert!(
                            outcome
                                .unwrap_err()
                                .message
                                .contains("no product route yet")
                        );
                    }
                    if header_count == 1 && !argument_scope {
                        // The `u64` literal has the declared type: it takes
                        // its canonical spelling.
                        assert!(matches!(
                            program.type_reference_table.type_reference(argument),
                            TypeReferenceNode::Named { symbol, name }
                                if !symbol.is_valid() && name.as_str() == "1"
                        ));
                    } else {
                        // `ZERO + 1` is not a literal: it waits as written
                        // for an evaluator.
                        assert_eq!(
                            program.type_reference_table.type_reference(argument),
                            &TypeReferenceNode::ConstExpression(value)
                        );
                    }
                    assert_eq!(
                        program
                            .type_reference_table
                            .operand_lifetime_owner(if argument_scope {
                                argument
                            } else {
                                application
                            }),
                        supplier
                    );
                    if header_count == 1 {
                        for (literal, found) in [
                            (
                                IntegerLiteral::from_value(1).with_landing(IntegerLanding {
                                    landed_type: LandedIntegerType::U8,
                                    domain: ArithmeticDomain::Exact,
                                }),
                                "must be a `u64`, found a `u8` literal",
                            ),
                            (
                                IntegerLiteral::from_value(-1),
                                "must be a `u64`, found `-1`, which is outside that type",
                            ),
                        ] {
                            let refused = program
                                .expression_table
                                .insert(ExpressionNode::Integer(literal));
                            program.type_reference_table.substitute_node(
                                argument,
                                TypeReferenceNode::ConstExpression(refused),
                            );
                            let error = normalize(&mut program, 0).unwrap_err();
                            assert!(error.message.contains(found), "{}", error.message);
                            assert_eq!(
                                program.type_reference_table.type_reference(argument),
                                &TypeReferenceNode::ConstExpression(refused)
                            );
                        }
                    }
                }
            }

            /// `data Alias<T, const N: carrier>` applied at `<u8, value>`.
            /// The carrier is `T`, `u8 in Wrapping` or `bool`.
            fn applied_alias(
                carrier: &str,
                value: ExpressionNode,
            ) -> (TypedTrees, typed_trees::types::TypeReferenceHandle) {
                let mut builder = SymbolTableBuilder::new();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let children = builder.insert_children(
                    root,
                    symbols::builtin_type_symbols()
                        .into_iter()
                        .chain([(SymbolKind::TypeAlias, SymbolNameRef::Static("Alias"))]),
                );
                let alias = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
                let formals = builder.insert_children(
                    alias,
                    [
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("N")),
                    ],
                );
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let named = |program: &mut TypedTrees, symbol, name: &'static str| {
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol,
                            name: Identifier::generated(name),
                        })
                };
                let atom =
                    |atom: symbols::BuiltinTypeAtom| children.handle_at(atom.ordinal() as u32);
                let u8_type = named(&mut program, atom(symbols::BuiltinTypeAtom::U8), "u8");
                let carrier = match carrier {
                    "T" => named(&mut program, formals.handle_at(0), "T"),
                    "bool" => named(&mut program, atom(symbols::BuiltinTypeAtom::Bool), "bool"),
                    _ => {
                        let constraints = program.type_reference_table.insert_constraints([
                            typed_trees::types::TypeConstraintNode::ArithmeticDomain(
                                ArithmeticDomain::Wrapping,
                            ),
                        ]);
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Constrained {
                                base_type: u8_type,
                                constraints,
                            })
                    }
                };
                let parameters = program.data_type_parameters.insert_many([
                    TypeParameter {
                        symbol: formals.handle_at(0),
                        name: Identifier::generated("T"),
                        kind: TypeParameterKind::Type,
                        ..Default::default()
                    },
                    TypeParameter {
                        symbol: formals.handle_at(1),
                        name: Identifier::generated("N"),
                        kind: TypeParameterKind::Const {
                            type_reference: carrier,
                        },
                        ..Default::default()
                    },
                ]);
                program.push_type_alias_definition(TypeAliasDefinition {
                    symbol: alias,
                    name: Identifier::generated("Alias"),
                    type_parameters: parameters,
                    expansion: u8_type,
                    ..Default::default()
                });
                let value = program.expression_table.insert(value);
                let argument = program
                    .type_reference_table
                    .insert(TypeReferenceNode::ConstExpression(value));
                let supplied = named(&mut program, atom(symbols::BuiltinTypeAtom::U8), "u8");
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles([supplied, argument]);
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: alias,
                        base_name: Identifier::generated("Alias"),
                        arguments,
                        lifetime_arguments: Vec::new(),
                    });
                (program, argument)
            }

            fn integer(value: i64) -> ExpressionNode {
                ExpressionNode::Integer(IntegerLiteral::from_value(value))
            }

            #[test]
            fn a_carrier_naming_a_type_parameter_is_the_type_the_application_supplies() {
                let (mut program, argument) = applied_alias("T", integer(255));
                normalize(&mut program, 0).unwrap();
                assert!(matches!(
                    program.type_reference_table.type_reference(argument),
                    TypeReferenceNode::Named { symbol, name }
                        if !symbol.is_valid() && name.as_str() == "255"
                ));
                let (mut program, _) = applied_alias("T", integer(256));
                let error = normalize(&mut program, 0).unwrap_err();
                assert!(
                    error
                        .message
                        .contains("const argument for `N` of `Alias` must be a `u8`, found `256`"),
                    "{}",
                    error.message
                );
            }

            #[test]
            fn a_boolean_carrier_takes_a_boolean_literal_only() {
                let (mut program, argument) = applied_alias("bool", ExpressionNode::Boolean(true));
                normalize(&mut program, 0).unwrap();
                assert!(matches!(
                    program.type_reference_table.type_reference(argument),
                    TypeReferenceNode::Named { symbol, name }
                        if !symbol.is_valid() && name.as_str() == "true"
                ));
                let (mut program, _) = applied_alias("bool", integer(1));
                let error = normalize(&mut program, 0).unwrap_err();
                assert!(
                    error
                        .message
                        .contains("must be a `bool`, found an integer literal")
                );
            }

            #[test]
            fn membership_in_a_constrained_carrier_is_not_decided_by_typing() {
                // 300 is outside `u8`, and whether it is a member of
                // `u8 in Wrapping` is the checker's question: typing neither
                // accepts nor rejects, and the argument stays unadmitted.
                let (mut program, argument) = applied_alias("u8 in Wrapping", integer(300));
                let authored = program.clone();
                normalize(&mut program, 0).unwrap();
                assert!(matches!(
                    program.type_reference_table.type_reference(argument),
                    TypeReferenceNode::ConstExpression(_)
                ));
                assert_eq!(program, authored);
            }
        }
    }

    pub(crate) fn normalize_const_argument_carriers(
        program: &mut TypedTrees,
        frontier: usize,
    ) -> Result<(), Diagnostic> {
        let mut replacements = Vec::new();
        for (site, symbol, spelling) in program.type_reference_table.named_references() {
            if usize::try_from(site.arena_index()).is_ok_and(|index| index <= frontier) {
                continue;
            }
            let Some(declaration) = program.const_declaration_by_symbol(symbol) else {
                continue;
            };
            let Some(_) = declaration
                .evaluated_value
                .as_ref()
                .and_then(|value| value.encoding())
            else {
                continue;
            };
            let Some(value) = canonical_const_declaration_value(program, symbol) else {
                return Err(Diagnostic::error(
                    "selected constant has no canonical static carrier value",
                ));
            };
            let decoded = value.decode_encoding().ok_or_else(|| {
                Diagnostic::error("selected constant has malformed canonical value encoding")
            })?;
            if !matches!(
                decoded,
                DecodedCanonicalConstValue::Record { .. }
                    | DecodedCanonicalConstValue::Variant { .. }
            ) {
                continue;
            }
            if let Some(retained) = CanonicalConstValue::from_atom(spelling)
                && retained.encoding != value.encoding
            {
                return Err(Diagnostic::error(
                    "structured constant argument differs from its selected declaration value",
                ));
            }
            replacements.push((site, value.atom()));
        }
        for (site, spelling) in replacements {
            program.type_reference_table.substitute_node(
                site,
                TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated(spelling),
                },
            );
        }
        closed_applications::normalize(program, frontier)
    }
}
pub(crate) mod constraints {
    use crate::tables::{type_constraints, type_references};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use symbol_resolved_trees as resolved;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use typed_trees as typed;

    pub(crate) fn lower_type_constraint_node_span_from_table(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        constraints: HandleSpan<resolved::types::TypeConstraintNode>,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
        let mut span = HandleSpan::empty();

        for constraint in type_references(source_trees).constraints(constraints) {
            let constraint = match constraint {
                resolved::types::TypeConstraintNode::Named(name) => {
                    typed::types::TypeConstraintNode::Named(crate::lowerer::name::lower_name(name))
                }
                resolved::types::TypeConstraintNode::Domain(domain) => {
                    let arguments = type_references(source_trees)
                        .type_reference_handles(domain.arguments)
                        .iter()
                        .map(|argument| {
                            super::table::lower_type_reference_handle_from_table_with_context(
                                source_trees,
                                typed_trees,
                                *argument,
                                exposure,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    lower_domain_constraint_node(&domain.name, arguments, exposure)
                }

                resolved::types::TypeConstraintNode::ArithmeticDomain(domain) => {
                    typed::types::TypeConstraintNode::ArithmeticDomain(*domain)
                }
            };
            typed_trees
                .type_reference_table
                .push_constraint(&mut span, constraint);
        }

        Ok(span)
    }

    pub(crate) fn lower_type_constraint_node_span_with_context(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        constraints: HandleSpan<resolved::types::TypeConstraint>,
        exposure: impl Into<Option<AuthoredDeclarationSelectionExposure>>,
    ) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
        let exposure = exposure.into();
        let mut span = HandleSpan::empty();

        for constraint in type_constraints(source_trees).span_or_empty(constraints) {
            let constraint = lower_type_constraint_node_with_context(
                source_trees,
                typed_trees,
                constraint,
                exposure,
            )?;
            typed_trees
                .type_reference_table
                .push_constraint(&mut span, constraint);
        }

        Ok(span)
    }

    fn lower_domain_constraint_node(
        domain_name: &resolved::name::DiagnosticName,
        arguments: Vec<typed::types::TypeReferenceHandle>,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> typed::types::TypeConstraintNode {
        let name = crate::lowerer::name::lower_name(domain_name);
        typed::types::TypeConstraintNode::Domain(typed::types::DomainConstraint {
            subject: classify_domain_constraint_subject(name.as_str(), arguments.len()),
            name,
            arguments,
            source_selection: domain_name.is_source_backed().then(|| {
                typed::types::DomainConstraintSource {
                    source_span: domain_name.source_span(),
                    authored_exposure: exposure,
                }
            }),
            ..Default::default()
        })
    }

    fn lower_type_constraint_node_with_context(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        constraint: &resolved::types::TypeConstraint,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> Result<typed::types::TypeConstraintNode, Diagnostic> {
        match constraint {
            resolved::types::TypeConstraint::Named(name) => Ok(
                typed::types::TypeConstraintNode::Named(crate::lowerer::name::lower_name(name)),
            ),
            resolved::types::TypeConstraint::Domain(domain) => {
                let arguments = source_trees
                    .tables
                    .declarations
                    .child_type_references
                    .span_or_empty(domain.arguments)
                    .iter()
                    .map(|argument| {
                        crate::type_reference::lower_type_reference_into_trees_with_exposure(
                            source_trees,
                            typed_trees,
                            argument,
                            exposure,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(lower_domain_constraint_node(
                    &domain.name,
                    arguments,
                    exposure,
                ))
            }

            resolved::types::TypeConstraint::ArithmeticDomain(domain) => {
                Ok(typed::types::TypeConstraintNode::ArithmeticDomain(*domain))
            }
        }
    }

    pub(crate) fn classify_domain_constraint_subject(
        name: &str,
        argument_count: usize,
    ) -> typed::types::DomainConstraintSubject {
        use typed::types::{DomainConstraintSubject, OmegaLayoutGrammar};

        if argument_count == 0 {
            if let Some(permission) = language_semantics::CarryPermission::from_name(name) {
                return DomainConstraintSubject::Carry(permission);
            }
            if let Some(domain) = language_semantics::value_domain::ValueDomain::from_name(name) {
                return DomainConstraintSubject::Value(domain);
            }
        }
        if name == "OmegaLayout" && argument_count == 1 {
            return DomainConstraintSubject::OmegaLayout {
                grammar: OmegaLayoutGrammar::Derived,
            };
        }
        DomainConstraintSubject::Declared
    }
}
pub(crate) mod direct {
    use diagnostics::Diagnostic;
    use language_semantics::const_value;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
    use symbol_resolved_trees as resolved;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use typed_trees as typed;

    use crate::lowerer::name::{lower_name, lower_names};
    use crate::type_reference::constraints::lower_type_constraint_node_span_with_context;

    /*
        `u<N>` and `i<N>` reach typing only when their width is not a closed
        number: Psi 02 already replaced every closed application by the
        carrier it names. What is left is a width the declaration is generic
        in, which `wiki/spec/language/generics.md` ("Binder forms") admits
        only for a `const` binder: "Value must be statically known within the
        compiled specialization", whereas `N: u64` is "runtime-capable" and
        "a static-only use of a dynamic binder rejects". The name was
        resolved in the scope of the declaration that wrote the type, so a
        runtime value that only shares its spelling with a constant or a
        parameter somewhere else is told apart here by its symbol.

        A width written as an expression is left for the specialization that
        would have to compute it.
    */
    fn integer_family_width(
        source_trees: &SymbolResolvedTrees,
        generic: &resolved::types::GenericTypeReference,
        arguments: &[resolved::types::TypeReference],
    ) -> Result<(), Diagnostic> {
        let Some(sign) = source_trees.symbols.integer_family(generic.base_symbol) else {
            return Ok(());
        };
        let letter = match sign {
            numerics::integer_carrier::IntegerSign::Unsigned => 'u',
            numerics::integer_carrier::IntegerSign::Signed => 'i',
        };
        let [resolved::types::TypeReference::Named { symbol, name, .. }] = arguments else {
            return Ok(());
        };
        let is_const_binder = symbol.is_valid()
            && source_trees.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
            && source_trees
                .tables
                .declarations
                .data_type_parameters
                .iter()
                .any(|(_, parameter)| {
                    parameter.symbol == *symbol
                        && matches!(
                            parameter.kind,
                            resolved::data::TypeParameterKind::Const { .. }
                        )
                });
        /* A declared constant whose value Psi 02 did not close is not a runtime value. */
        let is_constant = symbol.is_valid()
            && source_trees.symbols.get(*symbol).kind == symbols::SymbolKind::Const;
        if is_const_binder || is_constant {
            return Ok(());
        }
        Err(Diagnostic::error(format!(
            "the width of `{letter}<{name}>` is a runtime value: `{name}` is not a constant or a \
             `const` parameter in scope here, and an integer width is known before the program runs"
        ))
        .with_source_span(generic.base_name.source_span()))
    }

    pub(crate) fn lower_type_reference_handle_with_context(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        type_reference: &resolved::types::TypeReference,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
        macro_rules! lower {
            ($reference:expr) => {
                lower_type_reference_handle_with_context(
                    source_trees,
                    typed_trees,
                    $reference,
                    exposure,
                )
            };
        }
        let result: Result<_, Diagnostic> = match type_reference {
            resolved::types::TypeReference::Reference(reference) => {
                let referee = lower!(source_trees.child_type_reference(reference.referee))?;
                Ok(typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Reference {
                        referee,
                        access: reference.access,
                        lifetime: reference.lifetime.as_ref().map(lower_name),
                    },
                ))
            }
            resolved::types::TypeReference::Constrained(constrained) => {
                let base_type = lower!(source_trees.child_type_reference(constrained.base_type))?;
                let constraints = lower_type_constraint_node_span_with_context(
                    source_trees,
                    typed_trees,
                    constrained.constraints,
                    exposure,
                )?;
                Ok(typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    },
                ))
            }
            resolved::types::TypeReference::FixedArray(fixed_array) => {
                let element_type =
                    lower!(source_trees.child_type_reference(fixed_array.element_type))?;
                lower_fixed_array_type(
                    source_trees,
                    typed_trees,
                    element_type,
                    &fixed_array.length,
                    exposure,
                )
            }
            resolved::types::TypeReference::Slice(slice) => {
                let element_type = lower!(source_trees.child_type_reference(slice.element_type))?;
                Ok(typed_trees
                    .type_reference_table
                    .insert(typed::types::TypeReferenceNode::Slice { element_type }))
            }
            resolved::types::TypeReference::Generic(generic) => {
                if generic.arguments.is_empty()
                    && let Some(application) = super::generic_origins::region_application(
                        source_trees,
                        &generic.base_name,
                        generic.base_symbol,
                        &generic.lifetime_arguments,
                    )?
                {
                    lower_type_reference_handle_with_context(
                        source_trees,
                        typed_trees,
                        application,
                        exposure,
                    )
                } else {
                    let source_arguments = source_trees.child_type_references(generic.arguments);
                    integer_family_width(source_trees, generic, source_arguments)?;
                    let mut lowered_arguments = Vec::with_capacity(source_arguments.len());
                    for argument in source_arguments {
                        let argument = lower!(argument)?;
                        lowered_arguments.push(argument);
                    }
                    let arguments = typed_trees
                        .type_reference_table
                        .insert_type_reference_handles(lowered_arguments);

                    super::retain_type_reference_selection(
                        source_trees,
                        typed_trees,
                        &generic.base_name,
                        generic.base_symbol,
                        exposure,
                        AuthoredDeclarationSelectionKind::TypeReference,
                    )?;
                    Ok(typed_trees.type_reference_table.insert(
                        typed::types::TypeReferenceNode::Generic {
                            base_symbol: generic.base_symbol,
                            base_name: lower_name(&generic.base_name),
                            lifetime_arguments: lower_names(&generic.lifetime_arguments),
                            arguments,
                        },
                    ))
                }
            }
            resolved::types::TypeReference::ConstExpression(expression) => {
                let expression = crate::expressions::expression::lower_expression_handle_from_table_with_symbol_substitution(
                Some(source_trees),
                &source_trees.tables.bodies.expressions,
                typed_trees,
                expression.expression,
                None,
            )?;
                Ok(typed_trees
                    .type_reference_table
                    .insert(typed::types::TypeReferenceNode::ConstExpression(expression)))
            }
            resolved::types::TypeReference::DynamicTrait {
                symbol,
                name,
                arguments,
                ..
            } => {
                let source_arguments = source_trees.child_type_references(*arguments);
                let mut lowered_arguments = Vec::with_capacity(source_arguments.len());
                for argument in source_arguments {
                    lowered_arguments.push(lower!(argument)?);
                }
                let arguments = typed_trees
                    .type_reference_table
                    .insert_type_reference_handles(lowered_arguments);
                super::lower_dynamic_trait_type_reference(
                    source_trees,
                    typed_trees,
                    *symbol,
                    name,
                    arguments,
                    exposure,
                )
            }
            resolved::types::TypeReference::Named { symbol, name, .. } => {
                super::lower_named_type_reference(
                    source_trees,
                    typed_trees,
                    *symbol,
                    name,
                    exposure,
                )
            }
            resolved::types::TypeReference::AliasUse {
                expansion,
                application,
                declaration_source,
                ..
            } => {
                let expansion = lower_type_reference_handle_with_context(
                    source_trees,
                    typed_trees,
                    expansion,
                    None,
                )?;
                let reference = match application.as_ref() {
                    resolved::types::TypeReference::Generic(value) => value.base_name.source_span(),
                    resolved::types::TypeReference::Named { name, .. } => name.source_span(),
                    _ => {
                        return Err(Diagnostic::error(
                            "type alias application lost its source declaration head",
                        ));
                    }
                };
                let application = lower!(application)?;
                super::alias_uses::retain_application(
                    source_trees,
                    typed_trees,
                    expansion,
                    application,
                    *declaration_source,
                    reference,
                )
            }
            resolved::types::TypeReference::Unit => Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Unit)),
        };
        let reference = result?;
        super::retain_supplied_operand_owner(
            source_trees,
            typed_trees,
            reference,
            type_reference.supplied_operand_owner(),
            type_reference.operand_lexical_owner(),
        )?;
        super::retain_computed_result_types(&source_trees.symbols, typed_trees, reference, None);
        Ok(reference)
    }

    pub(crate) fn lower_fixed_array_type(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        element_type: typed::types::TypeReferenceHandle,
        length: &resolved::types::FixedArrayLength,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
        let mut origin = None;
        let mut extent_argument = None;
        let length = match length {
            resolved::types::FixedArrayLength::Literal(value) => {
                typed::types::FixedArrayLength::Literal(*value)
            }
            resolved::types::FixedArrayLength::ConstParameter { symbol, name } => {
                if symbol.is_valid()
                    && source_trees.symbols.get(*symbol).kind == symbols::SymbolKind::Const
                {
                    let declaration =
                        source_trees
                            .roots
                            .const_declaration(*symbol)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "array length constant has no resolved declaration",
                                )
                                .with_source_span(name.source_span())
                            })?;
                    super::retain_type_reference_selection(
                        source_trees,
                        typed_trees,
                        name,
                        *symbol,
                        exposure,
                        AuthoredDeclarationSelectionKind::StaticPathSegment,
                    )?;
                    origin = Some(typed::types::TypeConstantOrigin {
                        declaration: *symbol,
                        reference: name.source_span(),
                    });
                    if let Some(evaluated) = &declaration.evaluated_value {
                        let value = evaluated.encoding().and_then(|encoding| {
                            const_value::CanonicalConstValue::new("", encoding, "")
                                .decode_encoding()
                        });
                        let Some(const_value::DecodedCanonicalConstValue::Integer {
                            value, ..
                        }) = value
                        else {
                            return Err(Diagnostic::error(format!(
                                "fixed-array length `{name}` requires an evaluated integer constant"
                            ))
                            .with_source_span(name.source_span()));
                        };
                        let value = usize::try_from(value).map_err(|_| Diagnostic::error(format!(
                        "fixed-array length `{name}` must be nonnegative and fit the supported array extent"
                    )).with_source_span(name.source_span()))?;
                        typed::types::FixedArrayLength::Literal(value)
                    } else {
                        typed::types::FixedArrayLength::ConstParameter {
                            symbol: *symbol,
                            name: lower_name(name),
                        }
                    }
                } else {
                    typed::types::FixedArrayLength::ConstParameter {
                        symbol: *symbol,
                        name: lower_name(name),
                    }
                }
            }
            resolved::types::FixedArrayLength::Expression(value) => {
                let argument = lower_type_reference_handle_with_context(
                    source_trees,
                    typed_trees,
                    source_trees.child_type_reference(*value),
                    exposure,
                )?;
                extent_argument = Some(argument);
                let completed = match typed_trees.type_reference_table.type_reference(argument) {
                    typed::types::TypeReferenceNode::ConstExpression(expression)
                        if typed_trees
                            .expression_table
                            .materialization_source(*expression)
                            .is_valid() =>
                    {
                        match typed_trees.expression_table.expression(*expression) {
                            typed::expression::ExpressionNode::Integer(literal) => literal
                                .value_u64()
                                .and_then(|value| usize::try_from(value).ok()),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                completed.map_or(
                    typed::types::FixedArrayLength::Expression(argument),
                    typed::types::FixedArrayLength::Literal,
                )
            }
            resolved::types::FixedArrayLength::ConstCall { machine, name } => {
                typed::types::FixedArrayLength::ConstCall {
                    machine: *machine,
                    name: typed::name::Identifier::generated({
                        let authored = source_trees.symbols.source_text(name.source_span());
                        if authored.is_empty() {
                            name.as_str()
                        } else {
                            authored
                        }
                    }),
                    source_span: name.source_span(),
                }
            }
        };
        let reference =
            typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                });
        if let Some(argument) = extent_argument {
            typed_trees
                .type_reference_table
                .retain_array_extent_argument(reference, argument)
                .map_err(Diagnostic::error)?;
        }
        typed_trees
            .type_reference_table
            .attach_constant_origins(reference, origin)
            .map_err(Diagnostic::error)?;
        Ok(reference)
    }
}
pub(crate) mod generic_origins {
    use super::{Diagnostic, resolved};
    use arena::{Handle, HandleSpan};
    use resolved::name::DiagnosticName;
    use resolved::types::{FixedArrayLength, TypeConstraint, TypeReference};
    use symbols::{SymbolHandle, SymbolKind};

    pub(crate) fn validate_computed_arguments(
        source: &resolved::SymbolResolvedTrees,
    ) -> Result<(), Diagnostic> {
        if source.tables.types.generic_application_origins.is_empty()
            || (!source
                .tables
                .bodies
                .expressions
                .iter_expressions()
                .any(|(expression, _)| {
                    source
                        .tables
                        .bodies
                        .expressions
                        .materialization_source(expression)
                        .is_valid()
                }))
        {
            return Ok(());
        }
        let mut definitions = source
            .data_definitions
            .iter()
            .inspect(|_| {})
            .collect::<Vec<_>>();
        definitions.sort_by_key(|definition| symbol_key(definition.symbol));
        let mut origins = Vec::new();
        let mut malformed_origin = false;
        for (_, origin) in source.tables.types.generic_application_origins.iter() {
            let references = &source.tables.declarations.child_type_references;
            if !references.is_valid(origin.instance) || !references.is_valid(origin.application) {
                malformed_origin = true;
                break;
            }
            let instance = source
                .tables
                .declarations
                .child_type_references
                .get(origin.instance);
            let (name, symbol) = match instance {
                TypeReference::Named { name, symbol, .. } => (name, *symbol),
                TypeReference::Generic(value) => (&value.base_name, value.base_symbol),
                _ => {
                    malformed_origin = true;
                    break;
                }
            };
            origins.push((name, symbol, origin));
        }
        if origins.is_empty() && malformed_origin {
            if let Some((_, origin)) = source
                .tables
                .types
                .generic_application_origins
                .iter()
                .next()
            {
                match source.child_type_reference(origin.instance) {
                    TypeReference::Named { name, .. } => return Err(mismatch(name)),
                    TypeReference::Generic(value) => return Err(mismatch(&value.base_name)),
                    _ => {}
                }
            }
            return Err(Diagnostic::error(
                "generated instance lost its retained type",
            ));
        }
        let mut uses = origins.iter().collect::<Vec<_>>();
        uses.sort_by_key(|(name, symbol, _)| use_key(name, *symbol));
        for (name, symbol, origin) in &origins {
            let first = uses.partition_point(|(other_name, other_symbol, _)| {
                use_key(other_name, *other_symbol) < use_key(name, *symbol)
            });
            if !std::ptr::eq(uses[first].2, *origin) {
                continue;
            }
            let definition_start = definitions
                .partition_point(|definition| symbol_key(definition.symbol) < symbol_key(*symbol));
            let definition = definitions
                .get(definition_start)
                .filter(|definition| definition.symbol == *symbol)
                .ok_or_else(|| mismatch(name))?;
            if definitions
                .get(definition_start + 1)
                .is_some_and(|next| next.symbol == *symbol)
            {
                return Err(mismatch(name));
            }
            let mut previous = None;
            for (_, _, member) in
                uses[first..]
                    .iter()
                    .take_while(|(other_name, other_symbol, _)| {
                        use_key(other_name, *other_symbol) == use_key(name, *symbol)
                    })
            {
                previous = Some(validate_origin(
                    source,
                    EqualityPhase::Completed,
                    name,
                    member,
                    definition,
                    previous,
                )?);
            }
            if malformed_origin {
                return Err(mismatch(name));
            }
        }
        Ok(())
    }

    fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
        (symbol.arena_index(), symbol.generation())
    }

    fn use_key(name: &DiagnosticName, symbol: SymbolHandle) -> ((u32, u32), usize, usize, usize) {
        let span = name.source_span();
        (
            symbol_key(symbol),
            span.source_id.0,
            span.span.start,
            span.span.end,
        )
    }

    use symbol_resolved_trees::lookup_indexes::GenericOriginIndex;

    fn with_generic_origin_index<R>(
        source: &resolved::SymbolResolvedTrees,
        run: impl FnOnce(&GenericOriginIndex) -> R,
    ) -> R {
        source.with_generic_origin_index(
            |index| {
                let origins = &source.tables.types.generic_application_origins;
                let references = &source.tables.declarations.child_type_references;
                for (handle, origin) in origins.iter() {
                    let instance_symbol = if references.is_valid(origin.instance)
                        && references.is_valid(origin.application)
                    {
                        match references.get(origin.instance) {
                            TypeReference::Named { symbol, .. } => *symbol,
                            TypeReference::Generic(value) => value.base_symbol,
                            _ => {
                                index.malformed = true;
                                break;
                            }
                        }
                    } else {
                        index.malformed = true;
                        break;
                    };
                    index
                        .origins_by_symbol
                        .entry(instance_symbol)
                        .or_default()
                        .push(handle);
                }
                for (position, definition) in source.data_definitions.iter().enumerate() {
                    index
                        .definitions_by_symbol
                        .entry(definition.symbol)
                        .or_default()
                        .push(position);
                }
            },
            run,
        )
    }

    pub(crate) fn application<'source>(
        source: &'source resolved::SymbolResolvedTrees,
        name: &DiagnosticName,
        symbol: SymbolHandle,
    ) -> Result<Option<&'source TypeReference>, Diagnostic> {
        if with_generic_origin_index(source, |index| index.malformed) {
            return application_scanned(source, name, symbol);
        }
        let mut selected = None;
        let mut selected_definition = None;
        for handle in with_generic_origin_index(source, |index| {
            index
                .origins_by_symbol
                .get(&symbol)
                .cloned()
                .unwrap_or_default()
        }) {
            let origin = source.tables.types.generic_application_origins.get(handle);
            let references = &source.tables.declarations.child_type_references;
            let instance = references.get(origin.instance);
            if !instance_identity_matches(instance, symbol, name)? {
                continue;
            }
            let definition = match selected_definition {
                Some(definition) => definition,
                None => {
                    let bucket = with_generic_origin_index(source, |index| {
                        index
                            .definitions_by_symbol
                            .get(&symbol)
                            .cloned()
                            .unwrap_or_default()
                    });
                    let definition = bucket
                        .first()
                        .map(|position| &source.data_definitions[*position])
                        .ok_or_else(|| mismatch(name))?;
                    if bucket.len() > 1 {
                        return Err(mismatch(name));
                    }
                    definition
                }
            };
            selected = Some(validate_origin(
                source,
                EqualityPhase::Declaration,
                name,
                origin,
                definition,
                selected,
            )?);
            selected_definition = Some(definition);
        }
        if selected.is_none()
            && with_generic_origin_index(source, |index| {
                index
                    .definitions_by_symbol
                    .get(&symbol)
                    .is_some_and(|bucket| {
                        bucket.iter().any(|position| {
                            source.data_definitions[*position]
                                .generic_instance
                                .is_some()
                        })
                    })
            })
        {
            return Err(mismatch(name));
        }
        Ok(selected)
    }

    pub(crate) fn region_application<'source>(
        source: &'source resolved::SymbolResolvedTrees,
        name: &DiagnosticName,
        symbol: SymbolHandle,
        lifetimes: &[DiagnosticName],
    ) -> Result<Option<&'source TypeReference>, Diagnostic> {
        let Some(application) = application(source, name, symbol)? else {
            return Ok(None);
        };
        let TypeReference::Generic(original) = application else {
            return Err(mismatch(name));
        };
        if original.base_symbol == symbol
            || original.lifetime_arguments.len() != lifetimes.len()
            || !original
                .lifetime_arguments
                .iter()
                .zip(lifetimes)
                .all(|(original, current)| original.as_str() == current.as_str())
        {
            return Err(mismatch(name));
        }
        Ok(Some(application))
    }

    fn application_scanned<'source>(
        source: &'source resolved::SymbolResolvedTrees,
        name: &DiagnosticName,
        symbol: SymbolHandle,
    ) -> Result<Option<&'source TypeReference>, Diagnostic> {
        let mut selected = None;
        let mut selected_definition = None;
        for (_, origin) in source.tables.types.generic_application_origins.iter() {
            let references = &source.tables.declarations.child_type_references;
            if !references.is_valid(origin.instance) || !references.is_valid(origin.application) {
                return Err(mismatch(name));
            }
            let instance = references.get(origin.instance);
            if !instance_identity_matches(instance, symbol, name)? {
                continue;
            }
            let definition = match selected_definition {
                Some(definition) => definition,
                None => {
                    let mut definitions = source
                        .data_definitions
                        .iter()
                        .inspect(|_| {})
                        .filter(|data| data.symbol == symbol);
                    let definition = definitions.next().ok_or_else(|| mismatch(name))?;
                    if definitions.next().is_some() {
                        return Err(mismatch(name));
                    }
                    definition
                }
            };
            selected = Some(validate_origin(
                source,
                EqualityPhase::Declaration,
                name,
                origin,
                definition,
                selected,
            )?);
            selected_definition = Some(definition);
        }
        if selected.is_none()
            && source.data_definitions.iter().any(|definition| {
                definition.symbol == symbol && definition.generic_instance.is_some()
            })
        {
            return Err(mismatch(name));
        }
        Ok(selected)
    }

    #[derive(Clone, Copy)]
    enum EqualityPhase {
        Declaration,
        Completed,
    }

    fn validate_origin<'source>(
        source: &'source resolved::SymbolResolvedTrees,
        phase: EqualityPhase,
        name: &DiagnosticName,
        origin: &resolved::types::GenericApplicationOrigin,
        definition: &'source resolved::data::DataDefinition,
        selected: Option<&TypeReference>,
    ) -> Result<&'source TypeReference, Diagnostic> {
        let references = &source.tables.declarations.child_type_references;
        let lifetimes = match references.get(origin.instance) {
            TypeReference::Named { .. } => &[][..],
            TypeReference::Generic(value) => value.lifetime_arguments.as_slice(),
            _ => return Err(mismatch(name)),
        };
        let application = references.get(origin.application);
        let TypeReference::Generic(arguments) = application else {
            return Err(mismatch(name));
        };
        if lifetimes.len() != arguments.lifetime_arguments.len()
            || !lifetimes
                .iter()
                .zip(&arguments.lifetime_arguments)
                .all(|(left, right)| left.as_str() == right.as_str())
        {
            return Err(mismatch(name));
        }
        let Some(TypeReference::Generic(shared)) = &definition.generic_instance else {
            return Err(mismatch(name));
        };
        let mut equality = Equality {
            source,
            phase,
            active: Vec::new(),
            failure: None,
            canonical_lifetimes: &definition.lifetime_parameters,
            actual_lifetimes: &arguments.lifetime_arguments,
        };
        if source.symbols.get(arguments.base_symbol).kind != SymbolKind::Data
            || arguments.base_symbol != shared.base_symbol
            || !equality.arguments(arguments.arguments, shared.arguments)
        {
            return Err(equality.failure.take().unwrap_or_else(|| mismatch(name)));
        }
        if let Some(previous) = selected {
            let mut exact = Equality {
                source,
                phase,
                active: Vec::new(),
                failure: None,
                canonical_lifetimes: &[],
                actual_lifetimes: &[],
            };
            if !exact.reference(previous, application) {
                return Err(exact.failure.take().unwrap_or_else(|| mismatch(name)));
            }
        }
        Ok(application)
    }

    fn instance_identity_matches(
        instance: &TypeReference,
        symbol: SymbolHandle,
        name: &DiagnosticName,
    ) -> Result<bool, Diagnostic> {
        let (instance_symbol, instance_name) = match instance {
            TypeReference::Named { symbol, name, .. } => (*symbol, name),
            TypeReference::Generic(value) => (value.base_symbol, &value.base_name),
            _ => return Err(mismatch(name)),
        };
        Ok(instance_symbol == symbol && instance_name.source_span() == name.source_span())
    }

    fn mismatch(name: &DiagnosticName) -> Diagnostic {
        Diagnostic::error(
            "generated generic application does not match its exact retained instance origin",
        )
        .with_source_span(name.source_span())
    }

    struct Equality<'source> {
        source: &'source resolved::SymbolResolvedTrees,
        phase: EqualityPhase,
        active: Vec<(Handle<TypeReference>, Handle<TypeReference>)>,
        failure: Option<Diagnostic>,
        canonical_lifetimes: &'source [DiagnosticName],
        actual_lifetimes: &'source [DiagnosticName],
    }

    impl Equality<'_> {
        fn arguments(
            &mut self,
            left: HandleSpan<TypeReference>,
            right: HandleSpan<TypeReference>,
        ) -> bool {
            let arena = &self.source.tables.declarations.child_type_references;
            if left.count() != right.count()
                || arena.span(left).is_none()
                || arena.span(right).is_none()
            {
                return false;
            }
            (0..left.count())
                .all(|offset| self.child(left.handle_at(offset), right.handle_at(offset)))
        }

        fn child(&mut self, left: Handle<TypeReference>, right: Handle<TypeReference>) -> bool {
            let arena = &self.source.tables.declarations.child_type_references;
            if !arena.is_valid(left)
                || !arena.is_valid(right)
                || self.active.contains(&(left, right))
            {
                return false;
            }
            self.active.push((left, right));
            let equal = self.reference(arena.get(left), arena.get(right));
            self.active.pop();
            equal
        }

        fn reference(&mut self, left: &TypeReference, right: &TypeReference) -> bool {
            match (left, right) {
                (TypeReference::AliasUse { expansion, .. }, right) => {
                    self.reference(expansion, right)
                }
                (left, TypeReference::AliasUse { expansion, .. }) => {
                    self.reference(left, expansion)
                }
                (
                    TypeReference::Named {
                        symbol: left,
                        name: left_name,
                        ..
                    },
                    TypeReference::Named {
                        symbol: right,
                        name: right_name,
                        ..
                    },
                ) => {
                    if left.is_valid() || right.is_valid() {
                        return self.live(*left) && left == right;
                    }
                    use language_semantics::const_value::CanonicalConstValue;
                    if let (Some(left), Some(right)) = (
                        CanonicalConstValue::from_atom(left_name.as_str()),
                        CanonicalConstValue::from_atom(right_name.as_str()),
                    ) {
                        return left == right;
                    }
                    match (
                        left_name.as_str().parse::<i128>(),
                        right_name.as_str().parse::<i128>(),
                    ) {
                        (Ok(left), Ok(right)) => left == right,
                        _ => false,
                    }
                }
                (TypeReference::Reference(left), TypeReference::Reference(right)) => {
                    left.access == right.access
                        && match (&left.lifetime, &right.lifetime) {
                            (Some(left), Some(right)) => self.lifetime(left, right),
                            (None, None) => true,
                            _ => false,
                        }
                        && self.child(left.referee, right.referee)
                }
                (TypeReference::Slice(left), TypeReference::Slice(right)) => {
                    self.child(left.element_type, right.element_type)
                }
                (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
                    let length = match (&left.length, &right.length) {
                        (FixedArrayLength::Literal(left), FixedArrayLength::Literal(right)) => {
                            left == right
                        }
                        (
                            FixedArrayLength::ConstParameter { symbol: left, .. },
                            FixedArrayLength::ConstParameter { symbol: right, .. },
                        ) => self.live(*left) && left == right,
                        (
                            FixedArrayLength::Expression(left),
                            FixedArrayLength::Expression(right),
                        ) => self.child(*left, *right),
                        _ => false,
                    };
                    length && self.child(left.element_type, right.element_type)
                }
                (TypeReference::Generic(left), TypeReference::Generic(right)) => {
                    self.live(left.base_symbol)
                        && left.base_symbol == right.base_symbol
                        && left.lifetime_arguments.len() == right.lifetime_arguments.len()
                        && left
                            .lifetime_arguments
                            .iter()
                            .zip(&right.lifetime_arguments)
                            .all(|(left, right)| self.lifetime(left, right))
                        && self.arguments(left.arguments, right.arguments)
                }
                (TypeReference::Constrained(left), TypeReference::Constrained(right)) => {
                    let constraints = &self.source.tables.types.constraints;
                    let (Some(first), Some(second)) = (
                        constraints.span(left.constraints),
                        constraints.span(right.constraints),
                    ) else {
                        return false;
                    };
                    self.child(left.base_type, right.base_type)
                        && first.len() == second.len()
                        && first.iter().zip(second).all(|(first, second)| {
                            self.constraint(first, second, left.base_type, right.base_type)
                        })
                }
                (
                    TypeReference::DynamicTrait {
                        symbol: left,
                        arguments: left_arguments,
                        ..
                    },
                    TypeReference::DynamicTrait {
                        symbol: right,
                        arguments: right_arguments,
                        ..
                    },
                ) => {
                    self.live(*left)
                        && left == right
                        && self.arguments(*left_arguments, *right_arguments)
                }
                (TypeReference::ConstExpression(left), TypeReference::ConstExpression(right)) => {
                    if !left.expression.is_valid() || !right.expression.is_valid() {
                        return false;
                    }
                    if left == right {
                        return true;
                    }
                    let expressions = &self.source.tables.bodies.expressions;
                    if !expressions
                        .materialization_source(left.expression)
                        .is_valid()
                        || !expressions
                            .materialization_source(right.expression)
                            .is_valid()
                    {
                        return false;
                    }
                    matches!(self.phase, EqualityPhase::Declaration)
                        || self
                            .closed_materialized_value(left)
                            .is_some_and(|left| Some(left) == self.closed_materialized_value(right))
                }
                (TypeReference::Unit, TypeReference::Unit) => true,
                _ => false,
            }
        }

        fn constraint(
            &mut self,
            left: &TypeConstraint,
            right: &TypeConstraint,
            left_carrier: Handle<TypeReference>,
            right_carrier: Handle<TypeReference>,
        ) -> bool {
            match (left, right) {
                (
                    TypeConstraint::ArithmeticDomain(left),
                    TypeConstraint::ArithmeticDomain(right),
                ) => left == right,
                (TypeConstraint::Named(left), TypeConstraint::Named(right)) => self
                    .domain(left, left_carrier)
                    .is_some_and(|symbol| Some(symbol) == self.domain(right, right_carrier)),
                (TypeConstraint::Domain(left), TypeConstraint::Domain(right)) => {
                    self.domain(&left.name, left_carrier).is_some_and(|symbol| {
                        Some(symbol) == self.domain(&right.name, right_carrier)
                    }) && self.arguments(left.arguments, right.arguments)
                }

                _ => false,
            }
        }

        fn closed_materialized_value(
            &mut self,
            source: &resolved::types::ConstExpressionTypeReference,
        ) -> Option<(String, String)> {
            let span = self
                .source
                .tables
                .bodies
                .expressions
                .source_span(source.expression);
            self.failure.get_or_insert_with(|| {
                crate::unavailable_judgment("closed const argument replay").with_source_span(span)
            });
            None
        }

        fn lifetime(&self, actual: &DiagnosticName, canonical: &DiagnosticName) -> bool {
            self.canonical_lifetimes
                .iter()
                .position(|parameter| parameter.as_str() == canonical.as_str())
                .and_then(|ordinal| self.actual_lifetimes.get(ordinal))
                .map_or_else(
                    || actual.as_str() == canonical.as_str(),
                    |expected| actual.as_str() == expected.as_str(),
                )
        }

        fn live(&self, symbol: SymbolHandle) -> bool {
            symbol.is_valid() && self.source.symbols.get(symbol).kind != SymbolKind::Unknown
        }

        fn domain(
            &self,
            name: &DiagnosticName,
            carrier: Handle<TypeReference>,
        ) -> Option<SymbolHandle> {
            let TypeReference::Named {
                symbol: carrier, ..
            } = self.source.child_type_reference(carrier)
            else {
                return None;
            };
            if !self.live(*carrier) {
                return None;
            }
            let qualified = self
                .source
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    name.as_str(),
                    &[SymbolKind::Domain],
                    name.source_span(),
                );
            let mut candidates = self.source.domain_definitions.iter().filter(|domain| {
            (matches!(&domain.target_type, TypeReference::Named { symbol, .. } if symbol == carrier)
                || self.source.data_type_parameters(domain.type_parameters).first().is_some_and(|parameter| {
                    matches!(parameter.kind, resolved::data::TypeParameterKind::Type)
                        && matches!(&domain.target_type, TypeReference::Named { symbol, .. } if *symbol == parameter.symbol)
                }))
                && (qualified == Some(domain.symbol)
                    || (!name.as_str().contains("::")
                        && domain.name.as_str().rsplit("::").next() == Some(name.as_str())))
                && self
                    .source
                    .symbols
                    .source_reference_can_see_symbol(name.source_span(), domain.symbol)
        });
            let selected = candidates.next()?.symbol;
            candidates.next().is_none().then_some(selected)
        }
    }
}
pub(crate) mod table {
    use crate::tables::type_references;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use symbol_resolved_trees as resolved;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use typed_trees as typed;

    use crate::lowerer::name::lower_names;
    use crate::type_reference::constraints::lower_type_constraint_node_span_from_table;
    use crate::type_reference::direct::lower_fixed_array_type;

    fn lower_type_argument_handles(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        arguments: HandleSpan<resolved::types::TypeReferenceHandle>,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> Result<HandleSpan<typed::types::TypeReferenceHandle>, Diagnostic> {
        let mut lowered_arguments = HandleSpan::empty();
        for argument in type_references(source_trees).type_reference_handles(arguments) {
            let argument = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *argument,
                exposure,
            )?;
            typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut lowered_arguments, argument);
        }
        Ok(lowered_arguments)
    }

    pub(crate) fn lower_type_reference_handle_from_table_with_context(
        source_trees: &SymbolResolvedTrees,
        typed_trees: &mut typed::TypedTrees,
        type_reference: resolved::types::TypeReferenceHandle,
        exposure: Option<AuthoredDeclarationSelectionExposure>,
    ) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
        let result: Result<_, Diagnostic> = match type_references(source_trees)
            .type_reference(type_reference)
        {
            resolved::types::TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let referee = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *referee,
                    exposure,
                )?;
                Ok(typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Reference {
                        referee,
                        access: *access,
                        lifetime: lifetime.as_ref().map(crate::lowerer::name::lower_name),
                    },
                ))
            }
            resolved::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_type = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *base_type,
                    exposure,
                )?;
                let constraints = lower_type_constraint_node_span_from_table(
                    source_trees,
                    typed_trees,
                    *constraints,
                    exposure,
                )?;
                Ok(typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    },
                ))
            }
            resolved::types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_type = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *element_type,
                    exposure,
                )?;
                lower_fixed_array_type(source_trees, typed_trees, element_type, length, exposure)
            }
            resolved::types::TypeReferenceNode::Slice { element_type } => {
                let element_type = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *element_type,
                    exposure,
                )?;
                Ok(typed_trees
                    .type_reference_table
                    .insert(typed::types::TypeReferenceNode::Slice { element_type }))
            }
            resolved::types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if arguments.is_empty()
                    && let Some(application) = super::generic_origins::region_application(
                        source_trees,
                        base_name,
                        *base_symbol,
                        lifetime_arguments,
                    )?
                {
                    super::direct::lower_type_reference_handle_with_context(
                        source_trees,
                        typed_trees,
                        application,
                        exposure,
                    )
                } else {
                    let lowered_arguments = lower_type_argument_handles(
                        source_trees,
                        typed_trees,
                        *arguments,
                        exposure,
                    )?;

                    super::retain_type_reference_selection(
                    source_trees,
                    typed_trees,
                    base_name,
                    *base_symbol,
                    exposure,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
                )?;
                    Ok(typed_trees.type_reference_table.insert(
                        typed::types::TypeReferenceNode::Generic {
                            base_symbol: *base_symbol,
                            base_name: crate::lowerer::name::lower_name(base_name),
                            lifetime_arguments: lower_names(lifetime_arguments),
                            arguments: lowered_arguments,
                        },
                    ))
                }
            }
            resolved::types::TypeReferenceNode::ConstExpression(expression) => {
                let expression = crate::expressions::expression::lower_expression_handle_from_table_with_symbol_substitution(
                Some(source_trees),
                &source_trees.tables.bodies.expressions,
                typed_trees,
                *expression,
                None,
            )?;
                Ok(typed_trees
                    .type_reference_table
                    .insert(typed::types::TypeReferenceNode::ConstExpression(expression)))
            }
            resolved::types::TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => {
                let lowered_arguments =
                    lower_type_argument_handles(source_trees, typed_trees, *arguments, exposure)?;
                super::lower_dynamic_trait_type_reference(
                    source_trees,
                    typed_trees,
                    *symbol,
                    name,
                    lowered_arguments,
                    exposure,
                )
            }
            resolved::types::TypeReferenceNode::Named { symbol, name } => {
                super::lower_named_type_reference(
                    source_trees,
                    typed_trees,
                    *symbol,
                    name,
                    exposure,
                )
            }
            resolved::types::TypeReferenceNode::AliasUse {
                expansion,
                application,
                declaration_source,
                ..
            } => {
                let expansion = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *expansion,
                    None,
                )?;
                let reference = match type_references(source_trees).type_reference(*application) {
                    resolved::types::TypeReferenceNode::Generic { base_name, .. } => {
                        base_name.source_span()
                    }
                    resolved::types::TypeReferenceNode::Named { name, .. } => name.source_span(),
                    _ => {
                        return Err(Diagnostic::error(
                            "type alias application lost its source declaration head",
                        ));
                    }
                };
                let application = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *application,
                    exposure,
                )?;
                super::alias_uses::retain_application(
                    source_trees,
                    typed_trees,
                    expansion,
                    application,
                    *declaration_source,
                    reference,
                )
            }
            resolved::types::TypeReferenceNode::Unit => Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Unit)),
        };
        let reference = result?;
        super::retain_supplied_operand_owner(
            source_trees,
            typed_trees,
            reference,
            type_references(source_trees).supplied_operand_owner(type_reference),
            type_references(source_trees).operand_lexical_owner(type_reference),
        )?;
        super::retain_computed_result_types(&source_trees.symbols, typed_trees, reference, None);
        Ok(reference)
    }
}

pub(crate) fn retain_supplied_operand_owner(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    reference: typed::types::TypeReferenceHandle,
    owner: resolved::types::OperandLifetimeOwnerHandle,
    lexical_owner: SymbolHandle,
) -> Result<(), Diagnostic> {
    if lexical_owner.is_valid() {
        typed_trees
            .type_reference_table
            .retain_operand_lifetime_owner(reference, lexical_owner)
            .map_err(Diagnostic::error)?;
    }
    if !owner.is_valid() {
        return Ok(());
    }
    let declaration = source_trees
        .tables
        .types
        .operand_lifetime_owners
        .get(owner)
        .declaration;
    if !declaration.is_valid() {
        return Err(Diagnostic::error(
            "supplied operand has no resolved lifetime owner",
        ));
    }
    typed_trees
        .type_reference_table
        .retain_operand_lifetime_owner(reference, declaration)
        .map_err(Diagnostic::error)
}

pub(crate) use arithmetic_results::retain_arithmetic_result_type;
pub(crate) use computed_results::retain_computed_result_types;
pub(crate) use const_arguments::normalize_const_argument_carriers;
pub(crate) use generic_origins::validate_computed_arguments;

use direct::lower_type_reference_handle_with_context;
use table::lower_type_reference_handle_from_table_with_context;

pub(crate) fn lower_type_reference_into_table(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_into_trees_with_exposure(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
        lowerer.type_reference_exposure,
    )
}

pub(crate) fn lower_return_type(
    lowerer: &mut Lowerer,
    written: &Option<resolved::types::TypeReference>,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    match written {
        Some(type_reference) => lower_type_reference_into_table(lowerer, type_reference),
        None => Ok(typed::types::TypeReferenceHandle::invalid()),
    }
}

pub(crate) fn lower_type_reference_into_trees_with_exposure(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
    exposure: impl Into<Option<AuthoredDeclarationSelectionExposure>>,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_with_context(
        source_trees,
        typed_trees,
        type_reference,
        exposure.into(),
    )
}

pub(crate) fn lower_type_reference_handle_from_table(
    lowerer: &mut Lowerer,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_from_table_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
        Some(AuthoredDeclarationSelectionExposure::PrivateImplementation),
    )
}

pub(crate) fn retain_type_reference_recipe_selection(
    source: &resolved::SymbolResolvedTrees,
    typed: &mut typed::TypedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: SymbolHandle,
    exposure: Option<AuthoredDeclarationSelectionExposure>,
    kind: AuthoredDeclarationSelectionKind,
) -> Result<Option<AuthoredDeclarationSelectionOccurrenceId>, Diagnostic> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionCoordinateError as CoordinateError,
        AuthoredDeclarationSelectionLateBinding as Binding,
    };
    let Some(exposure) = exposure else {
        return Ok(None);
    };
    if !name.is_source_backed() {
        return Ok(None);
    }
    if symbol.is_valid()
        && (source.symbols.builtin_type_atom(symbol).is_some()
            || matches!(
                source.symbols.get(symbol).kind,
                SymbolKind::Parameter
                    | SymbolKind::TypeParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::ConformanceParameter
                    | SymbolKind::PropositionParameter
                    | SymbolKind::PropositionMachineParameter
                    | SymbolKind::Local
            ))
    {
        return Ok(None);
    }
    if symbol.is_valid() {
        retain_type_reference_selection(source, typed, name, symbol, Some(exposure), kind)?;
    }
    match typed
        .authored_declaration_selections()
        .source_coordinate_occurrence(name.source_span(), exposure, kind)
    {
        Ok(occurrence) => Ok(Some(occurrence)),
        Err(CoordinateError::Missing) if !symbol.is_valid() => typed
            .record_late_bound_authored_declaration_selection(
                name.source_span(),
                exposure,
                kind,
                Binding::CheckedStaticPathSegment,
            )
            .map(Some)
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "failed to retain original static type recipe selection: {reason:?}"
                ))
                .with_source_span(name.source_span())
            }),
        Err(reason) => Err(Diagnostic::error(format!(
            "original static type recipe selection has no unique source custody: {reason}"
        ))
        .with_source_span(name.source_span())),
    }
}

pub(crate) fn retain_type_reference_selection(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: SymbolHandle,
    exposure: impl Into<Option<AuthoredDeclarationSelectionExposure>>,
    kind: AuthoredDeclarationSelectionKind,
) -> Result<(), Diagnostic> {
    let Some(exposure) = exposure.into() else {
        return Ok(());
    };
    if !name.is_source_backed()
        || !symbol.is_valid()
        || resolved::types::PrimitiveType::from_name(name.as_str()).is_some()
    {
        return Ok(());
    }
    if let Some(application) = generic_origins::application(source_trees, name, symbol)? {
        lower_type_reference_handle_with_context(
            source_trees,
            typed_trees,
            application,
            Some(exposure),
        )?;
        return Ok(());
    }
    if matches!(
        source_trees.symbols.get(symbol).kind,
        SymbolKind::Parameter
            | SymbolKind::TypeParameter
            | SymbolKind::MachineParameter
            | SymbolKind::ConformanceParameter
            | SymbolKind::PropositionParameter
            | SymbolKind::PropositionMachineParameter
            | SymbolKind::Local
    ) {
        return Ok(());
    }
    typed_trees
        .record_resolved_authored_declaration_selection_once(
            name.source_span(),
            exposure,
            kind,
            symbol,
        )
        .map(|_| ())
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to retain authored type-reference selection: {error:?}"
            ))
            .with_source_span(name.source_span())
        })
}

pub(crate) fn lower_dynamic_trait_type_reference(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    symbol: SymbolHandle,
    name: &resolved::name::DiagnosticName,
    arguments: arena::HandleSpan<typed::types::TypeReferenceHandle>,
    exposure: Option<AuthoredDeclarationSelectionExposure>,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    retain_type_reference_selection(
        source_trees,
        typed_trees,
        name,
        symbol,
        exposure,
        AuthoredDeclarationSelectionKind::TypeReference,
    )?;
    Ok(typed_trees
        .type_reference_table
        .insert(typed::types::TypeReferenceNode::DynamicTrait {
            symbol,
            name: crate::lowerer::name::lower_name(name),
            arguments,
        }))
}

pub(crate) fn lower_named_type_reference(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    symbol: SymbolHandle,
    name: &resolved::name::DiagnosticName,
    exposure: Option<AuthoredDeclarationSelectionExposure>,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    retain_type_reference_selection(
        source_trees,
        typed_trees,
        name,
        symbol,
        exposure,
        AuthoredDeclarationSelectionKind::TypeReference,
    )?;
    let reference =
        typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named {
                symbol,
                name: crate::lowerer::name::lower_name(name),
            });
    if source_trees.const_declaration(symbol).is_some() {
        typed_trees
            .type_reference_table
            .attach_constant_origins(
                reference,
                [typed::types::TypeConstantOrigin {
                    declaration: symbol,
                    reference: name.source_span(),
                }],
            )
            .map_err(Diagnostic::error)?;
    }
    Ok(reference)
}

pub(crate) fn retain_static_path_selection(
    typed_trees: &mut typed::TypedTrees,
    path: &[resolved::name::DiagnosticName],
    symbol: SymbolHandle,
    exposure: impl Into<Option<AuthoredDeclarationSelectionExposure>>,
    context: &str,
) -> Result<(), Diagnostic> {
    retain_path_selection(
        typed_trees,
        path,
        symbol,
        exposure,
        AuthoredDeclarationSelectionKind::StaticPathSegment,
        context,
    )
}

pub(crate) fn retain_path_selection(
    typed_trees: &mut typed::TypedTrees,
    path: &[resolved::name::DiagnosticName],
    symbol: SymbolHandle,
    exposure: impl Into<Option<AuthoredDeclarationSelectionExposure>>,
    kind: AuthoredDeclarationSelectionKind,
    context: &str,
) -> Result<(), Diagnostic> {
    retain_path_selection_occurrence(typed_trees, path, symbol, exposure, kind, context).map(|_| ())
}

pub(crate) fn retain_path_selection_occurrence(
    typed_trees: &mut typed::TypedTrees,
    path: &[resolved::name::DiagnosticName],
    symbol: SymbolHandle,
    exposure: impl Into<Option<AuthoredDeclarationSelectionExposure>>,
    kind: AuthoredDeclarationSelectionKind,
    context: &str,
) -> Result<Option<AuthoredDeclarationSelectionOccurrenceId>, Diagnostic> {
    let Some(exposure) = exposure.into() else {
        return Ok(None);
    };
    if !symbol.is_valid() {
        return Ok(None);
    }
    let authored = path
        .iter()
        .filter(|member| member.is_source_backed())
        .collect::<Vec<_>>();
    let (Some(first), Some(last)) = (authored.first(), authored.last()) else {
        return Ok(None);
    };
    if first.source_span().source_id != last.source_span().source_id {
        return Err(Diagnostic::error(format!(
            "authored {context} path spans more than one source file"
        ))
        .with_source_span(first.source_span()));
    }
    let source_span = source::SourceSpan::new(
        first.source_span().source_id,
        source::Span::new(first.source_span().span.start, last.source_span().span.end),
    );
    typed_trees
        .record_resolved_authored_declaration_selection_once(source_span, exposure, kind, symbol)
        .map(Some)
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to retain authored {context} selection: {error:?}"
            ))
            .with_source_span(source_span)
        })
}
