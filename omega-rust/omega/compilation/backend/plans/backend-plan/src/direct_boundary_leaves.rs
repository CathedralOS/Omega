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
    /// Complete original one-row satisfaction plan, before callable projection.
    pub semantic_plan: effects::provider_plan::ProviderPlan,
    pub plan: effects::provider_plan::ProviderPlan,
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
    /// Project one exact callable from its original checked satisfaction row.
    /// Target, provider provenance, binding and full method contract remain
    /// owned by the original plan; only the invocation coordinate changes.
    pub fn project_plan(
        semantic: &effects::provider_plan::ProviderPlan,
        callable: &str,
        boundary: &str,
    ) -> Result<effects::provider_plan::ProviderPlan, &'static str> {
        let [row] = semantic.rows.as_slice() else {
            return Err("direct boundary lost its one original satisfaction row");
        };
        if let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
            && machine != callable
        {
            return Err("direct intrinsic does not name its exact realization callable");
        }
        let requirement = semantic.identities.text(row.requirement_identity);
        let mut projected = semantic.clone();
        projected.schema.methods.retain(|method| {
            semantic.schema.identity_text(method.requirement_identity) == requirement
        });
        let [method] = projected.schema.methods.as_mut_slice() else {
            return Err("direct boundary lost its exact original requirement contract");
        };
        projected.schema.receiver_applications.clear();
        projected.name = format!("direct-boundary:{}:{callable}", semantic.target);
        projected.provider_type = callable.to_owned();
        projected.provider_type_package_identity = semantic.origin_package_identity;
        projected.schema.trait_name = projected.schema.identities.intern(boundary);
        projected.schema.trait_package_identity = semantic.origin_package_identity;
        projected.rows[0].requirement_identity = projected.identities.intern(boundary);
        method.requirement_identity = projected.schema.identities.intern(boundary);
        Ok(projected)
    }
}
