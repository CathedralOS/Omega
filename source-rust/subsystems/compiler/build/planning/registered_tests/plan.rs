//! Prepare the registered products and their destinations before invoking the first guest.
//! Execution consumes the same checked product used to report its bindings.
use super::{
    Artifact, CompileRequest, CompletedProduct, Diagnostic, EngineError,
    InstallationEnvironmentAssumption, ProductStage, Program, RegisteredTests, Run, TargetProfile,
    TestExecution, run_test,
};
use provider_admission::IsolatedExecutionGrant;
use source_compiler::registered_tests::TestRoot;

#[path = "plan/bindings.rs"]
mod bindings;

pub(super) struct PlannedTest {
    root: TestRoot,
    target: TargetProfile,
    request: TestExecution,
    engines: Engines,
    product: Result<PreparedProduct, Vec<Diagnostic>>,
}

struct PreparedProduct {
    report: CompletedProduct,
    bindings: String,
}

pub(super) struct Engines {
    pub(super) psi: Destination,
    pub(super) native: Destination,
}

#[derive(Clone, Copy)]
pub(super) enum Destination {
    Scheduled,
    Unavailable(&'static str),
}

impl Destination {
    fn describe(self, request: TestExecution) -> String {
        let requirement = if request == TestExecution::Compare {
            "required"
        } else {
            "if available"
        };
        match self {
            Self::Scheduled => format!("scheduled ({requirement})"),
            Self::Unavailable(reason) => format!("unavailable ({requirement}, {reason})"),
        }
    }

    pub(super) fn run<T>(self, invoke: impl FnOnce() -> Run<T>) -> Run<T> {
        match self {
            Self::Scheduled => invoke(),
            Self::Unavailable(reason) => Err(EngineError::Unavailable(reason.into())),
        }
    }
}

impl PlannedTest {
    pub(super) fn prepare(
        stage: &ProductStage<'_>,
        suite: &RegisteredTests<'_>,
        root: TestRoot,
        target: TargetProfile,
        assumptions: &[InstallationEnvironmentAssumption],
        request: TestExecution,
    ) -> Self {
        let private_grant = root.inputs.private_filesystem.then(|| {
            IsolatedExecutionGrant::private_linux_filesystem(target)
        }).flatten();
        let psi = if root.inputs.private_filesystem && private_grant.is_none() {
            Destination::Unavailable("the target has no private filesystem adapter")
        } else {
            Destination::Scheduled
        };
        let native = if TargetProfile::host_if_supported() == Some(target) {
            psi
        } else {
            Destination::Unavailable("its target does not execute on this host")
        };
        let prepare = || {
            let mut native_target = stage.native_target(target, assumptions.to_vec())?;
            native_target.isolated_execution_grant = Some(
                private_grant.unwrap_or_else(IsolatedExecutionGrant::captured_process),
            );
            let program = Program::CheckedSource {
                publication: suite.compile(&root, target.target_name())?,
                suppliers: stage.product,
                runtime_consent: stage.runtime_consent(target)?,
                target: native_target,
            };
            let request = match native {
                Destination::Scheduled => CompileRequest::Native {
                    program,
                    entry_memory: &[],
                },
                Destination::Unavailable(_) => CompileRequest::Runtime {
                    program,
                    entry_memory: &[],
                },
            };
            let result = compiler::compile(request, Default::default())
                .map_err(compiler::stage_validators::NativeValidationError::into_diagnostics)?;
            let report = match (native, result.artifact) {
                (Destination::Scheduled, Artifact::Native(product))
                | (Destination::Unavailable(_), Artifact::Runtime(product)) => product,
                _ => {
                    return Err(vec![Diagnostic::error(
                        "compiler returned a product that does not match the registered test plan",
                    )]);
                }
            };
            let bindings =
                bindings::describe(&report).map_err(|reason| vec![Diagnostic::error(reason)])?;
            Ok(PreparedProduct { report, bindings })
        };
        let product = prepare();
        Self {
            root,
            target,
            request,
            engines: Engines { psi, native },
            product,
        }
    }

    pub(super) fn describe(&self) -> String {
        let label = format!(
            "test plan for `{}` in group `{}` (target `{}`)",
            self.root.name,
            self.root.group,
            self.target.target_name()
        );
        match &self.product {
            Err(errors) => format!(
                "{label}: not scheduled; cannot be produced: {}",
                diagnostics::join_messages(errors)
            ),
            Ok(product) => format!(
                "{label}: Psi interpreter {}; native {}; {}",
                self.engines.psi.describe(self.request),
                self.engines.native.describe(self.request),
                product.bindings
            ),
        }
    }

    pub(super) fn execute(self) -> Vec<Diagnostic> {
        let name = format!("test `{}` in group `{}`", self.root.name, self.root.group);
        let target = self.target.target_name();
        let product = match self.product {
            Ok(product) => product,
            Err(mut rows) => {
                for row in &mut rows {
                    if row.is_error() {
                        row.message = format!(
                            "{name} cannot be produced: {} (target `{target}`)",
                            row.message
                        );
                    }
                }
                return rows;
            }
        };
        vec![
            match run_test(product.report, &self.engines, &self.root.inputs, self.target, self.request) {
                Ok(status) => Diagnostic::note(format!(
                    "{name} (target `{}`): {status}",
                    self.target.target_name()
                )),
                Err(reason) => Diagnostic::error(format!(
                    "{name} {reason} (target `{}`)",
                    self.target.target_name()
                )),
            },
        ]
    }
}
