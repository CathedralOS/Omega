use omega_lowering::{BlockTerminator, MachineBody, ReturnEdge};

pub fn sole_return(body: &MachineBody) -> ReturnEdge {
    let [block] = body.blocks.as_slice() else { panic!("expected one block") };
    assert_eq!(block.id.get(), 1);
    let BlockTerminator::Return { id, result, ref cleanup } = block.terminator else {
        panic!("expected a return")
    };
    assert_eq!(id.get(), 1);
    assert!(cleanup.is_empty());
    result
}
