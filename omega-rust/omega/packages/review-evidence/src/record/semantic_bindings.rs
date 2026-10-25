use effects::provider_plan::ServiceSchema;
use package_compilation::AcceptedSemanticBinding;

/// One compiler-issued semantic-binding proposal together with the exact
/// checked service schema from which a consumer may author permission rows.
///
/// The schema and any hosted permission rows are review proposals, not grants.
/// Permissions bind exact requirement identities to the compiler's hosted-leaf
/// classifications; consumer scoping removes them from non-root/build inputs.
/// The resulting binding still requires complete checked recompilation and
/// the project's explicit policy decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticBindingReviewCandidate {
    binding: AcceptedSemanticBinding,
    service_schema: ServiceSchema,
}

impl SemanticBindingReviewCandidate {
    /// Pair an already validated binding with its exact schema for review or
    /// cache recovery. This record confers no permission or admission authority.
    pub fn new(binding: AcceptedSemanticBinding, service_schema: ServiceSchema) -> Self {
        debug_assert_eq!(
            binding.normalized_schema_digest(),
            package_compilation::accepted_service_schema_digest(binding.role(), &service_schema,),
        );
        Self {
            binding,
            service_schema,
        }
    }

    pub const fn binding(&self) -> &AcceptedSemanticBinding {
        &self.binding
    }

    pub const fn service_schema(&self) -> &ServiceSchema {
        &self.service_schema
    }
}
