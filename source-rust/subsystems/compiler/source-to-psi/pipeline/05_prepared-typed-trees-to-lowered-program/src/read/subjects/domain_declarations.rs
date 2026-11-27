//! A transparent domain alias checks its constituent declarations, including
//! dependencies that the package check would otherwise reach only at a use.
use crate::read::refuse_at;

use super::{Program, judged_where_used};
use crate::source_state::domain::{
    DomainAliasConstituent, DomainDefinition, domain_accepts_underlying_carrier,
    index_argument_is_closed,
};
use crate::source_state::type_identity::ExactOwnerTypeIdentityRequest;
use crate::{Unserved, reject};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

#[derive(Clone, Copy)]
pub(super) enum DomainForm {
    Unread,
    // A failed traversal may leave ancestors open. Only an ancestor of the
    // current root is a cycle when another subject declaration is read.
    Visiting(SymbolHandle),
    Checked,
}

impl<'source> Program<'source> {
    pub(super) fn subject_domain(
        &mut self,
        id: MachineId,
        domain: &DomainDefinition,
        schemas: &[prepared_source::DomainSchema],
        forms: &mut [DomainForm],
    ) -> Checked<()> {
        // A stack keeps long chains of nested aliases off the host call stack.
        enum Visit<'a> {
            Read(&'a DomainDefinition),
            Finish(usize, SymbolHandle),
        }
        let typed = self.typed;
        let unsupported = |reason| refuse_at(id, Unserved::Declaration(reason));
        let mut pending = vec![Visit::Read(domain)];
        let root = domain.symbol;
        while let Some(visit) = pending.pop() {
            let domain = match visit {
                Visit::Finish(index, symbol) => {
                    forms[index] = DomainForm::Checked;
                    self.admitted_declarations.include(symbol);
                    continue;
                }
                Visit::Read(domain) => domain,
            };
            let index = typed
                .domain_position_by_symbol(domain.symbol)
                .ok_or_else(|| unsupported("domain declaration without its typed row"))?;
            if matches!(forms[index], DomainForm::Checked) {
                continue;
            }
            self.served(id, domain.symbol)?;
            if let Some(alias) = &domain.alias {
                let at = self.declared_at(domain.symbol);
                if matches!(forms[index], DomainForm::Visiting(owner) if owner == root) {
                    return Err(reject(
                        id,
                        at,
                        format!("domain alias cycle through `{}`", domain.name),
                    ));
                }
                if alias.constituents.is_empty() {
                    return Err(reject(
                        id,
                        at,
                        format!(
                            "domain alias `{}` must name at least one constituent",
                            domain.name
                        ),
                    ));
                }
                if !domain.type_parameters.is_empty()
                    || !domain.index_arguments.is_empty()
                    || domain.cleanup_selection.is_some()
                    || !domain.operators.is_empty()
                    || domain.predicate_presence.is_present()
                    || !domain.facts.is_empty()
                    || domain.classification.is_some()
                    || !domain.semantic_roles.is_empty()
                    || domain.access_policy.is_some()
                    || !domain.establishment_routes.is_empty()
                    || !domain.content_projections.is_empty()
                {
                    return Err(unsupported(
                        "domain alias with its own parameters or semantic obligations",
                    ));
                }
                self.value_type(id, domain.target_type)?;
                forms[index] = DomainForm::Visiting(root);
                pending.push(Visit::Finish(index, domain.symbol));
                for constituent in alias.constituents.iter().rev() {
                    let member = self.domain_alias_constituent(id, domain, constituent)?;
                    pending.push(Visit::Read(member));
                }
            } else {
                if !judged_where_used(typed, domain) {
                    return Err(unsupported(
                        "domain with a cleanup machine, operators or unsupported parameters",
                    ));
                }
                let schema = if domain.type_parameters.is_empty() {
                    None
                } else {
                    schemas
                        .binary_search_by_key(&index, |row| {
                            typed
                                .domain_position_by_symbol(row.declaration)
                                .unwrap_or(usize::MAX)
                        })
                        .ok()
                        .map(|position| &schemas[position])
                };
                self.declared_domain_schema(id, domain, schema)?;
                self.admitted_declarations.include(domain.symbol);
                forms[index] = DomainForm::Checked;
            }
        }
        Ok(())
    }

    fn domain_alias_constituent(
        &mut self,
        id: MachineId,
        domain: &DomainDefinition,
        constituent: &DomainAliasConstituent,
    ) -> Checked<&'source DomainDefinition> {
        let typed = self.typed;
        let at = self.declared_at(domain.symbol);
        let unsupported = |reason| refuse_at(id, Unserved::Declaration(reason));
        let Some(member) = typed.domain_definition(constituent.domain_symbol) else {
            let path = typed
                .domain_path_members(constituent.domain)
                .iter()
                .map(|part| part.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let mut refusal =
                unsupported("domain alias constituent without a resolved declaration");
            refusal
                .detail
                .push_str(&format!(": `{path}` in `{}`", domain.name));
            refusal.source_span = at.span();
            return Err(refusal);
        };
        if domain.is_public && !member.is_public {
            return Err(reject(
                id,
                at,
                format!(
                    "public domain alias `{}` names private domain `{}`",
                    domain.name, member.name
                ),
            ));
        }
        if !member.type_parameters.is_empty() || !member.index_arguments.is_empty() {
            let Some([applied_carrier]) =
                typed.domain_arguments_for_instance(member.symbol, constituent.semantic_domain)
            else {
                return Err(unsupported(
                    "domain alias constituent requiring a type or index application",
                ));
            };
            let alias_carrier = typed
                .type_reference_table
                .unconstrained_type_reference(domain.target_type);
            if !crate::source_state::domain::has_generic_carrier(typed, member)
                || !crate::source_state::domain::index_parameters(typed, member).is_empty()
                || typed.package_qualified_type_identity(*applied_carrier)
                    != typed.package_qualified_type_identity(alias_carrier)
            {
                return Err(unsupported(
                    "domain alias constituent without its declared carrier application",
                ));
            }
            let instance = crate::body::DomainInstance {
                declaration: member.symbol,
                identity: constituent.semantic_domain,
            };
            let theory = self.domain_theory(id, instance)?;
            let carrier = self.declaration_subject(id, theory)?;
            self.declared_domain(id, instance, carrier, at)?;
            self.declared_predicates(id, theory)?;
        }
        if !domain_accepts_underlying_carrier(typed, member, domain.target_type, &[]) {
            let known = [domain.target_type, member.target_type]
                .into_iter()
                .all(|reference| {
                    index_argument_is_closed(typed, reference)
                        && typed
                            .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                                type_reference: reference,
                                substitutions: &[],
                                lifetime_scope: None,
                                binders: &[],
                                exact_toolchain_sources: &[],
                            })
                            .is_some()
                });
            if !known {
                return Err(unsupported(
                    "domain alias with an unresolved constituent carrier",
                ));
            }
            return Err(reject(
                id,
                at,
                format!(
                    "domain alias `{}` and constituent `{}` have incompatible carriers",
                    domain.name, member.name
                ),
            ));
        }
        Ok(member)
    }
}
