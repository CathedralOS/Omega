//! Form the sealed relation using one transparent, universe-polymorphic Id.
//!
//! Conversion retains this checked declaration's meaning, so a mathematical
//! wrapper returns the same typed identity value as a direct application.
//! Neither formation nor the declaration body supplies a witness of the Id.

use super::{Elaborator, Term, TermHandle};
use diagnostics::Diagnostic;
use proof_admission::{Budget, DEFAULT_CONVERSION_STEPS, Declaration, Level, Sort, infer_sort};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::types::TypeReferenceHandle;

impl Elaborator<'_> {
    pub(super) fn elaborate_identity(
        &mut self,
        expression: ExpressionHandle,
        call: &TableCallExpression,
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let application =
            validation::mathematical_applications::identity_application(self.program, call)
                .map_err(|reason| self.refuse(reason.to_owned()))?;
        let ty = self.elaborate_source_operand_type(application.carrier)?;
        self.retain_identity_carrier(application.carrier, ty);
        let context = self.instantiation_context();
        let sort = infer_sort(
            &mut self.arena,
            &context,
            ty,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .map_err(|error| {
            self.refuse(format!("core::Identity carrier fails formation: {error:?}"))
        })?;
        let Sort::Type(level) = sort else {
            return Err(self.refuse("core::Identity requires a relevant Type carrier".to_owned()));
        };
        let declaration = self.identity_declaration();
        let mut term = self.arena.insert(Term::Constant {
            declaration,
            levels: vec![level],
        });
        term = self.arena.insert(Term::Apply {
            function: term,
            argument: ty,
        });
        for subject in application.subjects {
            let value = self.elaborate_expression(subject)?;
            self.retain_source_result(subject, ty, value, false)?;
            term = self.arena.insert(Term::Apply {
                function: term,
                argument: value,
            });
        }
        self.span = Some(self.program.expression_table.source_span(expression));
        Ok(term)
    }

    fn identity_declaration(&mut self) -> u32 {
        if let Some(declaration) = self.identity_declaration {
            return declaration;
        }
        let universe = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let a = self.arena.insert(Term::Variable(0));
        let a_under_left = self.arena.insert(Term::Variable(1));
        let a_under_both = self.arena.insert(Term::Variable(2));
        let right = self.arena.insert(Term::Variable(0));
        let left = self.arena.insert(Term::Variable(1));
        let result = self.arena.insert(Term::Id {
            ty: a_under_both,
            left,
            right,
        });
        let body = self.arena.insert(Term::Lambda {
            domain: a_under_left,
            body: result,
        });
        let body = self.arena.insert(Term::Lambda { domain: a, body });
        let body = self.arena.insert(Term::Lambda {
            domain: universe,
            body,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: a_under_left,
            codomain: universe,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: a,
            codomain: ty,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: universe,
            codomain: ty,
        });
        let declaration = self.declarations.len() as u32;
        self.declarations.push(Declaration {
            level_arity: 1,
            ty,
            body: Some(body),
        });
        self.identity_declaration = Some(declaration);
        declaration
    }

    pub(super) fn call_returns_type(&self, call: &TableCallExpression) -> bool {
        validation::mathematical_applications::is_identity_call(self.program, call)
            || self
                .type_returning_declarations
                .contains(&call.target_symbol)
    }

    pub(super) fn retain_identity_carrier(
        &mut self,
        source_type: TypeReferenceHandle,
        ty: TermHandle,
    ) {
        let type_identity = self.program.normalized_type_identity(source_type);
        if !self
            .prepared
            .identity_carriers
            .iter()
            .any(|(_, row)| row.ty == ty && row.type_identity == type_identity)
        {
            self.prepared.identity_carriers.append(
                validation::mathematical_applications::PreparedIdentityCarrier {
                    ty,
                    source_type,
                    type_identity,
                },
            );
        }
    }

    pub(super) fn normalize_identity_carriers(
        &mut self,
        signature: &proof_admission::Signature,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut rows = std::mem::take(&mut self.prepared.identity_carriers);
        let handles = rows.iter().map(|(handle, _)| handle).collect::<Vec<_>>();
        for handle in handles {
            let row = rows.get_mut(handle);
            row.ty = super::normalization::normalize(
                &mut self.arena,
                signature,
                row.ty,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                &|declaration| {
                    self.identity_declaration == Some(declaration)
                        || self
                            .source_aliases
                            .iter()
                            .any(|alias| alias.declaration == declaration)
                        || self
                            .indexed_families
                            .values()
                            .any(|family| family.declaration == declaration)
                },
            )
            .map_err(|error| {
                self.refuse(format!("identity carrier reduction failed: {error:?}"))
            })?;
        }
        self.prepared.identity_carriers = rows;
        Ok(())
    }
}
