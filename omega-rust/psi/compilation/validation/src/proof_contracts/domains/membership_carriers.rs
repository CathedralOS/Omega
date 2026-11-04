//! A domain over one carrier classifies only values of that carrier, whether
//! the membership is an executable test or a proof fact in a contract,
//! statement, or domain/data predicate. Seven fits both `u8` and `u16`, but
//! `7u8` is not a member of a `u16` domain; the program converts first.
//!
//! The carrier is a formation obligation judged before predicate expansion:
//! once a predicate is substituted, a mismatched subject can look like an
//! ordinary integer comparison or appear to select conflicting member copies.
//! Unsuffixed literals have no carrier of their own and land in the domain's
//! carrier, so only an operand with a retained type is compared.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::domain::DomainDefinition;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

pub fn validate_domain_membership_carriers(
    program: &dyn crate::CheckingInput,
    contracts: &arena::Arena<checked_trees::ContractProofFact>,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
) -> Result<(), Diagnostic> {
    for (expression, _) in program.expression_table.iter_expressions() {
        for test in program.expression_table.executable_domain_tests(expression) {
            let Some(domain) = program.domain_definition(test.domain) else {
                continue;
            };
            validate_membership_carrier(
                program,
                contracts,
                owners,
                expression,
                test.subject,
                domain,
                "executable domain test",
            )?;
        }
    }
    for (_, fact) in program.proof_facts.iter() {
        let typed_trees::domain::ProofFact::Membership(membership) = fact else {
            continue;
        };
        if !membership.value.is_valid() {
            continue;
        }
        let Some(domain) = program
            .domain_definition_for_instance(membership.domain_symbol, membership.semantic_domain)
            .or_else(|| program.domain_definition(membership.domain_symbol))
        else {
            continue;
        };
        validate_membership_carrier(
            program,
            contracts,
            owners,
            membership.value,
            membership.value,
            domain,
            "domain membership",
        )?;
    }
    Ok(())
}

/// Capacity-specialized declarations such as `[u8; 5]::Utf8` and
/// `[u8; 16]::Utf8` are one semantic domain with a declaration per carrier;
/// name resolution keeps one of them. The subject is judged against the
/// declaration of that exact semantic identity whose carrier it has.
fn validate_membership_carrier(
    program: &dyn crate::CheckingInput,
    contracts: &arena::Arena<checked_trees::ContractProofFact>,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
    owner_expression: ExpressionHandle,
    subject: ExpressionHandle,
    domain: &DomainDefinition,
    label: &str,
) -> Result<(), Diagnostic> {
    let Err(diagnostic) = validate_declaration_carrier(
        program,
        contracts,
        owners,
        owner_expression,
        subject,
        domain,
        label,
    ) else {
        return Ok(());
    };
    let same_identity = program.domain_definitions().iter().filter(|candidate| {
        candidate.symbol != domain.symbol
            && domain.semantic_id != language_semantics::SemanticDomainId::NULL
            && candidate.semantic_id == domain.semantic_id
    });
    for candidate in same_identity {
        if validate_declaration_carrier(
            program,
            contracts,
            owners,
            owner_expression,
            subject,
            candidate,
            label,
        )
        .is_ok()
        {
            return Ok(());
        }
    }
    Err(diagnostic)
}

fn validate_declaration_carrier(
    program: &dyn crate::CheckingInput,
    contracts: &arena::Arena<checked_trees::ContractProofFact>,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
    owner_expression: ExpressionHandle,
    subject: ExpressionHandle,
    domain: &DomainDefinition,
    label: &str,
) -> Result<(), Diagnostic> {
    let Some(subject_type) =
        crate::expression_owners::checked_expression_type_reference_from_exact_owner(
            program,
            contracts,
            owner_expression,
            subject,
            Some(owners),
        )
        .and_then(|reference| crate::unwrapped_type_reference(program, reference))
    else {
        // Open applications are judged after their binders are closed.
        // Missing type information grants no carrier correspondence.
        return Ok(());
    };
    let Some(target) = crate::unwrapped_type_reference(program, domain.target_type) else {
        return Ok(());
    };
    if crate::declared_carriers_match(program, subject_type, target) {
        return Ok(());
    }
    let substitutions = declared_type_equations(program, owners, owner_expression);
    let normalized_subject = executable_domain_carrier(program, subject_type, &substitutions);
    let normalized_target = executable_domain_carrier(program, target, &substitutions);
    if normalized_subject
        .zip(normalized_target)
        .is_some_and(|(subject, target)| {
            crate::value_custody::places::declared_carriers_match_with_substitutions(
                program,
                subject,
                target,
                &substitutions,
            )
        })
    {
        return Ok(());
    }
    let subject_type = normalized_subject.unwrap_or(subject_type);
    let target = normalized_target.unwrap_or(target);
    if mentions_type_parameter(program, subject_type) || mentions_type_parameter(program, target) {
        // A generic declaration is judged on each specialized copy, where the
        // binder has been replaced by the caller's exact carrier.
        return Ok(());
    }
    Err(Diagnostic::error(format!(
        "{label} `{}` expects subject carrier `{}`, got `{}`",
        domain.name.as_str(),
        program.display_type_reference_with_constraints(target),
        program.display_type_reference_with_constraints(subject_type),
    ))
    .with_source_span(program.expression_table.source_span(owner_expression)))
}

fn mentions_type_parameter(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
    let mut pending = vec![reference];
    let mut seen = Vec::new();
    while let Some(reference) = pending.pop() {
        if !reference.is_valid() || seen.contains(&reference) {
            continue;
        }
        seen.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                {
                    return true;
                }
            }
            TypeReferenceNode::Generic { arguments, .. }
            | TypeReferenceNode::DynamicTrait { arguments, .. } => {
                pending.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .copied(),
                );
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
        }
    }
    false
}

/// Apply the same shell query to an open declaration as to its closed type.
/// An equation can reveal another reference or constrained shell, so the
/// substitution and shell queries must compose until neither exposes more.
/// This is only the domain's carrier judgment; ordinary predicate reads still
/// prove access to the original subject and never gain authority from a peel.
fn executable_domain_carrier(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return None;
        }
        visited.push(reference);
        let substituted = crate::value_custody::places::substituted_declared_carrier(
            program,
            reference,
            substitutions,
        )?;
        let unwrapped = crate::unwrapped_type_reference(program, substituted)?;
        if unwrapped == substituted {
            return Some(unwrapped);
        }
        reference = unwrapped;
    }
}

fn declared_type_equations(
    program: &dyn crate::CheckingInput,
    owners: &crate::expression_owners::OwnerEnvironmentIndex,
    expression: ExpressionHandle,
) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
    let Some(owner) =
        crate::expression_owners::source_declaration_owner(program, expression, owners)
    else {
        return Vec::new();
    };
    crate::value_custody::places::declaration_type_equations(program, owner)
}
