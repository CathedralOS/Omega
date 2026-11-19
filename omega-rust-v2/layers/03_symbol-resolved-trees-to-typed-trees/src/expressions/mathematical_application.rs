use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::DeclarationId;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn mathematical_citation(
        &mut self,
        node_index: usize,
        id: &DeclarationId,
        expected: &ValueType,
    ) -> Result<(), TypeError> {
        let signature = &self.catalog.signatures[id];
        let mut conclusion = expected.clone();
        for (span, parameter) in &signature.parameters {
            let ValueType::Function(input, output) = conclusion else {
                return Err(TypeError::at(self.unit.file, node_index, "mathematical citation has incompatible parameters"));
            };
            if *input != *parameter {
                return Err(TypeError::at(self.unit.file, node_index, "mathematical citation has incompatible parameters"));
            }
            let mut binder = signature.site.clone();
            binder.span = *span;
            conclusion = output.apply_subject(&ValueType::Subject {
                declaration: binder,
                version: None,
            });
        }
        if self.catalog.types.arity(id) != 0
            || !matches!(conclusion, ValueType::FamilyApplication { .. } | ValueType::Squash(_))
            || (signature.result != ValueType::NoResult && signature.result != conclusion)
        {
            return Err(TypeError::at(
                self.unit.file,
                node_index,
                "citation needs a closed mathematical evidence interface",
            ));
        }
        self.record_obligation(
            node_index,
            ObligationKind::RefineMathematicalCitation {
                machine: id.clone(),
                expected: expected.clone(),
            },
        );
        Ok(())
    }

    pub(super) fn mathematical_application(
        &mut self,
        node_index: usize,
        callee: usize,
        definition: Option<&DeclarationId>,
        mut function: ValueType,
        arguments: &[usize],
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        if let Some(assumptions) = definition.and_then(|id| self.catalog.assumptions.get(id)) {
            if self.context != super::body_typer::ExpressionContext::Proof {
                return Err(TypeError::at(
                    source,
                    node_index,
                    "mathematical assumption cannot supply executable data or control",
                ));
            }
            for assumption in assumptions {
                self.record_obligation(node_index, ObligationKind::AdmitMathematicalAssumption(assumption.clone()));
            }
        }
        let mut formals = Vec::new();
        let explicit = definition.map_or(&[][..], |id| self.unit.arguments_for(callee, id));
        if let Some(id) = definition {
            formals = self
                .unit
                .applied_formals(callee, id, explicit, &self.catalog.types, Clone::clone)?;
        } else if source.nodes[callee].kind == NodeKind::PathExpression && !source.nodes[callee].children.is_empty() {
            return Err(TypeError::at(source, callee, "mathematical parameter has no generic telescope"));
        }
        let mut bindings = vec![None; formals.len()];
        for (slot, value) in bindings.iter_mut().zip(&formals[..explicit.len()]) {
            *slot = Some(value.clone());
        }
        let mut supplied = Vec::with_capacity(arguments.len());
        for &arg in arguments {
            // The remaining codomain can itself be a function after specialization.
            let applied = definition.map_or_else(|| function.clone(), |id| function.substitute(id, &formals));
            let ValueType::Function(parameter, rest) = applied else {
                return Err(TypeError::at(source, node_index, "excess mathematical arguments"));
            };
            let actual = self.type_expression_with_expected_type(arg, (!parameter.contains_parameter()).then_some(&parameter))?;
            if let Some(id) = definition {
                parameter.infer(&actual, id, &mut bindings);
                for (formal, bound) in formals.iter_mut().zip(&bindings) {
                    if let Some(value) = bound {
                        *formal = value.clone();
                    }
                }
            }
            let version = match self.unit.symbol(arg) {
                Some(Symbol::Binding { span, .. }) if self.writable.contains(span) => self.version.as_ref(),
                _ => None,
            };
            let subject = self
                .unit
                .mathematical_subject(arg, &parameter, &self.catalog.types, version)
                .unwrap_or_else(|_| self.unit.term_subject(arg, &parameter, version));
            function = rest.apply_subject(&subject);
            supplied.push((arg, *parameter, actual));
        }
        let generic_arguments = if let Some(id) = definition {
            if let Some(expected) = expected {
                function.infer(expected, id, &mut bindings);
            }
            let selected = bindings
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| TypeError::at(source, callee, "cannot infer all mathematical generic arguments"))?;
            self.validate_call_bindings(node_index, callee, id, explicit, &selected, &[])?;
            function = function.substitute(id, &selected);
            selected
        } else {
            Vec::new()
        };
        for (arg, param, actual) in supplied {
            let input = definition.map_or(param.clone(), |id| param.substitute(id, &generic_arguments));
            self.check_destination(arg, &actual, &input)?;
        }
        self.record_obligation(
            node_index,
            ObligationKind::MathematicalApplication {
                definition: definition.cloned(),
                generic_arguments,
                argument_count: arguments.len(),
            },
        );
        Ok(function)
    }
}
