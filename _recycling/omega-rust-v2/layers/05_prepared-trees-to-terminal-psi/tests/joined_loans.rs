#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, CallArgument, Parameter};
use psi_handoff_consumer::check_joined_loan;

#[test]
fn a_parked_call_retains_the_view_arriving_from_the_taken_branch() {
    for (positive, negative, formals) in [("left", "right", [1, 2]), ("right", "left", [2, 1])] {
        let text = format!(
            "machine inspect(bytes:@[u8])->u64 suspends {{ bytes.length }}
             machine forward(flag:bool,left:@[u8],right:@[u8])->u64 suspends {{
                 let chosen = match flag {{true -> {positive}, false -> {negative},}};
                 let count:u64 = suspend inspect(chosen);
                 count +% 1u64
             }}"
        );
        let mut package = source::lower(&[("joined_loans.omg", &text)]).unwrap();
        let check = |package: &_| check_joined_loan(package, "forward", "inspect", formals);
        assert_eq!(check(&package), Ok(()));

        let caller = package
            .machines
            .iter_mut()
            .find(|machine| machine.declaration.path == "forward")
            .unwrap();
        let body = &mut caller.body;
        let [_, Parameter::Structural(left), Parameter::Structural(right)] = body.parameters[..] else {
            panic!("expected the incoming views");
        };
        for block in &mut body.blocks {
            let BlockTerminator::Jump(edge) = &mut block.terminator else {
                continue;
            };
            if let [CallArgument::CopyWhole(descriptor)] = edge.structural_arguments.as_mut_slice() {
                if *descriptor == left {
                    *descriptor = right;
                } else if *descriptor == right {
                    *descriptor = left;
                }
            }
        }
        assert_eq!(check(&package), Err("branch supplies the wrong incoming descriptor"));
    }
}
