#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DeclarationId<K: Ord> {
    pub kind: K,
    pub name: std::sync::Arc<DeclarationName<K>>,
}

impl<K: Ord> Ord for DeclarationId<K> {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| self.kind.cmp(&other.kind))
    }
}

impl<K: Ord> PartialOrd for DeclarationId<K> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The resolver shares one qualified name and overload identity across all its occurrences.
/// Independent package interfaces compare by these contents.
#[derive(Clone, Debug)]
pub struct DeclarationName<K: Ord> {
    pub package: String,
    pub path: String,
    pub operands: Vec<String>,
    pub result_domains: Vec<String>,
    pub signature_owners: Vec<(String, String)>,
    /// Source shapes with bound names substituted, before typing or evaluation.
    pub resolved_signature: Option<ResolvedSignature>,
    pub enclosing_machine: Option<DeclarationId<K>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResolvedSignature {
    pub operands: Vec<String>,
    pub result_domains: Vec<String>,
}

type AuthoredSignature<'a> = (&'a [String], &'a [String], &'a [(String, String)]);
type DeclarationKey<'a, K> = (
    &'a str,
    &'a str,
    &'a Option<DeclarationId<K>>,
    &'a Option<ResolvedSignature>,
    Option<AuthoredSignature<'a>>,
);

impl<K: Ord> DeclarationName<K> {
    fn identity(&self) -> DeclarationKey<'_, K> {
        (
            &self.package,
            &self.path,
            &self.enclosing_machine,
            &self.resolved_signature,
            self.resolved_signature
                .is_none()
                .then_some((self.operands.as_slice(), self.result_domains.as_slice(), self.signature_owners.as_slice())),
        )
    }
}

impl<K: Ord> PartialEq for DeclarationName<K> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.identity() == other.identity()
    }
}

impl<K: Ord> Eq for DeclarationName<K> {}

impl<K: Ord> Ord for DeclarationName<K> {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.identity().cmp(&other.identity())
    }
}

impl<K: Ord> PartialOrd for DeclarationName<K> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ord + std::hash::Hash> std::hash::Hash for DeclarationName<K> {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        std::hash::Hash::hash(&self.identity(), state);
    }
}

impl<K: Ord> std::ops::Deref for DeclarationId<K> {
    type Target = DeclarationName<K>;
    fn deref(&self) -> &Self::Target {
        &self.name
    }
}

impl<K: Ord> DeclarationId<K> {
    /// Preserve the complete overload and enclosing-machine identity while projecting its declaration kind.
    pub fn try_map_kind<T: Ord>(
        &self,
        map: &impl Fn(&K) -> Option<T>,
    ) -> Option<DeclarationId<T>> {
        let kind = map(&self.kind)?;
        let enclosing_machine = match &self.enclosing_machine {
            Some(owner) => Some(owner.try_map_kind(map)?),
            None => None,
        };
        Some(DeclarationId {
            kind,
            name: std::sync::Arc::new(DeclarationName {
                package: self.package.clone(),
                path: self.path.clone(),
                operands: self.operands.clone(),
                result_domains: self.result_domains.clone(),
                signature_owners: self.signature_owners.clone(),
                resolved_signature: self.resolved_signature.clone(),
                enclosing_machine,
            }),
        })
    }
}
