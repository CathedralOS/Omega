//! Read count predicates owed by a closed constructor body and a zero-initialized record field.
use omega_psi::{
    BlockTerminator, ContractClause, DeclarationId, LoweredData, LoweredMachine, LoweredPackage, OperationKind, OperationResult, PlaceOrigin,
    PredicateOperator, PredicateStep, ReturnEdge, ScalarLiteral, ScalarType, TypeId, TypeShape, ValueOperand, ValueOrigin,
};

#[derive(Debug, PartialEq, Eq)]
pub struct ZeroCountClause<'a> {
    pub parent: Option<&'a DeclarationId>,
    pub field: &'a DeclarationId,
    pub comparison: PredicateOperator,
    pub limit: u64,
}

fn count_clause<'a>(
    clause: &'a ContractClause,
    parent: Option<&'a DeclarationId>,
) -> Option<ZeroCountClause<'a>> {
    let [
        PredicateStep::SelfValue,
        PredicateStep::Field(field),
        PredicateStep::Integer(limit),
        PredicateStep::Apply(comparison),
    ] = clause.predicate.as_slice()
    else {
        return None;
    };
    Some(ZeroCountClause {
        parent,
        field,
        comparison: *comparison,
        limit: limit.try_into().ok()?,
    })
}

fn record(
    package: &LoweredPackage,
    row: TypeId,
) -> Option<&LoweredData> {
    let TypeShape::Nominal(id) = package.types.get(row)? else { return None };
    package
        .data
        .get(id.get() - 1)
        .filter(|data| data.id == *id && data.cases.is_empty())
}

/// The count predicates the returned record owes where the body wrote a zero
/// count, then those of each record field the body zeroed whole.
pub fn returned_counts<'a>(
    package: &'a LoweredPackage,
    machine: &'a LoweredMachine,
) -> Option<Vec<ZeroCountClause<'a>>> {
    let body = &machine.body;
    let [block] = body.blocks.as_slice() else { return None };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, .. },
        ..
    } = &block.terminator
    else {
        return None;
    };
    let PlaceOrigin::Operation(established) = body.places.get(source.index())?.origin else {
        return None;
    };
    let OperationKind::EstablishRecord { fields } = &body.operations.get(established.index())?.kind else {
        return None;
    };
    let returned = record(package, body.places[source.index()].ty)?;
    if fields.len() != returned.fields.len() {
        return None;
    }
    let unsigned = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 64 });
    let mut clauses = Vec::new();
    for clause in &returned.requires {
        let clause = count_clause(clause, None)?;
        let position = returned
            .fields
            .iter()
            .position(|field| &field.declaration == clause.field)?;
        // The operand is a zero constant of the count's carrier.
        let ValueOperand::Scalar(value) = fields[position].operand else { return None };
        let value = body.values.get(value.index())?;
        let ValueOrigin::Operation(constant) = value.origin else { return None };
        let zero = matches!(&body.operations.get(constant.index())?.kind, OperationKind::Constant(ScalarLiteral::Integer(n)) if n == &0.into());
        if fields[position].field != *clause.field || package.types.get(value.ty) != Some(&unsigned) || value.ty != returned.fields[position].carrier || !zero {
            return None;
        }
        clauses.push(clause);
    }
    for (initializer, field) in fields.iter().zip(&returned.fields) {
        let ValueOperand::MoveWhole(place) = initializer.operand else { continue };
        let place = body.places.get(place.index())?;
        let PlaceOrigin::Operation(producer) = place.origin else { continue };
        let producer = body.operations.get(producer.index())?;
        if producer.kind != OperationKind::EstablishZeroedStructural || !matches!(producer.result, OperationResult::Structural(_)) {
            continue;
        }
        let data = record(package, place.ty)?;
        if initializer.field != field.declaration || place.ty != field.carrier {
            return None;
        }
        for clause in &data.requires {
            let clause = count_clause(clause, Some(&field.declaration))?;
            let count = data
                .fields
                .iter()
                .find(|field| &field.declaration == clause.field)?;
            if package.types.get(count.carrier) != Some(&unsigned) {
                return None;
            }
            clauses.push(clause);
        }
    }
    Some(clauses)
}
