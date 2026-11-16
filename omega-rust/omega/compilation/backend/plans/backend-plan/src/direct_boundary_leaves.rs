/*
We retain exact boundary callable dependencies before executable demand selects
them, pairing each satisfied service contract with its foreign or intrinsic
execution projection and authenticating calling custody against the receiving
Terminal module without granting execution authority.
*/

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectBoundaryLeaf {
    pub declaration_identity: String,
    pub callable_identity: String,
    pub satisfied_schema: effects::provider_plan::ServiceSchema,
    pub plan: effects::provider_plan::ProviderPlan,
    pub satisfied_requirement_identity: String,
    pub target_realization: Option<target::CanonicalFloatRealization>,
    pub execution: DirectBoundaryExecution, // Preserves exact refusal until executable demand activates this row.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectBoundaryExecution {
    Foreign(Result<calling_conventions::ExternalBindingRow, Vec<diagnostics::Diagnostic>>),
    Intrinsic(Option<effects::CompilerIntrinsicExecutionIdentity>),
}

impl DirectBoundaryLeaf {
    /// Keep the closed callable tied to its original method and complete ABI
    /// instance. Sharing a template or physical signature is insufficient.
    pub fn declared_calling_abi(
        module: &terminal_psi::TerminalModule,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<&terminal_psi::CallingAbiDeclaration, String> {
        let declaration = module
            .boundary_machine(id)
            .and_then(|boundary| boundary.declaration.as_ref())
            .ok_or("direct binding lost its checked callable declaration")?;
        let abi = module.boundary_calling_abi(id)?;
        if !declaration
            .matches_calling_abi(&module.identities, &abi.identity(&module.identities)?)?
        {
            return Err("direct binding has another declaration's calling ABI".into());
        }
        Ok(abi)
    }

    /// The complete source-issued application owning a directly bound raw
    /// requirement. A native machine realization uses its conformance instead.
    pub fn declared_schema(
        module: &terminal_psi::TerminalModule,
        id: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<&terminal_psi::TerminalServiceSchema, String> {
        let boundary = module
            .boundary_machine(id)
            .ok_or("direct binding names an absent boundary")?;
        if boundary.kind != semantic_vocabulary::BoundaryRequirementKind::Trait
            || boundary.role != terminal_psi::BoundaryDeclarationRole::Callable
            || boundary.numeric_intrinsic.is_some()
            || boundary.declaration.as_ref().is_none_or(|declaration| {
                declaration.execution != terminal_psi::CheckedBoundaryExecution::Provider
            })
        {
            return Err(
                "direct foreign binding needs an ordinary checked trait requirement".into(),
            );
        }
        let service = boundary
            .owning_service
            .ok_or("direct foreign requirement has no owning service application")?;
        let mut services = module.services.iter().filter(|row| row.id == service);
        let (Some(service), None) = (services.next(), services.next()) else {
            return Err("direct foreign requirement has no unique service application".into());
        };
        let schema = module.service_schema(boundary.owner.identity(), service.identity)?;
        let abi = Self::declared_calling_abi(module, id)?;
        let requirement = module
            .identities
            .text(abi.requirement_identity)
            .filter(|identity| !identity.is_empty())
            .ok_or("direct foreign requirement lost its canonical identity")?;
        let mut methods = schema.surface.methods.iter().filter(|method| {
            schema.surface.identity_text(method.requirement_identity) == requirement
        });
        if !matches!((methods.next(), methods.next()), (Some(_), None)) {
            return Err("direct foreign requirement has no unique complete declared method".into());
        }
        Ok(schema)
    }

    /// Plain requirements owe their own full checked contract and a completed
    /// Build locator. No external conformance substitutes for this association.
    pub fn validate_plain_execution(
        &self,
        module: &terminal_psi::TerminalModule,
        evaluated: &[effects::provider_plan::EvaluatedNativeBinding],
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<(), String> {
        self.validate_bound_calling_custody(module)?;
        let DirectBoundaryExecution::Foreign(Ok(binding)) = &self.execution else {
            return Err("plain requirement has no completed foreign execution".into());
        };
        if binding.target_name != self.plan.target
            || self.plan.provider_declaration.is_some()
            || !self.plan.provider_type.is_empty()
            || !binding.provider_type.is_empty()
        {
            return Err("plain binding changes its target or invents a named provider".into());
        }
        let mut receipts = evaluated.iter().filter(|row| {
            row.target().target_name() == self.plan.target
                && row.receipt().producer_package().is_some()
                && row.receipt().build_origin().is_some_and(|origin| {
                    identities.text(origin.requirement_identity)
                        == Some(
                            self.plan
                                .identities
                                .text(self.plan.rows[0].requirement_identity),
                        )
                })
        });
        let (Some(receipt), None) = (receipts.next(), receipts.next()) else {
            return Err("plain requirement has no unique completed Build locator receipt".into());
        };
        receipt.reintern_producer(
            identities,
            &mut terminal_psi::TerminalIdentityTableBuilder::default(),
        )?;
        if self.plan.rows[0].binding != receipt.provider_binding() {
            return Err("plain binding changes its evaluated locator or receipt".into());
        }
        match (receipt, &binding.binding) {
            (
                effects::provider_plan::EvaluatedNativeBinding::Import(import),
                calling_conventions::ExternalBindingKind::Import { locator },
            ) if import.locator() == locator => {}
            (
                effects::provider_plan::EvaluatedNativeBinding::Syscall(syscall),
                calling_conventions::ExternalBindingKind::Syscall { number },
            ) if syscall.number() == *number => {}
            _ => return Err("plain binding changes its normalized execution mechanism".into()),
        }
        Ok(())
    }

    /// Bind the projected callable to its exact receiving declaration before
    /// deriving any schema-keyed permission or executable realization.
    /// A retained but unused source declaration may be absent from this module.
    pub fn bind_to_terminal(
        &mut self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<bool, String> {
        let mut declarations = module.boundary_machines.iter().filter(|boundary| {
            boundary.declaration.as_ref().is_some_and(|declaration| {
                module.identities.text(
                    if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait
                        || declaration.execution != terminal_psi::CheckedBoundaryExecution::Provider
                    {
                        declaration.source.semantic_declaration
                    } else {
                        declaration.source.machine.identity()
                    },
                ) == Some(self.declaration_identity.as_str())
                    && module.identities.text(declaration.source.callable)
                        == Some(self.callable_identity.as_str())
            })
        });
        let Some(boundary) = declarations.next() else {
            return Ok(false);
        };
        if declarations.next().is_some() {
            return Err("direct boundary leaf has multiple exact Terminal declarations".into());
        }
        self.validate_calling_custody()?;
        let identity = module
            .identities
            .text(boundary.identity)
            .filter(|text| !text.is_empty())
            .ok_or("direct boundary leaf has no Terminal boundary identity")?;
        let declaration = std::sync::Arc::make_mut(&mut self.plan.schema.declaration);
        let terminal_requirement = declaration.identities.intern(identity);
        declaration.trait_name = terminal_requirement;
        self.plan.rows[0].requirement_identity = self.plan.identities.intern(identity);
        self.plan.rows[0].declared_requirement_identity = self.plan.rows[0].requirement_identity;
        declaration.methods[0].requirement_identity = terminal_requirement;
        if let DirectBoundaryExecution::Foreign(Ok(binding)) = &mut self.execution {
            binding.trait_name = identity.to_owned();
            binding.requirement_identity = identity.to_owned();
        }
        self.validate_bound_calling_custody(module)?;
        Ok(true)
    }

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

    pub fn validate_bound_calling_custody(
        &self,
        module: &terminal_psi::TerminalModule,
    ) -> Result<(), String> {
        self.validate_calling_custody()?;
        let route = self
            .plan
            .identities
            .text(self.plan.rows[0].requirement_identity);
        let mut matching = module.boundary_machines.iter().filter(|boundary| {
            module.identities.text(boundary.identity) == Some(route)
                && boundary.declaration.as_ref().is_some_and(|declaration| {
                    module.identities.text(
                        if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait
                            || declaration.execution
                                != terminal_psi::CheckedBoundaryExecution::Provider
                        {
                            declaration.source.semantic_declaration
                        } else {
                            declaration.source.machine.identity()
                        },
                    ) == Some(self.declaration_identity.as_str())
                        && module.identities.text(declaration.source.callable)
                            == Some(self.callable_identity.as_str())
                })
        });
        let boundary = matching
            .next()
            .ok_or("direct callable route has no exact receiving declaration")?;
        if matching.next().is_some() {
            return Err(
                "direct callable route rejoins multiple exact Source/Terminal declarations".into(),
            );
        }
        let numeric = boundary.declaration.as_ref().is_some_and(|declaration| {
            declaration.execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive
        });
        if numeric {
            let expected_schema =
                crate::canonical_schemas::numeric_requirement_schema(module, boundary)?;
            if self.satisfied_schema != expected_schema {
                return Err("numeric leaf changes its canonical declared schema".into());
            }
            let meaning = boundary
                .numeric_intrinsic
                .ok_or("numeric leaf lost its closed meaning")?;
            if self.execution != DirectBoundaryExecution::Intrinsic(Some(meaning.into())) {
                return Err("numeric leaf changes its declared operation".into());
            }
            let profile = target::TargetProfile::from_canonical_target_name(&self.plan.target)
                .map_err(|_| "numeric leaf has an unknown target")?;
            let expected = profile
                .canonical_float_realization(route)
                .ok_or("numeric leaf has no target table entry")?;
            if !matches!(
                expected,
                target::CanonicalFloatRealization::CatalogIntrinsic { .. }
            ) || self.target_realization != Some(expected)
            {
                return Err(
                    "numeric leaf differs from its target realization and admissions".into(),
                );
            }
        } else if self.target_realization.is_some() {
            return Err("a nonnumeric leaf carries a numeric target realization".into());
        }
        if numeric {
            return Ok(());
        }
        if let Some(terminal_psi::CheckedBoundaryExecution::HostedPrimitive {
            operation,
            requirement,
        }) = boundary
            .declaration
            .as_ref()
            .map(|declaration| declaration.execution)
        {
            let expected = crate::canonical_schemas::hosted_requirement_schema(
                module,
                operation,
                requirement,
            )?;
            let execution = match operation {
                language_core::sealed_intrinsics::HostedOperation::ProcessExit => {
                    effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
                }
                _ => return Err("hosted primitive has no exact native execution".into()),
            };
            if self.satisfied_schema != expected
                || self.execution != DirectBoundaryExecution::Intrinsic(Some(execution))
            {
                return Err("hosted primitive changes its canonical contract or execution".into());
            }
            return Ok(());
        }
        if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait {
            let schema = Self::declared_schema(module, boundary.id)?;
            let declaration = Self::declared_calling_abi(module, boundary.id)?;
            let requirement = module
                .identities
                .text(declaration.requirement_identity)
                .ok_or("direct foreign binding lost its declared template")?;
            if self.satisfied_schema.declaration.as_ref() != schema.surface.as_ref()
                || self.satisfied_requirement_identity != requirement
                || self.plan.rows[0].calling_signature != Some(declaration.id)
            {
                return Err(
                    "direct foreign binding changes its complete declared service contract".into(),
                );
            }
            let DirectBoundaryExecution::Foreign(Ok(binding)) = &self.execution else {
                return Err("plain foreign requirement has no evaluated foreign execution".into());
            };
            if binding.calling_applications.len() != 1 {
                return Err("direct foreign binding has no exact declared Calling instance".into());
            }
            let abi = declaration.identity(&module.identities)?;
            let application = binding
                .calling_application(&abi)
                .ok_or("direct foreign binding changes its exact declaration ABI")?;
            if application.recomputed_commitment()? != application.commitment {
                return Err("direct foreign binding changes its Calling commitment".into());
            }
        }
        Ok(())
    }
}
