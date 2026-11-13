/*
  Verified Psi optimization-unit construction (`build_verified_psi_optimization_unit`,
  `VerifiedPsiOptimizationUnit`).

  Omega 01 reconstructs and validates its mutable control-flow, value,
  ownership, and fact representation (`PsiOptimizationUnit`) from the admitted
  input Omega 00 hands it (`VerifiedPsiOptimizationInput`) before abstract
  rewrites run. The input type lives in `verified-optimization-input`, so this
  stage builds its own working form without depending on Omega 00.
*/
mod error {
    use semantic_vocabulary::{MachineId, ObligationId, OperationId};
    use terminal_codec::CodecError;
    #[derive(Debug)]
    pub enum VerifiedPsiOptimizationUnitBuildError {
        Unit(optimization_unit::OptimizationUnitBuildError),
        MissingReconstructedObligation {
            machine: MachineId,
            operation: OperationId,
            obligation: ObligationId,
        },
        MissingAcceptedObligation {
            machine: MachineId,
            operation: OperationId,
            obligation: ObligationId,
        },
        PropositionCodec(CodecError),
        FactIndex(optimization_unit::AcceptedObligationFactIndexError),
        ProofQuestionIndex(optimization_unit::ProofQuestionIndexError),
        MissingStructuralCatalogMachine(MachineId),
        /// An evidence seal does not match the operation obligation it names:
        /// its parts changed after sealing, the operation is not in the module,
        /// or the module's own goal for that operation differs from the sealed goal.
        EvidenceSealMismatch {
            machine: MachineId,
            operation: OperationId,
            obligation: ObligationId,
        },
        /// An evidence seal names no accepted operation obligation of the unit.
        UnattachedEvidenceSeal {
            machine: MachineId,
            obligation: ObligationId,
        },
    }

    impl std::fmt::Display for VerifiedPsiOptimizationUnitBuildError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "cannot construct verified Psi optimization unit: {self:?}"
            )
        }
    }

    impl std::error::Error for VerifiedPsiOptimizationUnitBuildError {}

    impl From<optimization_unit::OptimizationUnitBuildError> for VerifiedPsiOptimizationUnitBuildError {
        fn from(error: optimization_unit::OptimizationUnitBuildError) -> Self {
            Self::Unit(error)
        }
    }

    impl From<CodecError> for VerifiedPsiOptimizationUnitBuildError {
        fn from(error: CodecError) -> Self {
            Self::PropositionCodec(error)
        }
    }

    impl From<optimization_unit::AcceptedObligationFactIndexError>
        for VerifiedPsiOptimizationUnitBuildError
    {
        fn from(error: optimization_unit::AcceptedObligationFactIndexError) -> Self {
            Self::FactIndex(error)
        }
    }

    impl From<optimization_unit::ProofQuestionIndexError> for VerifiedPsiOptimizationUnitBuildError {
        fn from(error: optimization_unit::ProofQuestionIndexError) -> Self {
            Self::ProofQuestionIndex(error)
        }
    }
}

pub use error::VerifiedPsiOptimizationUnitBuildError;

use optimization_unit::{AcceptedObligationFact, PsiOptimizationUnit};
use std::collections::BTreeSet;
use verified_optimization_input::VerifiedPsiOptimizationInput;

pub fn build_verified_psi_optimization_unit(
    input: VerifiedPsiOptimizationInput,
    work_schedule: semantic_vocabulary::WorkScheduleIdentity,
) -> Result<VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError> {
    let mut seed =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), work_schedule)?;
    let context = input.context();
    optimization_unit::attach_verified_module_context(&mut seed, context.module())
        .map_err(VerifiedPsiOptimizationUnitBuildError::MissingStructuralCatalogMachine)?;
    let seals = input.evidence_seals();
    let facts = source_checked_obligation_facts(&seed, context.module(), seals)?;
    if std::env::var_os("OMEGA_PROOF_MEASUREMENTS").is_some() {
        eprint!(
            "{}",
            product_obligation_census(context.module(), &facts, seals.len())
        );
    }
    let unit = optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}

/// The census that `wiki/drafts/designs/check_on_lowered_form.md` asks for
/// before route one: how many obligations the product accepts, and how many of
/// those carry evidence. It prints one line per kind of owning Terminal
/// operation and reads only what this stage already built, so verdicts and
/// artifacts are unchanged. "Evidence" means a nonzero proof-bundle
/// fingerprint on the accepted fact; a reconstructed goal is counted apart
/// because it states the obligation without proving it. A call's `requires`
/// obligations travel on the call and never become accepted facts here, so
/// they are counted as carried, with the callee's source-checked `requires`.
fn product_obligation_census(
    module: &terminal_psi::TerminalModule,
    facts: &[AcceptedObligationFact],
    seals: usize,
) -> String {
    use semantic_vocabulary::{MachineId, OperationId};
    use std::collections::BTreeMap;
    let mut owners: Vec<((MachineId, OperationId), &'static str)> = Vec::new();
    let mut carried_requires = 0usize;
    let mut source_checked_requires = 0usize;
    for machine in &module.machines {
        source_checked_requires += machine.contract.source_checked_requires.len();
        for block in &machine.blocks {
            carried_requires += block
                .boundary_invocations()
                .map(|call| call.requirement_obligations.len())
                .sum::<usize>();
            for operation in &block.operations {
                owners.push((
                    (machine.id, operation.id),
                    terminal_semantics::OperationSemanticTag::for_operation(&operation.kind).name(),
                ));
                carried_requires += match &operation.kind {
                    terminal_psi::OperationKind::CallDynamic {
                        requirement_obligations,
                        ..
                    } => requirement_obligations.len(),
                    kind => kind
                        .module_invocation()
                        .map_or(0, |call| call.requirement_obligations.len()),
                };
            }
        }
    }
    owners.sort_unstable();
    // ponytail: string-keyed tally, measurement output only; never feeds a decision.
    let mut kinds: BTreeMap<&'static str, [usize; 3]> = BTreeMap::new();
    for fact in facts {
        let kind = match fact.owner {
            optimization_unit::PsiProvenance::Operation(operation) => owners
                .binary_search_by_key(&(fact.machine, operation), |(key, _)| *key)
                .map_or("unknown_operation", |found| owners[found].1),
            optimization_unit::PsiProvenance::Edge(_) => "EdgeRuntimeIndex",
        };
        let row = kinds.entry(kind).or_default();
        row[0] += 1;
        row[1] += usize::from(fact.proof_bundle_fingerprint != [0; 32]);
        row[2] += usize::from(!fact.proposition.is_empty());
    }
    let mut census = String::new();
    for (kind, [accepted, evidence, goal]) in kinds {
        census.push_str(&format!(
            "product_obligation_census kind={kind} accepted={accepted} \
             with_evidence={evidence} reconstructed_goal={goal}\n"
        ));
    }
    census.push_str(&format!(
        "product_obligation_census kind=CallRequires carried={carried_requires} \
         accepted=0 with_evidence=0 callee_source_checked_requires={source_checked_requires}\n"
    ));
    // The whole-module count: accepted facts that carry a checked evidence
    // seal, with their denominator. Only a module from the lowered-form route
    // arrives with seals; every other module reports zero.
    let sealed = facts
        .iter()
        .filter(|fact| fact.proof_bundle_fingerprint != [0; 32])
        .count();
    census.push_str(&format!(
        "product_obligation_census total accepted={} with_evidence={sealed} seals={seals}\n",
        facts.len()
    ));
    census
}

fn source_checked_obligation_facts(
    unit: &PsiOptimizationUnit,
    module: &terminal_psi::TerminalModule,
    seals: &[terminal_codec::ObligationEvidenceSeal],
) -> Result<Vec<AcceptedObligationFact>, VerifiedPsiOptimizationUnitBuildError> {
    let mut coordinates = BTreeSet::new();
    let validator = terminal_codec::EvidenceSealValidator::new(module, seals);
    let mut attached = vec![false; seals.len()];
    let mut facts = Vec::new();
    for function in &unit.functions {
        for fact in &function.facts {
            let optimization_unit::OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner = optimization_unit::PsiProvenance::Operation(*support);
            if coordinates.insert((function.machine, owner, *obligation)) {
                let representation =
                    function.block_nodes().find_map(|node| {
                        let abstract_operations::AbstractOperation::EstablishElementView {
                            psi_operation,
                            source,
                            element,
                            region:
                                region @ terminal_psi::ElementViewRegion::WholeRepresentation {
                                    source_length,
                                    obligation: count_obligation,
                                },
                            ..
                        } = &node.operation
                        else {
                            return None;
                        };
                        (*psi_operation == *support && *count_obligation == *obligation)
                            .then_some((source, element, region, source_length))
                    });
                let proposition = if let Some((source, element, region, source_length)) =
                    representation
                {
                    let missing =
                        || VerifiedPsiOptimizationUnitBuildError::MissingReconstructedObligation {
                            machine: function.machine,
                            operation: *support,
                            obligation: *obligation,
                        };
                    let root = optimization_unit::field_evidence(function, source.place)
                        .and_then(|row| row.root_type)
                        .ok_or_else(missing)?;
                    let collection = terminal_semantics::runtime_structural_path_tip(
                        unit.structural_types.iter(),
                        root,
                        &source.path,
                    )
                    .ok_or_else(missing)?;
                    let Some(terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                        source: source_layout,
                        target: target_layout,
                        ..
                    }) = terminal_semantics::element_view_shape_region_geometry(
                        unit.structural_types.iter(),
                        &collection.shape,
                        *element,
                        *region,
                        source.access,
                    )
                    else {
                        return Err(missing());
                    };
                    let factor = source_layout
                        .size
                        .checked_div(target_layout.size)
                        .ok_or_else(missing)?;
                    let goal =
                        terminal_semantics::whole_representation_count_goal(*source_length, factor)
                            .ok_or_else(missing)?;
                    terminal_codec::canonical_proposition_order_key(&goal)?
                } else {
                    Vec::new()
                };
                let fingerprint = sealed_fingerprint(
                    &validator,
                    &mut attached,
                    function.machine,
                    *support,
                    *obligation,
                )?;
                let fact = AcceptedObligationFact::new(
                    unit.psi,
                    fingerprint,
                    function.machine,
                    owner,
                    *obligation,
                    proposition,
                );
                let receiving = function.block_nodes().find_map(|node| {
                    let (operation, retained_obligation, value, restriction) = match &node.operation
                    {
                        abstract_operations::AbstractOperation::WriteOnlyPrimitiveStore {
                            psi_operation,
                            destination,
                            path,
                            value,
                            range_obligation,
                        } => (
                            *psi_operation,
                            *range_obligation,
                            value.value,
                            {
                                let (whole, projected) = match destination {
                                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
                                    | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { parameter, .. } => (&parameter.qualifications, &parameter.projected_qualifications),
                                    abstract_operations::StructuralScalarStoreDestination::OwnedResult { result, .. } => (&result.qualifications, &result.projected_qualifications),
                                };
                                terminal_semantics::primitive_store_restriction(unit.structural_types.iter(), unit.structural_domains.iter(), destination.structural_type(), whole, projected, destination.additional_input_qualifications(), path)??
                            },
                        ),
                        abstract_operations::AbstractOperation::EstablishPrimitiveLocal {
                            psi_operation,
                            result,
                            value,
                            range_obligation,
                        } => (
                            *psi_operation,
                            *range_obligation,
                            value.value,
                            terminal_semantics::primitive_projection_restriction(
                                unit.structural_types.iter(),
                                result.structural_type,
                                &[],
                            )?.clone(),
                        ),
                        abstract_operations::AbstractOperation::AtomicEvent { psi_operation, event, resident_obligation, .. } => {
                            let location = event.location()?;
                            let root = optimization_unit::field_evidence(function, location.root)?.root_type?;
                            let cell = terminal_semantics::atomic_cell_type(unit.structural_types.iter(), root, &location.path)?;
                            let value = event.replacement().or(match event {
                                abstract_operations::AbstractAtomicEvent::ReadModifyWrite { operand, .. } => Some(*operand),
                                _ => None,
                            })?;
                            (*psi_operation, *resident_obligation, value, terminal_psi::StructuralFieldType::from(cell))
                        },
                        _ => return None,
                    };
                    if operation != *support || retained_obligation != Some(*obligation) {
                        return None;
                    }
                    Some(optimization_unit::SourceCheckedScalarReceiving {
                        value,
                        restriction: restriction.clone(),
                    })
                });
                facts.push(match receiving {
                    Some(receiving) => fact.with_scalar_receiving(receiving),
                    None => fact,
                });
            }
        }
        for node in function.block_nodes() {
            let paths = node
                .successors
                .iter()
                .flat_map(|edge| {
                    edge.structural_bindings
                        .iter()
                        .map(move |binding| (edge.psi_edge, binding.argument.path.as_slice()))
                })
                .chain(
                    node.operation
                        .cleanup_rosters()
                        .flat_map(|(edge, actions)| {
                            actions.iter().map(move |action| (edge, action.path()))
                        }),
                );
            for (edge, path) in paths {
                for (_, obligation) in path
                    .iter()
                    .filter_map(terminal_psi::StructuralPathSegment::runtime_index)
                {
                    let owner = optimization_unit::PsiProvenance::Edge(edge);
                    if coordinates.insert((function.machine, owner, obligation)) {
                        facts.push(AcceptedObligationFact::new(
                            unit.psi,
                            [0; 32],
                            function.machine,
                            owner,
                            obligation,
                            Vec::new(),
                        ));
                    }
                }
            }
        }
    }
    if let Some(position) = attached.iter().position(|used| !used) {
        let seal = &seals[position];
        return Err(
            VerifiedPsiOptimizationUnitBuildError::UnattachedEvidenceSeal {
                machine: seal.machine,
                obligation: seal.obligation,
            },
        );
    }
    Ok(facts)
}

/// The proof-bundle fingerprint of one accepted operation obligation: zero
/// when no seal names it, as for every module the current pipeline produces.
///
/// A seal is used only after three checks. Its fingerprint must be the
/// fingerprint of its own parts (goal, hypotheses, certificate). The
/// operation must be in this module and own this obligation. The goal Terminal
/// semantics states for that operation must equal the sealed goal. What is
/// still taken on faith: that the proof kernel accepted the certificate under
/// those hypotheses, and that the hypotheses held where the obligation sits.
/// This stage does not re-run the kernel.
fn sealed_fingerprint(
    validator: &terminal_codec::EvidenceSealValidator<'_, '_>,
    attached: &mut [bool],
    machine: semantic_vocabulary::MachineId,
    operation: semantic_vocabulary::OperationId,
    obligation: semantic_vocabulary::ObligationId,
) -> Result<[u8; 32], VerifiedPsiOptimizationUnitBuildError> {
    let found = validator
        .fingerprint(machine, operation, obligation)
        .map_err(|error| match error {
            terminal_codec::EvidenceSealValidationError::Codec(error) => {
                VerifiedPsiOptimizationUnitBuildError::PropositionCodec(error)
            }
            _ => VerifiedPsiOptimizationUnitBuildError::EvidenceSealMismatch {
                machine,
                operation,
                obligation,
            },
        })?;
    let Some((position, fingerprint)) = found else {
        return Ok([0; 32]);
    };
    attached[position] = true;
    Ok(fingerprint)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationUnit {
    input: VerifiedPsiOptimizationInput,
    unit: PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationUnit {
    pub const fn input(&self) -> &VerifiedPsiOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(self) -> (VerifiedPsiOptimizationInput, PsiOptimizationUnit) {
        (self.input, self.unit)
    }
}

#[cfg(test)]
mod evidence_seal_tests {
    // A scalar program from the lowered-form route, discharged with kernel
    // evidence, reaches this stage with its seals in memory: the accepted
    // fact carries the seal's fingerprint, a module without seals keeps the
    // zero fingerprint, and a seal that does not match is refused.
    use semantic_vocabulary::{
        IdTable, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, Proposition,
        ScalarTerm, ScalarType,
    };
    use structure_elaboration::{
        Body, CrashCeiling, Expression, ExpressionKind, Signature, SourceOrigin, Statement, Strict,
        Target, Termination, Unpositioned,
    };
    use terminal_codec::{EvidenceSeals, ObligationEvidenceSeal};

    use super::{VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit};
    use terminal_psi_to_abstract_operations::{
        ArtifactSections, lower_artifact, lower_module_with_evidence,
    };
    use verified_optimization_input::{ObligationEvidence, VerifiedPsiOptimizationInput};

    fn u32_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    fn slot(slot: usize) -> Expression {
        Expression {
            kind: ExpressionKind::Slot(slot),
            result: Some(u32_type().into()),
            source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        }
    }

    fn at_most(position: u64) -> Proposition {
        let ScalarType::Integer(integer) = u32_type() else {
            unreachable!()
        };
        Proposition::LessOrEqual(
            ScalarTerm::value(
                semantic_vocabulary::ValueId::new(position).unwrap(),
                u32_type(),
            ),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap(),
        )
    }

    /// `add(a, b) requires a <= 100, b <= 100 { return a + b }`, lowered,
    /// discharged and sealed.
    fn sealed_add() -> (terminal_psi::TerminalModule, Vec<ObligationEvidenceSeal>) {
        let sum = Expression {
            kind: ExpressionKind::Strict {
                operation: Strict::Add,
                left: Box::new(slot(0)),
                right: Box::new(slot(1)),
            },
            result: Some(u32_type().into()),
            source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
        };
        let mut bodies: IdTable<MachineId, Body> = IdTable::default();
        bodies.push(Body {
            slots: vec![u32_type().into(), u32_type().into()],
            statements: vec![Statement::Arm {
                guard: None,
                target: Target::Value(sum),
                source: SourceOrigin::Unpositioned(Unpositioned::SpanAbsent),
            }],
        });
        let mut signatures: IdTable<MachineId, Signature> = IdTable::default();
        signatures.push(Signature {
            parameters: vec![u32_type().into(), u32_type().into()],
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_type().into()),
            requires: vec![at_most(1), at_most(2)],
            ensures: Vec::new(),
            reaches: structure_elaboration::Ceiling::Inferred,
            invokes: structure_elaboration::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
        let entry = MachineId::new(1).unwrap();
        let mut elaboration =
            structure_elaboration::lower_bodies(entry, &bodies, &signatures, &IdTable::default())
                .unwrap();
        let discharge =
            lowered_discharge::discharge(&mut elaboration, &mut Default::default()).unwrap();
        let seals = discharge.operation_seals(&elaboration).unwrap();
        assert_eq!(seals.len(), 1, "the add's safety obligation is discharged");
        (elaboration.module, seals)
    }

    /// Lowers `module` in memory with `seals`, the only way to attach them.
    fn build(
        module: &terminal_psi::TerminalModule,
        seals: Vec<ObligationEvidenceSeal>,
    ) -> Result<Vec<[u8; 32]>, VerifiedPsiOptimizationUnitBuildError> {
        let evidence = EvidenceSeals::for_module(module, seals).unwrap();
        let input = lower_module_with_evidence(module, evidence)
            .unwrap()
            .into_optimization_artifact()
            .into_optimization_input();
        fingerprints(input)
    }

    /// Lowers `module` from its bytes, as a stored artifact or a cache read is.
    fn build_from_bytes(
        module: &terminal_psi::TerminalModule,
    ) -> Result<Vec<[u8; 32]>, VerifiedPsiOptimizationUnitBuildError> {
        let bytes = terminal_codec::encode_module(module).unwrap();
        let input = lower_artifact(ArtifactSections {
            semantic_bytes: &bytes,
        })
        .unwrap()
        .into_optimization_artifact()
        .into_optimization_input();
        assert_eq!(input.evidence(), &ObligationEvidence::ReadFromBytes);
        fingerprints(input)
    }

    fn fingerprints(
        input: VerifiedPsiOptimizationInput,
    ) -> Result<Vec<[u8; 32]>, VerifiedPsiOptimizationUnitBuildError> {
        let (_, unit) = build_verified_psi_optimization_unit(
            input,
            psi_work::WorkSchedule::CURRENT.identity(),
        )?
        .into_parts();
        Ok(unit
            .accepted_obligation_facts
            .iter()
            .map(|fact| fact.proof_bundle_fingerprint)
            .collect())
    }

    #[test]
    fn a_checked_seal_fills_the_fingerprint_and_no_seal_keeps_zero() {
        let (module, seals) = sealed_add();
        let expected = seals[0].fingerprint;
        assert_eq!(build(&module, seals).unwrap(), vec![expected]);
        assert_eq!(build(&module, Vec::new()).unwrap(), vec![[0; 32]]);
    }

    #[test]
    fn a_module_read_back_from_bytes_has_no_seals_and_zero_fingerprints() {
        // The same module, after a byte round trip as a cache read does it: no
        // error, no evidence, and no way to attach the seals made before.
        let (module, _seals) = sealed_add();
        assert_eq!(build_from_bytes(&module).unwrap(), vec![[0; 32]]);
    }

    #[test]
    fn seals_for_another_module_are_refused() {
        // Seals from an earlier build name that build's module identity; a
        // changed module (here a different `requires` bound) does not accept
        // them.
        let (module, seals) = sealed_add();
        let evidence = EvidenceSeals::for_module(&module, seals).unwrap();
        let mut changed = module.clone();
        changed.machines[0].contract.requires.pop();
        let refused = lower_module_with_evidence(&changed, evidence)
            .expect_err("seals for another module are refused");
        assert_eq!(format!("{refused:?}"), "EvidenceForAnotherModule");
    }

    #[test]
    fn a_seal_that_does_not_match_is_refused() {
        let (module, seals) = sealed_add();
        // Changed after sealing: the fingerprint no longer matches its parts.
        let mut altered = seals.clone();
        altered[0].goal = Proposition::Truth;
        assert!(matches!(
            build(&module, altered),
            Err(VerifiedPsiOptimizationUnitBuildError::EvidenceSealMismatch { .. })
        ));
        // Sealed intact, but for a goal the module does not state.
        let original = &seals[0];
        let resealed = ObligationEvidenceSeal::seal(
            original.machine,
            original.obligation,
            Proposition::Truth,
            original.assumptions.clone(),
            original.axioms.clone(),
            original.certificate.clone(),
        )
        .unwrap();
        assert!(matches!(
            build(&module, vec![resealed]),
            Err(VerifiedPsiOptimizationUnitBuildError::EvidenceSealMismatch { .. })
        ));
        // A seal for an obligation no accepted fact owns.
        let mut stray = seals[0].clone();
        stray.obligation = ObligationId::new(99).unwrap();
        let stray = ObligationEvidenceSeal::seal(
            stray.machine,
            stray.obligation,
            stray.goal,
            stray.assumptions,
            stray.axioms,
            stray.certificate,
        )
        .unwrap();
        assert!(matches!(
            build(&module, vec![stray]),
            Err(VerifiedPsiOptimizationUnitBuildError::UnattachedEvidenceSeal { .. })
        ));
    }
}

#[cfg(test)]
mod qualified_cleanup_receiving_tests {
    use terminal_codec::{CanonicalTerminalArtifact, build_identity_optimization_execution_record};
    use terminal_psi::{
        Operation, OperationKind, OperationResult, RecordFieldValue, StructuralAccess,
        TerminalAffineCleanupAction, TerminalModule, Terminator,
    };

    fn independently_validate_ownership(module: TerminalModule) -> Result<(), String> {
        let record = build_identity_optimization_execution_record(&module)
            .expect("foundation-valid module has an identity record");
        let artifact = CanonicalTerminalArtifact::from_parts(module, record, None)
            .expect("foundation-valid mutation can be encoded");
        let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
            .expect("mutant must reach semantic validation after canonical admission");
        let semantic_bytes = terminal_codec::encode_module(artifact.semantic_module())
            .expect("admitted module re-encodes");
        let input = terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic_bytes,
            },
        )
        .expect("artifact lowering converts data; it does not prove issuance")
        .into_optimization_artifact()
        .into_optimization_input();
        let verified = super::build_verified_psi_optimization_unit(
            input,
            psi_work::WorkSchedule::CURRENT.identity(),
        )
        .expect("construct exact unit before explicit semantic validation");
        let (_, unit) = verified.into_parts();
        let frontiers = optimization_unit_semantics::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(&unit, &[])
        .map_err(|error| format!("ownership reconstruction: {error:?}"))?;
        let unit = optimization_unit::attach_ownership_frontier_facts(unit, frontiers)
            .map_err(|error| format!("ownership attachment: {error:?}"))?;
        optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
            &unit,
            &[],
        )
        .map_err(|error| format!("semantic ownership validation: {error:?}"))
    }

    #[test]
    #[ignore = "explicit qualification supplies OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT"]
    fn qualified_owned_cleanup_replays_transport_and_single_disposal() {
        let path = std::env::var_os("OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT")
            .expect("supply actual public CLI canonical output");
        let bytes = std::fs::read(path).expect("read actual canonical bytes");
        let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
            .expect("independent canonical admission of actual artifact");
        independently_validate_ownership(artifact.semantic_module().clone())
            .expect("intact artifact must pass the explicit semantic validator");

        for constructor_child in [true, false] {
            let mut module = artifact.semantic_module().clone();
            let selected = module
                .machines
                .iter()
                .enumerate()
                .find_map(|(machine_index, machine)| {
                    let sources: Vec<_> = if constructor_child {
                        machine
                            .operations()
                            .flat_map(|operation| match &operation.kind {
                                OperationKind::EstablishRecord { fields }
                                | OperationKind::EstablishStructuralCase { fields, .. } => fields
                                    .iter()
                                    .filter_map(|field| match &field.value {
                                        RecordFieldValue::Structural(argument) => {
                                            Some(argument.place)
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>(),
                                _ => Vec::new(),
                            })
                            .collect()
                    } else {
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| match &block.terminator {
                                Terminator::Jump {
                                    structural_arguments,
                                    ..
                                } => structural_arguments
                                    .iter()
                                    .map(|argument| argument.place)
                                    .collect::<Vec<_>>(),
                                _ => Vec::new(),
                            })
                            .collect()
                    };
                    sources.into_iter().find_map(|place| {
                        let (operation, _) = machine.producing_operation(place)?;
                        let result = operation.result.as_structural()?;
                        (!result.qualifications.is_empty()).then_some((machine_index, operation.id))
                    })
                })
                .expect("fixture has an actual qualified source at this transport boundary");
            let producer = module.machines[selected.0]
                .operation(selected.1)
                .unwrap()
                .kind
                .direct_callee()
                .expect("actual qualified source is a call result");
            let terminal_psi::TerminalMachineResult::Structural(result) = &mut module
                .machines
                .iter_mut()
                .find(|machine| machine.id == producer)
                .unwrap()
                .result
            else {
                unreachable!()
            };
            assert!(!result.qualifications.is_empty());
            result.qualifications.clear();
            for machine in &mut module.machines {
                for block in &mut machine.blocks {
                    for operation in &mut block.operations {
                        if operation.kind.direct_callee() == Some(producer) {
                            let OperationResult::Structural(result) = &mut operation.result else {
                                unreachable!()
                            };
                            result.qualifications.clear();
                        }
                    }
                }
            }
            let reason = independently_validate_ownership(module).expect_err(
                "deleted actual source membership cannot satisfy unchanged destinations",
            );
            assert_eq!(
                reason,
                "ownership reconstruction: VerifiedOptimizationUnitProjectionMismatch"
            );
            println!(
                "weakened actual producer used by {} rejected at reported boundary: {reason}",
                if constructor_child {
                    "constructor-child"
                } else {
                    "owned-edge"
                }
            );
        }

        let mut module = artifact.semantic_module().clone();
        let next_id = semantic_vocabulary::OperationId::new(
            module
                .machines
                .iter()
                .flat_map(|machine| machine.operations())
                .map(|operation| operation.id.get())
                .max()
                .unwrap()
                .checked_add(1)
                .unwrap(),
        )
        .unwrap();
        let (machine_index, parameter_index, block_index, read_index) = module.machines.iter()
        .enumerate().find_map(|(mi, machine)| machine.structural_parameters.iter()
            .enumerate().find_map(|(pi, parameter)| {
                if parameter.access != StructuralAccess::SharedBorrow
                    || parameter.qualifications.is_empty() { return None; }
                machine.blocks.iter().enumerate().find_map(|(bi, block)|
                    block.operations.iter().enumerate().find_map(|(oi, operation)|
                        matches!(&operation.kind, OperationKind::ScalarStructuralField { source, .. }
                            if *source == parameter.place).then_some((mi, pi, bi, oi))))
            })).expect("fixture has an actual qualified scalar observer");
        let observer = &mut module.machines[machine_index];
        let observer_id = observer.id;
        let parameter = &mut observer.structural_parameters[parameter_index];
        let position = parameter.position as usize;
        parameter.access = StructuralAccess::MutableBorrow;
        parameter.qualifications.clear();
        assert!(parameter.projected_qualifications.is_empty());
        let block = &mut observer.blocks[block_index];
        let read = &block.operations[read_index];
        let value = read
            .result
            .as_scalar()
            .expect("actual read has a scalar result")
            .id;
        let OperationKind::ScalarStructuralField {
            source,
            path,
            field,
        } = &read.kind
        else {
            unreachable!()
        };
        assert!(path.is_empty(), "actual observer reads a direct field");
        let store = Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: next_id,
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: *source,
                path: Vec::new(),
                field: *field,
                value,
                range_obligation: None,
            },
        };
        block.operations.insert(read_index + 1, store);
        let mut calls = 0;
        for machine in &mut module.machines {
            for block in &mut machine.blocks {
                for operation in &mut block.operations {
                    if operation.kind.direct_callee() == Some(observer_id) {
                        let arguments = operation.kind.structural_arguments_mut().unwrap();
                        assert_eq!(arguments[position].access, StructuralAccess::SharedBorrow);
                        arguments[position].access = StructuralAccess::MutableBorrow;
                        calls += 1;
                    }
                }
            }
        }
        assert!(calls > 0, "mutate real calls to the actual observer");
        let reason = independently_validate_ownership(module)
            .expect_err("unqualified mutable call must not retain opaque caller membership");
        assert_eq!(
            reason,
            "ownership reconstruction: StructuralCallContractMismatch(OperationSite { machine: MachineId(1), block: BlockId(9), node: 0 })"
        );
        println!("mutating unqualified observer rejected: {reason}");

        let mut module = artifact.semantic_module().clone();
        let mut doubled = false;
        for machine in &mut module.machines {
            for block in &mut machine.blocks {
                let rosters: Vec<_> = match &mut block.terminator {
                    Terminator::Jump {
                        cleanup_actions, ..
                    } => vec![cleanup_actions],
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        vec![
                            &mut when_true.cleanup_actions,
                            &mut when_false.cleanup_actions,
                        ]
                    }
                    Terminator::StructuralCase { cases, .. } => cases
                        .iter_mut()
                        .map(|case| &mut case.cleanup_actions)
                        .collect(),
                    _ => Vec::new(),
                };
                for actions in rosters {
                    if let Some(action) = actions
                        .iter()
                        .find(|action| {
                            matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
                        })
                        .cloned()
                    {
                        actions.push(action);
                        doubled = true;
                        break;
                    }
                }
                if doubled {
                    break;
                }
            }
            if doubled {
                break;
            }
        }
        assert!(
            doubled,
            "fixture must contain a real residual cleanup action"
        );
        let reason = build_identity_optimization_execution_record(&module).expect_err(
            "canonical uniqueness rejects duplicate actual residuals before semantic replay",
        );
        assert!(matches!(
            &reason,
            terminal_codec::PsiOptimizationExecutionRecordBuildError::Semantic(
                terminal_codec::CodecError::NonCanonicalOrder(
                    "partial affine residual discards are unique"
                )
            )
        ));
        println!("double cleanup rejected by canonical foundation: {reason:?}");
    }
}
