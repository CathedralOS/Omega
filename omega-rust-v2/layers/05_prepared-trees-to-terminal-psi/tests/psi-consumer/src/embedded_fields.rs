use omega_psi::{EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredPackage, OwnedCustody, Parameter, PlaceProjection, ReferenceAccess, TypeShape};

/// Follow the declared fields of each incoming owner independently of the suspension rows.
pub fn check_incoming_fields(
    package: &LoweredPackage,
    paths: &[&[&str]],
) -> Result<(), &'static str> {
    let body = &super::machine(package, "forward").ok_or("missing caller")?.body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one crossing");
    };
    if site.retained_owners.len() != body.parameters.len() || !site.retained_loans.is_empty() {
        return Err("unexpected custody");
    }
    for parameter in &body.parameters {
        let Parameter::Structural(formal) = *parameter else {
            return Err("expected an owned record");
        };
        let mut owners = site
            .retained_owners
            .iter()
            .filter(|owner| owner.place == formal);
        let owner = owners.next().ok_or("missing owner")?;
        if owners.next().is_some()
            || owner.custody != OwnedCustody::Caller
            || !owner.moved.is_empty()
            || owner.embedded_loans.len() != paths.len()
            || owner.ty != body.places[formal.index()].ty
        {
            return Err("wrong owner or field count");
        }
        for (names, loan) in paths.iter().zip(&owner.embedded_loans) {
            let mut ty = owner.ty;
            let mut path = Vec::new();
            for &name in *names {
                let Some(TypeShape::Nominal(id)) = package.types.get(ty) else {
                    return Err("expected record");
                };
                let field = package.data[id.get() - 1]
                    .fields
                    .iter()
                    .find(|field| field.declaration.path == name)
                    .ok_or("missing field")?;
                path.push(PlaceProjection::Field(field.declaration.clone()));
                ty = field.carrier;
            }
            if !matches!(
                package.types.get(ty),
                Some(TypeShape::ScalarReference {
                    access: ReferenceAccess::Shared,
                    ..
                })
            ) || loan.path != path
                || loan.ty != ty
                || loan.access != ReferenceAccess::Shared
                || loan.origins
                    != [EmbeddedLoanOrigin {
                        arrival: Vec::new(),
                        reborrows: Vec::new(),
                        source: EmbeddedLoanSource::Formal { formal, path },
                    }]
            {
                return Err("wrong embedded loan");
            }
        }
    }
    Ok(())
}

/// Read the loan in the *destination* of the whole store, not the temporary
/// moved into it. Compare the origin separately with the authored formal.
pub fn parked_replacement_origin(package: &LoweredPackage) -> Option<&EmbeddedLoanOrigin> {
    use omega_psi::{CallSite, CallTarget, OperationKind, OperationResult};
    let body = &super::machine(package, "forward")?.body;
    let [_, rhs, store, call] = body.operations.as_slice() else { return None };
    let OperationKind::StructuralStore { destination, value, .. } = &store.kind else {
        return None;
    };
    let [site] = body.suspension_sites.as_slice() else { return None };
    let [owner] = site.retained_owners.as_slice() else { return None };
    let [loan] = owner.embedded_loans.as_slice() else { return None };
    let [origin] = loan.origins.as_slice() else { return None };
    let holder = package.data.iter().find(|d| d.declaration.path == "Holder")?;
    let [field] = holder.fields.as_slice() else { return None };
    let wait = CallTarget::Machine(super::machine(package, "wait")?.id);
    if rhs.result != OperationResult::Structural(*value)
        || !destination.path.is_empty()
        || body.mutable_homes != [destination.root]
        || owner.place != destination.root
        || owner.ty != holder.carrier
        || !matches!(&call.kind, OperationKind::Call { callee, arguments, .. } if *callee == wait && arguments.is_empty())
        || (site.call, site.callee) != (CallSite::Operation(call.id), wait)
        || !site.retained_loans.is_empty()
        || !owner.moved.is_empty()
        || owner.custody != OwnedCustody::Caller
        || field.declaration.path != "Holder::r"
        || loan.path != [PlaceProjection::Field(field.declaration.clone())]
        || loan.ty != field.carrier
        || loan.access != ReferenceAccess::Shared
    {
        return None;
    }
    Some(origin)
}

/// Read a constructed field's actual operand and compare it with the parked occurrence.
pub fn check_constructed_field(
    package: &LoweredPackage,
    selected_slot: usize,
) -> Result<(), &'static str> {
    use omega_psi::{OperationKind, OperationResult, ValueOperand};
    let body = &super::machine(package, "forward").ok_or("missing caller")?.body;
    let Some(Parameter::Structural(formal)) = body.parameters.get(selected_slot) else {
        return Err("missing input descriptor");
    };
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one crossing");
    };
    let [owner] = site.retained_owners.as_slice() else {
        return Err("expected one retained record");
    };
    if owner.custody != OwnedCustody::Caller || !owner.moved.is_empty() {
        return Err("wrong record custody");
    }
    let op = body
        .operations
        .iter()
        .find(|op| op.result == OperationResult::Structural(owner.place))
        .ok_or("missing record definition")?;
    let OperationKind::EstablishRecord { fields } = &op.kind else {
        return Err("expected constructed record");
    };
    let [field] = fields.as_slice() else { return Err("expected one initializer") };
    if field.field.path != "Holder::r" || field.operand != ValueOperand::CopyWhole(*formal) {
        return Err("wrong initializer");
    }
    let [loan] = owner.embedded_loans.as_slice() else {
        return Err("missing stored descriptor");
    };
    if loan.path != [PlaceProjection::Field(field.field.clone())]
        || loan.ty != body.places[formal.index()].ty
        || loan.access != ReferenceAccess::Shared
        || loan.origins
            != [EmbeddedLoanOrigin {
                arrival: Vec::new(),
                reborrows: Vec::new(),
                source: EmbeddedLoanSource::Formal { formal: *formal, path: vec![] },
            }]
    {
        return Err("wrong initializer origin");
    }
    Ok(())
}

/// Read how a machine's single normal return ends the loans stored in its one
/// owned record. Discarding a loan carrier ends only that loan, so the record
/// must be discarded whole and no cleanup machine may run for any referent.
/// Returns the access of each stored loan field in declaration order.
pub fn discarded_loan_fields<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> Result<Vec<(&'a str, ReferenceAccess)>, &'static str> {
    use omega_psi::{BlockTerminator, CleanupAction};
    let body = &super::machine(package, name).ok_or("missing machine")?.body;
    let [Parameter::Structural(formal)] = body.parameters[..] else {
        return Err("expected one owned record");
    };
    let mut returns = body.blocks.iter().filter_map(|block| match &block.terminator {
        BlockTerminator::Return { cleanup, .. } => Some(cleanup.as_slice()),
        _ => None,
    });
    let (Some(cleanup), None) = (returns.next(), returns.next()) else {
        return Err("expected one normal return");
    };
    if cleanup != [CleanupAction::DiscardRoot(formal)] {
        return Err("the record is not discarded whole");
    }
    let invokes = |actions: &[CleanupAction]| {
        actions
            .iter()
            .any(|action| matches!(action, CleanupAction::Invoke { .. }))
    };
    if body.blocks.iter().any(|block| match &block.terminator {
        BlockTerminator::Return { cleanup, .. } | BlockTerminator::TerminalCall { cleanup, .. } => invokes(cleanup),
        BlockTerminator::Jump(edge) => invokes(&edge.cleanup),
        BlockTerminator::Conditional { when_true, when_false, .. } => invokes(&when_true.cleanup) || invokes(&when_false.cleanup),
        BlockTerminator::ExternalExit(_) => false,
    }) {
        return Err("a referent's cleanup machine runs");
    }
    let Some(TypeShape::Nominal(id)) = package.types.get(body.places[formal.index()].ty) else {
        return Err("expected record");
    };
    let mut loans = Vec::new();
    for field in &package.data[id.get() - 1].fields {
        let access = match package.types.get(field.carrier) {
            Some(TypeShape::ScalarReference { access, .. } | TypeShape::NominalReference { access, .. }) => *access,
            Some(TypeShape::MutableElementView { .. } | TypeShape::MutableScalarElementView { .. }) => ReferenceAccess::Mutable,
            Some(TypeShape::SharedElementView { .. } | TypeShape::SharedByteView) => ReferenceAccess::Shared,
            _ => continue,
        };
        loans.push((field.declaration.path.as_str(), access));
    }
    Ok(loans)
}
