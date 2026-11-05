//! Read stored pointer bytes under the original entry permission.
//! Captured pointees remain separate from their carriers; lifetime and write
//! checks keep their ordinary strict storage judgments.

use super::{
    Custody, Leaf, PlaceId, PsiOptimizationFunction, ReferenceOriginProjection, StructuralAccess,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, access_covers,
    backing_storage_access_conflicts, borrowed_arrivals, descriptor_backing, find_structural_type,
    leaf_reference_access, source_contract,
};

/// A spatial read of one physical descriptor carrier. Preparing it once keeps
/// the original entry's typed inline path separate from every captured pointee.
/// Lifetime debts are checked independently before this query is used.
pub(super) struct CarrierStorageRead<'a> {
    function: &'a PsiOptimizationFunction,
    custody: &'a Custody,
    types: &'a [StructuralTypeDeclaration],
    place: PlaceId,
    path: &'a [StructuralPathSegment],
    inline_entry: bool,
    storage_path: Vec<ReferenceOriginProjection>,
}

impl<'a> CarrierStorageRead<'a> {
    pub(super) fn new(
        function: &'a PsiOptimizationFunction,
        custody: &'a Custody,
        types: &'a [StructuralTypeDeclaration],
        place: PlaceId,
        path: &'a [StructuralPathSegment],
    ) -> Self {
        // State aliases must first resolve their real storage and grants;
        // an inline-looking receiving type alone grants no independent read.
        let inline_entry = !custody.borrowed_sources.contains_key(&place)
            && function
                .structural_parameter(place)
                .is_some_and(|parameter| {
                    access_covers(parameter.access, StructuralAccess::SharedBorrow)
                        && terminal_semantics::projection_stays_in_inline_storage(
                            types.iter(),
                            parameter.structural_type,
                            path,
                        )
                });
        let storage_path = if inline_entry {
            path.iter()
                .cloned()
                .map(ReferenceOriginProjection::Exact)
                .collect()
        } else {
            Vec::new()
        };
        Self {
            function,
            custody,
            types,
            place,
            path,
            inline_entry,
            storage_path,
        }
    }

    pub(super) fn conflicts(&self, reference: &Leaf) -> bool {
        if borrowed_arrivals::lends_from(
            self.custody,
            self.place,
            &reference.identity,
            StructuralAccess::SharedBorrow,
        ) || leaf_reference_access(self.types, reference) == Some(StructuralAccess::SharedBorrow)
        {
            return false;
        }
        if !self.inline_entry {
            return reference.backing.iter().any(|backing| {
                backing_storage_access_conflicts(
                    self.function,
                    self.custody,
                    self.types,
                    backing,
                    self.place,
                    self.path,
                )
            });
        }
        let origin_overlaps = |origin_path: &[ReferenceOriginProjection]| {
            let Some(origin) = source_contract(self.function, self.custody, self.place)
                .and_then(|root| {
                    terminal_semantics::captured_reference_origin_type(
                        self.types.iter(),
                        root,
                        origin_path,
                    )
                })
                .and_then(|identity| find_structural_type(self.types, identity))
            else {
                return true;
            };
            !matches!(origin.shape, StructuralTypeShape::Reference { .. })
                && terminal_semantics::reference_origin_paths_may_overlap(
                    &self.storage_path,
                    origin_path,
                )
        };
        reference.backing.iter().any(|backing| {
            descriptor_backing::overlaps_inline_carrier_read(
                backing,
                self.function.machine,
                self.place,
                self.path,
                &origin_overlaps,
            )
        })
    }
}
