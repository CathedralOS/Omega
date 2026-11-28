//! Prepare the registered products and their destinations before invoking the first guest.
//! Execution consumes the same checked product used to report its bindings.
use super::{
    Artifact, CompileRequest, CompletedProduct, Diagnostic, EngineError,
    InstallationEnvironmentAssumption, ProductStage, Program, RegisteredTests, Run, TargetProfile,
    TestExecution, run_test,
};
use provider_admission::IsolatedExecutionGrant;
use source_compiler::registered_tests::{TestEnginePolicy, TestRoot};

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
    pub(super) native: Option<Destination>,
}

#[derive(Clone, Copy)]
pub(super) struct Destination {
    policy: TestEnginePolicy,
    unavailable: Option<&'static str>,
}

impl Destination {
    fn scheduled(self) -> bool {
        self.policy != TestEnginePolicy::Disabled && self.unavailable.is_none()
    }

    fn describe(self, request: TestExecution) -> String {
        if self.policy == TestEnginePolicy::Disabled {
            return "explicitly disabled".into();
        }
        let requirement =
            if self.policy == TestEnginePolicy::Required || request == TestExecution::Compare {
                "required"
            } else {
                "if available"
            };
        match self.unavailable {
            None => format!("scheduled ({requirement})"),
            Some(reason) => format!("unavailable ({requirement}, {reason})"),
        }
    }

    pub(super) fn run<T>(self, invoke: impl FnOnce() -> Run<T>) -> Run<T> {
        if self.policy == TestEnginePolicy::Disabled {
            return Err(EngineError::Unavailable(
                "explicitly disabled by the Build group".into(),
            ));
        }
        let outcome = match self.unavailable {
            None => invoke(),
            Some(reason) => Err(EngineError::Unavailable(reason.into())),
        };
        match (self.policy, outcome) {
            (TestEnginePolicy::Required, Err(EngineError::Unavailable(reason))) => {
                Err(EngineError::Incomplete(format!(
                    "required by the Build group but unavailable: {reason}"
                )))
            }
            (_, outcome) => outcome,
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
        let linux = matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64);
        let missing_adapter = (root.inputs.private_filesystem && !linux)
            .then_some("the target has no private filesystem adapter");
        let psi = Destination {
            policy: root.engines.psi,
            unavailable: missing_adapter,
        };
        let native = match (request, root.engines.native) {
            (TestExecution::Interpret, TestEnginePolicy::Required) => Some(Destination {
                policy: TestEnginePolicy::Required,
                unavailable: Some("use `omega build` for required native test execution"),
            }),
            (TestExecution::Interpret, _) => None,
            (_, policy) => Some(Destination {
                policy,
                unavailable: if TargetProfile::host_if_supported() == Some(target) {
                    missing_adapter
                } else {
                    Some("its target does not execute on this host")
                },
            }),
        };
        let compile_native = native.is_some_and(Destination::scheduled);
        let prepare = || {
            if !root.inputs.private_filesystem
                && (!root.inputs.files.is_empty() || !root.expected.files.is_empty())
            {
                return Err(vec![Diagnostic::error(
                    "declared test files need a filesystem backing for this invocation",
                )]);
            }
            let mut native_target = stage.native_target(target, assumptions.to_vec())?;
            native_target.isolated_execution_grant =
                Some(IsolatedExecutionGrant::captured_process(
                    target,
                    super::standard_input(&root.inputs, target).is_some(),
                    root.inputs.private_filesystem && linux,
                ));
            let program = Program::CheckedSource {
                publication: suite.compile(&root, target.target_name())?,
                suppliers: stage.product,
                runtime_consent: stage.runtime_consent(target)?,
                target: native_target,
            };
            let request = match compile_native {
                true => CompileRequest::Native {
                    program,
                    entry_memory: &[],
                },
                false => CompileRequest::Runtime {
                    program,
                    entry_memory: &[],
                },
            };
            let result = compiler::compile(request, Default::default())
                .map_err(compiler::stage_validators::NativeValidationError::into_diagnostics)?;
            let report = match (compile_native, result.artifact) {
                (true, Artifact::Native(product)) | (false, Artifact::Runtime(product)) => product,
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
                "{label}: Psi interpreter {}{}; {}",
                self.engines.psi.describe(self.request),
                self.engines
                    .native
                    .map_or_else(String::new, |destination| format!(
                        "; native {}",
                        destination.describe(self.request)
                    )),
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
        vec![match run_test(
            product.report,
            &self.engines,
            &self.root.inputs,
            &self.root.expected,
            self.target,
            self.request,
        ) {
            Ok(status) => Diagnostic::note(format!(
                "{name} (target `{}`): {status}",
                self.target.target_name()
            )),
            Err(reason) => Diagnostic::error(format!(
                "{name} {reason} (target `{}`)",
                self.target.target_name()
            )),
        }]
    }
}
