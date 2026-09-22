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
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment, TerminalModule};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed, &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
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
    let checked = checked(DIRECT_FIELD);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .unwrap_or_else(|error| panic!("{DIRECT_FIELD}: {error:#?}"));
    assert!(
        carrier_path_storing_the_call_result(&lowered.semantic_module).is_empty(),
        "an empty carrier path denotes a field directly on the root record"
    );
}

#[test]
fn same_statement_field_store_composes_with_a_carrier_path() {
    let checked = checked(CARRIER_PATH_FIELD);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .unwrap_or_else(|error| panic!("{CARRIER_PATH_FIELD}: {error:#?}"));
    assert_eq!(
        carrier_path_storing_the_call_result(&lowered.semantic_module),
        vec![StructuralPathSegment::Field("status".into())],
        "the ordered path reaches the carrier record holding `rc`"
    );
}

#[test]
fn same_statement_field_store_still_refuses_an_unproved_bounded_destination() {
    let tokens = Lexer::new(BOUNDED_DESTINATION)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let Err(diagnostics) = lower_typed_trees(typed, &CheckingRequest::settled()) else {
        panic!("a call result with no range evidence cannot land in a bounded field");
    };
    assert!(
        !diagnostics.is_empty(),
        "the refusal names at least one diagnostic: {diagnostics:#?}"
    );
}
