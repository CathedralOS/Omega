//! Closed input to object publication and its independent byte checks.
//! No producer, selected instruction graph or source lookup capability escapes
//! into the image through this carrier. Original programs belong to replay only.
use crate::FunctionFragmentObjectArtifactError as Error;
use crate::body_readback::NativeBodyReadback;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectEmissionInput {
    pub allocation: register_homes::PostAllocationOptimizationManifest,
    pub readback: Arc<NativeBodyReadback>,
    publication: object_file::ObjectPublication,
    facts: Arc<physical_program::EmissionFacts>,
    authority: Arc<installation_binding::BoundInstallationAuthority>,
    pub frames: Vec<machine_code::FunctionAppliedFrameProtocol>,
    pub statistics: crate::function_fragments::reporting::FunctionPublicationStatistics,
    pub termination: Result<(), String>,
}

impl ObjectEmissionInput {
    pub fn capture(
        source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
    ) -> Result<Self, Error> {
        let current = source.source().source().source().source();
        Ok(Self {
            allocation: current.post_allocation_manifest().record().clone(),
            readback: Arc::new(NativeBodyReadback::capture(source)),
            publication: source.publication(),
            facts: Arc::new(source.emission_facts().clone()),
            authority: Arc::clone(current.installation_authority()),
            frames: source.source().source().application().functions.clone(),
            statistics:
                crate::function_fragments::reporting::FunctionPublicationStatistics::from_source(
                    source,
                )
                .map_err(Error::PhysicalEntry)?,
            termination: crate::function_fragments::capture_privileged_termination(source)
                .map_err(|error| error.to_string()),
        })
    }
    pub fn publication(&self) -> object_file::ObjectPublication {
        self.publication.clone()
    }
    pub fn emission_facts(&self) -> &physical_program::EmissionFacts {
        &self.facts
    }
    pub fn text_section(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        self.publication.text()
    }
    pub fn fragments(&self) -> &machine_code::FunctionFragmentEmissionPlan {
        &self.readback.fragments
    }
    pub fn frame_layout(&self) -> &machine_code::TargetFrameLayoutPlan {
        &self.readback.frame
    }
    pub fn installation_authority(&self) -> &Arc<installation_binding::BoundInstallationAuthority> {
        &self.authority
    }
}
