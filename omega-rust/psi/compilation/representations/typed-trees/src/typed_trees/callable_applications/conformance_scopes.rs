//! Preserve the public application selected by static evidence dispatch.
//! Report strings cannot reconstruct a named map's simultaneous Type/Const
//! telescope. Prepare the same retained source frames used by requirement
//! applications; checking still owns operand and selected-row admission.

use crate::TypedTrees;
use crate::name::Identifier;
use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use crate::typed_trees::ClosedConformanceApplicationSite;

impl TypedTrees {
    /// Portable identity of the complete selected implementation telescope.
    /// Its satisfied trait can omit phantom own arguments, so that receiver
    /// view cannot identify the implementation application itself.
    pub fn portable_conformance_application_identity(
        &self,
        application: &crate::typed_trees::ClosedConformanceApplication,
        supplying_application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Option<crate::type_identity::NormalizedTypeIdentity> {
        if !supplying_application.is_valid() {
            return None;
        }
        let source = self
            .expression_table
            .static_machine_application(supplying_application);
        self.portable_conformance_application_identity_in_owner(application, source.lexical_owner)
    }

    fn portable_conformance_application_identity_in_owner(
        &self,
        application: &crate::typed_trees::ClosedConformanceApplication,
        lexical_owner: symbols::SymbolHandle,
    ) -> Option<crate::type_identity::NormalizedTypeIdentity> {
        let selected = crate::expression::StaticMachineArgument {
            symbol: application.declaration,
            application: Some(Box::new(crate::expression::StaticSymbolApplication {
                member: Default::default(),
                lifetime_arguments: application
                    .lifetime_arguments
                    .iter()
                    .map(Identifier::generated)
                    .collect(),
                arguments: application.arguments.clone(),
            })),
            ..Default::default()
        };
        crate::type_identity::portable_applied_index_identity(
            self,
            crate::type_identity::IndexIdentityArgument::StaticArgument(&selected),
            lexical_owner,
            &[],
        )
    }

    /// Compare capture code inputs without confusing a structural type's arena
    /// address with its meaning. Each supplying occurrence retains its own
    /// ordinary formation/binding receipt; this query grants no source custody.
    /// The same original owner and complete receiving scope remain mandatory.
    pub fn closed_conformance_applications_match(
        &self,
        left: &crate::typed_trees::ClosedConformanceApplication,
        right: &crate::typed_trees::ClosedConformanceApplication,
    ) -> bool {
        if left == right {
            return true;
        }
        if !left.same_selection_metadata(right) {
            return false;
        }
        let (Some(left_scope), Some(right_scope)) =
            (left.trait_scope.scope(self), right.trait_scope.scope(self))
        else {
            return false;
        };
        let owner = left_scope.argument_owner;
        if !owner.is_valid()
            || owner != right_scope.argument_owner
            || !left_scope.same_application(self, &right_scope)
        {
            return false;
        }
        let left_identity = self.portable_conformance_application_identity_in_owner(left, owner);
        left_identity.is_some()
            && left_identity
                == self.portable_conformance_application_identity_in_owner(right, owner)
    }

    pub fn prepare_closed_conformance_scopes(&mut self) -> Result<(), String> {
        let pending = self
            .closed_conformance_application_sites()
            .into_iter()
            .map(|(site, _, supplying, application)| (site, supplying, application.clone()))
            .collect::<Vec<_>>();
        for (site, binding_application, application) in pending {
            if !binding_application.is_valid() {
                return Err(
                    "closed implementation application lost its supplying source application"
                        .into(),
                );
            }
            // Specialization owns the executable body, not its caller's regions.
            // The retained binding application preserves the supplying telescope.
            let owner = self
                .expression_table
                .static_machine_application(binding_application)
                .lexical_owner;
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.static_operand_type_reference(argument))
                .collect::<Result<Vec<_>, _>>()?;
            let lifetimes = application
                .lifetime_arguments
                .iter()
                .map(Identifier::generated)
                .collect::<Vec<_>>();
            let declaration = self
                .conformance_by_symbol(application.declaration)
                .ok_or_else(|| {
                    let source_owner = match site {
                        ClosedConformanceApplicationSite::TraitArgument(handle) => self
                            .trait_implementation_applications
                            .get(handle)
                            .source_scope
                            .supplying_owner(),
                        _ => symbols::SymbolHandle::invalid(),
                    };
                    format!(
                        "closed implementation application lost its declaration: site={site:?}, declaration={:?} ({:?}), supplying_application={binding_application:?}, generated_owner={owner:?} ({:?}), retained_source_owner={source_owner:?} ({:?})",
                        application.declaration,
                        self.symbols.display_path(application.declaration, "::"),
                        self.symbols.display_path(owner, "::"),
                        self.symbols.display_path(source_owner, "::"),
                    )
                })?;
            let scope = AppliedTraitScope::from_conformance_application(
                self,
                declaration,
                TraitApplicationArguments {
                    lexical_owner: owner,
                    enclosing_data_applications: &[],
                    arguments: &arguments,
                    lifetimes: &lifetimes,
                },
            )
            .ok_or("closed implementation application lost its complete source telescope")?;
            let scope = scope.environment().retain(self)?;
            self.closed_conformance_application_at_mut(site)
                .ok_or("closed implementation application lost its occurrence")?
                .trait_scope = scope;
        }
        Ok(())
    }
}
