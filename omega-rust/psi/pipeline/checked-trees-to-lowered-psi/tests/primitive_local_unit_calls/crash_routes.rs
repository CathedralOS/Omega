//! A local actual cannot suppress the ordinary callee's surviving crash ceiling.

use terminal_psi::OperationKind;

#[test]
fn local_unit_call_retains_scalar_substituted_crash_routes() {
    let source = super::source("u64")
        .replace("value: u64) {", "value: u64) crashes Abort value == 0u64 {")
        .replace(
            "replacement: u64) {",
            "replacement: u64) crashes Abort replacement == 0u64 {",
        );
    let artifact = super::artifact(&source);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller_index = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let caller = &module.machines[caller_index];
    let (block_index, operation_index) = caller
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .operations
                .iter()
                .position(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
                .map(|operation_index| (block_index, operation_index))
        })
        .unwrap();
    let OperationKind::CallUnit {
        callee,
        crash_continuations,
        ..
    } = &caller.blocks[block_index].operations[operation_index].kind
    else {
        panic!("Unit call");
    };
    assert!(!crash_continuations.is_empty());
    assert_eq!(crash_continuations, &caller.contract.crash_routes);
    let foreign = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap()
        .contract
        .crash_routes
        .clone();
    for mutation in 0..3 {
        let mut changed = module.clone();
        let OperationKind::CallUnit {
            arguments,
            crash_continuations,
            ..
        } = &mut changed.machines[caller_index].blocks[block_index].operations[operation_index]
            .kind
        else {
            panic!("Unit call");
        };
        match mutation {
            0 => crash_continuations.clear(),
            1 => *crash_continuations = foreign.clone(),
            2 => arguments[0] = caller.parameters[0].id,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                terminal_verifier::validate_module(&changed),
                Err(terminal_verifier::ModuleError::CallCrashContinuationsMismatch { .. })
            ),
            "crash continuation mutation {mutation}"
        );
    }
    let initial = super::integer(
        semantic_vocabulary::IntegerSign::Unsigned,
        64,
        semantic_vocabulary::IntegerValue::Unsigned(9),
    );
    let replacement = super::integer(
        semantic_vocabulary::IntegerSign::Unsigned,
        64,
        semantic_vocabulary::IntegerValue::Unsigned(0),
    );
    // A true published ceiling permits a crash; it does not execute one.
    super::execute(&artifact, initial, replacement, &[initial, replacement]);
}
