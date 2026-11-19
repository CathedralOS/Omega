/*
We stage and validate the optimizer's relocation-free object container
(`StagedOptimizedRelocationFreeObjectContainer`), binding the placed fixed-frame
text section, the constructed `RelocationFreeObjectPlan`, the encoded `OMGTRO`
wire container, the `OMGTOM` publication manifest, and the custody receipt into
one verified package. During normal compilation we trust the freshly constructed
text section and encoded container without re-decoding; independent validation
(`validate_optimized_relocation_free_object_container`) replays the full source,
object, wire-decode, manifest, and receipt correspondence for retained artifacts
and tests.
*/

use crate::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerStage,
    FunctionFragmentObjectContainerUnavailableData, RelocationFreeObjectContainer,
    RelocationFreeObjectDecodeError, RelocationFreeObjectError, RelocationFreeObjectFromTextError,
    RelocationFreeObjectPlan, construct_relocation_free_object_from_text,
    decode_relocation_free_object, encode_relocation_free_object,
    validate_relocation_free_object_from_text,
};
use function_layout::{
    RelocationFreeTextSectionPlacementError, validate_optimized_fixed_frame_text_section,
};
use machine_code::code_layout::{NativeCodeLayout, NativeCodeLayoutError};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
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
        use machine_code::import_code::ImportCodeIdentity;
        let layout = &self.layout;
        let imports = &bindings.imports.imports;
        let mismatch = "encoded import bodies differ from the object's import symbols";
        if imports.len() != layout.normalized_imports.len() {
            return Err(mismatch);
        }
        let mut encoded = Vec::with_capacity(imports.len());
        for entry in imports {
            let ImportCodeIdentity::Normalized(locator) = &entry.identity else {
                return Err(mismatch);
            };
            encoded.push((locator.identity_digest(), locator));
        }
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

pub fn stage_optimized_relocation_free_object_container(
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
    let object = construct_relocation_free_object_from_text(
        source.text_section(),
        source.manifest().record().selections,
    )
    .map_err(object_error)?;
    let container = encode_relocation_free_object(&object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    let manifest = construct_manifest(&source, &object, &container)?;
    let custody = receipt(&manifest, &object, &container);
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
        custody,
    };
    let replay = frame_application.map(|frame_application| {
        Arc::new(StagedOptimizedRelocationFreeObjectContainer {
            publication: publication.clone(),
            source,
            frame_application,
            container: Arc::new(container),
            manifest,
        })
    });
    Ok(PublishedFunctionObject {
        publication,
        replay,
    })
}

/// Stage 16 releases placement history before ordinary image emission.
/// An explicit audit keeps that history beside the exact object issued with it.
pub struct PublishedFunctionObject {
    publication: ObjectPublication,
    replay: Option<Arc<StagedOptimizedRelocationFreeObjectContainer>>,
}

impl PublishedFunctionObject {
    pub fn into_parts(
        self,
    ) -> (
        ObjectPublication,
        Option<Arc<StagedOptimizedRelocationFreeObjectContainer>>,
    ) {
        (self.publication, self.replay)
    }
}

pub fn validate_optimized_relocation_free_object_container(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedRelocationFreeObjectContainerCustodyReceipt, RelocationFreeObjectContainerError> {
    staged.verify_placement(&staged.source)?;
    validate_optimized_fixed_frame_text_section(&staged.source, &staged.frame_application)
        .map_err(RelocationFreeObjectContainerError::Source)?;
    validate_relocation_free_object_from_text(
        staged.source.text_section(),
        staged.source.manifest().record().selections,
        staged.object(),
    )
    .map_err(object_error)?;
    if staged.container.object != staged.object().identity
        || staged.container.identity
            != RelocationFreeObjectContainerIdentity::from_canonical_bytes(&staged.container.bytes)
    {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    let decoded = decode_relocation_free_object(&staged.container.bytes)
        .map_err(RelocationFreeObjectContainerError::InvalidContainer)?;
    if decoded != *staged.object() {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    validate_manifest(staged)?;
    let expected = receipt(&staged.manifest, staged.object(), &staged.container);
    if staged.custody() != expected {
        return Err(RelocationFreeObjectContainerError::ReceiptMismatch);
    }
    Ok(expected)
}

fn validate_manifest(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), RelocationFreeObjectContainerError> {
    let expected = construct_manifest(&staged.source, staged.object(), staged.container())?;
    if staged.manifest.record() == expected.record() {
        Ok(())
    } else {
        Err(RelocationFreeObjectContainerError::ManifestMismatch)
    }
}

fn construct_manifest(
    source: &PlacedFunctionText,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<ValidatedFunctionFragmentObjectContainerManifest, RelocationFreeObjectContainerError> {
    let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
    let statistics = crate::relocation_free_object_statistics(object, container)
        .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?;
    let mut record = FunctionFragmentObjectContainerManifest {
        identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
        source_text_section_manifest: source.manifest().record().identity,
        text_section: source.text_section().identity,
        psi: object.psi,
        work_schedule: object.work_schedule,
        selections: object.selections,
        selected: object.selected,
        target: object.target,
        semantic_entry: object.semantic_entry,
        semantic_entry_symbol: object.semantic_entry_symbol,
        symbol_policy: object.symbol_policy,
        object: object.identity,
        object_container: container.identity,
        relocation_requirements: object.relocation_requirements,
        statistics,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentObjectContainerManifest {
        record: Arc::new(record),
    })
}

fn receipt(
    manifest: &ValidatedFunctionFragmentObjectContainerManifest,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> StagedRelocationFreeObjectContainerCustodyReceipt {
    StagedRelocationFreeObjectContainerCustodyReceipt {
        source_text_section_manifest: manifest.record.source_text_section_manifest,
        text_section: object.source_text_section,
        object: object.identity,
        object_container: container.identity,
        manifest: manifest.record.identity,
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentObjectContainerManifest {
    record: Arc<FunctionFragmentObjectContainerManifest>,
}

impl ValidatedFunctionFragmentObjectContainerManifest {
    pub fn record(&self) -> &FunctionFragmentObjectContainerManifest {
        &self.record
    }
}

#[derive(Debug)]
#[must_use = "a staged object container owns its complete text-section custody"]
pub struct StagedOptimizedRelocationFreeObjectContainer {
    publication: ObjectPublication,
    source: PlacedFunctionText,
    frame_application: function_layout::StagedFunctionFragmentFrameApplication,
    container: Arc<RelocationFreeObjectContainer>,
    manifest: ValidatedFunctionFragmentObjectContainerManifest,
}

impl StagedOptimizedRelocationFreeObjectContainer {
    pub fn original_target(&self) -> Option<&physical_program::RetainedTargetOperations> {
        self.source.contracts().body.original_for_audit()
    }

    pub fn original_encoding(&self) -> Option<&machine_code::layout::PhysicalEncodingReadback> {
        self.source.contracts().original_encoding()
    }

    /// Share the placed code, object relocations, body evidence and custody
    /// receipt that image construction consumes.
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
            .encoding_subject(encoded.frame)
            .map_err(|_| RelocationFreeObjectContainerError::PublicationMismatch)?;
        if bound != encoded
            || self.publication.contracts.needs_installed_branch_checks()
                != placed.placed_body().has_installed_branch_checks()
            || self.publication.contracts != placed.contracts().body.facts
            || self.publication.custody.source_text_section_manifest
                != placed.manifest().record().identity
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

    pub fn container(&self) -> &RelocationFreeObjectContainer {
        &self.container
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentObjectContainerManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeObjectContainerCustodyReceipt {
        self.publication.custody
    }

    pub fn emission_facts(&self) -> &physical_program::EmissionFacts {
        &self.publication.contracts.facts
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
    custody: StagedRelocationFreeObjectContainerCustodyReceipt,
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
    pub fn custody(&self) -> StagedRelocationFreeObjectContainerCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedRelocationFreeObjectContainerCustodyReceipt {
    source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    text_section: TerminalRelocationFreeTextSectionIdentity,
    object: RelocationFreeObjectPlanIdentity,
    object_container: RelocationFreeObjectContainerIdentity,
    manifest: FunctionFragmentObjectContainerManifestIdentity,
}

impl StagedRelocationFreeObjectContainerCustodyReceipt {
    pub const fn source_text_section_manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.source_text_section_manifest
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn object(self) -> RelocationFreeObjectPlanIdentity {
        self.object
    }

    pub const fn object_container(self) -> RelocationFreeObjectContainerIdentity {
        self.object_container
    }

    pub const fn manifest(self) -> FunctionFragmentObjectContainerManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectContainerError {
    PublicationMismatch,
    Source(RelocationFreeTextSectionPlacementError),
    InvalidObject(RelocationFreeObjectError),
    InvalidContainer(RelocationFreeObjectDecodeError),
    LengthOverflow,
    MissingSemanticEntry,
    ArtifactMismatch,
    ContainerMismatch,
    ManifestMismatch,
    ReceiptMismatch,
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
