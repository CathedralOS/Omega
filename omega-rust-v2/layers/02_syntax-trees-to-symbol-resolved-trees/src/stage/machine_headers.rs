//! Bind a machine's qualifier once. A module qualifies its name without choosing an operator home.
use super::{Catalog, Context, Entry, NameError, NameErrorKind, NameRole, NameUse, OperatorDeclaration, Symbol, callable};
use crate::syntax::File;
use omega_syntax::NodeKind;

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
        let head = file
            .child(node, Path)
            .map(|head| (head, file.nodes[head].path()));
        if let Some((head, name)) = &head
            && let Some((module, _)) = name.rsplit_once('\\')
            && !name.contains("::")
        {
            let head = *head;
            let path = super::module_child(&file.namespace, module);
            if !self.modules.contains(&(file.package.into(), path.clone())) {
                return Err(file.error(NameErrorKind::UnresolvedModuleOwner, file.span(head), format!("unresolved module owner {module}")));
            }
            uses.push(super::usage(
                file,
                head,
                NameRole::Qualifier,
                file.nodes[head].qualifier_span().expect("module prefix"),
                Symbol::Module {
                    package: file.package.into(),
                    path,
                },
            ));
        }
        if let Some((head, path)) = head
            && let Some((name, _)) = path.rsplit_once("::")
        {
            let span = file.span(head);
            let mut slots = self.lookup(file, ctx, head, name)?;
            let attachments = self.attached_domains(file, ctx, head, name, true)?;
            slots.extend(attachments.iter().map(|a| a.slot));
            slots.sort_unstable();
            slots.dedup();
            if slots.iter().any(|&i| self.entries[i].id.kind == Data) {
                return Err(file.error(NameErrorKind::IneligibleMachineOwner, span, "a data carrier cannot own a machine"));
            }
            if slots.iter().any(|&i| callable(self.entries[i].id.kind)) {
                return Err(file.error(
                    NameErrorKind::IneligibleMachineOwner,
                    span,
                    format!("machine {name} cannot qualify a declaration outside its body"),
                ));
            }
            let qualifier = file.nodes[head].qualifier_span().expect("qualified machine head");
            let symbol = match slots.as_slice() {
                [] => return Err(file.error(NameErrorKind::UnresolvedMachineOwner, span, format!("unresolved machine owner {name}"))),
                // A single family match is unambiguous, but its carrier may still be wrong.
                [_] if attachments.iter().any(|a| a.carrier_deferred) => self.domain_candidates(&slots, &attachments, String::new()),
                [slot] if matches!(self.entries[*slot].id.kind, Domain | Trait | Conformance) => Symbol::Declaration(self.entries[*slot].id.clone()),
                [_] => return Err(file.error(NameErrorKind::IneligibleMachineOwner, span, format!("{name} cannot qualify a machine declaration"))),
                _ => self.occurrence_symbol(file, ctx, qualifier, &slots, &attachments, None).map_err(|cause| {
                    file.error(NameErrorKind::AmbiguousMachineOwner, qualifier, format!("ambiguous machine owner {name}: {}", cause.message))
                })?,
            };
            for slot in slots {
                self.require_carrier(file, slot, head, span, visibility)?;
            }
            owner = Some(symbol.clone());
            uses.push(super::usage(file, head, NameRole::Qualifier, qualifier, symbol));
        }
        Ok(file
            .child(node, OperatorToken)
            .map(|token| OperatorDeclaration {
                node: file.node_id(token),
                token: file.nodes[token].leaves.iter().map(|leaf| leaf.text).collect(),
                public: machine.public && machine.lexical_owner.is_none(),
                machine: machine.id.clone(),
                declared_home: owner,
            }))
    }
}
