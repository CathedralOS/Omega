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

// Propagate a completed path without disguising it as a failed judgment.
macro_rules! continuing {
    ($evaluation:expr) => {
        match $evaluation? {
            $crate::lower::Reached::Value(value) => value,
            $crate::lower::Reached::Ended(call) => {
                return Ok($crate::lower::Reached::Ended(call));
            }
        }
    };
}

mod calls;
pub(crate) use calls::publish_write_frames;
mod control;
mod operations;
pub(crate) mod places;
mod ranking;

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IdTable, MachineId, ObligationId, OperationId, PlaceId,
    Proposition, ScalarQualificationSetId, ScalarType, StructuralPlaceKind, StructuralTypeId,
    ValueId,
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
use crate::body::{Body, Signature, Signatures, Statement, ValueType};
use crate::canonical;
use crate::outcome::Outcome;
use crate::{MachineSources, SourceOrigin, Unpositioned, Unserved, refuse};

#[derive(Debug)]
pub(crate) struct Lowered {
    pub machine: TerminalMachine,
    pub tables: MachineTables,
    pub sources: MachineSources,
    pub designated: Vec<(OperationId, crate::DesignatedOperands)>,
    pub calls: Vec<(
        MachineId,
        typed_trees::expression::ExpressionHandle,
        typed_trees::statement::StatementHandle,
        semantic_vocabulary::InvocationSite,
    )>,
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
    structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Each authored formal keeps its own slot, whichever runtime lane it uses.
    bindings: Vec<Option<Bound>>,
}

/// Lowers machine `id`. `component` holds the members of its call component
/// when it lies on a call cycle (itself among them) and is empty otherwise;
/// `bodies` is read for those members only.
pub(crate) fn machine(
    id: MachineId,
    contract: ContractId,
    bodies: &IdTable<MachineId, Body>,
    component: &[MachineId],
    signatures: Signatures<'_>,
    outcomes: &IdTable<MachineId, Outcome>,
    records: &Records<'_>,
    requirement: bool,
    qualifications: &mut crate::qualifications::Qualifications,
) -> Checked<Lowered> {
    let body = terminal_check_tables::row(bodies, id, id, Construct::Machine)?;
    let signature = signatures.get(id.into(), id)?;
    let mut lowering = Lowering::new(
        id,
        signature,
        signatures,
        outcomes,
        records,
        &body.slots,
        qualifications,
    );
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
        return lowering.finish(entry, formals, result, contract);
    }
    if component.is_empty() {
        // Acyclic bodies still owe formation of an authored measure. In
        // particular, its partial arithmetic cannot disappear unchecked.
        lowering.begin_ranking(id)?;
        lowering.body(body)?;
    } else {
        lowering.component(id, bodies, component, &formals)?;
    }
    lowering.finish(entry, formals, result, contract)
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
    qualifications: &mut crate::qualifications::Qualifications,
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
        qualifications,
    );
    let (mut formals, mut result) = lowering.formals()?;
    for formal in &mut formals.scalars {
        lowering.qualify_declaration(formal)?;
    }
    if let TerminalMachineResult::Scalar(result) = &mut result {
        lowering.qualify_declaration(result)?;
    }
    let mut scalar_domain_requirements = Vec::new();
    for (index, formal) in formals.scalars.iter().enumerate() {
        for domain in lowering
            .qualifications
            .members(caller, formal.qualifications)?
        {
            scalar_domain_requirements.push(terminal_psi::ScalarDomainRequirement {
                argument_index: u32::try_from(index)
                    .map_err(|_| unavailable("boundary scalar formal ordinal overflow"))?,
                domain: *domain,
            });
        }
    }
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
            terminal_psi::BoundaryMachineResult::Scalar(semantic_vocabulary::QualifiedScalarType {
                scalar_type: result.scalar_type,
                qualifications: result.qualifications,
            })
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
        // Scalar qualifications and predicates belong to this interface,
        // independently of any selected provider's implementation.
        requires: Vec::new(),
        normal_qualifications: Vec::new(),
        scalar_domain_requirements,
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

fn contract_header(
    id: ContractId,
    signature: &Signature,
    formals: Formals,
    result: TerminalMachineResult,
    ensures: Vec<ContractClause>,
    outcome: &Outcome,
) -> MachineContract {
    MachineContract {
        selected_application: None,
        parameters: formals.scalars,
        parameter_order: formals.order,
        structural_parameters: formals.records,
        result,
        declared_service_reach: match &signature.reaches {
            crate::Ceiling::Authored(ceiling) => Some(ceiling.clone()),
            crate::Ceiling::Inferred => None,
        },
        declared_synchronous_entries: match &signature.invokes {
            crate::Ceiling::Authored(ceiling) => Some(ceiling.clone()),
            crate::Ceiling::Inferred => None,
        },
        declared_crash_routes: signature.declared_crash_routes().map(<[_]>::to_vec),
        declared_suspension: signature.operational.suspension,
        declared_blocking: signature.operational.blocking,
        published_may_suspend: outcome.operational[0],
        published_may_block: outcome.operational[1],
        published_service_ceiling: outcome.service_reach.clone(),
        published_synchronous_entries: outcome.synchronous_entries.clone(),
        termination: match signature.termination {
            crate::Termination::Silent => semantic_vocabulary::Termination::NoGuarantee,
            crate::Termination::Promised => semantic_vocabulary::Termination::Terminates,
        },
        crash_routes: outcome.crash_routes.clone(),
        erased_scalar_formals: formals.erased,
        requires: signature.requires.clone(),
        ensures,
        ..MachineContract::empty(id)
    }
}

/// Read a requirement's full header through the same formal construction as a
/// machine, without allocating a block or claiming a body. An omitted effect
/// clause stays open in the declared fields; it gains no empty-effect promise.
pub(crate) fn declaration_contract(
    caller: MachineId,
    id: ContractId,
    signature: &Signature,
    selected_body: Option<MachineId>,
    signatures: Signatures<'_>,
    outcomes: &IdTable<MachineId, Outcome>,
    records: &Records<'_>,
    qualifications: &mut crate::qualifications::Qualifications,
) -> Checked<MachineContract> {
    let selected = selected_body
        .map(|body| terminal_check_tables::row(outcomes, body, caller, Construct::Machine))
        .transpose()?;
    let outcome = Outcome {
        operational: signature
            .operational
            .published(selected.map_or([false; 2], |outcome| outcome.operational)),
        completes_normally: selected.is_none_or(|outcome| outcome.completes_normally),
        crash_routes: signature
            .declared_crash_routes()
            .or_else(|| selected.map(|outcome| outcome.crash_routes.as_slice()))
            .unwrap_or_default()
            .to_vec(),
        service_reach: match &signature.reaches {
            crate::Ceiling::Authored(row) => row.clone(),
            crate::Ceiling::Inferred => selected
                .map(|outcome| outcome.service_reach.clone())
                .unwrap_or_default(),
        },
        synchronous_entries: match &signature.invokes {
            crate::Ceiling::Authored(row) => row.clone(),
            crate::Ceiling::Inferred => selected
                .map(|outcome| outcome.synchronous_entries.clone())
                .unwrap_or_default(),
        },
    };
    let mut lowering = Lowering::new(
        caller,
        signature,
        signatures,
        outcomes,
        records,
        &signature.parameters,
        qualifications,
    );
    let (formals, result) = lowering.formals()?;
    let ensures = signature
        .ensures
        .iter()
        .enumerate()
        .map(|(position, proposition)| {
            let obligation = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(ObligationId::new)
                .ok_or_else(|| lowering.internal("public guarantee coordinate overflow"))?;
            Ok(ContractClause {
                obligation,
                proposition: proposition.clone(),
            })
        })
        .collect::<Checked<Vec<_>>>()?;
    let mut contract = contract_header(id, signature, formals, result, ensures, &outcome);
    lowering.qualify_contract(&mut contract)?;
    Ok(contract)
}

impl<'a> Lowering<'a> {
    fn new(
        id: MachineId,
        signature: &'a Signature,
        signatures: Signatures<'a>,
        outcomes: &'a IdTable<MachineId, Outcome>,
        records: &'a Records<'a>,
        slot_types: &'a [ValueType],
        qualifications: &'a mut crate::qualifications::Qualifications,
    ) -> Self {
        let coercions_start = qualifications.catalog.coercions.len();
        Self {
            id,
            signature,
            signatures,
            outcomes,
            records,
            roots: IdTable::default(),
            demanded_contents: Vec::new(),
            path: Vec::new(),
            tables: MachineTables::default(),
            sources: MachineSources::default(),
            designated: Vec::new(),
            calls: Vec::new(),
            call_formations: Vec::new(),
            blocks: IdTable::default(),
            operation_locations: IdTable::default(),
            current: None,
            point: None,
            slots: vec![None; slot_types.len()],
            selected: Vec::new(),
            slot_types,
            qualifications,
            coercions_start,
            value_qualifications: Vec::new(),
            published: Vec::new(),
            heads: Vec::new(),
            invariants: Vec::new(),
            borrowed_roots: Vec::new(),
            exclusive_roots: Vec::new(),
            joining: 0,
            fallthrough: None,
            block_ranks: IdTable::default(),
            active_rank: None,
            successor_ranks: Vec::new(),
        }
    }
}

impl Lowering<'_> {
    /// A body and an interface declare the same scalar and storage formals.
    /// Construct those identities once without inventing executable blocks
    /// or guarantees for a bodyless declaration.
    /// Machine entries and loop heads bind the same authored formals. Only
    /// their declaration site differs; access and multiplicity stay unchanged.
    fn parameter_formals(
        &mut self,
        signature: &Signature,
        block: Option<BlockId>,
    ) -> Checked<Formals> {
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
                    ValueType::Record(record, _, _) => (record, false),
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
                let kind =
                    block.map_or(StructuralPlaceKind::Parameter { position: lane }, |block| {
                        StructuralPlaceKind::BlockParameter {
                            block,
                            position: lane,
                        }
                    });
                let (place, _) = self.root(kind, referent)?;
                if block.is_none() && place.get() != formals.records.len() as u64 + 1 {
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
                if borrowed.access == language_core::ReferenceAccess::Mutable {
                    self.exclusive_roots.push(place);
                }
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
                    let origin = match block {
                        Some(block) => ValueOrigin::BlockParameter {
                            block,
                            position: self.position(formals.scalars.len())?,
                        },
                        None => ValueOrigin::MachineParameter {
                            position: self.position(position)?,
                        },
                    };
                    let value = self.tables.values.push(ValueRow {
                        scalar_type,
                        origin,
                        rigid,
                    });
                    if let Some(block) = block {
                        self.parameter(block, value, scalar_type)?;
                    }
                    // Clause propositions name each value formal by the
                    // identity `body::formal_values` gives it.
                    if block.is_none() && formal_values.get(position) != Some(&Some(value)) {
                        return Err(self.internal("scalar formals are not the first values"));
                    }
                    formals.scalars.push(declaration(value, scalar_type));
                    self.qualify(value, value_type.qualification())?;
                    formals.order.push(BoundaryParameterKind::Scalar);
                    *self.slot_mut(position)? = Some(Bound::Scalar(value));
                }
                // A record formal is a root place the machine owns for the
                // activation; what it holds at entry is entry content.
                ValueType::Record(record, _, _) => {
                    let lane = self.position(formals.records.len())?;
                    let kind =
                        block.map_or(StructuralPlaceKind::Parameter { position: lane }, |block| {
                            StructuralPlaceKind::BlockParameter {
                                block,
                                position: lane,
                            }
                        });
                    let (place, _) = self.root(kind, record)?;
                    // Clause terms name the `k`-th record formal as place
                    // `k + 1` (`Signature::requires`).
                    if block.is_none() && place.get() != formals.records.len() as u64 + 1 {
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
        if let Some(block) = block {
            self.blocks
                .get_mut(block)
                .ok_or_else(|| Refusal {
                    reason: RefusalReason::ForeignIdentity,
                    machine: self.id,
                    concerns: Construct::Block(block),
                    detail: String::from("a parameter block is missing"),
                })?
                .structural_parameters = formals.records.clone();
        }
        Ok(formals)
    }

    fn formals(&mut self) -> Checked<(Formals, TerminalMachineResult)> {
        let signature = self.signature;
        let mut formals = self.parameter_formals(signature, None)?;
        let formal_values = signature.formal_values();
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
                self.qualify(value, value_type.qualification())?;
                TerminalMachineResult::Scalar(declaration(value, scalar_type))
            }
            Some(ValueType::Record(record, _, _)) => {
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
            self.qualify(value, signature.parameters[*position].qualification())?;
            *self.slot_mut(*position)? = Some(Bound::Erased(Witness::Value(value, scalar_type)));
        }
        let bindings: Vec<_> = formals
            .records
            .iter()
            .map(|formal| {
                self.exact_root(formal.place)
                    .map(|exact| (formal.place, exact))
            })
            .collect::<Checked<_>>()?;
        for clause in signature.requires.iter().chain(&signature.ensures) {
            self.name_clause_contents(
                clause,
                &bindings,
                result.structural().map(|result| result.place),
            )?;
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
    qualifications: &'a mut crate::qualifications::Qualifications,
    /// The first qualification edge minted by this machine, carried from
    /// construction instead of searching all preceding machines' edges.
    coercions_start: usize,
    /// Complete declared types indexed by the values this lowering minted.
    /// Synthetic proof temporaries have no qualification; expression results
    /// and formals set theirs through the same owner before publication.
    value_qualifications: Vec<Option<crate::body::Qualification>>,
    /// What each root place is and the record type it holds, indexed by its
    /// `PlaceId`: the machine's structural place declarations.
    roots: IdTable<PlaceId, (StructuralPlaceKind, StructuralTypeId)>,
    /// Scalar leaves requested by operations and clauses, indexed by exact
    /// place. Final lowering allocates their entry and returned snapshots.
    demanded_contents: Vec<Option<ScalarType>>,
    /// The branch decisions on the path to the open block, in program order:
    /// each the branch value equal to the Boolean its edge took. Every arm
    /// completes the machine, and a short-circuit or `match` rejoins its
    /// own branches, so the path to a statement is one sequence. A case a
    /// dispatch selected is not such a decision and is not here; see
    /// `selected`.
    path: Vec<Proposition>,
    tables: MachineTables,
    sources: MachineSources,
    designated: Vec<(OperationId, crate::DesignatedOperands)>,
    calls: Vec<(
        MachineId,
        typed_trees::expression::ExpressionHandle,
        typed_trees::statement::StatementHandle,
        semantic_vocabulary::InvocationSite,
    )>,
    call_formations: Vec<terminal_psi::CallFormation>,
    blocks: IdTable<BlockId, Draft>,
    /// Coordinates of emitted operations in this in-progress machine. A call
    /// reads exact scalar definitions without rescanning all earlier blocks.
    operation_locations: IdTable<OperationId, Option<(BlockId, usize)>>,
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
    /// lent, in increasing place order. Formals allocate roots in that order.
    /// A loan of a place beneath one reborrows the incoming loan
    /// (`Lender::Incoming`) rather than lending owned storage.
    borrowed_roots: Vec<PlaceId>,
    /// Those of `borrowed_roots` lent with `&mut`: the only borrowed storage
    /// an owned value may be moved out of (`places::record`).
    exclusive_roots: Vec<PlaceId>,
    /// How many operands that only some paths evaluate and that rejoin
    /// (the right of `&&` and `||`, an arm of a value `match`) enclose what
    /// is being lowered. A home made there would hold the local on those
    /// paths only, so a loan that needs one is refused.
    joining: usize,
    /// The block a guarded arm's guard turns away to, when nothing has been
    /// lowered into it yet: an unguarded arm that begins there is taken
    /// only on that edge and is an authored branch too.
    fallthrough: Option<crate::Branch>,
    /// A state-head measure remains the same observation in its implementation
    /// blocks. Tail transfers separately measure the successor's actual values.
    block_ranks: IdTable<BlockId, Option<Vec<terminal_psi::TerminalIntegerRankValue>>>,
    active_rank: Option<Vec<terminal_psi::TerminalIntegerRankValue>>,
    successor_ranks: Vec<(
        BlockId,
        MachineId,
        Vec<terminal_psi::TerminalIntegerRankValue>,
    )>,
}

/// What lowering an evaluation reached: its result, or the expression with
/// no normal result at which every path of it ended.
#[derive(Debug)]
pub(super) enum Reached<T> {
    Value(T),
    Ended(SourceOrigin),
}

/// An ended path is a successful lowering outcome, not a refusal.
pub(super) type Evaluation<T> = Checked<Reached<T>>;

impl<T> Reached<T> {
    fn map<U>(self, transform: impl FnOnce(T) -> U) -> Reached<U> {
        match self {
            Self::Value(value) => Reached::Value(transform(value)),
            Self::Ended(call) => Reached::Ended(call),
        }
    }
}

#[derive(Default)]
struct Draft {
    parameters: Vec<ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
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
    /// A real owned value whose declared domain excludes every inhabitant.
    /// A borrowed referent, provider attachment or result placeholder is not
    /// such a value and cannot eliminate its consumer's continuation.
    fn empty_owned_value(&self, root: PlaceId) -> bool {
        self.roots.get(root).is_some_and(|(kind, ty)| {
            !matches!(
                kind,
                StructuralPlaceKind::Result | StructuralPlaceKind::ProviderAttachment { .. }
            ) && self.borrowed_roots.binary_search(&root).is_err()
                && self.records.inhabitation.status(*ty)
                    == terminal_semantics::inhabitation::Status::ProvedEmpty
        })
    }

    fn qualify(
        &mut self,
        value: ValueId,
        qualification: crate::body::Qualification,
    ) -> Checked<()> {
        self.value_qualifications
            .resize(self.tables.values.len(), None);
        let slot = usize::try_from(value.get() - 1)
            .ok()
            .and_then(|index| self.value_qualifications.get_mut(index))
            .ok_or_else(|| terminal_check_tables::Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine: self.id,
                concerns: Construct::Machine,
                detail: "a qualification names a value outside this lowering".into(),
            })?;
        if slot.is_some_and(|previous| previous != qualification) {
            return Err(self.internal("a qualification change reused its operand's value identity"));
        }
        *slot = Some(qualification);
        Ok(())
    }

    fn qualify_declaration(&mut self, value: &mut ValueDeclaration) -> Checked<()> {
        let qualification = self
            .value_qualifications
            .get((value.id.get() - 1) as usize)
            .copied()
            .flatten()
            .unwrap_or(crate::body::Qualification::NONE);
        value.qualifications =
            self.qualifications
                .scalar(self.id, value.scalar_type, qualification)?;
        Ok(())
    }

    fn qualify_contract(&mut self, contract: &mut MachineContract) -> Checked<()> {
        for value in contract
            .parameters
            .iter_mut()
            .chain(&mut contract.erased_scalar_formals)
        {
            self.qualify_declaration(value)?;
        }
        if let TerminalMachineResult::Scalar(value) = &mut contract.result {
            self.qualify_declaration(value)?;
        }
        Ok(())
    }

    /// Lowers one body's statements into the open block, and the implicit
    /// Unit completion when they fall off the end.
    ///
    /// `wiki/spec/language/state_contracts.md`, "Statements after a call
    /// that never returns": "When checked call information excludes normal
    /// return, the call has no normal successor." A statement every path of
    /// which ends at such a call, written as the statement itself or inside
    /// its expression (`let x: u32 = never();`), ends the body there, and
    /// the statements after it are not lowered: "Unreachable statements
    /// contribute no executed continuation, no synthetic Unit result, no
    /// cleanup edge after the call, no ownership frontier, and no `ensures`
    /// fact handed to a surviving continuation", and they owe "no fresh
    /// proofs, resources or ownership dispositions". They are source all the
    /// same, and the stages before this one checked their names and types
    /// and the reader read them. The evidence is the callee's outcome
    /// (`completes_normally`): a callee some path of which returns keeps its
    /// continuation, and so does a statement some path of which goes on past
    /// the call (the right operand of `&&`, one arm of a `match`).
    fn body(&mut self, body: &Body) -> Checked<()> {
        let mut statements = body.statements.iter();
        while let Some(statement) = statements.next() {
            let lowered = self.statement(statement);
            let Reached::Ended(call) = lowered? else {
                continue;
            };
            // An erased binding evaluates nothing, so it is no executable
            // statement and none is reported for it.
            let first = statements.find_map(|statement| match statement {
                Statement::Witness { .. } => None,
                Statement::Bind { value, .. } | Statement::Evaluate(value) => Some(value.source),
                Statement::Declare { source, .. }
                | Statement::Store { source, .. }
                | Statement::Dispatch { source, .. }
                | Statement::Arm { source, .. } => Some(*source),
                Statement::CaptureReference { .. } => {
                    Some(SourceOrigin::Unpositioned(Unpositioned::SpanAbsent))
                }
            });
            if let Some(statement) = first {
                self.sources
                    .unreachable
                    .push(crate::Unreachable { statement, call });
            }
            break;
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
        let entered_bindings = self.slots.clone();
        for member in component {
            let signature = self.signatures.get((*member).into(), id)?;
            let block = self.block(SourceOrigin::Unpositioned(Unpositioned::LoopHead));
            self.sources.heads.push((block, *member));
            self.slots = vec![None; signature.parameters.len()];
            let parameters = self.parameter_formals(signature, Some(block))?;
            let scalars: Vec<_> = parameters.scalars.iter().map(|formal| formal.id).collect();
            let roots: Vec<_> = parameters
                .records
                .iter()
                .map(|formal| formal.place)
                .collect();
            if !signature.requires.is_empty() {
                let mut clauses = Vec::with_capacity(signature.requires.len());
                for clause in &signature.requires {
                    clauses.push(canonical::substitute_parameters(
                        clause, &scalars, &roots, id,
                    )?);
                }
                self.invariants.push(ScalarBlockInvariant {
                    machine: id,
                    header: block,
                    predicate: canonical::conjunction(clauses, id)?,
                    arrivals: Vec::new(),
                });
            }
            let bindings = parameters
                .records
                .iter()
                .enumerate()
                .map(|(position, formal)| {
                    let source = PlaceId::new(position as u64 + 1)
                        .ok_or_else(|| self.internal("structural formal identity overflow"))?;
                    Ok((source, self.exact_root(formal.place)?))
                })
                .collect::<Checked<Vec<_>>>()?;
            for clause in &signature.requires {
                self.name_clause_contents(clause, &bindings, None)?;
            }
            self.heads.push(Head {
                member: *member,
                block,
                parameters: scalars,
                structural_parameters: parameters.records,
                bindings: self.slots.clone(),
            });
        }
        self.slots = entered_bindings;
        let entered: Vec<ValueId> = formals.scalars.iter().map(|formal| formal.id).collect();
        let structural = formals
            .records
            .iter()
            .map(|formal| terminal_psi::StructuralArgument::whole(formal.place, formal.access))
            .collect();
        self.arrive(
            id,
            entered,
            structural,
            Vec::new(),
            SourceOrigin::Unpositioned(Unpositioned::MachineEntry),
            None,
        )?;
        for position in 0..self.heads.len() {
            let (member, block) = (self.heads[position].member, self.heads[position].block);
            let body = terminal_check_tables::row(bodies, member, id, Construct::Machine)?;
            // Every head binds its own arguments. Branch facts from a prior
            // invocation cannot enter the next one without an invariant.
            self.slot_types = &body.slots;
            self.slots = self.heads[position].bindings.clone();
            self.slots.resize(body.slots.len(), None);
            self.path.clear();
            self.enter(block);
            self.begin_ranking(member)?;
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
                    structural_arguments,
                    ..
                }) => vec![(
                    *edge,
                    *target,
                    !arguments.is_empty() || !structural_arguments.is_empty(),
                )],
                Some(Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                }) => vec![
                    (
                        when_true.edge,
                        when_true.target,
                        !when_true.arguments.is_empty()
                            || !when_true.structural_arguments.is_empty(),
                    ),
                    (
                        when_false.edge,
                        when_false.target,
                        !when_false.arguments.is_empty()
                            || !when_false.structural_arguments.is_empty(),
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
        contract_id: ContractId,
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
        self.name_storage_contents()?;
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
            block.structural_parameters = std::mem::take(&mut draft.structural_parameters);
            block.operations = operations;
            blocks.push(block);
        }
        let outcome =
            terminal_check_tables::row(self.outcomes, self.id, self.id, Construct::Machine)?;
        let mut contract = contract_header(
            contract_id,
            self.signature,
            formals,
            result,
            ensures,
            outcome,
        );
        self.qualify_contract(&mut contract)?;
        for block in &mut blocks {
            for value in &mut block.parameters {
                self.qualify_declaration(value)?;
            }
            for operation in &mut block.operations {
                if let terminal_psi::OperationResult::Scalar(value) = &mut operation.result {
                    self.qualify_declaration(value)?;
                }
            }
        }
        let structural_places = self
            .roots
            .iter()
            .map(|(id, (kind, _))| StructuralPlaceDeclaration { id, kind: *kind })
            .collect();
        self.call_formations.sort_unstable_by_key(|row| row.site);
        let mut machine = TerminalMachine {
            assembly_target_requirement: RegisterTargetApplicability::Any,
            id: self.id,
            attachment: None,
            nominal_cleanup_subject: None,
            ranked_scc: None,
            structural_places,
            call_formations: std::mem::take(&mut self.call_formations),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks,
            contract,
        };
        self.publish_rankings(&mut machine)?;
        self.qualifications
            .complete(&machine, self.coercions_start)?;
        // A scalar machine has no structural place, so its place, root and
        // write tables are empty; `places` filled them for every record.
        Ok(Lowered {
            machine,
            tables: self.tables,
            sources: self.sources,
            designated: self.designated,
            calls: self.calls,
            invariants: self.invariants,
        })
    }
}
