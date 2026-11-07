//! Form the sealed relation using one transparent, universe-polymorphic Id.
//!
//! Conversion retains this checked declaration's meaning, so a mathematical
//! wrapper returns the same typed identity value as a direct application.
//! Neither formation nor the declaration body supplies a witness of the Id.

use super::{Elaborator, Term, TermHandle};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use proof_admission::{Budget, DEFAULT_CONVERSION_STEPS, Declaration, Level, Sort, infer_sort};
use symbols::SymbolHandle;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, applied_type_identity,
};
use typed_trees::types::TypeReferenceHandle;
use validation::mathematical_applications::{
    MathematicalBinderKind, PreparedIdentityValue, source_generic_argument_plan,
};

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
        self.record_identity_emission(
            term,
            application.carrier,
            self.source_owner,
            Vec::new(),
            Vec::new(),
        )?;
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

    /// Every emission keeps its source type template and exact application
    /// scopes. The calculus value follows the same level and beta substitution
    /// as the authored declaration; no consumer reconstructs a source carrier
    /// from an ambiguous shared kernel type.
    fn record_identity_emission(
        &mut self,
        value: TermHandle,
        source_type: TypeReferenceHandle,
        source_owner: SymbolHandle,
        source_scopes: Vec<IdentitySourceScope>,
        pending: Vec<IdentitySourceParameter>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut abstraction = value;
        for entry in self.scope.iter().rev() {
            abstraction = self.arena.insert(Term::Lambda {
                domain: entry.domain,
                body: abstraction,
            });
        }
        let Some(requirements) = self
            .identity_requirements_by_owner
            .get_mut(self.source_owner.arena_index() as usize)
        else {
            return Err(self.refuse("Id emission has no retained source owner".into()));
        };
        if requirements.owner != self.source_owner {
            *requirements = IdentityRequirements {
                owner: self.source_owner,
                ..Default::default()
            };
        }
        requirements.emissions.push(self.identity_emissions.len());
        self.identity_emissions.push(IdentityEmission {
            value,
            abstraction,
            source_type,
            source_owner,
            source_scopes,
            pending,
        });
        Ok(())
    }

    pub(super) fn transport_identity_requirements(
        &mut self,
        call: &TableCallExpression,
        callee: TermHandle,
        inputs: &[super::source_domains::SourceInput],
    ) -> Result<(), Vec<Diagnostic>> {
        let actuals = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        self.transport_identity_value(
            call.target_symbol,
            &call.machine_arguments,
            actuals,
            callee,
            inputs,
        )
    }

    pub(super) fn transport_identity_value(
        &mut self,
        target: SymbolHandle,
        static_arguments: &[StaticMachineArgument],
        actuals: &[ExpressionHandle],
        mut callee: TermHandle,
        inputs: &[super::source_domains::SourceInput],
    ) -> Result<(), Vec<Diagnostic>> {
        if !self.authored_index.contains_key(&target) {
            if self.scope.iter().any(|entry| entry.symbol == Some(target)) {
                let mut value = callee;
                for input in inputs {
                    value = self.arena.insert(Term::Apply {
                        function: value,
                        argument: input.term,
                    });
                }
                self.record_identity_application(IdentityApplication {
                    parameter: target,
                    value,
                    abstraction: value,
                    arguments: inputs.iter().map(|input| input.expression).collect(),
                    source_scopes: Vec::new(),
                    pending: Vec::new(),
                })?;
            }
            return Ok(());
        }
        let requirements = self
            .identity_requirements_by_owner
            .get(target.arena_index() as usize)
            .filter(|requirements| requirements.owner == target)
            .cloned()
            .unwrap_or_default();
        if requirements.emissions.is_empty() && requirements.applications.is_empty() {
            return Ok(());
        }
        let emitted = requirements
            .emissions
            .iter()
            .map(|position| self.identity_emissions[*position].clone())
            .collect::<Vec<_>>();
        let Some(&index) = self.authored_index.get(&target) else {
            return Err(self.refuse("Id value lost its mathematical declaration owner".into()));
        };
        let definition = &self.program.mathematical_definitions()[index];
        let binders = self
            .program
            .data_type_parameters
            .span_or_empty(definition.binders);
        let plan =
            source_generic_argument_plan(self.program, binders, static_arguments, &self.fixed_core)
                .ok_or_else(|| {
                    self.refuse("Id value lost its checked generic application telescope".into())
                })?;
        let mut arguments = Vec::new();
        for slot in plan {
            if matches!(slot.kind, MathematicalBinderKind::Level) {
                continue;
            }
            let actual = slot.argument.ok_or_else(|| {
                self.refuse("Id application lost a supplied generic operand".into())
            })?;
            arguments.push((
                slot.binder.symbol,
                IdentitySourceArgument::Static(actual.clone()),
            ));
        }
        let formals = self.program.mathematical_parameters(definition.parameters);
        arguments.extend(
            formals.iter().zip(actuals).map(|(formal, actual)| {
                (formal.symbol, IdentitySourceArgument::Expression(*actual))
            }),
        );
        let scope = IdentitySourceScope {
            declaration: definition.symbol,
            arguments,
        };
        let mut operands = Vec::new();
        while let Term::Apply { function, argument } = self.arena.get(callee) {
            operands.push(argument);
            callee = function;
        }
        operands.reverse();
        operands.extend(inputs.iter().map(|input| input.term));
        let Term::Constant { levels, .. } = self.arena.get(callee) else {
            return Err(self.refuse("Id application has no checked constant spine".into()));
        };
        for row in emitted {
            let value =
                self.instantiate_identity_requirement(row.abstraction, &levels, &operands)?;
            self.retain_instantiated_identity(row, value, &scope, formals, actuals, &[])?;
        }
        let static_arity = operands.len() - inputs.len();
        for mut application in requirements.applications {
            let ordinal = formals
                .iter()
                .position(|formal| formal.symbol == application.parameter)
                .or_else(|| {
                    application
                        .pending
                        .iter()
                        .position(|parameter| parameter.binder == application.parameter)
                        .map(|position| formals.len() + position)
                });
            let Some(ordinal) = ordinal else {
                return Err(self.refuse("Id function application lost its exact formal".into()));
            };
            let trailing = actuals
                .iter()
                .skip(formals.len())
                .copied()
                .collect::<Vec<_>>();
            self.complete_identity_scopes(
                &mut application.source_scopes,
                &application.pending,
                &trailing,
            )?;
            let mut pending = formals
                .iter()
                .skip(actuals.len())
                .map(|formal| IdentitySourceParameter {
                    declaration: definition.symbol,
                    binder: formal.symbol,
                })
                .collect::<Vec<_>>();
            pending.extend(application.pending.iter().skip(trailing.len()).cloned());
            application.pending = pending;
            let Some(input) = inputs.get(ordinal) else {
                // An unapplied function formal stays in the checked closure;
                // the next prefix application will supply its requirements.
                let value = self.instantiate_identity_requirement(
                    application.abstraction,
                    &levels,
                    &operands,
                )?;
                let mut application = application;
                application.value = value;
                application.source_scopes.insert(0, scope.clone());
                self.record_identity_application(application)?;
                continue;
            };
            let rows = input
                .identity_emissions
                .clone()
                .map(|position| self.identity_emissions[position].clone())
                .collect::<Vec<_>>();
            for row in rows {
                let mut kernel_operands = operands.clone();
                kernel_operands[static_arity + ordinal] = row.value;
                let value = self.instantiate_identity_requirement(
                    application.abstraction,
                    &levels,
                    &kernel_operands,
                )?;
                let mut row = row;
                row.source_scopes
                    .splice(0..0, application.source_scopes.clone());
                self.retain_instantiated_identity(
                    row,
                    value,
                    &scope,
                    formals,
                    actuals,
                    &application.arguments,
                )?;
            }
            if let typed_trees::expression::ExpressionNode::Name(path) =
                self.program.expression_table.expression(input.expression)
                && self
                    .scope
                    .iter()
                    .any(|entry| entry.symbol == Some(path.symbol))
            {
                let value = self.instantiate_identity_requirement(
                    application.abstraction,
                    &levels,
                    &operands,
                )?;
                let mut application = application;
                application.parameter = path.symbol;
                application.value = value;
                application.source_scopes.insert(0, scope.clone());
                self.record_identity_application(application)?;
            }
        }
        Ok(())
    }

    fn instantiate_identity_requirement(
        &mut self,
        abstraction: TermHandle,
        levels: &[Level],
        arguments: &[TermHandle],
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let mut value = proof_admission::instantiate_levels(&mut self.arena, abstraction, levels)
            .map_err(|level| {
            self.refuse(format!("Id application lost universe parameter {level}"))
        })?;
        for argument in arguments {
            value = self.arena.insert(Term::Apply {
                function: value,
                argument: *argument,
            });
        }
        Ok(value)
    }

    fn retain_instantiated_identity(
        &mut self,
        row: IdentityEmission,
        value: TermHandle,
        scope: &IdentitySourceScope,
        formals: &[typed_trees::mathematical::MathematicalParameter],
        actuals: &[ExpressionHandle],
        function_arguments: &[ExpressionHandle],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut scopes = Vec::with_capacity(row.source_scopes.len() + 1);
        scopes.push(scope.clone());
        scopes.extend(row.source_scopes);
        let trailing = actuals
            .iter()
            .skip(formals.len())
            .copied()
            .collect::<Vec<_>>();
        let supplied = function_arguments
            .iter()
            .copied()
            .chain(trailing)
            .collect::<Vec<_>>();
        self.complete_identity_scopes(&mut scopes, &row.pending, &supplied)?;
        let mut pending = formals
            .iter()
            .skip(actuals.len())
            .map(|formal| IdentitySourceParameter {
                declaration: scope.declaration,
                binder: formal.symbol,
            })
            .collect::<Vec<_>>();
        pending.extend(row.pending.into_iter().skip(supplied.len()));
        self.record_identity_emission(value, row.source_type, row.source_owner, scopes, pending)
    }

    fn complete_identity_scopes(
        &self,
        scopes: &mut [IdentitySourceScope],
        pending: &[IdentitySourceParameter],
        supplied: &[ExpressionHandle],
    ) -> Result<(), Vec<Diagnostic>> {
        for (parameter, actual) in pending.iter().zip(supplied) {
            let Some(scope) = scopes
                .iter_mut()
                .find(|scope| scope.declaration == parameter.declaration)
            else {
                return Err(
                    self.refuse("Id prefix application lost its supplying telescope".into())
                );
            };
            scope.arguments.push((
                parameter.binder,
                IdentitySourceArgument::Expression(*actual),
            ));
        }
        Ok(())
    }

    fn record_identity_application(
        &mut self,
        mut application: IdentityApplication,
    ) -> Result<(), Vec<Diagnostic>> {
        application.abstraction = application.value;
        for entry in self.scope.iter().rev() {
            application.abstraction = self.arena.insert(Term::Lambda {
                domain: entry.domain,
                body: application.abstraction,
            });
        }
        let Some(requirements) = self
            .identity_requirements_by_owner
            .get_mut(self.source_owner.arena_index() as usize)
        else {
            return Err(self.refuse("Id function application has no retained source owner".into()));
        };
        if requirements.owner != self.source_owner {
            *requirements = IdentityRequirements {
                owner: self.source_owner,
                ..Default::default()
            };
        }
        requirements.applications.push(application);
        Ok(())
    }

    pub(super) fn prepare_identity_values(
        &mut self,
        signature: &proof_admission::Signature,
        context: &proof_admission::Context,
        retained_constants: &std::collections::HashSet<u32>,
        emissions: &std::ops::Range<usize>,
    ) -> Result<HandleSpan<PreparedIdentityValue>, Vec<Diagnostic>> {
        let mut values = Vec::with_capacity(emissions.len());
        for position in emissions.clone() {
            let row = &self.identity_emissions[position];
            let term = super::normalization::normalize(
                &mut self.arena,
                signature,
                row.value,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
                &|declaration| retained_constants.contains(&declaration),
            )
            .map_err(|error| self.refuse(format!("Id value substitution failed: {error:?}")))?;
            let mut head = term;
            let mut arity = 0;
            while let Term::Apply { function, .. } = self.arena.get(head) {
                arity += 1;
                head = function;
            }
            if arity != 3
                || !matches!(self.arena.get(head), Term::Constant { declaration, .. } if Some(declaration) == self.identity_declaration)
            {
                continue;
            }
            if !row.pending.is_empty() {
                return Err(self.refuse("formed Id retains unapplied source subjects".into()));
            }
            let arguments = row
                .source_scopes
                .iter()
                .map(|scope| {
                    scope
                        .arguments
                        .iter()
                        .map(|(binder, actual)| {
                            (
                                *binder,
                                match actual {
                                    IdentitySourceArgument::Static(actual) => {
                                        IndexIdentityArgument::StaticArgument(actual)
                                    }
                                    IdentitySourceArgument::Expression(actual) => {
                                        IndexIdentityArgument::Expression(*actual)
                                    }
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let scopes = row
                .source_scopes
                .iter()
                .zip(&arguments)
                .map(|(scope, arguments)| AppliedTypeIdentityScope {
                    declaration: scope.declaration,
                    arguments,
                    lifetimes: &[],
                })
                .collect::<Vec<_>>();
            let type_identity =
                applied_type_identity(self.program, row.source_type, row.source_owner, &scopes)
                    .ok_or_else(|| {
                        self.refuse(
                            "Id application lost its exact source carrier substitution".into(),
                        )
                    })?;
            let ty = proof_admission::infer_type(
                &mut self.arena,
                context,
                term,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .map_err(|error| self.refuse(format!("emitted Id value fails formation: {error:?}")))?;
            values.push(PreparedIdentityValue {
                term,
                ty,
                type_identity,
            });
        }
        Ok(self.prepared.identity_values.insert_many(values))
    }
}

#[derive(Clone)]
pub(super) struct IdentityEmission {
    value: TermHandle,
    abstraction: TermHandle,
    source_type: TypeReferenceHandle,
    source_owner: SymbolHandle,
    source_scopes: Vec<IdentitySourceScope>,
    pending: Vec<IdentitySourceParameter>,
}

#[derive(Clone)]
struct IdentitySourceScope {
    declaration: SymbolHandle,
    arguments: Vec<(SymbolHandle, IdentitySourceArgument)>,
}

#[derive(Clone)]
enum IdentitySourceArgument {
    Static(StaticMachineArgument),
    Expression(ExpressionHandle),
}

#[derive(Clone)]
struct IdentitySourceParameter {
    declaration: SymbolHandle,
    binder: SymbolHandle,
}

/// Requirements are retained on the checked mathematical value's owner. A
/// function formal records its typed application closure until an actual
/// supplies the emissions; the same level/beta substitution transports both.
#[derive(Clone, Default)]
pub(super) struct IdentityRequirements {
    owner: SymbolHandle,
    emissions: Vec<usize>,
    applications: Vec<IdentityApplication>,
}

#[derive(Clone)]
struct IdentityApplication {
    parameter: SymbolHandle,
    value: TermHandle,
    abstraction: TermHandle,
    arguments: Vec<ExpressionHandle>,
    source_scopes: Vec<IdentitySourceScope>,
    pending: Vec<IdentitySourceParameter>,
}
