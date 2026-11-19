impl PackageBuildSession {
    pub fn source_build_request(
        &mut self,
        capture: &package_compilation::CapturedLocalSource,
        entry: &Path,
        profile: target::TargetProfile,
        execution: package_compilation::BuildExecutionInputs,
    ) -> Result<compiler::CheckedCompileRequest, CompileResolvedPackageReviewsError> {
        self.require_open()?;
        let profile = execution.profile.unwrap_or(profile);
        let directory = match execution.directory {
            Some(directory) => directory,
            None => self.create_build_directory(self.root.join("source"))?,
        };
        Ok(compiler::CheckedCompileRequest {
            source_capture: Some(capture.clone()),
            build: package_compilation::BuildExecutionInputs {
                profile: Some(profile),
                snapshot: execution
                    .snapshot
                    .or_else(|| Some(package_compilation::BuildSnapshotRequest::new(empty()))),
                directory: Some(directory),
                filesystem_sponsor: execution
                    .filesystem_sponsor
                    .or_else(|| Some(self.filesystem_sponsor.clone())),
                evaluation_sponsor: execution
                    .evaluation_sponsor
                    .or_else(|| Some(self.evaluation_sponsor.clone())),
                restricted_grants: execution.restricted_grants,
                dependency_activation: execution.dependency_activation,
            },
            ..compiler::CheckedCompileRequest::new(entry, Some(profile.target_name()))
        })
    }
}
