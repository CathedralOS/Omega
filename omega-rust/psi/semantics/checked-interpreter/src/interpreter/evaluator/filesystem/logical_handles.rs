//! Logical handle inputs: recording, validating and completing them,
//! reserving live handles and rejecting cross-domain or rooted results.

use crate::interpreter::evaluator::{
    BuildEvaluationLiveFilesystemHandleLease, BuildEvaluationSponsor, EvalResult, Evaluator,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemLogicalHandleOutput,
    FilesystemLogicalHandleOutputSource, Halt, PreparedFilesystemCall,
    PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
    filesystem::logical_handle_store, trap,
};

impl<'program> Evaluator<'program> {
    pub(crate) fn record_prepared_filesystem_logical_handle_input(
        &mut self,
        attempt_index: usize,
        operand_ordinal: u8,
        kind: FilesystemLogicalHandleKind,
        raw: i64,
        null_allowed: bool,
    ) {
        let resolution = self
            .filesystem_logical_handles
            .resolve(kind, raw, null_allowed);
        self.filesystem_operation_attempts[attempt_index]
            .logical_handle_inputs
            .push(FilesystemLogicalHandleInput {
                operand_ordinal,
                kind,
                resolution,
            });
    }

    pub(crate) fn validate_incremental_logical_handle_inputs(
        &mut self,
        attempt_index: usize,
        plan: &PreparedFilesystemLogicalHandlePlan,
    ) -> EvalResult<()> {
        let canonical_inputs = plan
            .inputs
            .iter()
            .map(|input| FilesystemLogicalHandleInput {
                operand_ordinal: input.operand_ordinal,
                kind: input.kind,
                resolution: self.filesystem_logical_handles.resolve(
                    input.kind,
                    input.raw,
                    input.null_allowed,
                ),
            })
            .collect::<Vec<_>>();
        if self.filesystem_operation_attempts[attempt_index].logical_handle_inputs
            != canonical_inputs
        {
            return trap(
                "incremental filesystem logical-handle evidence disagrees with the fully prepared call",
            );
        }
        Ok(())
    }

    pub(crate) fn reject_cross_domain_logical_handle_inputs(
        &self,
        plan: &PreparedFilesystemLogicalHandlePlan,
    ) -> EvalResult<()> {
        if plan.inputs.iter().any(|input| {
            self.filesystem_logical_handles.conflicts_with_live_domain(
                input.kind,
                input.raw,
                input.null_allowed,
            )
        }) {
            return trap(
                "filesystem input aliases a live token from another logical-handle domain",
            );
        }
        Ok(())
    }

    pub(crate) fn reject_rooted_final_path_result(
        &self,
        attempt_index: usize,
        call: &PreparedFilesystemCall,
    ) -> EvalResult<()> {
        if !self.rooted_build_paths_required
            || !matches!(call, PreparedFilesystemCall::FinalPathNameByHandle { .. })
        {
            return Ok(());
        }
        if matches!(
            self.filesystem_operation_attempts[attempt_index]
                .logical_handle_inputs
                .as_slice(),
            [FilesystemLogicalHandleInput {
                kind: FilesystemLogicalHandleKind::Native,
                resolution: FilesystemLogicalHandleInputResolution::Unknown,
                ..
            }]
        ) {
            return Ok(());
        }
        trap(
            "package build filesystem operation `final_path_name_by_handle` would expose a host-absolute path",
        )
    }

    pub(crate) fn complete_logical_handle_observations(
        &mut self,
        attempt_index: usize,
        plan: &PreparedFilesystemLogicalHandlePlan,
        result: i64,
        live_handle_lease: &mut Option<BuildEvaluationLiveFilesystemHandleLease>,
    ) -> EvalResult<()> {
        if plan
            .input_success
            .is_some_and(|success| success.accepts(result))
            && self.filesystem_operation_attempts[attempt_index]
                .logical_handle_inputs
                .iter()
                .any(|input| {
                    matches!(
                        input.resolution,
                        FilesystemLogicalHandleInputResolution::Unknown
                    )
                })
        {
            return trap(
                "filesystem provider accepted an input outside its live logical-handle domain",
            );
        }
        if let Some(output) = plan.output {
            let (success, kind, source) = match output {
                PreparedFilesystemLogicalHandleOutput::Created { kind, success } => {
                    (success, kind, FilesystemLogicalHandleOutputSource::Created)
                }
                PreparedFilesystemLogicalHandleOutput::Duplicated {
                    source_operand_ordinal,
                    success,
                } => {
                    let source = self.resolved_logical_handle_input(
                        attempt_index,
                        source_operand_ordinal,
                        FilesystemLogicalHandleKind::Descriptor,
                    );
                    if success.accepts(result) && source.is_none() {
                        return trap(
                            "filesystem provider duplicated an unresolved descriptor token",
                        );
                    }
                    (
                        success,
                        FilesystemLogicalHandleKind::Descriptor,
                        source
                            .map(FilesystemLogicalHandleOutputSource::Duplicated)
                            .unwrap_or(FilesystemLogicalHandleOutputSource::Created),
                    )
                }
                PreparedFilesystemLogicalHandleOutput::Borrowed {
                    source_operand_ordinal,
                    success,
                } => {
                    let source = self.resolved_logical_handle_input(
                        attempt_index,
                        source_operand_ordinal,
                        FilesystemLogicalHandleKind::Descriptor,
                    );
                    if success.accepts(result) && source.is_none() {
                        return trap(
                            "filesystem provider returned a native view for an unresolved descriptor token",
                        );
                    }
                    (
                        success,
                        FilesystemLogicalHandleKind::Native,
                        source
                            .map(FilesystemLogicalHandleOutputSource::Borrowed)
                            .unwrap_or(FilesystemLogicalHandleOutputSource::Created),
                    )
                }
            };
            if success.accepts(result) {
                let identity = match source {
                    FilesystemLogicalHandleOutputSource::Borrowed(source) => self
                        .filesystem_logical_handles
                        .borrow_native(result, source),
                    FilesystemLogicalHandleOutputSource::Created
                    | FilesystemLogicalHandleOutputSource::Duplicated(_) => {
                        self.filesystem_logical_handles.create(kind, result)
                    }
                }
                .map_err(filesystem_logical_handle_halt)?;
                self.filesystem_operation_attempts[attempt_index].logical_handle_output =
                    Some(FilesystemLogicalHandleOutput {
                        kind,
                        identity,
                        source,
                    });
                if let Some(lease) = live_handle_lease.take() {
                    let prior = self.filesystem_live_handle_leases.insert(identity, lease);
                    debug_assert!(prior.is_none());
                }
            }
        }

        if let Some(retirement) = plan.retirement
            && retirement.success.accepts(result)
        {
            let input = self.filesystem_operation_attempts[attempt_index]
                .logical_handle_inputs
                .iter()
                .find(|input| input.operand_ordinal == retirement.operand_ordinal)
                .copied()
                .expect("retirement plan must name one retained handle input");
            let FilesystemLogicalHandleInputResolution::Resolved(identity) = input.resolution
            else {
                return trap("filesystem provider closed an unresolved logical handle token");
            };
            let retired = self.filesystem_logical_handles.retire(input.kind, identity);
            for retired_identity in &retired {
                self.filesystem_live_handle_leases.remove(retired_identity);
            }
            self.filesystem_operation_attempts[attempt_index].retired_logical_handles = retired;
        }
        Ok(())
    }

    pub(crate) fn reserve_prepared_live_filesystem_handle(
        &self,
        plan: &PreparedFilesystemLogicalHandlePlan,
    ) -> EvalResult<Option<BuildEvaluationLiveFilesystemHandleLease>> {
        let owns_new_resource = matches!(
            plan.output,
            Some(PreparedFilesystemLogicalHandleOutput::Created { .. })
                | Some(PreparedFilesystemLogicalHandleOutput::Duplicated { .. })
        );
        if !owns_new_resource {
            return Ok(None);
        }
        self.build_evaluation_sponsor
            .as_ref()
            .map(BuildEvaluationSponsor::reserve_live_filesystem_handle)
            .transpose()
            .map_err(Halt::Resource)
    }

    fn resolved_logical_handle_input(
        &self,
        attempt_index: usize,
        operand_ordinal: u8,
        kind: FilesystemLogicalHandleKind,
    ) -> Option<crate::FilesystemLogicalHandleIdentity> {
        self.filesystem_operation_attempts[attempt_index]
            .logical_handle_inputs
            .iter()
            .find(|input| input.operand_ordinal == operand_ordinal && input.kind == kind)
            .and_then(|input| match input.resolution {
                FilesystemLogicalHandleInputResolution::Resolved(identity) => Some(identity),
                FilesystemLogicalHandleInputResolution::Null
                | FilesystemLogicalHandleInputResolution::Unknown => None,
            })
    }
}

fn filesystem_logical_handle_halt(
    error: logical_handle_store::FilesystemLogicalHandleError,
) -> Halt {
    use crate::interpreter::evaluator::filesystem::logical_handle_store::FilesystemLogicalHandleError as Error;
    match error {
        Error::IdentityExhausted => {
            Halt::Resource("filesystem logical-handle identity space exhausted".to_owned())
        }
        Error::LiveProviderTokenCollision { kind, raw } => Halt::Trap(format!(
            "filesystem provider reused live {kind:?} token `{raw}`"
        )),
        Error::BorrowSourceMismatch { raw } => Halt::Trap(format!(
            "filesystem provider reused native token `{raw}` for a different descriptor source"
        )),
    }
}
