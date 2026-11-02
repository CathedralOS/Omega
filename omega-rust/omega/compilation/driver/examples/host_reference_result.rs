//! Public SDK completion of captured-input reference results and refusal controls.
#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use semantic_vocabulary::{
    BoundaryMachineId, CanonicalStructuralPathSegment as Path, IntegerValue,
};
use std::cell::RefCell;
use std::rc::Rc;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryReferenceSelection, TerminalBoundaryStorageBuffer,
    TerminalCallStorageRequirements, TerminalEffect, TerminalEffectHandler,
    TerminalEffectRejection, TerminalEffectResult, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalImportAdapter, TerminalImportSignature, TerminalInstantiation,
    TerminalProgram, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::StructuralAccess;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Good,
    NoOptIn,
    NoCapture,
    Stale,
    WrongType,
    WrongPath,
    Shared,
    Opaque,
}
#[derive(Default)]
struct Observation {
    calls: usize,
    first: Option<TerminalBoundaryReferenceSelection>,
}
struct Host {
    signature: TerminalImportSignature,
    mode: Mode,
    value_path: Vec<Path>,
    observed: Rc<RefCell<Observation>>,
}
impl TerminalImportAdapter for Host {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for Host {
    fn accepts_reference_results(&self, _: BoundaryMachineId) -> bool {
        self.mode != Mode::NoOptIn
    }
    fn storage_requirements(
        &self,
        _: BoundaryMachineId,
        _: &[TerminalScalarValue],
    ) -> TerminalCallStorageRequirements<'_> {
        TerminalCallStorageRequirements {
            arguments: if self.mode == Mode::NoCapture {
                &[]
            } else {
                &[0]
            },
            original: false,
        }
    }
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        unreachable!("buffer interface")
    }
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        _: &mut [TerminalBoundaryByteBuffer],
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let mut observed = self.observed.borrow_mut();
        observed.calls += 1;
        let TerminalBoundaryStorageBuffer::Structural(buffer) = &mut buffers[0] else {
            panic!("Cell storage")
        };
        let mut values = buffer.values().to_vec();
        let first_field = self.value_path.clone();
        // Use the admitted nominal field identity and preserve both neighbors.
        let value = values
            .iter_mut()
            .find(|value| value.path == first_field)
            .unwrap();
        let TerminalStorageContents::Scalar {
            value: TerminalScalarValue::Integer { value, .. },
            ..
        } = &mut value.contents
        else {
            panic!("u64")
        };
        *value = IntegerValue::Unsigned(0x2525252525252525);
        buffer.replace(values)?;
        let access = if self.mode == Mode::Shared {
            StructuralAccess::SharedBorrow
        } else {
            StructuralAccess::MutableBorrow
        };
        let path = match self.mode {
            Mode::WrongType => first_field,
            Mode::WrongPath => vec![Path::FixedIndex(999)],
            _ => vec![],
        };
        let current = buffers[0].select_reference(&path, access)?;
        if self.mode == Mode::Opaque {
            let TerminalEffect::BoundaryCall {
                result: terminal_psi::BoundaryMachineResult::Structural(result),
                ..
            } = effect
            else {
                unreachable!()
            };
            return Ok(TerminalEffectResult::Structural(
                terminal_interpreter::TerminalStructuralValue::minted(
                    999,
                    result.structural_type,
                    vec![],
                ),
            ));
        }
        let selected = if self.mode == Mode::Stale && observed.calls > 1 {
            observed.first.clone().unwrap()
        } else {
            current.clone()
        };
        observed.first.get_or_insert(current);
        Ok(TerminalEffectResult::Reference(selected))
    }
}

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--build-profile")) {
        println!("{}", compiler::compiler_build_profile());
        return;
    }
    let root = fixture_package_inputs::repo_root()
        .join("samples/cli/facts/foreign_reference_result/local/main.omg");
    let prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(&root, None)
        .expect("ordinary package acceptance");
    let mut request = compiler::CheckedCompileRequest::new(&prepared.root_path, None);
    request.package_inputs = prepared.package_inputs;
    let checked = omega_build::check_source(&mut compiler::CompilationSession::default(), request)
        .result
        .unwrap_or_else(|error| panic!("{:?}", error.into_diagnostics()));
    let produced = psi_compiler::TerminalProductionRequest::new(
        checked.execution().provider_source.program(),
        psi_compiler::TerminalMachineSelection::Name("main"),
        checked.sources().exact_toolchain_sources(),
    )
    .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
    .unwrap();
    let artifact = produced.artifact().unwrap();
    for decoded in [false, true] {
        let program = if decoded {
            TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap()
        } else {
            TerminalProgram::borrow(artifact.view())
        };
        for mode in [
            Mode::Good,
            Mode::NoOptIn,
            Mode::NoCapture,
            Mode::Stale,
            Mode::WrongType,
            Mode::WrongPath,
            Mode::Shared,
            Mode::Opaque,
        ] {
            let observed = Rc::new(RefCell::new(Observation::default()));
            let mut instance = program
                .instantiate(TerminalInstantiation::default())
                .unwrap();
            let imports = instance.unbound_imports().collect::<Vec<_>>();
            assert_eq!(imports.len(), 1);
            let declaration = artifact
                .semantic_module()
                .boundary_machine(imports[0])
                .unwrap();
            assert_eq!(declaration.result_reference_regions.len(), 1);
            let cell = artifact
                .semantic_module()
                .structural_types
                .iter()
                .find(|row| row.id == declaration.structural_parameters[0].structural_type)
                .unwrap();
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &cell.shape else {
                panic!("Cell")
            };
            let value_path = vec![Path::Field(
                fields
                    .iter()
                    .find(|field| {
                        artifact.semantic_module().identities.text(field.identity) == Some("value")
                    })
                    .unwrap()
                    .id,
            )];
            instance
                .bind_import(
                    imports[0],
                    Host {
                        signature: declaration.into(),
                        mode,
                        value_path,
                        observed: Rc::clone(&observed),
                    },
                )
                .unwrap();
            let result = instance.invoke(&mut terminal_fuel::TerminalFuelMeter::unbounded());
            if mode == Mode::Good {
                assert_eq!(
                    result.unwrap(),
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                );
                assert_eq!(
                    observed.borrow().calls,
                    2,
                    "each authored call executes exactly once"
                );
            } else {
                assert!(result.is_err(), "{mode:?} must reject, got {result:?}");
                assert_eq!(
                    observed.borrow().calls,
                    match mode {
                        Mode::NoOptIn | Mode::NoCapture => 0,
                        Mode::Stale => 2,
                        _ => 1,
                    }
                );
                assert!(instance.result().is_none());
            }
            println!(
                "host-reference decoded={decoded} mode={mode:?} calls={} passed",
                observed.borrow().calls
            );
        }
    }
}
