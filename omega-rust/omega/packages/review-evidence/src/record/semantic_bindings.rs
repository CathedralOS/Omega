// Compiler-issued semantic-binding proposals paired with their exact checked
// service schemas (`SemanticBindingReviewCandidate`).
//
// The schema and any hosted permission rows are review proposals, not grants:
// permissions bind exact requirement identities to the compiler's hosted-leaf
// classifications, consumer scoping removes them from non-root/build inputs,
// and the resulting binding still requires complete checked recompilation and
// the project's explicit policy decision.

use crate::record::accessors::field_projection_accessors;
use effects::provider_plan::ServiceSchema;
use package_compilation::AcceptedSemanticBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticBindingReviewCandidate {
    binding: AcceptedSemanticBinding,
    service_schema: ServiceSchema,
}

impl SemanticBindingReviewCandidate {
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

    field_projection_accessors!(borrowed binding: &AcceptedSemanticBinding);
    field_projection_accessors!(borrowed service_schema: &ServiceSchema);
}
