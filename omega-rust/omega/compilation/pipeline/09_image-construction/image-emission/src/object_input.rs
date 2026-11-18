//! Closed input to object publication and its independent byte checks.
//! No producer, selected instruction graph or source lookup capability escapes
//! into the image through this carrier. Original programs belong to replay only.
use crate::FunctionFragmentObjectArtifactError as Error;
use machine_code::placed_body::NativeBodyReadback;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectEmissionInput {
    pub readback: Arc<NativeBodyReadback>,
    publication: object_file::ObjectPublication,
    pub statistics: crate::function_fragments::reporting::FunctionPublicationStatistics,
    pub termination: Result<(), String>,
}

impl ObjectEmissionInput {
    pub fn from_publication(publication: object_file::ObjectPublication) -> Result<Self, Error> {
        let body = publication.body();
        let readback = Arc::clone(&body.code);
        let statistics =
            crate::function_fragments::reporting::FunctionPublicationStatistics::from_current(
                &readback,
            )
            .map_err(Error::PhysicalEntry)?;
        let termination = body
            .facts
            .termination
            .clone()
            .map_err(|detail| Error::PhysicalSourceObservation(detail).to_string());
        Ok(Self {
            publication,
            readback,
            statistics,
            termination,
        })
    }
    pub fn publication(&self) -> object_file::ObjectPublication {
        self.publication.clone()
    }
    pub fn emission_facts(&self) -> &physical_program::EmissionFacts {
        &self.publication.body().facts
    }
    pub fn text_section(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        self.publication.text()
    }
    pub fn fragments(&self) -> &machine_code::FunctionFragmentEmissionPlan {
        &self.readback.frame_application.fragments
    }
    pub fn frame_layout(&self) -> &machine_code::TargetFrameLayoutPlan {
        &self.readback.frame
    }
    pub fn installation_authority(
        &self,
    ) -> &Arc<installation_admission::BoundInstallationAuthority> {
        &self.publication.body().authority
    }
}
