//! Source checking decides whether a program is valid and publishes its checked
//! portable module with evidence seals. Every product consumes that result;
//! refusal or failed publication never retries a retired checker or producer.
//!
//! The executable closure and each generic body's admitted alternatives must
//! all pass before the closure is published. Explicit entry changes use this
//! same path and receive new evidence for the selected module.

use diagnostics::Diagnostic;
use lowered_discharge::CompilationJudgment;

use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

/// What the route judges: the declarations the request claims, which are
/// the same `structure_elaboration::elaborate` roots.
pub(super) struct RouteScope<'a> {
    pub(super) entries: &'a [SymbolHandle],
    pub(super) subject: Option<PackageKeyIdentity>,
    pub(super) selected: &'a [SymbolHandle],
    pub(super) bindings: &'a [typed_trees::callable_signature_shapes::BoundCallableShape],
}

impl RouteScope<'_> {
    /// The route's judgment of `typed`: the closure of the compilation's
    /// roots, and the check form of each generic machine the subject
    /// declares. generics.md, "Static type equality": "An unspecialized body
    /// checks every admitted alternative; one consumer cannot authorize an
    /// otherwise invalid generic body." So a generic body is judged as
    /// written whether or not the program applies it, and the machines stage
    /// 04 generated for the program's own applications are judged as closed
    /// machines of the closure. The route accepts only when it accepts every
    /// one of them.
    ///
    /// The source verdict and executable publication are separate results.
    /// A valid program whose checked module cannot be published keeps that
    /// exact failure; product emission must never substitute another module.
    /// Only the first verdict owns executable code. The remaining verdicts
    /// check generic bodies and must all pass before publication is attempted.
    pub(super) fn judge(
        &self,
        typed: &std::sync::Arc<typed_trees::FrozenTypedTrees>,
    ) -> (
        CompilationJudgment,
        Result<std::sync::Arc<compilation_products::CheckedApplicationProgram>, Vec<Diagnostic>>,
    ) {
        let (judgment, publication) = psi_compiler::CheckedInvocationSource::judge(
            std::sync::Arc::clone(typed),
            structure_elaboration::Compilation {
                entries: self.entries,
                subject: self.subject,
                selected: self.selected,
                bindings: self.bindings,
            },
        );
        let published = publication.and_then(|publication| {
            compilation_products::CheckedApplicationProgram::new(std::sync::Arc::new(publication))
                .map(std::sync::Arc::new)
        });
        (judgment, published)
    }
}

/// Check explicitly selected roots without rewriting an accepted artifact's
/// entry or evidence seals. Keep the original source obligations alongside the
/// new entries, and check every newly installed entry binding.
pub(crate) fn publish_selected_roots(
    source: &std::sync::Arc<compilation_products::CheckedApplicationProgram>,
    entries: &[SymbolHandle],
    bindings: &[typed_trees::callable_signature_shapes::BoundCallableShape],
) -> Result<std::sync::Arc<compilation_products::CheckedApplicationProgram>, Vec<Diagnostic>> {
    let declarations = source.correspondence();
    let module = source.module();
    let selected = module
        .machines
        .iter()
        .map(|machine| {
            declarations
                .machines
                .get(machine.id)
                .copied()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "root selection lost a checked machine's source declaration",
                    )]
                })
        })
        .chain(module.boundary_machines.iter().map(|boundary| {
            declarations
                .boundaries
                .get(boundary.id)
                .copied()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "root selection lost a checked boundary's source declaration",
                    )]
                })
        }))
        .collect::<Result<Vec<_>, _>>()?;
    let (verdict, published) = RouteScope {
        entries,
        subject: None,
        selected: &selected,
        bindings,
    }
    .judge(source.typed());
    verdict.verdict()?;
    published
}
