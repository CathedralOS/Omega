//! Shared slice returns transport descriptors, with independently reconstructed
//! backing provenance. A full range uses the same subslice producer as a
//! narrowed range; neither introduces a pointer to a private descriptor.
use crate::{TerminalMachineSelection, lower_machine, lowering_error::LoweringError};

#[test]
fn parameter_whole_view_result_reaches_module_validation() {
    let checked = crate::front_end::checked_program(
        r#"
            data Nv { tag: u64; }

            machine Nv::head_all(&self, x: &[u8]) -> &[u8] {
                x[0..x.len]
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Nv::head_all"))
        .expect("whole parameter views pass module validation");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("whole parameter views verify");
    let module = &lowered.semantic_module;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| {
            module.structural_types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && declaration.shape.is_borrowed_byte_view()
            })
        })
        .expect("the explicit borrowed byte-view parameter");
    let terminal_psi::TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("structural result");
    };
    assert!(
        result.reference_sources.is_empty(),
        "descriptors are not permission references"
    );
    assert_eq!(
        result.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
    );
    let ranges = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::ByteSequenceSubslice { source, .. } => Some(source),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ranges,
        vec![parameter.place],
        "the returned range retains the actual backing"
    );
    assert!(
        !machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishReference { .. }
            ))
    );
}

#[test]
fn stored_view_field_cannot_be_replaced_with_a_reference_to_its_descriptor() {
    let checked = crate::front_end::checked_program(
        r#"
            data Wv<'r> { view: &'r [u8]; tag: u64; }

            machine Wv::head_all(&self) -> &'r [u8] {
                self.view[0..self.view.len]
            }
        "#,
    );
    // Range storage does not yet materialize a stored descriptor field.
    // It must reject rather than lend the field's temporary descriptor home.
    assert!(lower_machine(&checked, TerminalMachineSelection::Name("Wv::head_all")).is_err());
}

#[test]
fn owned_backing_cannot_escape_through_a_returned_descriptor() {
    // Descriptor return does not extend activation-owned backing storage.
    let checked = crate::front_end::checked_program(
        r#"
            data Nv { tag: u64; }

            machine Nv::head_owned(&self, x: [u8; 64]) -> &[u8] {
                x[0..x.len]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Nv::head_owned"))
        .expect_err("owned array backing cannot outlive its activation");
    assert!(
        matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::ByteSequenceSubsliceReturnUnsupported { .. }
            )
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn named_narrowed_and_forwarded_descriptors_share_return_custody() {
    for body in ["bytes", "bytes[bytes.len..bytes.len]", "whole(bytes)"] {
        let source = format!(
            r"
            machine whole(bytes: &[u8]) -> &[u8] {{ bytes[0..bytes.len] }}
            machine returned(bytes: &[u8]) -> &[u8] {{ {body} }}
        "
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("returned"))
            .unwrap_or_else(|error| panic!("{body}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{body}: {error:?}"));
    }
}

#[test]
fn returned_descriptor_is_available_to_a_subsequent_call() {
    let checked = crate::front_end::checked_program(
        r"
        data Main { bytes: [u8; 4]; }
        machine whole(bytes: &[u8]) -> &[u8] { bytes[0..bytes.len] }
        machine first(bytes: &[u8]) -> u8 {
            transition bytes.len > 0 { true -> read(bytes) _ -> (0) }
            state read(bytes: &[u8]) -> u8 { bytes[0] }
        }
        machine Main::main(main: &mut Main) -> u8 {
            main.bytes[0] = 70;
            let view: &[u8] = whole(&main.bytes);
            let value: u8 = first(view);
            value
        }
    ",
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("returned descriptors feed subsequent calls");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("returned descriptor retains original backing through its consumer");
}
