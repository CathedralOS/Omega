//! Validated Terminal-SCC custody for loop consumers.
//!
//! The unranked fixture enters the validated SCC roster without any countdown
//! certificate. Every loop consumer must then key its countdown custody on the
//! certificate-bearing subset of that roster instead of requiring the roster
//! to be the exact countdown slice. The certified fixture then proves the
//! counted-loop summary projects its region, boundary edges, and trip count
//! from the same validated roster rather than a private loop-forest or
//! control-flow re-derivation.

use super::super::VerifiedPsiOptimizationSession;
use super::{AbstractOperation, OptimizationUnitIdentity, verified_unranked_cycle_unit};
use crate::{
    CountedLoopAnalysisError, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_unit::{ProvenanceDisposition, PsiProvenance, PsiRealizationSite};
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::{BlockId, MachineId, OperationId};

/// A certified countdown loop: the `remaining > 0` guard and `remaining - 1`
/// backedge carry verifier-admitted `Natural` ranking evidence that projects
/// to the exact unsigned countdown certificate, and the entry dispatch block
/// outside the component is its single preheader edge source.
const CERTIFIED_COUNTDOWN_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u64 in Wrapping, remaining: u64 [0..=5])
    terminates by remaining -> Nat::Descending;
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u64 in Wrapping, remaining: u64 [0..=5]) {
            transition remaining > 0 {
                true -> step(s, remaining - 1)
                _ -> finish(s)
            }
        }
        state finish(r: u64 in Wrapping) {}
    }
"#;

/// A countdown whose other carried value reads the pre-decrement counter
/// (`acc * n` beside `n - 1`), reached as a scalar callee. The `n > 0` guard
/// edge passes both carried values to the decrement block as that block's own
/// parameters, so the backedge subtracts the decrement block's parameter
/// rather than the header's.
const GUARD_BOUND_COUNTDOWN_SOURCE: &str = r#"
    data Root {
        result: u64;
    }

    machine Root::fact(&mut self, n: u64, acc: u64 in Wrapping)
    terminates by n;
    -> u64
    {
        transition n > 0 {
            true -> fact(n - 1, acc * (n as u64 in Wrapping))
            false -> (acc as u64)
        }
    }

    machine Root::main(&mut self) {
        self.result = self.fact(5, 1);
    }
"#;

fn certified_countdown_session() -> VerifiedPsiOptimizationSession {
    countdown_session(CERTIFIED_COUNTDOWN_SOURCE, "Root::scan")
}

fn countdown_session(source: &str, machine: &str) -> VerifiedPsiOptimizationSession {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize countdown");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse countdown");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve countdown");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type countdown");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check countdown");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(machine),
    )
    .expect("lower countdown");
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .expect("countdown optimizer admission");
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("countdown optimizer unit");
    VerifiedPsiOptimizationSession::new(verified).expect("countdown session")
}

#[test]
fn unranked_component_enters_the_validated_scc_roster_without_certificates() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let [component] = session.cycle_components().components() else {
        panic!("one validated Terminal SCC")
    };
    assert_eq!(component.id.machine, MachineId::new(501).unwrap());
    assert_eq!(component.members, [BlockId::new(505).unwrap()]);
    assert_eq!(component.id.internal_edges.len(), 1);
    let [entry] = component.entries.as_slice() else {
        panic!("unique entry")
    };
    assert_eq!(entry.source, BlockId::new(503).unwrap());
    let [exit] = component.exits.as_slice() else {
        panic!("single exit")
    };
    assert_eq!(exit.target, BlockId::new(511).unwrap());
    assert!(session.ranking_certificates().certificates().is_empty());
}

#[test]
fn counted_loop_chain_covers_only_certificate_bearing_components() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let counted = session.counted_loop_analysis().unwrap();
    assert!(counted.loops().is_empty());
    session
        .validate_counted_loop_analysis(counted.snapshot())
        .unwrap();
    let invariants = session.countdown_invariant_constant_analysis().unwrap();
    assert!(invariants.loops().is_empty());
    let placements = session
        .countdown_invariant_constant_placement_analysis()
        .unwrap();
    assert!(placements.loops().is_empty());
}

#[test]
fn counted_loop_replay_rejects_forged_rows_on_an_uncertified_roster() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let counted = session.counted_loop_analysis().unwrap();
    let mut forged = counted.snapshot().clone();
    forged.revision = OptimizationUnitIdentity::from_canonical_bytes(b"forged-revision");
    assert_eq!(
        session.validate_counted_loop_analysis(&forged),
        Err(CountedLoopAnalysisError::SnapshotMismatch)
    );
}

#[test]
fn scalar_leaf_motion_relocates_through_validated_scc_custody() {
    let session = VerifiedPsiOptimizationSession::new(verified_unranked_cycle_unit()).unwrap();
    let components = session.cycle_components().components().to_vec();
    let input = session.unit().identity;
    let candidates = propose_loop_invariant_scalar_motion(&session, 4).unwrap();
    let [candidate] = candidates.as_slice() else {
        panic!("one candidate for the single-entry unranked component")
    };
    assert_eq!(candidate.component(), &components[0].id);
    let [relocation] = candidate.relocations() else {
        panic!("the dead integer constant is the one admissible leaf")
    };
    assert_eq!(
        relocation.node().psi_operation(),
        OperationId::new(507).unwrap()
    );
    assert_eq!(
        relocation.node().location().block,
        BlockId::new(505).unwrap()
    );
    assert_eq!(
        relocation.node().provenance().first(),
        Some(&PsiProvenance::Operation(OperationId::new(507).unwrap()))
    );
    assert_eq!(relocation.destination().block, BlockId::new(503).unwrap());

    let validated = validate_loop_invariant_scalar_motion(&session, candidate).unwrap();
    let applied = apply_loop_invariant_scalar_motion(session, validated).unwrap();
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.candidate, candidate.identity());
    assert_eq!(record.input, input);
    assert_eq!(record.output, applied.session().unit().identity);
    assert!(record.provenance.iter().any(|row| {
        row.input == PsiRealizationSite::Node(relocation.node().location())
            && row.disposition
                == ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                    relocation.destination(),
                ))
            && row.sources.as_slice() == relocation.node().provenance()
            && row.fuel.as_slice() == relocation.node().fuel()
    }));

    // The transformed session keeps the same validated SCC roster and still
    // carries no countdown certificates; the leaf now sits in the preheader.
    let next = applied.session();
    assert_eq!(next.cycle_components().components(), components.as_slice());
    assert!(next.ranking_certificates().certificates().is_empty());
    assert!(next.counted_loop_analysis().unwrap().loops().is_empty());
    let function = &next.unit().functions[0];
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == BlockId::new(503).unwrap())
        .unwrap();
    assert!(matches!(
        preheader.nodes[0].operation,
        AbstractOperation::IntegerConstant { .. }
    ));
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == BlockId::new(505).unwrap())
        .unwrap();
    assert!(
        header
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::IntegerConstant { .. }))
    );
}

#[test]
fn certified_countdown_summary_projects_its_region_from_validated_scc_custody() {
    let session = certified_countdown_session();
    let [component] = session.cycle_components().components() else {
        panic!("one validated Terminal SCC")
    };
    let [certificate] = session.ranking_certificates().certificates() else {
        panic!("one countdown certificate on the certified component")
    };
    assert_eq!(certificate.component, component.id);
    let [entry] = component.entries.as_slice() else {
        panic!("unique entry edge into the certified component")
    };
    let [exit] = component.exits.as_slice() else {
        panic!("single exit edge out of the certified component")
    };

    let counted = session.counted_loop_analysis().unwrap();
    let [summary] = counted.loops() else {
        panic!("one counted-loop summary for the certificate")
    };
    // The summary's region is the validated Terminal-SCC custody projection
    // keyed by the certificate: member identity, boundary edges, and the
    // reducible header all come from the roster, not from a private
    // loop-forest or control-flow re-derivation.
    assert_eq!(summary.certificate, *certificate);
    assert_eq!(summary.region.header, Some(certificate.header));
    assert_eq!(summary.region.blocks, component.members);
    assert!(!summary.region.irreducible);
    assert_eq!(summary.preheader_edge, *entry);
    assert_eq!(summary.exit_edge, *exit);
    assert_eq!(summary.trip_count.scalar_type, certificate.rank_type);
    session
        .validate_counted_loop_analysis(counted.snapshot())
        .unwrap();
}

#[test]
fn counted_loop_replay_rejects_a_tampered_region_on_a_certified_roster() {
    let session = certified_countdown_session();
    let counted = session.counted_loop_analysis().unwrap();
    let mut forged = counted.snapshot().clone();
    forged.loops[0].region.header = None;
    assert_eq!(
        session.validate_counted_loop_analysis(&forged),
        Err(CountedLoopAnalysisError::SnapshotMismatch)
    );
    forged.loops[0].region.header = Some(forged.loops[0].certificate.header);
    forged.loops[0].region.irreducible = true;
    assert_eq!(
        session.validate_counted_loop_analysis(&forged),
        Err(CountedLoopAnalysisError::SnapshotMismatch)
    );
}

#[test]
fn a_guard_bound_countdown_counter_carries_its_certificate() {
    let session = countdown_session(GUARD_BOUND_COUNTDOWN_SOURCE, "Root::main");
    let [certificate] = session.ranking_certificates().certificates() else {
        panic!("one countdown certificate on the guard-bound component")
    };
    let descent = &certificate.descent;
    assert_ne!(descent.source_parameter, certificate.rank_parameter);
    let header = session
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find(|block| block.id == certificate.header)
        .expect("certificate header block");
    let Some(AbstractOperation::Conditional { when_true, .. }) =
        header.nodes.last().map(|node| &node.operation)
    else {
        panic!("countdown header ends in its guard")
    };
    assert!(when_true.bindings.iter().any(|binding| {
        binding.parameter == descent.source_parameter
            && binding.argument == certificate.rank_parameter
    }));
    session
        .validate_counted_loop_analysis(session.counted_loop_analysis().unwrap().snapshot())
        .unwrap();
}

#[test]
fn a_decrement_parameter_bound_from_another_value_carries_no_certificate() {
    let session = countdown_session(GUARD_BOUND_COUNTDOWN_SOURCE, "Root::main");
    let certificate = session.ranking_certificates().certificates()[0].clone();
    let (input, mut unit) = session.into_parts();
    let header = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|block| block.id == certificate.header)
        .expect("certificate header block");
    let other = header
        .parameters
        .iter()
        .map(|parameter| parameter.value)
        .find(|value| *value != certificate.rank_parameter)
        .expect("a second carried value");
    let Some(AbstractOperation::Conditional { when_true, .. }) =
        header.nodes.last_mut().map(|node| &mut node.operation)
    else {
        panic!("countdown header ends in its guard")
    };
    let binding = when_true
        .bindings
        .iter_mut()
        .find(|binding| binding.parameter == certificate.descent.source_parameter)
        .expect("guard binds the decremented parameter");
    binding.argument = other;
    assert_eq!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit).err(),
        Some(
            OptimizationUnitValidationError::RankedCycleRankingEvidenceMismatch {
                machine: certificate.component.machine,
            }
        )
    );
}
