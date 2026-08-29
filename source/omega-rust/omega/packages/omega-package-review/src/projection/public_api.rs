use super::contracts::*;
use super::evidence::*;
use super::exact_identity::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_public_propositions(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewPropositionShape>>, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

    let mut rows = Vec::new();
    for declaration in compilation
        .propositions()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let (binders, parameter_types) = project_proposition_signature(compilation, declaration)?;
        let nested_source_locations = match (
            &declaration.body,
            declaration.transparent_formula_source_span,
        ) {
            (PropositionBody::Transparent { .. }, Some(source_span)) => {
                vec![ProjectedNestedSourceLocation {
                    source_span,
                    role: PackageReviewSourceLocationRole::PropositionFormula,
                }]
            }
            (PropositionBody::Primitive | PropositionBody::Witness { .. }, None) => Vec::new(),
            (PropositionBody::Transparent { .. }, None) => {
                return Err(vec![Diagnostic::error(format!(
                    "public transparent proposition `{}` has no exact formula source custody",
                    identity.path
                ))]);
            }
            (PropositionBody::Primitive | PropositionBody::Witness { .. }, Some(_)) => {
                return Err(vec![Diagnostic::error(format!(
                    "public non-transparent proposition `{}` retains contradictory formula source custody",
                    identity.path
                ))]);
            }
        };
        let body = match &declaration.body {
            PropositionBody::Primitive | PropositionBody::Witness { .. } => {
                let matching = compilation
                    .facts
                    .proof
                    .proposition_vocabulary
                    .declarations
                    .iter()
                    .filter(|checked| checked.symbol == declaration.symbol)
                    .collect::<Vec<_>>();
                let [checked] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "public proposition `{}` has {} checked declaration rows; expected one",
                        identity.path,
                        matching.len()
                    ))]);
                };
                if !checked.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "public proposition `{}` lost visibility during checked lowering",
                        identity.path
                    ))]);
                }
                match declaration.body {
                    PropositionBody::Primitive => PackageReviewPublicPropositionBody::Primitive,
                    PropositionBody::Witness { evidence } => {
                        let declaration_binders = compilation.proposition_binders(declaration);
                        let binder_symbols = declaration_binders
                            .iter()
                            .enumerate()
                            .map(|(position, binder)| {
                                (binder.symbol, format!("proposition-binder:{position}"))
                            })
                            .collect::<Vec<_>>();
                        PackageReviewPublicPropositionBody::Witness(project_evidence_interface(
                            compilation,
                            evidence,
                            &binder_symbols,
                        )?)
                    }
                    PropositionBody::Transparent { .. } => unreachable!(),
                }
            }
            PropositionBody::Transparent { proposition } => {
                let parameters = compilation.proposition_parameters(declaration);
                let declaration_binders = compilation.proposition_binders(declaration);
                let binder_symbols = declaration_binders
                    .iter()
                    .enumerate()
                    .map(|(position, binder)| {
                        (binder.symbol, format!("proposition-binder:{position}"))
                    })
                    .collect::<Vec<_>>();
                let context = ContractProjectionContext {
                    subject_kind: "public proposition",
                    subject_name: &identity.path,
                    owner: psi_checked_trees::ContractProofFactOwner::Unknown,
                    point: psi_facts::ProgramPoint::Definition {
                        symbol: declaration.symbol,
                    },
                    parameters,
                    domain_symbol: None,
                    data_symbol: None,
                    lifetime_binders: &[],
                };
                let mut visiting = vec![declaration.symbol];
                let expansion = match proposition {
                    PropositionFormula::Application(application) => project_contract_proposition(
                        compilation,
                        &context,
                        &binder_symbols,
                        application,
                        None,
                        &[],
                        &[],
                        &mut visiting,
                        0,
                    )?,
                    PropositionFormula::BooleanExpression(expression) => {
                        PackageReviewContractFact::Expression(project_contract_expression(
                            compilation,
                            &context,
                            &binder_symbols,
                            *expression,
                            None,
                            0,
                        )?)
                    }
                };
                PackageReviewPublicPropositionBody::Transparent(expansion)
            }
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewPropositionShape {
                identity,
                binders,
                parameter_types,
                body,
            },
            declaration: declaration.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

pub(crate) fn project_public_consts(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewConstShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for declaration in compilation
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let Some(canonical_value_encoding) = declaration.canonical_value_encoding.clone() else {
            return Err(vec![Diagnostic::error(format!(
                "public const `{}` has no canonical declaration value",
                identity.path
            ))]);
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewConstShape {
                identity,
                declared_type: review_type_identity_with_binders(
                    compilation,
                    declaration.declared_type,
                    &[],
                )?,
                canonical_value_encoding,
            },
            declaration: declaration.symbol,
            nested_source_locations: vec![ProjectedNestedSourceLocation {
                source_span: declaration.initializer_source_span,
                role: PackageReviewSourceLocationRole::ConstInitializer,
            }],
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

pub(crate) fn project_operator_coordinate(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<PackageReviewOperatorCoordinate, Vec<Diagnostic>> {
    let identity = nominal_identity(compilation, declaration.symbol)?;
    let overload = compilation.normalized_operator_overload_identity(declaration);
    Ok(PackageReviewOperatorCoordinate {
        identity,
        parameter_dispatch: overload.parameters().to_owned(),
        // Only explicitly named boundary requirements participate in
        // expected-result dispatch. Fixed tokens and ordinary named operators
        // remain operand-directed; their complete return type stays in the
        // row value so a change is one changed declaration, not remove/add.
        result_dispatch: if declaration.is_boundary && declaration.spelling.is_none() {
            overload.result_dispatch().identity()
        } else {
            String::new()
        },
    })
}

pub(crate) fn project_public_operators(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewOperatorShape>>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_operator_crash_contracts(
        &compilation.typed,
    );
    if derived != compilation.facts.operators.operator_crash_contracts {
        return Err(vec![Diagnostic::error(format!(
            "retained checked operator-crash evidence does not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation.facts.operators.operator_crash_contracts.len(),
            derived.len(),
        ))]);
    }
    let mut rows = Vec::new();
    let operators = compilation.operators().iter().chain(
        compilation
            .domain_definitions()
            .iter()
            .flat_map(|domain| compilation.domain_operators(domain)),
    );
    for declaration in operators.filter(|declaration| declaration.is_public) {
        let coordinate = project_operator_coordinate(compilation, declaration)?;
        if !reviewed_package_owns(&coordinate.identity, package)? {
            continue;
        }
        let declaration_path = coordinate.identity.path.as_str();
        let declaration_type_parameters = compilation.operator_type_parameters(declaration);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            declaration_type_parameters,
            "operator",
            declaration_path,
            &declaration.lifetime_parameters,
        )?;
        let parameters = compilation
            .operator_parameters(declaration)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewCallableParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        parameter.type_reference,
                        &binders,
                        &declaration.lifetime_parameters,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let context = ContractProjectionContext {
            subject_kind: "public operator",
            subject_name: declaration_path,
            owner: psi_checked_trees::ContractProofFactOwner::OperatorDeclaration {
                operator_symbol: declaration.symbol,
            },
            point: psi_facts::ProgramPoint::Definition {
                symbol: declaration.symbol,
            },
            parameters: compilation.operator_parameters(declaration),
            domain_symbol: None,
            data_symbol: None,
            lifetime_binders: &declaration.lifetime_parameters,
        };
        let contracts = project_contracts(
            compilation,
            compilation.operator_contracts(declaration),
            &context,
            &binders,
        )?;
        let matching_crash = compilation
            .facts
            .operators
            .operator_crash_contracts
            .iter()
            .filter(|checked| checked.operator_symbol() == declaration.symbol)
            .collect::<Vec<_>>();
        let [checked_crash] = matching_crash.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public operator `{declaration_path}` has {} exact checked crash-contract rows; expected one",
                matching_crash.len(),
            ))]);
        };
        let published_crash =
            project_operator_crash_routes(compilation, checked_crash, &context, &binders)?;
        let mut nested_source_locations = project_contract_source_locations(
            compilation,
            compilation.operator_contracts(declaration),
        )?;
        collect_callable_parameter_source_locations(
            compilation,
            compilation.operator_parameters(declaration),
            "public operator parameter",
            &mut nested_source_locations,
        )?;
        collect_type_parameter_source_locations(
            compilation,
            declaration_type_parameters,
            &mut nested_source_locations,
        )?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewOperatorShape {
                coordinate,
                is_boundary: declaration.is_boundary,
                spelling: declaration.spelling,
                lifetime_parameter_count: declaration.lifetime_parameters.len(),
                type_parameters,
                parameters,
                return_type: review_signature_type_identity_with_binders(
                    compilation,
                    declaration.return_type,
                    &binders,
                    &declaration.lifetime_parameters,
                )?,
                contracts,
                published_crash,
            },
            declaration: declaration.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.coordinate.cmp(&right.row.coordinate));
    if rows
        .windows(2)
        .any(|pair| pair[0].row.coordinate == pair[1].row.coordinate)
    {
        return Err(vec![Diagnostic::error(
            "public operator review produced a duplicate overload coordinate",
        )]);
    }
    Ok(rows)
}

pub(crate) fn project_operator_crash_routes(
    compilation: &CheckedCompilation,
    checked: &psi_checked_trees::CheckedOperatorCrashContract,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    use psi_typed_trees::domain::ProofFact;

    checked
        .buckets()
        .iter()
        .map(|bucket| {
            let alternative_guards = if bucket.is_unconditional() {
                if !bucket.facts().is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "public operator `{}` has an unconditional checked crash bucket with retained guarded facts",
                        context.subject_name
                    ))]);
                }
                vec![PackageReviewCrashRouteGuard::Truth]
            } else {
                let mut guards = bucket
                    .facts()
                    .iter()
                    .map(|fact| {
                        let ProofFact::Expression(expression) = compilation.proof_facts.get(*fact)
                        else {
                            return Err(vec![Diagnostic::error(format!(
                                "public operator `{}` has a non-expression checked crash route",
                                context.subject_name
                            ))]);
                        };
                        project_contract_expression(
                            compilation,
                            context,
                            binders,
                            *expression,
                            Some(*fact),
                            0,
                        )
                        .map(PackageReviewCrashRouteGuard::Expression)
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                guards.sort();
                guards.dedup();
                if guards.is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "public operator `{}` has an empty guarded checked crash bucket",
                        context.subject_name
                    ))]);
                }
                guards
            };
            Ok(PackageReviewCrashRoute {
                cause: bucket.cause(),
                alternative_guards,
            })
        })
        .collect()
}

pub(crate) fn project_public_domains(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDomainShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation
        .domain_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.domain_type_parameters(definition);
        let (binders, type_parameters) =
            project_type_parameters(compilation, parameters, "domain", &identity.path, &[])?;
        let predicate_facts =
            project_domain_predicate_facts(compilation, definition, &identity, &binders)?;
        let alias_expansion = definition
            .alias
            .as_ref()
            .map(|_| project_domain_alias_expansion(compilation, definition.symbol))
            .transpose()?;
        let classification = definition
            .classification
            .map(|classification| match classification {
                psi_language_semantics::DomainClassification::ProgressProfile => {
                    PackageReviewDomainClassification::ProgressProfile
                }
            });
        let mut establishment_routes = definition
            .establishment_routes
            .iter()
            .map(|route| project_domain_establishment_route(compilation, *route))
            .collect::<Result<Vec<_>, _>>()?;
        establishment_routes.sort();
        establishment_routes.dedup();
        let semantic_roles = project_domain_semantic_roles(definition, &identity)?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewDomainShape {
                identity,
                type_parameters,
                target_type: review_type_identity_with_binders(
                    compilation,
                    definition.target_type,
                    &binders,
                )?,
                index_arguments: definition
                    .index_arguments
                    .iter()
                    .map(|argument| {
                        review_type_identity_with_binders(compilation, *argument, &binders)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                predicate_body: definition.predicate_body,
                predicate_facts,
                alias_expansion,
                classification,
                semantic_roles,
                establishment_routes,
            },
            declaration: definition.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                locations.extend(project_required_proof_fact_source_locations(
                    compilation,
                    definition.facts,
                    "public domain predicate",
                )?);
                locations
            },
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

pub(crate) fn project_domain_semantic_roles(
    definition: &psi_typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
) -> Result<Vec<PackageReviewDomainSemanticRole>, Vec<Diagnostic>> {
    let mut roles = Vec::new();
    for (role, semantic_identity) in [
        (
            PackageReviewDomainSemanticRole::DenotationDimension,
            definition.semantic_roles.denotation_dimension,
        ),
        (
            PackageReviewDomainSemanticRole::ArithmeticPolicy,
            definition.semantic_roles.arithmetic_policy,
        ),
    ] {
        let Some(semantic_identity) = semantic_identity else {
            continue;
        };
        if semantic_identity != definition.semantic_id {
            return Err(vec![Diagnostic::error(format!(
                "public domain `{}` semantic role does not name its exact typed semantic identity",
                identity.path
            ))]);
        }
        roles.push(role);
    }
    Ok(roles)
}

pub(crate) fn project_domain_predicate_facts(
    compilation: &CheckedCompilation,
    definition: &psi_typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public domain",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::Unknown,
        point: psi_facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: Some(definition.symbol),
        data_symbol: None,
        lifetime_binders: &[],
    };
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "domain predicate review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for offset in 0..definition.facts.count() {
        let fact_handle = psi_arena::Handle::from_parts(
            definition
                .facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("domain predicate fact handle index overflow"),
            definition.facts.start().generation(),
        );
        require_exact_checked_domain_fact(compilation, definition.symbol, fact_handle, identity)?;
        projected.push(project_definition_contract_fact(
            compilation,
            &context,
            binders,
            fact_handle,
            reviewed_package,
        )?);
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

pub(crate) fn project_definition_contract_fact(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    reviewed_package: PackageKeyIdentity,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use psi_typed_trees::domain::ProofFact;

    match compilation.proof_facts.get(fact_handle) {
        ProofFact::Expression(expression) => Ok(PackageReviewContractFact::Expression(
            project_contract_expression(
                compilation,
                context,
                binders,
                *expression,
                Some(fact_handle),
                0,
            )?,
        )),
        ProofFact::Membership(membership) => {
            let domain = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "{} `{}` predicate refers to an unresolved domain",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            let domain_identity = nominal_identity(compilation, domain.symbol)?;
            if reviewed_package_owns(&domain_identity, reviewed_package)? && !domain.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "{} `{}` predicate exposes non-public domain `{}`",
                    context.subject_kind, context.subject_name, domain.name
                ))]);
            }
            Ok(PackageReviewContractFact::Membership {
                value: project_contract_expression(
                    compilation,
                    context,
                    binders,
                    membership.value,
                    Some(fact_handle),
                    0,
                )?,
                domain: domain_identity,
            })
        }
        ProofFact::Proposition(application) => project_contract_proposition(
            compilation,
            context,
            binders,
            application,
            Some(fact_handle),
            &[],
            &[],
            &mut Vec::new(),
            0,
        ),
    }
}

pub(crate) fn require_exact_checked_domain_fact(
    compilation: &CheckedCompilation,
    domain_symbol: SymbolHandle,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let point = psi_facts::ProgramPoint::Definition {
        symbol: domain_symbol,
    };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == psi_facts::FactOrigin::DomainDefinition { domain_symbol }
                && fact.evidence == psi_facts::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = compilation
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .filter(|(_, record)| record.domain_symbol == domain_symbol && record.fact == fact_handle)
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record.semantic_fact == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

pub(crate) fn semantic_fact_matches_definition_fact(
    compilation: &CheckedCompilation,
    semantic_fact: &psi_facts::Fact,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
) -> bool {
    use psi_facts::FactPayload;
    use psi_typed_trees::domain::ProofFact;

    match (
        compilation.proof_facts.get(fact_handle),
        semantic_fact.payload,
    ) {
        (ProofFact::Expression(expected), FactPayload::BooleanExpression(actual)) => {
            *expected == actual
        }
        (
            ProofFact::Membership(expected),
            FactPayload::DomainMembership {
                value,
                domain,
                domain_symbol,
            },
        ) => {
            expected.value == value
                && expected.domain == domain
                && expected.domain_symbol == domain_symbol
        }
        (
            ProofFact::Proposition(expected),
            FactPayload::PropositionApplication { fact, proposition },
        ) => fact == fact_handle && proposition == expected.proposition,
        _ => false,
    }
}

pub(crate) fn project_domain_alias_expansion(
    compilation: &CheckedCompilation,
    domain_symbol: SymbolHandle,
) -> Result<Vec<PackageReviewDomainAliasAtom>, Vec<Diagnostic>> {
    fn expand(
        compilation: &CheckedCompilation,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        atoms: &mut Vec<PackageReviewDomainAliasAtom>,
    ) -> Result<(), Vec<Diagnostic>> {
        if stack.contains(&domain_symbol) {
            return Err(vec![Diagnostic::error(
                "package review encountered a cycle in checked domain alias expansion",
            )]);
        }
        let definitions = compilation
            .domain_definitions()
            .iter()
            .filter(|candidate| candidate.symbol == domain_symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "package review domain alias resolves to {} declarations; expected exactly one",
                definitions.len()
            ))]);
        };
        let Some(alias) = definition.alias.as_ref() else {
            atoms.push(PackageReviewDomainAliasAtom::Declared(nominal_identity(
                compilation,
                definition.symbol,
            )?));
            return Ok(());
        };
        stack.push(domain_symbol);
        for constituent in &alias.constituents {
            let label = compilation
                .domain_path_members(constituent.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if !constituent.domain_symbol.is_valid() && label == "Carry::Portable" {
                atoms.extend(
                    psi_language_semantics::CarryPermission::ALL
                        .map(PackageReviewDomainAliasAtom::Carry),
                );
            } else if !constituent.domain_symbol.is_valid()
                && let Some(permission) = psi_language_semantics::CarryPermission::from_name(&label)
            {
                atoms.push(PackageReviewDomainAliasAtom::Carry(permission));
            } else {
                if !constituent.domain_symbol.is_valid() {
                    return Err(vec![Diagnostic::error(format!(
                        "package review domain alias has unresolved constituent `{label}`"
                    ))]);
                }
                expand(compilation, constituent.domain_symbol, stack, atoms)?;
            }
        }
        stack.pop();
        Ok(())
    }

    let mut atoms = Vec::new();
    expand(compilation, domain_symbol, &mut Vec::new(), &mut atoms)?;
    atoms.sort();
    atoms.dedup();
    if atoms.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review domain alias has an empty canonical expansion",
        )]);
    }
    Ok(atoms)
}

pub(crate) fn project_domain_establishment_route(
    compilation: &CheckedCompilation,
    route: psi_language_semantics::DomainEstablishmentRoute,
) -> Result<PackageReviewDomainEstablishmentRoute, Vec<Diagnostic>> {
    let (kind, trait_symbol, requirement_symbol, expects_boundary) = match route {
        psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => (
            PackageReviewDomainEstablishmentKind::CheckedRequirement,
            trait_definition,
            requirement,
            false,
        ),
        psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => (
            PackageReviewDomainEstablishmentKind::BoundaryRequirement,
            boundary_trait,
            requirement,
            true,
        ),
    };
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review domain establishment route resolves to {} trait declarations; expected exactly one",
            owners.len()
        ))]);
    };
    if owner.is_boundary != expects_boundary {
        return Err(vec![Diagnostic::error(
            "package review domain establishment route kind disagrees with its exact trait declaration",
        )]);
    }
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review domain establishment route resolves to {} requirements under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    Ok(PackageReviewDomainEstablishmentRoute {
        kind,
        trait_identity: nominal_identity(compilation, owner.symbol)?,
        requirement_identity: trait_requirement_identity(compilation, owner, requirement)?,
    })
}

pub(crate) fn project_data_invariant_facts(
    compilation: &CheckedCompilation,
    definition: &psi_typed_trees::data::DataDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public data",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::Unknown,
        point: psi_facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: None,
        data_symbol: Some(definition.symbol),
        lifetime_binders: &definition.lifetime_parameters,
    };
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "data invariant review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for offset in 0..definition.where_facts.count() {
        let fact_handle = psi_arena::Handle::from_parts(
            definition
                .where_facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("data invariant fact handle index overflow"),
            definition.where_facts.start().generation(),
        );
        require_exact_checked_data_fact(compilation, definition.symbol, fact_handle, identity)?;
        projected.push(project_definition_contract_fact(
            compilation,
            &context,
            binders,
            fact_handle,
            reviewed_package,
        )?);
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionFact {
    pub(crate) data_symbol: SymbolHandle,
    pub(crate) fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    pub(crate) semantic_fact: RecheckedSemanticFact,
    pub(crate) dependencies: Vec<RecheckedDataDefinitionFactDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionFactDependency {
    pub(crate) expression: psi_typed_trees::expression::ExpressionHandle,
    pub(crate) place: RecheckedFactPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedSemanticFact {
    pub(crate) place: RecheckedSemanticFactPlace,
    pub(crate) point: psi_facts::ProgramPoint,
    pub(crate) origin: psi_facts::FactOrigin,
    pub(crate) evidence: psi_facts::QualificationEvidence,
    pub(crate) payload: psi_facts::FactPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecheckedSemanticFactPlace {
    Unknown,
    Place(RecheckedFactPlace),
    Symbol(SymbolHandle),
    Expression(psi_typed_trees::expression::ExpressionHandle),
    TypeReference(psi_typed_trees::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedFactPlace {
    pub(crate) root: psi_facts::PlaceRoot,
    pub(crate) segments: Vec<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionEvidence {
    pub(crate) definitions: Vec<RecheckedDataDefinitionFact>,
    pub(crate) semantic_facts: Vec<RecheckedSemanticFact>,
    pub(crate) refs: Vec<RecheckedSemanticFact>,
    pub(crate) contexts: Vec<RecheckedDataFactContext>,
    pub(crate) symbol_sets: Vec<RecheckedDataSymbolFactSet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataFactContext {
    pub(crate) point: psi_facts::ProgramPoint,
    pub(crate) facts: Vec<RecheckedSemanticFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataSymbolFactSet {
    pub(crate) symbol: SymbolHandle,
    pub(crate) facts: Vec<RecheckedSemanticFact>,
}

pub(crate) fn require_rederived_data_definition_facts(
    compilation: &CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let rederived = psi_facts::build_definition_fact_plan(&compilation.typed);
    let data_symbols = compilation
        .data_definitions()
        .iter()
        .map(|definition| definition.symbol)
        .collect::<Vec<_>>();
    let Some(expected) = rechecked_data_definition_evidence(&rederived, &data_symbols) else {
        return Err(vec![Diagnostic::error(
            "compiler-rederived data invariant evidence is internally malformed",
        )]);
    };
    let Some(retained) =
        rechecked_data_definition_evidence(&compilation.facts.semantic, &data_symbols)
    else {
        return Err(vec![Diagnostic::error(
            "retained checked data invariant evidence is internally malformed",
        )]);
    };
    if retained != expected {
        return Err(vec![Diagnostic::error(
            "retained checked data invariant evidence disagrees with the compiler-rederived typed program",
        )]);
    }
    Ok(())
}

pub(crate) fn rechecked_data_definition_evidence(
    facts: &psi_facts::FactPlan,
    data_symbols: &[SymbolHandle],
) -> Option<RecheckedDataDefinitionEvidence> {
    fact_plan_arena_links_are_well_formed(facts).then_some(())?;
    let definitions = facts
        .data_definition_facts
        .iter()
        .map(|(_, record)| {
            let semantic_fact = rechecked_semantic_fact(facts, record.semantic_fact)?;
            let dependencies = record
                .dependencies
                .iter()
                .map(|dependency| {
                    Some(RecheckedDataDefinitionFactDependency {
                        expression: dependency.expression,
                        place: rechecked_fact_place(facts, dependency.place)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(RecheckedDataDefinitionFact {
                data_symbol: record.data_symbol,
                fact: record.fact,
                semantic_fact,
                dependencies,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let semantic_facts = facts
        .facts
        .iter()
        .filter_map(|(_, fact)| {
            matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let refs = facts
        .refs
        .iter()
        .filter_map(|(_, fact_ref)| {
            let fact = facts
                .facts
                .iter()
                .find_map(|(handle, fact)| (handle == fact_ref.fact).then_some(fact))?;
            matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let contexts = facts
        .contexts
        .iter()
        .filter_map(|(_, context)| {
            let at_data_definition = matches!(
                context.point,
                psi_facts::ProgramPoint::Definition { symbol }
                    if data_symbols.contains(&symbol)
            );
            let references = match facts.refs.span(context.facts) {
                Some(references) => references,
                None if at_data_definition => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.iter().any(|(handle, fact)| {
                    handle == fact_ref.fact
                        && matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                })
            });
            (at_data_definition || contains_data_fact).then(|| {
                Some(RecheckedDataFactContext {
                    point: context.point,
                    facts: references
                        .iter()
                        .map(|fact_ref| rechecked_semantic_fact(facts, fact_ref.fact))
                        .collect::<Option<Vec<_>>>()?,
                })
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let symbol_sets = facts
        .symbol_sets
        .iter()
        .filter_map(|(_, set)| {
            let references = match facts.refs.span(set.facts) {
                Some(references) => references,
                None if data_symbols.contains(&set.symbol) => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.iter().any(|(handle, fact)| {
                    handle == fact_ref.fact
                        && matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                })
            });
            (data_symbols.contains(&set.symbol) || contains_data_fact).then(|| {
                Some(RecheckedDataSymbolFactSet {
                    symbol: set.symbol,
                    facts: references
                        .iter()
                        .map(|fact_ref| rechecked_semantic_fact(facts, fact_ref.fact))
                        .collect::<Option<Vec<_>>>()?,
                })
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(RecheckedDataDefinitionEvidence {
        definitions,
        semantic_facts,
        refs,
        contexts,
        symbol_sets,
    })
}

pub(crate) fn fact_plan_arena_links_are_well_formed(facts: &psi_facts::FactPlan) -> bool {
    facts
        .places
        .iter()
        .all(|(_, place)| facts.place_segments.span(place.segments).is_some())
        && facts.facts.iter().all(|(_, fact)| match fact.place {
            psi_facts::FactPlace::Place(place) => facts.places.is_valid(place),
            psi_facts::FactPlace::Unknown
            | psi_facts::FactPlace::Symbol(_)
            | psi_facts::FactPlace::Expression(_)
            | psi_facts::FactPlace::TypeReference(_) => true,
        })
        && facts
            .refs
            .iter()
            .all(|(_, fact_ref)| facts.facts.is_valid(fact_ref.fact))
        && facts
            .contexts
            .iter()
            .all(|(_, context)| facts.refs.span(context.facts).is_some())
        && facts
            .symbol_sets
            .iter()
            .all(|(_, set)| facts.refs.span(set.facts).is_some())
}

pub(crate) fn rechecked_semantic_fact(
    facts: &psi_facts::FactPlan,
    fact_handle: psi_facts::FactHandle,
) -> Option<RecheckedSemanticFact> {
    let fact = facts
        .facts
        .iter()
        .find_map(|(handle, fact)| (handle == fact_handle).then_some(fact))?;
    rechecked_semantic_fact_value(facts, fact)
}

pub(crate) fn rechecked_semantic_fact_value(
    facts: &psi_facts::FactPlan,
    fact: &psi_facts::Fact,
) -> Option<RecheckedSemanticFact> {
    Some(RecheckedSemanticFact {
        place: rechecked_semantic_fact_place(facts, fact.place)?,
        point: fact.point,
        origin: fact.origin,
        evidence: fact.evidence,
        payload: fact.payload,
    })
}

pub(crate) fn rechecked_semantic_fact_place(
    facts: &psi_facts::FactPlan,
    place: psi_facts::FactPlace,
) -> Option<RecheckedSemanticFactPlace> {
    Some(match place {
        psi_facts::FactPlace::Unknown => RecheckedSemanticFactPlace::Unknown,
        psi_facts::FactPlace::Place(place) => {
            RecheckedSemanticFactPlace::Place(rechecked_fact_place(facts, place)?)
        }
        psi_facts::FactPlace::Symbol(symbol) => RecheckedSemanticFactPlace::Symbol(symbol),
        psi_facts::FactPlace::Expression(expression) => {
            RecheckedSemanticFactPlace::Expression(expression)
        }
        psi_facts::FactPlace::TypeReference(type_reference) => {
            RecheckedSemanticFactPlace::TypeReference(type_reference)
        }
    })
}

pub(crate) fn rechecked_fact_place(
    facts: &psi_facts::FactPlan,
    place_handle: psi_facts::PlaceHandle,
) -> Option<RecheckedFactPlace> {
    let place = facts
        .places
        .iter()
        .find_map(|(handle, place)| (handle == place_handle).then_some(place))?;
    Some(RecheckedFactPlace {
        root: place.root,
        segments: facts.place_segments.span(place.segments)?.to_vec(),
    })
}

pub(crate) fn require_exact_checked_data_fact(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let point = psi_facts::ProgramPoint::Definition {
        symbol: data_symbol,
    };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == psi_facts::FactOrigin::DataDefinition { data_symbol }
                && fact.evidence == psi_facts::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public data `{}` invariant fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = compilation
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .filter(|(_, record)| record.data_symbol == data_symbol && record.fact == fact_handle)
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record.semantic_fact == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public data `{}` invariant fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

pub(crate) fn project_public_data(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDataShape>>, Vec<Diagnostic>> {
    require_rederived_data_definition_facts(compilation)?;
    let quotient_formations = psi_validation::validate_quotient_formations(compilation)?;
    let mut rows = Vec::new();
    for definition in compilation
        .data_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.data_type_parameters(definition);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "data",
            &identity.path,
            &definition.lifetime_parameters,
        )?;
        let kind = if definition.quotient.is_some() {
            let matching_formations = quotient_formations
                .iter()
                .filter(|formation| formation.data_symbol == definition.symbol)
                .collect::<Vec<_>>();
            let [formation] = matching_formations.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` has {} independently rederived formation rows; expected one",
                    identity.path,
                    matching_formations.len()
                ))]);
            };
            let matching_relations = compilation
                .propositions()
                .iter()
                .filter(|relation| relation.symbol == formation.relation_symbol)
                .collect::<Vec<_>>();
            let [relation] = matching_relations.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` has {} exact relation declarations; expected one",
                    identity.path,
                    matching_relations.len()
                ))]);
            };
            if !relation.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` exposes non-public relation `{}`",
                    identity.path, relation.name
                ))]);
            }
            PackageReviewDataKind::Quotient {
                carrier: review_signature_type_identity_with_binders(
                    compilation,
                    formation.carrier,
                    &binders,
                    &definition.lifetime_parameters,
                )?,
                relation: nominal_identity(compilation, formation.relation_symbol)?,
            }
        } else {
            PackageReviewDataKind::Ordinary
        };
        let invariants =
            project_data_invariant_facts(compilation, definition, &identity, &binders)?;

        let members = compilation
            .data_members(definition)
            .iter()
            .map(
                |member| -> Result<PackageReviewDataMember, Vec<Diagnostic>> {
                    Ok(match member {
                        psi_typed_trees::data::DataMember::Field(field) => {
                            PackageReviewDataMember::Field(project_data_field(
                                compilation,
                                field,
                                &binders,
                                &definition.lifetime_parameters,
                            )?)
                        }
                        psi_typed_trees::data::DataMember::Variant(variant) => {
                            let mut retired_payload_identities =
                                variant.retired_payload_identities.clone();
                            retired_payload_identities.sort_unstable();
                            retired_payload_identities.dedup();
                            PackageReviewDataMember::Variant {
                                identity: variant.identity,
                                name: variant.name.as_str().to_owned(),
                                payload: compilation
                                    .data_payload_fields(variant)
                                    .iter()
                                    .map(|field| {
                                        project_data_field(
                                            compilation,
                                            field,
                                            &binders,
                                            &definition.lifetime_parameters,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                retired_payload_identities,
                            }
                        }
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        let mut retired_identities = definition.retired_identities.clone();
        retired_identities.sort_unstable();
        retired_identities.dedup();
        rows.push(ProjectedReviewRow {
            row: PackageReviewDataShape {
                identity,
                kind,
                supply: definition.supply_mode,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                properties: definition.properties,
                zero_gated: definition.zero_gated,
                invariants,
                retired_identities,
                members,
            },
            declaration: definition.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                for member in compilation.data_members(definition) {
                    match member {
                        psi_typed_trees::data::DataMember::Field(field) => {
                            locations.push(project_nested_declaration_source_location(
                                compilation,
                                field.symbol,
                                PackageReviewSourceLocationRole::DataMember,
                                "public data field",
                            )?);
                        }
                        psi_typed_trees::data::DataMember::Variant(variant) => {
                            locations.push(project_nested_declaration_source_location(
                                compilation,
                                variant.symbol,
                                PackageReviewSourceLocationRole::DataMember,
                                "public data case",
                            )?);
                            for field in compilation.data_payload_fields(variant) {
                                locations.push(project_nested_declaration_source_location(
                                    compilation,
                                    field.symbol,
                                    PackageReviewSourceLocationRole::DataMember,
                                    "public data case payload field",
                                )?);
                            }
                        }
                    }
                }
                locations.extend(project_required_proof_fact_source_locations(
                    compilation,
                    definition.where_facts,
                    "public data invariant",
                )?);
                locations
            },
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
