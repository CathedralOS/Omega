//! Mutable declaration and reach tables belong to source settlement. IDs and
//! completed semantic records remain in their representation vocabulary; these
//! interners and seeded language choices never enter native compilation.

use declaration_contracts::ExternalBindingId;
use declaration_contracts::ExternalBindingIdentity;
use declaration_contracts::SemanticDomainId;
use declaration_contracts::ServiceReachId;
use declaration_contracts::ServiceReachRowId;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ExternalBindingTable {
    identities: arena::RetainedRows<ExternalBindingIdentity>,
}

impl ExternalBindingTable {
    pub fn intern(&mut self, identity: ExternalBindingIdentity) -> ExternalBindingId {
        if let Some(index) = self
            .identities
            .iter()
            .position(|existing| existing == &identity)
        {
            return ExternalBindingId(index as u32 + 1);
        }
        self.identities.push(identity);
        ExternalBindingId(self.identities.len() as u32)
    }

    pub fn identity(&self, binding: ExternalBindingId) -> Option<&ExternalBindingIdentity> {
        let index = usize::try_from(binding.0).ok()?.checked_sub(1)?;
        self.identities.get(index)
    }

    pub fn identities(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExternalBindingId, &ExternalBindingIdentity)> {
        self.identities
            .iter()
            .enumerate()
            .map(|(index, identity)| (ExternalBindingId(index as u32 + 1), identity))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SemanticDomainTable {
    names: arena::RetainedRows<String>,
}

impl Default for SemanticDomainTable {
    fn default() -> Self {
        Self {
            names: vec![
                "Wrapping".to_owned(),
                "Saturating".to_owned(),
                "Trapping".to_owned(),
            ]
            .into(),
        }
    }
}

impl SemanticDomainTable {
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub const WRAPPING: SemanticDomainId = SemanticDomainId(1);
    pub const SATURATING: SemanticDomainId = SemanticDomainId(2);
    pub const TRAPPING: SemanticDomainId = SemanticDomainId(3);

    pub fn intern(&mut self, name: &str) -> SemanticDomainId {
        if let Some(position) = self.names.iter().position(|candidate| candidate == name) {
            return SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"));
        }
        self.names.push(name.to_owned());
        SemanticDomainId(u32::try_from(self.names.len()).expect("domain table fits u32"))
    }

    pub fn name(&self, id: SemanticDomainId) -> Option<&str> {
        id.0.checked_sub(1)
            .and_then(|index| self.names.get(index as usize))
            .map(String::as_str)
    }

    pub fn lookup(&self, name: &str) -> Option<SemanticDomainId> {
        self.names
            .iter()
            .position(|candidate| candidate == name)
            .map(|position| {
                SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub(crate) enum ServiceReachRow {
    Concrete(Vec<ServiceReachId>),
    AbstractBounded(ServiceReachRowId),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ServiceReachRowTable {
    pub(crate) rows: arena::RetainedRows<ServiceReachRow>,
}

impl ServiceReachRowTable {
    pub const EMPTY_ROW: ServiceReachRowId = ServiceReachRowId(1);

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn starts_with(&self, retained: &Self) -> bool {
        self.rows.starts_with(&retained.rows)
    }

    pub fn intern(&mut self, mut services: Vec<ServiceReachId>) -> ServiceReachRowId {
        services.sort_by_key(|service| service.0);
        services.dedup();
        if self.rows.is_empty() {
            self.rows.push(ServiceReachRow::Concrete(Vec::new()));
        }
        if let Some(position) = self
            .rows
            .iter()
            .position(|row| matches!(row, ServiceReachRow::Concrete(row) if *row == services))
        {
            return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
        }
        self.rows.push(ServiceReachRow::Concrete(services));
        ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
    }

    pub fn intern_abstract(&mut self, bound: ServiceReachRowId) -> ServiceReachRowId {
        if self.rows.is_empty() {
            self.rows.push(ServiceReachRow::Concrete(Vec::new()));
        }
        if let Some(position) = self.rows.iter().position(
            |row| matches!(row, ServiceReachRow::AbstractBounded(existing) if *existing == bound),
        ) {
            return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
        }
        self.rows.push(ServiceReachRow::AbstractBounded(bound));
        ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
    }

    pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
        match row
            .0
            .checked_sub(1)
            .and_then(|index| self.rows.get(index as usize))
        {
            Some(ServiceReachRow::Concrete(services)) => services.as_slice(),
            Some(ServiceReachRow::AbstractBounded(bound)) => self.services(*bound),
            None => &[],
        }
    }

    pub fn is_abstract(&self, row: ServiceReachRowId) -> bool {
        matches!(self.entry(row), Some(ServiceReachRow::AbstractBounded(_)))
    }

    pub fn abstract_bound(&self, row: ServiceReachRowId) -> Option<ServiceReachRowId> {
        match self.entry(row) {
            Some(ServiceReachRow::AbstractBounded(bound)) => Some(*bound),
            _ => None,
        }
    }

    fn entry(&self, row: ServiceReachRowId) -> Option<&ServiceReachRow> {
        row.0
            .checked_sub(1)
            .and_then(|index| self.rows.get(index as usize))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ServiceReachDefinition {
    pub symbol: symbols::SymbolHandle,
    pub name: String,
    pub parents: Vec<ServiceReachId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ServiceReachTable {
    definitions: arena::RetainedRows<ServiceReachDefinition>,
}

impl ServiceReachTable {
    pub fn intern(&mut self, symbol: symbols::SymbolHandle, name: &str) -> ServiceReachId {
        if let Some((index, _)) = self
            .definitions
            .iter()
            .enumerate()
            .find(|(_, definition)| definition.symbol == symbol)
        {
            return ServiceReachId(u32::try_from(index + 1).expect("service table fits u32"));
        }
        self.definitions.push(ServiceReachDefinition {
            symbol,
            name: name.to_owned(),
            parents: Vec::new(),
        });
        ServiceReachId(u32::try_from(self.definitions.len()).expect("service table fits u32"))
    }

    pub fn id_for_symbol(&self, symbol: symbols::SymbolHandle) -> Option<ServiceReachId> {
        self.definitions
            .iter()
            .position(|definition| definition.symbol == symbol)
            .map(|index| ServiceReachId(u32::try_from(index + 1).expect("service table fits u32")))
    }

    pub fn id_for_name(&self, name: &str) -> Option<ServiceReachId> {
        self.definitions
            .iter()
            .position(|definition| definition.name == name)
            .map(|index| ServiceReachId(u32::try_from(index + 1).expect("service table fits u32")))
    }

    pub fn definition(&self, id: ServiceReachId) -> Option<&ServiceReachDefinition> {
        id.0.checked_sub(1)
            .and_then(|index| self.definitions.get(index as usize))
    }

    pub fn definitions(&self) -> &arena::RetainedRows<ServiceReachDefinition> {
        &self.definitions
    }

    pub fn set_parents(&mut self, id: ServiceReachId, mut parents: Vec<ServiceReachId>) {
        parents.sort_by_key(|parent| parent.0);
        parents.dedup();
        if let Some(definition) =
            id.0.checked_sub(1)
                .and_then(|index| self.definitions.get_mut(index as usize))
        {
            definition.parents = parents;
        }
    }

    pub fn extend_closure(&self, service: ServiceReachId, services: &mut Vec<ServiceReachId>) {
        if services.contains(&service) {
            return;
        }
        services.push(service);
        if let Some(definition) = self.definition(service) {
            for parent in &definition.parents {
                self.extend_closure(*parent, services);
            }
        }
    }
}
#[cfg(test)]
#[path = "source_tables_tests.rs"]
mod tests;

#[cfg(test)]
mod retained_side_record_tests {
    use super::*;

    #[test]
    fn service_and_domain_successors_append_without_copying_retained_payloads() {
        let mut services = ServiceReachTable::default();
        let first = services.intern(symbols::SymbolHandle::from_parts(1, 7), "first");
        let mut rows = ServiceReachRowTable::default();
        let old_row = rows.intern(vec![first]);
        let mut domains = SemanticDomainTable::default();
        let old_domain = domains.intern("original");
        let (mut next_services, mut next_rows, mut next_domains) =
            (services.clone(), rows.clone(), domains.clone());
        let second = next_services.intern(symbols::SymbolHandle::from_parts(2, 11), "second");
        let next_row = next_rows.intern(vec![first, second]);
        next_domains.intern("new");
        assert!(std::ptr::eq(
            services.definition(first).unwrap(),
            next_services.definition(first).unwrap()
        ));
        assert!(std::ptr::eq(
            rows.services(old_row).as_ptr(),
            next_rows.services(old_row).as_ptr()
        ));
        assert!(std::ptr::eq(
            domains.name(old_domain).unwrap().as_ptr(),
            next_domains.name(old_domain).unwrap().as_ptr()
        ));
        assert!(services.definition(second).is_none());
        assert!(domains.lookup("new").is_none());
        assert_eq!(rows.services(old_row), &[first]);
        assert_eq!(next_rows.services(next_row), &[first, second]);
        next_services.set_parents(first, vec![second]);
        assert!(services.definition(first).unwrap().parents.is_empty());
        assert_eq!(
            next_services.definition(first).unwrap().parents,
            vec![second]
        );
        assert_eq!(services.definition(first).unwrap().symbol.generation(), 7);
    }
}
