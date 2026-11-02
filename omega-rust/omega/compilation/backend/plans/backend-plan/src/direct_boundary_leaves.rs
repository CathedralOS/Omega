//! Exact boundary callable dependencies, before executable demand selects them.
//!
//! A direct call names its leaf, not the whole-trait implementation whose
//! member may forward to it. Retain the satisfied contract and execution
//! projection together; none of this inventory supplies execution authority.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectBoundaryLeaf {
    /// The declaration this leaf names, in the vocabulary its consumer joins
    /// on: the hermetic package/toolchain identity when the declaration has
    /// one, and the exact `program-local::<path>` coordinate for a declaration
    /// authored in the compilation's own root program.
    pub declaration_identity: String,
    pub callable_identity: String,
    pub satisfied_schema: effects::provider_plan::ServiceSchema,
    pub plan: effects::provider_plan::ProviderPlan,
    pub satisfied_requirement_identity: String,
    /// Native projection may be unavailable for a checked but unexecuted leaf.
    /// Preserve its exact refusal until executable demand activates this row.
    pub execution: DirectBoundaryExecution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectBoundaryExecution {
    Foreign(Result<calling_conventions::ExternalBindingRow, Vec<diagnostics::Diagnostic>>),
    Intrinsic(Option<effects::CompilerIntrinsicExecutionIdentity>),
}

impl DirectBoundaryLeaf {
    /// Authenticate a callable route without changing its public declaration ABI.
    /// Original policy choices remain independently retained beside execution;
    /// self-consistent application bytes alone never authorize another policy.
    pub fn validate_calling_custody(&self) -> Result<(), String> {
        self.satisfied_schema
            .declaration
            .validate_receiver_applications()?;
        self.satisfied_schema.validate_calling_applications()?;
        let [row] = self.plan.rows.as_slice() else {
            return Err("direct boundary does not retain one callable row".into());
        };
        let [method] = self.plan.schema.declaration.methods.as_slice() else {
            return Err("direct boundary does not retain one callable contract".into());
        };
        let route = self.plan.identities.text(row.requirement_identity);
        if route.is_empty() {
            return Err("direct callable route has no requirement identity".into());
        }
        if self.plan.schema.identity_text(method.requirement_identity) != route {
            return Err("direct method coordinate differs from its callable route".into());
        }
        if self
            .plan
            .schema
            .identity_text(self.plan.schema.declaration.trait_name)
            != route
        {
            return Err("direct schema coordinate differs from its callable route".into());
        }
        if !self
            .plan
            .schema
            .declaration
            .receiver_applications
            .is_empty()
        {
            return Err("synthetic direct route retained a public receiver roster".into());
        }
        if !self.plan.schema.calling_applications.is_empty() {
            return Err("synthetic direct route relabelled public calling attachments".into());
        }
        let source = &self.satisfied_schema;
        let mut sources = source.declaration.methods.iter().filter(|method| {
            source.identity_text(method.requirement_identity) == self.satisfied_requirement_identity
        });
        let original = sources
            .next()
            .ok_or("direct boundary lost its original requirement")?;
        if sources.next().is_some() {
            return Err("direct boundary has competing original requirements".into());
        }
        let original_coordinate = self
            .plan
            .schema
            .declaration
            .identities
            .id_at(&self.satisfied_requirement_identity)
            .ok_or("direct callable table lost its original declaration coordinate")?;
        let mut projected = method.clone();
        projected.requirement_identity = original_coordinate;
        if !projected.eq_in(
            &self.plan.schema.declaration.identities,
            original,
            &source.declaration.identities,
        ) {
            return Err("direct callable changed its original semantic contract".into());
        }
        let DirectBoundaryExecution::Foreign(Ok(binding)) = &self.execution else {
            // A deferred refusal supplies no physical execution. Intrinsics
            // enter their own source-owned semantic settlement.
            return Ok(());
        };
        if binding.requirement_identity != route {
            return Err("direct foreign execution changed its callable coordinate".into());
        }
        let expected = source.calling_for_requirement(original.requirement_identity);
        if !expected.is_empty() && expected.len() != binding.calling_applications.len() {
            return Err(
                "direct foreign execution changed its complete selected calling roster".into(),
            );
        }
        for (position, application) in binding.calling_applications.iter().enumerate() {
            if binding.calling_applications[..position]
                .iter()
                .any(|previous| previous.abi == application.abi)
            {
                return Err(
                    "direct foreign execution repeats a declaration ABI application".into(),
                );
            }
            if application.requirement_identity != self.satisfied_requirement_identity
                || application.abi.requirement_identity != self.satisfied_requirement_identity
                || application.recomputed_commitment()? != application.commitment
            {
                return Err("direct foreign execution changed its public declaration ABI".into());
            }
            if !expected.is_empty() {
                let selected = source
                    .calling_application(&application.abi)
                    .ok_or("direct foreign execution has no such selected declaration ABI")?;
                if selected.report_fingerprint != application.report_fingerprint
                    || selected.commitment.as_bytes() != application.commitment
                {
                    return Err(
                        "direct foreign execution substituted its selected calling policy".into(),
                    );
                }
            }
        }
        Ok(())
    }

    /// Terminal binding changes the route coordinate, never either retained
    /// Source identity or its public ABI. Rejoin all three against this exact
    /// receiving module before the route can supply native execution.
    pub fn validate_bound_calling_custody(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<(), String> {
        self.validate_calling_custody()?;
        let route = self
            .plan
            .identities
            .text(self.plan.rows[0].requirement_identity);
        let count = module
            .boundary_machines
            .iter()
            .filter(|boundary| {
                module.identities.text(boundary.identity) == Some(route)
                    && boundary.declaration.as_ref().is_some_and(|declaration| {
                        module
                            .identities
                            .text(declaration.source.semantic_declaration)
                            == Some(self.declaration_identity.as_str())
                            && module.identities.text(declaration.source.callable)
                                == Some(self.callable_identity.as_str())
                    })
            })
            .count();
        if count != 1 {
            return Err(format!(
                "direct callable route rejoins {count} exact Source/Terminal declarations"
            ));
        }
        Ok(())
    }
}
