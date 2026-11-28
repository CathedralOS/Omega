use crate::source_state::TypedTrees;
use crate::source_state::name::Identifier;

impl TypedTrees {
    pub fn portable_conformance_application_identity(
        &self,
        application: &crate::source_state::ClosedConformanceApplication,
        supplying_application: crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) -> Option<crate::source_state::type_identity::NormalizedTypeIdentity> {
        if !supplying_application.is_valid() {
            return None;
        }
        let source = self.static_machine_application(supplying_application);
        self.portable_conformance_application_identity_in_owner(application, source.lexical_owner)
    }

    pub(crate) fn portable_conformance_application_identity_in_owner(
        &self,
        application: &crate::source_state::ClosedConformanceApplication,
        lexical_owner: symbols::SymbolHandle,
    ) -> Option<crate::source_state::type_identity::NormalizedTypeIdentity> {
        let lexical_owner = if application.trait_scope.supplying_owner().is_valid() {
            application.trait_scope.scope(self)?.argument_owner
        } else {
            lexical_owner
        };
        let selected = crate::source_state::expression::StaticMachineArgument {
            symbol: application.declaration,
            application: Some(Box::new(
                crate::source_state::expression::StaticSymbolApplication {
                    member: Default::default(),
                    lifetime_arguments: application
                        .lifetime_arguments
                        .iter()
                        .map(Identifier::generated)
                        .collect(),
                    arguments: application.arguments.clone(),
                },
            )),
            ..Default::default()
        };
        crate::source_state::type_identity::portable_applied_index_identity(
            self,
            crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(&selected),
            lexical_owner,
            &[],
        )
    }
}
