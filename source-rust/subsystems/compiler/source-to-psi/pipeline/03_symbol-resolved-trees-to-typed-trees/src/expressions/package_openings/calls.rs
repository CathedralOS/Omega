//! Decide whether an evaluated call can replace the package that lent it state.
use super::*;
use typed::operator::unconstrained_type_reference;
use typed::signature::StateSignature;
use typed::types::TypeReferenceHandle;

impl OpeningScope<'_> {
    /// Passing a stored reference reborrows its referent. Passing the package
    /// or the reference slot itself can replace the opening.
    pub(super) fn reborrowed_opening(
        &self,
        program: &TypedTrees,
        callee: ExpressionHandle,
        arguments: &[ExpressionHandle],
    ) -> Result<Option<PackageOpeningHandle>, Diagnostic> {
        if self.facts.openings.is_empty()
            || !arguments
                .iter()
                .any(|&arg| program.package_projection(arg).is_some())
        {
            return Ok(None);
        }
        let callee_type = crate::types::typing_queries::scoped_expression_type_reference(
            program,
            program.state_parameters(self.state),
            self.state.return_type,
            callee,
            Some(self.state),
        )?;
        let Some(signature) = callee_type.and_then(|ty| closed_call_signature(program, ty)) else {
            return Ok(None);
        };
        let parameters = program.state_signature_parameters(signature);
        if parameters.len() != arguments.len() {
            return Ok(None);
        }
        let mut borrowed = None;
        for (&argument, parameter) in arguments.iter().zip(parameters) {
            if scalar_argument(program, parameter.type_reference) {
                continue;
            }
            let Some(field) = program.package_projection(argument) else {
                return Ok(None);
            };
            if !self.facts.openings.contains(&field.opening)
                || borrowed.is_some_and(|old| old != field.opening)
                || !reborrows_witness(program, field, parameter.type_reference)
            {
                return Ok(None);
            }
            borrowed = Some(field.opening);
        }
        Ok(borrowed)
    }
}

/// Without another entry, external service or suspension, the callable's
/// authority to write storage comes from its arguments.
fn closed_call_signature(program: &TypedTrees, ty: TypeReferenceHandle) -> Option<&StateSignature> {
    let ty = unconstrained_type_reference(program, ty)?;
    let TypeReferenceNode::Callable { contract } = program.type_reference_table.type_reference(ty)
    else {
        return None;
    };
    let signature = &program
        .declared_callable_contract(*contract)
        .ok()?
        .signature;
    let reach = signature.service_reach_row;
    if signature.blocks
        || signature.suspends
        || signature.service_reach_is_installation_bound
        || reach.0 == 0
        || reach.0 as usize > program.service_reach_rows.len()
        || program.service_reach_rows.abstract_bound(reach).is_some()
        || !program.service_reach_rows.services(reach).is_empty()
        || program
            .authored_service_reach_rows_for(signature.symbol)
            .any(|row| !row.targets.is_empty())
        || !program.state_signature_invokes(signature).is_empty()
    {
        return None;
    }
    Some(signature)
}

/// A scalar or a buffer of scalar elements cannot hold a package or its
/// reference slots. The callee can fill an output buffer during a reborrow.
fn scalar_argument(program: &TypedTrees, ty: TypeReferenceHandle) -> bool {
    if program.primitive_type_reference(ty).is_some() {
        return true;
    }
    let types = &program.type_reference_table;
    let Some(ty) = unconstrained_type_reference(program, ty) else {
        return false;
    };
    let TypeReferenceNode::Reference { referee, .. } = types.type_reference(ty) else {
        return false;
    };
    let mut element = *referee;
    for _ in 0..types.type_reference_count() {
        let Some(ty) = unconstrained_type_reference(program, element) else {
            return false;
        };
        match types.type_reference(ty) {
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => element = *element_type,
            _ => return program.primitive_type_reference(ty).is_some(),
        }
    }
    false
}

fn reborrows_witness(
    program: &TypedTrees,
    field: &OpenedPackageField,
    formal: TypeReferenceHandle,
) -> bool {
    let types = &program.type_reference_table;
    let referent = |ty| match types.type_reference(unconstrained_type_reference(program, ty)?) {
        TypeReferenceNode::Reference { referee, .. } => types.unconstrained_named_symbol(*referee),
        _ => None,
    };
    let Some(witness) = referent(field.result_type) else {
        return false;
    };
    let opening = program.package_opening(field.opening);
    referent(formal) == Some(witness)
        && opening.witnesses.iter().any(|&(_, ty)| types.unconstrained_named_symbol(ty) == Some(witness))
        // A scalar argument could still alias a mutable index in this subject.
        && !opening.subject.steps.iter().any(|step| matches!(step, PackageSubjectStep::IndexBinding(_)))
}
