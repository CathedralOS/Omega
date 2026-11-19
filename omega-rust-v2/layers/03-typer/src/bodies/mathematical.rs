use super::*;
use omega_resolver::DeclarationId;

impl Body<'_, '_> {
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
            supplied.push((arg, *parameter, actual));
            function = *rest;
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
