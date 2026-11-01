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
        if self.plan.identities.text(row.requirement_identity) != self.callable_identity
            || self.plan.schema.identity_text(method.requirement_identity) != self.callable_identity
            || self
                .plan
                .schema
                .identity_text(self.plan.schema.declaration.trait_name)
                != self.callable_identity
            || !self
                .plan
                .schema
                .declaration
                .receiver_applications
                .is_empty()
            || !self.plan.schema.calling_applications.is_empty()
        {
            return Err(
                "direct callable route substituted declaration or calling ownership".into(),
            );
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
        if binding.requirement_identity != self.callable_identity {
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
}
