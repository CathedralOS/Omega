//! Retained native placement of scalar leaves, sum tags and opaque regions.
//! Target lowering produces these coordinates with its ordinary storage
//! geometry. A host adapter may encode captured semantic fields with them;
//! neither the layout nor an offset establishes a loan or permits replacement.
//! Gaps are deliberately visible: a semantic field snapshot cannot invent the
//! contents of native padding, pointers or other unmodeled storage. Case paths
//! select payload namespaces; native ordinal mappings never become Psi case IDs.

use arena::{Arena, HandleSpan};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralCaseId, StructuralTypeId,
};

#[derive(Debug, Clone)]
pub struct NativeBorrowedStorageField {
    path: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
    pub byte_offset: u32,
    pub logical_scalar: ScalarType,
    pub stored_scalar: ScalarType,
}

impl Default for NativeBorrowedStorageField {
    fn default() -> Self {
        Self {
            path: HandleSpan::empty(),
            repetitions: HandleSpan::empty(),
            byte_offset: 0,
            logical_scalar: ScalarType::Boolean,
            stored_scalar: ScalarType::Boolean,
        }
    }
}

#[derive(Debug, Clone)]
struct NativeStoragePathStep(CanonicalStructuralPathSegment);

impl Default for NativeStoragePathStep {
    fn default() -> Self {
        Self(CanonicalStructuralPathSegment::FixedIndex(0))
    }
}

/// A fixed-array dimension stays a recipe, not one compiler row per element.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeStorageRepetition {
    pub path_position: usize,
    pub length: u64,
    pub byte_stride: u32,
}

/// Native storage whose contents are not represented by scalar or tag recipes.
/// A matching semantic leaf keeps its custody; these bytes cannot be decoded
/// as a replacement merely because another case overlays scalar storage here.
#[derive(Debug, Clone, Default)]
pub struct NativeBorrowedStorageOpaqueRegion {
    path: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
    pub byte_offset: u32,
    pub byte_size: u32,
}

/// Nominal case identity and its signed 32-bit conventional native tag.
#[derive(Debug, Clone, Copy)]
pub struct NativeStorageCaseTag {
    pub case: StructuralCaseId,
    pub native_tag: i32,
}

/// Placement of one sum's discriminator below the borrowed storage root.
/// The path selects the sum itself, including any enclosing payload cases.
#[derive(Debug, Clone)]
pub struct NativeBorrowedStorageDiscriminator {
    path: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
    cases: HandleSpan<NativeStorageCaseTagSlot>,
    pub structural_type: StructuralTypeId,
    pub byte_offset: u32,
}

// Arena vacancy is distinct from any semantic identity. The slots supply the
// arena's default without manufacturing a case or type for an empty row.
#[derive(Debug, Clone, Default)]
struct NativeStorageDiscriminatorSlot(Option<NativeBorrowedStorageDiscriminator>);

#[derive(Debug, Clone, Default)]
struct NativeStorageCaseTagSlot(Option<NativeStorageCaseTag>);

#[derive(Debug, Clone)]
pub struct NativeBorrowedStorageLayout {
    pub structural_type: StructuralTypeId,
    pub byte_size: u32,
    pub alignment: u16,
    storage: Arena<NativeBorrowedStorageField>,
    fields: HandleSpan<NativeBorrowedStorageField>,
    paths: Arena<NativeStoragePathStep>,
    repetitions: Arena<NativeStorageRepetition>,
    discriminator_storage: Arena<NativeStorageDiscriminatorSlot>,
    discriminators: HandleSpan<NativeStorageDiscriminatorSlot>,
    cases: Arena<NativeStorageCaseTagSlot>,
    opaque_storage: Arena<NativeBorrowedStorageOpaqueRegion>,
    opaque_regions: HandleSpan<NativeBorrowedStorageOpaqueRegion>,
}

impl NativeBorrowedStorageLayout {
    /// Consume the lowering owner's construction scratch, not source syntax.
    pub fn new(
        structural_type: StructuralTypeId,
        byte_size: u32,
        alignment: u16,
        fields: impl IntoIterator<
            Item = (
                Vec<CanonicalStructuralPathSegment>,
                Vec<NativeStorageRepetition>,
                u32,
                ScalarType,
                ScalarType,
            ),
        >,
        discriminators: impl IntoIterator<
            Item = (
                Vec<CanonicalStructuralPathSegment>,
                Vec<NativeStorageRepetition>,
                StructuralTypeId,
                u32,
                Vec<NativeStorageCaseTag>,
            ),
        >,
        opaque_regions: impl IntoIterator<
            Item = (
                Vec<CanonicalStructuralPathSegment>,
                Vec<NativeStorageRepetition>,
                u32,
                u32,
            ),
        >,
    ) -> Self {
        let mut paths = Arena::new();
        let mut repetitions = Arena::new();
        let fields = fields.into_iter().map(
            |(path, dimensions, byte_offset, logical_scalar, stored_scalar)| {
                NativeBorrowedStorageField {
                    path: paths.insert_many(path.into_iter().map(NativeStoragePathStep)),
                    repetitions: repetitions.insert_many(dimensions),
                    byte_offset,
                    logical_scalar,
                    stored_scalar,
                }
            },
        );
        let mut storage = Arena::new();
        let fields = storage.insert_many(fields);
        let mut cases = Arena::new();
        let discriminators = discriminators.into_iter().map(
            |(path, dimensions, structural_type, byte_offset, mappings)| {
                NativeStorageDiscriminatorSlot(Some(NativeBorrowedStorageDiscriminator {
                    path: paths.insert_many(path.into_iter().map(NativeStoragePathStep)),
                    repetitions: repetitions.insert_many(dimensions),
                    cases: cases.insert_many(
                        mappings
                            .into_iter()
                            .map(|mapping| NativeStorageCaseTagSlot(Some(mapping))),
                    ),
                    structural_type,
                    byte_offset,
                }))
            },
        );
        let mut discriminator_storage = Arena::new();
        let discriminators = discriminator_storage.insert_many(discriminators);
        let opaque_regions =
            opaque_regions
                .into_iter()
                .map(|(path, dimensions, byte_offset, byte_size)| {
                    NativeBorrowedStorageOpaqueRegion {
                        path: paths.insert_many(path.into_iter().map(NativeStoragePathStep)),
                        repetitions: repetitions.insert_many(dimensions),
                        byte_offset,
                        byte_size,
                    }
                });
        let mut opaque_storage = Arena::new();
        let opaque_regions = opaque_storage.insert_many(opaque_regions);
        Self {
            structural_type,
            byte_size,
            alignment,
            storage,
            fields,
            paths,
            repetitions,
            discriminator_storage,
            discriminators,
            cases,
            opaque_storage,
            opaque_regions,
        }
    }

    pub fn fields(&self) -> &[NativeBorrowedStorageField] {
        self.storage.span_or_empty(self.fields)
    }

    pub fn field_path(
        &self,
        field: &NativeBorrowedStorageField,
    ) -> impl ExactSizeIterator<Item = CanonicalStructuralPathSegment> + '_ {
        self.paths
            .span_or_empty(field.path)
            .iter()
            .map(|step| step.0)
    }

    pub fn field_repetitions(
        &self,
        field: &NativeBorrowedStorageField,
    ) -> &[NativeStorageRepetition] {
        self.repetitions.span_or_empty(field.repetitions)
    }

    /// Recipes for every sum occurrence, including sums inside inactive cases.
    /// The semantic snapshot selects which case-qualified recipes are active.
    pub fn discriminators(&self) -> impl Iterator<Item = &NativeBorrowedStorageDiscriminator> {
        self.discriminator_storage
            .span_or_empty(self.discriminators)
            .iter()
            .filter_map(|slot| slot.0.as_ref())
    }

    /// Nominal declaration path to the sum, before any array repetition is expanded.
    pub fn discriminator_path(
        &self,
        discriminator: &NativeBorrowedStorageDiscriminator,
    ) -> impl ExactSizeIterator<Item = CanonicalStructuralPathSegment> + '_ {
        self.paths
            .span_or_empty(discriminator.path)
            .iter()
            .map(|step| step.0)
    }

    /// Fixed-array dimensions shared by this discriminator's storage occurrences.
    pub fn discriminator_repetitions(
        &self,
        discriminator: &NativeBorrowedStorageDiscriminator,
    ) -> &[NativeStorageRepetition] {
        self.repetitions.span_or_empty(discriminator.repetitions)
    }

    /// Exact nominal case to native ordinal mapping supplied by target lowering.
    pub fn discriminator_cases(
        &self,
        discriminator: &NativeBorrowedStorageDiscriminator,
    ) -> impl Iterator<Item = &NativeStorageCaseTag> {
        self.cases
            .span_or_empty(discriminator.cases)
            .iter()
            .filter_map(|slot| slot.0.as_ref())
    }

    pub fn opaque_regions(&self) -> &[NativeBorrowedStorageOpaqueRegion] {
        self.opaque_storage.span_or_empty(self.opaque_regions)
    }

    pub fn opaque_region_path(
        &self,
        region: &NativeBorrowedStorageOpaqueRegion,
    ) -> impl ExactSizeIterator<Item = CanonicalStructuralPathSegment> + '_ {
        self.paths
            .span_or_empty(region.path)
            .iter()
            .map(|step| step.0)
    }

    pub fn opaque_region_repetitions(
        &self,
        region: &NativeBorrowedStorageOpaqueRegion,
    ) -> &[NativeStorageRepetition] {
        self.repetitions.span_or_empty(region.repetitions)
    }
}
