/*
Lowering of one closed body to a Terminal Psi machine and its check tables.
Every identity is minted by pushing its row, so a `ValueId`, `EdgeId`,
`ObligationId`, `InvocationId` or root `PlaceId` is the same integer in the
module and in `MachineTables`, and each block, operation, edge and obligation
also gets a `MachineSources` row in lockstep. Records, their places and the
write log are in `places`.

Evaluation order is `wiki/spec/language/expressions.md`, "Evaluation
schedule": strict operands left then right, call arguments left to right,
`&&`/`||` evaluate the right operand only when the left does not decide, and
only the selected transition arm runs. Each operation is emitted after its
operands, so block order is evaluation order.

Obligations are created, never discharged:

- `OperationSafety` for each partial operation (control_flow.md: "Each
  partial operation still proves its own safety"). Partial means the
  operation's `integer_policy_bridge` has formation conditions; for `Exact`
  integers that is every add, subtract, multiply, divide, remainder and shift.
- `CallRequirement` per callee `requires` clause per call (fact_validity.md:
  "one exact obligation for every callee `requires` clause").
- `NormalGuarantee` per `ensures` clause per normal exit (control_flow.md:
  "All normal returns ... must establish the published guarantees.").
- `ArrivalInvariant` per edge that passes scalar arguments to a block
  (control_flow.md: a block invariant has "one obligation for every actual
  incoming edge"). Short-circuit joins are the blocks that take them.
- `CrashCoverage` per crash site and cause (calls_and_outcomes.md: "The
  verifier proves every site guard from independent entry/CFG facts and
  checks same-cause coverage"). A site is a block whose terminator leaves
  without a successor (a `Crash` with its one cause, or an `InvokeNoNormal`
  with one obligation per cause among the callee's substituted routes), or a
  call operation whose callee may return and also publishes crash routes,
  again with one obligation per cause among the substituted routes.

Each is created at the point before the construct it guards: the program point
after the preceding operation, or the block entry when none precedes it.

A crash is a `Crash` terminator carrying its cause and its site guard: the
exact incoming conjunction of branch decisions on the path to it ("The guard
retains the exact incoming conjunction"), each stated as the branch value
equal to the Boolean its edge took. A call of a machine with no normal
completion is an `InvokeNoNormal` terminator: there is no normal successor to
return through ("A no-normal-result terminal transfer publishes no fictional
result or normal-return guarantee"). Both edges go to `Destination::NoNormal`.
*/

mod calls;
mod control;
mod operations;
pub(crate) mod places;

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IdTable, MachineId, ObligationId, PlaceId, Proposition,
    ScalarQualificationSetId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::values::{RigidType, ValueOrigin, ValueRow};
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    Block, BoundaryParameterKind, ContractClause, MachineContract, Operation,
    RegisterTargetApplicability, ScalarBlockInvariant, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    SuccessorEdge, TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration,
};

use self::places::{Bound, Records, Witness};
use crate::body::{Body, Signature, Signatures, ValueType};
use crate::canonical;
use crate::outcome::Outcome;
use crate::{MachineSources, SourceOrigin, Unpositioned, Unserved, refuse};

#[derive(Debug)]
pub(crate) struct Lowered {
    pub machine: TerminalMachine,
    pub tables: MachineTables,
    pub sources: MachineSources,
    /// The block invariant of each loop head whose member has a `requires`,
    /// in head order: rows of the module's `scalar_block_invariants`.
    pub invariants: Vec<ScalarBlockInvariant>,
}

/// One member of the call component being lowered as a loop: its head block
/// and the block parameters that stand for its scalar formals.
struct Head {
    member: MachineId,
    block: BlockId,
    parameters: Vec<ValueId>,
}

/// Lowers machine `id`. `component` holds the members of its call component
/// when it lies on a call cycle (itself among them) and is empty otherwise;
/// `bodies` is read for those members only.
pub(crate) fn machine(
    id: MachineId,
    bodies: &IdTable<MachineId, Body>,
    component: &[MachineId],
    signatures: Signatures<'_>,
    outcomes: &IdTable<MachineId, Outcome>,
    records: &Records<'_>,
    requirement: bool,
) -> Checked<Lowered> {
    let body = terminal_check_tables::row(bodies, id, id, Construct::Machine)?;
    let signature = signatures.get(id.into(), id)?;
    let mut lowering = Lowering::new(id, signature, signatures, outcomes, records, &body.slots);
    let entry = lowering.block(SourceOrigin::Unpositioned(Unpositioned::MachineEntry));
    let (formals, result) = lowering.formals()?;
    lowering.enter(entry);
    // One published identity per `ensures` clause, the one `ContractClause`
    // carries; every normal exit's guarantee names it.
    for clause in 0..signature.ensures.len() {
        let clause = lowering.position(clause)?;
        let published = lowering.obligation(
            ObligationKind::PublishedGuarantee { clause },
            ProgramPoint::BlockEntry(entry),
            SourceOrigin::Unpositioned(Unpositioned::MachineEntry),
        );
        lowering.published.push(published);
    }
    // A requirement is its formals, its result and its contract. It has no
    // block: the entry block opened above for those identities is dropped.
    if requirement {
        lowering.blocks = IdTable::default();
        lowering.sources.blocks = IdTable::default();
        return lowering.finish(entry, formals, result);
    }
    if component.is_empty() {
        lowering.body(body)?;
    } else {
        lowering.component(id, bodies, component, &formals)?;
    }
    lowering.finish(entry, formals, result)
}

/// Declare a bodyless interface using the same formal/storage construction
/// as a body. The supplied header names real source declarations; this emits
/// no executable block, checked-body marker or provider installation.
pub(crate) fn boundary(
    caller: MachineId,
    id: semantic_vocabulary::BoundaryMachineId,
    signatures: Signatures<'_>,
    outcomes: &IdTable<MachineId, Outcome>,
    records: &Records<'_>,
    metadata: (
        terminal_psi::NominalDeclarationIdentity,
        terminal_psi::CheckedBoundaryCallable,
    ),
    service: semantic_vocabulary::ServiceId,
) -> Checked<terminal_psi::BoundaryMachineDeclaration> {
    let signature = signatures.get(
        semantic_vocabulary::TargetMachineCallable::Boundary(id),
        caller,
    )?;
    let unavailable = |detail| refuse(caller, Construct::Machine, Unserved::Declaration(detail));
    if !signature.erased.is_empty()
        || signature
            .parameters
            .iter()
            .any(|parameter| matches!(parameter, ValueType::Rigid(_)))
        || matches!(signature.result, Some(ValueType::Rigid(_)))
    {
        return Err(unavailable(
            "boundary declaration needs its closed runtime telescope",
        ));
    }
    let crash_routes = signature
        .declared_crash_routes()
        .ok_or_else(|| unavailable("boundary declaration has no published crash ceiling"))?;
    let mut lowering = Lowering::new(
        caller,
        signature,
        signatures,
        outcomes,
        records,
        &signature.parameters,
    );
    let (formals, result) = lowering.formals()?;
    // effects.md: a boundary requirement "publishes one stable interface
    // across providers", so its written ceiling is both what it may reach
    // and what a caller is charged.
    let crate::Ceiling::Authored(reach) = &signature.reaches else {
        return Err(unavailable(
            "boundary declaration has no declared reach ceiling",
        ));
    };
    // A requirement's `invokes` bounds "the entries of every admitted
    // implementation"; the reader states omission as the empty row.
    let crate::Ceiling::Authored(entries) = &signature.invokes else {
        return Err(unavailable(
            "boundary declaration has no declared entry ceiling",
        ));
    };
    let result = match result {
        TerminalMachineResult::Unit => terminal_psi::BoundaryMachineResult::Unit,
        TerminalMachineResult::Scalar(result) => {
            terminal_psi::BoundaryMachineResult::Scalar(result.scalar_type.into())
        }
        TerminalMachineResult::Structural(result) => {
            terminal_psi::BoundaryMachineResult::Structural(
                terminal_psi::BoundaryStructuralResultDeclaration {
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: result.qualifications,
                    projected_qualifications: result.projected_qualifications,
                },
            )
        }
    };
    Ok(terminal_psi::BoundaryMachineDeclaration {
        id,
        identity: metadata.1.source.callable,
        owner: metadata.0,
        declaration: Some(metadata.1),
        numeric_intrinsic: None,
        role: terminal_psi::BoundaryDeclarationRole::Callable,
        kind: semantic_vocabulary::BoundaryRequirementKind::Trait,
        completion: signatures.completion(id, caller)?,
        unconditional_termination: signature.termination != crate::Termination::Silent,
        attachment: None,
        parameter_order: formals.order,
        scalar_parameters: formals
            .scalars
            .iter()
            .map(|formal| formal.scalar_type)
            .collect(),
        structural_parameters: formals.records,
        result,
        result_reference_regions: Vec::new(),
        crash_routes: crash_routes.to_vec(),
        scalar_requires: signature.requires.clone(),
        scalar_ensures: signature.ensures.clone(),
        // The shared signature reader refuses declared domains, reference
        // results, suspension and blocking before this point. Those
        // actual unsupported dimensions cannot be replaced by empty claims.
        requires: Vec::new(),
        normal_qualifications: Vec::new(),
        scalar_domain_requirements: Vec::new(),
        reference_outputs: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: reach.clone(),
        owning_service: Some(service),
        published_service_ceiling: reach.clone(),
        published_synchronous_entries: entries.clone(),
    })
}

/// A machine's formals in Terminal Psi's two lanes, and the authored order
/// that interleaves them.
#[derive(Default)]
struct Formals {
    scalars: Vec<ValueDeclaration>,
    records: Vec<StructuralParameterDeclaration>,
    order: Vec<BoundaryParameterKind>,
    /// The `[erased]` scalar formals, in position order: declared in the
    /// contract, absent from both runtime lanes and from `order`.
    erased: Vec<ValueDeclaration>,
}

impl<'a> Lowering<'a> {
    fn new(
        id: MachineId,
        signature: &'a Signature,
        signatures: Signatures<'a>,
        outcomes: &'a IdTable<MachineId, Outcome>,
        records: &'a Records<'a>,
        slot_types: &'a [ValueType],
    ) -> Self {
        Self {
            id,
            signature,
            signatures,
            outcomes,
            records,
            roots: IdTable::default(),
            entry_contents: Vec::new(),
            path: Vec::new(),
            tables: MachineTables::default(),
            sources: MachineSources::default(),
            blocks: IdTable::default(),
            current: None,
            point: None,
            slots: vec![None; slot_types.len()],
            selected: Vec::new(),
            slot_types,
            published: Vec::new(),
            heads: Vec::new(),
            invariants: Vec::new(),
            borrowed_roots: Vec::new(),
            joining: 0,
        }
    }
}

impl Lowering<'_> {
    /// A body and an interface declare the same scalar and storage formals.
    /// Construct those identities once without inventing executable blocks
    /// or guarantees for a bodyless declaration.
    fn formals(&mut self) -> Checked<(Formals, TerminalMachineResult)> {
        let signature = self.signature;
        let mut formals = Formals::default();
        let formal_values = signature.formal_values();
        for (position, value_type) in signature.parameters.iter().enumerate() {
            if signature.is_erased(position) {
                continue;
            }
            // A borrowed formal is a root place holding the caller's storage for
            // the activation, declared with the reference's access; what it
            // holds at entry is entry content. A scalar referent is a place of
            // its primitive type, read and stored like a home.
            if let Some(borrowed) = signature.borrowed(position) {
                let (referent, scalar) = match *value_type {
                    ValueType::Record(record, _) => (record, false),
                    ValueType::Scalar(..) => (
                        self.records
                            .primitive(*value_type, borrowed.home.as_deref(), self.id)?,
                        true,
                    ),
                    ValueType::Rigid(_) => {
                        return Err(self.internal("a borrowed formal of a rigid type"));
                    }
                };
                let lane = self.position(formals.records.len())?;
                let (place, _) =
                    self.root(StructuralPlaceKind::Parameter { position: lane }, referent)?;
                if place.get() != formals.records.len() as u64 + 1 {
                    return Err(self.internal("place formals are not the first places"));
                }
                formals.records.push(StructuralParameterDeclaration {
                    place,
                    position: lane,
                    structural_type: referent,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: places::borrowed_access(borrowed.access)
                        .ok_or_else(|| self.internal("a write-only borrowed formal"))?,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                });
                formals.order.push(BoundaryParameterKind::Structural);
                self.borrowed_roots.push(place);
                *self.slot_mut(position)? = Some(if scalar {
                    Bound::Home(place)
                } else {
                    Bound::Record(place)
                });
                continue;
            }
            match *value_type {
                // A formal of a rigid type is a value like a scalar formal: it
                // has an identity, a call passes it, and nothing is known of it.
                ValueType::Scalar(..) | ValueType::Rigid(_) => {
                    let (scalar_type, rigid) = value_type
                        .declared()
                        .ok_or_else(|| self.internal("a value formal has no declaration"))?;
                    let value = self.tables.values.push(ValueRow {
                        scalar_type,
                        origin: ValueOrigin::MachineParameter {
                            position: self.position(position)?,
                        },
                        rigid,
                    });
                    // Clause propositions name each value formal by the
                    // identity `body::formal_values` gives it.
                    if formal_values.get(position) != Some(&Some(value)) {
                        return Err(self.internal("scalar formals are not the first values"));
                    }
                    formals.scalars.push(declaration(value, scalar_type));
                    formals.order.push(BoundaryParameterKind::Scalar);
                    *self.slot_mut(position)? = Some(Bound::Scalar(value));
                }
                // A record formal is a root place the machine owns for the
                // activation; what it holds at entry is entry content.
                ValueType::Record(record, _) => {
                    let lane = self.position(formals.records.len())?;
                    let (place, _) =
                        self.root(StructuralPlaceKind::Parameter { position: lane }, record)?;
                    // Clause terms name the `k`-th record formal as place
                    // `k + 1` (`Signature::requires`).
                    if place.get() != formals.records.len() as u64 + 1 {
                        return Err(self.internal("place formals are not the first places"));
                    }
                    formals.records.push(StructuralParameterDeclaration {
                        place,
                        position: lane,
                        structural_type: record,
                        multiplicity: self.records.multiplicity(record, self.id)?,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    });
                    formals.order.push(BoundaryParameterKind::Structural);
                    *self.slot_mut(position)? = Some(Bound::Record(place));
                }
            }
        }
        // The scalar result pseudo-value comes right after the scalar formals,
        // which is how `ensures` clauses name `result` (`Signature::result`). A
        // record result is the machine's result place, a root of its own.
        let result = match signature.result {
            None => TerminalMachineResult::Unit,
            Some(value_type @ (ValueType::Scalar(..) | ValueType::Rigid(_))) => {
                let (scalar_type, rigid) = value_type
                    .declared()
                    .ok_or_else(|| self.internal("a value result has no declaration"))?;
                let value = match rigid {
                    None => self.value(scalar_type, ValueOrigin::MachineResult),
                    Some(rigid) => self.rigid_value(rigid, ValueOrigin::MachineResult),
                };
                if value.get() != formals.scalars.len() as u64 + 1 {
                    return Err(self.internal("the result does not follow the scalar formals"));
                }
                TerminalMachineResult::Scalar(declaration(value, scalar_type))
            }
            Some(ValueType::Record(record, _)) => {
                let (place, _) = self.root(StructuralPlaceKind::Result, record)?;
                TerminalMachineResult::Structural(StructuralResultDeclaration {
                    place,
                    structural_type: record,
                    multiplicity: self.records.multiplicity(record, self.id)?,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: Vec::new(),
                })
            }
        };
        // The erased scalar formals come after the result pseudo-value. Each is
        // an invocation formal with a value identity and no runtime lane:
        // Terminal Psi declares it in the contract, not among the parameters
        // (`wiki/spec/terminal-psi/encoding.md`: "Erased formals occupy no tag").
        for position in &signature.erased {
            let Some(ValueType::Scalar(scalar_type, _)) =
                signature.parameters.get(*position).copied()
            else {
                return Err(refuse(
                    self.id,
                    Construct::Machine,
                    Unserved::Parameter("erased parameter of a data type"),
                ));
            };
            let value = self.value(
                scalar_type,
                ValueOrigin::MachineParameter {
                    position: self.position(*position)?,
                },
            );
            if formal_values.get(*position) != Some(&Some(value)) {
                return Err(self.internal("erased formals do not follow the result"));
            }
            formals.erased.push(declaration(value, scalar_type));
            *self.slot_mut(*position)? = Some(Bound::Erased(Witness::Value(value, scalar_type)));
        }
        Ok((formals, result))
    }
}

struct Lowering<'a> {
    id: MachineId,
    signature: &'a Signature,
    signatures: Signatures<'a>,
    outcomes: &'a IdTable<MachineId, Outcome>,
    records: &'a Records<'a>,
    /// What each root place is and the record type it holds, indexed by its
    /// `PlaceId`: the machine's structural place declarations.
    roots: IdTable<PlaceId, (StructuralPlaceKind, StructuralTypeId)>,
    /// The entry content value minted for a formal's scalar leaf, indexed by
    /// that leaf's `ExactPlaceId`; `None` until the leaf is first read.
    entry_contents: Vec<Option<ValueId>>,
    /// The branch decisions on the path to the open block, in program order:
    /// each the branch value equal to the Boolean its edge took. Every arm
    /// completes the machine, and a short-circuit or `match` rejoins its
    /// own branches, so the path to a statement is one sequence. A case a
    /// dispatch selected is not such a decision and is not here; see
    /// `selected`.
    path: Vec<Proposition>,
    tables: MachineTables,
    sources: MachineSources,
    blocks: IdTable<BlockId, Draft>,
    /// The open block, `None` once it is terminated and no block is entered.
    current: Option<BlockId>,
    /// The program point after the last thing emitted in `current`.
    point: Option<ProgramPoint>,
    slots: Vec<Option<Bound>>,
    /// The dispatch arms the open block lies in, outermost first: for a case
    /// arm the sum it selected on, the case, and the block parameter each
    /// scalar payload field arrived as.
    selected: Vec<places::Chosen>,
    /// The declared type of each slot, as the body states it.
    slot_types: &'a [ValueType],
    /// The `PublishedGuarantee` of each `ensures` clause, by position: the
    /// identity Terminal Psi's `ContractClause` holds.
    published: Vec<ObligationId>,
    /// The loop heads of this machine's call component, in member order;
    /// empty for a machine on no call cycle.
    heads: Vec<Head>,
    invariants: Vec<ScalarBlockInvariant>,
    /// The roots of this machine's borrowed formals: storage the caller
    /// lent, so a loan of a place beneath one reborrows the incoming loan
    /// (`Lender::Incoming`) rather than lending owned storage.
    borrowed_roots: Vec<PlaceId>,
    /// How many operands that only some paths evaluate and that rejoin
    /// (the right of `&&` and `||`, an arm of a value `match`) enclose what
    /// is being lowered. A home made there would hold the local on those
    /// paths only, so a loan that needs one is refused.
    joining: usize,
}

#[derive(Default)]
struct Draft {
    parameters: Vec<ValueDeclaration>,
    operations: Vec<Operation>,
    terminator: Option<Terminator>,
}

enum Value {
    Scalar(ValueId),
    /// A root place holding the record value, which nothing else names.
    Record(PlaceId),
    Unit,
}

fn declaration(id: ValueId, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id,
        scalar_type,
        qualifications: ScalarQualificationSetId::ZERO,
    }
}

fn successor(edge: EdgeId, target: BlockId, arguments: Vec<ValueId>) -> SuccessorEdge {
    SuccessorEdge {
        edge,
        target,
        arguments,
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    }
}

impl<'a> Lowering<'a> {
    /// Lowers one body's statements into the open block, and the implicit
    /// Unit completion when they fall off the end.
    fn body(&mut self, body: &Body) -> Checked<()> {
        for statement in &body.statements {
            self.statement(statement)?;
        }
        if self.open() {
            match self.signature.result {
                None => {
                    self.return_unit(SourceOrigin::Unpositioned(Unpositioned::ImplicitReturn))?
                }
                Some(_) => {
                    return Err(self.internal("a value machine fell off its last statement"));
                }
            }
        }
        Ok(())
    }

    /// Lowers the call component of machine `id` as a loop inside its graph.
    ///
    /// `wiki/spec/terminal-psi/control_flow.md`: "Constant-stack terminal
    /// cycles may become block backedges". Each member gets one head block
    /// whose parameters are its scalar formals, and its body is lowered once
    /// beneath it; the entry jumps to `id`'s own head with the machine's
    /// formals, and a terminal call of a member is a jump to that member's
    /// head (`control`, `arrive`). The entry itself is not a head:
    /// "The machine entry cannot carry a row: invocation supplies the
    /// machine's declared preconditions rather than an explicit control-flow
    /// edge."
    ///
    /// A member's `requires`, over its head's parameters, is that head's
    /// block invariant (control_flow.md: "A scalar block invariant is a
    /// semantic row identifying a machine, destination block, scalar
    /// predicate, and one obligation for every actual incoming edge."),
    /// because `wiki/spec/language/state_contracts.md` requires it of every
    /// arrival: "Machine `requires` applies at every invocation, including
    /// terminal and recursive calls."
    fn component(
        &mut self,
        id: MachineId,
        bodies: &'a IdTable<MachineId, Body>,
        component: &[MachineId],
        formals: &Formals,
    ) -> Checked<()> {
        for member in component {
            let signature = self.signatures.get((*member).into(), id)?;
            let block = self.block(SourceOrigin::Unpositioned(Unpositioned::LoopHead));
            self.sources.heads.push((block, *member));
            let mut parameters = Vec::with_capacity(signature.parameters.len());
            for value_type in &signature.parameters {
                let ValueType::Scalar(scalar_type, _) = *value_type else {
                    return Err(self.internal("a record formal on a call cycle"));
                };
                let position = self.position(parameters.len())?;
                let value =
                    self.value(scalar_type, ValueOrigin::BlockParameter { block, position });
                self.parameter(block, value, scalar_type)?;
                parameters.push(value);
            }
            if !signature.requires.is_empty() {
                let mut clauses = Vec::with_capacity(signature.requires.len());
                for clause in &signature.requires {
                    clauses.push(canonical::substitute(clause, &parameters, id)?);
                }
                self.invariants.push(ScalarBlockInvariant {
                    machine: id,
                    header: block,
                    predicate: canonical::conjunction(clauses, id)?,
                    arrivals: Vec::new(),
                });
            }
            self.heads.push(Head {
                member: *member,
                block,
                parameters,
            });
        }
        let entered: Vec<ValueId> = formals.scalars.iter().map(|formal| formal.id).collect();
        self.arrive(
            id,
            entered,
            SourceOrigin::Unpositioned(Unpositioned::MachineEntry),
        )?;
        for position in 0..self.heads.len() {
            let (member, block) = (self.heads[position].member, self.heads[position].block);
            let body = terminal_check_tables::row(bodies, member, id, Construct::Machine)?;
            // Each member's body names its own slots, and the branch
            // decisions before a head do not lie on every path to it.
            self.slot_types = &body.slots;
            self.slots = vec![None; body.slots.len()];
            for (slot, value) in self.heads[position]
                .parameters
                .clone()
                .into_iter()
                .enumerate()
            {
                *self.slot_mut(slot)? = Some(Bound::Scalar(value));
            }
            self.path.clear();
            self.enter(block);
            self.body(body)?;
        }
        Ok(())
    }

    fn internal(&self, detail: &str) -> Refusal {
        Refusal {
            reason: RefusalReason::Unsupported,
            machine: self.id,
            concerns: self.current.map_or(Construct::Machine, Construct::Block),
            detail: format!("completion {detail}"),
        }
    }

    fn position(&self, index: usize) -> Checked<u32> {
        u32::try_from(index).map_err(|_| self.internal("position exceeds u32"))
    }

    fn slot_mut(&mut self, slot: usize) -> Checked<&mut Option<Bound>> {
        let id = self.id;
        self.slots.get_mut(slot).ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: id,
            concerns: Construct::Machine,
            detail: String::from("slot is past the end of this body"),
        })
    }

    fn slot_type(&self, slot: usize) -> Checked<ValueType> {
        self.slot_types.get(slot).copied().ok_or(Refusal {
            reason: RefusalReason::ForeignIdentity,
            machine: self.id,
            concerns: Construct::Machine,
            detail: String::from("slot is past the end of this body"),
        })
    }

    fn value(&mut self, scalar_type: ScalarType, origin: ValueOrigin) -> ValueId {
        self.tables.values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        })
    }

    /// A value of a rigid type: declared with the stand-in the lowered form
    /// needs, and marked rigid in its row.
    fn rigid_value(&mut self, rigid: RigidType, origin: ValueOrigin) -> ValueId {
        self.tables.values.push(ValueRow {
            scalar_type: RigidType::DECLARED_AS,
            origin,
            rigid: Some(rigid),
        })
    }

    fn obligation(
        &mut self,
        kind: ObligationKind,
        created_at: ProgramPoint,
        source: SourceOrigin,
    ) -> ObligationId {
        let obligation = self
            .tables
            .obligations
            .push(ObligationRow { kind, created_at });
        self.sources.obligations.push(source);
        obligation
    }

    /// One `ArrivalInvariant` per edge that passes scalar arguments to a
    /// block, at that edge. control_flow.md: a block invariant has "one
    /// obligation for every actual incoming edge", and fact_validity.md:
    /// "Every arrival must check before the destination predicate can justify
    /// downstream obligations." Only a `Jump` or a `Conditional` successor
    /// passes arguments; a dispatch arm's parameters are bound from the
    /// payload (`Justification::DispatchBinding`), not by an argument. The
    /// predicate is the checker's; lowering only names the arrivals.
    fn arrivals(&mut self) -> Checked<()> {
        let mut arrivals = Vec::new();
        for (_, draft) in self.blocks.iter() {
            let successors: Vec<(EdgeId, BlockId, bool)> = match &draft.terminator {
                Some(Terminator::Jump {
                    edge,
                    target,
                    arguments,
                    ..
                }) => vec![(*edge, *target, !arguments.is_empty())],
                Some(Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                }) => vec![
                    (
                        when_true.edge,
                        when_true.target,
                        !when_true.arguments.is_empty(),
                    ),
                    (
                        when_false.edge,
                        when_false.target,
                        !when_false.arguments.is_empty(),
                    ),
                ],
                _ => Vec::new(),
            };
            for (edge, target, passes) in successors {
                if !passes {
                    continue;
                }
                let source = *self
                    .sources
                    .edges
                    .get(edge)
                    .ok_or_else(|| self.internal("an edge has no source row"))?;
                arrivals.push((edge, target, source));
            }
        }
        for (edge, target, source) in arrivals {
            let obligation = self.obligation(
                ObligationKind::ArrivalInvariant { edge },
                ProgramPoint::Edge(edge),
                source,
            );
            // A loop head's declared invariant lists every arrival it owes;
            // a join's predicate is the checker's and has no row here.
            for invariant in &mut self.invariants {
                if invariant.header == target {
                    invariant
                        .arrivals
                        .push(terminal_psi::ScalarBlockInvariantArrival { edge, obligation });
                }
            }
        }
        Ok(())
    }

    fn finish(
        mut self,
        entry: BlockId,
        formals: Formals,
        result: TerminalMachineResult,
    ) -> Checked<Lowered> {
        let ensures = self
            .signature
            .ensures
            .iter()
            .zip(&self.published)
            .map(|(proposition, obligation)| ContractClause {
                obligation: *obligation,
                proposition: proposition.clone(),
            })
            .collect();
        self.arrivals()?;
        let mut drafts = std::mem::take(&mut self.blocks);
        let mut blocks = Vec::with_capacity(drafts.len());
        for position in 1..=drafts.len() as u64 {
            let block =
                BlockId::new(position).ok_or_else(|| self.internal("block identity overflow"))?;
            let draft = drafts
                .get_mut(block)
                .ok_or_else(|| self.internal("block is past the end"))?;
            let operations = std::mem::take(&mut draft.operations);
            let block_parameters = std::mem::take(&mut draft.parameters);
            let Some(terminator) = draft.terminator.take() else {
                return Err(self.internal("a block was left without a terminator"));
            };
            let mut block = Block::plain(block, block_parameters, terminator);
            block.operations = operations;
            blocks.push(block);
        }
        let outcome =
            terminal_check_tables::row(self.outcomes, self.id, self.id, Construct::Machine)?;
        let crash_routes = outcome.crash_routes.clone();
        // The written ceiling beside the row callers rely on. They are the
        // same row when a clause is written; an omitted clause declares
        // nothing and publishes what the body's calls contribute.
        let declared_service_reach = match &self.signature.reaches {
            crate::Ceiling::Authored(ceiling) => ceiling.clone(),
            crate::Ceiling::Inferred => Vec::new(),
        };
        let published_service_ceiling = outcome.service_reach.clone();
        let published_synchronous_entries = outcome.synchronous_entries.clone();
        let contract = MachineContract {
            crash_routes,
            erased_scalar_formals: formals.erased,
            requires: self.signature.requires.clone(),
            ensures,
            ..MachineContract::empty(
                ContractId::new(self.id.get()).ok_or_else(|| self.internal("contract identity"))?,
            )
        };
        let structural_places = self
            .roots
            .iter()
            .map(|(id, (kind, _))| StructuralPlaceDeclaration { id, kind: *kind })
            .collect();
        let machine = TerminalMachine {
            assembly_target_requirement: RegisterTargetApplicability::Any,
            id: self.id,
            // Stated by `declare` for a compilation's module, from the typed
            // trees; lowering has only the closed body.
            declaration: None,
            attachment: None,
            nominal_cleanup_subject: None,
            parameters: formals.scalars,
            parameter_order: formals.order,
            structural_parameters: formals.records,
            ranked_scc: None,
            result,
            structural_places,
            entry_claims: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            declared_service_reach,
            closed_reach_application: None,
            published_synchronous_entries,
            published_service_ceiling,
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks,
            contract,
        };
        // A scalar machine has no structural place, so its place, root and
        // write tables are empty; `places` filled them for every record.
        Ok(Lowered {
            machine,
            tables: self.tables,
            sources: self.sources,
            invariants: self.invariants,
        })
    }
}
