/*
Turns the mathematical declarations this reader has read into one kernel
signature (`terminal_check_tables::mathematics`).

Each entry refers only to earlier ones. Data becomes a W type and one
definition per case. A machine with a result becomes a definition: a body that
dispatches on one formal is an `indW` whose branch for a case binds that
case's fields, and a call of the machine itself at such a field is the
recursion's result for it. A machine with no result states one law per
`ensures` equality. Its body is not a term: the calls it makes are recorded as
citations, a call of the law itself at a field of the dispatched case as the
induction hypothesis, and stage 06 builds the proof from them.

A cited law is read before the law that cites it, and a law that reaches
itself through its citations is refused, so no statement can justify itself.
*/

use arena::IdTable;
use mathematical_kernel::{Declaration, Level, PlainData, Sort, Term, TermHandle};
use semantic_vocabulary::{
    ClauseCarrier, ClauseIdentity, ClauseOperand, MachineId, Proposition, StructuralTypeId,
};
use symbols::SymbolHandle;
use terminal_check_tables::mathematics::{
    Citation, Evidence, Law, LawCase, LawProof, MathematicalSignature, OperandLicense,
    SignatureEntry, SignatureEntryKind, operand_law,
};
use terminal_check_tables::{Checked, Construct, Refusal};

use super::Program;
use crate::lowering::body::{
    ArmGuard, Body, CallTarget, Expression, ExpressionKind, Statement, Target, ValueType,
};
use crate::lowering::{Unserved, refuse};

/// The trait requirement a mathematical machine satisfies, read at the
/// trait's actual type arguments.
pub(crate) struct Satisfaction {
    machine: MachineId,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    scope: Vec<ValueType>,
    ensures: Vec<Proposition>,
}

impl Program<'_> {
    /// Reads the requirement named by the machine's `satisfies` clause under
    /// that clause's type arguments. The requirement's equalities apply the
    /// trait's operations; the signature resolves each to its satisfier.
    pub(crate) fn satisfaction(&mut self, id: MachineId) -> Checked<Option<Satisfaction>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let symbol = *self
            .symbols
            .get(id)
            .ok_or_else(|| unserved("mathematical machine lost its declaration"))?;
        let (machine, _) = self.declaration(id, symbol)?;
        let mut edges = typed.machine_trait_conformances(machine).into_iter();
        let Some(edge) = edges.next() else {
            return Ok(None);
        };
        if edges.next().is_some() {
            return Err(unserved(
                "mathematical machine satisfying several requirements",
            ));
        }
        if edge.external_binding.is_some()
            || edge.via_expression.is_valid()
            || edge.alias.is_some()
            || !edge.trait_lifetime_arguments.is_empty()
        {
            return Err(unserved(
                "`satisfies` clause with a binding, an alias or lifetimes",
            ));
        }
        let Some(crate::source_state::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        }) = crate::source_state::machine::resolve_satisfied_declaration(typed, machine, edge)
        else {
            return Err(unserved(
                "`satisfies` clause that names no requirement of a trait",
            ));
        };
        let application =
            crate::source_state::trait_definition::AppliedTraitScope::from_machine_conformance(
                typed, machine, edge,
            )
            .ok_or_else(|| unserved("`satisfies` clause lost its trait application"))?;
        // The requirement is read as the trait wrote it, with each type
        // parameter standing for this application's argument.
        let scope = self.applied_scope(id, &application)?;
        let required = self
            .static_requirement_signature(id, requirement, scope.clone(), false, false)
            .map_err(|refusal| {
                self.read_failure_at(refusal, symbol, "the requirement it satisfies")
            })?;
        let own = self
            .signatures
            .get(id)
            .ok_or_else(|| unserved("mathematical machine lost its signature"))?;
        if own.result.is_some()
            && (!required.ensures.is_empty() || !required.membership_ensures.is_empty())
        {
            return Err(self.read_failure_at(
                unserved("operation requirement with a guarantee"),
                symbol,
                "the requirement it satisfies",
            ));
        }
        if required.parameters != own.parameters
            || required.result != own.result
            || !required.requires.is_empty()
        {
            return Err(self.read_failure_at(
                unserved("member signature differs from its requirement's"),
                symbol,
                "the requirement it satisfies",
            ));
        }
        Ok(Some(Satisfaction {
            machine: id,
            declaring_trait: definition.symbol,
            requirement: self.requirement_template(requirement.symbol),
            scope: scope.into_iter().map(|(_, value)| value).collect(),
            ensures: required.ensures,
        }))
    }

    fn requirement_template(&self, requirement: SymbolHandle) -> SymbolHandle {
        self.typed
            .closed_requirement_application(requirement)
            .map_or(requirement, |row| row.template)
    }

    /// The signature of every mathematical machine read so far.
    pub(crate) fn mathematical_signature(
        &self,
        bodies: &IdTable<MachineId, Body>,
        satisfactions: &[Satisfaction],
    ) -> Result<MathematicalSignature, Vec<Refusal>> {
        let mut machines = IdTable::default();
        for _ in self.symbols.iter() {
            machines.push(Read::Unread);
        }
        let mut converter = Converter {
            program: self,
            bodies,
            satisfactions,
            signature: MathematicalSignature::default(),
            data: Vec::new(),
            machines,
        };
        let mut refused = Vec::new();
        let diagnostic = MachineId::new(1).expect("1 is nonzero");
        for &record in &self.self_containing {
            if let Err(refusal) = converter.data(diagnostic, record) {
                let declaration = self
                    .record_sources
                    .get(record)
                    .map_or_else(SymbolHandle::invalid, |source| source.declaration);
                refused.push(self.read_failure_at(
                    refusal,
                    declaration,
                    "the mathematical declaration",
                ));
            }
        }
        // A law whose requirement states associativity or commutativity of
        // an operation is read before the other machines. A proof may only
        // rest on statements that come before its own, so this is what lets
        // any other law regroup and reorder under these wherever the source
        // declares them.
        let mut first = Vec::new();
        for satisfaction in satisfactions {
            let id = satisfaction.machine;
            let read = match converter.requires_operand_law(satisfaction) {
                Ok(false) => continue,
                Ok(true) => converter.declare(id),
                Err(refusal) => {
                    if let Some(state @ Read::Unread) = converter.machines.get_mut(id) {
                        *state = Read::Refused;
                    }
                    Err(refusal)
                }
            };
            first.push(id);
            if let Err(refusal) = read {
                let symbol = self
                    .symbols
                    .get(id)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                refused.push(self.read_failure_at(refusal, symbol, "the mathematical declaration"));
            }
        }
        for (id, symbol) in self.symbols.iter() {
            if first.contains(&id) {
                continue;
            }
            let read = converter.declare(id).map_err(|refusal| {
                self.read_failure_at(refusal, *symbol, "the mathematical declaration")
            });
            if let Err(refusal) = read {
                refused.push(refusal);
            }
        }
        // A machine that states no law means something only where a law
        // calls it. One that nothing calls would be left unread.
        for (id, symbol) in self.symbols.iter() {
            if matches!(
                converter.machines.get(id),
                Some(Read::Cites { called: false })
            ) {
                refused.push(self.read_failure_at(
                    unserved(
                        id,
                        "mathematical machine that states no law and that no law calls",
                    ),
                    *symbol,
                    "the mathematical declaration",
                ));
            }
        }
        if !refused.is_empty() {
            return Err(refused);
        }
        // The license is the declared conformance: the law and the
        // operation it speaks of satisfy requirements of one trait at the
        // same type arguments.
        let mut licenses = Vec::new();
        for satisfaction in satisfactions {
            let Some(Read::Laws(laws)) = converter.machines.get(satisfaction.machine) else {
                continue;
            };
            for &law in laws {
                let statement = converter.signature.laws[law as usize].statement;
                let stated = converter.signature.declarations[statement as usize].ty;
                let Some((operation, law)) = operand_law(&converter.signature.arena, stated) else {
                    continue;
                };
                let conforms = satisfactions.iter().any(|other| {
                    other.declaring_trait == satisfaction.declaring_trait
                        && other.scope == satisfaction.scope
                        && matches!(
                            converter.machines.get(other.machine),
                            Some(Read::Function(entry)) if *entry == operation
                        )
                });
                if conforms {
                    licenses.push(OperandLicense {
                        operation,
                        statement,
                        law,
                    });
                }
            }
        }
        licenses.sort_by_key(|license| license.statement);
        converter.signature.licenses = licenses;
        Ok(converter.signature)
    }
}

#[derive(Clone)]
enum Read {
    Unread,
    /// On the path being read: reaching it again is a cycle.
    Reading,
    Function(u32),
    /// The laws the machine states, by their index.
    Laws(Vec<u32>),
    /// A machine that states no law. Its body is read where a law calls it,
    /// with its formals standing for that call's arguments.
    Cites {
        called: bool,
    },
    Refused,
}

/// What a name of the body stands for, and whether it is a field of the
/// dispatched case: only such a term may carry a recursive call.
#[derive(Clone, Copy)]
struct Bound {
    term: TermHandle,
    field: Option<u32>,
}

struct Scope<'s> {
    /// What each formal stands for, by position.
    slots: &'s [Option<Bound>],
    /// The formal each slot of the body names: a dispatch reads its subject
    /// through a local bound to it.
    aliases: &'s [usize],
    /// The dispatched formal, the case being read and its fields.
    case: Option<(usize, usize, &'s [TermHandle])>,
    /// The machine whose body this is and, for each field of the case, the
    /// recursion's result for it.
    recursion: Option<(MachineId, usize, &'s [TermHandle])>,
}

struct Converter<'p, 't> {
    program: &'p Program<'t>,
    bodies: &'p IdTable<MachineId, Body>,
    satisfactions: &'p [Satisfaction],
    signature: MathematicalSignature,
    /// Each data record with its entry, in the order they were declared.
    data: Vec<(StructuralTypeId, u32)>,
    machines: IdTable<MachineId, Read>,
}

fn unserved(id: MachineId, what: &'static str) -> Refusal {
    refuse(id, Construct::Machine, Unserved::Machine(what))
}

/// The slot each slot of a body stands for, following every local bound to
/// another slot, and the statements that remain.
fn spine(body: &Body) -> (Vec<usize>, Vec<&Statement>) {
    let mut aliases: Vec<usize> = (0..body.slots.len()).collect();
    let mut statements = Vec::with_capacity(body.statements.len());
    for statement in &body.statements {
        match statement {
            Statement::Bind { slot, value }
                if matches!(value.kind, ExpressionKind::Slot(source) if source < aliases.len())
                    && *slot < aliases.len() =>
            {
                let ExpressionKind::Slot(source) = value.kind else {
                    unreachable!("matched above")
                };
                aliases[*slot] = aliases[source];
            }
            other => statements.push(other),
        }
    }
    (aliases, statements)
}

fn index(id: MachineId, length: usize) -> Checked<u32> {
    u32::try_from(length).map_err(|_| unserved(id, "mathematical signature index overflow"))
}

impl Converter<'_, '_> {
    fn constant(&mut self, declaration: u32) -> TermHandle {
        self.signature.arena.insert(Term::Constant {
            declaration,
            levels: Vec::new(),
        })
    }

    fn variable(&mut self, index: u32) -> TermHandle {
        self.signature.arena.insert(Term::Variable(index))
    }

    fn name(&self, symbol: SymbolHandle) -> String {
        self.program.typed.symbols.name(symbol).to_owned()
    }

    fn push(&mut self, declaration: Declaration, kind: SignatureEntryKind, name: String) -> u32 {
        let position = self.signature.declarations.len() as u32;
        self.signature.declarations.push(declaration);
        self.signature.entries.push(SignatureEntry { kind, name });
        position
    }

    /// The entry of a data declaration that contains itself, declaring it and
    /// its cases the first time.
    fn data(&mut self, id: MachineId, record: StructuralTypeId) -> Checked<u32> {
        if let Some(&(_, entry)) = self.data.iter().find(|(known, _)| *known == record) {
            return Ok(entry);
        }
        let program = self.program;
        if program.self_containing.binary_search(&record).is_err() {
            return Err(unserved(
                id,
                "mathematical declaration over a type other than data that contains itself",
            ));
        }
        let declared = program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        let source = program
            .record_sources
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its declaration"))?;
        let mut arities = Vec::with_capacity(declared.cases().len());
        for case in declared.cases() {
            if case
                .fields
                .iter()
                .any(|field| field.value_type != ValueType::record(record))
            {
                return Err(unserved(
                    id,
                    "data that contains itself with a field of another type",
                ));
            }
            arities.push(index(id, case.fields.len())?);
        }
        let shape = PlainData::new(arities)
            .ok_or_else(|| unserved(id, "data that contains itself without a case"))?;
        index(
            id,
            self.signature.declarations.len() + declared.cases().len() + 1,
        )?;
        let arena = &mut self.signature.arena;
        let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let body = shape.ty(arena);
        let name = self.name(source.declaration);
        let entry = self.push(
            Declaration::definition(0, universe, body),
            SignatureEntryKind::Data {
                declaration: source.declaration,
                shape: shape.clone(),
                type_identity: declared.identity.clone(),
            },
            name.clone(),
        );
        self.data.push((record, entry));
        let ty = self.constant(entry);
        for (case, (symbol, _)) in source.cases.iter().enumerate() {
            let arena = &mut self.signature.arena;
            let constructor_type = shape.constructor_type(arena, case, ty);
            let constructor = shape.constructor(arena, case, ty);
            let case_name = format!("{name}::{}", self.name(*symbol));
            self.push(
                Declaration::definition(0, constructor_type, constructor),
                SignatureEntryKind::Constructor {
                    data: entry,
                    case: case as u32,
                },
                case_name,
            );
        }
        Ok(entry)
    }

    fn shape(&self, data: u32) -> PlainData {
        match &self.signature.entries[data as usize].kind {
            SignatureEntryKind::Data { shape, .. } => shape.clone(),
            _ => unreachable!("a data entry was recorded with its shape"),
        }
    }

    /// The closed kernel type of a formal or a result.
    fn ty(&mut self, id: MachineId, value_type: ValueType) -> Checked<(TermHandle, u32)> {
        let ValueType::Record(record, ..) = value_type else {
            return Err(unserved(
                id,
                "mathematical declaration over a type other than data that contains itself",
            ));
        };
        let entry = self.data(id, record)?;
        Ok((self.constant(entry), entry))
    }

    fn formals(&mut self, id: MachineId) -> Checked<Vec<(TermHandle, u32)>> {
        let signature = self
            .program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        if !signature.erased.is_empty()
            || !signature.borrowed.is_empty()
            || !signature.requires.is_empty()
        {
            return Err(unserved(
                id,
                "mathematical machine with an erased or borrowed formal or a premise",
            ));
        }
        signature
            .parameters
            .clone()
            .into_iter()
            .map(|formal| self.ty(id, formal))
            .collect()
    }

    /// Whether the requirement this machine satisfies states associativity
    /// or commutativity of an operation, read without the machine's body.
    fn requires_operand_law(&mut self, satisfaction: &Satisfaction) -> Checked<bool> {
        let id = satisfaction.machine;
        let [clause] = satisfaction.ensures.as_slice() else {
            return Ok(false);
        };
        let formals = self.formals(id)?;
        let count = formals.len();
        let open: Vec<_> = (0..count)
            .map(|formal| self.variable((count - 1 - formal) as u32))
            .collect();
        let mut statement = self.equality(id, clause, &open, Some(satisfaction))?;
        for &(formal, _) in formals.iter().rev() {
            statement = self.signature.arena.insert(Term::Pi {
                domain: formal,
                codomain: statement,
            });
        }
        Ok(operand_law(&self.signature.arena, statement).is_some())
    }

    fn declare(&mut self, id: MachineId) -> Checked<()> {
        let result = self
            .program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?
            .result;
        match result {
            Some(_) => self.function(id).map(|_| ()),
            None => self.laws(id).map(|_| ()),
        }
    }

    fn begin(&mut self, id: MachineId, cycle: &'static str) -> Checked<Option<Read>> {
        let state = self
            .machines
            .get(id)
            .cloned()
            .ok_or_else(|| unserved(id, "mathematical machine outside this reader"))?;
        match state {
            Read::Unread => {
                *self.machines.get_mut(id).expect("row read above") = Read::Reading;
                Ok(None)
            }
            Read::Reading => Err(unserved(id, cycle)),
            Read::Refused => Err(unserved(
                id,
                "use of a mathematical machine that is refused above",
            )),
            known => Ok(Some(known)),
        }
    }

    fn finish<T>(&mut self, id: MachineId, read: Checked<(T, Read)>) -> Checked<T> {
        let (value, state) = match read {
            Ok((value, state)) => (Ok(value), state),
            Err(refusal) => (Err(refusal), Read::Refused),
        };
        *self.machines.get_mut(id).expect("row read at begin") = state;
        value
    }

    /// A machine with a result, as a definition.
    fn function(&mut self, id: MachineId) -> Checked<u32> {
        match self.begin(id, "mathematical machines that call each other in a cycle")? {
            None => {}
            Some(Read::Function(entry)) => return Ok(entry),
            Some(_) => return Err(unserved(id, "term that calls a machine with no result")),
        }
        let read = self.read_function(id);
        self.finish(id, read.map(|entry| (entry, Read::Function(entry))))
    }

    fn read_function(&mut self, id: MachineId) -> Checked<u32> {
        let program = self.program;
        let signature = program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        if !signature.ensures.is_empty() || !signature.membership_ensures.is_empty() {
            return Err(unserved(
                id,
                "mathematical machine with a result and a guarantee",
            ));
        }
        let formals = self.formals(id)?;
        let (result, _) = self.ty(
            id,
            signature
                .result
                .ok_or_else(|| unserved(id, "mathematical function without a result"))?,
        )?;
        let count = formals.len();
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
        let (aliases, statements) = spine(body);
        let named = |slot: usize| aliases.get(slot).copied().unwrap_or(slot);
        let mut term = match statements.as_slice() {
            [
                Statement::Arm {
                    guard: ArmGuard::Always,
                    target: Target::Value(value),
                    ..
                },
            ] => {
                let slots: Vec<_> = (0..count)
                    .map(|formal| {
                        Some(Bound {
                            term: self.variable((count - 1 - formal) as u32),
                            field: None,
                        })
                    })
                    .collect();
                let scope = Scope {
                    slots: &slots,
                    aliases: &aliases,
                    case: None,
                    recursion: None,
                };
                self.expression(id, value, &scope)?.term
            }
            [
                Statement::Dispatch {
                    subject,
                    arms,
                    default: None,
                    ..
                },
            ] => {
                let ExpressionKind::Slot(subject) = subject.kind else {
                    return Err(unserved(id, "mathematical dispatch on a computed subject"));
                };
                let subject = named(subject);
                if subject >= count {
                    return Err(unserved(id, "mathematical dispatch on a local"));
                }
                self.ranked_on(id, subject, false)?;
                let data = formals[subject].1;
                let shape = self.shape(data);
                let others: Vec<usize> = (0..count).filter(|formal| *formal != subject).collect();
                let rest = others.len() as u32;
                // The motive returns a function of the other formals, so a
                // recursive call may pass them different values.
                let mut landing = result;
                for &other in others.iter().rev() {
                    landing = self.signature.arena.insert(Term::Pi {
                        domain: formals[other].0,
                        codomain: landing,
                    });
                }
                let motive = self.signature.arena.insert(Term::Lambda {
                    domain: formals[subject].0,
                    body: landing,
                });
                let mut branches = Vec::with_capacity(shape.cases());
                for case in 0..shape.cases() {
                    let mut selected = arms.iter().filter(|(selector, _, _)| selector.case == case);
                    let (Some((selector, Target::Value(value), _)), None) =
                        (selected.next(), selected.next())
                    else {
                        return Err(unserved(
                            id,
                            "mathematical dispatch without exactly one value arm per case",
                        ));
                    };
                    if selector.condition.is_some() {
                        return Err(unserved(id, "mathematical dispatch on a field value"));
                    }
                    let arena = &mut self.signature.arena;
                    let fields: Vec<_> = (0..shape.arity(case))
                        .map(|field| shape.field(arena, case, field, rest))
                        .collect();
                    let results: Vec<_> = (0..shape.arity(case))
                        .map(|field| shape.hypothesis(arena, case, field, rest))
                        .collect();
                    let mut slots = vec![None; count];
                    for (position, &other) in others.iter().enumerate() {
                        slots[other] = Some(Bound {
                            term: self.variable(rest - 1 - position as u32),
                            field: None,
                        });
                    }
                    let scope = Scope {
                        slots: &slots,
                        aliases: &aliases,
                        case: Some((subject, case, &fields)),
                        recursion: Some((id, subject, &results)),
                    };
                    let mut branch = self.expression(id, value, &scope)?.term;
                    for &other in others.iter().rev() {
                        branch = self.signature.arena.insert(Term::Lambda {
                            domain: formals[other].0,
                            body: branch,
                        });
                    }
                    branches.push(branch);
                }
                if arms.len() != shape.cases() {
                    return Err(unserved(
                        id,
                        "mathematical dispatch without exactly one value arm per case",
                    ));
                }
                let step = shape.step(&mut self.signature.arena, motive, &branches);
                let tree = self.variable((count - 1 - subject) as u32);
                let induction = self
                    .signature
                    .arena
                    .insert(Term::IndW { motive, step, tree });
                let arguments: Vec<_> = others
                    .iter()
                    .map(|&other| self.variable((count - 1 - other) as u32))
                    .collect();
                self.signature.arena.apply_arguments(induction, &arguments)
            }
            _ => {
                return Err(unserved(
                    id,
                    "mathematical function body other than one value or one dispatch",
                ));
            }
        };
        let mut ty = result;
        for &(formal, _) in formals.iter().rev() {
            term = self.signature.arena.insert(Term::Lambda {
                domain: formal,
                body: term,
            });
            ty = self.signature.arena.insert(Term::Pi {
                domain: formal,
                codomain: ty,
            });
        }
        index(id, self.signature.declarations.len() + 1)?;
        let symbol = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        let name = self.name(symbol);
        Ok(self.push(
            Declaration::definition(0, ty, term),
            SignatureEntryKind::Function { machine: symbol },
            name,
        ))
    }

    /// A recursive machine must rank on the formal it dispatches on: the
    /// kernel's induction descends on that formal and no other.
    fn ranked_on(&self, id: MachineId, subject: usize, required: bool) -> Checked<()> {
        let ranking = self
            .program
            .signatures
            .get(id)
            .and_then(|signature| signature.ranking.as_ref());
        match ranking {
            None if required => Err(unserved(
                id,
                "recursive mathematical machine without `terminates by` its dispatched formal",
            )),
            None => Ok(()),
            Some(ranking)
                if matches!(ranking.measures.as_slice(),
                    [measure] if measure.kind == ExpressionKind::Slot(subject)) =>
            {
                Ok(())
            }
            Some(_) => Err(unserved(
                id,
                "mathematical machine ranked on a formal it does not dispatch on",
            )),
        }
    }

    /// A value expression of a mathematical body as a term.
    fn expression(
        &mut self,
        id: MachineId,
        expression: &Expression,
        scope: &Scope<'_>,
    ) -> Checked<Bound> {
        let plain = |term| Bound { term, field: None };
        match &expression.kind {
            ExpressionKind::Slot(slot) => scope
                .slots
                .get(scope.aliases.get(*slot).copied().unwrap_or(*slot))
                .copied()
                .flatten()
                .ok_or_else(|| {
                    unserved(
                        id,
                        "mathematical term names the dispatched formal or a local",
                    )
                }),
            ExpressionKind::Payload { base, case, field } => match (&base.kind, scope.case) {
                (ExpressionKind::Slot(slot), Some((subject, selected, fields)))
                    if scope.aliases.get(*slot).copied().unwrap_or(*slot) == subject
                        && *case == selected
                        && *field < fields.len() =>
                {
                    Ok(Bound {
                        term: fields[*field],
                        field: Some(*field as u32),
                    })
                }
                _ => Err(unserved(
                    id,
                    "mathematical term reads a field outside the dispatched case",
                )),
            },
            ExpressionKind::Case { case, fields } => {
                let Some(ValueType::Record(record, ..)) = expression.result else {
                    return Err(unserved(id, "mathematical constructor lost its data"));
                };
                let data = self.data(id, record)?;
                let mut operands = vec![None; fields.len()];
                for (position, operand) in fields {
                    let term = self.expression(id, operand, scope)?.term;
                    *operands
                        .get_mut(*position)
                        .ok_or_else(|| unserved(id, "mathematical constructor field position"))? =
                        Some(term);
                }
                let operands: Vec<TermHandle> = operands
                    .into_iter()
                    .collect::<Option<_>>()
                    .ok_or_else(|| unserved(id, "mathematical constructor misses a field"))?;
                let constructor = self.constant(data + 1 + *case as u32);
                Ok(plain(
                    self.signature.arena.apply_arguments(constructor, &operands),
                ))
            }
            ExpressionKind::Call {
                callee: CallTarget::Machine(callee),
                arguments,
                ..
            } => {
                let mut operands = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    operands.push(self.expression(id, argument, scope)?);
                }
                if let Some((machine, subject, results)) = scope.recursion
                    && machine == *callee
                {
                    let field = operands
                        .get(subject)
                        .and_then(|operand| operand.field)
                        .ok_or_else(|| {
                            unserved(
                                id,
                                "recursive call whose ranked argument is not a field of the dispatched case",
                            )
                        })?;
                    self.ranked_on(id, subject, true)?;
                    let rest: Vec<_> = operands
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| *position != subject)
                        .map(|(_, operand)| operand.term)
                        .collect();
                    return Ok(plain(
                        self.signature
                            .arena
                            .apply_arguments(results[field as usize], &rest),
                    ));
                }
                let function = self.function(*callee)?;
                let function = self.constant(function);
                let operands: Vec<_> = operands.iter().map(|operand| operand.term).collect();
                Ok(plain(
                    self.signature.arena.apply_arguments(function, &operands),
                ))
            }
            _ => Err(unserved(
                id,
                "mathematical term other than a formal, a field, a constructor or a call",
            )),
        }
    }

    /// The laws a machine with no result states, declaring them the first
    /// time. A machine that states none only cites; it is read at its call.
    fn laws(&mut self, id: MachineId) -> Checked<Vec<u32>> {
        match self.begin(id, "law that reaches itself through its citations")? {
            None => {}
            Some(Read::Laws(laws)) => return Ok(laws),
            Some(Read::Cites { .. }) => return Ok(Vec::new()),
            Some(_) => return Err(unserved(id, "citation of a machine with a result")),
        }
        let read = self.read_laws(id);
        self.finish(
            id,
            read.map(|laws| {
                let state = if laws.is_empty() {
                    Read::Cites { called: false }
                } else {
                    Read::Laws(laws.clone())
                };
                (laws, state)
            }),
        )
    }

    fn read_laws(&mut self, id: MachineId) -> Checked<Vec<u32>> {
        let program = self.program;
        let signature = program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        let names = program
            .names
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its names"))?;
        let formals = self.formals(id)?;
        let count = formals.len();
        let open: Vec<_> = (0..count)
            .map(|formal| self.variable((count - 1 - formal) as u32))
            .collect();
        let satisfaction = self
            .satisfactions
            .iter()
            .find(|satisfaction| satisfaction.machine == id);
        let mut declared = Vec::new();
        if let Some(satisfaction) = satisfaction {
            for clause in &satisfaction.ensures {
                declared.push(self.equality(id, clause, &open, Some(satisfaction))?);
            }
        }
        // The machine's own equalities, else the ones its requirement declares.
        let mut goals = Vec::new();
        for (clause, proposition) in signature.ensures.iter().enumerate() {
            let goal = self.equality(id, proposition, &open, None)?;
            let text = names
                .ensures
                .get(clause)
                .map(|text| (names.authored(text), text.source));
            goals.push((goal, text));
        }
        let own = !goals.is_empty();
        let declared = match (own, declared.as_slice()) {
            (false, _) => {
                goals = declared.into_iter().map(|goal| (goal, None)).collect();
                None
            }
            (true, []) => None,
            (true, [declared]) if goals.len() == 1 => Some(*declared),
            (true, _) => {
                return Err(unserved(
                    id,
                    "law with several equalities satisfying a requirement",
                ));
            }
        };
        if goals.is_empty() {
            return Ok(Vec::new());
        }
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
        let proof = self.script(id, body, &formals)?;
        let symbol = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        let mut laws = Vec::with_capacity(goals.len());
        for (clause, (goal, text)) in goals.into_iter().enumerate() {
            let law = index(id, self.signature.laws.len())?;
            index(id, self.signature.declarations.len() + 1)?;
            let mut statement = goal;
            for &(formal, _) in formals.iter().rev() {
                statement = self.signature.arena.insert(Term::Pi {
                    domain: formal,
                    codomain: statement,
                });
            }
            let entry = self.push(
                Declaration::assumption(0, statement),
                SignatureEntryKind::Statement { law },
                names.machine.clone(),
            );
            let (text, source) = text.unwrap_or_default();
            self.signature.laws.push(Law {
                machine: symbol,
                clause: clause as u32,
                statement: entry,
                formals: formals.iter().map(|(formal, _)| *formal).collect(),
                goal,
                declared: declared.filter(|_| clause == 0),
                proof: proof.clone(),
                formal_names: names.formals.clone(),
                machine_name: names.machine.clone(),
                text,
                source: source.or(names.declared),
            });
            laws.push(law);
        }
        Ok(laws)
    }

    /// One `ensures` equality under the law's formals.
    fn equality(
        &mut self,
        id: MachineId,
        proposition: &Proposition,
        formals: &[TermHandle],
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<TermHandle> {
        let Proposition::DeclarationIdentity(clause) = proposition else {
            return Err(unserved(id, "law guarantee other than an Identity"));
        };
        self.identity(id, clause, formals, satisfaction)
    }

    fn identity(
        &mut self,
        id: MachineId,
        clause: &ClauseIdentity,
        formals: &[TermHandle],
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<TermHandle> {
        clause
            .validate()
            .map_err(|_| unserved(id, "malformed declaration Identity"))?;
        let mut terms = Vec::<TermHandle>::with_capacity(clause.values.len());
        let mut carriers = Vec::with_capacity(clause.values.len());
        for value in &clause.values {
            let ClauseCarrier::Structural(record) = value.carrier else {
                return Err(unserved(
                    id,
                    "law over a carrier other than mathematical data",
                ));
            };
            carriers.push(self.data(id, record)?);
            let term = match &value.operand {
                // Every formal is data, so the formal at position `k` is place `k + 1`.
                ClauseOperand::Input(subject) if subject.path().is_empty() => subject
                    .root()
                    .get()
                    .checked_sub(1)
                    .and_then(|formal| formals.get(usize::try_from(formal).ok()?))
                    .copied()
                    .ok_or_else(|| unserved(id, "law names storage outside its formals"))?,
                ClauseOperand::Application {
                    declaration,
                    arguments,
                } if declaration.supplier.is_none() => {
                    let member =
                        SymbolHandle::from_parts(declaration.member.0, declaration.member.1);
                    let operands: Vec<_> = arguments
                        .iter()
                        .map(|&argument| terms[argument as usize])
                        .collect();
                    let head = self.declaration(id, member, satisfaction)?;
                    let head = self.constant(head);
                    self.signature.arena.apply_arguments(head, &operands)
                }
                _ => {
                    return Err(unserved(
                        id,
                        "law operand other than a formal, a constructor or a mathematical call",
                    ));
                }
            };
            terms.push(term);
        }
        let ty = self.constant(carriers[clause.left as usize]);
        Ok(self.signature.arena.insert(Term::Id {
            ty,
            left: terms[clause.left as usize],
            right: terms[clause.right as usize],
        }))
    }

    /// The entry a clause applies: a machine with a result, a case, or the
    /// satisfier of a trait operation under the law's own trait application.
    fn declaration(
        &mut self,
        id: MachineId,
        member: SymbolHandle,
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<u32> {
        let program = self.program;
        if let Some((machine, _)) = program.symbols.iter().find(|(_, known)| **known == member) {
            return self.function(machine);
        }
        for &record in &program.self_containing {
            let Some(source) = program.record_sources.get(record) else {
                continue;
            };
            if let Some(case) = source.cases.iter().position(|(case, _)| *case == member) {
                return Ok(self.data(id, record)? + 1 + case as u32);
            }
        }
        if let Some(law) = satisfaction {
            let operation = self
                .satisfactions
                .iter()
                .find(|other| {
                    other.declaring_trait == law.declaring_trait
                        && other.scope == law.scope
                        && other.requirement == member
                })
                .ok_or_else(|| {
                    unserved(
                        id,
                        "declared law applies an operation that no machine satisfies at these type arguments",
                    )
                })?;
            return self.function(operation.machine);
        }
        Err(unserved(
            id,
            "clause applies a declaration outside the mathematical signature",
        ))
    }

    /// How the body argues: its citations, per case when it dispatches.
    fn script(
        &mut self,
        id: MachineId,
        body: &Body,
        formals: &[(TermHandle, u32)],
    ) -> Checked<LawProof> {
        let count = formals.len();
        let (aliases, statements) = spine(body);
        let (dispatch, before) = match statements.split_last() {
            Some((
                Statement::Dispatch {
                    subject,
                    arms,
                    default,
                    ..
                },
                before,
            )) => (Some((subject, arms, default)), before),
            _ => (None, statements.as_slice()),
        };
        let Some((subject, arms, default)) = dispatch else {
            let slots: Vec<_> = (0..count)
                .map(|formal| {
                    Some(Bound {
                        term: self.variable((count - 1 - formal) as u32),
                        field: None,
                    })
                })
                .collect();
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: None,
                recursion: None,
            };
            let mut case = LawCase::default();
            self.citations(id, id, before, &scope, &mut case.citations)?;
            return Ok(LawProof::Direct(case));
        };
        let ExpressionKind::Slot(subject) = subject.kind else {
            return Err(unserved(id, "law dispatch on a computed subject"));
        };
        let subject = aliases.get(subject).copied().unwrap_or(subject);
        if subject >= count || default.is_some() {
            return Err(unserved(
                id,
                "law dispatch on a local or with a default arm",
            ));
        }
        self.ranked_on(id, subject, false)?;
        let data = formals[subject].1;
        let shape = self.shape(data);
        let record = self
            .data
            .iter()
            .find(|(_, entry)| *entry == data)
            .map(|(record, _)| *record)
            .expect("a declared data entry has its record");
        let declared = self
            .program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        if arms.len() != shape.cases() {
            return Err(unserved(
                id,
                "law dispatch without exactly one arm per case",
            ));
        }
        let mut cases = Vec::with_capacity(shape.cases());
        for case in 0..shape.cases() {
            let mut selected = arms.iter().filter(|(selector, _, _)| selector.case == case);
            let (Some((selector, target, origin)), None) = (selected.next(), selected.next())
            else {
                return Err(unserved(
                    id,
                    "law dispatch without exactly one arm per case",
                ));
            };
            if selector.condition.is_some() {
                return Err(unserved(id, "law dispatch on a field value"));
            }
            // A case's terms live under the formals, then its fields.
            let arity = shape.arity(case);
            let fields: Vec<_> = (0..arity)
                .map(|field| self.variable(arity - 1 - field))
                .collect();
            let slots: Vec<_> = (0..count)
                .map(|formal| {
                    Some(Bound {
                        term: self.variable((count - 1 - formal) as u32 + arity),
                        field: None,
                    })
                })
                .collect();
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: Some((subject, case, &fields)),
                recursion: Some((id, subject, &[])),
            };
            let mut script = LawCase {
                citations: Vec::new(),
                field_names: declared.cases()[case]
                    .fields
                    .iter()
                    .map(|field| field.identity.clone())
                    .collect(),
                source: match origin {
                    crate::lowering::SourceOrigin::Authored(span) => Some(*span),
                    _ => None,
                },
            };
            self.citations(id, id, before, &scope, &mut script.citations)?;
            match target {
                Target::Unit => {}
                Target::Value(value) => self.cite(id, id, value, &scope, &mut script.citations)?,
                Target::Crash(_) => return Err(unserved(id, "law case that crashes")),
            }
            cases.push(script);
        }
        Ok(LawProof::Cases {
            formal: subject as u32,
            data,
            cases,
        })
    }

    /// The citations a statement list makes. `law` is the law being read and
    /// `machine` the body the statements belong to, the law's or a helper's.
    fn citations(
        &mut self,
        law: MachineId,
        machine: MachineId,
        statements: &[&Statement],
        scope: &Scope<'_>,
        citations: &mut Vec<Citation>,
    ) -> Checked<()> {
        for statement in statements.iter().copied() {
            match statement {
                Statement::Evaluate { expression, .. } => {
                    self.cite(law, machine, expression, scope, citations)?
                }
                Statement::Arm {
                    guard: ArmGuard::Always,
                    target: Target::Unit,
                    ..
                } => {}
                Statement::Arm {
                    guard: ArmGuard::Always,
                    target: Target::Value(value),
                    ..
                } => self.cite(law, machine, value, scope, citations)?,
                _ => {
                    return Err(unserved(
                        machine,
                        "law body statement other than a citation or one dispatch",
                    ));
                }
            }
        }
        Ok(())
    }

    fn cite(
        &mut self,
        law: MachineId,
        machine: MachineId,
        call: &Expression,
        scope: &Scope<'_>,
        citations: &mut Vec<Citation>,
    ) -> Checked<()> {
        let ExpressionKind::Call {
            callee: CallTarget::Machine(callee),
            arguments,
            ..
        } = &call.kind
        else {
            return Err(unserved(machine, "law body statement other than a call"));
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            operands.push(self.expression(machine, argument, scope)?);
        }
        if *callee == law {
            // The law at a field of the dispatched case is the induction
            // hypothesis. At any other argument it would be its own evidence.
            let Some((_, subject, _)) = scope.recursion else {
                return Err(unserved(
                    machine,
                    "law cites itself without dispatching on a formal",
                ));
            };
            let field = operands
                .get(subject)
                .and_then(|operand| operand.field)
                .ok_or_else(|| {
                    unserved(
                        machine,
                        "law cites itself at an argument that is not a field of the dispatched case",
                    )
                })?;
            self.ranked_on(law, subject, true)?;
            citations.push(Citation {
                evidence: Evidence::Hypothesis { field },
                arguments: operands
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| *position != subject)
                    .map(|(_, operand)| operand.term)
                    .collect(),
            });
            return Ok(());
        }
        let cited = self
            .program
            .signatures
            .get(*callee)
            .ok_or_else(|| unserved(machine, "citation lost its machine"))?;
        if cited.result.is_some() {
            return Err(unserved(machine, "law body calls a machine with a result"));
        }
        let laws = self.laws(*callee)?;
        if !laws.is_empty() {
            let arguments: Vec<_> = operands.iter().map(|operand| operand.term).collect();
            for cited in laws {
                citations.push(Citation {
                    evidence: Evidence::Statement(self.signature.laws[cited as usize].statement),
                    arguments: arguments.clone(),
                });
            }
            return Ok(());
        }
        // A machine that states nothing is read in place, with its formals
        // standing for the arguments of this call.
        *self.machines.get_mut(*callee).expect("row read by `laws`") = Read::Reading;
        let slots: Vec<_> = operands.iter().copied().map(Some).collect();
        let body = self
            .bodies
            .get(*callee)
            .ok_or_else(|| unserved(machine, "citation lost its body"))?;
        let (aliases, statements) = spine(body);
        let inner = Scope {
            slots: &slots,
            aliases: &aliases,
            case: None,
            recursion: scope.recursion,
        };
        let read = self.citations(law, *callee, &statements, &inner, citations);
        *self.machines.get_mut(*callee).expect("row read by `laws`") = match read {
            Ok(()) => Read::Cites { called: true },
            Err(_) => Read::Refused,
        };
        read
    }
}
