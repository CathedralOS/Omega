//! The closed content algebras have mathematical values, rather than the
//! start/end fields used by their source literal formers. Source types and
//! captures use the same opaque carrier as canonical content derivations.

use super::Elaborator;
use diagnostics::Diagnostic;
use proof_admission::{
    Budget, DEFAULT_CONVERSION_STEPS, Sort, SourceContentTheory, TermHandle, infer_sort,
};
use typed_trees::types::TypeReferenceHandle;

impl Elaborator<'_> {
    pub(super) fn elaborate_content_carrier(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
        let Some(algebra) = validation::content_algebra_application(self.program, reference) else {
            return Ok(None);
        };
        let parameter = self.elaborate_type_reference(algebra.parameter)?;
        let context = self.instantiation_context();
        let sort = infer_sort(
            &mut self.arena,
            &context,
            parameter,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .map_err(|error| {
            self.refuse(format!(
                "canonical content parameter fails formation: {error:?}"
            ))
        })?;
        let Sort::Type(level) = sort else {
            return Err(self.refuse(
                "canonical content requires a relevant coordinate or unit type".to_owned(),
            ));
        };
        let theory = self.source_content.get_or_insert_with(|| {
            SourceContentTheory::new(&mut self.arena, &mut self.declarations)
        });
        theory
            .carrier(
                &mut self.arena,
                &mut self.declarations,
                algebra.identity.canonical_algebra().kind,
                parameter,
                level,
            )
            .map(Some)
            .map_err(|error| {
                self.refuse(format!(
                    "canonical content carrier fails formation: {error}"
                ))
            })
    }
}
