//! Execute a dependency-ordered package request through the compiler session.
//!
//! Package management supplies immutable source custody, accepted bindings and
//! occurrence-specific consent. This owner joins generated outputs, retains
//! dependency continuations, and completes only demanded independent products
//! before checking the root. Package callers do not assemble compiler stages.

use super::CompilationSession;
use build_declarations::DependencyPurpose;
use compilation_products::{CheckedCompileRequest, CompileFailure, PreparedPackageBuild};
use diagnostics::Diagnostic;
use package_compilation::{IndependentComponentDescription, PackageGeneratedSourceBundle};
use semantic_vocabulary::PackageKeyIdentity;

/// One dependency-ordered source closure. The last request is its Product root;
/// earlier requests are dependency occurrences, including separate Build roles.
pub struct PackageCompilationRequest {
    pub occurrences: Vec<CheckedCompileRequest>,
    pub product: crate::CompilationProduct,
    /// Present only for a locked root Product. No lock supplies no permissions;
    /// a missing occurrence inside an existing lock must reject at admission.
    pub terminal_permissions:
        Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

/// The occurrence responsible for a rejected compiler request. Package naming
/// and presentation remain with the package owner.
pub struct PackageCompileFailure {
    pub occurrence: usize,
    pub failure: CompileFailure,
}

struct OccurrenceProduct {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    generated: PackageGeneratedSourceBundle,
    prepared: Option<PreparedPackageBuild>,
    description: Option<IndependentComponentDescription>,
}

impl CompilationSession {
    /// Compile the complete admitted roster through its requested products.
    /// Source checking, consent comparison and production belong to this call;
    /// package operations provide policy inputs, not a mid-pipeline callback.
    pub fn compile_packages(
        &mut self,
        requests: Vec<PackageCompilationRequest>,
    ) -> Vec<Result<crate::CompilationOutput, PackageCompileFailure>> {
        let mut outcomes: Vec<_> = (0..requests.len()).map(|_| None).collect();
        let mut selections = Vec::new();
        let mut builds = Vec::new();
        for (position, mut request) in requests.into_iter().enumerate() {
            let root = request.occurrences.len().saturating_sub(1);
            let path = request
                .occurrences
                .last()
                .map(|root| root.root_path.clone())
                .unwrap_or_default();
            for occurrence in &mut request.occurrences {
                occurrence.optimization_rollback = request.product.optimization_rollback();
            }
            match self.prepare_package(request.occurrences) {
                Ok(build) => {
                    selections.push((
                        position,
                        root,
                        path,
                        request.product,
                        request.terminal_permissions,
                    ));
                    builds.push(build);
                }
                Err(error) => outcomes[position] = Some(Err(error)),
            }
        }

        // All roots enter source completion together. Consent consumes the
        // resulting declarations, before any requested native product exists.
        let mut positions = Vec::new();
        let mut products = Vec::new();
        for (checked, (position, root, path, product, permissions)) in
            self.check_build_batch(builds).into_iter().zip(selections)
        {
            let checked = checked.result.and_then(|checked| {
                if let Some(permissions) = permissions {
                    permissions
                        .validate(&checked)
                        .map_err(CompileFailure::from)?;
                }
                Ok(checked)
            });
            match checked {
                Ok(checked) => {
                    positions.push((position, root));
                    products.push((checked, path, product));
                }
                Err(failure) => {
                    outcomes[position] = Some(Err(PackageCompileFailure {
                        occurrence: root,
                        failure,
                    }))
                }
            }
        }
        for ((position, root), product) in positions
            .into_iter()
            .zip(crate::compiler::finish_compilation(products))
        {
            outcomes[position] = Some(product.map_err(|error| PackageCompileFailure {
                occurrence: root,
                failure: error.into_diagnostics().into(),
            }));
        }
        outcomes
            .into_iter()
            .map(|outcome| {
                outcome.unwrap_or_else(|| {
                    Err(PackageCompileFailure {
                        occurrence: 0,
                        failure: vec![Diagnostic::error(
                            "package compilation lost a requested product",
                        )]
                        .into(),
                    })
                })
            })
            .collect()
    }

    fn prepare_package(
        &mut self,
        mut request: Vec<CheckedCompileRequest>,
    ) -> Result<PreparedPackageBuild, PackageCompileFailure> {
        validate_order(&request)?;
        let root = request.pop().ok_or_else(|| PackageCompileFailure {
            occurrence: 0,
            failure: vec![Diagnostic::error("package compilation has no Product root")].into(),
        })?;
        let root_position = request.len();
        let mut products = Vec::with_capacity(root_position);
        for (position, occurrence) in request.into_iter().enumerate() {
            let prepared =
                self.prepare_occurrence(occurrence, &mut products)
                    .map_err(|failure| PackageCompileFailure {
                        occurrence: position,
                        failure,
                    })?;
            products.push(OccurrenceProduct {
                package: prepared.package_inputs().root(),
                purpose: prepared.package_inputs().compilation_purpose(),
                generated: prepared.generated_source_bundle().clone(),
                prepared: Some(prepared),
                description: None,
            });
        }
        self.prepare_occurrence(root, &mut products)
            .map_err(|failure| PackageCompileFailure {
                occurrence: root_position,
                failure,
            })
    }

    /// Join earlier outputs to this occurrence, execute its Build, then satisfy
    /// its independent-component selections from retained dependency products.
    fn prepare_occurrence(
        &mut self,
        mut occurrence: CheckedCompileRequest,
        products: &mut [OccurrenceProduct],
    ) -> Result<PreparedPackageBuild, CompileFailure> {
        let inputs = occurrence.package_inputs.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "package compilation requires source custody",
            )]
        })?;
        let package = inputs.root();
        let purpose = inputs.compilation_purpose();
        let required = inputs.required_dependency_source_instances();
        let mut generated = Vec::with_capacity(required.len());
        for (dependency, dependency_purpose) in required {
            let product = products
                .iter()
                .find(|product| {
                    product.package == dependency && product.purpose == dependency_purpose
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "dependency Build output is missing for its required purpose",
                    )]
                })?;
            generated.push(product.generated.clone());
        }
        occurrence.package_inputs = Some(
            inputs
                .with_complete_dependency_generated_sources(generated)
                .map_err(input_diagnostics)?,
        );
        let prepared = self.prepare_build(occurrence)?;
        let mut descriptions = Vec::new();
        for selection in prepared.build().independent_components().selections {
            let Some(component) = selection.component else {
                continue;
            };
            if component == package
                || descriptions
                    .iter()
                    .any(|description: &IndependentComponentDescription| {
                        description.package() == component
                    })
            {
                continue;
            }
            let product = products
                .iter_mut()
                .find(|product| product.package == component && product.purpose == purpose)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "independent component has no prepared Build for this purpose",
                    )]
                })?;
            if product.description.is_none() {
                let continuation = product.prepared.take().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "independent component continuation was already consumed",
                    )]
                })?;
                let checked = self.check_build(continuation).result?;
                product.description = Some(crate::compiler::package::publish_checked_component(
                    checked,
                )?);
            }
            descriptions.extend(product.description.iter().cloned());
        }
        prepared
            .with_independent_component_descriptions(descriptions)
            .map_err(|errors| CompileFailure::from(input_diagnostics(errors)))
    }
}

/// Refuse malformed plans before any Build effect. Order is supplied by the
/// package resolver; checking it is request admission, not graph reconstruction.
fn validate_order(request: &[CheckedCompileRequest]) -> Result<(), PackageCompileFailure> {
    if request.is_empty() {
        return Err(PackageCompileFailure {
            occurrence: 0,
            failure: vec![Diagnostic::error("package compilation has no Product root")].into(),
        });
    }
    for (position, occurrence) in request.iter().enumerate() {
        let refuse = |message| PackageCompileFailure {
            occurrence: position,
            failure: vec![Diagnostic::error(message)].into(),
        };
        let inputs = occurrence
            .package_inputs
            .as_ref()
            .ok_or_else(|| refuse("package compilation requires source custody"))?;
        let previous = &request[..position];
        if previous
            .iter()
            .filter_map(|request| request.package_inputs.as_ref())
            .any(|prior| {
                prior.root() == inputs.root()
                    && prior.compilation_purpose() == inputs.compilation_purpose()
            })
        {
            return Err(refuse("package compilation repeats a package occurrence"));
        }
        for (package, purpose) in inputs.required_dependency_source_instances() {
            if !previous
                .iter()
                .filter_map(|request| request.package_inputs.as_ref())
                .any(|prior| prior.root() == package && prior.compilation_purpose() == purpose)
            {
                return Err(refuse(
                    "package compilation requires dependency-first occurrence order",
                ));
            }
        }
        if position + 1 == request.len() && !inputs.compilation_purpose().is_product() {
            return Err(refuse("package compilation must end at its Product root"));
        }
    }
    Ok(())
}

fn input_diagnostics(
    errors: Vec<package_compilation::PackageCompilationInputError>,
) -> Vec<Diagnostic> {
    errors
        .into_iter()
        .map(|error| Diagnostic::error(error.to_string()))
        .collect()
}
