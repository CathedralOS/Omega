//! Zero establishment for omitted fields and local bindings.
//!
//! An omitted field receives the zero of its declared type
//! (`wiki/spec/language/data_and_literals.md#explicit-field-omission`). Nothing
//! about that zero is asserted here; each form is an operation the checker
//! judges:
//!
//! - A Boolean, integer or float is a zero constant of its carrier, so the
//!   record that receives it still owes its predicates over a visible operand.
//! - A record or fixed array whose every leaf is a scalar is one
//!   `EstablishZeroedStructural` of the whole type, whatever its extent
//!   (`source-rust/subsystems/compiler/psi/artifact/wire/encoding.md`, operation 94). That operation owes
//!   the predicates of the result's schema and of every schema inside it.
//! - Any other record is `EstablishRecord` over the zeros of its fields, a sum
//!   is `EstablishCase` of its first declared case over the zeros of that
//!   payload, and any other fixed array establishes one zero per element. Each
//!   nominal establishment owes its own schema's `requires`, as `Type { .. }`
//!   would. Operation 94 does not list sums, so a sum is never zeroed whole.
//!
//! `..` supplies no borrowed reference, callable or authority, so a type whose
//! zero would need one has no zero here. Only the active payload counts: a
//! reference in a case other than the first never stops the sum's zero.
use super::super::expressions::{EmitOperation, Expressions, OperationOutput};
use super::super::references::zero_discardable_record;
use crate::{
    DeclarationId, FieldInitializer, IeeeFloatBits, IeeeFloatFormat, LoweredData, LoweredField, LoweringError, LoweringTypes, MachineBody, Multiplicity,
    OperationKind, OperationResult, ScalarLiteral, ScalarType, Site, TypeId, TypeShape,
};
use std::collections::{BTreeMap, BTreeSet};

/// Why a type has no zero that lowering can establish.
pub(super) enum Stop {
    /// The zero would hold a reference, and `..` never supplies a borrow.
    Reference,
    /// The zero would hold a callable descriptor, and `..` never supplies one.
    Callable,
    /// The zero may be valid, but this lowering has no form that establishes it.
    Blocked(&'static str),
    /// The element-by-element zero needs more operations than a `usize` counts.
    Host,
}

/// A refusal and the field of the omitted type that caused it. `None` means
/// the omitted field's own type stopped, not a field below it.
pub(super) struct Refusal<'a> {
    pub(super) stop: Stop,
    pub(super) below: Option<&'a DeclarationId>,
}

/// What `establish` appends to the machine body for one omitted field.
#[derive(Clone, Copy)]
pub(super) struct Roster {
    operations: usize,
    places: usize,
    values: usize,
}

enum Form<'a> {
    Scalar(ScalarLiteral),
    /// Every leaf is a scalar: one operation zeroes the whole type.
    Zeroed,
    /// A stored-length sequence established with no elements.
    EmptySequence,
    /// One completed place per element.
    Array {
        element: TypeId,
        length: u64,
    },
    /// The record's fields, or the first declared case and its payload.
    Data {
        case: Option<&'a DeclarationId>,
        fields: &'a [LoweredField],
    },
}

/// An all-zero representation alone cannot establish custody or select a cleanup.
fn custody(
    schema: &LoweredData,
    ty: TypeId,
) -> Result<(), Stop> {
    if schema.carrier != ty {
        return Err(Stop::Blocked("its data schema names a different carrier"));
    }
    if schema.multiplicity == Multiplicity::Linear {
        return Err(Stop::Blocked("a linear value needs its claim established"));
    }
    if schema.cleanup_target.is_some() {
        return Err(Stop::Blocked("a value with authored cleanup needs custody that zero storage does not supply"));
    }
    Ok(())
}

/// Whether one `EstablishZeroedStructural` zeroes this whole type: fixed
/// arrays, at any depth and extent, of scalars or of the plain scalar-leaf
/// records a zero-initialized local already admits.
fn zeroed_whole(
    types: &LoweringTypes,
    data: &[LoweredData],
    mut ty: TypeId,
) -> bool {
    loop {
        match types.get(ty) {
            Some(&TypeShape::FixedArray { element, .. }) => ty = element,
            Some(TypeShape::PrimitiveScalar(_)) => return true,
            Some(TypeShape::Nominal(id)) => return custody(&data[id.get() - 1], ty).is_ok() && zero_discardable_record(types, data, *id),
            _ => return false,
        }
    }
}

/// The one operation that establishes the zero of `ty`, given its components.
fn form<'a>(
    types: &LoweringTypes,
    data: &'a [LoweredData],
    ty: TypeId,
) -> Result<Form<'a>, Stop> {
    match types.get(ty) {
        Some(TypeShape::PrimitiveScalar(ScalarType::Boolean)) => Ok(Form::Scalar(ScalarLiteral::Boolean(false))),
        Some(TypeShape::PrimitiveScalar(ScalarType::Integer { width: 1.., .. })) => Ok(Form::Scalar(ScalarLiteral::Integer(0.into()))),
        // All-zero bits are positive zero in both formats.
        Some(TypeShape::PrimitiveScalar(ScalarType::IeeeFloat(format))) => Ok(Form::Scalar(ScalarLiteral::IeeeFloat(match format {
            IeeeFloatFormat::Binary32 => IeeeFloatBits::Binary32(0),
            IeeeFloatFormat::Binary64 => IeeeFloatBits::Binary64(0),
        }))),
        Some(&TypeShape::FixedArray { element, length }) => {
            if zeroed_whole(types, data, element) {
                return Ok(Form::Zeroed);
            }
            // An authored literal cannot assemble an empty array of owned
            // elements either; there is no ordinal for custody to continue under.
            if length == 0 && super::super::ownership::owned(element, types, data) {
                return Err(Stop::Blocked("an empty array of owned elements has no construction form"));
            }
            Ok(Form::Array { element, length })
        }
        Some(TypeShape::Nominal(id)) => {
            let schema = &data[id.get() - 1];
            custody(schema, ty)?;
            if zero_discardable_record(types, data, *id) {
                return Ok(Form::Zeroed);
            }
            Ok(match schema.cases.first() {
                Some(first) => Form::Data {
                    case: Some(&first.declaration),
                    fields: &first.fields,
                },
                None => Form::Data {
                    case: None,
                    fields: &schema.fields,
                },
            })
        }
        Some(
            TypeShape::ScalarReference { .. }
            | TypeShape::SharedQualifiedScalarReference { .. }
            | TypeShape::NominalReference { .. }
            | TypeShape::IndexedSharedReference(_),
        ) => Err(Stop::Reference),
        Some(TypeShape::Callable(_)) => Err(Stop::Callable),
        Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. } | TypeShape::MutableElementView { .. }) => {
            Err(Stop::Blocked("an empty borrowed view has no establishing operation"))
        }
        Some(TypeShape::ElementSequence { .. }) => Ok(Form::EmptySequence),
        Some(TypeShape::QualifiedScalar { .. } | TypeShape::QualifiedNominal { .. }) => {
            Err(Stop::Blocked("a domain-qualified zero needs its membership established"))
        }
        Some(TypeShape::PrimitiveScalar(_)) => Err(Stop::Blocked("this scalar has no zero literal form")),
        None => Err(Stop::Blocked("its type has no lowered shape")),
    }
}

/// Decide, before any operation is emitted, that `establish` can build the
/// whole zero of `ty`, and count what it will append. Every field of every
/// active payload is visited once per type, since a stop anywhere below leaves
/// the omitted field without a value. Counts multiply through array extents
/// with checked arithmetic; nothing here bounds a source extent.
pub(super) fn admit<'a>(
    types: &LoweringTypes,
    data: &'a [LoweredData],
    root: TypeId,
) -> Result<Roster, Refusal<'a>> {
    // Operations, places and values, per type. An entry is a type, the field
    // that holds it, and whether its components have been counted.
    let mut counted: BTreeMap<TypeId, [u64; 3]> = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut pending: Vec<(TypeId, Option<&'a DeclarationId>, bool)> = vec![(root, None, false)];
    while let Some((ty, below, components_counted)) = pending.pop() {
        if !components_counted && counted.contains_key(&ty) {
            continue;
        }
        let refuse = |stop| Refusal { stop, below };
        let form = form(types, data, ty).map_err(refuse)?;
        if !components_counted && matches!(form, Form::Array { .. } | Form::Data { .. }) {
            if !active.insert(ty) {
                return Err(refuse(Stop::Blocked("its type contains itself by value")));
            }
            pending.push((ty, below, true));
            match form {
                Form::Array { element, .. } => pending.push((element, below, false)),
                // Reversed, so the first declared field that stops is the one reported.
                Form::Data { fields, .. } => pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|field| (field.carrier, Some(&field.declaration), false)),
                ),
                _ => unreachable!("only arrays and data have components"),
            }
            continue;
        }
        let total = match form {
            Form::Scalar(_) => Some([1, 0, 1]),
            Form::Zeroed | Form::EmptySequence => Some([1, 1, 0]),
            Form::Array { element, length } => {
                let [operations, places, values] = counted[&element];
                (|| {
                    Some([
                        operations.checked_mul(length)?.checked_add(1)?,
                        places.checked_mul(length)?.checked_add(1)?,
                        values.checked_mul(length)?,
                    ])
                })()
            }
            Form::Data { fields, .. } => fields.iter().try_fold([1u64, 1, 0], |sum, field| {
                let each = counted[&field.carrier];
                Some([
                    sum[0].checked_add(each[0])?,
                    sum[1].checked_add(each[1])?,
                    sum[2].checked_add(each[2])?,
                ])
            }),
        };
        active.remove(&ty);
        counted.insert(ty, total.ok_or_else(|| refuse(Stop::Host))?);
    }
    let size = |count: u64| usize::try_from(count).map_err(|_| Refusal { stop: Stop::Host, below: None });
    let [operations, places, values] = counted[&root];
    Ok(Roster {
        operations: size(operations)?,
        places: size(places)?,
        values: size(values)?,
    })
}

enum Step<'a> {
    Enter(TypeId),
    /// Establish the next element, or assemble the array once none remain.
    Array {
        carrier: TypeId,
        element: TypeId,
        remaining: usize,
        length: usize,
    },
    Data {
        carrier: TypeId,
        case: Option<&'a DeclarationId>,
        fields: &'a [LoweredField],
    },
}

/// Emit the zero of an admitted type for omitted `field` and return its
/// result. Components are established before the operation that consumes
/// them, in declaration and element order. Every operation reports `site`, the
/// literal whose `..` requested this field. The body's storage for the whole
/// roster is reserved first, so a zero the host cannot hold is a refusal
/// rather than an aborted compiler.
pub(super) fn establish(
    expressions: &Expressions<'_>,
    body: &mut MachineBody,
    field: Option<&DeclarationId>,
    carrier: TypeId,
    roster: Roster,
    site: &Site,
) -> Result<OperationResult, LoweringError> {
    let (types, data) = (expressions.types, expressions.data);
    let unadmitted = || LoweringError::new(site, "zero was not admitted before its establishment");
    let exhausted = || {
        let subject = field.map_or_else(|| "local".to_owned(), |f| format!("omitted field `{}`", f.path));
        LoweringError::new(site, format!("the zero of {subject} needs {} operations, more than this host can allocate", roster.operations))
    };
    body.operations
        .try_reserve(roster.operations)
        .and_then(|()| body.diagnostics.operations.try_reserve(roster.operations))
        .and_then(|()| body.places.try_reserve(roster.places))
        .and_then(|()| body.values.try_reserve(roster.values))
        .map_err(|_| exhausted())?;
    let mut work = vec![Step::Enter(carrier)];
    let mut values: Vec<OperationResult> = Vec::new();
    while let Some(step) = work.pop() {
        let (output, kind) = match step {
            Step::Enter(ty) => match form(types, data, ty).map_err(|_| unadmitted())? {
                Form::Scalar(literal) => (OperationOutput::Scalar(ty), OperationKind::Constant(literal)),
                Form::Zeroed => (OperationOutput::Structural(ty), OperationKind::EstablishZeroedStructural),
                Form::EmptySequence => (OperationOutput::Structural(ty), OperationKind::EstablishStructuralArray { elements: vec![] }),
                Form::Array { element, length } => {
                    let length = usize::try_from(length).map_err(|_| unadmitted())?;
                    work.push(Step::Array {
                        carrier: ty,
                        element,
                        remaining: length,
                        length,
                    });
                    continue;
                }
                Form::Data { case, fields } => {
                    work.push(Step::Data { carrier: ty, case, fields });
                    work.extend(fields.iter().rev().map(|field| Step::Enter(field.carrier)));
                    continue;
                }
            },
            // Each structural element is its own completed place; an owned
            // element transfers once.
            Step::Array {
                carrier,
                element,
                remaining,
                length,
            } if remaining > 0 => {
                work.push(Step::Array {
                    carrier,
                    element,
                    remaining: remaining - 1,
                    length,
                });
                work.push(Step::Enter(element));
                continue;
            }
            Step::Array { carrier, length, .. } => {
                let mut elements = Vec::new();
                elements.try_reserve_exact(length).map_err(|_| exhausted())?;
                for element in values.drain(values.len() - length..) {
                    let OperationResult::Structural(place) = element else {
                        return Err(unadmitted());
                    };
                    elements.push(place);
                }
                (OperationOutput::Structural(carrier), OperationKind::EstablishStructuralArray { elements })
            }
            Step::Data { carrier, case, fields } => {
                let operands: Vec<_> = values.drain(values.len() - fields.len()..).collect();
                let fields = fields
                    .iter()
                    .zip(operands)
                    .map(|(field, value)| {
                        Ok(FieldInitializer {
                            field: field.declaration.clone(),
                            operand: expressions.value_argument(value, body, site)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let kind = match case {
                    Some(case) => OperationKind::EstablishCase { case: case.clone(), fields },
                    None => OperationKind::EstablishRecord { fields },
                };
                (OperationOutput::Structural(carrier), kind)
            }
        };
        values.try_reserve(1).map_err(|_| exhausted())?;
        values.push(body.push_operation(output, kind, site.clone()));
    }
    match values.as_slice() {
        [value] => Ok(*value),
        _ => Err(unadmitted()),
    }
}
