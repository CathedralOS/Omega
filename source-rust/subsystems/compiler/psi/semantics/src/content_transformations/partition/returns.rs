//! Read normal-return interval endpoints from the actual SSA values and record fields.
use super::intervals::{Bounds, Interval};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment as Segment, ContentProjectionScalar, EdgeId,
    IntegerMathTerm as Term, IntegerSign, IntegerValue, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use terminal_psi::{
    Operation, OperationKind as Op, RecordFieldValue, StructuralAccess, StructuralArgument,
    StructuralPathSegment as Path, TerminalMachine, TerminalModule, Terminator,
};

type Checked<T> = Result<T, &'static str>;

#[derive(Clone)]
struct Rows<Id, T>(arena::IdTable<Id, Option<T>>);

impl<Id, T> Default for Rows<Id, T> {
    fn default() -> Self {
        Self(arena::IdTable::default())
    }
}

impl<Id: arena::NumericId, T> Rows<Id, T> {
    fn get(&self, id: &Id) -> Option<&T> {
        self.0.get(*id)?.as_ref()
    }

    fn insert(&mut self, id: Id, value: T) -> Checked<Option<T>> {
        let index = usize::try_from(id.get())
            .ok()
            .filter(|i| *i <= 65_536)
            .ok_or("partition exceeds its dense identity budget")?;
        while self.0.len() < index {
            self.0.push(None);
        }
        Ok(self
            .0
            .get_mut(id)
            .ok_or("missing dense partition identity")?
            .replace(value))
    }
}

#[derive(Clone)]
enum Slot {
    Scalar(Term),
    Record(Record),
}

#[derive(Clone)]
struct Record {
    ty: StructuralTypeId,
    fields: Fields,
}

#[derive(Clone)]
enum Fields {
    Entry(PlaceId, Vec<Segment>),
    Built(PlaceId, Rc<Rows<StructuralFieldId, Slot>>),
}

impl Record {
    fn select(&self, module: &TerminalModule, path: &[Path]) -> Checked<Slot> {
        let mut slot = Slot::Record(self.clone());
        for step in path {
            let (Slot::Record(record), Path::Field(identity)) = (slot, step) else {
                return Err("interval projection requires a named record field");
            };
            let field = module
                .structural_type(record.ty)
                .and_then(|ty| ty.shape.direct_fields())
                .and_then(|fields| fields.iter().find(|field| field.identity == *identity))
                .ok_or("interval projection selects an absent field")?;
            slot = match &record.fields {
                Fields::Built(_, fields) => fields
                    .get(&field.id)
                    .cloned()
                    .ok_or("returned record has no initializer for an interval field")?,
                Fields::Entry(root, path) => {
                    let mut path = path.clone();
                    path.push(Segment::Field(field.id));
                    if let Some(ScalarType::Integer(integer)) = field.field_type.scalar_type() {
                        Slot::Scalar(embed(ScalarTerm::integer_field_path(*root, path, integer))?)
                    } else if let terminal_psi::StructuralFieldType::Structural(ty) =
                        field.field_type
                    {
                        Slot::Record(Record {
                            ty,
                            fields: Fields::Entry(*root, path),
                        })
                    } else {
                        return Err("interval projection selects a noninteger field");
                    }
                }
            };
        }
        Ok(slot)
    }
}

fn embed(term: ScalarTerm) -> Checked<Term> {
    Term::embed(term).ok_or("interval endpoint is not an integer observation")
}

fn checked(term: Term) -> Checked<Term> {
    closed_predicates::check_integer_math_term_size(&term, &term)
        .map_err(|_| "interval endpoint expression exceeds its proof budget")?;
    Ok(term)
}

#[derive(Clone)]
enum Boolean {
    Literal(bool),
    Less(Term, Term),
    AtMost(Term, Term),
    Equal(Term, Term),
    Not(Box<Boolean>),
}

impl Boolean {
    fn known(&self, bounds: &Bounds) -> Checked<Option<bool>> {
        use std::cmp::Ordering;
        Ok(match self {
            Self::Literal(value) => Some(*value),
            Self::Equal(a, b) => bounds.compare(a, b)?.map(|order| order == Ordering::Equal),
            Self::Less(a, b) => bounds.compare(a, b)?.map(|order| order == Ordering::Less),
            Self::AtMost(a, b) => bounds
                .compare(a, b)?
                .map(|order| order != Ordering::Greater),
            Self::Not(inner) => inner.known(bounds)?.map(|value| !value),
        })
    }

    fn assume(&self, value: bool, bounds: &mut Bounds) {
        match (self, value) {
            (Self::Not(inner), value) => inner.assume(!value, bounds),
            (Self::Less(a, b) | Self::AtMost(a, b), true) => bounds.order(a.clone(), b.clone()),
            (Self::Less(a, b) | Self::AtMost(a, b), false) => bounds.order(b.clone(), a.clone()),
            (Self::Equal(a, b), true) => {
                bounds.order(a.clone(), b.clone());
                bounds.order(b.clone(), a.clone());
            }
            _ => {}
        }
    }
}

#[derive(Clone, Default)]
struct Flow {
    values: Rows<ValueId, Term>,
    booleans: Rows<ValueId, Boolean>,
    storage: Rows<PlaceId, Record>,
    bounds: Bounds,
    opened: BTreeSet<PlaceId>,
    // Exact arrivals of constructed children, including the destination of a
    // qualification-changing edge. Construction identity survives owned moves.
    arrivals: BTreeMap<(EdgeId, PlaceId), PlaceId>,
}

impl Flow {
    fn number(&self, id: ValueId) -> Checked<Term> {
        self.values
            .get(&id)
            .cloned()
            .ok_or("interval field has no checked scalar origin")
    }

    fn record(&self, module: &TerminalModule, root: PlaceId, path: &[Path]) -> Checked<Record> {
        let slot = self
            .storage
            .get(&root)
            .ok_or("interval field has no original record")?
            .select(module, path)?;
        match slot {
            Slot::Record(r) => Ok(r),
            _ => Err("interval output path does not name a record"),
        }
    }

    fn argument(&self, module: &TerminalModule, arg: &StructuralArgument) -> Checked<Record> {
        if arg.access != StructuralAccess::Owned {
            return Err("interval partition crosses a borrowed result");
        }
        self.record(module, arg.place, &arg.path)
    }

    fn operation(
        &mut self,
        module: &TerminalModule,
        op: &Operation,
        inputs: &BTreeSet<PlaceId>,
    ) -> Checked<()> {
        let scalar = |flow: &mut Self, value| -> Checked<()> {
            let result = op
                .result
                .as_scalar()
                .ok_or("partition scalar operation has no result")?;
            let value = checked(value)?;
            if matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned)
            {
                flow.bounds.nonnegative(value.clone());
            }
            flow.values.insert(result.id, value)?;
            Ok(())
        };
        let boolean = |flow: &mut Self, value| -> Checked<()> {
            let result = op
                .result
                .as_scalar()
                .ok_or("partition comparison has no result")?;
            flow.booleans.insert(result.id, value)?;
            Ok(())
        };
        match &op.kind {
            Op::DecomposeStructuralValue { subject }
                if inputs.contains(&subject.place) && subject.path.is_empty() && self.opened.insert(subject.place) =>
            {
                Ok(())
            }
            Op::PrimitiveScalarRead { source, path } => {
                let record = self
                    .storage
                    .get(source)
                    .ok_or("partition read has no known source")?;
                let Slot::Scalar(term) = record.select(module, path)? else {
                    return Err("partition read is not scalar");
                };
                scalar(self, term)
            }
            Op::IntegerConstant { value } => scalar(self, Term::literal(*value)),
            Op::ExactIntegerAdd { left, right, .. } => scalar(
                self,
                Term::Add(
                    Box::new(self.number(*left)?),
                    Box::new(self.number(*right)?),
                ),
            ),
            Op::ExactIntegerSubtract { left, right, .. } => scalar(
                self,
                Term::Subtract(
                    Box::new(self.number(*left)?),
                    Box::new(self.number(*right)?),
                ),
            ),
            Op::IntegerLessThan { left, right } => boolean(
                self,
                Boolean::Less(self.number(*left)?, self.number(*right)?),
            ),
            Op::IntegerLessOrEqual { left, right } => boolean(
                self,
                Boolean::AtMost(self.number(*left)?, self.number(*right)?),
            ),
            Op::IntegerEqual { left, right } => boolean(
                self,
                Boolean::Equal(self.number(*left)?, self.number(*right)?),
            ),
            Op::BooleanConstant { value } => boolean(self, Boolean::Literal(*value)),
            Op::BooleanEqual { left, right } => {
                let (Some(a), Some(b)) = (self.booleans.get(left), self.booleans.get(right)) else {
                    return Err("partition branch compares unknown booleans");
                };
                let predicate = match (a, b) {
                    (Boolean::Literal(true), x) | (x, Boolean::Literal(true)) => x.clone(),
                    (Boolean::Literal(false), x) | (x, Boolean::Literal(false)) => match x {
                        Boolean::Not(inner) => (**inner).clone(),
                        other => Boolean::Not(Box::new(other.clone())),
                    },
                    _ => return Err("partition branch needs an established comparison"),
                };
                boolean(self, predicate)
            }
            Op::EstablishRecord { fields } => {
                let out = op
                    .result
                    .as_structural()
                    .ok_or("partition constructor has no record")?;
                let mut initialized = Rows::default();
                for field in fields {
                    let value = match &field.value {
                        RecordFieldValue::Scalar { value, .. } => {
                            Slot::Scalar(self.number(*value)?)
                        }
                        RecordFieldValue::Structural(arg) => {
                            Slot::Record(self.argument(module, arg)?)
                        }
                    };
                    if initialized.insert(field.field, value)?.is_some() {
                        return Err("partition field is initialized twice");
                    }
                }
                self.storage.insert(
                    out.place,
                    Record {
                        ty: out.structural_type,
                        fields: Fields::Built(out.place, Rc::new(initialized)),
                    },
                )?;
                Ok(())
            }
            Op::MoveStructuralValue { source } => {
                let out = op
                    .result
                    .as_structural()
                    .ok_or("partition move has no result")?;
                let record = self.argument(module, source)?;
                self.storage.insert(out.place, record)?;
                Ok(())
            }
            _ => Err("interval partition has an operation with unproved custody or geometry"),
        }
    }

    fn assume_requirement(&mut self, p: &Proposition) -> Checked<()> {
        let (left, right, equal) = match p {
            Proposition::Equal(a, b)
            | Proposition::LessOrEqual(a, b)
            | Proposition::LessThan(a, b) => {
                let (Ok(a), Ok(b)) = (embed(a.clone()), embed(b.clone())) else {
                    return Ok(());
                };
                (a, b, matches!(p, Proposition::Equal(..)))
            }
            Proposition::IntegerMathEqual(a, b)
            | Proposition::IntegerMathLessOrEqual(a, b)
            | Proposition::IntegerMathLessThan(a, b) => {
                closed_predicates::check_integer_math_term_size(a, b)
                    .map_err(|_| "interval assumption exceeds its proof budget")?;
                (
                    a.clone(),
                    b.clone(),
                    matches!(p, Proposition::IntegerMathEqual(..)),
                )
            }
            _ => return Ok(()),
        };
        if equal {
            self.bounds.order(right.clone(), left.clone());
        }
        self.bounds.order(left, right);
        Ok(())
    }

    fn coordinate(
        &mut self,
        module: &TerminalModule,
        record: &Record,
        expr: &ContentProjectionScalar,
        depth: usize,
    ) -> Checked<Term> {
        use ContentProjectionScalar as Expr;
        if depth > 128 {
            return Err("interval projection expression is too deep");
        }
        let next = |flow: &mut Self, x| flow.coordinate(module, record, x, depth + 1).map(Box::new);
        checked(match expr {
            Expr::RuntimeScalarEmbedding(names) => {
                let (_, ty) = crate::domain_preservation::content_scalar_path(
                    module.types.structural_types.iter(),
                    &module.identities,
                    record.ty,
                    names,
                )
                .ok_or("interval owner projection has an unknown scalar field")?;
                let ScalarType::Integer(integer) = ty else {
                    return Err("interval owner projection is not integer-valued");
                };
                if integer.sign() != IntegerSign::Unsigned {
                    return Err("interval owner projection needs an unsigned observation");
                }
                let path = names
                    .iter()
                    .map(|name| {
                        module
                            .identities
                            .index_of(name)
                            .map(Path::Field)
                            .ok_or("interval owner projection has an unknown field identity")
                    })
                    .collect::<Checked<Vec<_>>>()?;
                let Slot::Scalar(term) = record.select(module, &path)? else {
                    return Err("interval owner projection did not read a scalar");
                };
                self.bounds.nonnegative(term.clone());
                term
            }
            Expr::Natural(lit) => Term::literal(IntegerValue::Unsigned(
                lit.parse::<u128>()
                    .map_err(|_| "interval literal exceeds the supported projection budget")?,
            )),
            Expr::Successor(a) => Term::Add(
                next(self, a)?,
                Box::new(Term::literal(IntegerValue::Unsigned(1))),
            ),
            Expr::Add(a, b) => Term::Add(next(self, a)?, next(self, b)?),
            Expr::Subtract(a, b) => Term::Subtract(next(self, a)?, next(self, b)?),
            Expr::Multiply(a, b) => Term::Multiply(next(self, a)?, next(self, b)?),
            Expr::SubjectField(_) => {
                return Err("interval partition requires an evaluated natural field");
            }
        })
    }

    fn interval(
        &mut self,
        module: &TerminalModule,
        record: &Record,
        shape: &(ContentProjectionScalar, ContentProjectionScalar),
    ) -> Checked<Interval> {
        Ok((
            self.coordinate(module, record, &shape.0, 0)?,
            self.coordinate(module, record, &shape.1, 0)?,
        ))
    }
}

/// Check each alternative of a disjunctive precondition separately. Other
/// preconditions may be weakened: every remaining case must prove the law.
fn assume(
    mut flow: Flow,
    p: &Proposition,
    depth: usize,
    remaining: &mut usize,
) -> Checked<Vec<Flow>> {
    if depth > 128 {
        return Err("interval precondition exceeds its nesting budget");
    }
    *remaining = remaining
        .checked_sub(1 + flow.bounds.len() + flow.storage.0.len())
        .ok_or("interval partition exceeds its receiving work budget")?;
    match p {
        Proposition::Falsehood => Ok(Vec::new()),
        Proposition::Disjunction(arms) => {
            let mut cases = Vec::new();
            for arm in arms {
                cases.extend(assume(flow.clone(), arm, depth + 1, remaining)?);
            }
            Ok(cases)
        }
        Proposition::Conjunction(parts) => assume_all(vec![flow], parts, depth + 1, remaining),
        _ => {
            flow.assume_requirement(p)?;
            Ok(vec![flow])
        }
    }
}

fn assume_all(
    mut flows: Vec<Flow>,
    predicates: &[Proposition],
    depth: usize,
    remaining: &mut usize,
) -> Checked<Vec<Flow>> {
    for p in predicates {
        let mut next = Vec::new();
        for flow in flows {
            next.extend(assume(flow, p, depth, remaining)?);
        }
        flows = next;
    }
    Ok(flows)
}

pub(super) fn check(
    module: &TerminalModule,
    machine: &TerminalMachine,
    inputs: &BTreeSet<PlaceId>,
    paths: &[Vec<Path>],
    shape: &(ContentProjectionScalar, ContentProjectionScalar),
) -> Checked<BTreeSet<(EdgeId, PlaceId)>> {
    let mut initial = Flow::default();
    for param in &machine.parameters {
        if matches!(param.scalar_type, ScalarType::Integer(_)) {
            let term = embed(ScalarTerm::Value {
                id: param.id,
                scalar_type: param.scalar_type,
            })?;
            if matches!(param.scalar_type,ScalarType::Integer(i) if i.sign()==IntegerSign::Unsigned)
            {
                initial.bounds.nonnegative(term.clone());
            }
            initial.values.insert(param.id, term)?;
        }
    }
    for param in &machine.structural_parameters {
        initial.storage.insert(
            param.place,
            Record {
                ty: param.structural_type,
                fields: Fields::Entry(param.place, Vec::new()),
            },
        )?;
    }
    let mut originals = Vec::new();
    for input in inputs {
        let record = initial.record(module, *input, &[])?;
        originals.push(initial.interval(module, &record, shape)?);
    }
    let mut remaining = 65_536usize;
    let mut pending = assume_all(vec![initial], &machine.requires, 0, &mut remaining)?
        .into_iter()
        .map(|flow| (machine.entry, flow, BTreeSet::<BlockId>::new()))
        .collect::<Vec<_>>();
    let mut returned = false;
    let mut retained = BTreeSet::new();
    let mut lost = BTreeSet::new();
    while let Some((current, mut flow, mut visited)) = pending.pop() {
        if !visited.insert(current) {
            return Err("interval partition requires a loop invariant");
        }
        let block = machine
            .block(&current)
            .ok_or("partition path names a missing block")?;
        remaining = remaining
            .checked_sub(block.operations.len() + 1)
            .ok_or("interval partition exceeds its receiving work budget")?;
        for op in &block.operations {
            flow.operation(module, op, inputs)?;
        }
        if block
            .terminator
            .cleanup_rosters()
            .any(|(_, actions)| actions.iter().any(|a| a.invoked_machine().is_some()))
        {
            return Err("interval partition crosses an unexamined cleanup call");
        }
        match &block.terminator {
            Terminator::ReturnStructural {
                source,
                returned_claims,
                ..
            } => {
                if &flow.opened != inputs {
                    return Err("interval partition did not consume its promised original input");
                }
                let mut pieces = Vec::new();
                let mut origins = BTreeSet::new();
                for path in paths {
                    if crate::exactly_one::exactly_one(returned_claims.iter(), |r| r.path == *path)
                        .is_none()
                    {
                        return Err("partition output lacks its actual returned claim");
                    }
                    let record = flow.record(module, *source, path)?;
                    let Fields::Built(origin, _) = &record.fields else {
                        return Err("partition output has no constructed child");
                    };
                    if !origins.insert(*origin) {
                        return Err("partition returns the same constructed child more than once");
                    }
                    pieces.push(flow.interval(module, &record, shape)?);
                }
                match (&originals[..], &pieces[..]) {
                    ([whole], _) => flow.bounds.partition(whole, &pieces)?,
                    (_, [whole]) => flow.bounds.partition(whole, &originals)?,
                    _ => return Err("partition requires a single whole on one side"),
                }
                for (arrival, origin) in &flow.arrivals {
                    if origins.contains(origin) {
                        retained.insert(*arrival);
                    } else {
                        lost.insert(*arrival);
                    }
                }
                returned = true;
            }
            Terminator::Crash { .. } => {}
            Terminator::Jump { .. } | Terminator::Conditional { .. } => {
                let arrivals = match &block.terminator {
                    Terminator::Jump {
                        edge,
                        target,
                        arguments,
                        structural_arguments,
                        ..
                    } => [
                        Some((*edge, *target, arguments, structural_arguments)),
                        None,
                    ],
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => [when_true, when_false].map(|to| {
                        Some((to.edge, to.target, &to.arguments, &to.structural_arguments))
                    }),
                    _ => unreachable!(),
                };
                for (edge, target, scalars, records) in arrivals.into_iter().flatten() {
                    let dest = machine
                        .block(&target)
                        .ok_or("partition arrival has no destination")?;
                    if scalars.len() != dest.parameters.len()
                        || records.len() != dest.structural_parameters.len()
                    {
                        return Err("partition arrival changes parameter arity");
                    }
                    // A branch copies the retained value origins along its path.
                    remaining = remaining
                        .checked_sub(
                            flow.values.0.len()
                                + flow.booleans.0.len()
                                + flow.storage.0.len()
                                + flow.arrivals.len()
                                + visited.len()
                                + flow.bounds.len(),
                        )
                        .ok_or("interval partition exceeds its receiving work budget")?;
                    let mut next = flow.clone();
                    if let Terminator::Conditional {
                        condition,
                        when_true,
                        ..
                    } = &block.terminator
                    {
                        let truth = when_true.edge == edge;
                        if let Some(test) = flow.booleans.get(condition) {
                            if test.known(&flow.bounds)?.is_some_and(|value| value != truth) {
                                continue;
                            }
                            test.assume(truth, &mut next.bounds);
                        }
                    }
                    for (formal, actual) in dest.parameters.iter().zip(scalars) {
                        if let Some(value) = flow.booleans.get(actual) {
                            next.booleans.insert(formal.id, value.clone())?;
                        } else {
                            next.values.insert(formal.id, flow.number(*actual)?)?;
                        }
                    }
                    for (formal, actual) in dest.structural_parameters.iter().zip(records) {
                        let record = flow.argument(module, actual)?;
                        if let Fields::Built(origin, _) = &record.fields {
                            next.arrivals.insert((edge, formal.place), *origin);
                        }
                        next.storage.insert(formal.place, record)?;
                    }
                    pending.push((target, next, visited.clone()));
                }
            }
            _ => return Err("interval partition has an unsupported normal control transfer"),
        }
    }
    if !returned {
        return Err("interval partition has no checked normal return");
    }
    retained.retain(|arrival| !lost.contains(arrival));
    Ok(retained)
}
