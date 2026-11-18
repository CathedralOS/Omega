//! Review the native bindings the package's Build evaluated for this target.
//! Target settlement alone omits bindings no runtime entry reaches.

use super::{
    calling,
    declarations::unavailable,
    input::PolicySource,
    providers::{AcceptedDeclarations, symbol_key},
};
use crate::record::{
    PackagePolicyCallable, PackagePolicyEvaluatedBindingProducer, PackagePolicyNativeBinding,
    PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall,
    PackagePolicyServiceMethod, PackageReviewForeignLocator, PackageReviewNominalIdentity,
    PackageReviewNominalOwner,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{EvaluatedNativeBinding, ServiceSchema};
use semantic_vocabulary::{ForeignLocatorCandidate, PackageKeyIdentity};
use target::TargetProfile;

/// An authored binding's source identity and checked method. Its final calling
/// plan and native receipt arrive with the selected target.
pub(super) struct PreparedNativeBindings {
    target: TargetProfile,
    rows: Vec<NativeDeclaration>,
    declared: Vec<(PackagePolicyServiceMethod, PackagePolicyCallable)>,
}

struct NativeDeclaration {
    policy_index: usize,
    boundary: semantic_vocabulary::BoundaryMachineId,
    callable: String,
    abi: semantic_vocabulary::CallingAbiIdentity,
    materialization: evaluation_receipts::NativeBindingMaterialization,
    application: Option<String>,
    producer: PackagePolicyEvaluatedBindingProducer,
}

pub(super) fn prepare(
    input: &PolicySource<'_>,
    package: PackageKeyIdentity,
    target: TargetProfile,
    declarations: &AcceptedDeclarations<'_>,
) -> Result<PreparedNativeBindings, Vec<Diagnostic>> {
    let supplies = input
        .build()
        .config
        .native_bindings
        .iter()
        .filter(|supply| supply.selection.target.canonical_name() == target.target_name());
    let mut rows = Vec::new();
    let mut declared = Vec::new();
    if supplies.clone().next().is_none() {
        return Ok(PreparedNativeBindings {
            target,
            rows,
            declared: Vec::new(),
        });
    }
    let accepted = declarations.accepted;
    let module = accepted.module();
    let source = input.typed();
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
                .ok_or_else(|| unavailable("the native binding's complete requirement schema"))?;
            let mut methods = (0..schema.declaration.methods.len()).collect::<Vec<_>>();
            methods.sort_unstable_by_key(|index| {
                schema.identity_text(schema.declaration.methods[*index].requirement_identity)
            });
            Ok::<_, Vec<Diagnostic>>((slot, schema, methods))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for supply in supplies {
        let selection = &supply.selection;
        if selection.supplier_package != package || selection.producer.package != package {
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
                service.symbol == declaration.slot && signature.symbol == declaration.requirement
            })
            .ok_or_else(|| unavailable("the native binding's authored requirement"))?;
        let canonical = source.trait_requirement_callable_identity_text(service, signature);
        let application = selection
            .application
            .map(|handle| {
                let handle =
                    source_compiler::evaluation::provider_declarations::source_application(handle);
                let closed = source
                    .product_requirement_application(handle)
                    .map_err(unavailable)?;
                if closed.signature.symbol != selection.requirement
                    || closed.declaring_trait != selection.subject
                {
                    return Err(unavailable(
                        "the native binding's exact closed requirement application",
                    ));
                }
                let identity = source
                    .product_operand_application_identity(handle)
                    .map_err(unavailable)?
                    .ok_or_else(|| {
                        unavailable("the native binding's complete application identity")
                    })?;
                if identity != canonical {
                    return Err(unavailable(
                        "the native binding's unchanged closed callable identity",
                    ));
                }
                Ok::<_, Vec<Diagnostic>>(identity)
            })
            .transpose()?;
        let boundary_callable = boundary
            .declaration
            .as_ref()
            .and_then(|declaration| module.identities.text(declaration.source.callable));
        if boundary_callable != Some(canonical.as_str()) {
            return Err(unavailable(
                "the native binding's complete callable identity",
            ));
        }
        let abi = backend_plan::DirectBoundaryLeaf::declared_calling_abi(module, boundary.id)
            .map_err(|reason| unavailable(&reason))?;
        let declared_identity = module
            .identities
            .text(abi.requirement_identity)
            .ok_or_else(|| unavailable("the native binding's original method identity"))?;
        let template = source
            .closed_requirement_application(signature.symbol)
            .map_or(signature.symbol, |application| application.template);
        let (_, declared_signature) = source
            .trait_machine_signature(template)
            .filter(|(owner, _)| owner.symbol == service.symbol)
            .ok_or_else(|| unavailable("the native binding's original method declaration"))?;
        if source.trait_requirement_callable_identity_text(service, declared_signature)
            != declared_identity
        {
            return Err(unavailable(
                "the native binding's exact original schema method",
            ));
        }
        let producer = PackagePolicyEvaluatedBindingProducer {
            declaration: PackageReviewNominalIdentity {
                owner: PackageReviewNominalOwner::Package(package),
                path: selection.producer.declaration_path.clone(),
            },
            package: Some(package),
            callable_identity: selection.producer.callable_identity.clone(),
        };
        let schema_index = schemas
            .binary_search_by_key(&symbol_key(declaration.slot), |(slot, _, _)| {
                symbol_key(*slot)
            })
            .map_err(|_| unavailable("the exact native binding service schema"))?;
        let (_, schema, methods) = &schemas[schema_index];
        let matches = |index: usize| {
            schema.identity_text(schema.declaration.methods[index].requirement_identity)
        };
        let first = methods.partition_point(|&index| matches(index) < declared_identity);
        let last = methods.partition_point(|&index| matches(index) <= declared_identity);
        let [index] = &methods[first..last] else {
            return Err(unavailable("one native binding method in the exact schema"));
        };
        let method = &schema.declaration.methods[*index];
        let (method, callable) = input.checked_method(
            signature,
            boundary,
            method,
            schema,
            input.nominal(template)?,
            input.nominal(service.symbol)?,
            declarations,
        )?;
        declared.push((rows.len(), method, callable));
        rows.push(NativeDeclaration {
            policy_index: 0,
            boundary: boundary.id,
            callable: canonical,
            abi: abi
                .identity(&module.identities)
                .map_err(|reason| unavailable(&reason))?,
            materialization: supply.materialization(),
            application,
            producer,
        });
    }
    // Source coverage searches declarations by requirement identity. The
    // retained planner rows are in the original Build's order.
    declared.sort_by(|(left, left_method, _), (right, right_method, _)| {
        let left = &rows[*left];
        let right = &rows[*right];
        (
            &left_method.requirement,
            &left.application,
            &left.producer.callable_identity,
        )
            .cmp(&(
                &right_method.requirement,
                &right.application,
                &right.producer.callable_identity,
            ))
    });
    let declared = declared
        .into_iter()
        .enumerate()
        .map(|(policy_index, (source_index, method, callable))| {
            rows[source_index].policy_index = policy_index;
            (method, callable)
        })
        .collect();
    Ok(PreparedNativeBindings {
        target,
        rows,
        declared,
    })
}

impl PreparedNativeBindings {
    pub(super) fn declared(&self) -> &[(PackagePolicyServiceMethod, PackagePolicyCallable)] {
        &self.declared
    }

    pub(super) fn finish(
        &self,
        checked: &products::CheckedCompilation,
        target: TargetProfile,
    ) -> Result<Vec<(PackagePolicyNativeBinding, PackagePolicyCallable)>, Vec<Diagnostic>> {
        if target != self.target || checked.execution().target_profile() != Some(target) {
            return Err(unavailable("the complete selected native supply custody"));
        }
        let realization = checked.execution().realization();
        let selected = realization.evaluated_native_bindings();
        let reviewed = match checked.execution().package_review() {
            Some(review) => review.bindings.as_ref().map_err(Clone::clone)?,
            None if self.rows.is_empty() && selected.is_empty() => return Ok(Vec::new()),
            None => return Err(unavailable("the package's target review plan")),
        };
        if reviewed.rows.len() != self.rows.len() {
            return Err(unavailable("the complete selected native supply custody"));
        }
        if self.rows.is_empty() && selected.is_empty() {
            return Ok(Vec::new());
        }
        let mut receipts = Vec::with_capacity(reviewed.rows.len());
        for (source, row) in self.rows.iter().zip(&reviewed.rows) {
            if source.boundary != row.boundary
                || source.materialization != row.materialization
                || row.binding.target() != target
            {
                return Err(unavailable("the complete selected native supply custody"));
            }
            receipts.push((
                row.boundary,
                row.binding.receipt().identity_digest(),
                source.callable.as_str(),
            ));
        }
        receipts.sort_unstable();
        if receipts.windows(2).any(|pair| pair[0].0 == pair[1].0) {
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
            if binding.target() != target
                || !receipts.iter().any(|(_, receipt, callable)| {
                    *receipt == binding.receipt().identity_digest() && *callable == requirement
                })
            {
                return Err(unavailable("the complete selected native supply custody"));
            }
        }
        let original = checked
            .published_artifact()
            .ok_or_else(|| unavailable("the checked program for native calling policy"))?;
        let effective = checked
            .execution()
            .effective_artifact(original)
            .map_err(|reason| unavailable(&reason))?;
        realization
            .require_program(effective.view().identity())
            .map_err(|reason| unavailable(&reason))?;
        let applications = reviewed.calling.as_ref().map_err(Clone::clone)?;
        if applications.len() != self.rows.len() {
            return Err(unavailable(
                "one validated calling application per native binding",
            ));
        }
        let mut policies = Vec::with_capacity(self.rows.len());
        for ((source, row), application) in self.rows.iter().zip(&reviewed.rows).zip(applications) {
            let requirement = row
                .binding
                .receipt()
                .build_origin()
                .and_then(|origin| reviewed.identities.text(origin.requirement_identity));
            if requirement != Some(source.callable.as_str()) || source.abi != application.abi {
                return Err(unavailable(
                    "the native binding's exact calling requirement",
                ));
            }
            let (method, callable) = &self.declared[source.policy_index];
            let mut method = method.clone();
            method
                .calling
                .push(calling::project(application, &method, target)?);
            policies.push((
                PackagePolicyNativeBinding {
                    application: source.application.clone(),
                    method,
                    binding: binding_policy(&row.binding, source.producer.clone()),
                },
                callable.clone(),
            ));
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
