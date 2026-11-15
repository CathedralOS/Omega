//! Review the native bindings the package's Build evaluated for this target.
//! Target settlement alone omits bindings no runtime entry reaches.

use super::{
    PackageReviewInput, calling,
    declarations::unavailable,
    providers::{AcceptedDeclarations, symbol_key},
};
use crate::record::{
    PackagePolicyCallable, PackagePolicyEvaluatedBindingProducer, PackagePolicyNativeBinding,
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackageReviewForeignLocator, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{EvaluatedNativeBinding, ServiceSchema};
use provider_planning::target_supply::NormalizedNativeSupply;
use semantic_vocabulary::{ForeignLocatorCandidate, PackageKeyIdentity};
use target::TargetProfile;

impl PackageReviewInput<'_> {
    pub(super) fn native_binding_policy(
        &self,
        package: PackageKeyIdentity,
        target: TargetProfile,
        declarations: &AcceptedDeclarations<'_>,
    ) -> Result<Vec<(PackagePolicyNativeBinding, PackagePolicyCallable)>, Vec<Diagnostic>> {
        let checked = self.compilation();
        let realization = checked.execution().realization();
        let selected = realization.evaluated_native_bindings();
        let supplies = checked
            .build()
            .config
            .native_bindings
            .iter()
            .filter(|supply| supply.selection.target.canonical_name() == target.target_name());
        if supplies.clone().next().is_none() && selected.is_empty() {
            return Ok(Vec::new());
        }
        let publication = checked.terminal_publication();
        let accepted = declarations.accepted;
        let module = accepted.module();
        let source = self.typed();
        let mut requirements = accepted
            .boundary_requirements_declared()
            .map_err(|reason| vec![reason])?;
        let key = |row: &source_compiler::DeclaredBoundaryRequirement| {
            (symbol_key(row.slot), symbol_key(row.requirement))
        };
        requirements.sort_unstable_by_key(key);
        if requirements
            .windows(2)
            .any(|pair| key(&pair[0]) == key(&pair[1]))
        {
            return Err(unavailable(
                "one accepted declaration for a native requirement",
            ));
        }
        let mut slots = supplies
            .clone()
            .map(|supply| supply.selection.subject)
            .collect::<Vec<_>>();
        slots.sort_unstable_by_key(|slot| symbol_key(*slot));
        slots.dedup();
        let schemas = slots
            .into_iter()
            .map(|slot| {
                let schema = accepted
                    .slot_schema(slot)
                    .map(ServiceSchema::from_declaration)
                    .ok_or_else(|| {
                        unavailable("the native binding's complete requirement schema")
                    })?;
                let mut methods = (0..schema.declaration.methods.len()).collect::<Vec<_>>();
                methods.sort_unstable_by_key(|index| {
                    schema.identity_text(schema.declaration.methods[*index].requirement_identity)
                });
                Ok::<_, Vec<Diagnostic>>((slot, schema, methods))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut normalized = Vec::new();
        let mut policies = Vec::new();
        for supply in supplies {
            let selection = &supply.selection;
            if selection.supplier_package != package
                || selection.producer.package != package
                || selection.application.is_some()
            {
                return Err(unavailable(
                    "the native binding's complete local Build custody",
                ));
            }
            let index = requirements
                .binary_search_by_key(
                    &(
                        symbol_key(selection.subject),
                        symbol_key(selection.requirement),
                    ),
                    key,
                )
                .map_err(|_| unavailable("the native binding's exact accepted requirement"))?;
            let declaration = &requirements[index];
            let boundary = module
                .boundary_machine(declaration.boundary)
                .ok_or_else(|| unavailable("the native binding's checked boundary"))?;
            let (service, signature) = source
                .trait_machine_signature(declaration.requirement)
                .filter(|(service, signature)| {
                    service.symbol == declaration.slot
                        && signature.symbol == declaration.requirement
                })
                .ok_or_else(|| unavailable("the native binding's authored requirement"))?;
            let canonical = source.trait_requirement_callable_identity_text(service, signature);
            let boundary_callable = boundary
                .declaration
                .as_ref()
                .and_then(|declaration| module.identities.text(declaration.source.callable));
            if boundary_callable != Some(canonical.as_str()) {
                return Err(unavailable(
                    "the native binding's complete callable identity",
                ));
            }
            let binding = provider_planning::native_bindings::normalize_native_binding(
                supply,
                &canonical,
                &mut identities,
            )
            .map_err(|reason| unavailable(&reason))?;
            let producer = PackagePolicyEvaluatedBindingProducer {
                declaration: PackageReviewNominalIdentity {
                    owner: PackageReviewNominalOwner::Package(package),
                    path: selection.producer.declaration_path.clone(),
                },
                package: Some(package),
                callable_identity: selection.producer.callable_identity.clone(),
            };
            let policy_binding = binding_policy(&binding, producer);
            let schema_index = schemas
                .binary_search_by_key(&symbol_key(declaration.slot), |(slot, _, _)| {
                    symbol_key(*slot)
                })
                .map_err(|_| unavailable("the exact native binding service schema"))?;
            let (_, schema, methods) = &schemas[schema_index];
            let matches = |index: usize| {
                schema.identity_text(schema.declaration.methods[index].requirement_identity)
            };
            let first = methods.partition_point(|&index| matches(index) < canonical.as_str());
            let last = methods.partition_point(|&index| matches(index) <= canonical.as_str());
            let [index] = &methods[first..last] else {
                return Err(unavailable("one native binding method in the exact schema"));
            };
            let method = &schema.declaration.methods[*index];
            let (method, callable) = self.checked_method(
                service,
                signature,
                boundary,
                method,
                schema,
                self.nominal(signature.symbol)?,
                self.nominal(service.symbol)?,
                declarations,
            )?;
            normalized.push(NormalizedNativeSupply {
                boundary: boundary.id,
                binding,
            });
            policies.push((
                PackagePolicyNativeBinding {
                    application: None,
                    method,
                    binding: policy_binding,
                },
                callable,
            ));
        }
        let mut normalized_receipts = normalized
            .iter()
            .map(|row| (row.boundary, row.binding.receipt().identity_digest()))
            .collect::<Vec<_>>();
        normalized_receipts.sort_unstable();
        if normalized_receipts
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(unavailable("the complete selected native supply custody"));
        }
        for binding in selected {
            let origin = binding.receipt().build_origin().ok_or_else(|| {
                unavailable("the selected native binding's retained Build origin")
            })?;
            let requirement = realization
                .evaluated_binding_identities()
                .text(origin.requirement_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    unavailable("the selected native binding's exact requirement identity")
                })?;
            if binding.target().target_name() != target.target_name()
                || !normalized_receipts.iter().any(|(boundary, receipt)| {
                    *receipt == binding.receipt().identity_digest()
                        && module
                            .boundary_machine(*boundary)
                            .and_then(|boundary| boundary.declaration.as_ref())
                            .and_then(|declaration| {
                                module.identities.text(declaration.source.callable)
                            })
                            == Some(requirement)
                })
            {
                return Err(unavailable("the complete selected native supply custody"));
            }
        }
        let program = publication
            .published()
            .ok_or_else(|| unavailable("the checked program for native calling policy"))?;
        let program = provider_planning::calling_policy_plans::CallingProgram {
            artifact: std::sync::Arc::clone(program.artifact()),
            evaluator: build_settings::CALLING_POLICY_EVALUATOR,
        };
        let applications = provider_planning::calling_policy_plans::native_calling_applications(
            target,
            &program,
            &normalized,
        )?;
        if applications.len() != normalized.len() {
            return Err(unavailable(
                "one validated calling application per native binding",
            ));
        }
        // The planner preserves the binding order, after rejoining each
        // exact requirement to the calling declaration in this artifact.
        for (((policy, _), row), application) in
            policies.iter_mut().zip(&normalized).zip(applications)
        {
            let requirement = row
                .binding
                .receipt()
                .build_origin()
                .and_then(|origin| identities.as_table().text(origin.requirement_identity));
            if requirement != Some(application.requirement_identity.as_str()) {
                return Err(unavailable(
                    "the native binding's exact calling requirement",
                ));
            }
            policy
                .method
                .calling
                .push(calling::project(&application, &policy.method, target)?);
        }
        policies.sort_by(|(left, _), (right, _)| left.key().cmp(&right.key()));
        Ok(policies)
    }
}

fn binding_policy(
    binding: &EvaluatedNativeBinding,
    producer: PackagePolicyEvaluatedBindingProducer,
) -> PackagePolicyProviderBinding {
    match binding {
        EvaluatedNativeBinding::Import(import) => {
            let locator = import.locator();
            PackagePolicyProviderBinding::Import {
                target: locator.target().identity().as_str().to_owned(),
                locator: match locator.locator() {
                    ForeignLocatorCandidate::PeByName { library, export } => {
                        PackageReviewForeignLocator::PeByName {
                            library: library.clone(),
                            export: export.clone(),
                        }
                    }
                    ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        PackageReviewForeignLocator::PeByOrdinal {
                            library: library.clone(),
                            ordinal: *ordinal,
                        }
                    }
                    ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => PackageReviewForeignLocator::ElfVersioned {
                        object: object.clone(),
                        symbol: symbol.clone(),
                        version: version.clone(),
                    },
                    ForeignLocatorCandidate::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => PackageReviewForeignLocator::MachODylibSymbol {
                        install_name: install_name.clone(),
                        symbol: symbol.clone(),
                    },
                },
                producer,
            }
        }
        EvaluatedNativeBinding::Syscall(syscall) => PackagePolicyProviderBinding::Syscall {
            number: syscall.number(),
            evaluated: Some(PackagePolicyProviderEvaluatedSyscall {
                target: syscall.target().identity().as_str().to_owned(),
                producer,
            }),
        },
    }
}
