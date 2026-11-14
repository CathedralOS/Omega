/*
The closed list of obligations. An obligation is an identity, not a fact.
Two families share one table and one identity space:

- Conserved obligations (claims, loans, windows, debts) live across edges.
  Every edge accounts for each incoming one exactly once
  (`edges::Disposition`), and none is ever dropped silently.
- Proof questions are reconstructed at one site and discharged there, once,
  by evidence; they do not travel along edges.

This enum is the one extension point for obligations. A construct-family
lane adds a variant here, with the specification sentence that creates it,
and a row in the crate README. It never adds a table or a key form.
*/

use language_core::{Multiplicity, ReferenceAccess};
use semantic_vocabulary::{
    BlockId, ClaimId, CrashCause, CycleComponentId, DomainSemanticId, EdgeId, ObligationId,
    OperationId, ValueId,
};

use crate::{ExactPlaceId, InvocationId, ProgramPoint};

/// One obligation, indexed by the lowered form's `ObligationId`. Elaboration
/// mints an `ObligationId` for every obligation it reconstructs, of either
/// family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObligationRow {
    pub kind: ObligationKind,
    pub created_at: ProgramPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationKind {
    // Conserved obligations.
    /// An owned claim. `wiki/spec/terminal-psi/ownership.md`: "Structural
    /// ownership is an exact live set of roots, projected claims, and
    /// remaining subtrees". `established_at` is where it was established, not
    /// its current home: "A claim declaration's path describes its
    /// establishment, not its permanent location." `frontier` is the
    /// lowered form's claim identity for the same claim.
    Claim {
        established_at: ExactPlaceId,
        multiplicity: Multiplicity,
        frontier: ClaimId,
    },
    /// A loan. `wiki/spec/terminal-psi/loans.md`: "Loan resource | Exact
    /// occurrence, owner lineage, captured place, access, parent lifetime,
    /// and restoration obligation." The occurrence is `created_at`.
    Loan {
        captured: ExactPlaceId,
        access: ReferenceAccess,
        lender: Lender,
    },
    /// Restoration debt on a hole in borrowed storage.
    /// `wiki/spec/language/ownership.md`: "The move transfers the removed
    /// value's exact custody to its destination and records a restoration
    /// obligation on the original place and loan."
    RestorationWindow {
        hole: ExactPlaceId,
        loan: ObligationId,
    },
    /// A domain that a write broke and that must be proved again.
    /// `wiki/spec/language/dependent_values.md`: "Otherwise the write opens a
    /// window on the place, whose actual new contents remain available to
    /// ordinary flow reasoning. The domain must be proved again at the next
    /// consumption point". Distinct from `RestorationWindow`: "Ownership
    /// restoration and domain restoration are separate obligations;
    /// satisfying one cannot discharge the other."
    DomainDebt {
        place: ExactPlaceId,
        domain: DomainSemanticId,
    },
    /// Storage that must be established as a whole valid value before typed
    /// use. dependent_values.md: "Exclusive storage recasts may open initial
    /// target-establishment debt before a valid target value exists."
    EstablishmentDebt { place: ExactPlaceId },

    // Proof questions.
    /// A used static integer width is positive at its owning entry.
    /// `wiki/spec/language/numeric_values.md`, "Width-indexed integers":
    /// "Generic formation proves positivity under the declaration's static
    /// requirements." The subject is the actual const u64 input, not a
    /// runtime formal or a value supplied by a current specialization.
    IntegerWidthPositive { parameter: ValueId },
    /// One callee `requires` clause at one invocation, by position.
    /// `wiki/spec/terminal-psi/fact_validity.md`: "Call coverage reconstructs
    /// one exact obligation for every callee `requires` clause."
    CallRequirement {
        invocation: InvocationId,
        clause: u32,
    },
    /// The safety of a partial operation, proved before it executes.
    /// `wiki/spec/terminal-psi/control_flow.md`: "Each partial operation still
    /// proves its own safety from facts available before it executes".
    OperationSafety { site: ExecutionSite },
    /// One clause of a data type's `requires`, owed by one operation that
    /// forms or changes a value of that type: a record or case construction,
    /// a zero establishment, or a store into a field of a place of that type
    /// or of a type that contains it. `wiki/spec/language/data_and_literals.md`:
    /// "Construction and every restoring write prove the owning
    /// default-domain clauses." The goal is the clause with `self` replaced
    /// by the values the place holds once the operation completes, proved
    /// from what is known before it.
    /// `wiki/spec/terminal-psi/structural_access.md`: "The obligation binds
    /// the initializer or replacement SSA value, never the old field
    /// contents."
    ///
    /// One operation can owe clauses of several types: a store to `l.from.x`
    /// owes `Point`'s for `l.from` and `Line`'s for `l`. `clause` is the
    /// position in the operation's whole list: the types that own the
    /// written path from the innermost outward, or for a zero establishment
    /// the data-typed places of the zero value in declaration order, parent
    /// first; within one type its clauses in authored order.
    DataDomain { operation: OperationId, clause: u32 },
    /// A block invariant on one incoming edge. control_flow.md: "A scalar
    /// block invariant is a semantic row identifying a machine, destination
    /// block, scalar predicate, and one obligation for every actual incoming
    /// edge."
    ArrivalInvariant { edge: EdgeId },
    /// One published guarantee, by position, on one normal exit edge.
    /// control_flow.md: "All normal returns, including returns after a cycle,
    /// must establish the published guarantees." The real obligation is per
    /// return edge; `published` names the clause's `PublishedGuarantee`, the
    /// identity the module's `ContractClause` carries.
    NormalGuarantee {
        exit: EdgeId,
        clause: u32,
        published: ObligationId,
    },
    /// One published `ensures` clause, by position: the per-clause identity
    /// Terminal Psi's `ContractClause` holds. It holds exactly when every
    /// normal return edge's `NormalGuarantee` for this clause holds, which is
    /// vacuously so for a machine with no normal return (control_flow.md:
    /// "All normal returns ... must establish the published guarantees").
    /// It is discharged from those rows, never proved on its own.
    PublishedGuarantee { clause: u32 },
    /// A crash site's guard and same-cause route coverage, one per site and
    /// cause. `wiki/spec/terminal-psi/calls_and_outcomes.md`: "The verifier
    /// proves every site guard from independent entry/CFG facts and checks
    /// same-cause coverage".
    CrashCoverage { site: CrashSite, cause: CrashCause },
    /// Well-foundedness of one recursive component's rank relation.
    /// control_flow.md: "One grouped control judgment covers each
    /// reconstructed component, with a shared well-foundedness question and
    /// comparison obligations for every internal edge."
    WellFoundedness { component: CycleComponentId },
    /// The rank comparison on one internal edge of that component.
    RankComparison {
        component: CycleComponentId,
        edge: EdgeId,
    },
}

/// The instruction or terminator whose execution consumes an obligation.
/// A terminator is identified by its block; an operation has its own identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionSite {
    Block(BlockId),
    Operation(OperationId),
}

/// Crash coverage uses the same instruction and terminator coordinates.
pub type CrashSite = ExecutionSite;

/// Where a loan's authority comes from. loans.md: "A direct reborrow names
/// its immediate parent resource, exact formation event, and projection. A
/// multihop chain cannot skip intermediate parents."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lender {
    /// Lent directly from owned storage at the captured place's root.
    Owner,
    /// Reborrowed from this immediate parent loan.
    Loan(ObligationId),
    /// Reborrowed from the loan the invocation passed in at the captured
    /// place's root, a borrowed formal. That parent resource is the
    /// caller's, so this machine has no row for it and names it by the
    /// root; its access is the formal's declared access. loans.md: "A
    /// direct reborrow names its immediate parent resource, exact formation
    /// event, and projection."
    Incoming,
}

impl ObligationKind {
    /// True for the family every edge must account for exactly once.
    pub const fn is_conserved(&self) -> bool {
        matches!(
            self,
            Self::Claim { .. }
                | Self::Loan { .. }
                | Self::RestorationWindow { .. }
                | Self::DomainDebt { .. }
                | Self::EstablishmentDebt { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::PsiSemanticId;

    #[test]
    fn resources_are_conserved_and_questions_are_not() {
        let place = ExactPlaceId::new(1).unwrap();
        let edge = EdgeId::new(1).unwrap();
        let loan = ObligationKind::Loan {
            captured: place,
            access: ReferenceAccess::Mutable,
            lender: Lender::Owner,
        };
        let window = ObligationKind::RestorationWindow {
            hole: place,
            loan: ObligationId::new(1).unwrap(),
        };
        assert!(loan.is_conserved() && window.is_conserved());
        assert!(!ObligationKind::ArrivalInvariant { edge }.is_conserved());
        assert!(
            !ObligationKind::DataDomain {
                operation: OperationId::new(1).unwrap(),
                clause: 0,
            }
            .is_conserved()
        );
        assert!(
            !ObligationKind::NormalGuarantee {
                exit: edge,
                clause: 0,
                published: ObligationId::new(1).unwrap(),
            }
            .is_conserved()
        );
    }
}

/// The actual width inputs each machine's declared carriers use, in machine
/// order, including empty rows. This is an inventory, not a positivity fact.
/// Header storage is followed even when unread or behind a zero-sized array:
/// type formation does not depend on whether execution observes a value.
pub fn integer_width_parameters(
    module: &terminal_psi::TerminalModule,
) -> Result<
    Vec<(semantic_vocabulary::MachineId, Vec<ValueId>)>,
    (semantic_vocabulary::MachineId, &'static str),
> {
    use semantic_vocabulary::StructuralPlaceKind;
    use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};
    use terminal_psi::TerminalMachineResult;

    let diagnostic = module
        .machines
        .first()
        .map(|machine| machine.id)
        .unwrap_or_else(|| MachineId::new(1).expect("one is an identity"));
    let mut catalog = module.structural_types.iter().collect::<Vec<_>>();
    catalog.sort_unstable_by_key(|declaration| declaration.id);
    if catalog.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err((
            diagnostic,
            "integer width inventory has duplicate structural types",
        ));
    }
    let mut referents = module
        .structural_referents
        .iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    referents.sort_unstable();
    if referents.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err((
            diagnostic,
            "integer width inventory has duplicate referent identities",
        ));
    }

    // Resolve the graph once, including dormant references. Closed branches
    // can then be skipped without concealing a missing declaration. Flat
    // edge/seed ranges are indexed by this actual catalog, never by raw IDs.
    let mut children = Vec::new();
    let mut child_ranges = Vec::with_capacity(catalog.len());
    let mut seeds = Vec::new();
    let mut seed_ranges = Vec::with_capacity(catalog.len());
    let mut roots = Vec::new();
    for declaration in &catalog {
        roots.clear();
        let seed_start = seeds.len();
        width_type_parts(declaration, &mut roots, &mut seeds);
        seed_ranges.push(seed_start..seeds.len());
        let child_start = children.len();
        for child in &roots {
            if let Some(index) = width_type_index(*child, &catalog, &referents, diagnostic)? {
                children.push(index);
            }
        }
        child_ranges.push(child_start..children.len());
    }
    let mut symbolic = seed_ranges
        .iter()
        .map(|range| !range.is_empty())
        .collect::<Vec<_>>();
    let mut pending = Vec::new();
    if !seeds.is_empty() {
        // Ordinary closed modules need neither reverse adjacency nor a
        // per-machine graph walk after the one reference-validation pass.
        let mut reverse = Vec::with_capacity(children.len());
        for (parent, range) in child_ranges.iter().enumerate() {
            reverse.extend(children[range.clone()].iter().map(|child| (*child, parent)));
        }
        reverse.sort_unstable();
        let mut reverse_ranges = Vec::with_capacity(catalog.len());
        let mut cursor = 0;
        for child in 0..catalog.len() {
            let start = cursor;
            while reverse.get(cursor).is_some_and(|(at, _)| *at == child) {
                cursor += 1;
            }
            reverse_ranges.push(start..cursor);
        }
        pending.extend(
            symbolic
                .iter()
                .enumerate()
                .filter_map(|(index, present)| present.then_some(index)),
        );
        while let Some(child) = pending.pop() {
            for (_, parent) in &reverse[reverse_ranges[child].clone()] {
                if !symbolic[*parent] {
                    symbolic[*parent] = true;
                    pending.push(*parent);
                }
            }
        }
    }
    // Only symbolic branches are visited per machine. Shared closed graphs
    // cost one catalog traversal for the entire module; epochs deduplicate
    // the remaining selected branches without clearing a catalog per owner.
    let mut visited = vec![0; catalog.len()];
    let mut output = Vec::with_capacity(module.machines.len());
    for (position, machine) in module.machines.iter().enumerate() {
        let epoch = position + 1;
        let mut widths = Vec::new();
        roots.clear();
        for formal in machine
            .parameters
            .iter()
            .chain(&machine.erased_scalar_formals)
        {
            width_scalar(machine.id, formal.scalar_type, &mut widths)?;
        }
        roots.extend(machine.attachment);
        roots.extend(machine.nominal_cleanup_subject);
        roots.extend(
            machine
                .structural_parameters
                .iter()
                .map(|formal| formal.structural_type),
        );
        for place in &machine.structural_places {
            roots.extend(place.kind.produced_structural_type());
            match place.kind {
                StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                    roots.push(attachment)
                }
                StructuralPlaceKind::TrivialAffineLocal {
                    construction: Some(construction),
                    ..
                } => roots.push(construction.root_structural_type),
                _ => {}
            }
        }
        match &machine.result {
            TerminalMachineResult::Unit => {}
            TerminalMachineResult::Scalar(result) => {
                width_scalar(machine.id, result.scalar_type, &mut widths)?
            }
            TerminalMachineResult::Structural(result) => roots.push(result.structural_type),
        }
        for block in &machine.blocks {
            for value in block.parameters.iter().chain(&block.erased_scalar_formals) {
                width_scalar(machine.id, value.scalar_type, &mut widths)?;
            }
            roots.extend(
                block
                    .structural_parameters
                    .iter()
                    .map(|formal| formal.structural_type),
            );
            for operation in &block.operations {
                if let Some(value) = operation.result.as_scalar() {
                    width_scalar(machine.id, value.scalar_type, &mut widths)?;
                }
                if let Some(result) = operation.result.as_structural() {
                    roots.push(result.structural_type);
                }
            }
        }
        pending.clear();
        for root in &roots {
            if let Some(index) = width_type_index(*root, &catalog, &referents, machine.id)?
                && symbolic[index]
            {
                pending.push(index);
            }
        }
        while let Some(index) = pending.pop() {
            if visited[index] == epoch {
                continue;
            }
            visited[index] = epoch;
            for parameter in &seeds[seed_ranges[index].clone()] {
                width_parameter(machine.id, *parameter, &mut widths)?;
            }
            pending.extend(
                children[child_ranges[index].clone()]
                    .iter()
                    .copied()
                    .filter(|child| symbolic[*child]),
            );
        }
        if !widths.is_empty() {
            widths.sort_unstable();
            widths.dedup();
            let mut inputs = machine
                .parameters
                .iter()
                .map(|value| (value.id, value.scalar_type))
                .collect::<Vec<_>>();
            inputs.sort_unstable_by_key(|(value, _)| *value);
            let carrier = ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a carrier"),
            );
            for width in &widths {
                let Ok(position) = inputs.binary_search_by_key(width, |(value, _)| *value) else {
                    return Err((machine.id, "integer width is not a declared scalar input"));
                };
                if inputs[position].1 != carrier {
                    return Err((
                        machine.id,
                        "integer width input does not have the exact u64 carrier",
                    ));
                }
            }
        }
        output.push((machine.id, widths));
    }
    Ok(output)
}

fn width_type_index(
    identity: semantic_vocabulary::StructuralTypeId,
    catalog: &[&terminal_psi::StructuralTypeDeclaration],
    referents: &[semantic_vocabulary::StructuralTypeId],
    owner: semantic_vocabulary::MachineId,
) -> Result<Option<usize>, (semantic_vocabulary::MachineId, &'static str)> {
    match catalog.binary_search_by_key(&identity, |declaration| declaration.id) {
        Ok(index) => Ok(Some(index)),
        // An authenticated abstract referent grants no numeric carrier.
        Err(_) if referents.binary_search(&identity).is_ok() => Ok(None),
        Err(_) => Err((
            owner,
            "integer width inventory has a missing structural type",
        )),
    }
}

fn scalar_width(
    scalar: semantic_vocabulary::ScalarType,
) -> Option<semantic_vocabulary::WidthParameter> {
    let semantic_vocabulary::ScalarType::Integer(integer) = scalar else {
        return None;
    };
    let semantic_vocabulary::IntegerWidth::Parameter(parameter) = integer.width() else {
        return None;
    };
    Some(parameter)
}

fn width_parameter(
    owner: semantic_vocabulary::MachineId,
    parameter: semantic_vocabulary::WidthParameter,
    widths: &mut Vec<ValueId>,
) -> Result<(), (semantic_vocabulary::MachineId, &'static str)> {
    if parameter.owner != owner {
        return Err((owner, "integer width belongs to another machine"));
    }
    widths.push(parameter.value);
    Ok(())
}

fn width_scalar(
    owner: semantic_vocabulary::MachineId,
    scalar: semantic_vocabulary::ScalarType,
    widths: &mut Vec<ValueId>,
) -> Result<(), (semantic_vocabulary::MachineId, &'static str)> {
    if let Some(parameter) = scalar_width(scalar) {
        width_parameter(owner, parameter, widths)?;
    }
    Ok(())
}

fn width_type_parts(
    declaration: &terminal_psi::StructuralTypeDeclaration,
    children: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    widths: &mut Vec<semantic_vocabulary::WidthParameter>,
) {
    use terminal_psi::StructuralTypeShape;
    children.extend(declaration.quotient_carrier);
    if let Some(restriction) = &declaration.scalar_restriction {
        widths.extend(restriction.scalar_type().and_then(scalar_width));
        children.extend(restriction.as_structural());
    }
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => widths.extend(scalar_width(*scalar)),
        StructuralTypeShape::AtomicCell(cell) => widths.extend(scalar_width(cell.scalar_type())),
        StructuralTypeShape::Reference { referent, .. } => children.push(*referent),
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::ElementView { element, .. } => children.push(*element),
        StructuralTypeShape::Record { fields, .. } => width_fields(fields, children, widths),
        StructuralTypeShape::Sum { cases } => {
            for case in cases {
                width_fields(&case.fields, children, widths);
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            width_fields(fields, children, widths);
            for case in cases {
                width_fields(&case.fields, children, widths);
            }
        }
        StructuralTypeShape::Opaque | StructuralTypeShape::ByteSequence(_) => {}
    }
}

fn width_fields(
    fields: &[terminal_psi::StructuralFieldDeclaration],
    children: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    widths: &mut Vec<semantic_vocabulary::WidthParameter>,
) {
    for field in fields {
        widths.extend(field.field_type.scalar_type().and_then(scalar_width));
        children.extend(field.field_type.as_structural());
        children.extend(field.inline_carrier);
    }
}

#[cfg(test)]
mod width_inventory_tests {
    use super::*;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, PlaceId, ScalarType,
        StructuralPlaceKind, StructuralTypeId, WidthParameter,
    };
    use terminal_psi::{
        Block, BoundaryParameterKind, MachineContract, StructuralAccess, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
    };

    fn id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            quotient_carrier: None,
            id: id(raw),
            identity: terminal_psi::TerminalIdentityIndex::ABSENT,
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            shape,
            scalar_restriction: None,
            requires: Vec::new(),
        }
    }

    fn machine(raw: u64, root: Option<StructuralTypeId>) -> TerminalMachine {
        let block = BlockId::new(1).unwrap();
        let mut contract = MachineContract::empty(ContractId::new(raw).unwrap());
        contract.parameters.push(ValueDeclaration {
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        });
        contract.parameter_order.push(BoundaryParameterKind::Scalar);
        if let Some(root) = root {
            contract
                .parameter_order
                .push(BoundaryParameterKind::Structural);
            contract
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 1,
                    structural_type: root,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                });
        }
        TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: MachineId::new(raw).unwrap(),
            attachment: None,
            nominal_cleanup_subject: None,
            ranked_scc: None,
            structural_places: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block::plain(
                block,
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            )],
            contract,
        }
    }

    fn symbolic() -> ScalarType {
        ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Unsigned,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(1).unwrap(),
            },
        ))
    }

    #[test]
    fn an_unread_local_still_owes_its_width_formation() {
        let mut module = TerminalModule::declarations();
        module.structural_types.push(declaration(
            1_000_000,
            StructuralTypeShape::PrimitiveScalar(symbolic()),
        ));
        let mut machine = machine(1, None);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: id(1_000_000),
                construction: None,
            },
        });
        module.machines.push(machine);
        assert_eq!(
            integer_width_parameters(&module).unwrap(),
            vec![(MachineId::new(1).unwrap(), vec![ValueId::new(1).unwrap()])]
        );
    }

    #[test]
    fn shared_closed_graphs_remain_empty_and_dangling_edges_are_not_pruned() {
        let mut module = TerminalModule::declarations();
        module.structural_types.push(declaration(
            1,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        ));
        for index in 2..=256 {
            module.structural_types.push(declaration(
                index,
                StructuralTypeShape::Reference {
                    referent: id(index - 1),
                    access: StructuralAccess::SharedBorrow,
                },
            ));
        }
        module
            .machines
            .extend((1..=512).map(|index| machine(index, Some(id(256)))));
        let result = integer_width_parameters(&module).unwrap();
        assert_eq!(result.len(), 512);
        assert!(result.iter().all(|(_, widths)| widths.is_empty()));
        module.structural_types.push(declaration(
            1_000_000,
            StructuralTypeShape::Reference {
                referent: id(2_000_000),
                access: StructuralAccess::SharedBorrow,
            },
        ));
        assert!(integer_width_parameters(&module).is_err());
    }

    #[test]
    fn zero_array_headers_retain_the_width_owner_through_shared_types() {
        let mut module = TerminalModule::declarations();
        module.structural_types.push(declaration(
            1_000_000,
            StructuralTypeShape::PrimitiveScalar(symbolic()),
        ));
        module.structural_types.push(declaration(
            2_000_000,
            StructuralTypeShape::FixedArray {
                element: id(1_000_000),
                length: terminal_psi::ArrayExtent::Literal(0),
            },
        ));
        module.structural_types.push(declaration(
            3_000_000,
            StructuralTypeShape::Reference {
                referent: id(2_000_000),
                access: StructuralAccess::SharedBorrow,
            },
        ));
        module.machines.push(machine(1, Some(id(3_000_000))));
        assert_eq!(
            integer_width_parameters(&module).unwrap()[0].1,
            vec![ValueId::new(1).unwrap()]
        );
        module.machines.push(machine(2, Some(id(3_000_000))));
        assert!(integer_width_parameters(&module).is_err());
    }
}
