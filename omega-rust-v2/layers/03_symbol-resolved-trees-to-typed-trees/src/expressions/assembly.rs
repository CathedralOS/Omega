use crate::TypeError;
use crate::expressions::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::site_at;

enum Actual {
    Value { ty: crate::ValueType, register: Option<String> },
    Block(omega_resolved::DeclarationId),
}

impl Actual {
    fn matches(
        &self,
        parameter: &crate::InstructionParameter<crate::ValueType>,
    ) -> bool {
        use crate::InstructionParameter::*;
        match (self, parameter) {
            (Self::Block(_), LocalBlock) => true,
            (Self::Value { ty, register }, Register(expected)) => register.is_some() && ty == expected,
            (Self::Value { ty, register }, Expression(expected)) => register.is_none() && ty.fits(expected),
            (Self::Value { ty, .. }, Scalar(expected)) => ty.fits(expected),
            _ => false,
        }
    }
}

/// Check a selected contract against its operands and the catalog. Returns the
/// catalog declarations of the locations the contract names directly.
fn validate(
    semantics: &crate::InstructionSemantics,
    operands: &[crate::TypedInstructionOperand],
    target: &crate::AssemblyTypes,
) -> Result<Vec<(String, crate::AssemblyLocation)>, &'static str> {
    use crate::{InstructionExit as Exit, InstructionInput as Input, InstructionValue as Meaning, LocationRole, TypedInstructionOperand as Operand, ValueType};
    let mut implicit = std::collections::BTreeMap::new();
    // A destination is a register operand or a declared location, with the role
    // that decides how a repeated write is reported.
    let mut resolve = |source: &Input| -> Result<(Option<(String, LocationRole)>, ValueType), &'static str> {
        match source {
            Input::Location(name) => {
                let location = target
                    .locations
                    .get(name)
                    .ok_or("instruction contract names an undeclared location")?;
                implicit.insert(name.clone(), location.clone());
                Ok((Some((name.clone(), location.role)), location.ty.clone()))
            }
            Input::Operand(index) => match operands.get(*index) {
                Some(Operand::Value { ty, .. }) => Ok((None, ty.clone())),
                Some(Operand::Register { name, ty, .. }) => Ok((Some((name.clone(), LocationRole::Register)), ty.clone())),
                _ => Err("instruction contract needs a scalar operand"),
            },
        }
    };
    let mut written = std::collections::HashSet::new();
    for write in &semantics.writes {
        let (destination, ty) = resolve(&write.destination)?;
        let (name, role) = destination.ok_or("instruction destination needs a register")?;
        if !written.insert(name) {
            return Err(match role {
                LocationRole::Register => "instruction writes the same register twice",
                LocationRole::State => "instruction writes the same state twice",
            });
        }
        let result = match &write.value {
            Meaning::Copy(input) => resolve(input)?.1,
            Meaning::IsZero(input) => {
                if !matches!(resolve(input)?.1, ValueType::Integer { width: Some(1..), .. }) {
                    return Err("zero test requires an integer operand");
                }
                ValueType::Bool
            }
            Meaning::Boolean(_) => ValueType::Bool,
        };
        if ty != result {
            return Err("instruction writes an incompatible register type");
        }
    }
    let jump = match &semantics.exit {
        Exit::Jump { target } => Some(*target),
        Exit::Branch { condition, target } => {
            if resolve(condition)?.1 != ValueType::Bool {
                return Err("conditional instruction requires a Boolean input");
            }
            Some(*target)
        }
        Exit::Return | Exit::Continue => None,
    };
    if jump.is_some_and(|i| !matches!(operands.get(i), Some(Operand::Block { .. })))
        || operands
            .iter()
            .enumerate()
            .any(|(i, operand)| matches!(operand, Operand::Block { .. }) && Some(i) != jump)
    {
        return Err("instruction control target disagrees with its local block operand");
    }
    Ok(implicit.into_iter().collect())
}

impl BodyTyper<'_, '_> {
    pub(super) fn assembly(
        &mut self,
        node_index: usize,
    ) -> Result<(), TypeError> {
        use NodeKind::*;
        let source = self.unit.file;
        let target = self
            .catalog
            .assembly
            .ok_or_else(|| TypeError::at(source, node_index, "assembly typing requires an admitted target"))?;
        target
            .target
            .validate()
            .map_err(|err| TypeError::at(source, node_index, err.to_string()))?;
        // A local-block operand names a continuation of this region: an assembly
        // machine nested in the outermost enclosing assembly machine. That root is
        // a separately callable entry, and an inline region owns no blocks.
        let root = std::iter::once(node_index)
            .chain(source.ancestors(node_index))
            .take_while(|&n| source.nodes[n].kind == AsmMachine)
            .last();
        let owns_block = |block: &omega_resolved::DeclarationId| {
            root.is_some_and(|root| {
                (root + 1..source.nodes[root].end).any(|n| source.nodes[n].kind == AsmMachine && self.unit.declaration_at(n) == Some(block))
            })
        };
        let mut region = omega_typed::TypedAssemblyRegion {
            target: target.target.clone(),
            bindings: Vec::new(),
            clobbers: None,
        };
        for &part in &source.nodes[node_index].children {
            let node = &source.nodes[part];
            match node.kind {
                Registers | Clobbers => {
                    if node.kind == Clobbers {
                        region.clobbers.get_or_insert_with(Vec::new);
                    }
                    for occurrence in self.unit.names(part) {
                        let Symbol::TargetRegister(register) = &occurrence.target else { continue };
                        let location = target
                            .register(register)
                            .ok_or_else(|| TypeError::at(source, part, format!("missing register type for {register}")))?;
                        let expected = &location.ty;
                        if node.kind == Registers {
                            let expr = self.unit.single_child(part)?;
                            let value = self.type_expression_with_expected_type(expr, Some(expected))?;
                            self.check_destination(expr, &value, expected)?;
                            region
                                .bindings
                                .push((source.node_id(expr), register.clone(), location.clone()));
                        } else if let Some(names) = &mut region.clobbers {
                            names.push(register.clone());
                        }
                    }
                }
                Instruction => {
                    let Some(Symbol::TargetInstruction(mnemonic)) = self.unit.symbol(part) else {
                        return Err(TypeError::at(source, part, "missing resolved instruction identity"));
                    };
                    let variants = target
                        .instructions
                        .get(mnemonic)
                        .ok_or_else(|| TypeError::at(source, part, format!("no operand contract for {mnemonic}")))?;
                    let mut actual = Vec::new();
                    for &operand in &node.children {
                        let block = self
                            .unit
                            .symbol(operand)
                            .and_then(|symbol| match symbol.declaration_candidates() {
                                [id] if id.kind == AsmMachine => Some(id.clone()),
                                _ => None,
                            });
                        actual.push(if let Some(block) = block {
                            if !owns_block(&block) {
                                return Err(TypeError::at(source, operand, "instruction target lies outside this assembly region"));
                            }
                            Actual::Block(block)
                        } else {
                            let register = match self.unit.symbol(source.ungroup(operand)) {
                                Some(Symbol::TargetRegister(name)) => Some(name.clone()),
                                _ => None,
                            };
                            Actual::Value {
                                ty: self.type_expression(operand)?,
                                register,
                            }
                        });
                    }
                    let mut matching = variants.iter().filter(|variant| {
                        variant.parameters.len() == actual.len()
                            && variant
                                .parameters
                                .iter()
                                .zip(&actual)
                                .all(|(param, arg)| arg.matches(param))
                    });
                    let Some(variant) = matching.next() else {
                        return Err(TypeError::at(source, part, format!("no matching instruction operands for {mnemonic}")));
                    };
                    if matching.next().is_some() {
                        return Err(TypeError::at(source, part, format!("ambiguous instruction operands for {mnemonic}")));
                    }
                    let mut operands = Vec::new();
                    for ((&operand, value), param) in node.children.iter().zip(actual).zip(&variant.parameters) {
                        let id = source.node_id(operand);
                        operands.push(match (value, param) {
                            (Actual::Block(declaration), crate::InstructionParameter::LocalBlock) => {
                                crate::TypedInstructionOperand::Block { node: id, declaration }
                            }
                            (
                                Actual::Value { ty: value, register },
                                crate::InstructionParameter::Scalar(ty)
                                | crate::InstructionParameter::Register(ty)
                                | crate::InstructionParameter::Expression(ty),
                            ) => {
                                self.check_destination(operand, &value, ty)?;
                                if let Some(name) = register {
                                    let lifetime = target
                                        .register(&name)
                                        .ok_or_else(|| TypeError::at(source, operand, format!("missing register type for {name}")))?
                                        .lifetime;
                                    crate::TypedInstructionOperand::Register {
                                        node: id,
                                        name,
                                        ty: ty.clone(),
                                        lifetime,
                                    }
                                } else {
                                    crate::TypedInstructionOperand::Value { node: id, ty: ty.clone() }
                                }
                            }
                            _ => unreachable!("matching variant selected above"),
                        });
                    }
                    let implicit_locations = validate(&variant.semantics, &operands, target).map_err(|message| TypeError::at(source, part, message))?;
                    self.output.instructions.push(crate::TypedInstruction {
                        node: source.node_id(part),
                        site: site_at(source, part),
                        target: target.target.clone(),
                        mnemonic: mnemonic.clone(),
                        operands,
                        implicit_locations,
                        semantics: variant.semantics.clone(),
                    });
                }
                _ => {}
            }
        }
        if let Some(names) = &mut region.clobbers {
            names.sort_unstable();
            names.dedup();
        }
        self.output
            .assembly_regions
            .insert(source.node_id(node_index), region);
        Ok(())
    }
}
