//! Source types keep one universe per binder, including when a data description
//! is prepared inside another declaration. A family template has its own level
//! telescope; applying it substitutes the actual type levels before any field,
//! constructor or theorem consumes the description.

use super::{Elaborator, Level, Term, TermHandle};
use symbols::SymbolHandle;
use typed_trees::{
    data::{TypeParameter, TypeParameterKind},
    name::Identifier,
};

pub(super) struct SourceLevelScope {
    binders: Vec<SymbolHandle>,
    names: Vec<Identifier>,
    generalized: u32,
    types: Vec<(SymbolHandle, Level)>,
}

impl Elaborator<'_> {
    pub(super) fn enter_family_levels(&mut self, parameters: &[TypeParameter]) -> SourceLevelScope {
        let saved = SourceLevelScope {
            binders: std::mem::take(&mut self.level_binders),
            names: std::mem::take(&mut self.level_names),
            generalized: std::mem::take(&mut self.generalized_levels),
            types: std::mem::take(&mut self.source_type_levels),
        };
        for parameter in parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
        {
            self.source_type_level(parameter.symbol);
        }
        saved
    }

    pub(super) fn restore_source_levels(&mut self, saved: SourceLevelScope) {
        self.level_binders = saved.binders;
        self.level_names = saved.names;
        self.generalized_levels = saved.generalized;
        self.source_type_levels = saved.types;
    }

    pub(super) fn family_actual_levels(
        &mut self,
        parameters: &[TypeParameter],
        actuals: &[TermHandle],
    ) -> Result<Vec<Level>, Vec<diagnostics::Diagnostic>> {
        if parameters.len() != actuals.len() {
            return Err(self.refuse("source family lost its exact level/type telescope".into()));
        }
        let context = self.instantiation_context();
        let mut levels = Vec::new();
        for (parameter, actual) in parameters.iter().zip(actuals) {
            if matches!(parameter.kind, TypeParameterKind::Type) {
                let sort = proof_admission::infer_sort(
                    &mut self.arena,
                    &context,
                    *actual,
                    &mut proof_admission::Budget::new(proof_admission::DEFAULT_CONVERSION_STEPS),
                )
                .map_err(|error| {
                    self.refuse(format!(
                        "source family type argument fails universe formation: {error:?}"
                    ))
                })?;
                let proof_admission::Sort::Type(level) = sort else {
                    return Err(self
                        .refuse("source family Type argument belongs to the strict layer".into()));
                };
                levels.push(level);
            }
        }
        Ok(levels)
    }

    pub(super) fn quantified_constant(&mut self, position: u32) -> TermHandle {
        let arity = self.declarations[position as usize].level_arity;
        self.arena.insert(Term::Constant {
            declaration: position,
            levels: (0..arity).map(Level::Parameter).collect(),
        })
    }
}
