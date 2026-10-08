//! Retained native placement of portable scalar leaves in one borrowed object.
//! Target lowering produces these coordinates with its ordinary storage
//! geometry. A host adapter may encode captured semantic fields with them;
//! neither the layout nor an offset establishes a loan or permits replacement.
//! Gaps are deliberately visible: a semantic field snapshot cannot invent the
//! contents of native padding, tags, pointers or other unmodeled storage.

use arena::{Arena, HandleSpan};
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarType, StructuralTypeId};

#[derive(Debug, Clone)]
pub struct NativeBorrowedStorageField {
    path: HandleSpan<NativeStoragePathStep>,
    repetitions: HandleSpan<NativeStorageRepetition>,
    pub byte_offset: u32,
    pub stored_scalar: ScalarType,
}

impl Default for NativeBorrowedStorageField {
    fn default() -> Self {
        Self {
            path: HandleSpan::empty(),
            repetitions: HandleSpan::empty(),
            byte_offset: 0,
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

#[derive(Debug, Clone)]
pub struct NativeBorrowedStorageLayout {
    pub structural_type: StructuralTypeId,
    pub byte_size: u32,
    pub alignment: u16,
    storage: Arena<NativeBorrowedStorageField>,
    fields: HandleSpan<NativeBorrowedStorageField>,
    paths: Arena<NativeStoragePathStep>,
    repetitions: Arena<NativeStorageRepetition>,
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
            ),
        >,
    ) -> Self {
        let mut paths = Arena::new();
        let mut repetitions = Arena::new();
        let fields = fields
            .into_iter()
            .map(
                |(path, dimensions, byte_offset, stored_scalar)| NativeBorrowedStorageField {
                    path: paths.insert_many(path.into_iter().map(NativeStoragePathStep)),
                    repetitions: repetitions.insert_many(dimensions),
                    byte_offset,
                    stored_scalar,
                },
            );
        let mut storage = Arena::new();
        let fields = storage.insert_many(fields);
        Self {
            structural_type,
            byte_size,
            alignment,
            storage,
            fields,
            paths,
            repetitions,
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
}
