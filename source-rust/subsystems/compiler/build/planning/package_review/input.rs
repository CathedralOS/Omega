//! Owners retained only while deriving the policy of a checked occurrence.
use diagnostics::Diagnostic;
use source_compiler::publication::AcceptedTerminalSource;
use source_compiler::representations::source_state::TypedTrees;

pub(super) struct PolicySource<'a> {
    pub(super) carried_types: super::carried_types::CarriedTypes,
    pub(super) declarations: &'a source_compiler::CheckedPackageDeclarations,
    pub(super) accepted: &'a AcceptedTerminalSource,
    pub(super) sources: &'a package_compilation::CheckedSourceCustody,
    pub(super) evaluation: &'a build_execution::EvaluatedBuild,
}

impl<'a> PolicySource<'a> {
    pub(super) fn checked(
        declarations: &'a source_compiler::CheckedPackageDeclarations,
        sources: &'a package_compilation::CheckedSourceCustody,
        evaluation: &'a build_execution::EvaluatedBuild,
        inputs: &package_compilation::PackageCompilationInputs,
        target: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let accepted = declarations.accepted()?;
        sources
            .require_owner(accepted.source())
            .map_err(|diagnostic| vec![diagnostic])?;
        if accepted.is_invocation() || sources.package_identity().is_none() {
            return Err(vec![Diagnostic::error(
                "package review requires a complete checked package",
            )]);
        }
        Ok(Self {
            carried_types: super::carried_types::CarriedTypes::new(inputs, target)?,
            declarations,
            accepted,
            sources,
            evaluation,
        })
    }
    /// A bare service reference carries no authority over other applications.
    pub(super) fn unapplied_service(&self, symbol: symbols::SymbolHandle) ->
        Result<review_evidence::record::PackageReviewServiceIdentity, Vec<Diagnostic>> {
        let slot = self.typed().trait_definition_by_symbol(symbol)
            .filter(|slot| slot.is_boundary && slot.type_parameters.is_empty() && slot.lifetime_parameters.is_empty())
            .ok_or_else(|| super::declarations::unavailable("a boundary service's required static and lifetime application"))?;
        Ok(self.nominal(slot.symbol)?.into())
    }
    pub(super) fn reviewed_requirement_service(&self, service: &source_compiler::publication::RequirementService)
        -> Result<review_evidence::record::PackageReviewServiceIdentity, Vec<Diagnostic>> {
        Ok(review_evidence::record::PackageReviewServiceIdentity {
            declaration: self.nominal(service.declaration)?,
            application: service.application.as_ref().map(|canonical| review_evidence::record::PackageReviewTypeIdentity { canonical: canonical.clone() }),
        })
    }
    pub(super) fn effect_identity(
        &self,
        scope: &source_compiler::representations::source_state::trait_definition::AppliedTraitScope<'_>,
    ) -> Result<review_evidence::record::PackageReviewServiceIdentity, Vec<Diagnostic>> {
        let declaration = self.nominal(scope.declaration.symbol)?;
        if scope.declaration.type_parameters.is_empty() && scope.declaration.lifetime_parameters.is_empty() {
            return Ok(declaration.into());
        }
        let canonical = scope.portable_application_identity(self.typed())
            .and_then(|identity| self.typed().type_identity_text(identity))
            .ok_or_else(|| super::declarations::unavailable("a service's normalized static and lifetime inputs"))?;
        Ok(review_evidence::record::PackageReviewServiceIdentity {
            declaration,
            application: Some(review_evidence::record::PackageReviewTypeIdentity { canonical }),
        })
    }
    pub(super) fn schema_effect_scope(&self, symbol: symbols::SymbolHandle, schema: &provider_plans::ServiceSchema)
        -> Result<source_compiler::representations::source_state::trait_definition::AppliedTraitScope<'a>, Vec<Diagnostic>> {
        use source_compiler::representations::source_state::trait_definition::AppliedTraitScope;
        let source = self.typed();
        let declaration = source.trait_definition_by_symbol(symbol).filter(|slot| slot.is_boundary)
            .ok_or_else(|| super::declarations::unavailable("the owned declaration of a selected service"))?;
        if declaration.type_parameters.is_empty() && declaration.lifetime_parameters.is_empty() {
            return Ok(AppliedTraitScope::abstract_scope(declaration));
        }
        let requested = schema.root_application_identity()
            .ok_or_else(|| super::declarations::unavailable("a selected service's complete receiver application"))?;
        for root in source.applied_trait_roots() {
            let mut selected = None;
            root.try_visit(source, &mut |scope| {
                if scope.declaration.symbol == symbol
                    && scope.portable_application_identity(source).and_then(|id| source.type_identity_text(id)).as_deref() == Some(requested)
                {
                    selected = Some(scope.clone());
                }
                Ok(())
            }).map_err(super::declarations::unavailable)?;
            if let Some(scope) = selected { return Ok(scope); }
        }
        Err(super::declarations::unavailable("the typed source of the selected service application"))
    }
    pub(super) fn typed(&self) -> &'a TypedTrees {
        self.accepted.source()
    }
    pub(super) fn sources(&self) -> &'a package_compilation::CheckedSourceCustody {
        self.sources
    }
    pub(super) fn accepted(&self) -> &'a AcceptedTerminalSource {
        self.accepted
    }
    pub(super) fn build(&self) -> &build_evaluation_result::ComputedBuildConfig {
        &self.evaluation.configuration
    }

    pub(super) fn selected_build_symbol(
        &self,
    ) -> Result<Option<symbols::SymbolHandle>, Vec<Diagnostic>> {
        self.build()
            .executed_entry
            .as_ref()
            .map(|entry| entry.rejoin(self.typed()))
            .transpose()
    }
}
