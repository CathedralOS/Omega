#[allow(dead_code)]
#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, CallTarget, CompletionContinuation, LoweredMachine, LoweredPackage, MachineBody, OperationKind, OperationResult,
    Parameter, ReferenceAccess, ReturnEdge,
};
use omega_resolved::Symbol;

const PICK: &str = "machine pick(flag:bool,left:@[u8],right:@[u8],dead:@[u8])->@[u8] {
    transition flag { true -> left, false -> right, }
}";
const RELAY: &str = "machine relay(flag:bool,left:@[u8],right:@[u8],dead:@[u8])->@[u8] {
    pick(flag,right,left,dead)
}";
const FORWARD: &str = "machine forward(flag:bool,left:@[u8],right:@[u8],dead:@[u8])->@[u8] {
    transition { _ -> relay(flag,left,right,dead), }
}";
const INSPECT: &str = "machine inspect(bytes:@[u8])->u64 suspends { bytes.length }";
const USE: &str = "machine use_it(flag:bool,left:@[u8],right:@[u8],dead:@[u8])->u64 suspends {
    let selected = forward(flag,left,right,dead);
    suspend inspect(selected)
}";

#[test]
fn a_returned_byte_view_retains_the_branch_selected_formal_through_both_call_forms() {
    for definitions in [
        [PICK, RELAY, FORWARD, INSPECT, USE],
        [USE, INSPECT, FORWARD, RELAY, PICK],
    ] {
        let text = definitions.join("\n");
        let package = prepare_and_lower(&text);
        let pick = machine(&package, "pick");
        let relay = machine(&package, "relay");
        let forward = machine(&package, "forward");
        let user = machine(&package, "use_it");
        let inspect = machine(&package, "inspect");

        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &pick.body.blocks[0].terminator
        else {
            panic!("the selected return follows the flag");
        };
        assert_eq!(Parameter::Scalar(*condition), pick.body.parameters[0]);
        let alternatives = [when_true, when_false].map(|arm| {
            let BlockTerminator::Return {
                result: ReturnEdge::Structural { source, .. },
                cleanup,
                ..
            } = after_jumps(&pick.body, arm.target.get() - 1)
            else {
                panic!("each arm returns its selected byte view");
            };
            assert!(cleanup.is_empty());
            pick.body
                .parameters
                .iter()
                .position(|parameter| *parameter == Parameter::Structural(*source))
                .unwrap()
        });
        assert_eq!(alternatives, [1, 2]);

        let [ordinary] = relay.body.operations.as_slice() else {
            panic!("relay invokes pick exactly once");
        };
        let OperationKind::Call { callee, arguments, .. } = &ordinary.kind else {
            panic!("relay invokes pick as an ordinary call");
        };
        assert_eq!(*callee, CallTarget::Machine(pick.id));
        let relay_actuals = [0, 2, 1, 3];
        assert_eq!(arguments.as_slice(), arguments_at(&relay.body, relay_actuals));
        assert_eq!(alternatives.map(|formal| relay_actuals[formal]), [2, 1]);
        let OperationResult::Structural(result) = ordinary.result else {
            panic!("relay receives a view");
        };
        assert!(matches!(
            after_jumps(&relay.body, 0),
            BlockTerminator::Return { result: ReturnEdge::Structural { source, .. }, cleanup, .. }
                if *source == result && cleanup.is_empty()
        ));

        let BlockTerminator::TerminalCall {
            callee, arguments, cleanup, ..
        } = after_jumps(&forward.body, 0)
        else {
            panic!("forward invokes relay in terminal position");
        };
        assert_eq!(*callee, CallTarget::Machine(relay.id));
        assert_eq!(arguments.as_slice(), arguments_at(&forward.body, [0, 1, 2, 3]));
        assert!(cleanup.is_empty());

        let [completed, parked] = user.body.operations.as_slice() else {
            panic!("the call producing the view completes before inspect parks");
        };
        let OperationKind::Call { callee, arguments, .. } = &completed.kind else {
            panic!("use_it invokes forward");
        };
        assert_eq!(*callee, CallTarget::Machine(forward.id));
        assert_eq!(arguments.as_slice(), arguments_at(&user.body, [0, 1, 2, 3]));
        let OperationResult::Structural(descriptor) = completed.result else {
            panic!("forward establishes the view");
        };
        let OperationKind::Call { callee, arguments, .. } = &parked.kind else {
            panic!("use_it invokes inspect");
        };
        assert_eq!(
            (*callee, arguments.as_slice()),
            (CallTarget::Machine(inspect.id), [CallArgument::CopyWhole(descriptor)].as_slice())
        );
        let [site] = user.body.suspension_sites.as_slice() else {
            panic!("only inspect parks");
        };
        assert_eq!((site.call, site.callee), (CallSite::Operation(parked.id), *callee));
        assert_eq!(site.continuation, CompletionContinuation::Terminator(user.body.blocks[0].id));
        assert!(site.activation.is_empty());
        let [loan] = site.retained_loans.as_slice() else {
            panic!("the completed call determines one live loan");
        };
        assert_eq!(
            (loan.descriptor, loan.ty, loan.access),
            (descriptor, inspect.parameter_types[0], ReferenceAccess::Shared)
        );
        assert!(loan.needed_by_callee && !loan.needed_after_completion);
    }
}

fn prepare_and_lower(text: &str) -> LoweredPackage {
    let resolved = source::resolve(&[("returned_view_inputs.omg", text)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let typed_tree = typed.trees().next().unwrap();
    let tree = typed_tree.resolved();
    for (caller, callee, actuals) in [
        ("relay", "pick", ["flag", "right", "left", "dead"].as_slice()),
        ("forward", "relay", ["flag", "left", "right", "dead"].as_slice()),
        ("use_it", "forward", ["flag", "left", "right", "dead"].as_slice()),
        ("use_it", "inspect", ["selected"].as_slice()),
    ] {
        let call = typed
            .calls
            .iter()
            .find(|call| call.machine.path == callee && call.caller.as_ref().is_some_and(|owner| owner.path == caller))
            .unwrap();
        assert_eq!(call.operands.len(), actuals.len());
        assert_eq!(call.acknowledgements.suspend, callee == "inspect");
        let formals = if callee == "inspect" {
            ["bytes"].as_slice()
        } else {
            ["flag", "left", "right", "dead"].as_slice()
        };
        for ((operand, &actual), &formal) in call.operands.iter().zip(actuals).zip(formals) {
            assert_eq!(operand.site.span.slice(text.as_bytes()), Some(actual.as_bytes()));
            let Some(Symbol::Binding { span, .. }) = tree.symbol(operand.node.node) else {
                panic!("expected the caller's lexical binding");
            };
            let suffix = if actual == "selected" { " =" } else { ":" };
            assert_eq!(span.get_start_offset(), binding_offset(text, caller, actual, suffix));
            assert_eq!(operand.parameter.span.get_start_offset(), binding_offset(text, callee, formal, ":"));
        }
    }
    let package = omega_lowering::lower_package(&prepared).unwrap();
    for machine in &package.machines {
        source::control_flow::check_operation_ownership(&machine.body).unwrap();
    }
    package
}

fn binding_offset(
    text: &str,
    machine: &str,
    name: &str,
    suffix: &str,
) -> usize {
    let start = text.find(&format!("machine {machine}(")).unwrap();
    start + text[start..].find(&format!("{name}{suffix}")).unwrap()
}

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .unwrap()
}

fn after_jumps(
    body: &MachineBody,
    start: usize,
) -> &BlockTerminator {
    let mut block = &body.blocks[start];
    for _ in 0..body.blocks.len() {
        let BlockTerminator::Jump(edge) = &block.terminator else {
            return &block.terminator;
        };
        block = &body.blocks[edge.target.get() - 1];
    }
    panic!("expected a finite forwarding path")
}

fn arguments_at(
    body: &MachineBody,
    positions: [usize; 4],
) -> [CallArgument; 4] {
    positions.map(|index| match body.parameters[index] {
        Parameter::Scalar(value) => CallArgument::Scalar(value),
        Parameter::Structural(place) => CallArgument::CopyWhole(place),
    })
}
