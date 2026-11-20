use omega_psi::{EmbeddedLoanOrigin, LoweredPackage, OwnedCustody, Parameter, PlaceProjection, ReferenceAccess, TypeShape};

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
                || loan.origin
                    != (EmbeddedLoanOrigin {
                        reborrows: Vec::new(),
                        formal,
                        path,
                    })
            {
                return Err("wrong embedded loan");
            }
        }
    }
    Ok(())
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
        || loan.origin
            != (EmbeddedLoanOrigin {
                reborrows: Vec::new(),
                formal: *formal,
                path: vec![],
            })
    {
        return Err("wrong initializer origin");
    }
    Ok(())
}
