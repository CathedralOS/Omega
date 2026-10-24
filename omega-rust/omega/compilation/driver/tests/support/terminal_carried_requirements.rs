//! Actual public Terminal ingress preserves exact carried requirement identities.
//! Inputs below are explicit trusted host values under the inspection entry's
//! declared contract, as with the existing materialized-entry provision. This
//! does not introduce guest evidence, native provisions or private registration.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecutionResult, TerminalExecutionStatus, TerminalImportAdapter,
    TerminalImportSignature, TerminalInterpretError, TerminalProgram, TerminalScalarValue,
    TerminalStructuralInputs, TerminalStructuralScalarFieldValue, TerminalStructuralValue,
};

struct Sink {
    signature: TerminalImportSignature,
    calls: Arc<AtomicUsize>,
}
impl TerminalImportAdapter for Sink {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for Sink {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "typed boundary result required",
        ))
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !matches!(effect, TerminalEffect::BoundaryCall { .. }) {
            return Err(TerminalEffectRejection::new("expected one boundary call"));
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(TerminalEffectResult::Unit)
    }
}

pub(super) fn observe(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(artifact))).map_err(
        |payload| {
            let detail = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            format!("carried boundary ingress control failed: {detail}")
        },
    )
}

fn exercise(artifact: &CanonicalTerminalArtifact) {
    let program = TerminalProgram::borrow(artifact.view());
    let parameters = program
        .entry()
        .expect("inspection entry")
        .structural_parameters;
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0].structural_type, parameters[1].structural_type);
    assert_eq!(parameters[0].qualifications.len(), 1);
    assert_eq!(parameters[1].qualifications.len(), 1);
    assert_ne!(parameters[0].qualifications, parameters[1].qualifications);
    let module = artifact.semantic_module();
    let declaration = module
        .structural_types
        .iter()
        .find(|row| row.id == parameters[0].structural_type)
        .expect("input carrier");
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        panic!("ordinary materialized input")
    };
    assert_eq!(fields.len(), 1);
    let arguments: Vec<_> = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            TerminalStructuralValue::minted(
                91_000 + index as u64,
                parameter.structural_type,
                parameter.qualifications.clone(),
            )
        })
        .collect();
    let scalar_fields: Vec<_> = (0..2)
        .map(|argument_index| TerminalStructuralScalarFieldValue {
            argument_index,
            path: Vec::new(),
            field: fields[0].id,
            value: TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    32,
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Unsigned(7),
            },
        })
        .collect();
    let calls = Arc::new(AtomicUsize::new(0));
    // Missing and swapped domains keep the same valid type, identity and data.
    // Entry admission is the first required owner; neither may reach a host.
    let mut missing = arguments.clone();
    missing[0].qualifications.clear();
    let mut swapped = arguments.clone();
    swapped[0].qualifications = arguments[1].qualifications.clone();
    swapped[1].qualifications = arguments[0].qualifications.clone();
    for invalid in [missing, swapped] {
        assert!(matches!(program.instantiate(&[], TerminalStructuralInputs {
            arguments: &invalid, scalar_fields: &scalar_fields, ..Default::default()
        }), Err(TerminalArtifactInterpretError::Execution(
            TerminalInterpretError::StructuralQualificationMissing(place)
        )) if place == parameters[0].place));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    // The same source-produced program and ordinary public binding really
    // perform an effect with the correct inputs; refusal is not vacuous.
    let mut instance = program
        .instantiate(
            &[],
            TerminalStructuralInputs {
                arguments: &arguments,
                scalar_fields: &scalar_fields,
                ..Default::default()
            },
        )
        .expect("valid trusted contract inputs");
    let imports = instance.unbound_imports().collect::<Vec<_>>();
    let [slot] = imports.as_slice() else {
        panic!("one demanded host sink: {imports:?}")
    };
    let boundary = instance.import(*slot).expect("demanded import declaration");
    assert_eq!(boundary.requires.len(), 2);
    for (index, parameter) in parameters.iter().enumerate() {
        assert!(
            boundary
                .requires
                .iter()
                .any(|row| row.argument_index as usize == index
                    && row.domain == parameter.qualifications[0])
        );
    }
    instance
        .bind_import(
            boundary.id,
            Sink {
                signature: boundary.into(),
                calls: Arc::clone(&calls),
            },
        )
        .expect("exact public adapter signature");
    assert_eq!(
        instance
            .invoke(&mut TerminalFuelMeter::unbounded())
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    instance.close().unwrap();
}
