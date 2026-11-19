use super::*;
use omega_resolver::DeclarationId;

impl Body<'_, '_> {
    pub(super) fn mathematical_citation(
        &mut self,
        n: usize,
        id: &DeclarationId,
        expected: &ValueType,
    ) -> Result<(), TypeError> {
        let sig = &self.catalog.signatures[id];
        let mut conclusion = expected.clone();
        for (span, parameter) in &sig.params {
            let ValueType::Function(input, output) = conclusion else {
                return Err(TypeError::at(&self.unit.file, n, "mathematical citation has incompatible parameters"));
            };
            if *input != *parameter {
                return Err(TypeError::at(&self.unit.file, n, "mathematical citation has incompatible parameters"));
            }
            let mut binder = sig.site.clone();
            binder.span = *span;
            conclusion = output.apply_subject(&ValueType::Subject {
                declaration: binder,
                version: None,
            });
        }
        if self.catalog.types.arities[id] != 0
            || !matches!(conclusion, ValueType::FamilyApplication { .. } | ValueType::Squash(_))
            || (sig.result != ValueType::NoResult && sig.result != conclusion)
        {
            return Err(TypeError::at(&self.unit.file, n, "citation needs a closed mathematical evidence interface"));
        }
        self.obligation(
            n,
            ObligationKind::RefineMathematicalCitation {
                machine: id.clone(),
                expected: expected.clone(),
            },
        );
        Ok(())
    }

    pub(super) fn mathematical_application(
        &mut self,
        n: usize,
        callee: usize,
        definition: Option<&DeclarationId>,
        mut function: ValueType,
        arguments: &[usize],
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        if let Some(assumptions) = definition.and_then(|id| self.catalog.assumptions.get(id)) {
            let mathematical = f.enclosing_contexts(n).any(|p| {
                matches!(
                    f.nodes[p].kind,
                    NodeKind::MathLet | NodeKind::Requires | NodeKind::Ensures | NodeKind::Hypothesis
                )
            });
            if !mathematical && !self.logical_argument {
                return Err(TypeError::at(f, n, "mathematical assumption cannot supply executable data or control"));
            }
            for assumption in assumptions {
                self.obligation(n, ObligationKind::AdmitMathematicalAssumption(assumption.clone()));
            }
        }
        let mut formals = definition.map_or_else(Vec::new, |id| self.catalog.types.formals(id));
        let mut bindings = vec![None; formals.len()];
        if f.nodes[callee].kind == NodeKind::PathExpression {
            for &section in &f.nodes[callee].children {
                let Some(id) = definition else {
                    return Err(TypeError::at(f, section, "mathematical parameter has no generic telescope"));
                };
                if f.nodes[section].children.len() > formals.len() {
                    return Err(TypeError::at(f, section, "excess generic arguments"));
                }
                for (index, &arg) in f.nodes[section].children.iter().enumerate() {
                    let value = self.unit.argument(arg, &self.catalog.types, id, index)?;
                    formals[index] = value.clone();
                    bindings[index] = Some(value);
                }
            }
        }
        let mut supplied = Vec::with_capacity(arguments.len());
        for &arg in arguments {
            // The remaining codomain can itself be a function after specialization.
            let applied = definition.map_or_else(|| function.clone(), |id| function.substitute(id, &formals));
            let ValueType::Function(parameter, rest) = applied else {
                return Err(TypeError::at(f, n, "excess mathematical arguments"));
            };
            let actual = self.expected(arg, (!parameter.contains_parameter()).then_some(&parameter))?;
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
                .ok_or_else(|| TypeError::at(f, callee, "cannot infer all mathematical generic arguments"))?;
            self.validate_call_bindings(n, callee, id, &selected)?;
            function = function.substitute(id, &selected);
            selected
        } else {
            Vec::new()
        };
        for (arg, param, actual) in supplied {
            let input = definition.map_or(param.clone(), |id| param.substitute(id, &generic_arguments));
            self.destination(arg, &actual, &input)?;
        }
        self.obligation(
            n,
            ObligationKind::MathematicalApplication {
                definition: definition.cloned(),
                generic_arguments,
                argument_count: arguments.len(),
            },
        );
        Ok(function)
    }
}
