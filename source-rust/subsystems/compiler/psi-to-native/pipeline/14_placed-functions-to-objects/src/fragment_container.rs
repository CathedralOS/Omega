/*
We publish each body's relocation-free object (`publish_function_object`) and,
when a validator asks for it, retain the placed fixed-frame text section beside
it (`StagedOptimizedRelocationFreeObjectContainer`). Normal compilation trusts the
freshly constructed object. The validator
(`validate_optimized_relocation_free_object_container`) checks the placement and
frame, then rebuilds the object from the placed text and compares the two.
*/

use crate::{
    RelocationFreeObjectError, RelocationFreeObjectFromTextError, RelocationFreeObjectPlan,
    construct_relocation_free_object_from_text, validate_relocation_free_object_from_text,
};
use function_layout::{
    RelocationFreeTextSectionPlacementError, validate_optimized_fixed_frame_text_section,
};
use machine_code::code_layout::{NativeCodeLayout, NativeCodeLayoutError};
use std::sync::Arc;

type BodyPublication = machine_code::layout::EncodedBodyPublication<
    physical_program::BodyPublication<Arc<physical_program::BoundBodyContracts>>,
>;
type PlacedFunctionText = function_layout::StagedOptimizedFixedFrameTextSection<BodyPublication>;

impl crate::ObjectPlan {
    /// Each encoded import needs exactly one object symbol. A callback or
    /// private address must still name the function selected before encoding.
    pub fn check_bindings(
        &self,
        bindings: &machine_code::import_code::EncodedBodyBindings,
    ) -> Result<(), &'static str> {
        let layout = &self.layout;
        let imports = &bindings.imports.imports;
        let mismatch = "encoded import bodies differ from the object's import symbols";
        if imports.len() != layout.normalized_imports.len() {
            return Err(mismatch);
        }
        let mut encoded: Vec<_> = imports
            .iter()
            .map(|entry| (entry.identity.identity_digest(), &entry.identity))
            .collect();
        let mut symbols: Vec<_> = layout
            .normalized_imports
            .iter()
            .map(|symbol| (symbol.locator.identity_digest(), &symbol.locator))
            .collect();
        encoded.sort_unstable_by_key(|(digest, _)| *digest);
        symbols.sort_unstable_by_key(|(digest, _)| *digest);
        if encoded != symbols || encoded.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(mismatch);
        }
        let mut references: Vec<_> = layout
            .function_symbols
            .iter()
            .map(|(_, reference)| reference.identity)
            .collect();
        references.sort_unstable_by_key(|identity| identity.to_bytes());
        if references.as_slice() != bindings.private_references.as_ref() {
            return Err(
                "object publication changed the closed physical program's private references",
            );
        }
        Ok(())
    }
}

pub fn publish_function_object(
    placement: function_layout::FunctionPlacement<BodyPublication>,
) -> Result<PublishedFunctionObject, RelocationFreeObjectContainerError> {
    let (source, frame_application) = placement.into_parts();
    let bindings = &source.placed_body().bindings;
    if bindings.imports.target != source.text_section().target
        || bindings.windows_internet.is_some()
            && bindings.imports.target != target::NativeTarget::windows_x64()
    {
        return Err(RelocationFreeObjectContainerError::ArtifactMismatch);
    }
    let object =
        construct_relocation_free_object_from_text(source.text_section(), source.selections())
            .map_err(object_error)?;
    let target = source.contracts().body.original_for_audit();
    if target.is_some() != frame_application.is_some()
        || target.is_some() != source.contracts().original_encoding().is_some()
        || target.is_some_and(|original| {
            !original.belongs_to(&source.contracts().body.facts.facts.semantic)
        })
    {
        return Err(RelocationFreeObjectContainerError::PublicationMismatch);
    }
    let publication = ObjectPublication {
        contracts: Arc::clone(&source.contracts().body.facts),
        object: Arc::new(object),
        text: source.shared_text_section(),
        native_layouts: source.shared_native_code_layouts(),
        body: Arc::clone(source.placed_body()),
    };
    let sections = Arc::new(
        crate::FunctionObjectSections::assemble(&publication)
            .map_err(RelocationFreeObjectContainerError::Sections)?,
    );
    let replay = frame_application.map(|frame_application| {
        Arc::new(StagedOptimizedRelocationFreeObjectContainer {
            sections: Arc::clone(&sections),
            publication: publication.clone(),
            source,
            frame_application,
        })
    });
    Ok(PublishedFunctionObject {
        publication,
        sections,
        replay,
    })
}

/// Linkable object sections, physical source evidence, and any requested audit inputs.
/// Only object construction can issue the three together.
pub struct PublishedFunctionObject {
    publication: ObjectPublication,
    sections: Arc<crate::FunctionObjectSections>,
    replay: Option<Arc<StagedOptimizedRelocationFreeObjectContainer>>,
}
impl PublishedFunctionObject {
    pub fn sections(&self) -> &crate::FunctionObjectSections {
        &self.sections
    }
    pub fn into_parts(
        self,
    ) -> (
        ObjectPublication,
        crate::FunctionObjectSections,
        Option<Arc<StagedOptimizedRelocationFreeObjectContainer>>,
    ) {
        (
            self.publication,
            Arc::unwrap_or_clone(self.sections),
            self.replay,
        )
    }
}

pub fn validate_optimized_relocation_free_object_container(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), RelocationFreeObjectContainerError> {
    staged.verify_placement(&staged.source)?;
    validate_optimized_fixed_frame_text_section(&staged.source, &staged.frame_application)
        .map_err(RelocationFreeObjectContainerError::Source)?;
    validate_relocation_free_object_from_text(
        staged.source.text_section(),
        staged.source.selections(),
        staged.object(),
    )
    .map_err(object_error)
}

fn object_error(error: RelocationFreeObjectFromTextError) -> RelocationFreeObjectContainerError {
    match error {
        RelocationFreeObjectFromTextError::InvalidObject(error) => {
            RelocationFreeObjectContainerError::InvalidObject(error)
        }
        RelocationFreeObjectFromTextError::LengthOverflow => {
            RelocationFreeObjectContainerError::LengthOverflow
        }
        RelocationFreeObjectFromTextError::MissingSemanticEntry => {
            RelocationFreeObjectContainerError::MissingSemanticEntry
        }
        RelocationFreeObjectFromTextError::SourceMismatch => {
            RelocationFreeObjectContainerError::ArtifactMismatch
        }
    }
}

#[derive(Debug)]
#[must_use = "a staged object container owns its complete text-section custody"]
pub struct StagedOptimizedRelocationFreeObjectContainer {
    sections: Arc<crate::FunctionObjectSections>,
    publication: ObjectPublication,
    source: PlacedFunctionText,
    frame_application: function_layout::StagedFunctionFragmentFrameApplication,
}

impl StagedOptimizedRelocationFreeObjectContainer {
    pub fn published_object(&self) -> PublishedFunctionObject {
        PublishedFunctionObject {
            replay: None,
            publication: self.publication.clone(),
            sections: Arc::clone(&self.sections),
        }
    }

    pub fn original_target(&self) -> Option<&physical_program::RetainedTargetOperations> {
        self.source.contracts().body.original_for_audit()
    }

    pub fn original_encoding(&self) -> Option<&machine_code::layout::PhysicalEncodingReadback> {
        self.source.contracts().original_encoding()
    }

    /// Share the placed code, object relocations and body evidence that image
    /// construction consumes.
    pub fn publication(&self) -> ObjectPublication {
        self.publication.clone()
    }

    pub fn frame_application_for_audit(
        &self,
    ) -> &function_layout::StagedFunctionFragmentFrameApplication {
        &self.frame_application
    }

    /// Independently check the physical roots and frame of supplied placed code
    /// against the bindings issued for this object.
    pub fn verify_placement(
        &self,
        placed: &PlacedFunctionText,
    ) -> Result<(), RelocationFreeObjectContainerError> {
        let encoded = placed.encoding_subject();
        let bound = self
            .publication
            .contracts
            .encoding_subject(encoded.frame.clone())
            .map_err(|_| RelocationFreeObjectContainerError::PublicationMismatch)?;
        if bound != encoded
            || self.publication.contracts.needs_installed_branch_checks()
                != placed.placed_body().has_installed_branch_checks()
            || self.publication.contracts != placed.contracts().body.facts
            || self.publication.text != placed.shared_text_section()
        {
            return Err(RelocationFreeObjectContainerError::PublicationMismatch);
        }
        Ok(())
    }

    pub fn installation_authority(&self) -> &installation_admission::BoundInstallationAuthority {
        &self.publication.contracts.installations
    }

    pub const fn source(&self) -> &PlacedFunctionText {
        &self.source
    }

    pub fn object(&self) -> &RelocationFreeObjectPlan {
        self.publication.object()
    }

    pub fn emission_facts(&self) -> &physical_program::EmissionFacts {
        &self.publication.contracts.facts
    }

    pub fn same_text(&self, other: &Self) -> bool {
        self.publication.same_text(&other.publication)
    }
}

/// The complete current records needed to interpret an object's placed code.
/// Construction is restricted to the object producer; independent admission
/// compares these records with the explicitly retained originals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPublication {
    contracts: Arc<physical_program::BoundBodyContracts>,
    object: Arc<RelocationFreeObjectPlan>,
    text: Arc<machine_code::RelocationFreeTextSectionPlacement>,
    native_layouts: Result<Arc<[NativeCodeLayout]>, NativeCodeLayoutError>,
    body: Arc<function_layout::PlacedBodyEvidence>,
}

impl ObjectPublication {
    /// Clones of one publication share the object issued for that body.
    pub fn same_issuance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.object, &other.object)
    }

    pub fn emission_facts(&self) -> &physical_program::EmissionFacts {
        &self.contracts.facts
    }

    pub fn installation_authority(
        &self,
    ) -> &Arc<installation_admission::BoundInstallationAuthority> {
        &self.contracts.installations
    }

    pub fn bindings(&self) -> &machine_code::import_code::EncodedBodyBindings {
        &self.body.bindings
    }

    pub fn body(&self) -> &function_layout::PlacedBodyEvidence {
        &self.body
    }
    pub fn object(&self) -> &RelocationFreeObjectPlan {
        &self.object
    }
    pub fn native_code_layouts(&self) -> Result<Arc<[NativeCodeLayout]>, NativeCodeLayoutError> {
        self.native_layouts.clone()
    }
    pub fn text(&self) -> &machine_code::RelocationFreeTextSectionPlacement {
        &self.text
    }
    /// Two publications place the same code when their placed text sections
    /// are equal. Clones of one publication share the text and compare by address.
    pub fn same_text(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectContainerError {
    Sections(crate::body_object::Error),
    PublicationMismatch,
    Source(RelocationFreeTextSectionPlacementError),
    InvalidObject(RelocationFreeObjectError),
    LengthOverflow,
    MissingSemanticEntry,
    ArtifactMismatch,
}

impl std::fmt::Display for RelocationFreeObjectContainerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "relocation-free optimizer object custody failed: {self:?}"
        )
    }
}

impl std::error::Error for RelocationFreeObjectContainerError {}
