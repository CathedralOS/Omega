//! Follow the closed body of a record constructor, operation by operation.
//!
//! The body's type rows erase lifetime arguments. Each formal and the result
//! get their declared type back from the row and the machine's region slots.
//! Every operation must be one this judgment understands: a copy read through
//! a shared formal, the whole move of an owned formal, an operation that forms
//! a zero, and last the establishment of the returned record.
use omega_psi::{
    BlockTerminator, DataId, DeclarationId, FieldInitializer, LoweredData, LoweredField, LoweredMachine, LoweredPackage, MachineBody, OperationKind,
    OperationResult, Parameter, PlaceId, PlaceOrigin, PlaceProjection, PsiOperation, ReferenceAccess, ReturnEdge, ScalarLiteral, ScalarType, Site,
    TemplateAccess, TypeExpression, TypeExpressionStep as Step, TypeId, TypeShape, ValueId, ValueOperand, ValueOrigin,
};

type Check<T> = Result<T, &'static str>;

/// What one operation result holds, until the single operand that consumes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Origin {
    CopySharedFormal(usize),
    MoveFormal(usize),
    /// Exactly the all-zero value of the result's type row.
    Zero,
}

pub(crate) struct Body<'unit> {
    /// Each formal's declared type at the constructor's region.
    pub parameters: Vec<TypeExpression>,
    pub result: TypeExpression,
    pub record: &'unit LoweredData,
    /// The establishing operation's source occurrence.
    pub site: &'unit Site,
    /// The returned record's fields in declaration order.
    pub fields: Vec<(&'unit DeclarationId, Origin)>,
}

fn data(
    unit: &LoweredPackage,
    id: DataId,
) -> Check<&LoweredData> {
    unit.data
        .get(id.get() - 1)
        .filter(|row| row.id == id && unit.types.get(row.carrier) == Some(&TypeShape::Nominal(id)))
        .ok_or("type row names no data declaration of this unit")
}

/// The declared type of a formal or of the result: the record its row names,
/// with the machine region that fills each lifetime binder of that record.
/// Every binder needs exactly one region, and no slot may name another binder.
fn declared_type(
    unit: &LoweredPackage,
    machine: &LoweredMachine,
    row: TypeId,
    slots: impl Iterator<Item = (usize, usize)>,
) -> Check<TypeExpression> {
    let (record, shared) = match unit.types.get(row) {
        Some(TypeShape::Nominal(id)) => (data(unit, *id)?, false),
        Some(TypeShape::NominalReference {
            referent,
            access: ReferenceAccess::Shared,
        }) => (data(unit, *referent)?, true),
        _ => return Err("constructor formal or result needs a record or a shared reference to one"),
    };
    if !record.type_arguments.is_empty() || !record.value_indices.is_empty() {
        return Err("constructor formal or result needs a record with lifetime binders only");
    }
    let mut filled = vec![None; record.lifetime_count];
    for (binder, region) in slots {
        let slot = filled
            .get_mut(binder)
            .ok_or("region slot names no lifetime binder of its record")?;
        if slot.replace(region).is_some() {
            return Err("two regions fill one lifetime binder");
        }
    }
    let mut steps = Vec::with_capacity(record.lifetime_count + 2);
    for region in filled {
        steps.push(Step::GenericParameter {
            owner: machine.declaration.clone(),
            index: region.ok_or("a lifetime binder of a formal or result has no region")?,
        });
    }
    steps.push(Step::Named {
        declaration: record.declaration.clone(),
        argument_count: record.lifetime_count,
    });
    if shared {
        steps.push(Step::Reference(TemplateAccess::Shared));
    }
    Ok(TypeExpression(steps))
}

struct Walk<'unit> {
    unit: &'unit LoweredPackage,
    body: &'unit MachineBody,
    /// What each place holds and nothing has consumed yet.
    places: Vec<Option<Origin>>,
    /// The scalar values that are a zero constant nothing has consumed yet.
    zeros: Vec<bool>,
}

impl<'unit> Walk<'unit> {
    fn place(
        &self,
        index: usize,
        operation: &PsiOperation,
    ) -> Check<(PlaceId, TypeId)> {
        let OperationResult::Structural(place) = operation.result else {
            return Err("constructor operation needs a structural result");
        };
        let row = self
            .body
            .places
            .get(place.index())
            .filter(|row| operation.id.index() == index && row.origin == PlaceOrigin::Operation(operation.id))
            .ok_or("operation result does not belong to its operation")?;
        Ok((place, row.ty))
    }

    fn value(
        &self,
        index: usize,
        operation: &PsiOperation,
    ) -> Check<(ValueId, TypeId)> {
        let OperationResult::Scalar(value) = operation.result else {
            return Err("constructor constant needs a scalar result");
        };
        let row = self
            .body
            .values
            .get(value.index())
            .filter(|row| operation.id.index() == index && row.origin == ValueOrigin::Operation(operation.id))
            .ok_or("operation result does not belong to its operation")?;
        Ok((value, row.ty))
    }

    /// Consume `place`, which must hold a value of row `ty`.
    fn take(
        &mut self,
        place: PlaceId,
        ty: TypeId,
    ) -> Check<Origin> {
        if self.body.places.get(place.index()).map(|row| row.ty) != Some(ty) {
            return Err("constructor field origin has a different carrier");
        }
        self.places
            .get_mut(place.index())
            .and_then(Option::take)
            .ok_or("field reads a missing or spent input")
    }

    /// Consume an operand that must be the zero of row `ty`.
    fn take_zero(
        &mut self,
        operand: &ValueOperand,
        ty: TypeId,
    ) -> Check<()> {
        match operand {
            ValueOperand::Scalar(value) => {
                let known = self.body.values.get(value.index()).map(|row| row.ty) == Some(ty);
                let zero = self
                    .zeros
                    .get_mut(value.index())
                    .ok_or("field reads a missing or spent input")?;
                if known && std::mem::replace(zero, false) {
                    Ok(())
                } else {
                    Err("field reads a missing or spent input")
                }
            }
            ValueOperand::MoveWhole(place) if self.take(*place, ty)? == Origin::Zero => Ok(()),
            _ => Err("zero is formed from an operand that is not a zero"),
        }
    }

    /// A zero payload names each declared field once, in order, with its zero.
    fn zero_payload(
        &mut self,
        written: &[FieldInitializer],
        declared: &[LoweredField],
    ) -> Check<()> {
        if written.len() != declared.len() {
            return Err("zero omits a field of its declaration");
        }
        for (initializer, field) in written.iter().zip(declared) {
            if initializer.field != field.declaration {
                return Err("zero names a field outside its declaration order");
            }
            self.take_zero(&initializer.operand, field.carrier)?;
        }
        Ok(())
    }

    /// Follow one operation before the returned record. `formals` pairs each
    /// formal's entry place and row with whether an operation has used it.
    fn input(
        &mut self,
        index: usize,
        operation: &PsiOperation,
        formals: &mut [(PlaceId, TypeId, bool)],
    ) -> Check<()> {
        let unit = self.unit;
        let mut formal = |root: PlaceId| -> Check<(usize, TypeId)> {
            let position = formals
                .iter()
                .position(|(place, _, _)| *place == root)
                .ok_or("constructor reads a place that is not a formal")?;
            if std::mem::replace(&mut formals[position].2, true) {
                return Err("constructor formal used more than once");
            }
            Ok((position, formals[position].1))
        };
        if let OperationKind::Constant(literal) = &operation.kind {
            let (value, ty) = self.value(index, operation)?;
            let zero = match (literal, unit.types.get(ty)) {
                (ScalarLiteral::Integer(n), Some(TypeShape::PrimitiveScalar(ScalarType::Integer { width: 1.., .. }))) => n == &0.into(),
                (ScalarLiteral::Boolean(false), Some(TypeShape::PrimitiveScalar(ScalarType::Boolean))) => true,
                _ => false,
            };
            if !zero {
                return Err("constructor writes a constant that is not the zero of its carrier");
            }
            self.zeros[value.index()] = true;
            return Ok(());
        }
        let (place, ty) = self.place(index, operation)?;
        let row = unit.types.get(ty);
        let origin = match &operation.kind {
            OperationKind::CopyStructural { source } => {
                let (position, formal_row) = formal(source.root)?;
                let Some(TypeShape::NominalReference {
                    referent,
                    access: ReferenceAccess::Shared,
                }) = unit.types.get(formal_row)
                else {
                    return Err("copy requires a shared reference to the indicated carrier");
                };
                if source.path != [PlaceProjection::Referent] || data(unit, *referent)?.carrier != ty {
                    return Err("copy requires a shared reference to the indicated carrier");
                }
                Origin::CopySharedFormal(position)
            }
            OperationKind::MoveStructural { source } => {
                let (position, formal_row) = formal(source.root)?;
                if !source.path.is_empty() || formal_row != ty || !matches!(row, Some(TypeShape::Nominal(_))) {
                    return Err("whole move requires the indicated owned formal");
                }
                Origin::MoveFormal(position)
            }
            // The all-zero value of its row by definition. The field that
            // receives it decides whether that type has a zero at all.
            OperationKind::EstablishZeroedStructural if matches!(row, Some(TypeShape::Nominal(_) | TypeShape::FixedArray { .. })) => Origin::Zero,
            OperationKind::EstablishCase { case, fields } => {
                let Some(TypeShape::Nominal(id)) = row else {
                    return Err("zero case needs its sum");
                };
                // A sum's zero is its first case with a zero payload.
                let first = data(unit, *id)?
                    .cases
                    .first()
                    .filter(|first| &first.declaration == case)
                    .ok_or("zero selects a case that is not the first of its sum")?;
                self.zero_payload(fields, &first.fields)?;
                Origin::Zero
            }
            OperationKind::EstablishRecord { fields } => {
                let Some(TypeShape::Nominal(id)) = row else {
                    return Err("zero record needs its declaration");
                };
                let record = data(unit, *id)?;
                if !record.cases.is_empty() {
                    return Err("zero record needs a declaration without cases");
                }
                self.zero_payload(fields, &record.fields)?;
                Origin::Zero
            }
            OperationKind::EstablishStructuralArray { elements } => {
                let Some(TypeShape::FixedArray { element, length }) = row else {
                    return Err("zero array needs a fixed extent");
                };
                if u64::try_from(elements.len()) != Ok(*length) {
                    return Err("zero array does not fill its extent");
                }
                for source in elements {
                    if self.take(*source, *element)? != Origin::Zero {
                        return Err("zero is formed from an operand that is not a zero");
                    }
                }
                Origin::Zero
            }
            _ => return Err("unsupported operation in record constructor"),
        };
        self.places[place.index()] = Some(origin);
        Ok(())
    }
}

pub(crate) fn read<'unit>(
    unit: &'unit LoweredPackage,
    machine: &'unit LoweredMachine,
) -> Check<Body<'unit>> {
    let body = &machine.body;
    let count = machine.parameter_types.len();
    let slots = machine
        .regions
        .iter()
        .flat_map(|region| &region.aggregate_parameters);
    if machine
        .regions
        .iter()
        .any(|region| !region.reference_parameters.is_empty() || region.reference_result)
        || slots.clone().any(|slot| slot.parameter >= count)
    {
        return Err("unsupported constructor region slots");
    }
    let parameters = machine
        .parameter_types
        .iter()
        .enumerate()
        .map(|(position, row)| {
            let filled = machine.regions.iter().enumerate().flat_map(|(region, slots)| {
                slots
                    .aggregate_parameters
                    .iter()
                    .filter(move |slot| slot.parameter == position)
                    .map(move |slot| (slot.binder, region))
            });
            declared_type(unit, machine, *row, filled)
        })
        .collect::<Check<Vec<_>>>()?;
    let result_row = machine
        .result_type
        .ok_or("constructor does not return the declared record")?;
    let Some(TypeShape::Nominal(record)) = unit.types.get(result_row) else {
        return Err("constructor does not return the declared record");
    };
    let record = data(unit, *record)?;
    let result = declared_type(
        unit,
        machine,
        result_row,
        machine.regions.iter().enumerate().flat_map(|(region, slots)| {
            slots
                .aggregate_result
                .iter()
                .map(move |binder| (*binder, region))
        }),
    )?;

    // The body and each of its operations come from the declaring package.
    let sites = &body.diagnostics;
    if sites.body.package != machine.declaration.package
        || sites.operations.len() != body.operations.len()
        || sites
            .operations
            .iter()
            .any(|site| site.package != machine.declaration.package)
        || body.parameters.len() != count
    {
        return Err("constructor body does not belong to its declaration");
    }
    let mut formals = Vec::with_capacity(count);
    for (parameter, row) in body.parameters.iter().zip(&machine.parameter_types) {
        let Parameter::Structural(place) = parameter else {
            return Err("constructor formal needs a structural place");
        };
        let entry = body
            .places
            .get(place.index())
            .filter(|entry| entry.origin == PlaceOrigin::Parameter && entry.ty == *row);
        if entry.is_none() || formals.iter().any(|(known, _, _)| known == place) {
            return Err("constructor formal disagrees with its entry place");
        }
        formals.push((*place, *row, false));
    }
    let [block] = body.blocks.as_slice() else {
        return Err("unsupported control flow in record constructor");
    };
    if !block.scalar_parameters.is_empty()
        || !block.structural_parameters.is_empty()
        || !block.scalar_domain_invariants.is_empty()
        || block.operations != (0..body.operations.len())
    {
        return Err("unsupported control flow in record constructor");
    }
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result: returned },
        cleanup,
        ..
    } = &block.terminator
    else {
        return Err("missing returned record construction");
    };
    if !cleanup.is_empty() {
        return Err("constructor leaves implicit work at its return");
    }
    let Some((last, inputs)) = body.operations.split_last() else {
        return Err("missing returned record construction");
    };
    let mut walk = Walk {
        unit,
        body,
        places: vec![None; body.places.len()],
        zeros: vec![false; body.values.len()],
    };
    for (index, operation) in inputs.iter().enumerate() {
        walk.input(index, operation, &mut formals)?;
    }

    let OperationKind::EstablishRecord { fields: written } = &last.kind else {
        return Err("missing returned record construction");
    };
    let (established, row) = walk.place(inputs.len(), last)?;
    let returned = body
        .places
        .get(returned.index())
        .filter(|place| place.origin == PlaceOrigin::Result && place.ty == result_row);
    if row != result_row || *source != established || returned.is_none() || !record.cases.is_empty() {
        return Err("constructor does not return the declared record");
    }
    if written.len() != record.fields.len() {
        return Err("constructor does not establish its complete record declaration");
    }
    let mut fields = Vec::with_capacity(written.len());
    for (initializer, field) in written.iter().zip(&record.fields) {
        if initializer.field != field.declaration {
            return Err("constructor field order disagrees with its record declaration");
        }
        let origin = match &initializer.operand {
            ValueOperand::Scalar(_) => {
                walk.take_zero(&initializer.operand, field.carrier)?;
                Origin::Zero
            }
            // An owned value transfers whole. Only the copy read through the
            // shared formal is unrestricted, so only it may be copied in.
            ValueOperand::MoveWhole(place) => walk.take(*place, field.carrier)?,
            ValueOperand::CopyWhole(place) => match walk.take(*place, field.carrier)? {
                origin @ Origin::CopySharedFormal(_) => origin,
                _ => return Err("constructor copies an owned field input"),
            },
            _ => return Err("unsupported record field operand"),
        };
        fields.push((&field.declaration, origin));
    }
    if walk.places.iter().any(Option::is_some) || walk.zeros.contains(&true) {
        return Err("constructor discards a field input");
    }
    if formals.iter().any(|(_, _, used)| !used) {
        return Err("constructor leaves a formal unaccounted for");
    }
    Ok(Body {
        parameters,
        result,
        record,
        site: &sites.operations[inputs.len()],
        fields,
    })
}
