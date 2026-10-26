//! Source-produced mutable boundary calls retain receiver identity and stored
//! view lifetimes across host effects and fuel pauses. This provision supplies
//! an observation sink, not a replacement Build evaluator.
#[path = "terminal_provision.rs"]
mod terminal_provision;

use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, IntegerValue};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEntryStorageError, TerminalEntryStorageValue, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalProgram, TerminalScalarValue, TerminalStructuralInputs,
    TerminalStructuralValue,
};

pub(super) fn observe(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    terminal_provision::result(|| exercise(artifact), "mutable boundary provision failed")
}

pub(super) fn field(artifact: &CanonicalTerminalArtifact, name: &str) -> Path {
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program.entry().unwrap();
    let root = entry.structural_parameters[0].structural_type;
    let declaration = artifact
        .semantic_module()
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        panic!("record entry");
    };
    let identities = &artifact.semantic_module().identities;
    Path::Field(
        fields
            .iter()
            .find(|field| identities.text(field.identity) == Some(name))
            .unwrap()
            .id,
    )
}

struct ObserveBuild<'host> {
    signature: terminal_interpreter::TerminalImportSignature,
    original: TerminalStructuralValue,
    observations: &'host mut Vec<u128>,
}

impl terminal_interpreter::TerminalImportAdapter for ObserveBuild<'_> {
    fn implemented_signature(&self) -> terminal_interpreter::TerminalImportSignature {
        self.signature.clone()
    }
}

impl terminal_interpreter::TerminalEffectHandler for ObserveBuild<'_> {
    fn handle_effect(
        &mut self,
        effect: &terminal_interpreter::TerminalEffect,
    ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
        let terminal_interpreter::TerminalEffect::BoundaryCall {
            structural_arguments,
            arguments,
            ..
        } = effect
        else {
            return Err(terminal_interpreter::TerminalEffectRejection::new(
                "expected a Build boundary call",
            ));
        };
        if structural_arguments.as_slice() != std::slice::from_ref(&self.original) {
            return Err(terminal_interpreter::TerminalEffectRejection::new(
                "not the original Build",
            ));
        }
        let [
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(sequence),
                ..
            },
        ] = arguments.as_slice()
        else {
            return Err(terminal_interpreter::TerminalEffectRejection::new(
                "missing sequence",
            ));
        };
        self.observations.push(*sequence);
        Ok(())
    }
}

fn exercise(artifact: &CanonicalTerminalArtifact) {
    let identifier_path = [field(artifact, "identifier")];
    let program = TerminalProgram::load_envelope(&artifact.to_bytes()).expect("external artifact");
    let original = TerminalStructuralValue::minted(
        951,
        program.entry().unwrap().structural_parameters[0].structural_type,
        Vec::new(),
    );
    let mut observations = Vec::new();
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation {
            scalar_arguments: &[],
            structural_inputs: TerminalStructuralInputs {
                arguments: std::slice::from_ref(&original),
                zero_initialized_arguments: &[0],
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let imports = instance.unbound_imports().collect::<Vec<_>>();
    assert_eq!(imports.len(), 1);
    let boundary = artifact
        .semantic_module()
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == imports[0])
        .unwrap();
    assert_eq!(
        boundary.reference_outputs.len(),
        3,
        "identifier and both ELF paths have explicit normal-exit lifetime obligations"
    );
    assert!(
        boundary
            .reference_outputs
            .iter()
            .all(|output| !output.dependencies.is_empty())
    );
    instance
        .bind_import(
            imports[0],
            ObserveBuild {
                signature: boundary.into(),
                original: original.clone(),
                observations: &mut observations,
            },
        )
        .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    // Small replenishments stop inside the real activation; completed host
    // calls must not be replayed when the same continuation resumes.
    let mut saw_effect_pause = false;
    for _ in 0..1_000 {
        match instance.invoke(&mut meter).unwrap() {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                saw_effect_pause |= !instance.effects().is_empty();
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => break,
            status => panic!("unexpected Build completion: {status:?}"),
        }
    }
    assert!(instance.result().is_some(), "bounded probe completes");
    assert!(saw_effect_pause);
    let TerminalEntryStorageValue::Bytes(bytes) = instance
        .observe_borrowed_entry(0, &identifier_path)
        .unwrap()
    else {
        panic!("identifier byte readback");
    };
    let mut identifier = vec![0; bytes.len()];
    bytes.copy_to(&mut identifier).unwrap();
    assert_eq!(identifier, b"after");
    drop(instance);
    assert_eq!(observations, [1, 2]);

    let mut foreign = original.clone();
    foreign.opaque_identity += 1;
    let mut observations = Vec::new();
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation {
            scalar_arguments: &[],
            structural_inputs: TerminalStructuralInputs {
                arguments: std::slice::from_ref(&foreign),
                zero_initialized_arguments: &[0],
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    instance
        .bind_import(
            imports[0],
            ObserveBuild {
                signature: boundary.into(),
                original,
                observations: &mut observations,
            },
        )
        .unwrap();
    let error = instance
        .invoke(&mut TerminalFuelMeter::unbounded())
        .unwrap_err();
    assert!(format!("{error:?}").contains("not the original Build"));
    assert_eq!(
        instance.observe_borrowed_entry(0, &identifier_path),
        Err(TerminalEntryStorageError::NormalReturnRequired)
    );
    drop(instance);
    assert!(observations.is_empty());
}
