//! Read the ordered direct reborrows at ordinary runtime calls from Psi alone.
//! A lifetime-related input identifies how long the result may live. The
//! call operation establishes a distinct result descriptor.
use omega_psi::{
    CallArgument, CallTarget, DeclarationId, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceOrigin, PlaceProjection,
    ReferenceAccess, ScalarType, TemplateAccess, TypeExpression, TypeExpressionStep, TypeShape,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Referent<'a> {
    Scalar(ScalarType),
    Nominal(&'a DeclarationId),
}

#[derive(Debug, Eq, PartialEq)]
pub struct CallLoan<'a> {
    pub formal: usize,
    pub caller_parameter: usize,
    pub access: ReferenceAccess,
    pub referent: Referent<'a>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeLoans<'a> {
    pub descriptor_parameter: usize,
    pub loans: Vec<CallLoan<'a>>,
    pub result_lifetime_parameter: Option<usize>,
}

fn reference_shape<'a>(
    package: &'a LoweredPackage,
    shape: &TypeShape,
) -> Option<(ReferenceAccess, Referent<'a>)> {
    match shape {
        TypeShape::ScalarReference { access, referent } => Some((*access, Referent::Scalar(*referent))),
        TypeShape::NominalReference { access, referent } => {
            let data = package.data.iter().find(|d| d.id == *referent)?;
            Some((*access, Referent::Nominal(&data.declaration)))
        }
        _ => None,
    }
}

fn declared_reference(ty: &TypeExpression) -> Option<(ReferenceAccess, Referent<'_>)> {
    let [target, TypeExpressionStep::Reference(access)] = ty.0.as_slice() else {
        return None;
    };
    let access = match access {
        TemplateAccess::Shared => ReferenceAccess::Shared,
        TemplateAccess::Mut => ReferenceAccess::Mutable,
        TemplateAccess::Write => ReferenceAccess::WriteOnly,
    };
    let target = match target {
        TypeExpressionStep::Scalar(scalar) => Referent::Scalar(*scalar),
        TypeExpressionStep::Named {
            declaration,
            argument_count: 0,
        } => Referent::Nominal(declaration),
        _ => return None,
    };
    Some((access, target))
}

/// Resolve each runtime call's descriptor contract and compare its ordered
/// operands and result to their actual carriers. Each reborrow here is a
/// direct projection through a caller parameter.
pub fn runtime_loans<'a>(
    package: &'a LoweredPackage,
    machine: &LoweredMachine,
) -> Option<Vec<RuntimeLoans<'a>>> {
    let body = &machine.body;
    let mut calls = Vec::new();
    for operation in &body.operations {
        let OperationKind::Call {
            callee: CallTarget::Runtime { descriptor, contract },
            arguments,
            ..
        } = &operation.kind
        else {
            continue;
        };
        let descriptor_parameter = body
            .parameters
            .iter()
            .position(|p| *p == Parameter::Scalar(*descriptor))?;
        let descriptor_type = body.values.get(descriptor.index())?.ty;
        if machine.parameter_types.get(descriptor_parameter) != Some(&descriptor_type)
            || package.types.get(descriptor_type) != Some(&TypeShape::Callable(*contract))
        {
            return None;
        }
        let interface = package.callable_contracts.get(contract.index())?;
        if arguments.len() != interface.parameters.len() {
            return None;
        }
        let mut loans = Vec::new();
        for (formal, (argument, ty)) in arguments.iter().zip(&interface.parameters).enumerate() {
            match argument {
                CallArgument::Borrow { place, access } if place.path == [PlaceProjection::Referent] => {
                    let caller_parameter = body
                        .parameters
                        .iter()
                        .position(|p| *p == Parameter::Structural(place.root))?;
                    let carrier = body.places.get(place.root.index())?.ty;
                    let (actual_access, referent) = reference_shape(package, package.types.get(carrier)?)?;
                    if machine.parameter_types.get(caller_parameter) != Some(&carrier)
                        || *access != actual_access
                        || declared_reference(ty) != Some((*access, referent.clone()))
                    {
                        return None;
                    }
                    loans.push(CallLoan {
                        formal,
                        caller_parameter,
                        access: *access,
                        referent,
                    });
                }
                CallArgument::Scalar(value) => {
                    let TypeShape::PrimitiveScalar(scalar) = package.types.get(body.values.get(value.index())?.ty)? else {
                        return None;
                    };
                    if ty.0 != [TypeExpressionStep::Scalar(*scalar)] {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        let result_lifetime_parameter = match (operation.result, interface.result.as_ref()) {
            (OperationResult::Scalar(value), Some(ty)) if interface.result_lifetime_input.is_none() => {
                let TypeShape::PrimitiveScalar(scalar) = package.types.get(body.values.get(value.index())?.ty)? else {
                    return None;
                };
                if ty.0 != [TypeExpressionStep::Scalar(*scalar)] {
                    return None;
                }
                None
            }
            (OperationResult::Structural(place), Some(ty)) => {
                let result = body.places.get(place.index())?;
                if result.origin != PlaceOrigin::Operation(operation.id) || declared_reference(ty)? != reference_shape(package, package.types.get(result.ty)?)?
                {
                    return None;
                }
                let input = interface.result_lifetime_input?;
                Some(loans.iter().find(|loan| loan.formal == input)?.caller_parameter)
            }
            _ => return None,
        };
        calls.push(RuntimeLoans {
            descriptor_parameter,
            loans,
            result_lifetime_parameter,
        });
    }
    Some(calls)
}

/// Resolve a parked indirect call to the selected descriptor and its flags at
/// the exact operation/terminal edge. An acknowledgement alone is insufficient.
pub fn parked_runtime(package: &LoweredPackage, machine: &LoweredMachine) -> Option<Vec<(usize, bool, bool)>> {
    use omega_psi::{BlockTerminator, CallSite, CompletionContinuation};
    let body = &machine.body;
    let mut result = Vec::new();
    for crossing in &body.suspension_sites {
        let CallTarget::Runtime { descriptor, contract } = crossing.callee else { continue };
        let terminal = match crossing.call {
            CallSite::Operation(id) if matches!(&body.operations.get(id.index())?.kind, OperationKind::Call {callee,..} if *callee==crossing.callee) => false,
            CallSite::Edge(id) if body.blocks.iter().any(|b| matches!(b.terminator,BlockTerminator::TerminalCall{id: edge, callee,..} if edge==id && callee==crossing.callee)) => true,
            _ => return None,
        };
        if terminal != (crossing.continuation == CompletionContinuation::Return) || !package.callable_contracts.get(contract.index())?.pause.suspend
            || !body.call_acknowledgements.iter().any(|a| a.call==crossing.call && a.effects.suspend) { return None; }
        let row = crossing.retained_callables.iter().find(|r| r.descriptor == descriptor && r.contract == contract && r.needed_by_callee)?;
        if package.types.get(body.values.get(descriptor.index())?.ty) != Some(&TypeShape::Callable(contract)) { return None; }
        result.push((body.parameters.iter().position(|p| *p == Parameter::Scalar(descriptor))?, terminal, row.needed_after_completion));
    }
    Some(result)
}
