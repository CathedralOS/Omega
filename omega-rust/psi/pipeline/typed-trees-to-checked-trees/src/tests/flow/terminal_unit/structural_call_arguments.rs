//! A nested call whose result is structural may feed a call statement's
//! non-scalar argument position: the operand is sequenced before its outer
//! call and bound in the shared structural result namespace.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

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
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate: nested_coordinate,
            result: nested_result,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("nested structural call, consuming call, complete: {operations:#?}")
    };
    assert_eq!(nested_coordinate.statement_index, 0);
    assert_eq!(nested_coordinate.call_ordinal, 1);
    assert_eq!(nested_result.binding_ordinal, 0);
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
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
fn affine_plain_owned_result_feeds_a_call_statement_argument() {
    let operations = operations(
        r#"
        data Source { value: u64; }
        machine Source::make(value: u64) -> Source {
            Source { value: value }
        }
        data Holder { stored: u64; }
        machine Holder::keep(&mut self, source: Source) {
            let kept: u64 = source.value;
        }
        data Main { holder: Holder; }
        machine Main::main(&mut self) {
            self.holder.keep(Source::make(7));
        }
        "#,
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate: nested_coordinate,
            result: nested_result,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        ..,
    ] = operations.as_slice()
    else {
        panic!("affine nested call feeding the consuming call: {operations:#?}")
    };
    assert_eq!(nested_coordinate.call_ordinal, 1);
    assert_eq!(nested_result.binding_ordinal, 0);
    assert_eq!(nested_result.multiplicity, super::Multiplicity::Affine);
    let [argument] = structural_arguments.as_slice() else {
        panic!("one structural argument: {structural_arguments:#?}")
    };
    assert!(matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: 0
        }
    ));
}

#[test]
fn scalar_result_still_feeds_a_scalar_argument() {
    let operations = operations(
        r#"
        machine inner(value: u32) -> u32 { value }
        data Holder { total: u32; }
        machine Holder::set_total(&mut self, total: u32) {
            self.total = total;
        }
        data Main { holder: Holder; }
        machine Main::main(&mut self) {
            self.holder.set_total(inner(7) + 1u32);
        }
        "#,
    );
    assert!(
        operations
            .iter()
            .any(|operation| matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })),
        "scalar nested calls keep their computed operand lane: {operations:#?}"
    );
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
        data Command { case Ping; case Fetch(value: u64); }
        data Response { case Empty; case ScanResults(count: u64, addr: u64); }
        machine Response::is_scan_results(&self) -> bool { true }
        machine Response::get_result_count(&self) -> u64 { 0 }
        machine Response::get_result_address(&self, index: u64) -> u64 { index }
        data Engine {}
        machine Engine::dispatch_command(&mut self, command: Command) -> Response {
            Response::Empty
        }
        data Driver { engine: Engine; }
        machine Driver::list_command(&mut self) -> Command {
            Command::Ping
        }
        machine Driver::verify_page(&mut self) -> bool crashes Abort {
            let command: Command = self.list_command();
            let response: Response = self.engine.dispatch_command(command);
            let is_list: bool = response.is_scan_results();
            let count: u64 = response.get_result_count();
            is_list && count == 8
                && response.get_result_address(0) == 4096
                && response.get_result_address(1) == 4104
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
