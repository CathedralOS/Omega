/*
Authored contract invocations, `requires` proof facts, and parameter domain
membership (`contracts`) alongside shared parameter, type-parameter, evidence,
and callable-signature lowering (`signatures`).
*/

pub(crate) mod contracts {
    pub(crate) mod invocations {
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use symbols::SymbolKind;
        use typed_trees as typed;

        pub(crate) fn lower_authored_invocations(
            program: &resolved::SymbolResolvedTrees,
            declarations: &[resolved::name::DiagnosticName],
            parameters: &[resolved::signature::StateParameter],
            owner: &str,
        ) -> Result<Vec<typed::signature::AuthoredInvocation>, Diagnostic> {
            let mut lowered = Vec::with_capacity(declarations.len());
            for declaration in declarations {
                let target = if let Some((ordinal, parameter)) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.name.as_str() == declaration.as_str())
                {
                    typed::signature::AuthoredInvocationTarget::Parameter {
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(format!(
                        "callable `{owner}` has too many invocation parameters for portable identity"
                    ))
                })?,
                symbol: parameter.symbol,
            }
                } else {
                    invocation_target(program, declaration)
                };
                lowered.push(typed::signature::AuthoredInvocation {
                    name: crate::lowerer::name::lower_name(declaration),
                    source_span: declaration.source_span(),
                    target,
                });
            }
            Ok(lowered)
        }

        /// The requirement an `invokes` row that names no parameter selects,
        /// by the same scoped lookup that resolves every top-level name from
        /// the row's own source (modules.md, "Resolution"). When that lookup
        /// finds no requirement, it says why: nothing visible there has the
        /// spelling (`Undeclared`, wrong in every program), or something
        /// does (another kind, two requirements) and the row stays a request.
        /// A field path through a parameter is a valid row typing does not
        /// follow yet, so it is never classified as naming nothing.
        fn invocation_target(
            program: &resolved::SymbolResolvedTrees,
            declaration: &resolved::name::DiagnosticName,
        ) -> typed::signature::AuthoredInvocationTarget {
            use typed::signature::AuthoredInvocationTarget as Target;
            let name = declaration.as_str();
            let span = declaration.source_span();
            if let Some(service) = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(name, &[SymbolKind::Trait], span)
            {
                return Target::Service(service);
            }
            let plain_path = !name.is_empty()
                && name.split("::").all(|segment| {
                    !segment.is_empty()
                        && segment
                            .chars()
                            .all(|character| character == '_' || character.is_alphanumeric())
                });
            let visible = program
                .symbols
                .lookup_top_level_by_name_and_kinds_from_source_matching(
                    name,
                    &DECLARATION_KINDS,
                    span,
                    |_| true,
                );
            if declaration.is_source_backed()
                && plain_path
                && visible == symbols::SymbolLookup::NotFound
            {
                Target::Undeclared
            } else {
                Target::Unresolved
            }
        }

        /// Every kind a top-level name can select: a written row naming any
        /// of these names something, even when it is not a requirement.
        const DECLARATION_KINDS: [SymbolKind; 15] = [
            SymbolKind::Module,
            SymbolKind::BuiltinType,
            SymbolKind::BuiltinFunction,
            SymbolKind::Data,
            SymbolKind::Domain,
            SymbolKind::Machine,
            SymbolKind::Operator,
            SymbolKind::Measure,
            SymbolKind::Proposition,
            SymbolKind::MathematicalDefinition,
            SymbolKind::State,
            SymbolKind::Trait,
            SymbolKind::Conformance,
            SymbolKind::Const,
            SymbolKind::WireSchema,
        ];
    }
    pub(crate) mod parameter_domains {
        use crate::lowerer::Lowerer;
        use crate::signatures::parameters::lower_state_parameter;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        fn domain_constraints(
            program: &typed::TypedTrees,
            mut reference: typed::types::TypeReferenceHandle,
        ) -> Vec<(
            symbols::SymbolHandle,
            String,
            Vec<typed::types::TypeReferenceHandle>,
            language_semantics::SemanticDomainId,
        )> {
            use typed::types::{TypeConstraintNode, TypeReferenceNode};
            let mut domains = Vec::new();
            loop {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        for constraint in program.type_reference_table.constraints(*constraints) {
                            let TypeConstraintNode::Domain(domain) = constraint else {
                                continue;
                            };
                            if domain.symbol.is_valid() {
                                if let Some(definition) = program.domain_definition(domain.symbol) {
                                    domains.push((
                                        domain.symbol,
                                        definition.name.as_str().to_owned(),
                                        domain.arguments.clone(),
                                        domain.semantic_id,
                                    ));
                                }
                            } else if language_semantics::CarryPermission::from_name(
                                domain.name.as_str(),
                            )
                            .is_some()
                            {
                                domains.push((
                                    symbols::SymbolHandle::invalid(),
                                    domain.name.as_str().to_owned(),
                                    Vec::new(),
                                    language_semantics::SemanticDomainId::NULL,
                                ));
                            }
                        }
                        reference = *base_type;
                    }
                    _ => {
                        if let Some(storage) = program.layout_storage_arguments(reference) {
                            reference = storage.semantic_type;
                        } else {
                            break;
                        }
                    }
                }
            }
            domains
        }

        pub(crate) type DomainConstrainedParameter = (
            symbols::SymbolHandle,
            typed::name::Identifier,
            symbols::SymbolHandle,
            String,
            Vec<typed::types::TypeReferenceHandle>,
            language_semantics::SemanticDomainId,
        );

        pub(crate) fn lower_state_parameters_collecting_domain_constraints(
            lowerer: &mut Lowerer,
            parameters: arena::HandleSpan<resolved::signature::StateParameter>,
            mut push_parameter: impl FnMut(&mut typed::TypedTrees, typed::signature::StateParameter),
        ) -> Result<Vec<DomainConstrainedParameter>, diagnostics::Diagnostic> {
            let mut domain_constrained_parameters: Vec<DomainConstrainedParameter> = Vec::new();
            for parameter in lowerer.source_trees.state_parameters(parameters) {
                let parameter = lower_state_parameter(lowerer, parameter)?;
                for (domain_symbol, domain_full_name, domain_arguments, semantic_domain) in
                    domain_constraints(&lowerer.typed_trees, parameter.type_reference)
                {
                    domain_constrained_parameters.push((
                        parameter.symbol,
                        parameter.name.clone(),
                        domain_symbol,
                        domain_full_name,
                        domain_arguments,
                        semantic_domain,
                    ));
                }
                push_parameter(&mut lowerer.typed_trees, parameter);
            }
            Ok(domain_constrained_parameters)
        }

        pub(crate) fn push_domain_membership_contracts(
            lowerer: &mut Lowerer,
            domain_constrained_parameters: Vec<DomainConstrainedParameter>,
            mut push_contract: impl FnMut(&mut typed::TypedTrees, typed::signature::SignatureContract),
        ) {
            for (
                param_symbol,
                param_name,
                domain_symbol,
                domain_full_name,
                domain_arguments,
                semantic_domain,
            ) in domain_constrained_parameters
            {
                let contract = lowerer.typed_trees.parameter_domain_membership_contract(
                    param_symbol,
                    param_name,
                    domain_symbol,
                    &domain_full_name,
                    domain_arguments,
                    semantic_domain,
                );
                push_contract(&mut lowerer.typed_trees, contract);
            }
        }
    }
    pub(crate) mod proof_facts {
        pub(crate) mod domain_selections {
            use diagnostics::Diagnostic;
            use typed_trees::deferred_refusals::DeferredRequest;
            use typed_trees::{TypedTrees, domain::ProofFact};

            /// Select a pending membership's exact declared carrier family.
            /// Missing types and ambiguous families stay deferred on their
            /// real declaration owners; selecting a domain proves no membership.
            pub(crate) fn normalize(
                source: &symbol_resolved_trees::SymbolResolvedTrees,
                program: &mut TypedTrees,
                owners: &typed_trees::references::ProgramOperandOwners,
            ) -> Result<(), Diagnostic> {
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionKind as Kind,
                    AuthoredDeclarationSelectionLateBinding as Binding,
                    AuthoredDeclarationSelectionTarget as Target,
                };
                let unselected = program
                    .proof_facts
                    .iter()
                    .filter_map(|(handle, fact)| match fact {
                        ProofFact::Membership(membership)
                            if !membership.domain_symbol.is_valid()
                                && membership.authored_domain_selection.is_some() =>
                        {
                            Some((handle, membership.clone()))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if unselected.is_empty() {
                    return Ok(());
                }
                let mut rows = Vec::new();
                let mut updates = Vec::new();
                let mut types = crate::typing_queries::DeclaredExpressionTypes::default();
                for (handle, membership) in unselected {
                    let value = membership.value;
                    let source_span = program
                        .proof_fact_source_span(handle)
                        .or_else(|| program.expression_table.try_source_span(value))
                        .unwrap_or_default();
                    let holders = owners
                        .expression_owners(value)
                        .filter(|owner| owner.is_valid())
                        .collect::<Vec<_>>();
                    if holders.is_empty() {
                        return Err(crate::unavailable_judgment(
                            "contract membership carrier selection",
                        )
                        .with_source_span(source_span));
                    }
                    let mut carrier = None;
                    let mut missing = false;
                    for owner in &holders {
                        let found = types.get(program, *owner, value).and_then(|reference| {
                            super::membership_carrier_type(program, reference)
                        });
                        let Some(found) = found else {
                            missing = true;
                            continue;
                        };
                        if carrier.is_some_and(|earlier| {
                            program.package_qualified_type_identity(earlier)
                                != program.package_qualified_type_identity(found)
                        }) {
                            return Err(Diagnostic::error(
                                "domain membership has conflicting declared carrier contexts",
                            )
                            .with_source_span(source_span));
                        }
                        carrier = Some(found);
                    }
                    if !missing && let Some(carrier) = carrier {
                        let occurrence = membership.authored_domain_selection.ok_or_else(|| {
                            crate::unavailable_judgment("contract membership custody")
                        })?;
                        let selection = program
                            .authored_declaration_selections()
                            .get(occurrence)
                            .ok_or_else(|| {
                            crate::unavailable_judgment("contract membership custody")
                        })?;
                        let authored_span = selection.source_span();
                        if selection.kind() != Kind::DomainMembership
                            || authored_span.span.start >= authored_span.span.end
                            || !matches!(
                                selection.target(),
                                Target::LateBound(Binding::CheckedDomainMembership)
                                    | Target::Resolved(_)
                            )
                        {
                            return Err(crate::unavailable_judgment(
                                "contract membership domain custody",
                            )
                            .with_source_span(source_span));
                        }
                        let name = program
                            .domain_path_members(membership.domain)
                            .iter()
                            .map(|part| part.as_str())
                            .collect::<Vec<_>>()
                            .join("::");
                        let arguments = program
                            .type_reference_table
                            .type_reference_handles(membership.domain_arguments);
                        let matches =
                            crate::type_reference::domain_constraints::select_domain_candidates(
                                source,
                                program,
                                carrier,
                                &name,
                                Some(authored_span),
                                symbols::SymbolHandle::invalid(),
                                arguments,
                            );
                        if let [domain] = matches.as_slice() {
                            super::check_membership_argument_count(
                                program,
                                domain,
                                arguments.len(),
                            )
                            .map_err(|error| error.with_source_span(source_span))?;
                            updates.push((handle, occurrence, domain.symbol));
                            continue;
                        }
                    }
                    rows.extend(holders.into_iter().map(|owner| (owner, source_span)));
                }
                let mut selections = program.authored_declaration_selections().clone();
                for (_, occurrence, domain) in &updates {
                    if let Some(resolved) = selections
                        .get(*occurrence)
                        .and_then(|selection| selection.target().as_resolved().copied())
                    {
                        if resolved.selected_symbol() != *domain {
                            return Err(Diagnostic::error(
                                "copied contract membership changes its authored domain declaration",
                            ));
                        }
                    } else {
                        selections.finalize_late_bound(*occurrence, Binding::CheckedDomainMembership, *domain)
                            .map_err(|error| Diagnostic::error(format!(
                                "failed to finalize contract membership domain custody: {error:?}",
                            )))?;
                    }
                }
                program.retain_authored_declaration_selections(selections);
                for (handle, _, domain) in updates {
                    let ProofFact::Membership(membership) = program.proof_facts.get_mut(handle)
                    else {
                        return Err(Diagnostic::error("domain membership changed its fact kind"));
                    };
                    membership.domain_symbol = domain;
                }
                for (owner, source_span) in rows {
                    program.record_deferred_refusal(
                        owner,
                        DeferredRequest::ContractMembershipSelection,
                        source_span,
                    );
                }
                Ok(())
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionExposure as Exposure,
                    AuthoredDeclarationSelectionKind as Kind,
                    AuthoredDeclarationSelectionLateBinding as Binding,
                };
                use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
                use typed_trees::data::DataDefinition;
                use typed_trees::domain::ProofMembershipFact;
                use typed_trees::expression::ExpressionNode;
                use typed_trees::name::Identifier;

                /// A program with data `Held` and `Bystander`, and one
                /// membership fact whose domain selection stays late-bound.
                /// With `held`, the fact is `Held`'s `requires` clause;
                /// without it, no declaration holds the fact.
                fn unselected_membership(held: bool) -> (TypedTrees, [symbols::SymbolHandle; 2]) {
                    let mut builder = SymbolTableBuilder::new();
                    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                    let declarations = builder.insert_children(
                        root,
                        [
                            (SymbolKind::Data, SymbolNameRef::Static("Held")),
                            (SymbolKind::Data, SymbolNameRef::Static("Bystander")),
                        ],
                    );
                    let [holder, bystander] =
                        [declarations.handle_at(0), declarations.handle_at(1)];
                    let mut program = TypedTrees {
                        symbols: builder.finish(),
                        ..Default::default()
                    };
                    let span = source::SourceSpan::new(
                        source::SourceId::default(),
                        source::Span::new(4, 9),
                    );
                    let selection = program
                        .record_late_bound_authored_declaration_selection(
                            span,
                            Exposure::PrivateImplementation,
                            Kind::DomainMembership,
                            Binding::CheckedDomainMembership,
                        )
                        .unwrap();
                    let value = program.expression_table.insert(ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(1),
                    ));
                    let mut facts = arena::HandleSpan::empty();
                    let fact = program.proof_facts.append_to_span(
                        &mut facts,
                        ProofFact::Membership(ProofMembershipFact {
                            value,
                            authored_domain_selection: Some(selection),
                            ..Default::default()
                        }),
                    );
                    program.set_proof_fact_source_span(fact, span);
                    program.push_data_definition(DataDefinition {
                        symbol: holder,
                        name: Identifier::generated("Held"),
                        requires_facts: if held {
                            facts
                        } else {
                            arena::HandleSpan::empty()
                        },
                        ..Default::default()
                    });
                    program.push_data_definition(DataDefinition {
                        symbol: bystander,
                        name: Identifier::generated("Bystander"),
                        ..Default::default()
                    });
                    (program, [holder, bystander])
                }

                #[test]
                fn pending_reference_memberships_select_the_exact_referent_carrier() {
                    let program = crate::qualified_operator_home_tests::checked(&[(
                        "memberships.omg",
                        r#"
                            domain [u8; 4]::Ascii;
                            domain [u8; 16]::Ascii;
                            machine short(value: &[u8; 4]) ensures value in Ascii {}
                            machine long(value: &mut [u8; 16]) ensures value in Ascii {}
                        "#,
                    )])
                    .expect("reference membership retains its actual carrier family");
                    let mut seen = Vec::new();
                    for (_, fact) in program.proof_facts.iter() {
                        let ProofFact::Membership(membership) = fact else {
                            continue;
                        };
                        let occurrence = membership
                            .authored_domain_selection
                            .expect("written membership retains declaration custody");
                        let selection = program
                            .authored_declaration_selections()
                            .get(occurrence)
                            .expect("retained occurrence");
                        assert_eq!(
                            selection.target().as_resolved().unwrap().selected_symbol(),
                            membership.domain_symbol
                        );
                        assert!(membership.semantic_domain.is_valid());
                        let domain = program
                            .domain_definition_for_instance(
                                membership.domain_symbol,
                                membership.semantic_domain,
                            )
                            .expect("the complete selected domain application");
                        let typed_trees::types::TypeReferenceNode::FixedArray { length, .. } =
                            program
                                .type_reference_table
                                .type_reference(domain.target_type)
                        else {
                            panic!("the selected carrier is the actual fixed array")
                        };
                        let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                            panic!("the complete capacity is retained")
                        };
                        seen.push(*length);
                    }
                    seen.sort_unstable();
                    assert_eq!(seen, [4, 16]);
                }

                #[test]
                fn indexed_membership_selection_preserves_the_authored_arguments() {
                    let program = crate::qualified_operator_home_tests::checked(&[(
                        "indexed-membership.omg",
                        r#"
                            domain<Index> i64::Tagged<Index>;
                            machine retains(value: &i64) ensures value in Tagged<u32> {}
                        "#,
                    )])
                    .expect("the indexed membership retains its complete application");
                    let memberships = program
                        .proof_facts
                        .iter()
                        .filter_map(|(_, fact)| {
                            let ProofFact::Membership(membership) = fact else {
                                return None;
                            };
                            Some(membership)
                        })
                        .collect::<Vec<_>>();
                    let [membership] = memberships.as_slice() else {
                        panic!("one membership")
                    };
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments);
                    assert_eq!(arguments.len(), 1);
                    assert_eq!(
                        program.domain_arguments_for_instance(
                            membership.domain_symbol,
                            membership.semantic_domain
                        ),
                        Some(arguments)
                    );
                    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(arguments[0])
                    else {
                        panic!("the actual index type is retained")
                    };
                    assert_eq!(
                        program.symbols.builtin_type_atom(*symbol),
                        Some(symbols::BuiltinTypeAtom::U32)
                    );
                }

                #[test]
                fn a_membership_with_another_capacity_stays_unselected() {
                    let program = crate::qualified_operator_home_tests::checked(&[(
                        "mismatched-membership.omg",
                        r#"
                            domain [u8; 4]::Ascii;
                            domain [u8; 16]::Ascii;
                            machine wrong(value: &[u8; 32]) ensures value in Ascii {}
                        "#,
                    )])
                    .expect("unserved carrier selection stays on its actual declaration");
                    let wrong = program
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str() == "wrong")
                        .unwrap();
                    assert_eq!(
                        program
                            .deferred_refusal(wrong.symbol)
                            .map(|row| row.request),
                        Some(DeferredRequest::ContractMembershipSelection)
                    );
                    for (_, fact) in program.proof_facts.iter() {
                        let ProofFact::Membership(membership) = fact else {
                            continue;
                        };
                        assert!(!membership.domain_symbol.is_valid());
                        assert!(!membership.semantic_domain.is_valid());
                    }
                }

                #[test]
                fn copied_membership_cannot_change_its_authored_domain() {
                    let text = r#"
                        domain u32::First;
                        domain u32::Second;
                        machine keep(value: &u32) ensures value in First {}
                    "#;
                    let mut sources = source::SourceMap::default();
                    let id = sources
                        .add("membership-custody.omg".into(), text.to_owned())
                        .source_id;
                    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
                    let syntax =
                        tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
                    let mut request =
                        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
                    request.sources = Some(std::sync::Arc::new(sources));
                    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
                    let mut program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
                    let handle = program
                        .proof_facts
                        .iter()
                        .find_map(|(handle, fact)| {
                            matches!(fact, ProofFact::Membership(_)).then_some(handle)
                        })
                        .unwrap();
                    let second = program
                        .domain_path_members
                        .insert_many([Identifier::generated("Second")]);
                    let ProofFact::Membership(membership) = program.proof_facts.get_mut(handle)
                    else {
                        panic!("one authored membership")
                    };
                    membership.domain = second;
                    membership.domain_symbol = symbols::SymbolHandle::invalid();
                    membership.semantic_domain = language_semantics::SemanticDomainId::NULL;
                    let owners = typed_trees::references::collect_program_operand_owners(&program);
                    let error = normalize(&resolved, &mut program, &owners).unwrap_err();
                    assert!(
                        error
                            .message
                            .contains("changes its authored domain declaration"),
                        "{}",
                        error.message
                    );
                    let ProofFact::Membership(membership) = program.proof_facts.get(handle) else {
                        panic!("membership retained")
                    };
                    assert!(!membership.domain_symbol.is_valid());
                }

                #[test]
                fn an_unselected_membership_refuses_only_the_declaration_holding_it() {
                    let (mut program, [holder, bystander]) = unselected_membership(true);
                    let owners = typed_trees::references::collect_program_operand_owners(&program);
                    normalize(
                        &symbol_resolved_trees::SymbolResolvedTrees::default(),
                        &mut program,
                        &owners,
                    )
                    .unwrap();
                    assert_eq!(
                        program.deferred_refusal(holder).map(|row| row.request),
                        Some(DeferredRequest::ContractMembershipSelection)
                    );
                    assert!(program.deferred_refusal(bystander).is_none());
                }

                #[test]
                fn an_unselected_membership_no_declaration_holds_still_refuses() {
                    let (mut program, [holder, bystander]) = unselected_membership(false);
                    let owners = typed_trees::references::collect_program_operand_owners(&program);
                    let error = normalize(
                        &symbol_resolved_trees::SymbolResolvedTrees::default(),
                        &mut program,
                        &owners,
                    )
                    .unwrap_err();
                    assert!(
                        error
                            .message
                            .contains("contract membership carrier selection"),
                        "{}",
                        error.message
                    );
                    assert!(program.deferred_refusal(holder).is_none());
                    assert!(program.deferred_refusal(bystander).is_none());
                }
            }
        }
        use crate::type_reference::lower_type_reference_into_table;
        pub(crate) use domain_selections::normalize as normalize_membership_selections;

        use crate::expressions::expression::{
            lower_expression_handle_from_table_with_symbol_substitution, lower_fact_expression,
        };
        use crate::expressions::proposition::is_proposition_call_target;
        use crate::lowerer::Lowerer;
        use crate::lowerer::name::lower_name;
        use crate::tables::resolved_expressions;
        use crate::type_reference::domain_aliases::expand_domain_reference;
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use symbols::JoinedPath;
        use typed_trees as typed;

        pub(crate) fn lower_proof_facts(
            lowerer: &mut Lowerer,
            facts: HandleSpan<resolved::domain::ProofFact>,
        ) -> Result<HandleSpan<typed::domain::ProofFact>, Diagnostic> {
            let mut lowered = HandleSpan::empty();

            for (offset, fact) in lowerer.source_trees.proof_facts(facts).iter().enumerate() {
                let source_fact =
                    facts.handle_at(u32::try_from(offset).expect("proof fact offset overflow"));
                let source_span = lowerer.source_trees.proof_fact_source_span(source_fact);
                match fact {
                    resolved::domain::ProofFact::Expression(expression) => {
                        let fact = lower_expression_fact(lowerer, *expression)?;
                        let handle = lowerer
                            .typed_trees
                            .proof_facts
                            .append_to_span(&mut lowered, fact);
                        if let Some(source_span) = source_span {
                            lowerer
                                .typed_trees
                                .set_proof_fact_source_span(handle, source_span);
                        }
                    }
                    resolved::domain::ProofFact::Membership(membership) => {
                        let value = lower_expression_handle_from_table_with_symbol_substitution(
                            Some(lowerer.source_trees),
                            &lowerer.source_trees.tables.bodies.expressions,
                            &mut lowerer.typed_trees,
                            membership.value,
                            None,
                        )?;
                        let authored_path = lowerer
                            .source_trees
                            .domain_path_members(membership.domain)
                            .to_vec();
                        let domain_arguments =
                            lower_membership_domain_arguments(lowerer, membership, source_span)?;
                        let expanded = expand_domain_reference(
                            lowerer.source_trees,
                            membership.domain_symbol,
                            authored_path,
                        )?;
                        if !domain_arguments.is_empty() && expanded.len() != 1 {
                            let alias = lowerer
                                .source_trees
                                .domain_path_members(membership.domain)
                                .joined_path();
                            return Err(with_optional_source_span(
                                Diagnostic::error(format!(
                                    "domain alias `{alias}` does not take index arguments"
                                )),
                                source_span,
                            ));
                        }
                        for atom in expanded {
                            let mut domain = HandleSpan::empty();
                            for member in atom.path {
                                lowerer
                                    .typed_trees
                                    .domain_path_members
                                    .append_to_span(&mut domain, lower_name(&member));
                            }
                            let handle = lowerer.typed_trees.proof_facts.append_to_span(
                                &mut lowered,
                                typed::domain::ProofFact::Membership(
                                    typed::domain::ProofMembershipFact {
                                        value,
                                        domain,
                                        domain_symbol: atom.symbol,
                                        domain_arguments,
                                        semantic_domain: language_semantics::SemanticDomainId::NULL,
                                        authored_domain_selection: membership
                                            .authored_domain_selection,
                                    },
                                ),
                            );
                            if let Some(source_span) = source_span {
                                lowerer
                                    .typed_trees
                                    .set_proof_fact_source_span(handle, source_span);
                            }
                        }
                    }
                }
            }

            Ok(lowered)
        }

        fn with_optional_source_span(
            diagnostic: Diagnostic,
            source_span: Option<source::SourceSpan>,
        ) -> Diagnostic {
            match source_span {
                Some(source_span) => diagnostic.with_source_span(source_span),
                None => diagnostic,
            }
        }

        fn lower_membership_domain_arguments(
            lowerer: &mut Lowerer,
            membership: &resolved::domain::ProofMembershipFact,
            source_span: Option<source::SourceSpan>,
        ) -> Result<HandleSpan<typed::types::TypeReferenceHandle>, Diagnostic> {
            if membership.domain_arguments.is_empty() {
                return Ok(HandleSpan::empty());
            }
            let source_arguments = lowerer
                .source_trees
                .child_type_references(membership.domain_arguments)
                .to_vec();
            let mut arguments = Vec::with_capacity(source_arguments.len());
            for argument in &source_arguments {
                arguments.push(lower_type_reference_into_table(lowerer, argument)?);
            }
            if let Some(domain) = lowerer
                .typed_trees
                .domain_definition(membership.domain_symbol)
            {
                check_membership_argument_count(&lowerer.typed_trees, domain, arguments.len())
                    .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
            }
            Ok(lowerer
                .typed_trees
                .type_reference_table
                .insert_type_reference_handles(arguments))
        }

        fn check_membership_argument_count(
            program: &typed::TypedTrees,
            domain: &typed::domain::DomainDefinition,
            supplied: usize,
        ) -> Result<(), Diagnostic> {
            let index_parameters = typed::domain::index_parameters(program, domain);
            if supplied != index_parameters.len() {
                return Err(Diagnostic::error(format!(
                    "domain family `{}` requires {} closed index argument(s), but {} were supplied",
                    domain.name,
                    index_parameters.len(),
                    supplied
                )));
            }
            Ok(())
        }

        /// Membership on a reference names the domains of its referent.
        /// Keep the complete nominal carrier; neither field layout nor a
        /// receiving domain may replace the actual operand's declared type.
        fn membership_carrier_type(
            program: &typed::TypedTrees,
            reference: typed::types::TypeReferenceHandle,
        ) -> Option<typed::types::TypeReferenceHandle> {
            let types = &program.type_reference_table;
            if !types.contains_type_reference(reference) {
                return None;
            }
            let carrier = types.unconstrained_type_reference(reference);
            let carrier = match types.type_reference(carrier) {
                typed::types::TypeReferenceNode::Reference { referee, .. } => {
                    if !types.contains_type_reference(*referee) {
                        return None;
                    }
                    types.unconstrained_type_reference(*referee)
                }
                _ => carrier,
            };
            types.contains_type_reference(carrier).then_some(carrier)
        }

        pub(crate) fn has_pending_memberships(program: &typed::TypedTrees) -> bool {
            program.proof_facts.iter().any(|(_, fact)| {
                let Some(membership) = fact.as_membership() else {
                    return false;
                };
                if !membership.domain_symbol.is_valid() {
                    return membership.authored_domain_selection.is_some();
                }
                program
                    .domain_arguments_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )
                    .is_none()
            })
        }

        pub(crate) fn intern_proof_membership_instances(
            program: &mut typed::TypedTrees,
            owners: &typed::references::ProgramOperandOwners,
        ) -> Result<(), Diagnostic> {
            let mut types = crate::typing_queries::DeclaredExpressionTypes::default();
            let mut instances = Vec::new();
            for (handle, fact) in program.proof_facts.iter() {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if !membership.domain_symbol.is_valid() {
                    continue;
                }
                if program
                    .domain_arguments_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )
                    .is_some()
                {
                    continue;
                }
                let Some(domain) = program.domain_definition(membership.domain_symbol) else {
                    continue;
                };
                let source_span = program.proof_fact_source_span(handle);
                check_membership_argument_count(program, domain, membership.domain_arguments.len())
                    .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
                if program.domain_type_parameters(domain).is_empty() {
                    instances.push((
                        handle,
                        membership.domain_symbol,
                        None,
                        Vec::new(),
                        domain.semantic_id,
                    ));
                    continue;
                }
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .to_vec();
                let mut carrier = None;
                let mut missing = false;
                let holders = owners
                    .expression_owners(membership.value)
                    .collect::<Vec<_>>();
                for owner in &holders {
                    let Some(found) = types
                        .get(program, *owner, membership.value)
                        .and_then(|reference| membership_carrier_type(program, reference))
                    else {
                        missing = true;
                        continue;
                    };
                    if carrier.is_some_and(|earlier| {
                        program.package_qualified_type_identity(earlier)
                            != program.package_qualified_type_identity(found)
                    }) {
                        return Err(with_optional_source_span(
                            Diagnostic::error(
                                "domain membership has conflicting declared carrier contexts",
                            ),
                            source_span,
                        ));
                    }
                    carrier = Some(found);
                }
                if missing || carrier.is_none() {
                    // A missing receiving type is pending work on its actual
                    // declaration, not permission to guess the family's carrier.
                    instances.push((
                        handle,
                        membership.domain_symbol,
                        None,
                        arguments,
                        language_semantics::SemanticDomainId::NULL,
                    ));
                } else {
                    instances.push((
                        handle,
                        membership.domain_symbol,
                        carrier,
                        arguments,
                        language_semantics::SemanticDomainId::NULL,
                    ));
                }
            }
            for (handle, declaration, carrier, arguments, original) in instances {
                let semantic_domain = if let Some(carrier) = carrier {
                    program
                        .retain_domain_application(declaration, carrier, &arguments)
                        .map_err(|diagnostic| {
                            with_optional_source_span(
                                diagnostic,
                                program.proof_fact_source_span(handle),
                            )
                        })?
                } else if original.is_valid() {
                    original
                } else {
                    let typed::domain::ProofFact::Membership(membership) =
                        program.proof_facts.get(handle)
                    else {
                        return Err(Diagnostic::error("domain membership changed its fact kind"));
                    };
                    let holders = owners
                        .expression_owners(membership.value)
                        .collect::<Vec<_>>();
                    let source_span = program.proof_fact_source_span(handle).unwrap_or_default();
                    if holders.is_empty() {
                        return Err(with_optional_source_span(
                            Diagnostic::error(
                                "no product route yet: domain membership has no declared carrier owner",
                            ),
                            Some(source_span),
                        ));
                    }
                    for owner in holders {
                        program.record_deferred_refusal(
                            owner,
                            typed::deferred_refusals::DeferredRequest::ContractMembershipSelection,
                            source_span,
                        );
                    }
                    continue;
                };
                if let typed::domain::ProofFact::Membership(membership) =
                    program.proof_facts.get_mut(handle)
                {
                    membership.semantic_domain = semantic_domain;
                }
            }
            Ok(())
        }

        pub(crate) fn lower_contract_kind(
            kind: &resolved::signature::SignatureContractKind,
        ) -> typed::signature::SignatureContractKind {
            match kind {
                resolved::signature::SignatureContractKind::Requires => {
                    typed::signature::SignatureContractKind::Requires
                }
                resolved::signature::SignatureContractKind::Ensures => {
                    typed::signature::SignatureContractKind::Ensures
                }
                resolved::signature::SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } => typed::signature::SignatureContractKind::EnsuresForResultCase {
                    result_data: *result_data,
                    result_case: *result_case,
                },
                resolved::signature::SignatureContractKind::Crashes { cause } => {
                    typed::signature::SignatureContractKind::Crashes {
                        cause: match cause {
                            resolved::signature::CrashCause::Trap => {
                                typed::signature::CrashCause::Trap
                            }
                            resolved::signature::CrashCause::Abort => {
                                typed::signature::CrashCause::Abort
                            }
                        },
                    }
                }
            }
        }

        pub(crate) fn lower_signature_contract(
            lowerer: &mut Lowerer,
            contract: &resolved::signature::SignatureContract,
        ) -> Result<typed::signature::SignatureContract, Diagnostic> {
            // Every callable contract admits total policy arithmetic, including
            // requirements without a body. The declaration kind is irrelevant.
            let facts = lowerer.admitting_contract_policy_operators(|lowerer| {
                lower_proof_facts(lowerer, contract.facts)
            })?;
            Ok(typed::signature::SignatureContract {
                kind: lower_contract_kind(&contract.kind),
                keyword_source_span: contract.keyword_source_span,
                binding: contract
                    .binding
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                facts,
                token_count: contract.token_count,
            })
        }

        pub(crate) fn lower_expression_fact(
            lowerer: &mut Lowerer,
            expression: resolved::expression::ExpressionHandle,
        ) -> Result<typed::domain::ProofFact, Diagnostic> {
            if let resolved::expression::ExpressionNode::Call(call) =
                resolved_expressions(lowerer.source_trees).expression(expression)
                && is_proposition_call_target(&lowerer.source_trees.symbols, call.target_symbol)
            {
                return crate::expressions::proposition::lower_proposition_application(
                    lowerer, call,
                )
                .map(typed::domain::ProofFact::Proposition);
            }
            lower_fact_expression(lowerer, expression).map(typed::domain::ProofFact::Expression)
        }
        pub(crate) fn lower_statement_fact(
            lowerer: &mut Lowerer,
            expression: resolved::expression::ExpressionHandle,
        ) -> Result<arena::Handle<typed::domain::ProofFact>, Diagnostic> {
            let span = resolved_expressions(lowerer.source_trees).source_span(expression);
            let fact = lower_expression_fact(lowerer, expression)?;
            let handle = lowerer.typed_trees.proof_facts.insert(fact);
            lowerer.typed_trees.set_proof_fact_source_span(handle, span);
            Ok(handle)
        }
    }
}

pub(crate) mod signatures {
    pub(crate) mod callable_signature {
        use crate::lowerer::Lowerer;
        use crate::lowerer::name::lower_names;
        use crate::signatures::type_parameters::lower_type_parameters;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        use crate::contracts::invocations::lower_authored_invocations;

        pub(crate) fn lower_state_signature(
            lowerer: &mut Lowerer,
            signature: &resolved::signature::StateSignature,
        ) -> Result<typed::signature::StateSignature, Diagnostic> {
            let mut typed_signature = typed::signature::StateSignature {
                symbol: signature.symbol,
                name: crate::lowerer::name::lower_name(&signature.name),
                spelling: signature.spelling,
                lifetime_parameters: lower_names(&signature.lifetime_parameters),
                type_parameters: Default::default(),
                conformance_bounds: crate::signatures::evidence::lower_conformance_bounds(
                    lowerer,
                    &signature.conformance_bounds,
                )?,
                is_default: signature.is_default,
                parameters: Default::default(),
                native_callback_parameters: signature
                    .native_callback_parameters
                    .iter()
                    .map(|parameter| typed::signature::NativeCallbackParameter {
                        name: crate::lowerer::name::lower_name(&parameter.name),
                        binder: crate::lowerer::name::lower_name(&parameter.binder),
                        native_ordinal: parameter.native_ordinal,
                    })
                    .collect(),
                return_type: crate::type_reference::lower_return_type(
                    lowerer,
                    &signature.return_type,
                )?,
                invokes: Default::default(),
                service_reach_row: signature.service_reach_row,
                service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
                suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
                blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
                suspends: signature.suspends,
                blocks: signature.blocks,
                contracts: Default::default(),
                termination_guarantee: if signature.terminates_guarantee {
                    language_semantics::TerminationGuarantee::Terminates {
                        premises: Vec::new(),
                    }
                } else {
                    language_semantics::TerminationGuarantee::NoGuarantee
                },
                declares_no_crash_route: signature.declares_no_crash_route,
            };

            typed_signature.type_parameters =
                lower_type_parameters(lowerer, signature.type_parameters)?;

            let domain_constrained_parameters =
        crate::contracts::parameter_domains::lower_state_parameters_collecting_domain_constraints(
            lowerer,
            signature.parameters,
            |typed_trees, parameter| {
                typed_trees.push_state_signature_parameter(&mut typed_signature, parameter)
            },
        )?;

            let invocations = lower_authored_invocations(
                lowerer.source_trees,
                lowerer.source_trees.signature_invokes(signature.invokes),
                lowerer.source_trees.state_parameters(signature.parameters),
                signature.name.as_str(),
            )?;
            for invocation in invocations {
                lowerer
                    .typed_trees
                    .push_state_signature_invoke(&mut typed_signature, invocation);
            }

            for contract in lowerer
                .source_trees
                .signature_contracts(signature.contracts)
            {
                let contract =
                    crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
                lowerer
                    .typed_trees
                    .push_state_signature_contract(&mut typed_signature, contract);
            }

            crate::contracts::parameter_domains::push_domain_membership_contracts(
                lowerer,
                domain_constrained_parameters,
                |typed_trees, contract| {
                    typed_trees.push_state_signature_contract(&mut typed_signature, contract)
                },
            );

            Ok(typed_signature)
        }
    }
    pub(crate) mod evidence {
        use crate::lowerer::Lowerer;
        use crate::lowerer::name::lower_name;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;

        pub(crate) fn lower_conformance_bounds<'source>(
            lowerer: &mut Lowerer,
            bounds: impl IntoIterator<
                Item = &'source symbol_resolved_trees::machine::GenericConformanceBound,
            >,
        ) -> Result<Vec<typed_trees::machine::GenericConformanceBound>, Diagnostic> {
            bounds
                .into_iter()
                .map(|bound| {
                    let mut arguments = Vec::new();
                    for argument in lowerer.source_trees.child_type_references(bound.arguments) {
                        arguments.push(lower_type_reference_into_table(lowerer, argument)?);
                    }
                    Ok(typed_trees::machine::GenericConformanceBound {
                        binder: bound.binder,
                        binder_name: lower_name(&bound.binder_name),
                        trait_symbol: bound.trait_symbol,
                        trait_name: lower_name(&bound.trait_name),
                        lifetime_arguments: bound
                            .lifetime_arguments
                            .iter()
                            .map(lower_name)
                            .collect(),
                        arguments,
                    })
                })
                .collect()
        }
    }
    pub(crate) mod lexical_context {
        use crate::expressions::expression::lower_static_machine_argument;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
        use symbol_resolved_trees as resolved;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees as typed;

        fn enclosing_declarations<'program>(
            program: &'program resolved::SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
        ) -> impl Iterator<Item = &'program resolved::machine::Machine> {
            let mut owners = Vec::new();
            let mut parent = machine.lexical_parent;
            while parent.is_valid() {
                let owner = program.machines.get(parent);
                owners.push(owner);
                parent = owner.lexical_parent;
            }
            owners.into_iter().rev()
        }

        pub(crate) fn enclosing_parameters<'program>(
            program: &'program resolved::SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
        ) -> Vec<&'program resolved::data::TypeParameter> {
            enclosing_declarations(program, machine)
                .flat_map(|owner| program.machine_type_parameters(owner))
                .collect()
        }

        pub(crate) fn enclosing_bounds<'program>(
            program: &'program resolved::SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
        ) -> impl Iterator<Item = &'program resolved::machine::GenericConformanceBound> {
            enclosing_declarations(program, machine).flat_map(|owner| &owner.conformance_bounds)
        }

        pub(crate) fn call_arguments(
            program: Option<&resolved::SymbolResolvedTrees>,
            target: &mut typed::TypedTrees,
            exposure: Option<SelectionExposure>,
            callee: SymbolHandle,
            authored: &[resolved::expression::StaticMachineArgument],
        ) -> Result<Box<[typed::expression::StaticMachineArgument]>, Diagnostic> {
            let mut arguments = captured_arguments(program, callee);
            for argument in authored {
                arguments.push(lower_static_machine_argument(
                    program, target, exposure, argument,
                )?);
            }
            Ok(arguments.into_boxed_slice())
        }

        pub(crate) fn captured_arguments(
            program: Option<&resolved::SymbolResolvedTrees>,
            callee: SymbolHandle,
        ) -> Vec<typed::expression::StaticMachineArgument> {
            let Some(program) = program.filter(|_| callee.is_valid()) else {
                return Vec::new();
            };
            let owner = match program.symbols.get(callee).kind {
                SymbolKind::Machine => callee,
                SymbolKind::State => program.symbols.get(callee).parent,
                _ => return Vec::new(),
            };
            if !owner.is_valid() {
                return Vec::new();
            }
            let enclosing = program.symbols.get(owner).parent;
            if !enclosing.is_valid() || program.symbols.get(enclosing).kind != SymbolKind::Machine {
                return Vec::new();
            }
            let machine = program.machine(owner);
            machine.map_or_else(Vec::new, |machine| {
                enclosing_parameters(program, machine)
                    .into_iter()
                    .map(|parameter| typed::expression::StaticMachineArgument {
                        symbol: parameter.symbol,
                        path: vec![crate::lowerer::name::lower_name(&parameter.name)]
                            .into_boxed_slice(),
                        ..Default::default()
                    })
                    .chain(enclosing_bounds(program, machine).map(|bound| {
                        typed::expression::StaticMachineArgument {
                            symbol: bound.binder,
                            path: vec![crate::lowerer::name::lower_name(&bound.binder_name)]
                                .into_boxed_slice(),
                            ..Default::default()
                        }
                    }))
                    .collect()
            })
        }
    }
    pub(crate) mod parameters {
        use crate::lowerer::Lowerer;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_state_parameter(
            lowerer: &mut Lowerer,
            parameter: &resolved::signature::StateParameter,
        ) -> Result<typed::signature::StateParameter, Diagnostic> {
            let type_reference =
                lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
            crate::type_reference::domain_constraints::normalize_domain_constraints_for_type(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                type_reference,
            )?;
            Ok(state_parameter(parameter, type_reference))
        }

        pub(crate) fn state_parameter(
            parameter: &resolved::signature::StateParameter,
            type_reference: typed::types::TypeReferenceHandle,
        ) -> typed::signature::StateParameter {
            typed::signature::StateParameter {
                symbol: parameter.symbol,
                name: crate::lowerer::name::lower_name(&parameter.name),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                relevance: parameter.relevance,
            }
        }
    }
    pub(crate) mod type_parameters {
        use crate::lowerer::Lowerer;
        use crate::signatures::callable_signature::lower_state_signature;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_type_parameters(
            lowerer: &mut Lowerer,
            parameters: arena::HandleSpan<resolved::data::TypeParameter>,
        ) -> Result<arena::HandleSpan<typed::data::TypeParameter>, Diagnostic> {
            let parameters = lowerer
                .source_trees
                .data_type_parameters(parameters)
                .iter()
                .map(|parameter| lower_type_parameter(lowerer, parameter))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(lowerer
                .typed_trees
                .data_type_parameters
                .insert_many(parameters))
        }

        pub(crate) fn lower_type_parameter(
            lowerer: &mut Lowerer,
            parameter: &resolved::data::TypeParameter,
        ) -> Result<typed::data::TypeParameter, Diagnostic> {
            Ok(typed::data::TypeParameter {
                symbol: parameter.symbol,
                name: crate::lowerer::name::lower_name(&parameter.name),
                kind: lower_type_parameter_kind(lowerer, &parameter.kind)?,
                bounds: typed::data::DataProperties {
                    carry: parameter.bounds.carry,
                    multiplicity: parameter.bounds.multiplicity,
                },
            })
        }

        pub(crate) fn lower_type_parameter_kind(
            lowerer: &mut Lowerer,
            kind: &resolved::data::TypeParameterKind,
        ) -> Result<typed::data::TypeParameterKind, Diagnostic> {
            match kind {
                resolved::data::TypeParameterKind::Type => Ok(typed::data::TypeParameterKind::Type),
                resolved::data::TypeParameterKind::Implementation => {
                    Ok(typed::data::TypeParameterKind::Implementation)
                }
                resolved::data::TypeParameterKind::Const { type_reference } => {
                    Ok(typed::data::TypeParameterKind::Const {
                        type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                    })
                }
                resolved::data::TypeParameterKind::Value { type_reference } => {
                    Ok(typed::data::TypeParameterKind::Value {
                        type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                    })
                }
                resolved::data::TypeParameterKind::Machine { contract } => {
                    let contract = match contract {
                        resolved::data::MachineParameterContract::Inferred => {
                            return Err(Diagnostic::error(
                                "an inferred callable parameter reached typed lowering without an exact originating type",
                            ));
                        }
                        resolved::data::MachineParameterContract::RequirementIdentity => {
                            typed::data::MachineParameterContract::RequirementIdentity
                        }
                        resolved::data::MachineParameterContract::Structural(signature) => {
                            typed::data::MachineParameterContract::Structural(
                                lower_state_signature(lowerer, signature)?,
                            )
                        }
                        resolved::data::MachineParameterContract::AuthoredNominal { .. } => {
                            return Err(Diagnostic::error(
                                "an unresolved nominal machine-parameter requirement reached typed lowering",
                            ));
                        }
                        resolved::data::MachineParameterContract::Nominal {
                            trait_definition,
                            requirement,
                            authored_path,
                            trait_arguments,
                        } => {
                            let [trait_path @ .., requirement_name] = authored_path.as_slice()
                            else {
                                return Err(Diagnostic::error(
                                    "a nominal machine-parameter requirement lost its authored `Trait::requirement` path before typed lowering",
                                ));
                            };
                            if trait_path.is_empty() {
                                return Err(Diagnostic::error(
                                    "a nominal machine-parameter requirement lost its authored trait path before typed lowering",
                                ));
                            }
                            crate::type_reference::retain_path_selection(
                        &mut lowerer.typed_trees,
                        trait_path,
                        *trait_definition,
                        lowerer.type_reference_exposure,
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
                        "nominal machine-parameter trait requirement",
                    )?;
                            crate::type_reference::retain_static_path_selection(
                                &mut lowerer.typed_trees,
                                std::slice::from_ref(requirement_name),
                                *requirement,
                                lowerer.type_reference_exposure,
                                "nominal machine-parameter requirement",
                            )?;
                            let mut lowered_arguments = Vec::new();
                            for argument in
                                lowerer.source_trees.child_type_references(*trait_arguments)
                            {
                                lowered_arguments
                                    .push(lower_type_reference_into_table(lowerer, argument)?);
                            }
                            typed::data::MachineParameterContract::Nominal {
                                receiving_trait: *trait_definition,
                                receiving_arguments: lowered_arguments.clone(),
                                trait_definition: *trait_definition,
                                requirement: *requirement,
                                trait_arguments: lowered_arguments,
                                instantiated: None,
                            }
                        }
                    };
                    Ok(typed::data::TypeParameterKind::Machine { contract })
                }
                resolved::data::TypeParameterKind::Proposition { contract } => {
                    let mut parameters = arena::HandleSpan::empty();
                    for parameter in lowerer.source_trees.state_parameters(contract.parameters) {
                        let parameter = crate::signatures::parameters::lower_state_parameter(
                            lowerer, parameter,
                        )?;
                        lowerer
                            .typed_trees
                            .state_parameters
                            .append_to_span(&mut parameters, parameter);
                    }
                    Ok(typed::data::TypeParameterKind::Proposition {
                        contract: typed::data::PropositionParameterSignature {
                            name: crate::lowerer::name::lower_name(&contract.name),
                            parameters,
                        },
                    })
                }
            }
        }
    }
}
