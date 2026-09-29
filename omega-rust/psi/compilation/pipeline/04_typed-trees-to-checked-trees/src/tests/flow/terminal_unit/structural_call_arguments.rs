//! A nested call whose result is structural may feed a call statement's
//! non-scalar argument position: the operand is sequenced before its outer
//! call and bound in the shared structural result namespace.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::CheckedUnitCallResult;

fn operations(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked(source);
    let machine = machine_named(&checked, "main");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "the structural call argument plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

#[test]
fn copy_record_result_feeds_a_call_statement_argument() {
    let operations = operations(
        r#"
        data Info [copy] { process_id: u32; handle: u64; }
        machine Info::new(process_id: u32) -> Info {
            Info { process_id: process_id, handle: 0 }
        }
        data Holder { info: Info; }
        machine Holder::set_info(&mut self, info: Info) {
        }
        data Main { holder: Holder; }
        machine Main::main(&mut self) {
            self.holder.set_info(Info::new(7));
        }
        "#,
    );
    let [
        CheckedUnitEffectOperationPlan::Call(nested),
        CheckedUnitEffectOperationPlan::Call(consuming),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("nested structural call, consuming call, complete: {operations:#?}")
    };
    let Some(nested_result) = nested.structural_result() else {
        panic!("nested structural call, consuming call, complete: {operations:#?}")
    };
    assert!(
        !nested.is_boundary()
            && !consuming.is_boundary()
            && *consuming.result() == CheckedUnitCallResult::Unit,
        "nested structural call, consuming call, complete: {operations:#?}"
    );
    assert_eq!(nested.coordinate.statement_index, 0);
    assert_eq!(nested.coordinate.call_ordinal, 1);
    assert_eq!(nested_result.binding_ordinal, 0);
    assert_eq!(
        (
            consuming.coordinate.statement_index,
            consuming.coordinate.call_ordinal
        ),
        (0, 0)
    );
    let structural_arguments = &consuming.structural_arguments;
    let [argument] = structural_arguments.as_slice() else {
        panic!("the consuming call names exactly the nested result: {structural_arguments:#?}")
    };
    assert!(matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    ));
    assert!(argument.path.is_empty());
}

#[test]
fn scalar_calls_inside_a_conjunction_return_are_consumed() {
    let operations = operations(
        r#"
        data Response { ok: bool; }
        machine Response::is_ok(&self) -> bool { self.ok }
        machine Response::get_val(&self, index: u64) -> u64 { index }
        data Engine { response: Response; }
        data Main { engine: Engine; }
        machine Main::main(&mut self) -> bool {
            let ok: bool = self.engine.response.is_ok();
            ok && self.engine.response.get_val(0) == 4
                && self.engine.response.get_val(1) == 8
        }
        "#,
    );
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation, CheckedUnitEffectOperationPlan::Complete { .. })),
        "conjunction-tail scalar calls compose: {operations:#?}"
    );
}

#[test]
fn crash_return_with_structural_locals_and_conjunction_tail() {
    let operations = operations(
        r#"
        data Response { ok: bool; }
        machine Response::is_ok(&self) -> bool { self.ok }
        machine Response::get_val(&self, index: u64) -> u64 { index }
        data Engine { response: Response; }
        machine Engine::fetch(&mut self) -> Response {
            Response { ok: true }
        }
        data Main { engine: Engine; }
        machine Main::main(&mut self) -> bool crashes Abort {
            let response: Response = self.engine.fetch();
            let ok: bool = response.is_ok();
            ok && response.get_val(0) == 4
                && response.get_val(1) == 8
        }
        "#,
    );
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation, CheckedUnitEffectOperationPlan::Complete { .. })),
        "crash-body structural local + conjunction tail composes: {operations:#?}"
    );
}

#[test]
fn verify_page_shape_composes() {
    let source = r#"
        data ListRequest { page_index: u64; has_filters: bool }
        data ScanResultsCommand { case List(request: ListRequest); }
        data Command { case Scan(command: ScanResultsCommand); case Ping; }
        data ScanResult { address: u64; tag: [u8; 8]; tag_len: u64 }
        data ListPage { scan_results: [ScanResult; 4]; count: u64; addr: u64 }
        data ScanResultsResponse { case List(response: ListPage); }
        data Response { case Empty; case ScanResults(response: ScanResultsResponse); }
        machine Response::is_scan_results(&self) -> bool { true }
        machine Response::get_result_count(&self) -> u64 { 0 }
        machine Response::get_result_address(&self, index: u64) -> u64 { index }
        machine Response::get_result_value(&self, index: u64) -> u64 { index }
        data Engine {}
        machine Engine::dispatch_command(&mut self, command: Command, response: Response) -> Response {
            response
        }
        data Driver { engine: Engine; }
        machine Driver::verify_page(&mut self, command: Command, seed: Response) -> bool crashes Abort {
            let response: Response = self.engine.dispatch_command(command, seed);
            let is_list: bool = response.is_scan_results();
            let count: u64 = response.get_result_count();
            is_list && count == 8
                && response.get_result_address(0) == 4096 && response.get_result_value(0) == 0
                && response.get_result_address(1) == 4104 && response.get_result_value(1) == 0
        }
        "#;
    let checked = checked(source);
    let machine = machine_named(&checked, "verify_page");
    let operations = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .map(|plans| &plans.operations)
        .unwrap_or_else(|| {
            panic!(
                "verify_page shape composes: {:?}",
                checked.facts.flow.terminal_unit_effects.omissions
            )
        });
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation, CheckedUnitEffectOperationPlan::Complete { .. })),
        "verify_page shape composes: {operations:#?}"
    );
}

#[test]
fn mutable_borrow_of_indexed_structural_result_element_lends_the_element() {
    let operations = operations(
        r#"
        data Room { exit_count: u32; }
        data Main { }
        machine Main::bump(&mut self, room: &mut Room) {
            room.exit_count = 1;
        }
        machine Main::main(&mut self) {
            let rooms: [Room; 1] = [Room { exit_count: 0 }];
            let from_index: u64 = 0;
            self.bump(&mut rooms[from_index]);
        }
        "#,
    );
    let Some(call) = operations.iter().find_map(|operation| {
        operation
            .call()
            .filter(|call| !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit)
    }) else {
        panic!("the bump call is planned: {operations:#?}")
    };
    let structural_arguments = &call.structural_arguments;
    let [argument] = structural_arguments.as_slice() else {
        panic!("the call passes one structural argument: {structural_arguments:#?}")
    };
    assert!(
        matches!(
            argument.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: 0
            }
        ) && argument.access == checked_trees::CheckedStructuralAccess::MutableBorrow
            && argument.path.as_slice()
                == [checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    0
                )],
        "the caller's array local lends its folded element mutably: {argument:#?}"
    );
}
