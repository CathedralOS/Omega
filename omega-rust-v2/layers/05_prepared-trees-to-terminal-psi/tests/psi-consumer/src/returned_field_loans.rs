use omega_psi::{BlockTerminator, DataArgument, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceOrigin, PlaceProjection, ReferenceAccess, ReturnEdge, TypeShape};

/// Where a returned shared reference read out of an incoming record points,
/// recovered from published Psi alone.
#[derive(Debug, Eq, PartialEq)]
pub struct ReturnedStoredLoan {
    /// The incoming formal whose record stores the parent descriptor.
    pub formal: usize,
    /// The declared fields from that formal to the stored descriptor.
    pub fields: Vec<String>,
    /// The referent declaration named by the machine's public result type.
    pub referent: String,
    /// The referent's closed application arguments, empty for a plain record.
    pub arguments: Vec<DataArgument>,
}

/// Follow `name`'s public result type to its one return edge, the shared
/// borrow that defines the returned descriptor, and the field of an incoming
/// formal that stores the parent loan. The borrow must cross that stored
/// descriptor; a borrow of the record's own field would point into the
/// caller's private carrier storage instead of the original backing.
pub fn returned_stored_loan(
    package: &LoweredPackage,
    name: &str,
) -> Result<ReturnedStoredLoan, &'static str> {
    let machine = super::machine(package, name).ok_or("missing machine")?;
    let result = machine.result_type.ok_or("the machine returns no descriptor")?;
    let (referent, arguments) = match package.types.get(result) {
        Some(TypeShape::IndexedSharedReference(id)) => {
            let (declaration, arguments) = package.types.indexed_referent(*id).ok_or("missing indexed referent")?;
            (declaration.path.clone(), arguments.to_vec())
        }
        Some(TypeShape::NominalReference {
            referent,
            access: ReferenceAccess::Shared,
        }) => {
            let schema = package.data.get(referent.get() - 1).ok_or("missing referent schema")?;
            (schema.declaration.path.clone(), schema.type_arguments.clone())
        }
        _ => return Err("the result type must be a shared reference to a record"),
    };
    let body = &machine.body;
    let place_ty = |p: omega_psi::PlaceId| body.places.get(p.index()).map(|place| place.ty);
    let mut returns = body.blocks.iter().filter_map(|block| match &block.terminator {
        BlockTerminator::Return { result, .. } => Some(result),
        _ => None,
    });
    let (Some(ReturnEdge::Structural { source, result: slot }), None) = (returns.next(), returns.next()) else {
        return Err("expected one structural return");
    };
    if body.places.get(slot.index()).map(|place| place.origin) != Some(PlaceOrigin::Result) || place_ty(*slot) != Some(result) || place_ty(*source) != Some(result) {
        return Err("the return must pass a descriptor of the public result type");
    }
    let PlaceOrigin::Operation(definition) = body.places[source.index()].origin else {
        return Err("the returned descriptor must be formed in the body");
    };
    let op = body.operations.get(definition.index()).ok_or("missing defining operation")?;
    let OperationKind::BorrowPlace {
        source: borrowed,
        access: ReferenceAccess::Shared,
    } = &op.kind
    else {
        return Err("the returned descriptor must come from a shared borrow");
    };
    if op.result != OperationResult::Structural(*source) {
        return Err("the borrow must define the returned descriptor");
    }
    let formal = body
        .parameters
        .iter()
        .position(|parameter| *parameter == Parameter::Structural(borrowed.root))
        .ok_or("the borrow must start at an incoming formal")?;
    let [steps @ .., PlaceProjection::Referent] = borrowed.path.as_slice() else {
        return Err("the borrow must cross the stored descriptor, not select the record's own field");
    };
    let mut ty = place_ty(borrowed.root).ok_or("missing formal place")?;
    let mut fields = Vec::with_capacity(steps.len());
    for step in steps {
        let (PlaceProjection::Field(declaration), Some(TypeShape::Nominal(id))) = (step, package.types.get(ty)) else {
            return Err("each step before the crossing must select a field of the reached record");
        };
        let field = package.data[id.get() - 1]
            .fields
            .iter()
            .find(|field| &field.declaration == declaration)
            .ok_or("the record has no such field")?;
        fields.push(declaration.path.clone());
        ty = field.carrier;
    }
    if fields.is_empty() || ty != result {
        return Err("the crossed field must store a descriptor of the public result type");
    }
    Ok(ReturnedStoredLoan { formal, fields, referent, arguments })
}
