//! Selection preserves the exact legalized crash leaf independently of its ISA bytes.
use super::{SelectedSelectionConstraints, build, fixture};
use legalized_operations::LegalizedScalarTerminator;
use selected_instructions::{SelectedInstructionKind, SelectedTerminator};
use semantic_vocabulary::{ClaimId, EdgeId, Proposition};
use terminal_psi::{CrashCause, CrashPredicateTerm};

#[test]
fn explicit_crash_preserves_cause_site_frontier_and_fuel_without_successors() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for cause in [CrashCause::Trap, CrashCause::Abort] {
            for retain_prefix in [false, true] {
                let mut source = fixture(target, 0);
                source.attachment = None;
                if !retain_prefix {
                    source.blocks[0].instructions.clear();
                    source.provenance.operations.clear();
                }
                let LegalizedScalarTerminator::Return(returned) =
                    source.blocks[0].terminator.clone()
                else {
                    panic!("fixture return");
                };
                source.blocks[0].terminator = LegalizedScalarTerminator::Crash {
                    psi_edge: returned.edge,
                    cause,
                    site_guard: vec![CrashPredicateTerm::new(Proposition::Truth)],
                    frontier_lower_bound: vec![ClaimId::new(7).unwrap(), ClaimId::new(9).unwrap()],
                    fuel: vec![optimization_unit::FuelSettlement {
                        site: optimization_unit::PsiProvenance::Edge(returned.edge),
                        units: 3,
                    }],
                    effect: returned.effect,
                    ownership: Vec::new(),
                };
                let environment =
                    register_environment::baseline_target_register_environment(target).unwrap();
                let constraints = SelectedSelectionConstraints {
                    keys: environment.selected_keys(),
                    fixed_inputs: Vec::new(),
                };
                let selected = build(
                    0,
                    &source,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                let validate =
                    |source: &legalized_operations::LegalizedScalarFunction,
                     candidate: &selected_instructions::SelectedFunction| {
                        crate::selection::validation::scalar_graph::validate(
                            0,
                            source,
                            candidate,
                            target,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                        )
                    };
                validate(&source, &selected).unwrap();
                let SelectedTerminator::Crash {
                    instruction,
                    psi_edge,
                    cause: selected_cause,
                    site_guard,
                    frontier_lower_bound,
                } = &selected.blocks[0].terminator
                else {
                    panic!("crash terminal");
                };
                assert_eq!(instruction.kind, SelectedInstructionKind::Crash);
                assert_eq!(instruction.constraint, constraints.keys.crash);
                assert_eq!(*psi_edge, returned.edge);
                assert_eq!(*selected_cause, cause);
                assert_eq!(site_guard, &[CrashPredicateTerm::new(Proposition::Truth)]);
                assert_eq!(
                    frontier_lower_bound,
                    &[ClaimId::new(7).unwrap(), ClaimId::new(9).unwrap()]
                );
                assert!(instruction.operands.is_empty());
                let counter_name = if target.architecture == target::Architecture::X86_64 {
                    "rip"
                } else {
                    "pc"
                };
                let counter = environment
                    .physical()
                    .model()
                    .view_named(counter_name)
                    .unwrap();
                assert_eq!(instruction.implicit_uses, counter.units);
                assert_eq!(instruction.implicit_defs, counter.units);
                assert!(instruction.clobbers.is_empty());
                assert_eq!(instruction.provenance.edges, [returned.edge]);
                assert_eq!(instruction.provenance.fuel[0].units, 3);

                for mutation in 0..15 {
                    let mut candidate = selected.clone();
                    let SelectedTerminator::Crash {
                        instruction,
                        psi_edge,
                        cause,
                        site_guard,
                        frontier_lower_bound,
                    } = &mut candidate.blocks[0].terminator
                    else {
                        unreachable!()
                    };
                    match mutation {
                        0 => *psi_edge = EdgeId::new(99).unwrap(),
                        1 => {
                            *cause = if *cause == CrashCause::Trap {
                                CrashCause::Abort
                            } else {
                                CrashCause::Trap
                            }
                        }
                        2 => site_guard.clear(),
                        3 => frontier_lower_bound.reverse(),
                        4 => instruction.provenance.fuel.clear(),
                        5 => instruction.provenance.edges.clear(),
                        6 => instruction.kind = SelectedInstructionKind::HostedExitProcessI32,
                        7 => instruction.constraint = constraints.keys.return_unit,
                        8 => instruction.clobbers.push(register_model::RegisterUnitId(0)),
                        9 => {
                            candidate.blocks[0].terminator = SelectedTerminator::Return {
                                instruction: instruction.clone(),
                                psi_return_edge: *psi_edge,
                            }
                        }
                        10 => instruction.implicit_uses.clear(),
                        11 => instruction.implicit_defs.clear(),
                        12 => instruction.implicit_uses = vec![register_model::RegisterUnitId(0)],
                        13 => instruction.implicit_defs = vec![register_model::RegisterUnitId(0)],
                        14 => {
                            let mut deep_guard = Proposition::Truth;
                            for _ in 0..300 {
                                deep_guard = Proposition::Conjunction(vec![deep_guard]);
                            }
                            // Admission compares against the canonical source before
                            // identity encoding; malformed raw guards must reject here.
                            *site_guard = vec![CrashPredicateTerm::new(deep_guard)];
                        }
                        _ => unreachable!(),
                    }
                    assert!(
                        validate(&source, &candidate).is_err(),
                        "{target:?} mutation {mutation}"
                    );
                }
                // Corrupt the independently supplied legalized input while retaining the selected result.
                for mutation in 0..4 {
                    let mut changed_source = source.clone();
                    let LegalizedScalarTerminator::Crash {
                        psi_edge,
                        cause,
                        site_guard,
                        frontier_lower_bound,
                        ..
                    } = &mut changed_source.blocks[0].terminator
                    else {
                        unreachable!()
                    };
                    match mutation {
                        0 => *psi_edge = EdgeId::new(99).unwrap(),
                        1 => {
                            *cause = if *cause == CrashCause::Trap {
                                CrashCause::Abort
                            } else {
                                CrashCause::Trap
                            }
                        }
                        2 => site_guard.clear(),
                        3 => frontier_lower_bound.clear(),
                        _ => unreachable!(),
                    }
                    assert!(validate(&changed_source, &selected).is_err());
                }
            }
        }
    }
}
