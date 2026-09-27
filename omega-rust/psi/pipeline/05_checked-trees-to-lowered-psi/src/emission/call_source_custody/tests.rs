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

#[test]
fn explicit_owned_self_call_rejoins_only_its_authored_receiver() {
    let mut checked = crate::front_end::checked_program(
        "data Token [linear] { code: i32; }
         boundary requirement Token::finish(self);
         machine Token::settle(self, other: Token) {
             Token::finish(self);
             Token::finish(other);
         }",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Token::settle"))
        .expect("both owned parameters explicitly complete their boundary disposition");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Token::settle")
        .expect("settle machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("owned-self body has a Unit plan");
    let argument = plan
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            } => structural_arguments.first_mut(),
            _ => None,
        })
        .expect("first boundary call consumes self");
    assert_eq!(argument.source_parameter_index(), Some(0));
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert_eq!(
        lower_machine(&checked, TerminalMachineSelection::Name("Token::settle"))
            .expect_err("a same-typed parameter cannot replace the authored receiver"),
        crate::LoweringError::Unsupported("owned call argument changed its authored parameter")
    );
}

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

/// `cells[i].get()` lowers the runtime-selected borrowed element receiver to
/// a `RuntimeIndex` structural argument: the selector is the calling state's
/// own immutable scalar operand, the index bound is emitted as a call
/// obligation, and terminal verification discharges it both from the
/// retained-range spelling (`i: u64 [0..=1]`) and from the authored
/// `requires` conjunct.
#[test]
fn dynamic_indexed_shared_receiver_emits_a_runtime_index_argument() {
    for (index_decl, requires) in [("i: u64 [0..=1]", ""), ("i: u64", "requires i <= 1")] {
        let source = format!(
            "data Cell {{ value: u64; }}
             machine Cell::get(&self) -> u64 {{ self.value }}
             machine run(cells: &[Cell; 2], {index_decl}) -> u64
             {requires} {{ cells[i].get() }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
            .unwrap_or_else(|error| panic!("{index_decl} {requires}: {error:#?}"));
        let runtime_indexed = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .flat_map(|operation| match &operation.kind {
                OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                } => structural_arguments.clone(),
                _ => Vec::new(),
            })
            .flat_map(|argument| argument.path)
            .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }));
        assert!(
            runtime_indexed,
            "{index_decl} {requires}: the borrowed element receiver lowers to a RuntimeIndex segment"
        );
        let _ = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("run"),
        )
        .produce(
            terminal_production::TerminalProductionCustody::artifact_only(
                &mut terminal_production::TerminalProductionTimings::default(),
            ),
        )
        .unwrap_or_else(|error| {
            panic!(
                "{index_decl} {requires}: terminal production rejects the runtime-indexed receiver: {error:#?}"
            )
        });
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
