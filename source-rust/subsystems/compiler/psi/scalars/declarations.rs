use crate::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScalarDeclarations {
    pub scalar_applications: ScalarApplications,
    pub folded_constants: FoldedConstantClosure,
    pub float_meaning_projections: Vec<crate::FloatMeaningProjection>,
    pub float_meaning_equalities: Vec<crate::FloatMeaningEqualityProposition>,
    pub scalar_predicate_definitions: Vec<ScalarPredicateDefinition>,
}
