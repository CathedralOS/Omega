//! Completed-arm output, after every invocation boundary has been restored.
//!
//! Collectors retain the ordinary ordered leaf list. Universal correspondence
//! consumers can stop when their remaining requirement set becomes empty;
//! that stop is neither recognition failure nor normal-path exhaustion.

use super::{
    StructuralArrivalLog, StructuralCaseArm, StructuralJudge, StructuralPrefixVisitor,
    StructuralTerm, recognize_structural_state_leaves_with_output,
};
use crate::proof_contracts::contract_entailment::citations::CitationFacts;
use diagnostics::Diagnostic;
use std::ops::ControlFlow;
use typed_trees::machine::Machine;

pub(super) enum CompletedArmTraversal<'program> {
    Exhausted(Vec<StructuralCaseArm<'program>>),
    ConsumerStopped,
}

pub(super) enum CompletedArmOutput<'query, 'program> {
    Collect,
    Fold(&'query mut dyn FnMut(StructuralCaseArm<'program>) -> Option<ControlFlow<()>>),
}

impl<'program> CompletedArmOutput<'_, 'program> {
    pub(super) fn deliver(
        &mut self,
        arm: StructuralCaseArm<'program>,
        collected: &mut Vec<StructuralCaseArm<'program>>,
    ) -> Option<ControlFlow<()>> {
        match self {
            Self::Collect => {
                collected.push(arm);
                Some(ControlFlow::Continue(()))
            }
            Self::Fold(fold) => fold(arm),
        }
    }
}

/// Fold completed arms in authored traversal order. Consumer stop is distinct
/// from incomplete recognition and from exhausting a path with no normal arm.
pub(in crate::proof_contracts::contract_entailment) fn fold_structural_case_arms<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    fold: &mut dyn FnMut(StructuralCaseArm<'a>) -> Option<ControlFlow<()>>,
) -> Option<ControlFlow<()>> {
    match recognize_structural_case_arms_with_output(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        &mut [],
        &mut CompletedArmOutput::Fold(fold),
    )? {
        CompletedArmTraversal::Exhausted(_) => Some(ControlFlow::Continue(())),
        CompletedArmTraversal::ConsumerStopped => Some(ControlFlow::Break(())),
    }
}

pub(super) fn recognize_structural_case_arms_with_output<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    visitors: &mut [&mut (dyn StructuralPrefixVisitor + '_)],
    output: &mut CompletedArmOutput<'_, 'a>,
) -> Option<CompletedArmTraversal<'a>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let parameter_names: Vec<String> = program
        .state_parameters(root)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();
    let environment: Vec<(String, StructuralTerm)> = parameter_names
        .iter()
        .map(|name| (name.clone(), StructuralTerm::Variable(name.clone())))
        .collect();
    let mut path = Vec::new();
    let mut arrivals = StructuralArrivalLog::new();
    let mut fresh = 0usize;
    let arms = recognize_structural_state_leaves_with_output(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        states,
        root,
        &parameter_names,
        environment,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
        &mut path,
        &mut arrivals,
        usize::MAX,
        &mut fresh,
        visitors,
        CitationFacts::default(),
        output,
    )?;
    Some(arms)
}

impl<'program> CompletedArmOutput<'_, 'program> {
    /// A collecting caller restores all collected leaves at the original
    /// boundary. A folding caller restores each leaf before delivery, so its
    /// observation is in the same caller namespace and storage epoch.
    pub(super) fn forwarded(
        &mut self,
        mut complete: impl FnMut(StructuralCaseArm<'program>) -> Option<StructuralCaseArm<'program>>,
        visit: impl FnOnce(
            &mut CompletedArmOutput<'_, 'program>,
        ) -> Option<CompletedArmTraversal<'program>>,
    ) -> Option<CompletedArmTraversal<'program>> {
        match self {
            Self::Collect => match visit(&mut Self::Collect)? {
                CompletedArmTraversal::Exhausted(arms) => Some(CompletedArmTraversal::Exhausted(
                    arms.into_iter().map(complete).collect::<Option<Vec<_>>>()?,
                )),
                CompletedArmTraversal::ConsumerStopped => {
                    Some(CompletedArmTraversal::ConsumerStopped)
                }
            },
            Self::Fold(fold) => {
                let mut forward = |arm| fold(complete(arm)?);
                visit(&mut CompletedArmOutput::Fold(&mut forward))
            }
        }
    }
}
