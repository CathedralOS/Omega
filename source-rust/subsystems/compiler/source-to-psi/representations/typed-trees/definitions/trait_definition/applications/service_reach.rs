//! Bind service effects to checked trait applications. A reach row closes over
//! the declared parents, while an invocation keeps the directly named receiver.
use super::{
    AppliedTraitScope, TraitApplicationArguments, TraitApplicationEnvironment, TraitDefinition,
};
use crate::source_state::{TypedTrees, types::TypeReferenceNode};

impl TypedTrees {
    /// Recover the exact application carried by this effect coordinate, including
    /// the lexical owner and each substituted parent frame.
    pub fn service_reach_scope(
        &self,
        id: semantic_vocabulary::ServiceReachId,
    ) -> Option<AppliedTraitScope<'_>> {
        let row = self.service_reaches.definition(id)?;
        if let Some(application) = &row.application {
            return application.arguments.scope(self);
        }
        let declaration = self.trait_definition_by_symbol(row.symbol)?;
        if !declaration.is_boundary
            || !declaration.type_parameters.is_empty()
            || !declaration.lifetime_parameters.is_empty()
        {
            return None;
        }
        AppliedTraitScope::new(
            self,
            declaration,
            TraitApplicationArguments {
                lexical_owner: declaration.symbol,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
    }

    /// Describe an effect as seen by a selected service receiver. Service
    /// slots and their inherited parents use complete substituted identities.
    pub fn service_effect_scope<'program>(
        &'program self,
        id: semantic_vocabulary::ServiceReachId,
        receiver: Option<&AppliedTraitScope<'program>>,
    ) -> Result<AppliedTraitScope<'program>, &'static str> {
        let member = self
            .service_reach_scope(id)
            .ok_or("effect requires its complete boundary service application")?;
        Ok(match receiver {
            Some(owner) => member.preceded_by(owner),
            None => member,
        })
    }

    /// Form an explicitly written service row under the declaration's binder telescope.
    pub fn checked_service_scope<'a>(
        &'a self,
        declaration: &'a TraitDefinition,
        application: TraitApplicationArguments<'_>,
    ) -> Result<AppliedTraitScope<'a>, &'static str> {
        use crate::source_state::data::TypeParameterKind;
        let scope = AppliedTraitScope::new(self, declaration, application)
            .ok_or("wrong number of static or lifetime arguments for the boundary service")?;
        for (parameter, argument) in self
            .trait_type_parameters(declaration)
            .iter()
            .zip(application.arguments)
        {
            match parameter.kind {
                TypeParameterKind::Const { type_reference } => {
                    let Some(expected) = self.primitive_type_reference(type_reference) else {
                        return Err(
                            "an applied service const binder requires a supported scalar carrier",
                        );
                    };
                    if !crate::source_state::type_identity::scoped_scalar_argument_matches(
                        self,
                        *argument,
                        expected,
                        &[],
                        &[],
                    ) {
                        return Err(
                            "the boundary service const argument does not inhabit its declared carrier",
                        );
                    }
                }
                TypeParameterKind::Type => {
                    if matches!(
                        self.type_reference_node(*argument),
                        TypeReferenceNode::ConstExpression(_)
                    ) || self
                        .static_operand_from_type_reference(*argument)
                        .is_ok_and(|operand| {
                            self.data_type_parameter_by_symbol(operand.symbol)
                                .is_some_and(|binder| {
                                    matches!(
                                        binder.kind,
                                        TypeParameterKind::Const { .. }
                                            | TypeParameterKind::Value { .. }
                                    )
                                })
                                || self.const_declaration_by_symbol(operand.symbol).is_some()
                        })
                    {
                        return Err("the boundary service type parameter received a value");
                    }
                }
                _ => return Err("the applied service has an unsupported binder kind"),
            }
        }
        Ok(scope)
    }

    /// Close the authored application over the parent's own argument telescope.
    /// The entries axis uses the canonical ID alone and never takes this closure.
    pub fn settle_applied_service_lineage(&mut self) -> Result<(), &'static str> {
        let mut known = std::collections::BTreeMap::new();
        let mut pending = self
            .service_reaches
            .all_definitions()
            .filter_map(|(id, row)| row.application.is_some().then_some(id))
            .collect::<std::collections::VecDeque<_>>();
        for id in &pending {
            self.service_reach_scope(*id)
                .ok_or("an applied effect lost its declaration")?
                .try_visit(self, &mut |_| Ok(()))?;
        }
        let mut completed = std::collections::BTreeSet::new();
        while let Some(id) = pending.pop_front() {
            if !completed.insert(id.0) {
                continue;
            }
            let scope = self
                .service_reach_scope(id)
                .ok_or("an applied service lost its checked boundary")?;
            let identity = scope
                .portable_application_identity(self)
                .and_then(|id| self.type_identity_text(id))
                .ok_or("an applied service lost its normalized identity")?;
            let symbol = scope.declaration.symbol;
            let canonical = *known
                .entry((symbol.arena_index(), symbol.generation(), identity))
                .or_insert(id);
            let parents = scope
                .parent_scopes(self)?
                .into_iter()
                .filter(|scope| scope.declaration.is_boundary)
                .map(|scope| scope.environment())
                .collect::<Vec<_>>();
            self.service_reaches.set_canonical_id(id, canonical);
            let mut ids = Vec::new();
            for parent in parents {
                let parent = self.intern_service_reach_environment(&parent)?;
                pending.push_back(parent);
                ids.push(parent);
            }
            self.service_reaches.set_parents(id, ids);
        }
        Ok(())
    }

    pub(crate) fn intern_service_reach_environment(
        &mut self,
        environment: &TraitApplicationEnvironment,
    ) -> Result<semantic_vocabulary::ServiceReachId, &'static str> {
        let scope = environment
            .scope(self)
            .ok_or("service substitution lost its checked declaration")?;
        let symbol = scope.declaration.symbol;
        let name = scope.declaration.name.as_str().to_owned();
        let identity = scope
            .portable_application_identity(self)
            .and_then(|identity| self.type_identity_text(identity))
            .ok_or("substituted service lacks a portable application identity")?;
        if let Some((id, _)) = self.service_reaches.all_definitions().find(|(id, row)| {
            row.symbol == symbol
                && self.service_reach_application_identity(*id).as_ref() == Some(&identity)
        }) {
            return Ok(id);
        }
        let arguments = environment.retain(self)?;
        Ok(self
            .service_reaches
            .push_derived(crate::source_tables::ServiceReachDefinition {
                symbol,
                name,
                canonical: None,
                parents: Vec::new(),
                application: Some(crate::source_tables::ServiceReachApplication {
                    lexical_owner: arguments.supplying_owner(),
                    arguments,
                }),
            }))
    }

    pub fn service_reach_application_identity(
        &self,
        id: semantic_vocabulary::ServiceReachId,
    ) -> Option<String> {
        let identity = self
            .service_reach_scope(id)?
            .portable_application_identity(self)?;
        self.type_identity_text(identity)
    }
}
