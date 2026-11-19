//! Closed input to object publication and its independent byte checks.
//! No producer, selected instruction graph or source lookup capability escapes
//! into the image through this carrier. Original programs belong to replay only.
use crate::FunctionFragmentObjectArtifactError as Error;
use machine_code::placed_body::NativeBodyReadback;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectEmissionInput {
    publication: object_file::ObjectPublication,
    pub statistics: crate::function_fragments::reporting::FunctionPublicationStatistics,
    pub termination: Result<(), String>,
}

impl ObjectEmissionInput {
    pub fn from_publication(publication: object_file::ObjectPublication) -> Result<Self, Error> {
        let body = publication.body();
        let statistics =
            crate::function_fragments::reporting::FunctionPublicationStatistics::from_current(
                body.code(),
            )
            .map_err(Error::PhysicalEntry)?;
        let termination = publication
            .emission_facts()
            .physical
            .termination
            .clone()
            .map_err(|detail| Error::PhysicalSourceObservation(detail).to_string());
        Ok(Self {
            publication,
            statistics,
            termination,
        })
    }
    pub fn readback(&self) -> &NativeBodyReadback {
        self.publication.body().code()
    }
    pub fn publication(&self) -> &object_file::ObjectPublication {
        &self.publication
    }
    pub fn emission_facts(&self) -> &physical_program::EmissionFacts {
        self.publication.emission_facts()
    }
    pub fn text_section(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        self.publication.text()
    }
    pub fn fragments(&self) -> &machine_code::FunctionFragmentEmissionPlan {
        &self.readback().frame_application.fragments
    }
    pub fn frame_layout(&self) -> &machine_code::TargetFrameLayoutPlan {
        &self.readback().frame
    }
    pub fn installation_authority(
        &self,
    ) -> &Arc<installation_admission::BoundInstallationAuthority> {
        self.publication.installation_authority()
    }
}
