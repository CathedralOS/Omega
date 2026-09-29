//! Source-produced bounded byte fields presented to an external boundary.
//!
//! A field whose domain declares byte predicates is a restricted owner. A
//! shared loan grants no mutation, so a boundary may read such a field in
//! place. A bare mutable byte view could return bytes that break the field's
//! invariant, and Terminal opens no invariant window on a field, so
//! verification rejects that loan at the exact call argument
//! (`wiki/spec/terminal-psi/structural_access.md`: "passing a restricted field
//! to a bare mutable byte view is rejected").
use super::lower_machine;
use crate::{LoweringError, TerminalMachineSelection};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, TerminalModule};
use terminal_verifier::ModuleError;
mod checked_provider;

const INPUT_SOURCE: &str = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Input { machine read(out: &mut [u8]) reaches Input; }
        data Record { out: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        machine Record::run(&mut self) reaches Input {
            self.out = "old";
            self.other = "QQ";
            Input::read(&mut self.out);
            Input::read(&mut self.out);
            self.out = "fin";
        }
        "#;

const SEPARATE_DESTINATIONS_SOURCE: &str = r#"
        domain [u8; 3]::Utf8 requires valid_utf8(self);
        boundary trait Input { machine read(first: &mut [u8], second: &mut [u8]) reaches Input; }
        data Record { first: [u8; 3] in Utf8; second: [u8; 3] in Utf8; other: [u8; 3] in Utf8; }
        machine Record::run(&mut self) reaches Input {
            self.first = "old";
            self.second = "two";
            self.other = "QQ";
            Input::read(&mut self.second, &mut self.first);
            self.first = "one";
            self.second = "two";
        }
    "#;

/// The same program with every boundary loan shared.
fn shared_twin(source: &str) -> String {
    source
        .replace("&mut [u8]", "&[u8]")
        .replace("&mut self.", "&self.")
}

/// The first boundary call of the module's entry.
fn first_boundary_call(module: &TerminalModule) -> semantic_vocabulary::OperationId {
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::BoundaryCall { .. }).then_some(operation.id)
        })
        .expect("the authored boundary call")
}

/// A shared boundary loan of a restricted field verifies and executes: the
/// boundary observes the field's live bytes in place. The mutable twin of the
/// same program is refused at the first boundary call's first field loan.
#[test]
fn a_restricted_field_reaches_a_boundary_only_through_a_shared_loan() {
    for (source, observed) in [
        (
            INPUT_SOURCE,
            [b"old".as_slice(), b"old".as_slice()].as_slice(),
        ),
        (
            SEPARATE_DESTINATIONS_SOURCE,
            [b"two".as_slice(), b"old".as_slice()].as_slice(),
        ),
    ] {
        let shared = crate::front_end::checked_program(&shared_twin(source));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &shared,
            terminal_production::TerminalMachineSelection::Name("Record::run"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("a shared loan cannot break the field's predicate")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let first_call = first_boundary_call(&module);
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 73,
                    structural_type: entry.structural_parameters[0].structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            execution
                .resume(
                    &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                    &mut AcceptTerminalEffects
                )
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        let presented = execution
            .effects()
            .iter()
            .flat_map(|effect| match effect {
                TerminalEffect::BoundaryCall {
                    byte_sequence_arguments,
                    ..
                } => byte_sequence_arguments.clone(),
                _ => Vec::new(),
            })
            .map(|bytes| bytes.expect("a presented field"))
            .collect::<Vec<_>>();
        assert_eq!(presented, observed, "{source}");

        let mutable = crate::front_end::checked_program(source);
        assert_eq!(
            lower_machine(&mutable, TerminalMachineSelection::Name("Record::run")).unwrap_err(),
            LoweringError::InvalidTerminalModule(ModuleError::InvalidStructuralArgumentPath {
                operation: first_call,
                argument_index: 0,
            }),
            "{source}"
        );
    }
}
