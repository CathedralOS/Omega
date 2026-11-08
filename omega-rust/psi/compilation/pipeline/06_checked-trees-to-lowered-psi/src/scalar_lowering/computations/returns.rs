//! Normal-result formation is not an ordinary operand coercion. The source
//! checker has already proved the exact exit's predicates and provenance.
//! Complete its ordinary operand schedule first; only the fresh completion
//! parameter receives the declared result qualification. Other retained-value
//! entrypoints keep exact type equality and cannot borrow this admission.

use super::{
    Argument, CheckedScalarExpressionRole, Expansion, LoweringError, QualifiedScalarType, Site,
    storage, unsupported,
};
use crate::shared::checked_facts::value_facts;

#[derive(Clone)]
pub(crate) enum NormalCompletionTarget {
    Ordinary(usize),
    Selected(std::sync::Arc<NormalResultTarget>),
}

#[derive(Clone)]
pub(crate) enum NormalResultTarget {
    NoNormal {
        computation: checked_trees::CheckedScalarComputationHandle,
    },
    Value {
        computation: checked_trees::CheckedScalarComputationHandle,
        target: usize,
    },
    Dispatch {
        computation: checked_trees::CheckedScalarComputationHandle,
        arms: std::sync::Arc<
            [(
                checked_trees::CheckedScalarDispatchArm,
                std::sync::Arc<NormalResultTarget>,
            )],
        >,
    },
}

impl NormalResultTarget {
    pub(super) fn computation(&self) -> checked_trees::CheckedScalarComputationHandle {
        match self {
            Self::NoNormal { computation }
            | Self::Value { computation, .. }
            | Self::Dispatch { computation, .. } => *computation,
        }
    }

    fn map_targets(
        &self,
        map: &mut impl FnMut(usize) -> Result<usize, LoweringError>,
    ) -> Result<std::sync::Arc<Self>, LoweringError> {
        Ok(std::sync::Arc::new(match self {
            Self::NoNormal { computation } => Self::NoNormal {
                computation: *computation,
            },
            Self::Value {
                computation,
                target,
            } => Self::Value {
                computation: *computation,
                target: map(*target)?,
            },
            Self::Dispatch { computation, arms } => Self::Dispatch {
                computation: *computation,
                arms: arms
                    .iter()
                    .map(|(arm, child)| Ok((arm.clone(), child.map_targets(map)?)))
                    .collect::<Result<Vec<_>, LoweringError>>()?
                    .into(),
            },
        }))
    }
}

#[derive(Clone)]
pub(super) struct NormalResultScope {
    pub(super) result: std::sync::Arc<NormalResultTarget>,
    path: Option<std::sync::Arc<NormalResultPath>>,
}

struct NormalResultPath {
    parent: Option<std::sync::Arc<NormalResultPath>>,
    effects: Vec<super::LoweredScalarEffect>,
    destinations: std::sync::Arc<[language_semantics::PermissionClaimIdentity]>,
    route: Vec<crate::emission::claim_bindings::selections::SelectedDecision>,
}

#[derive(Clone)]
pub(super) struct NormalDispatchScope {
    pub(super) arms: std::sync::Arc<
        [(
            checked_trees::CheckedScalarDispatchArm,
            std::sync::Arc<NormalResultTarget>,
        )],
    >,
    path: Option<std::sync::Arc<NormalResultPath>>,
    effects: Vec<super::LoweredScalarEffect>,
}

impl NormalDispatchScope {
    pub(super) fn arm(
        &self,
        result: std::sync::Arc<NormalResultTarget>,
        destinations: std::sync::Arc<[language_semantics::PermissionClaimIdentity]>,
        route: Vec<crate::emission::claim_bindings::selections::SelectedDecision>,
    ) -> NormalResultScope {
        NormalResultScope {
            result,
            path: Some(std::sync::Arc::new(NormalResultPath {
                parent: self.path.clone(),
                effects: self.effects.clone(),
                destinations,
                route,
            })),
        }
    }

    pub(super) fn after_result(&mut self, effect: super::LoweredScalarEffect) {
        self.effects.push(effect);
    }

    pub(super) fn before_completion(&mut self, effect: super::LoweredScalarEffect) {
        self.effects.insert(0, effect);
    }
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn normal_return(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        exit: &checked_trees::FlowClaimJoinExit,
        copied_payloads: &[checked_trees::CheckedSelectedPayloadCopy],
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: &NormalCompletionTarget,
    ) -> Result<usize, LoweringError> {
        let statement = u32::try_from(exit.statement_index)
            .map_err(|_| LoweringError::Unsupported("scalar return statement exceeds u32"))?;
        let mut roots = value_facts(self.checked)
            .scalar_computations
            .roots_for_statement_role(exit.state_symbol, statement, role);
        let argument = if let Some(root) = roots.next() {
            if roots.next().is_some() {
                return unsupported("scalar computation root custody is duplicated or mismatched");
            }
            self.retained_root_argument(identities, root, bindings)?
        } else {
            Argument::Value(bindings.expression_at(
                identities,
                self.checked,
                exit.state_symbol,
                statement,
                role,
            )?)
        };
        let site = Site {
            state: exit.state_symbol,
            statement,
            bindings,
        };
        let source = self.argument_type(identities, &argument, &site, source_types)?;
        let completion = self.qualifications.return_completion(
            self.checked,
            self.machine,
            exit,
            source,
            result_type,
        )?;
        let destination = completion.destination();
        let copies = super::super::branch_states::structural_values::selected_copies(
            identities,
            self.checked,
            copied_payloads,
            self.structural_types,
        )?;
        let mut finish = |target: usize| -> Result<usize, LoweringError> {
            let target_state = target
                .checked_sub(self.base)
                .and_then(|index| self.states.get(index))
                .ok_or(LoweringError::Unsupported(
                    "scalar return completion target is absent",
                ))?;
            if target_state.parameter_types.len() != source_types.len() + 1
                || target_state.parameter_types[..source_types.len()] != *source_types
                || target_state.parameter_types.last() != Some(&destination)
            {
                return unsupported(
                    "checked scalar return completion disagrees with its destination",
                );
            }
            let target = if source == destination {
                target
            } else {
                let mut completed_types = source_types.to_vec();
                completed_types.push(source);
                self.push(super::LoweredScalarBranchState::continuing(
                    completed_types,
                    super::LoweredScalarBranchTerminator::CompleteReturn {
                        target,
                        exit: *exit,
                    },
                ))
            };
            let target = if copies.is_empty() {
                target
            } else {
                let mut completed_types = source_types.to_vec();
                completed_types.push(source);
                self.push(super::LoweredScalarBranchState::jumping(
                target, Vec::new(), completed_types.clone(), &completed_types,
                vec![crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect::CompleteSnapshots {
                    snapshots: copies.iter().map(|copy| {
                        crate::scalar_lowering::branch_states::prepared::CompletedSnapshot::Produced(copy.value)
                    }).collect(),
                }],
            ))
            };
            Ok(target)
        };
        let target = match target {
            NormalCompletionTarget::Ordinary(target) => {
                NormalCompletionTarget::Ordinary(finish(*target)?)
            }
            NormalCompletionTarget::Selected(result) => {
                NormalCompletionTarget::Selected(result.map_targets(&mut finish)?)
            }
        };
        let scope = match &target {
            NormalCompletionTarget::Ordinary(_) => None,
            NormalCompletionTarget::Selected(result) => Some(NormalResultScope {
                result: result.clone(),
                path: None,
            }),
        };
        let continuation = match target {
            NormalCompletionTarget::Ordinary(target) => target,
            NormalCompletionTarget::Selected(_) => 0,
        };
        let target = self.argument_with_normal_result(
            identities,
            &argument,
            source_types,
            continuation,
            &site,
            scope.as_ref(),
            &mut Vec::new(),
        )?;
        if copies.is_empty() {
            return Ok(target);
        }
        // This state is reached only after selection. Every snapshot completes
        // before the return's operands; only these private homes die afterward.
        Ok(self.push(super::LoweredScalarBranchState::jumping(
            target, Vec::new(), source_types.to_vec(), source_types,
            copies.into_iter().map(|copy| {
                crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect::ProduceStructuralValue {
                    value: copy.value, structural_type: copy.structural_type,
                }
            }).collect(),
        )))
    }
    pub(super) fn normal_result_scope(
        &mut self,
        scope: &NormalResultScope,
        types: &[QualifiedScalarType],
    ) -> (Option<NormalDispatchScope>, Option<usize>) {
        match scope.result.as_ref() {
            NormalResultTarget::NoNormal { .. } => (None, Some(0)),
            NormalResultTarget::Dispatch { arms, .. } => (
                Some(NormalDispatchScope {
                    arms: arms.clone(),
                    path: scope.path.clone(),
                    effects: Vec::new(),
                }),
                None,
            ),
            NormalResultTarget::Value { target, .. } => {
                let mut target = *target;
                let mut paths = Vec::new();
                let mut path = scope.path.as_deref();
                while let Some(current) = path {
                    paths.push(current);
                    path = current.parent.as_deref();
                }
                for path in paths.into_iter().rev() {
                    if !path.effects.is_empty() {
                        target = self.push(super::LoweredScalarBranchState::jumping(
                            target,
                            Vec::new(),
                            types.to_vec(),
                            types,
                            path.effects.clone(),
                        ));
                    }
                    if !path.destinations.is_empty() {
                        let state_position = self.states.len();
                        target = self.push(super::LoweredScalarBranchState::jumping(
                            target,
                            Vec::new(),
                            types.to_vec(),
                            types,
                            Vec::new(),
                        ));
                        self.claim_edges.push(super::PreparedClaimEdge {
                            state_position,
                            destinations: path.destinations.to_vec(),
                            route: path.route.clone(),
                        });
                    }
                }
                (None, Some(target))
            }
        }
    }
}
