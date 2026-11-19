use super::{Catalog, Context, NameError, NameUse, OperatorDeclaration, OperatorUse, Symbol};
use crate::syntax::File;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;

impl Catalog<'_> {
    pub(super) fn collect_operators(
        &self,
        file: &File<'_>,
        file_number: usize,
        ctx: &Context<'_>,
        uses: &[NameUse],
        operators: &mut Vec<OperatorUse>,
        declarations: &mut Vec<OperatorDeclaration>,
    ) -> Result<(), NameError> {
        use NodeKind::*;
        for (n, node) in file.nodes.iter().enumerate() {
            if matches!(node.kind, Binary | Unary | Index | Range) {
                let (Some(first), Some(last)) = (node.leaves.first(), node.leaves.last()) else {
                    continue;
                };
                let token = match node.kind {
                    Index => "[]".into(),
                    Range => node
                        .leaves
                        .iter()
                        .find(|t| t.text.starts_with(".."))
                        .map_or("[..]", |t| t.text)
                        .into(),
                    _ => node.leaves.iter().map(|t| t.text).collect(),
                };
                operators.push(OperatorUse {
                    node: file.node_id(n),
                    package: file.package.into(),
                    file: file.path.into(),
                    span: SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()),
                    token,
                });
            }
            if node.kind != OperatorToken {
                continue;
            }
            let machine = node
                .parent
                .and_then(|p| ctx.entries.get(&p))
                .map(|&i| &self.entries[i])
                .ok_or_else(|| file.error(file.span(n), "operator requires a machine"))?;
            let machine_node = node.parent.expect("operator machine");
            let owner = file
                .enclosing_contexts(machine_node)
                .find_map(|a| ctx.entries.get(&a))
                .map(|&i| &self.entries[i].id);
            if owner.is_some_and(|id| id.kind == Conformance) {
                return Err(file.error(file.span(n), "a conformance supplies the trait's token and cannot bind one"));
            }
            let mut homes = Vec::new();
            if let Some(trait_id) = owner.filter(|id| id.kind == Trait) {
                homes.push(trait_id.clone());
            } else if let Some(params) = file.child(machine_node, Params) {
                let param_span = file.span(params);
                for usage in uses
                    .iter()
                    .filter(|u| u.span.get_start_offset() >= param_span.get_start_offset() && u.span.get_end_offset() <= param_span.get_end_offset())
                {
                    if let Symbol::Declaration(id) = &usage.target
                        && id.kind == Domain
                        && !homes.contains(id)
                    {
                        homes.push(id.clone());
                    }
                }
            }
            if let Some(path) = file.child(machine_node, Path)
                && let Some((qualifier, _)) = file.nodes[path].path().rsplit_once("::")
            {
                let mut domains = self.lookup(file, file_number, ctx, n, qualifier);
                domains.extend(
                    ctx.exposed_domains
                        .keys()
                        .copied()
                        .filter(|&i| self.entries[i].id.path.rsplit("::").next() == Some(qualifier)),
                );
                for &slot in &domains {
                    let id = &self.entries[slot].id;
                    if id.kind == Domain && !homes.contains(id) {
                        homes.push(id.clone());
                    }
                }
                if homes
                    .iter()
                    .any(|id| id.path.rsplit("::").next() == Some(qualifier))
                {
                    homes.retain(|id| id.path.rsplit("::").next() == Some(qualifier));
                }
            }
            if homes.len() > 1 {
                return Err(file.error(file.span(n), "operator declaration has multiple possible semantic homes"));
            }
            if homes.first().is_some_and(|home| home.package != file.package) {
                return Err(file.error(file.span(n), "only the declaring package may bind an operator for this domain"));
            }
            let token: String = node.leaves.iter().map(|t| t.text).collect();
            if declarations.iter().any(|other| {
                other.token == token
                    && other.home.as_ref() == homes.first()
                    && other.machine.package == machine.id.package
                    && other.machine.operands == machine.id.operands
                    && other.machine.signature_owners == machine.id.signature_owners
                    && other.machine.enclosing_machines == machine.id.enclosing_machines
            }) {
                return Err(file.error(file.span(n), "duplicate operator binding for the same operand shape"));
            }
            declarations.push(OperatorDeclaration {
                token,
                machine: machine.id.clone(),
                home: homes.pop(),
            });
        }
        Ok(())
    }
}
