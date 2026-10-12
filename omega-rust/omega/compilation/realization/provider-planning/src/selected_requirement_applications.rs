//! Project complete public applications onto their actual selected source row.
//!
//! The source declaration and full receiver scope come from retained selection
//! provenance. The public header owns its complete static invocation identity.
//! This relation is independent of binding mechanism: checked-body instances,
//! evaluated foreign payloads and compiler execution receipts keep their own
//! distinct custody rather than manufacturing a common implementation body.

use diagnostics::Diagnostic;

/// Shared exact original-row projection for settlement and product publication.
pub(crate) struct SelectedRequirementApplicationProjection<'program> {
    pub(crate) correspondence: effects::SelectedRequirementApplication,
    pub(crate) public:
        checked_trees::typed_trees::typed_trees::calls::PublicTraitRequirementSignature<'program>,
}

pub(crate) fn project_selected_requirement_applications<'program>(
    program: &'program checked_trees::typed_trees::TypedTrees,
    selections: &[crate::SelectedProviderReviewProvenance],
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    (
        Vec<SelectedRequirementApplicationProjection<'program>>,
        terminal_psi::TerminalIdentityTable,
    ),
    Vec<Diagnostic>,
> {
    let context = crate::ProviderPlanningContext::new(program, source::DependencyScope::Product);
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut applications = Vec::new();
    for selection in selections {
        let plan = &selection.plan;
        let provenance = &selection.provider;
        if selected.plan_by_exact_evidence(plan.report_fingerprint(), plan) != Some(plan)
            || plan.rows.len() != provenance.row_requirements.len()
            || plan.rows.len() != provenance.row_realizations.len()
            || plan.rows.len() != provenance.row_receiver_scopes.len()
            || plan.rows.len() != provenance.row_target_machine_origins.len()
        {
            return Err(vec![Diagnostic::error(
                "selected requirement application lost its complete original row custody",
            )]);
        }
        for (ordinal, row) in plan.rows.iter().enumerate() {
            let requirement = provenance.row_requirements[ordinal];
            let Some((owner, original)) = program.trait_machine_signature(requirement) else {
                // Machine and operator applications retain their genuine existing
                // specialization/family receipts, not synthetic public headers.
                continue;
            };
            if !owner.is_boundary
                || !context.contains(requirement)
                || program.trait_requirement_callable_identity_text(owner, original)
                    != plan.identity_text(row.requirement_identity)
            {
                return Err(vec![Diagnostic::error(
                    "selected requirement application changed its original public declaration",
                )]);
            }
            let mut produced = program
                .instantiated_requirement_signatures
                .iter()
                .filter(|(_, header)| {
                    header.requirement_template == original.symbol
                        && header.declaring_trait == owner.symbol
                })
                .map(|(_, header)| {
                    (
                        program
                            .trait_machine_signatures
                            .get(header.signature)
                            .symbol,
                        header.binding_application,
                    )
                })
                .chain(
                    program
                        .closed_requirement_applications()
                        .filter(|application| {
                            application.template == original.symbol
                                && application.declaring_trait == owner.symbol
                        })
                        .map(|application| (application.signature.symbol, application.application)),
                )
                .peekable();
            if produced.peek().is_none() {
                // A retained source declaration without produced applications
                // creates no applied correspondence, including toolchain rows.
                continue;
            }
            let scope = provenance.row_receiver_scopes[ordinal].as_ref()
                .and_then(|payload| payload.bind(program))
                .ok_or_else(|| vec![Diagnostic::error("selected requirement application lost its original producing receiver scope")])?;
            if provenance.schema.symbol() != scope.declaration.symbol
                || provenance.row_target_machine_origins[ordinal]
                    .as_ref()
                    .is_some_and(|origin| origin.target != plan.target)
            {
                return Err(vec![Diagnostic::error(
                    "selected requirement application changed its original scope or target",
                )]);
            }
            let mut produced_parameters = Vec::new();
            for (symbol, application) in produced {
                // The two inventories may name the same producer. Only exact
                // program-owned signature and application handles collapse.
                if produced_parameters.contains(&(symbol, application)) {
                    continue;
                }
                produced_parameters.push((symbol, application));
                // These inventories enumerate issued public producers, not
                // source invocations. A finite producer retains its outer
                // tuple in RequirementInstance and its local tuple in this
                // application; it does not carry a caller's complete interface
                // telescope. Rejoin its exact owned row here. Genuine calls
                // still use selected_public_trait_requirement_signature and
                // discharge their complete original invocation separately.
                let public = program
                    .public_trait_requirement_signature(symbol)
                    .filter(|public| {
                        public.owner.symbol == owner.symbol
                            && public.declaration.symbol == original.symbol
                            && public.application == application
                    })
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected requirement application lost its original public producer",
                        )]
                    })?;
                let signature = public.signature;
                if !program.requirement_application_matches_scope(signature.symbol, &scope) {
                    continue;
                }
                let identity = program.trait_requirement_callable_identity_text(owner, signature);
                if identity.is_empty() {
                    return Err(vec![Diagnostic::error(
                        "selected requirement application lost its complete public identity",
                    )]);
                }
                applications.push(SelectedRequirementApplicationProjection {
                    public,
                    correspondence: effects::SelectedRequirementApplication {
                        provider_plan_report_identity: plan.report_fingerprint(),
                        provider_plan_digest: plan.identity_digest(),
                        selected_requirement_identity: identities
                            .intern(plan.identity_text(row.requirement_identity)),
                        requirement_identity: identities.intern(&identity),
                    },
                });
            }
        }
    }
    Ok((applications, identities.build()))
}
