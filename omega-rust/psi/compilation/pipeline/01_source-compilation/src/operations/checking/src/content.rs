//! Bind content endpoints only for this exact contract question. These rows
//! are proof context, never custody or a grant to establish a qualified value.

use super::{Bindings, Unbound};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentConservation, ContentIntervalInterpretation,
    ContentPlaceVersion, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralCaseId, ValueId,
};
use terminal_psi::{StructuralParameterDeclaration, TerminalMachineResult, TerminalModule};

pub(super) struct AtCut<'a, 'binding> {
    pub module: &'a TerminalModule,
    pub parameters: &'a [StructuralParameterDeclaration],
    pub result: Option<&'a TerminalMachineResult>,
    pub bindings: &'binding mut dyn Bindings<Error = Unbound>,
    pub interpretations: &'binding mut Vec<ContentIntervalInterpretation>,
}

impl Bindings for AtCut<'_, '_> {
    type Error = Unbound;
    fn value(&mut self, id: ValueId, ty: ScalarType) -> Result<ScalarTerm, Unbound> {
        self.bindings.value(id, ty)
    }
    fn value_alternatives(
        &mut self,
        id: ValueId,
        ty: ScalarType,
    ) -> Result<Vec<ScalarTerm>, Unbound> {
        self.bindings.value_alternatives(id, ty)
    }
    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[Segment],
        ty: ScalarType,
    ) -> Result<ScalarTerm, Unbound> {
        self.bindings.field(version, root, path, ty)
    }
    fn field_alternatives(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[Segment],
        ty: ScalarType,
    ) -> Result<Vec<ScalarTerm>, Unbound> {
        self.bindings.field_alternatives(version, root, path, ty)
    }
    fn extent(
        &mut self,
        root: PlaceId,
        path: &[Segment],
        ty: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Unbound> {
        self.bindings.extent(root, path, ty)
    }
    fn byte_predicate(
        &mut self,
        predicate: semantic_vocabulary::ByteSequencePredicate,
        root: PlaceId,
        path: &[Segment],
    ) -> Result<ScalarTerm, Unbound> {
        self.bindings.byte_predicate(predicate, root, path)
    }
    fn case(
        &mut self,
        root: PlaceId,
        path: &[Segment],
        case: StructuralCaseId,
    ) -> Result<bool, Unbound> {
        self.bindings.case(root, path, case)
    }
    fn case_predicate(
        &mut self,
        root: PlaceId,
        path: &[Segment],
        case: StructuralCaseId,
    ) -> Result<Proposition, Unbound> {
        self.bindings.case_predicate(root, path, case)
    }
    fn validate_float_meaning_source(&mut self, source: &ScalarTerm) -> Result<(), Unbound> {
        self.bindings.validate_float_meaning_source(source)
    }
    fn requires_complete_terms(&self) -> bool {
        self.bindings.requires_complete_terms()
    }
    fn computed(&mut self, term: ScalarTerm) -> Result<ScalarTerm, Unbound> {
        self.bindings.computed(term)
    }
    fn content(&mut self, equation: &ContentConservation) -> Result<Proposition, Unbound> {
        let roots = self
            .parameters
            .iter()
            .map(|parameter| (parameter.place, parameter.structural_type))
            .chain(
                self.result
                    .and_then(TerminalMachineResult::structural)
                    .map(|result| (result.place, result.structural_type)),
            );
        let occurrences = terminal_semantics::content_projections::resolve(
            &self.module.structural_types,
            &self.module.structural_domains,
            &self.module.identities,
            roots,
            equation,
        )?;
        let mut goals = Vec::new();
        for occurrence in occurrences {
            let (interpretation, formation) = terminal_semantics::content_projections::bind(
                &self.module.structural_types,
                &self.module.identities,
                &occurrence,
                self.bindings,
            )?;
            goals.extend(formation);
            if let Some(previous) = self.interpretations.iter().find(|previous| {
                previous.projection == interpretation.projection
                    && previous.subject == interpretation.subject
            }) {
                if previous != &interpretation {
                    return Err(
                        "content subject has inconsistent endpoint observations at one cut".into(),
                    );
                }
            } else {
                self.interpretations.push(interpretation);
            }
        }
        goals.extend(
            terminal_semantics::content_projections::separation_requirements(
                equation,
                self.interpretations,
            )?,
        );
        goals.push(Proposition::ContentConservation(equation.clone()));
        Ok(Proposition::Conjunction(goals))
    }
}

/// Keep exact endpoint meanings with every question, including certificate
/// replay. Ordinary scalar questions borrow their existing context unchanged.
pub(super) fn context<'a>(
    base: &'a semantic_vocabulary::PropositionContext,
    interpretations: Vec<ContentIntervalInterpretation>,
    machine: semantic_vocabulary::MachineId,
    obligation: semantic_vocabulary::ObligationId,
) -> terminal_check_tables::Checked<std::borrow::Cow<'a, semantic_vocabulary::PropositionContext>> {
    if interpretations.is_empty() {
        return Ok(std::borrow::Cow::Borrowed(base));
    }
    base.clone()
        .with_content_intervals(interpretations)
        .map(std::borrow::Cow::Owned)
        .map_err(|_| {
            super::refusal(
                terminal_check_tables::RefusalReason::ForeignIdentity,
                machine,
                terminal_check_tables::Construct::Obligation(obligation),
                "content endpoints are not valid observations of this question",
            )
        })
}
