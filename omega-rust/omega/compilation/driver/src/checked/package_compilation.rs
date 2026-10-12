//! Execute a dependency-ordered package request through the compiler session.
//!
//! Package management supplies immutable source custody, accepted bindings and
//! occurrence-specific consent. This owner joins generated outputs, retains
//! dependency continuations, and completes only demanded independent products
//! before checking the root. Package callers do not assemble compiler stages.

use super::CompilationSession;
use build_declarations::DependencyPurpose;
use compilation_products::{
    CheckedCompilation, CheckedCompileRequest, CompileFailure, PreparedPackageBuild,
};
use diagnostics::Diagnostic;
use package_compilation::{IndependentComponentDescription, PackageGeneratedSourceBundle};
use semantic_vocabulary::PackageKeyIdentity;

/// One dependency-ordered source closure. The last request is its Product root;
/// earlier requests are dependency occurrences, including separate Build roles.
pub struct PackageCompilationRequest {
    pub occurrences: Vec<CheckedCompileRequest>,
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
    /// Execute one admitted source closure. Generated dependencies and component
    /// descriptions travel forward; neither the root nor a component reruns a
    /// dependency Build when it demands the retained Product continuation.
    pub fn check_package(
        &mut self,
        mut request: PackageCompilationRequest,
    ) -> Result<CheckedCompilation, PackageCompileFailure> {
        validate_order(&request)?;
        let root = request
            .occurrences
            .pop()
            .ok_or_else(|| PackageCompileFailure {
                occurrence: 0,
                failure: vec![Diagnostic::error("package compilation has no Product root")].into(),
            })?;
        let root_position = request.occurrences.len();
        let mut products = Vec::with_capacity(root_position);
        for (position, occurrence) in request.occurrences.into_iter().enumerate() {
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
            .and_then(|prepared| self.check_build(prepared).result)
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
fn validate_order(request: &PackageCompilationRequest) -> Result<(), PackageCompileFailure> {
    if request.occurrences.is_empty() {
        return Err(PackageCompileFailure {
            occurrence: 0,
            failure: vec![Diagnostic::error("package compilation has no Product root")].into(),
        });
    }
    for (position, occurrence) in request.occurrences.iter().enumerate() {
        let refuse = |message| PackageCompileFailure {
            occurrence: position,
            failure: vec![Diagnostic::error(message)].into(),
        };
        let inputs = occurrence
            .package_inputs
            .as_ref()
            .ok_or_else(|| refuse("package compilation requires source custody"))?;
        let previous = &request.occurrences[..position];
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
        if position + 1 == request.occurrences.len() && !inputs.compilation_purpose().is_product() {
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
