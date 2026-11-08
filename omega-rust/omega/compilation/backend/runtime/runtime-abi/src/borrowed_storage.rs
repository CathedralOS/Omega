/*
We retain the native placement of scalar leaves, sum discriminators, and opaque
regions inside borrowed structural storage (`NativeBorrowedStorageLayout`).

Target lowering populates these coordinates from its ordinary storage geometry
so host adapters can encode and decode captured semantic fields. Neither the
layout nor a byte offset establishes a loan or permits replacement of unmodeled
storage: padding, pointers, and inactive sum cases stay in `opaque_regions` so
a semantic field snapshot never overwrites them.
*/

use arena::{Arena, HandleSpan};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralCaseId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeStoragePathStep(CanonicalStructuralPathSegment);

impl Default for NativeStoragePathStep {
    fn default() -> Self {
        Self(CanonicalStructuralPathSegment::FixedIndex(0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeStorageRepetition {
    pub path_position: usize, // Fixed-array dimension position in the structural path.
    pub length: u64,
    pub byte_stride: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeBorrowedStorageOpaqueRegion {
    path: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
    pub byte_offset: u32,
    pub byte_size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeStorageCaseTag {
    pub case: StructuralCaseId,
    pub native_tag: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBorrowedStorageDiscriminator {
    path: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
    cases: HandleSpan<NativeStorageCaseTagSlot>,
    pub structural_type: StructuralTypeId,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct NativeStorageDiscriminatorSlot(Option<NativeBorrowedStorageDiscriminator>);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct NativeStorageCaseTagSlot(Option<NativeStorageCaseTag>);

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn discriminators(&self) -> impl Iterator<Item = &NativeBorrowedStorageDiscriminator> {
        self.discriminator_storage
            .span_or_empty(self.discriminators)
            .iter()
            .filter_map(|slot| slot.0.as_ref())
    }

    pub fn discriminator_path(
        &self,
        discriminator: &NativeBorrowedStorageDiscriminator,
    ) -> impl ExactSizeIterator<Item = CanonicalStructuralPathSegment> + '_ {
        self.paths
            .span_or_empty(discriminator.path)
            .iter()
            .map(|step| step.0)
    }

    pub fn discriminator_repetitions(
        &self,
        discriminator: &NativeBorrowedStorageDiscriminator,
    ) -> &[NativeStorageRepetition] {
        self.repetitions.span_or_empty(discriminator.repetitions)
    }

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
