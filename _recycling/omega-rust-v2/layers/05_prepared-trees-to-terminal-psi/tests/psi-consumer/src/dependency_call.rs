//! Follows a returned call to the package that declares its callee, reading published Psi alone.
//! The caller and callee are named by package and path, so a same-named machine in another
//! package cannot stand in for the callee the Build-admitted alias selected.
use omega_psi::{BlockTerminator, CallTarget, LoweredMachine, LoweredPackage, OperationKind, OperationResult, ReturnEdge, ScalarLiteral};

/// The machine whose result a caller returns, and the constant that machine itself returns.
#[derive(Debug, Eq, PartialEq)]
pub struct ReturnedCall<'a> {
    pub package: &'a str,
    pub path: &'a str,
    pub arguments: usize,
    pub returns: Option<&'a ScalarLiteral>,
}

fn owned<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    path: &str,
) -> Option<&'a LoweredMachine> {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.package == owner && machine.declaration.path == path)
}

/// The constant a single-block machine returns, if its returned value is a constant.
fn returned_constant(machine: &LoweredMachine) -> Option<&ScalarLiteral> {
    let [block] = &machine.body.blocks[..] else { return None };
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = block.terminator
    else {
        return None;
    };
    machine
        .body
        .operations
        .iter()
        .find(|op| op.result == OperationResult::Scalar(returned))
        .and_then(|op| match &op.kind {
            OperationKind::Constant(value) => Some(value),
            _ => None,
        })
}

/// The direct call whose result `owner`'s `path` returns from its only block.
pub fn returned_call<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    path: &str,
) -> Result<ReturnedCall<'a>, String> {
    let caller = owned(package, owner, path).ok_or_else(|| format!("no machine {owner}:{path}"))?;
    let [block] = &caller.body.blocks[..] else {
        return Err(format!("{owner}:{path} has {} blocks", caller.body.blocks.len()));
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Scalar(returned),
        ..
    } = block.terminator
    else {
        return Err(format!("{owner}:{path} does not return a scalar"));
    };
    let operation = caller
        .body
        .operations
        .iter()
        .find(|op| op.result == OperationResult::Scalar(returned))
        .ok_or_else(|| format!("no operation produces the value {owner}:{path} returns"))?;
    let OperationKind::Call {
        callee: CallTarget::Machine(id),
        arguments,
        ..
    } = &operation.kind
    else {
        return Err(format!("{owner}:{path} returns {:?}, not a direct machine call", operation.kind));
    };
    let callee = package
        .machines
        .iter()
        .find(|machine| machine.id == *id)
        .ok_or_else(|| format!("call target {id:?} names no packaged machine"))?;
    Ok(ReturnedCall {
        package: &callee.declaration.package,
        path: &callee.declaration.path,
        arguments: arguments.len(),
        returns: returned_constant(callee),
    })
}
