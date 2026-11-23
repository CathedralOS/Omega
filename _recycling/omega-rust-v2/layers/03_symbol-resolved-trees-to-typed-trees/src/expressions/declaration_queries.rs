use super::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::{TypeError, ValueType};
use omega_resolved::{DeclarationId, Symbol};
use omega_typed::{DeclarationSelection, MachineApplication, site_at};
use omega_syntax::NodeKind;
use omega_toolchain::DeclarationQuery;

impl BodyTyper<'_, '_> {
    pub(super) fn declaration_query(
        &mut self,
        node_index: usize,
        callee: usize,
        args: usize,
        package: &str,
        path: &str,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let query = DeclarationQuery::identify(package, path).ok_or_else(|| TypeError::at(source, callee, "unknown declaration query"))?;
        if !source.reference_sections(callee).is_empty() {
            return Err(TypeError::at(source, callee, "a declaration query does not take type arguments"));
        }
        let [argument] = *source.nodes[args].children.as_slice() else {
            return Err(TypeError::at(source, node_index, "a declaration query requires one declaration path"));
        };
        let path = source.ungroup(argument);
        let selected = if source.nodes[path].kind == NodeKind::PathExpression {
            self.unit.symbol(path)
        } else {
            None
        };
        if query == DeclarationQuery::Callable && matches!(selected, Some(Symbol::Candidates(_))) {
            return Err(TypeError::at(source, argument, "declaration selector is ambiguous"));
        }
        let member = match selected {
            Some(Symbol::Declaration(id)) => Some(id),
            _ => None,
        };
        let (selection, carrier) = match (query, member) {
            (DeclarationQuery::Field, Some(field)) if matches!(field.kind, NodeKind::Field | NodeKind::CaseField) => {
                (self.describe_field(path, field)?, r"reflect\FieldInfo")
            }
            (DeclarationQuery::Callable, Some(machine)) if matches!(machine.kind, NodeKind::Machine | NodeKind::AsmMachine) => {
                (self.describe_callable(path, machine)?, r"reflect\CallableInfo")
            }
            (DeclarationQuery::Field, _) => return Err(TypeError::at(source, argument, "expected a field declaration")),
            (DeclarationQuery::Callable, _) => return Err(TypeError::at(source, argument, "expected a callable declaration")),
        };
        self.output.declaration_selections.insert(source.node_id(node_index), selection);
        // The query constructs fresh description data. Its backing must satisfy
        // the destination's lifetime. Unannotated locals share their evaluation
        // scope. Neither region is part of the declaration's identity.
        let carrier = omega_toolchain::core_interface().id(carrier);
        if let Some(ValueType::Applied(id, arguments)) = expected
            && id.as_ref() == carrier && arguments.len() == 1
        {
            return Ok(ValueType::Applied(id.clone(), arguments.clone()));
        }
        let storage = source.ancestors(node_index)
            .find(|&parent| matches!(source.nodes[parent].kind, NodeKind::Machine | NodeKind::Const))
            .unwrap_or(node_index);
        let region = ValueType::InferredLifetime { site: Box::new(site_at(source, storage)), index: 0 };
        Ok(ValueType::Applied(Box::new(carrier.clone()), vec![region]))
    }

    fn describe_field(&self, path: usize, field: &DeclarationId) -> Result<DeclarationSelection, TypeError> {
        let source = self.unit.file;
        let &(shape, position) = self.catalog.field_positions.get(field)
            .ok_or_else(|| TypeError::at(source, path, "the selected field has no typed declaration"))?;
        let case_owner = self.catalog.case_owner.get(shape).copied();
        let owner = case_owner.unwrap_or(shape);
        let carrier = self.unit.data_application(path, owner, &self.catalog.types)?;
        let carrier = self.catalog.types.normalize_at(carrier, self.unit, path).map_err(|message| TypeError::at(source, path, message))?;
        let args = match &carrier {
            ValueType::Nominal(id) if id.as_ref() == owner => &[][..],
            ValueType::Applied(id, args) if id.as_ref() == owner => args.as_slice(),
            _ => return Err(TypeError::at(source, path, "the selected field requires its declaring data application")),
        };
        let fields = self.catalog.fields_at(shape, source, path)?
            .ok_or_else(|| TypeError::at(source, path, "the selected field has no typed declaration"))?;
        let field_type = self.catalog.types.apply_type_arguments(&fields.ordered[position].ty, owner, args);
        Ok(DeclarationSelection::Field {
            owner: carrier, case: case_owner.map(|_| shape.clone()), field: field.clone(), position, field_type,
        })
    }

    fn describe_callable(&mut self, path: usize, machine: &DeclarationId) -> Result<DeclarationSelection, TypeError> {
        let source = self.unit.file;
        self.unit.check_signature_selection(path, machine)?;
        let types = &self.catalog.types;
        let mut enclosing_arguments = self.unit.receiver_arguments(path, self.catalog)?;
        let receiver = self.unit.qualifier(path);
        let requirement_owner = self.catalog.requirement_owners.get(machine).copied();
        if let Some(owner) = requirement_owner
            && !enclosing_arguments.iter().any(|(scope, _)| scope == owner)
        {
            if types.arity(owner) != 0 && !source.ancestors(path).any(|node| self.unit.declaration_at(node) == Some(owner)) {
                return Err(TypeError::at(source, path, "the selected requirement needs its declaring trait application"));
            }
            enclosing_arguments.push((owner.clone(), types.formals(owner)));
        }
        for &section in source.reference_sections(path) {
            let target = self.unit.application_name(section).map(|name| &name.target);
            let consumed_receiver = matches!(receiver, Some(Symbol::Declaration(id)) if matches!(id.kind, NodeKind::Trait | NodeKind::Conformance))
                && target == receiver;
            if !consumed_receiver && !matches!(target, Some(Symbol::Declaration(id)) if id == machine) {
                return Err(TypeError::at(source, section, "type arguments do not belong to the selected callable"));
            }
        }
        enclosing_arguments.extend(types.lexical_machine_arguments(machine));
        let explicit = self.unit.arguments_for(path, machine);
        let arguments = self.unit.checked_arguments_in_scope(path, machine, explicit, types, &enclosing_arguments)?;
        if !self.catalog.signatures.contains_key(machine) {
            if types.indices.declaration_is_pending(machine) {
                return Err(types.indices.wait(source, path));
            }
            return Err(TypeError::at(source, path, "the selected callable has no typed signature"));
        }
        self.validate_call_bindings(path, path, machine, explicit, &arguments, &enclosing_arguments)?;
        let application = MachineApplication { machine: Box::new(machine.clone()), arguments, enclosing_arguments };
        Ok(match requirement_owner {
            Some(owner) => DeclarationSelection::Requirement { owner: owner.clone(), application },
            None => DeclarationSelection::Machine(application),
        })
    }
}
