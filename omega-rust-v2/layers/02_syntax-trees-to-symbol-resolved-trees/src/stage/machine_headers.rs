//! Bind a machine's qualifier once. A module qualifies its name without choosing an operator home.
use super::{Catalog, Context, Entry, NameError, NameRole, NameUse, OperatorDeclaration, Symbol, callable, qualify};
use crate::syntax::File;
use omega_parser::NodeKind;

impl Catalog<'_> {
    pub(super) fn machine_header(
        &self,
        file: &File<'_>,
        ctx: &Context<'_>,
        machine: &Entry,
        node: usize,
        uses: &mut Vec<NameUse>,
        visibility: &mut Vec<super::CarrierVisibility>,
    ) -> Result<Option<OperatorDeclaration>, NameError> {
        use NodeKind::*;
        let mut owner = file
            .ancestors(node)
            .find_map(|parent| ctx.entries.get(&parent))
            .map(|&i| &self.entries[i].id)
            .filter(|id| matches!(id.kind, Domain | Trait | Conformance))
            .map(|id| Symbol::Declaration(id.clone()));
        if let Some(head) = file.child(node, Path)
            && let Some((name, _)) = file.nodes[head].path().rsplit_once("::")
        {
            let span = file.span(head);
            let mut slots = self.lookup(file, ctx, head, name)?;
            let attachments = self.attached_domains(file, ctx, head, name, true)?;
            slots.extend(attachments.iter().map(|a| a.slot));
            slots.sort_unstable();
            slots.dedup();
            if slots.iter().any(|&i| self.entries[i].id.kind == Data) {
                return Err(file.error(span, "a data carrier cannot own a machine"));
            }
            if slots.iter().any(|&i| callable(self.entries[i].id.kind)) {
                return Err(file.error(span, format!("machine {name} cannot qualify a declaration outside its body")));
            }
            let module = (file.package.to_owned(), qualify(&file.namespace, name));
            let symbol = match slots.as_slice() {
                [] if self.modules.contains(&module) => Symbol::Module {
                    package: module.0,
                    path: module.1,
                },
                [] => return Err(file.error(span, format!("unresolved machine owner {name}"))),
                [slot] if matches!(self.entries[*slot].id.kind, Domain | Trait | Conformance) => Symbol::Declaration(self.entries[*slot].id.clone()),
                [_] => return Err(file.error(span, format!("{name} cannot qualify a machine declaration"))),
                _ => self
                    .occurrence_symbol(file, ctx, span, &slots, &attachments)
                    .map_err(|_| file.error(span, format!("ambiguous machine owner {name}")))?,
            };
            for slot in slots {
                self.require_carrier(file, slot, head, span, visibility)?;
            }
            let qualifier = file.nodes[head]
                .qualifier_span()
                .expect("qualified machine head");
            owner = (!matches!(symbol, Symbol::Module { .. })).then(|| symbol.clone());
            uses.push(super::usage(file, head, NameRole::Qualifier, qualifier, symbol));
        }
        Ok(file
            .child(node, OperatorToken)
            .map(|token| OperatorDeclaration {
                node: file.node_id(token),
                token: file.nodes[token].leaves.iter().map(|leaf| leaf.text).collect(),
                public: machine.public,
                machine: machine.id.clone(),
                declared_home: owner,
            }))
    }
}
