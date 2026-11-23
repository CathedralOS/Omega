//! Recover the actual allocation lent through Outer::link from published Psi.
use omega_psi::{
    CallSite, EmbeddedLoanOrigin, EmbeddedLoanSource, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceOrigin, PlaceProjection, ReferenceAccess,
    ValueOperand,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Backing {
    /// Ordinal among the record constructions of this backing's type.
    Local(usize),
    Formal(usize),
}

pub fn local_backing(package: &LoweredPackage) -> Result<Backing, &'static str> {
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .ok_or("missing caller")?
        .body;
    let [site] = body.suspension_sites.as_slice() else {
        return Err("expected one parked call");
    };
    let CallSite::Operation(call) = site.call else {
        return Err("missing call operation");
    };
    let OperationKind::Call { arguments, .. } = &body.operations[call.index()].kind else {
        return Err("wrong call");
    };
    let [
        ValueOperand::Borrow {
            place,
            access: ReferenceAccess::Shared,
        },
    ] = arguments.as_slice()
    else {
        return Err("wrong call argument");
    };
    let [
        PlaceProjection::Field(link),
        PlaceProjection::Referent,
        PlaceProjection::Field(key),
    ] = place.path.as_slice()
    else {
        return Err("wrong reborrow path");
    };
    if link.path != "Outer::link" || key.path != "Inner::key" {
        return Err("wrong selected fields");
    }
    let PlaceOrigin::Operation(construct) = body.places[place.root.index()].origin else {
        return Err("missing outer constructor");
    };
    let OperationKind::EstablishRecord { fields } = &body.operations[construct.index()].kind else {
        return Err("wrong outer constructor");
    };
    let Some(ValueOperand::CopyWhole(input)) = fields.iter().find(|f| f.field == *link).map(|f| &f.operand) else {
        return Err("missing stored descriptor");
    };
    let outer = site
        .retained_homes
        .iter()
        .find(|h| h.place == place.root)
        .ok_or("missing outer home")?;
    if !outer.needed_by_callee || outer.needed_after_completion || !site.retained_owners.is_empty() {
        return Err("wrong custody");
    }
    let [loan] = outer.embedded_loans.as_slice() else {
        return Err("expected one stored loan");
    };
    if loan.ty != body.places[input.index()].ty || loan.access != ReferenceAccess::Shared || loan.path != [PlaceProjection::Field(link.clone())] {
        return Err("wrong stored loan");
    }
    let [EmbeddedLoanOrigin { arrival, source, reborrows }] = loan.origins.as_slice() else {
        return Err("wrong origin count");
    };
    if !arrival.is_empty() || !reborrows.is_empty() {
        return Err("wrong lineage");
    }
    match body.places[input.index()].origin {
        PlaceOrigin::Parameter => {
            if *source != (EmbeddedLoanSource::Formal { formal: *input, path: vec![] }) || site.retained_homes.len() != 1 {
                return Err("wrong incoming origin");
            }
            let position = body
                .parameters
                .iter()
                .position(|p| *p == Parameter::Structural(*input))
                .ok_or("missing formal")?;
            Ok(Backing::Formal(position))
        }
        PlaceOrigin::Operation(borrow) => {
            if *source != (EmbeddedLoanSource::Borrow { operation: borrow }) {
                return Err("wrong formation");
            }
            let OperationKind::BorrowPlace {
                source: captured,
                access: ReferenceAccess::Shared,
            } = &body.operations[borrow.index()].kind
            else {
                return Err("wrong formation operation");
            };
            if !captured.path.is_empty() {
                return Err("expected whole local backing");
            }
            let ty = body.places[captured.root.index()].ty;
            let home = site
                .retained_homes
                .iter()
                .find(|h| h.place == captured.root)
                .ok_or("missing local backing")?;
            if site.retained_homes.len() != 2 || home.ty != ty || !home.needed_by_callee || home.needed_after_completion || !home.embedded_loans.is_empty() {
                return Err("wrong backing needs");
            }
            let position = body
                .operations
                .iter()
                .filter(|op| {
                    matches!(op.kind, OperationKind::EstablishRecord { .. })
                        && matches!(op.result,OperationResult::Structural(root) if body.places[root.index()].ty == ty)
                })
                .position(|op| op.result == OperationResult::Structural(captured.root))
                .ok_or("missing backing establishment")?;
            Ok(Backing::Local(position))
        }
        _ => Err("unexpected descriptor source"),
    }
}
