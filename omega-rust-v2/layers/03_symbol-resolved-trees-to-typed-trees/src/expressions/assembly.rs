use crate::expressions::body_typer::BodyTyper;
use crate::{Site, TypeError};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

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
        for &part in &source.nodes[node_index].children {
            let node = &source.nodes[part];
            match node.kind {
                Registers | Clobbers => {
                    for occurrence in self.unit.names(part) {
                        let Symbol::TargetRegister(register) = &occurrence.target else { continue };
                        let expected = target
                            .registers
                            .get(register)
                            .ok_or_else(|| TypeError::at(source, part, format!("missing register type for {register}")))?;
                        if node.kind == Registers {
                            let expr = self.unit.single_child(part)?;
                            let value = self.type_expression_with_expected_type(expr, Some(expected))?;
                            self.check_destination(expr, &value, expected)?;
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
                        actual.push(self.type_expression(operand)?);
                    }
                    let mut matching = variants
                        .iter()
                        .filter(|shape| shape.len() == actual.len() && shape.iter().zip(&actual).all(|(p, v)| p.admits(v)));
                    let Some(params) = matching.next() else {
                        return Err(TypeError::at(source, part, format!("no matching instruction operands for {mnemonic}")));
                    };
                    if matching.next().is_some() {
                        return Err(TypeError::at(source, part, format!("ambiguous instruction operands for {mnemonic}")));
                    }
                    for ((&operand, value), expected) in node.children.iter().zip(actual).zip(params) {
                        self.check_destination(operand, &value, expected)?;
                    }
                    self.output.instructions.push(crate::TypedInstruction {
                        node: source.node_id(part),
                        site: Site::at(source, part),
                        target: target.target.clone(),
                        mnemonic: mnemonic.clone(),
                        operands: params.clone(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}
