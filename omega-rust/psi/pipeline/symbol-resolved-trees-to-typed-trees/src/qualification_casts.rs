use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionTarget,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Bind semantic `as ... in Domain` sites to declaration identity. Validation
/// owns the diagnostic policy; this pass only publishes deterministic
/// identities so no checked consumer re-resolves a user spelling.
pub(crate) fn normalize_qualification_casts(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    normalize_qualification_casts_from(source, program, 0)
}

/// Bind only semantic casts appended after a retained checkpoint.
pub(crate) fn normalize_qualification_casts_from(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    expression_frontier: usize,
) -> Result<(), Diagnostic> {
    let sites = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            if usize::try_from(handle.arena_index()).is_ok_and(|index| index <= expression_frontier)
            {
                return None;
            }
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            Some((handle, *cast))
        })
        .collect::<Vec<_>>();
    let mut updates = Vec::with_capacity(sites.len());

    for (handle, cast) in sites {
        if cast.semantic_domain.is_empty() {
            let result = if cast.domain == numerics::arithmetic::ArithmeticDomain::Exact {
                Some(cast.target_type)
            } else if let TypeReferenceNode::Named { symbol, .. } = program
                .type_reference_table
                .type_reference(cast.target_type)
            {
                program
                    .type_reference_table
                    .find_arithmetic_result_type_reference(*symbol, cast.domain)
            } else {
                program
                    .type_reference_table
                    .find_policy_qualified_type_reference(cast.target_type, cast.domain)
            };
            if let ExpressionNode::Cast(target) = program.expression_table.expression_mut(handle) {
                target.result_type = result.unwrap_or_default();
            }
            continue;
        }
        let matching_occurrences = program
            .expression_table
            .authored_selection_occurrences(handle)
            .filter(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(*occurrence)
                    .is_some_and(|selection| {
                        selection.kind() == AuthoredDeclarationSelectionKind::DomainMembership
                    })
            })
            .collect::<Vec<_>>();
        let [authored_occurrence] = matching_occurrences.as_slice() else {
            return Err(Diagnostic::error(format!(
                "semantic qualification cast retains {} authored domain-selection occurrences; expected exactly one",
                matching_occurrences.len(),
            )));
        };
        let authored_occurrence = *authored_occurrence;
        let Some(authored_selection) = program
            .authored_declaration_selections()
            .get(authored_occurrence)
        else {
            return Err(Diagnostic::error(
                "semantic qualification cast retains an unknown authored domain-selection occurrence",
            ));
        };
        if authored_selection.source_span().span.start >= authored_selection.source_span().span.end
            || authored_selection.target()
                != AuthoredDeclarationSelectionTarget::LateBound(
                    AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                )
        {
            return Err(
                Diagnostic::error(
                    "semantic qualification cast authored domain custody is empty or has the wrong binding family",
                )
                .with_source_span(authored_selection.source_span()),
            );
        }
        let name = program
            .expression_table
            .name_path_members(cast.semantic_domain)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let arguments = program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
            .to_vec();
        let carrier_label = program.display_type_reference_with_constraints(cast.target_type);
        let matches = program
            .domain_definitions()
            .iter()
            .filter(|domain| {
                same_semantic_name(domain.name.as_str(), &name)
                    && crate::domain_constraints::domain_accepts_carrier(
                        program,
                        domain,
                        cast.target_type,
                        &carrier_label,
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
        let [domain] = matches.as_slice() else {
            updates.push((
                handle,
                authored_occurrence,
                SymbolHandle::invalid(),
                language_semantics::SemanticDomainId::NULL,
            ));
            continue;
        };
        let index_parameters = typed_trees::domain::index_parameters(program, domain);
        if arguments.len() != index_parameters.len() {
            return Err(Diagnostic::error(format!(
                "domain family `{}` requires {} closed index argument(s), but {} were supplied by `as ... in {name}`",
                domain.name,
                index_parameters.len(),
                arguments.len(),
            )));
        }
        let instance_name = typed_trees::domain::indexed_domain_instance_name(
            program,
            domain,
            index_parameters,
            &arguments,
        )?;
        let semantic_id = if arguments.is_empty() {
            domain.semantic_id
        } else {
            program.semantic_domains.intern(&instance_name)
        };
        updates.push((handle, authored_occurrence, domain.symbol, semantic_id));
    }

    let mut selections = program.authored_declaration_selections().clone();
    for (_, authored_occurrence, domain, _) in &updates {
        if !domain.is_valid() {
            continue;
        }
        selections
            .finalize_late_bound(
                *authored_occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                *domain,
            )
            .map_err(|error| {
                Diagnostic::error(format!(
                    "failed to finalize semantic qualification-cast domain custody: {error:?}",
                ))
            })?;
    }
    program.retain_authored_declaration_selections(selections);

    for (handle, _, domain, semantic_id) in updates {
        let ExpressionNode::Cast(authored) = program.expression_table.expression(handle) else {
            continue;
        };
        let authored = *authored;
        let result_type = if let Some(declaration) = program
            .domain_definitions()
            .iter()
            .find(|declaration| declaration.symbol == domain)
        {
            // The cast owns this result edge. Reuse ordinary domain normalization
            // for alias expansion, indexed identity, predicates and routes; a
            // fabricated bare carrier or a consumer-side name search would erase
            // meaning before a surrounding Match or operator checks compatibility.
            // This derived type does not create another authored selection or
            // establish membership: the original cast still owes that proof.
            let qualification = DomainConstraint {
                name: declaration.name.clone(),
                arguments: program
                    .type_reference_table
                    .type_reference_handles(authored.semantic_domain_arguments)
                    .to_vec(),
                ..DomainConstraint::default()
            };
            let constraints = program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(qualification)]);
            let result = program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: authored.target_type,
                    constraints,
                });
            crate::domain_constraints::normalize_domain_constraints_for_type(
                source, program, result,
            )?;
            result
        } else {
            TypeReferenceHandle::invalid()
        };
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        cast.semantic_domain_symbol = domain;
        cast.semantic_domain_id = semantic_id;
        cast.result_type = result_type;
    }
    Ok(())
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}
