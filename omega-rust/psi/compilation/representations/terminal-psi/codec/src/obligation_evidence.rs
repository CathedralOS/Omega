/*
Evidence seals: the canonical bytes and fingerprint of one obligation's
accepted evidence, so a later stage can check that a fact it uses came with a
certificate the proof kernel accepted (`wiki/drafts/designs/stage_trust.md`,
rule 2).

A seal holds the obligation's identity, its goal, the hypotheses passed to
the kernel and the certificate, and a SHA-256 fingerprint over their canonical
encoding. Anyone holding the seal can recompute the fingerprint; a seal whose
parts were changed after sealing no longer matches it.

Seals travel in memory beside the Terminal module. They are not part of the
module's bytes: Terminal Psi's layout (`wiki/spec/terminal-psi/encoding.md`)
has no place for them yet. A seal is therefore lost on any byte round trip and
does not survive a stored artifact.
*/

#[cfg(test)]
use semantic_vocabulary::OperationId;
use semantic_vocabulary::{
    MachineId, ObligationId, PlaceId, Proposition, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    CertificateEnvelope, CorrelatedAffineBranchWitness, ExecutionSite, PrimitiveJudgment,
    ProofNode, ProofRule,
};

use crate::CodecError;
use crate::semantic_module::{Writer, encode_proposition, encode_scalar_term};
use crate::term_table::{TableChildren, TableTerm, encode_term_table};
use crate::term_wire::integer_math_term_wire::encode_integer_math_term;

const DOMAIN: &[u8] = b"psi-obligation-evidence-fingerprint\0";

/// One accepted obligation's evidence and its fingerprint.
///
/// In memory only: it is not encoded in the Terminal module, so it is lost on
/// a byte round trip and does not survive a stored artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationEvidenceSeal {
    pub machine: MachineId,
    pub obligation: ObligationId,
    pub goal: Proposition,
    /// The kernel's assumptions, in the order the certificate cites them.
    pub assumptions: Vec<Proposition>,
    /// The kernel's semantic axioms, in the order the certificate cites them.
    pub axioms: Vec<Proposition>,
    pub certificate: CertificateEnvelope,
    pub fingerprint: [u8; 32],
}

impl ObligationEvidenceSeal {
    /// Seals accepted evidence, computing its fingerprint.
    pub fn seal(
        machine: MachineId,
        obligation: ObligationId,
        goal: Proposition,
        assumptions: Vec<Proposition>,
        axioms: Vec<Proposition>,
        certificate: CertificateEnvelope,
    ) -> Result<Self, CodecError> {
        let mut seal = Self {
            machine,
            obligation,
            goal,
            assumptions,
            axioms,
            certificate,
            fingerprint: [0; 32],
        };
        seal.fingerprint = seal.recompute()?;
        Ok(seal)
    }

    /// The fingerprint of the seal's parts as they are now.
    pub fn recompute(&self) -> Result<[u8; 32], CodecError> {
        let mut writer = Writer::default();
        writer.id(self.machine);
        writer.id(self.obligation);
        encode_proposition(&mut writer, &self.goal)?;
        for list in [&self.assumptions, &self.axioms] {
            writer.len("evidence hypotheses", list.len())?;
            for proposition in list {
                encode_proposition(&mut writer, proposition)?;
            }
        }
        writer.id(self.certificate.identity);
        writer.u16(self.certificate.proof_system_marker.get());
        encode_proof_node(&mut writer, &self.certificate.proof)?;
        let bytes = writer.finish();
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(DOMAIN);
        let length = u64::try_from(bytes.len()).map_err(|_| CodecError::NonCanonicalEncoding)?;
        digest.raw(&length.to_le_bytes());
        digest.raw(&bytes);
        Ok(digest.finish())
    }

    /// Whether the stored fingerprint is the fingerprint of the stored parts.
    pub fn intact(&self) -> Result<bool, CodecError> {
        Ok(self.recompute()? == self.fingerprint)
    }
}

/// The seals produced beside one in-memory module, bound to that module's
/// identity. A consumer attaches them only to the module they name, so seals
/// from an earlier build never reach a different module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSeals {
    pub module: crate::TerminalPsiIdentity,
    pub seals: Vec<ObligationEvidenceSeal>,
}

impl EvidenceSeals {
    /// Binds evidence to the exact already closed module without encoding or
    /// closing it again. The owner retains the bytes behind this identity.
    pub fn for_closed_module(
        module: &crate::ClosedTerminalModule,
        seals: Vec<ObligationEvidenceSeal>,
    ) -> Self {
        Self {
            module: module.identity(),
            seals,
        }
    }

    pub fn for_module(
        module: &terminal_psi::TerminalModule,
        seals: Vec<ObligationEvidenceSeal>,
    ) -> Result<Self, CodecError> {
        Ok(Self {
            module: crate::terminal_psi_identity(module)?,
            seals,
        })
    }
}

/// Why in-memory evidence cannot be attached to the requested module or scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceSealValidationError {
    Codec(CodecError),
    ModuleIdentityMismatch,
    UnknownSelectedMachine(MachineId),
    DuplicateSeal {
        machine: MachineId,
        obligation: ObligationId,
    },
    /// No unique execution site owns this seal's obligation.
    UnownedSeal {
        machine: MachineId,
        obligation: ObligationId,
    },
    SealMismatch {
        machine: MachineId,
        site: ExecutionSite,
        obligation: ObligationId,
    },
}

impl std::fmt::Display for EvidenceSealValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "cannot attach obligation evidence: {self:?}")
    }
}

impl std::error::Error for EvidenceSealValidationError {}

impl From<CodecError> for EvidenceSealValidationError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl EvidenceSeals {
    /// Validates the complete bundle before selecting one native machine scope.
    /// Dormant evidence must be valid too: filtering cannot hide a damaged,
    /// duplicated or foreign seal. Projection preserves the original module
    /// identity and all retained seal coordinates and fingerprints.
    pub fn project_for_machines(
        &self,
        module: &terminal_psi::TerminalModule,
        selected: &[MachineId],
    ) -> Result<Self, EvidenceSealValidationError> {
        if self.module != crate::terminal_psi_identity(module)? {
            return Err(EvidenceSealValidationError::ModuleIdentityMismatch);
        }
        EvidenceSealValidator::new(module, &self.seals).validate_all()?;
        let mut available = module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>();
        available.sort_unstable();
        for &machine in selected {
            if available.binary_search(&machine).is_err() {
                return Err(EvidenceSealValidationError::UnknownSelectedMachine(machine));
            }
        }
        let mut selected = selected.to_vec();
        selected.sort_unstable();
        selected.dedup();
        Ok(Self {
            module: self.module,
            seals: self
                .seals
                .iter()
                .filter(|seal| selected.binary_search(&seal.machine).is_ok())
                .cloned()
                .collect(),
        })
    }
}

struct ExecutionEvidenceGoal {
    site: ExecutionSite,
    obligation: ObligationId,
    proposition: Option<Proposition>,
}

fn unique_by_id<T, Id: Ord>(rows: &[T], id: Id, key: impl Fn(&T) -> Id) -> Option<&T> {
    let position = rows.partition_point(|row| key(row) < id);
    let row = rows.get(position)?;
    (key(row) == id && rows.get(position + 1).is_none_or(|next| key(next) != id)).then_some(row)
}

fn machine_place_types(
    machine: &terminal_psi::TerminalMachine,
) -> Vec<(PlaceId, Option<StructuralTypeId>)> {
    let mut parameters = machine
        .structural_parameters
        .iter()
        .map(|parameter| {
            (
                parameter.place,
                StructuralPlaceKind::Parameter {
                    position: parameter.position,
                },
                Some(parameter.structural_type),
            )
        })
        .collect::<Vec<_>>();
    for block in &machine.blocks {
        parameters.extend(block.structural_parameters.iter().enumerate().map(
            |(position, parameter)| {
                (
                    parameter.place,
                    StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position: parameter.position,
                    },
                    (usize::try_from(parameter.position).ok() == Some(position))
                        .then_some(parameter.structural_type),
                )
            },
        ));
    }
    parameters.sort_unstable_by_key(|(place, _, _)| *place);
    let mut places = machine
        .structural_places
        .iter()
        .map(|place| {
            let declaration = match place.kind {
                StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::BlockParameter { .. } => {
                    unique_by_id(&parameters, place.id, |(id, _, _)| *id)
                        .filter(|(_, kind, _)| *kind == place.kind)
                        .and_then(|(_, _, ty)| *ty)
                }
                StructuralPlaceKind::Result => machine
                    .result
                    .structural()
                    .filter(|result| result.place == place.id)
                    .map(|result| result.structural_type),
                _ => place.kind.produced_structural_type(),
            };
            (place.id, declaration)
        })
        .collect::<Vec<_>>();
    places.sort_unstable_by_key(|(place, _)| *place);
    places
}

struct MachineEvidenceGoals<'module> {
    machine: &'module terminal_psi::TerminalMachine,
    sites: Vec<ExecutionSite>,
    goals: Vec<ExecutionEvidenceGoal>,
}

impl<'module> MachineEvidenceGoals<'module> {
    fn new(
        machine: &'module terminal_psi::TerminalMachine,
        structural_types: &'module [terminal_psi::StructuralTypeDeclaration],
        indexed_types: &mut Option<Vec<&'module terminal_psi::StructuralTypeDeclaration>>,
    ) -> Self {
        let declarations = machine
            .parameters
            .iter()
            .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .map(|declaration| (declaration.id, declaration.scalar_type))
            .collect();
        let types = terminal_semantics::SortedValueTypes::new(declarations);
        let mut sites = Vec::new();
        let mut goals = Vec::new();
        let mut places = None;
        for operation in machine.operations() {
            let site = ExecutionSite::Operation(operation.id);
            sites.push(site);
            if let Some(types) = &types
                && let Ok(Some((obligation, proposition))) =
                    terminal_semantics::operation_safety_goal(operation, types)
            {
                goals.push(ExecutionEvidenceGoal {
                    site,
                    obligation,
                    proposition: Some(proposition),
                });
            }
        }
        sites.extend(
            machine
                .blocks
                .iter()
                .map(|block| ExecutionSite::Block(block.id)),
        );
        // These reads execute at their owner's exact point. In particular a
        // discriminator is read once before selecting any outgoing case edge.
        let projections = machine
            .operations()
            .flat_map(|operation| {
                operation
                    .kind
                    .structural_projections()
                    .into_iter()
                    .map(move |projection| (ExecutionSite::Operation(operation.id), projection))
            })
            .chain(machine.blocks.iter().flat_map(|block| {
                block
                    .terminator
                    .storage_uses()
                    .into_iter()
                    .map(move |projection| (ExecutionSite::Block(block.id), projection))
            }));
        for (site, projection) in projections {
            let mut indices = projection
                .path
                .iter()
                .filter_map(terminal_psi::StructuralPathSegment::runtime_index)
                .peekable();
            if indices.peek().is_none() {
                continue;
            }
            let first = goals.len();
            let places = places.get_or_insert_with(|| machine_place_types(machine));
            let root =
                unique_by_id(places, projection.root, |(place, _)| *place).and_then(|(_, ty)| *ty);
            let valid = root.zip(types.as_ref()).is_some_and(|(root, types)| {
                let indexed_types = indexed_types.get_or_insert_with(|| {
                    let mut rows = structural_types.iter().collect::<Vec<_>>();
                    rows.sort_unstable_by_key(|row| row.id);
                    rows
                });
                terminal_semantics::visit_runtime_index_goals(
                    structural_types.iter(),
                    |id| unique_by_id(indexed_types, id, |row| row.id).copied(),
                    root,
                    projection.path,
                    types,
                    |obligation, proposition| {
                        goals.push(ExecutionEvidenceGoal {
                            site,
                            obligation,
                            proposition: Some(proposition),
                        })
                    },
                )
                .is_ok()
            });
            if !valid {
                // A malformed path cannot corroborate a seal or disappear
                // as a competing owner of the same obligation.
                goals.truncate(first);
                goals.extend(indices.map(|(_, obligation)| ExecutionEvidenceGoal {
                    site,
                    obligation,
                    proposition: None,
                }));
            }
        }
        sites.sort_unstable();
        goals.sort_unstable_by_key(|row| row.obligation);
        Self {
            machine,
            sites,
            goals,
        }
    }

    fn names_one_site(&self, site: ExecutionSite) -> bool {
        unique_by_id(&self.sites, site, |site| *site).is_some()
    }

    fn obligation(&self, obligation: ObligationId) -> Option<&ExecutionEvidenceGoal> {
        unique_by_id(&self.goals, obligation, |row| row.obligation)
    }
}

/// Borrows seals and their original semantic module. Goal reconstruction is
/// shared by whole-bundle projection and receiving operation-fact attachment.
/// This checks correspondence and fingerprints, never kernel derivations.
///
/// Construction indexes the operation goals once per machine with evidence. A
/// receiver may validate only its consumed seals and retain its own unmatched
/// seal refusal; projection must call `validate_all` before dropping any row.
pub struct EvidenceSealValidator<'module, 'seals> {
    seals: &'seals [ObligationEvidenceSeal],
    order: Vec<usize>,
    machines: Vec<MachineEvidenceGoals<'module>>,
}

impl<'module, 'seals> EvidenceSealValidator<'module, 'seals> {
    pub fn new(
        module: &'module terminal_psi::TerminalModule,
        seals: &'seals [ObligationEvidenceSeal],
    ) -> Self {
        let mut order = (0..seals.len()).collect::<Vec<_>>();
        order.sort_unstable_by_key(|&position| {
            let seal = &seals[position];
            (seal.machine, seal.obligation, position)
        });
        let mut selected = order
            .iter()
            .map(|&position| seals[position].machine)
            .collect::<Vec<_>>();
        selected.dedup();
        let mut indexed_types = None;
        let mut machines = module
            .machines
            .iter()
            .filter(|machine| selected.binary_search(&machine.id).is_ok())
            .map(|machine| {
                MachineEvidenceGoals::new(machine, &module.structural_types, &mut indexed_types)
            })
            .collect::<Vec<_>>();
        machines.sort_unstable_by_key(|row| row.machine.id);
        Self {
            seals,
            order,
            machines,
        }
    }

    fn machine(&self, machine: MachineId) -> Option<&MachineEvidenceGoals<'module>> {
        let position = self
            .machines
            .partition_point(|row| row.machine.id < machine);
        let found = self.machines.get(position)?;
        (found.machine.id == machine
            && self
                .machines
                .get(position + 1)
                .is_none_or(|next| next.machine.id != machine))
        .then_some(found)
    }

    pub fn validate_all(&self) -> Result<(), EvidenceSealValidationError> {
        for pair in self.order.windows(2) {
            let first = &self.seals[pair[0]];
            let second = &self.seals[pair[1]];
            if (first.machine, first.obligation) == (second.machine, second.obligation) {
                return Err(EvidenceSealValidationError::DuplicateSeal {
                    machine: first.machine,
                    obligation: first.obligation,
                });
            }
        }
        for (position, seal) in self.seals.iter().enumerate() {
            let site = self
                .machine(seal.machine)
                .and_then(|machine| machine.obligation(seal.obligation))
                .map(|goal| goal.site)
                .ok_or(EvidenceSealValidationError::UnownedSeal {
                    machine: seal.machine,
                    obligation: seal.obligation,
                })?;
            self.validate_at(position, site)?;
        }
        Ok(())
    }

    /// Returns the original seal index and fingerprint, or absence when this
    /// operation has no seal. The caller still accounts for every consumed
    /// seal, so a duplicate or otherwise unconsumed row cannot disappear.
    pub fn fingerprint(
        &self,
        machine: MachineId,
        site: impl Into<ExecutionSite>,
        obligation: ObligationId,
    ) -> Result<Option<(usize, [u8; 32])>, EvidenceSealValidationError> {
        let site = site.into();
        let coordinate = (machine, obligation);
        let ordered = self.order.partition_point(|&position| {
            let seal = &self.seals[position];
            (seal.machine, seal.obligation) < coordinate
        });
        let Some(&position) = self.order.get(ordered) else {
            return Ok(None);
        };
        let seal = &self.seals[position];
        if (seal.machine, seal.obligation) != coordinate {
            return Ok(None);
        }
        self.validate_at(position, site)
            .map(|fingerprint| Some((position, fingerprint)))
    }

    fn validate_at(
        &self,
        position: usize,
        site: ExecutionSite,
    ) -> Result<[u8; 32], EvidenceSealValidationError> {
        let seal = &self.seals[position];
        let mismatch = || EvidenceSealValidationError::SealMismatch {
            machine: seal.machine,
            site,
            obligation: seal.obligation,
        };
        let machine = self.machine(seal.machine).ok_or_else(mismatch)?;
        let goal = machine.obligation(seal.obligation).ok_or_else(mismatch)?;
        if goal.site != site
            || !machine.names_one_site(site)
            || goal.proposition.as_ref() != Some(&seal.goal)
            || !seal.intact()?
        {
            return Err(mismatch());
        }
        Ok(seal.fingerprint)
    }
}

/// The canonical bytes of one certificate.
pub fn canonical_certificate_bytes(proof: &ProofNode) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_proof_node(&mut writer, proof)?;
    Ok(writer.finish())
}

fn index(writer: &mut Writer, value: usize) -> Result<(), CodecError> {
    writer.u64(u64::try_from(value).map_err(|_| CodecError::NonCanonicalEncoding)?);
    Ok(())
}

fn optional_index(writer: &mut Writer, value: Option<usize>) -> Result<(), CodecError> {
    match value {
        None => writer.u8(0),
        Some(value) => {
            writer.u8(1);
            index(writer, value)?;
        }
    }
    Ok(())
}

fn indices(writer: &mut Writer, values: &[usize]) -> Result<(), CodecError> {
    writer.len("certificate indices", values.len())?;
    for value in values {
        index(writer, *value)?;
    }
    Ok(())
}

fn branch(writer: &mut Writer, branch: &CorrelatedAffineBranchWitness) -> Result<(), CodecError> {
    encode_scalar_term(writer, &branch.root)?;
    encode_scalar_term(writer, &branch.target)?;
    writer.len("correlated branch steps", branch.steps.len())?;
    for step in &branch.steps {
        index(writer, step.definition_axiom)?;
        optional_index(writer, step.literal_axiom)?;
    }
    Ok(())
}

/// A certificate is one postorder table (`crate::term_table`): each row is the
/// rule tag, the rule's non-premise fields in declaration order and the
/// conclusion, then one handle per premise. The match is exhaustive: a new
/// rule changes this encoding deliberately.
fn encode_proof_node(writer: &mut Writer, node: &ProofNode) -> Result<(), CodecError> {
    encode_term_table(writer, node)
}

impl TableTerm for ProofNode {
    const LABEL: &'static str = "certificate node";

    fn children<'term>(&'term self, premises: &mut Vec<&'term Self>) {
        match &self.rule {
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                premises.push(premise);
                premises.extend(equalities.iter());
            }
            ProofRule::PredicateDenotation { premise } => {
                premises.push(premise);
            }
            ProofRule::IeeeFloatOrderChain(children)
            | ProofRule::ConjunctionIntroduction(children) => {
                premises.extend(children.iter());
            }
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                premises.push(conjunction);
            }
            ProofRule::DisjunctionIntroduction { disjunct, .. } => {
                premises.push(disjunct);
            }
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => {
                premises.push(disjunction);
                premises.extend(branches.iter());
            }
            ProofRule::ImplicationIntroduction { body } => {
                premises.push(body);
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => {
                premises.push(implication);
                premises.push(premise);
            }
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                premises.push(left_equals_middle);
                premises.push(middle_equals_right);
            }
            ProofRule::EqualitySymmetry { equality } => {
                premises.push(equality);
            }
            ProofRule::IntegerOrderWeakening { relation } => {
                premises.push(relation);
            }
            ProofRule::IntegerOrderDiscreteness { relation } => {
                premises.push(relation);
            }
            ProofRule::IntegerPowerOfTwoOrder { order } => {
                premises.push(order);
            }
            ProofRule::IntegerMathOrderTranslation { relation, .. } => {
                premises.push(relation);
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                premises.push(forward);
                premises.push(backward);
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => {
                premises.push(difference);
                premises.push(positive);
            }
            ProofRule::IntegerAddOrder { sum, positive } => {
                premises.push(sum);
                premises.push(positive);
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                premises.push(smaller);
                premises.push(larger);
                premises.push(order);
            }
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => {
                premises.push(left_less_or_equal_middle);
                premises.push(middle_less_or_equal_right);
            }
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => {
                premises.push(left_to_middle);
                premises.push(middle_to_right);
            }
            ProofRule::IntegerOrderSubstitution {
                relation, equality, ..
            } => {
                premises.push(relation);
                premises.push(equality);
            }
            ProofRule::IntegerAffineBound { root_bound, .. } => {
                premises.push(root_bound);
            }
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                ..
            } => {
                premises.push(left_bound);
                premises.push(right_bound);
            }
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                ..
            } => {
                premises.push(left_bound);
                premises.push(right_bound);
            }
            ProofRule::IntegerCastBound { root_bound, .. } => {
                premises.push(root_bound);
            }
            _ => {}
        }
    }

    fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
        write_proof_row(writer, self)
    }

    fn read_row(
        _reader: &mut crate::semantic_module::Reader<'_>,
        _children: &mut TableChildren<'_, Self>,
    ) -> Result<Self, CodecError> {
        Err(CodecError::NonCanonicalEncoding)
    }
}

fn write_proof_row(writer: &mut Writer, node: &ProofNode) -> Result<(), CodecError> {
    match &node.rule {
        ProofRule::ValueEqualityTransport { equalities, .. } => {
            writer.u8(0);
            writer.len("certificate children", equalities.len())?;
        }
        ProofRule::PredicateDenotation { .. } => {
            writer.u8(1);
        }
        ProofRule::Primitive(judgment) => {
            writer.u8(2);
            writer.u8(match judgment {
                PrimitiveJudgment::Truth => 0,
                PrimitiveJudgment::ReflexiveEquality => 1,
                PrimitiveJudgment::ClosedIntegerRelation => 2,
                PrimitiveJudgment::IntegerCarrierBound => 3,
                PrimitiveJudgment::ClosedIeeeFloatRelation => 4,
            });
        }
        ProofRule::SemanticAxiom { index: cited } => {
            writer.u8(3);
            index(writer, *cited)?;
        }
        ProofRule::Assumption { index: cited } => {
            writer.u8(4);
            index(writer, *cited)?;
        }
        ProofRule::ConjunctionIntroduction(children) => {
            writer.u8(5);
            writer.len("certificate children", children.len())?;
        }
        ProofRule::ConjunctionElimination { conjunct, .. } => {
            writer.u8(6);
            index(writer, *conjunct)?;
        }
        ProofRule::DisjunctionIntroduction {
            index: position, ..
        } => {
            writer.u8(7);
            index(writer, *position)?;
        }
        ProofRule::DisjunctionElimination { branches, .. } => {
            writer.u8(8);
            writer.len("certificate children", branches.len())?;
        }
        ProofRule::ImplicationIntroduction { .. } => {
            writer.u8(9);
        }
        ProofRule::ImplicationElimination { .. } => {
            writer.u8(10);
        }
        ProofRule::EqualityTransitivity { .. } => {
            writer.u8(11);
        }
        ProofRule::EqualitySymmetry { .. } => {
            writer.u8(12);
        }
        ProofRule::IntegerOrderWeakening { .. } => {
            writer.u8(13);
        }
        ProofRule::IntegerOrderDiscreteness { .. } => {
            writer.u8(14);
        }
        ProofRule::IntegerMathOrderTranslation { addend, .. } => {
            writer.u8(15);
            encode_integer_math_term(writer, addend)?;
        }
        ProofRule::IntegerOrderAntisymmetry { .. } => {
            writer.u8(16);
        }
        ProofRule::IntegerSubtractOrder { .. } => {
            writer.u8(17);
        }
        ProofRule::IntegerAddOrder { .. } => {
            writer.u8(18);
        }
        ProofRule::IntegerSubtractAntitone { .. } => {
            writer.u8(19);
        }
        ProofRule::IntegerLessOrEqualTransitivity { .. } => {
            writer.u8(20);
        }
        ProofRule::IntegerStrictOrderTransitivity { .. } => {
            writer.u8(21);
        }
        ProofRule::IntegerOrderSubstitution { endpoint, .. } => {
            writer.u8(22);
            index(writer, *endpoint)?;
        }
        ProofRule::IntegerAffineBound { witness, .. } => {
            writer.u8(23);
            encode_scalar_term(writer, &witness.root)?;
            encode_scalar_term(writer, &witness.target)?;
            indices(writer, &witness.definition_axioms)?;
            writer.len("affine literal axioms", witness.literal_axioms.len())?;
            for literal in &witness.literal_axioms {
                optional_index(writer, *literal)?;
            }
        }
        ProofRule::IntegerExactAddDefinitionBound {
            definition_axiom, ..
        } => {
            writer.u8(24);
            index(writer, *definition_axiom)?;
        }
        ProofRule::IntegerExactSubtractDefinitionBound {
            definition_axiom, ..
        } => {
            writer.u8(25);
            index(writer, *definition_axiom)?;
        }
        ProofRule::IntegerCastBound { witness, .. } => {
            writer.u8(26);
            encode_scalar_term(writer, &witness.root)?;
            encode_scalar_term(writer, &witness.target)?;
            indices(writer, &witness.definition_axioms)?;
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            writer.u8(27);
            branch(writer, &witness.dividend)?;
            branch(writer, &witness.divisor)?;
            index(writer, witness.definition_axiom_count)?;
            index(writer, witness.lower_bound_axiom)?;
            index(writer, witness.upper_bound_axiom)?;
            encode_proposition(writer, &witness.conclusion)?;
        }
        ProofRule::IntegerPowerOfTwoOrder { .. } => {
            writer.u8(28);
        }
        ProofRule::IeeeFloatOrderChain(premises) => {
            writer.u8(29);
            writer.len("certificate children", premises.len())?;
        }
    }
    encode_proposition(writer, &node.conclusion)
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::EvidenceIdentity;
    use terminal_psi::ProofSystemMarker;

    use super::*;

    fn truth() -> ProofNode {
        ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }
    }

    fn seal(proof: ProofNode) -> ObligationEvidenceSeal {
        ObligationEvidenceSeal::seal(
            MachineId::new(1).unwrap(),
            ObligationId::new(2).unwrap(),
            Proposition::Truth,
            Vec::new(),
            Vec::new(),
            CertificateEnvelope {
                identity: EvidenceIdentity::new(2).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            },
        )
        .unwrap()
    }

    #[test]
    fn a_changed_part_no_longer_matches_its_fingerprint() {
        let sealed = seal(truth());
        assert!(sealed.intact().unwrap());
        assert_ne!(sealed.fingerprint, [0; 32]);
        let mut goal = sealed.clone();
        goal.goal = Proposition::Falsehood;
        assert!(!goal.intact().unwrap());
        let mut hypotheses = sealed.clone();
        hypotheses.assumptions.push(Proposition::Truth);
        assert!(!hypotheses.intact().unwrap());
        let mut obligation = sealed.clone();
        obligation.obligation = ObligationId::new(3).unwrap();
        assert!(!obligation.intact().unwrap());
        let mut certificate = sealed;
        certificate.certificate.proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::ConjunctionIntroduction(vec![truth(), truth()]),
        };
        assert!(!certificate.intact().unwrap());
    }

    #[test]
    fn equal_certificates_encode_equally_and_distinct_rules_do_not() {
        let assumption = |index| ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Assumption { index },
        };
        let axiom = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::SemanticAxiom { index: 0 },
        };
        assert_eq!(
            canonical_certificate_bytes(&assumption(0)).unwrap(),
            canonical_certificate_bytes(&assumption(0)).unwrap()
        );
        assert_ne!(
            canonical_certificate_bytes(&assumption(0)).unwrap(),
            canonical_certificate_bytes(&assumption(1)).unwrap()
        );
        assert_ne!(
            canonical_certificate_bytes(&assumption(0)).unwrap(),
            canonical_certificate_bytes(&axiom).unwrap()
        );
    }

    #[test]
    fn ieee_chain_fingerprint_covers_each_premise_and_the_closed_judgment() {
        let leaf = |index| ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Assumption { index },
        };
        let proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::IeeeFloatOrderChain(vec![leaf(0), leaf(1)]),
        };
        let original = canonical_certificate_bytes(&proof).unwrap();
        for position in 0..2 {
            let mut changed = proof.clone();
            let ProofRule::IeeeFloatOrderChain(children) = &mut changed.rule else {
                unreachable!()
            };
            children[position].rule = ProofRule::Assumption { index: 7 };
            assert_ne!(canonical_certificate_bytes(&changed).unwrap(), original);
        }
        let mut reversed = proof.clone();
        let ProofRule::IeeeFloatOrderChain(children) = &mut reversed.rule else {
            unreachable!()
        };
        children.reverse();
        assert_ne!(canonical_certificate_bytes(&reversed).unwrap(), original);
        let mut closed = leaf(0);
        closed.rule = ProofRule::Primitive(PrimitiveJudgment::ClosedIeeeFloatRelation);
        let encoded = canonical_certificate_bytes(&closed).unwrap();
        closed.rule = ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation);
        assert_ne!(canonical_certificate_bytes(&closed).unwrap(), encoded);
    }

    #[test]
    fn a_deep_certificate_encodes_without_host_stack_recursion() {
        std::thread::spawn(|| {
            let mut proof = truth();
            for _ in 0..4096 {
                proof = ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::EqualitySymmetry {
                        equality: Box::new(proof),
                    },
                };
            }
            let bytes = canonical_certificate_bytes(&proof).unwrap();
            assert_eq!(&bytes[..4], &4097_u32.to_le_bytes());
            assert_eq!(canonical_certificate_bytes(&proof).unwrap(), bytes);
        })
        .join()
        .expect("certificate encoding uses an explicit stack");
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use semantic_vocabulary::{
        ContractId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId,
    };
    use terminal_psi::{
        Operation, OperationKind, OperationResult, TerminalModule, ValueDeclaration,
    };

    fn machine_id(value: u64) -> MachineId {
        MachineId::new(value).unwrap()
    }

    fn module_and_seals() -> (TerminalModule, EvidenceSeals) {
        let mut module = TerminalModule::for_entry(machine_id(1));
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
        let mut seals = Vec::new();
        for number in 1..=2 {
            let machine = machine_id(number);
            let mut body = crate::semantic_module::entry_tests::unit_machine(machine);
            body.declaration = None;
            body.contract.id = ContractId::new(number).unwrap();
            let value = |offset| ValueDeclaration {
                id: ValueId::new(number * 10 + offset).unwrap(),
                scalar_type,
                qualifications: Default::default(),
            };
            let operation = |offset| OperationId::new(number * 10 + offset).unwrap();
            body.blocks[0].operations = vec![
                Operation::unsuspended(
                    operation(1),
                    OperationResult::Scalar(value(1)),
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(1),
                    },
                ),
                Operation::unsuspended(
                    operation(2),
                    OperationResult::Scalar(value(2)),
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(2),
                    },
                ),
                Operation::unsuspended(
                    operation(3),
                    OperationResult::Scalar(value(3)),
                    OperationKind::ExactIntegerAdd {
                        left: value(1).id,
                        right: value(2).id,
                        obligation: ObligationId::new(number).unwrap(),
                    },
                ),
            ];
            let goals = MachineEvidenceGoals::new(&body, &[], &mut None);
            let obligation = ObligationId::new(number).unwrap();
            let goal = goals
                .obligation(obligation)
                .unwrap()
                .proposition
                .clone()
                .unwrap();
            // Correspondence tests borrow the exact goal as an assumption.
            // No proof search or kernel replay belongs to this API.
            seals.push(
                ObligationEvidenceSeal::seal(
                    machine,
                    obligation,
                    goal.clone(),
                    vec![goal.clone()],
                    Vec::new(),
                    CertificateEnvelope {
                        identity: EvidenceIdentity::new(number).unwrap(),
                        proof_system_marker: terminal_psi::ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: goal,
                            rule: ProofRule::Assumption { index: 0 },
                        },
                    },
                )
                .unwrap(),
            );
            module.machines.push(body);
        }
        let evidence = EvidenceSeals::for_module(&module, seals).unwrap();
        (module, evidence)
    }

    #[test]
    fn closed_module_evidence_preserves_the_original_identity_and_seals() {
        let (module, original) = module_and_seals();
        let closed = crate::ClosedTerminalModule::new(module).unwrap();
        let evidence = EvidenceSeals::for_closed_module(&closed, original.seals.clone());
        assert_eq!(evidence.module, closed.identity());
        assert_eq!(evidence, original);
    }

    #[test]
    fn byte_bounds_seals_belong_to_the_exact_operation_endpoints() {
        use semantic_vocabulary::{PlaceId, StructuralTypeId};
        use terminal_psi::{
            StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
        };
        let mut module = TerminalModule::for_entry(machine_id(1));
        let mut body = crate::semantic_module::entry_tests::unit_machine(machine_id(1));
        let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let value = |raw| ValueId::new(raw).unwrap();
        let operation = |raw| OperationId::new(raw).unwrap();
        body.parameters = (1..=3)
            .map(|raw| ValueDeclaration {
                id: value(raw),
                scalar_type: count,
                qualifications: Default::default(),
            })
            .collect();
        body.blocks[0].operations = vec![
            Operation::unsuspended(
                operation(1),
                OperationResult::Scalar(ValueDeclaration {
                    id: value(4),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                    qualifications: Default::default(),
                }),
                OperationKind::ByteSequenceRead {
                    source: PlaceId::new(1).unwrap(),
                    index: value(1),
                    length: value(3),
                    obligation: ObligationId::new(1).unwrap(),
                },
            ),
            Operation::unsuspended(
                operation(2),
                OperationResult::Structural(StructuralOperationResult {
                    place: PlaceId::new(2).unwrap(),
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    additional_input_qualifications: vec![],
                    qualification_establishments: vec![],
                    claims: vec![],
                }),
                OperationKind::ByteSequenceSubslice {
                    source: StructuralArgument {
                        place: PlaceId::new(1).unwrap(),
                        path: vec![],
                        access: StructuralAccess::SharedBorrow,
                    },
                    start: value(1),
                    end: value(2),
                    length: value(3),
                    obligation: ObligationId::new(2).unwrap(),
                },
            ),
        ];
        // This API checks goal correspondence. Descriptor formation and
        // backing custody are the independent checker's separate obligations.
        let goals = MachineEvidenceGoals::new(&body, &[], &mut None);
        let seals = (1..=2)
            .map(|raw| {
                let obligation = ObligationId::new(raw).unwrap();
                let goal = goals
                    .obligation(obligation)
                    .unwrap()
                    .proposition
                    .clone()
                    .unwrap();
                ObligationEvidenceSeal::seal(
                    machine_id(1),
                    obligation,
                    goal.clone(),
                    vec![goal.clone()],
                    vec![],
                    CertificateEnvelope {
                        identity: EvidenceIdentity::new(raw).unwrap(),
                        proof_system_marker: terminal_psi::ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: goal,
                            rule: ProofRule::Assumption { index: 0 },
                        },
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        module.machines.push(body);
        assert!(
            EvidenceSealValidator::new(&module, &seals)
                .validate_all()
                .is_ok()
        );
        let OperationKind::ByteSequenceSubslice { end, .. } =
            &mut module.machines[0].blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        *end = value(3);
        assert!(matches!(
            EvidenceSealValidator::new(&module, &seals).validate_all(),
            Err(EvidenceSealValidationError::SealMismatch { .. })
        ));
    }

    #[test]
    fn operations_and_terminators_own_each_nested_array_bound() {
        use semantic_vocabulary::{PlaceId, ScalarTerm, StructuralPlaceKind, StructuralTypeId};
        use terminal_psi::{
            ArrayExtent, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
            StructuralTypeShape, TerminalIdentityIndex,
        };

        let machine = machine_id(1);
        let mut module = TerminalModule::for_entry(machine);
        let mut body = crate::semantic_module::entry_tests::unit_machine(machine);
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let ty = |raw| StructuralTypeId::new(raw).unwrap();
        let value = |raw| ValueId::new(raw).unwrap();
        let obligation = |raw| ObligationId::new(raw).unwrap();
        let root = PlaceId::new(1).unwrap();
        let operation = OperationId::new(1).unwrap();
        let scalar = |id, integer| ValueDeclaration {
            id: value(id),
            scalar_type: ScalarType::Integer(integer),
            qualifications: Default::default(),
        };
        body.parameters = vec![scalar(1, unsigned), scalar(2, signed)];
        module.structural_types = [
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(unsigned)),
            StructuralTypeShape::FixedArray {
                element: ty(1),
                length: ArrayExtent::Literal(5),
            },
            StructuralTypeShape::FixedArray {
                element: ty(2),
                length: ArrayExtent::Literal(2),
            },
        ]
        .into_iter()
        .enumerate()
        .map(|(position, shape)| StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: ty(position as u64 + 1),
            identity: TerminalIdentityIndex::ABSENT,
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            requires: vec![],
            shape,
        })
        .collect();
        body.structural_places.push(StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::BlockParameter {
                block: body.blocks[0].id,
                position: 0,
            },
        });
        body.blocks[0]
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: root,
                position: 0,
                structural_type: ty(3),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: vec![],
            });
        body.blocks[0].operations.push(Operation::unsuspended(
            operation,
            OperationResult::Scalar(scalar(3, unsigned)),
            OperationKind::PrimitiveScalarRead {
                source: root,
                path: [1, 2]
                    .into_iter()
                    .map(|index| StructuralPathSegment::RuntimeIndex {
                        index: value(index),
                        obligation: obligation(index),
                    })
                    .collect(),
            },
        ));
        module.machines.push(body);

        let outer_index = ScalarTerm::value(value(1), ScalarType::Integer(unsigned));
        let inner_index = ScalarTerm::value(value(2), ScalarType::Integer(signed));
        let seals = [
            Proposition::LessThan(
                outer_index,
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2)).unwrap(),
            ),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed, IntegerValue::Signed(0)).unwrap(),
                    inner_index.clone(),
                ),
                Proposition::LessThan(
                    inner_index,
                    ScalarTerm::integer(signed, IntegerValue::Signed(5)).unwrap(),
                ),
            ]),
        ]
        .into_iter()
        .zip(1..)
        .map(|(goal, id)| {
            ObligationEvidenceSeal::seal(
                machine,
                obligation(id),
                goal.clone(),
                vec![goal.clone()],
                vec![],
                CertificateEnvelope {
                    identity: EvidenceIdentity::new(id).unwrap(),
                    proof_system_marker: terminal_psi::ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::Assumption { index: 0 },
                    },
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
        let validator = EvidenceSealValidator::new(&module, &seals);
        assert!(validator.validate_all().is_ok());
        for (position, seal) in seals.iter().enumerate() {
            assert_eq!(
                validator
                    .fingerprint(machine, operation, seal.obligation)
                    .unwrap(),
                Some((position, seal.fingerprint))
            );
        }

        // The same indexed path can be the shared discriminator read. It
        // executes after the block's operations, before either case edge.
        let mut dispatch = module.clone();
        let OperationKind::PrimitiveScalarRead { path, .. } =
            &module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        let source_block = dispatch.machines[0].blocks[0].id;
        let cases = [1, 2].map(|raw| semantic_vocabulary::StructuralCaseId::new(raw).unwrap());
        dispatch.structural_types[0].shape = StructuralTypeShape::Sum {
            cases: cases
                .iter()
                .map(|case| terminal_psi::StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: *case,
                    identity: TerminalIdentityIndex::ABSENT,
                    fields: vec![],
                })
                .collect(),
        };
        dispatch.machines[0].blocks[0].operations.clear();
        dispatch.machines[0].blocks[0].terminator = terminal_psi::Terminator::StructuralCase {
            source: root,
            path: path.clone(),
            cases: cases
                .iter()
                .enumerate()
                .map(
                    |(position, case)| terminal_psi::StructuralCaseSuccessorEdge {
                        edge: semantic_vocabulary::EdgeId::new(position as u64 + 1).unwrap(),
                        target: semantic_vocabulary::BlockId::new(position as u64 + 2).unwrap(),
                        case: *case,
                        payload_fields: vec![],
                        structural_arguments: vec![],
                        cleanup_actions: vec![],
                    },
                )
                .collect(),
        };
        for raw in [2, 3] {
            let mut arm = crate::semantic_module::entry_tests::unit_machine(machine)
                .blocks
                .remove(0);
            arm.id = semantic_vocabulary::BlockId::new(raw).unwrap();
            arm.terminator = terminal_psi::Terminator::ReturnUnit {
                edge: semantic_vocabulary::EdgeId::new(raw + 1).unwrap(),
                cleanup_actions: vec![],
            };
            dispatch.machines[0].blocks.push(arm);
        }
        let validator = EvidenceSealValidator::new(&dispatch, &seals);
        validator.validate_all().unwrap();
        for (position, seal) in seals.iter().enumerate() {
            assert_eq!(
                validator
                    .fingerprint(machine, ExecutionSite::Block(source_block), seal.obligation)
                    .unwrap(),
                Some((position, seal.fingerprint))
            );
            // Operation 1 and block 1 have the same number but distinct sites.
            assert!(matches!(
                validator.fingerprint(machine, operation, seal.obligation),
                Err(EvidenceSealValidationError::SealMismatch { .. })
            ));
        }
        let mut duplicate_owner = dispatch.clone();
        duplicate_owner.machines[0].blocks[0].operations =
            module.machines[0].blocks[0].operations.clone();
        assert!(matches!(
            EvidenceSealValidator::new(&duplicate_owner, &seals).validate_all(),
            Err(EvidenceSealValidationError::UnownedSeal { .. })
        ));
        let mut changed_extent = dispatch.clone();
        let StructuralTypeShape::FixedArray { length, .. } =
            &mut changed_extent.structural_types[1].shape
        else {
            unreachable!()
        };
        *length = ArrayExtent::Literal(4);
        assert!(matches!(
            EvidenceSealValidator::new(&changed_extent, &seals).validate_all(),
            Err(EvidenceSealValidationError::SealMismatch { .. })
        ));
        let mut changed_selector = dispatch.clone();
        let terminal_psi::Terminator::StructuralCase { path, .. } =
            &mut changed_selector.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        path[1] = StructuralPathSegment::RuntimeIndex {
            index: value(1),
            obligation: obligation(2),
        };
        assert!(matches!(
            EvidenceSealValidator::new(&changed_selector, &seals).validate_all(),
            Err(EvidenceSealValidationError::SealMismatch { .. })
        ));

        let mut narrower = module.clone();
        let StructuralTypeShape::FixedArray { length, .. } =
            &mut narrower.structural_types[1].shape
        else {
            unreachable!()
        };
        *length = ArrayExtent::Literal(4);
        assert!(matches!(
            EvidenceSealValidator::new(&narrower, &seals).validate_all(),
            Err(EvidenceSealValidationError::SealMismatch { obligation: bound, .. })
                if bound == obligation(2)
        ));

        let mut ambiguous = module.clone();
        let places = &mut ambiguous.machines[0].structural_places;
        places.push(places[0]);
        assert!(matches!(
            EvidenceSealValidator::new(&ambiguous, &seals).validate_all(),
            Err(EvidenceSealValidationError::SealMismatch { .. })
        ));

        for selector in [value(2), value(99)] {
            let OperationKind::PrimitiveScalarRead { path, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                unreachable!()
            };
            path[1] = StructuralPathSegment::RuntimeIndex {
                index: selector,
                obligation: obligation(1),
            };
            assert!(matches!(
                EvidenceSealValidator::new(&module, &seals[..1]).validate_all(),
                Err(EvidenceSealValidationError::UnownedSeal { obligation: bound, .. })
                    if bound == obligation(1)
            ));
        }
    }

    #[test]
    fn projection_preserves_original_identity_coordinates_and_fingerprints() {
        let (module, evidence) = module_and_seals();
        let projected = evidence
            .project_for_machines(&module, &[machine_id(2)])
            .unwrap();
        assert_eq!(projected.module, evidence.module);
        assert_eq!(projected.seals, vec![evidence.seals[1].clone()]);
        let validator = EvidenceSealValidator::new(&module, &projected.seals);
        assert_eq!(
            validator
                .fingerprint(
                    machine_id(2),
                    OperationId::new(23).unwrap(),
                    ObligationId::new(2).unwrap()
                )
                .unwrap(),
            Some((0, evidence.seals[1].fingerprint)),
        );
        assert_eq!(
            validator
                .fingerprint(
                    machine_id(1),
                    OperationId::new(13).unwrap(),
                    ObligationId::new(1).unwrap()
                )
                .unwrap(),
            None,
        );
    }

    #[test]
    fn invalid_dormant_fingerprint_and_intact_wrong_goal_are_refused() {
        let (module, evidence) = module_and_seals();
        let mut damaged = evidence.clone();
        damaged.seals[1].fingerprint[0] ^= 1;
        assert!(
            matches!(damaged.project_for_machines(&module, &[machine_id(1)]),
            Err(EvidenceSealValidationError::SealMismatch { machine, .. }) if machine == machine_id(2))
        );
        let mut wrong_goal = evidence;
        let seal = &mut wrong_goal.seals[1];
        seal.goal = Proposition::Truth;
        seal.fingerprint = seal.recompute().unwrap();
        assert!(seal.intact().unwrap());
        assert!(
            matches!(wrong_goal.project_for_machines(&module, &[machine_id(1)]),
            Err(EvidenceSealValidationError::SealMismatch { machine, .. }) if machine == machine_id(2))
        );
    }

    #[test]
    fn duplicate_and_unowned_dormant_seals_are_refused() {
        let (module, evidence) = module_and_seals();
        let mut duplicate = evidence.clone();
        duplicate.seals.push(duplicate.seals[1].clone());
        assert!(
            matches!(duplicate.project_for_machines(&module, &[machine_id(1)]),
            Err(EvidenceSealValidationError::DuplicateSeal { machine, .. }) if machine == machine_id(2))
        );
        let mut unowned = evidence;
        unowned.seals[1].obligation = ObligationId::new(99).unwrap();
        unowned.seals[1].fingerprint = unowned.seals[1].recompute().unwrap();
        assert!(
            matches!(unowned.project_for_machines(&module, &[machine_id(1)]),
            Err(EvidenceSealValidationError::UnownedSeal { machine, .. }) if machine == machine_id(2))
        );
    }

    #[test]
    fn unknown_selected_machine_is_refused_even_without_seals() {
        let (module, mut evidence) = module_and_seals();
        for empty in [false, true] {
            if empty {
                evidence.seals.clear();
            }
            assert_eq!(
                evidence.project_for_machines(&module, &[machine_id(99)]),
                Err(EvidenceSealValidationError::UnknownSelectedMachine(
                    machine_id(99)
                )),
            );
        }
    }

    #[test]
    fn same_machine_ids_cannot_rebind_a_bundle_to_another_module() {
        let (module, evidence) = module_and_seals();
        let mut another = module.clone();
        another.machines[0].blocks[0].operations[0].kind = OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7),
        };
        let another_bundle = EvidenceSeals::for_module(&another, evidence.seals.clone()).unwrap();
        assert_ne!(evidence.module, another_bundle.module);
        assert_eq!(
            evidence.project_for_machines(&another, &[machine_id(1)]),
            Err(EvidenceSealValidationError::ModuleIdentityMismatch)
        );
        assert_eq!(
            another_bundle.project_for_machines(&module, &[machine_id(1)]),
            Err(EvidenceSealValidationError::ModuleIdentityMismatch)
        );
    }

    #[test]
    fn receiving_a_seal_requires_one_unique_operation_owner() {
        let (mut module, evidence) = module_and_seals();
        let mut duplicate = module.machines[0].blocks[0].operations[2].clone();
        duplicate.id = OperationId::new(14).unwrap();
        if let OperationResult::Scalar(result) = &mut duplicate.result {
            result.id = ValueId::new(14).unwrap();
        }
        module.machines[0].blocks[0].operations.push(duplicate);
        let validator = EvidenceSealValidator::new(&module, &evidence.seals);
        assert!(matches!(
            validator.fingerprint(
                machine_id(1),
                OperationId::new(13).unwrap(),
                ObligationId::new(1).unwrap()
            ),
            Err(EvidenceSealValidationError::SealMismatch { .. })
        ));
    }

    #[test]
    fn receiving_a_seal_for_a_different_actual_operation_is_refused() {
        let (module, evidence) = module_and_seals();
        let validator = EvidenceSealValidator::new(&module, &evidence.seals);
        assert!(matches!(
            validator.fingerprint(
                machine_id(1),
                OperationId::new(11).unwrap(),
                ObligationId::new(1).unwrap()
            ),
            Err(EvidenceSealValidationError::SealMismatch { .. })
        ));
    }
}
