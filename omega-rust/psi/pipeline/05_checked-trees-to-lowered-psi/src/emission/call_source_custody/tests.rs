//! Source custody for a scalar call result the same statement stores.
//!
//! `self.field = Host::close(..)` binds no authored local, so the boundary
//! call's result carries no `CheckedScalarExpressionRole::LocalInitializer`
//! and no `Return` role either. Custody rejoins the authored assignment's
//! right-hand side instead, and the consuming `StructuralScalarFieldStore`
//! reads the call's own SSA value. Without that arm the whole body refuses
//! with "scalar source custody disagrees with its authored destination role",
//! which is what kept `tests/omega/pass/filesystem/native_close` out of
//! lowering.
//!
//! `wiki/spec/terminal-psi/structural_access.md` gives
//! `StructuralScalarFieldStore` an "already-defined, exactly typed SSA value";
//! a completed call result is such a value, so no authored local has to name
//! it.

use crate::TerminalMachineSelection;
use crate::lower_machine;
use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment, TerminalModule};

/// The carrier path of the one field store that reads the one emitted call's
/// scalar result. Stores of other values, such as the preceding literal
/// assignment, are not this consumer.
fn carrier_path_storing_the_call_result(module: &TerminalModule) -> Vec<StructuralPathSegment> {
    let operations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let calls = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::Call { .. }
                    | OperationKind::CallStructuralScalar { .. }
                    | OperationKind::BoundaryCall { .. }
            )
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one emitted call per authored call site: {calls:#?}");
    };
    let OperationResult::Scalar(result) = &call.result else {
        panic!("a scalar boundary call declares a scalar result: {call:#?}");
    };
    let consumers = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::StructuralScalarFieldStore { value, path, .. }
                if *value == result.id =>
            {
                Some(path.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [path] = consumers.as_slice() else {
        panic!("exactly one store reads the call result: {consumers:#?}");
    };
    path.clone()
}

const DIRECT_FIELD: &str = r#"
    data Main { fd_in: i32; rc: i32; }
    boundary trait Host {
        machine close(fd: i32) -> i32 reaches Host;
    }
    machine Main::main(&mut self) reaches Host {
        self.fd_in = 5;
        self.rc = Host::close(self.fd_in);
    }
"#;

/// The carrier path is the only difference from `DIRECT_FIELD`: the same
/// statement shape must compose with an ordered path to the carrier record
/// rather than depending on the field sitting directly on the root.
const CARRIER_PATH_FIELD: &str = r#"
    data Status { rc: i32; }
    data Main { fd_in: i32; status: Status; }
    boundary trait Host {
        machine close(fd: i32) -> i32 reaches Host;
    }
    machine Main::main(&mut self) reaches Host {
        self.fd_in = 5;
        self.status.rc = Host::close(self.fd_in);
    }
"#;

/// The same store into a bounded destination field. The call result carries no
/// range evidence, so the destination-range check must still refuse it: the
/// custody arm admits a source, not an unproved value.
const BOUNDED_DESTINATION: &str = r#"
    data Main { fd_in: i32; rc: i32 [0..=16]; }
    boundary trait Host {
        machine close(fd: i32) -> i32 reaches Host;
    }
    machine Main::main(&mut self) reaches Host {
        self.fd_in = 5;
        self.rc = Host::close(self.fd_in);
    }
"#;

#[test]
fn same_statement_field_store_reads_its_own_boundary_call_result() {
    let checked = crate::front_end::checked_program(DIRECT_FIELD);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .unwrap_or_else(|error| panic!("{DIRECT_FIELD}: {error:#?}"));
    assert!(
        carrier_path_storing_the_call_result(&lowered.semantic_module).is_empty(),
        "an empty carrier path denotes a field directly on the root record"
    );
}

#[test]
fn same_statement_field_store_composes_with_a_carrier_path() {
    let checked = crate::front_end::checked_program(CARRIER_PATH_FIELD);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .unwrap_or_else(|error| panic!("{CARRIER_PATH_FIELD}: {error:#?}"));
    assert_eq!(
        carrier_path_storing_the_call_result(&lowered.semantic_module),
        vec![StructuralPathSegment::Field("status".into())],
        "the ordered path reaches the carrier record holding `rc`"
    );
}

/// `cells[i].get()` with the selector's bound published as contract facts:
/// both the retained-range spelling and the authored `requires` spelling
/// currently stop at the same two upstream owners. The terminal verifier
/// already replays requires-derived `RuntimeIndex` bounds (the
/// terminal-interpreter `runtime_index_arguments` tests pin that replay);
/// what remains upstream is (1) checked admission folding authored
/// `requires` conjuncts in `execution/terminal_unit/calls/argument_paths.rs`
/// — under a live CML4 claim — and (2) the scalar-wrapper source path
/// carrying a dynamic index in `unit/attached_unit/parameters/source_path.rs`
/// — under a live DOMAIN-ISSUER-ROUTES claim. When the checked stage cannot
/// prove the selector it emits no Unit-effect body for the caller at all,
/// so the `requires` variant surfaces as the machine-selection failure
/// below rather than as an admission diagnostic. This pin records the exact
/// frontier so landing either leg flips this expectation instead of hiding
/// inside a probe that only logs errors.
#[test]
fn dynamic_indexed_shared_receiver_lane_pends_on_upstream_legs() {
    for (index_decl, requires, expected) in [
        (
            "i: u64 [0..=1]",
            "",
            "scalar wrapper structural argument requires a literal index",
        ),
        (
            "i: u64",
            "requires i <= 1",
            "machine has no source-independent checked scalar control plan",
        ),
    ] {
        let source = format!(
            "data Cell {{ value: u64; }}
             machine Cell::get(&self) -> u64 {{ self.value }}
             machine run(cells: &[Cell; 2], {index_decl}) -> u64
             {requires} {{ cells[i].get() }}"
        );
        let checked = crate::front_end::checked_program(&source);
        match lower_machine(&checked, TerminalMachineSelection::Name("run")) {
            Err(crate::LoweringError::Unsupported(message)) => {
                assert_eq!(message, expected, "{index_decl} {requires}");
            }
            other => panic!(
                "{index_decl} {requires}: expected the pinned upstream rejection, \
                 got {other:?} — an upstream leg landed; flip this pin to assert \
                 the emitted RuntimeIndex segment and terminal production"
            ),
        }
        match terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("run"),
        )
        .produce(
            terminal_production::TerminalProductionCustody::artifact_only(
                &mut terminal_production::TerminalProductionTimings::default(),
            ),
        ) {
            Err(error) => match error.error() {
                // `LoweringError` arrives through terminal-production's own
                // dependency edge, so this test cannot name the variant —
                // compare the rendered `Unsupported("…")` instead.
                terminal_production::TerminalArtifactProductionError::Lowering(inner) => {
                    assert_eq!(
                        format!("{inner}"),
                        format!("Unsupported({expected:?})"),
                        "{index_decl} {requires}"
                    );
                }
                other => panic!(
                    "{index_decl} {requires}: expected the pinned production rejection, \
                     got {other:?} — an upstream leg landed; flip this pin to assert \
                     the emitted RuntimeIndex segment and terminal production"
                ),
            },
            Ok(_) => panic!(
                "{index_decl} {requires}: terminal production succeeded — an upstream \
                 leg landed; flip this pin to assert the emitted RuntimeIndex segment \
                 and terminal production"
            ),
        }
    }
}

#[test]
fn same_statement_field_store_still_refuses_an_unproved_bounded_destination() {
    let Err(diagnostics) = crate::front_end::checked_program_result(BOUNDED_DESTINATION) else {
        panic!("a call result with no range evidence cannot land in a bounded field");
    };
    assert!(
        !diagnostics.is_empty(),
        "the refusal names at least one diagnostic: {diagnostics:#?}"
    );
}
