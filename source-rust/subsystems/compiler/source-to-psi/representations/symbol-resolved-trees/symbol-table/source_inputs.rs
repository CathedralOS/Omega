//! Source settlement owns activation, visibility and the mutable source catalogue.
use source_admission::SourceFile;
use source_admission::SourceOrigin;
use source_admission::SourceResolutionStratum;
use source_provenance::DependencyScope;
use source_provenance::PackageKeyIdentity;
use source_text::{SourceId, SourceSpan};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Permit private source selections within one captured Build invocation.
/// This consent assigns no declaration owner, package edge or execution effect.
/// The receiving execution admission still checks the emitted program.
pub trait InvocationSourceConsent: Send + Sync {
    fn allows_selection(
        &self,
        sources: &Arc<SourceMap>,
        requester: SourceSpan,
        declaration: SourceSpan,
        purpose: DependencyScope,
    ) -> bool;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SourceMap {
    files: Vec<SourceFile>,
    selection: SourceSelection,
    source_rejoins: Vec<(SourceId, SourceId)>, // Producer-issued correspondence when an anonymous Build capture becomes a Product instance.
    product_dependencies: BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>, // Authorized `DependencyPurpose::Product` alias edges by requester.
    query_dependencies: Option<(PathBuf, BTreeMap<String, PackageKeyIdentity>)>, // Invocation-local routing before Build has a final declaration owner.
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
enum SourceSelection {
    #[default]
    All,
    Explicit(Vec<bool>),
}

#[derive(Debug, Default, record_storage::Record)]
pub struct SourceCatalogue {
    backing: SourceMap,
}

impl SourceCatalogue {
    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn is_empty(&self) -> bool {
        self.backing.is_empty()
    }

    pub fn fork(&self) -> Self {
        Self {
            backing: self.backing.clone(),
        }
    }

    pub fn begin_selection(&self) -> SourceMap {
        SourceMap {
            files: self.backing.files.clone(),
            selection: SourceSelection::Explicit(vec![false; self.backing.files.len()]),
            source_rejoins: Vec::new(),
            product_dependencies: BTreeMap::new(),
            query_dependencies: None,
        }
    }

    pub fn admit_checked_instance<'selection>(
        &mut self,
        selection: &'selection mut SourceMap,
        path: PathBuf,
        source: impl Into<Arc<str>>,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        resolution_stratum: SourceResolutionStratum,
        dependency_scope: DependencyScope,
    ) -> Result<&'selection SourceFile, String> {
        if !self.backing.retains_source_storage(selection) {
            return Err(
                "source selection does not belong to this catalogue's coordinate prefix".into(),
            );
        }
        resolution_stratum.directory_namespace(&path, &package_root)?;
        let source = source.into();
        if resolution_stratum != SourceResolutionStratum::CurrentActivationExtension
            && !(origin == SourceOrigin::User && package_identity.is_none())
        {
            let mut selected = selection
                .selected_files()
                .filter(|file| {
                    file.path == path
                        && file.source == source
                        && file.package_root == package_root
                        && file.package_identity == package_identity
                        && file.origin == origin
                        && file.resolution_stratum == resolution_stratum
                        && file.dependency_scope == dependency_scope
                })
                .map(|file| file.source_id);
            let existing = selected.next();
            if selected.next().is_some() {
                return Err(
                    "source frontier selected more than one exact dependency occurrence".into(),
                );
            }
            if let Some(existing) = existing {
                return Ok(&selection.files[existing.0]);
            }
        }
        let admitted = if resolution_stratum == SourceResolutionStratum::CurrentActivationExtension
            || (origin == SourceOrigin::User && package_identity.is_none())
        {
            self.backing.add_checked_instance(
                path,
                source,
                package_root,
                package_identity,
                origin,
                resolution_stratum,
                dependency_scope,
            )
        } else {
            self.backing.select_checked_instance(
                path,
                source,
                package_root,
                package_identity,
                origin,
                resolution_stratum,
                dependency_scope,
            )
        }
        .source_id;
        Ok(self.activate_instance(selection, admitted))
    }

    pub fn admit_source_occurrence<'selection>(
        &mut self,
        selection: &'selection mut SourceMap,
        occurrence: &SourceFile,
    ) -> Result<&'selection SourceFile, String> {
        if !self.backing.retains_source_storage(selection) {
            return Err(
                "source occurrence does not belong to this catalogue's coordinate prefix".into(),
            );
        }
        let admitted = self
            .backing
            .add_checked_instance(
                occurrence.path.clone(),
                Arc::clone(&occurrence.source),
                occurrence.package_root.clone(),
                occurrence.package_identity,
                occurrence.origin,
                occurrence.resolution_stratum,
                occurrence.dependency_scope,
            )
            .source_id;
        Ok(self.activate_instance(selection, admitted))
    }

    fn activate_instance<'selection>(
        &self,
        selection: &'selection mut SourceMap,
        admitted: SourceId,
    ) -> &'selection SourceFile {
        let previous_length = selection.files.len();
        if matches!(selection.selection, SourceSelection::All) {
            selection.selection = SourceSelection::Explicit(vec![true; previous_length]);
        }
        selection
            .files
            .extend_from_slice(&self.backing.files[previous_length..]);
        if let SourceSelection::Explicit(selected) = &mut selection.selection {
            selected.resize(self.backing.files.len(), false);
            selected[admitted.0] = true;
        }
        &selection.files[admitted.0]
    }
}

impl SourceSelection {
    fn includes(&self, source: SourceId) -> bool {
        match self {
            Self::All => true,
            Self::Explicit(selected) => selected.get(source.0).copied().unwrap_or(false),
        }
    }
}

impl SourceMap {
    pub fn from_files(files: Vec<SourceFile>) -> Self {
        Self {
            files,
            selection: SourceSelection::All,
            source_rejoins: Vec::new(),
            product_dependencies: BTreeMap::new(),
            query_dependencies: None,
        }
    }

    pub fn source_is_selected(&self, source: SourceId) -> bool {
        source.0 < self.files.len() && self.selection.includes(source)
    }

    pub fn retain_source_rejoin(
        &mut self,
        original: SourceId,
        selected: SourceId,
    ) -> Result<(), String> {
        if original == selected {
            return Ok(());
        }
        let before = self
            .get(original)
            .ok_or("source rejoin lost its original file")?;
        let after = self
            .get(selected)
            .ok_or("source rejoin lost its current file")?;
        if !self.source_is_selected(selected)
            || before.origin != SourceOrigin::User
            || (before.package_identity.is_some()
                && before.package_identity != after.package_identity)
            || before.resolution_stratum == SourceResolutionStratum::CurrentActivationExtension
            || before.source != after.source
            || before.path != after.path
            || before.package_root != after.package_root
            || before.origin != after.origin
            || before.resolution_stratum != after.resolution_stratum
            || before.dependency_scope != after.dependency_scope
        {
            return Err("source rejoin does not preserve its admitted captured instance".into());
        }
        if let Some((_, previous)) = self
            .source_rejoins
            .iter()
            .find(|(source, _)| *source == original)
        {
            if *previous != selected {
                return Err("source rejoin has more than one current destination".into());
            }
        } else {
            self.source_rejoins.push((original, selected));
        }
        Ok(())
    }

    pub fn source_rejoins(&self) -> impl Iterator<Item = (SourceId, SourceId)> + '_ {
        self.source_rejoins.iter().copied()
    }

    pub fn rejoined_source_id(&self, original: &SourceFile) -> Option<SourceId> {
        if self.get(original.source_id)? != original {
            return None;
        }
        if self.source_is_selected(original.source_id) {
            return Some(original.source_id);
        }
        self.source_rejoins.iter().find_map(|(source, selected)| {
            (*source == original.source_id && self.source_is_selected(*selected))
                .then_some(*selected)
        })
    }

    pub fn retain_selected_sources(&mut self, mut keep: impl FnMut(&SourceFile) -> bool) {
        self.selection = SourceSelection::Explicit(
            self.files
                .iter()
                .map(|file| self.source_is_selected(file.source_id) && keep(file))
                .collect(),
        );
    }

    pub fn same_product_dependency_scope(
        &self,
        other: &Self,
        requester: PackageKeyIdentity,
    ) -> bool {
        self.product_dependencies.get(&requester) == other.product_dependencies.get(&requester)
    }

    pub fn retains_source_storage(&self, predecessor: &Self) -> bool {
        self.files.starts_with(&predecessor.files)
    }

    pub fn select_checked_instance(
        &mut self,
        path: PathBuf,
        source: impl Into<Arc<str>>,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        resolution_stratum: SourceResolutionStratum,
        dependency_scope: DependencyScope,
    ) -> &SourceFile {
        let source = source.into();
        let retained = self.files.iter().rposition(|file| {
            file.path == path
                && file.source == source
                && file.package_root == package_root
                && file.package_identity == package_identity
                && file.origin == origin
                && file.resolution_stratum == resolution_stratum
                && file.dependency_scope == dependency_scope
        });
        if let Some(position) = retained {
            if let SourceSelection::Explicit(selected) = &mut self.selection {
                selected[position] = true;
            }
            return &self.files[position];
        }
        self.add_checked_instance(
            path,
            source,
            package_root,
            package_identity,
            origin,
            resolution_stratum,
            dependency_scope,
        )
    }

    pub fn bind_anonymous_owner(
        &mut self,
        root: &std::path::Path,
        identity: PackageKeyIdentity,
    ) -> Result<(), String> {
        for file in self.selected_files() {
            if file.origin == SourceOrigin::User
                && file.package_identity.is_none()
                && file.package_root != root
            {
                return Err(format!(
                    "source {} has no supplied owner outside the anonymous capture",
                    file.path.display()
                ));
            }
        }
        let selection = &self.selection;
        for file in &mut self.files {
            if !selection.includes(file.source_id) {
                continue;
            }
            if file.origin == SourceOrigin::User && file.package_identity.is_none() {
                file.package_identity = Some(identity);
            }
        }
        Ok(())
    }

    pub fn retain_product_dependency_scope<'a>(
        &mut self,
        edges: impl IntoIterator<Item = (PackageKeyIdentity, &'a str, PackageKeyIdentity)>,
    ) {
        for (requester, alias, target) in edges {
            self.product_dependencies
                .entry(requester)
                .or_default()
                .insert(alias.to_owned(), target);
        }
    }

    pub fn product_dependency_target(
        &self,
        requester: PackageKeyIdentity,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.product_dependencies
            .get(&requester)?
            .get(alias)
            .copied()
    }

    pub fn retain_query_dependencies(
        &mut self,
        root: PathBuf,
        dependencies: impl IntoIterator<Item = (String, PackageKeyIdentity)>,
    ) -> Result<(), String> {
        if self.query_dependencies.is_some() {
            return Err("Product query source scope was already supplied".into());
        }
        let mut edges = BTreeMap::new();
        for (alias, target) in dependencies {
            if edges.insert(alias, target).is_some() {
                return Err("Product query dependency alias is repeated".into());
            }
        }
        self.query_dependencies = Some((root, edges));
        Ok(())
    }

    pub fn product_dependency_at(
        &self,
        occurrence: SourceSpan,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        let file = self.file_at(occurrence)?;
        if let Some(owner) = file.package_identity {
            return self.product_dependency_target(owner, alias);
        }
        let (root, dependencies) = self.query_dependencies.as_ref()?;
        (file.origin == SourceOrigin::User && file.package_root == *root)
            .then(|| dependencies.get(alias).copied())
            .flatten()
    }

    pub fn add(&mut self, path: PathBuf, source: String) -> &SourceFile {
        let package_root = path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        self.add_with_metadata(path, source, package_root, None, SourceOrigin::User)
    }

    pub fn add_with_metadata(
        &mut self,
        path: PathBuf,
        source: String,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
    ) -> &SourceFile {
        self.add_with_metadata_and_resolution_stratum(
            path,
            source,
            package_root,
            package_identity,
            origin,
            SourceResolutionStratum::Base,
        )
    }

    pub fn add_with_metadata_and_resolution_stratum(
        &mut self,
        path: PathBuf,
        source: String,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        resolution_stratum: SourceResolutionStratum,
    ) -> &SourceFile {
        self.add_checked_instance(
            path,
            source,
            package_root,
            package_identity,
            origin,
            resolution_stratum,
            DependencyScope::Product,
        )
    }

    pub fn add_checked_instance(
        &mut self,
        path: PathBuf,
        source: impl Into<Arc<str>>,
        package_root: PathBuf,
        package_identity: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        resolution_stratum: SourceResolutionStratum,
        dependency_scope: DependencyScope,
    ) -> &SourceFile {
        self.files.push(SourceFile {
            source_id: SourceId(self.files.len()),
            path,
            package_root,
            package_identity,
            dependency_scope,
            origin,
            resolution_stratum,
            source: source.into(),
        });
        if let SourceSelection::Explicit(selected) = &mut self.selection {
            selected.push(true);
        }

        self.files
            .last()
            .expect("source map should contain added file")
    }

    pub fn get(&self, source_id: SourceId) -> Option<&SourceFile> {
        self.files.get(source_id.0)
    }

    pub fn file_at(&self, source_span: SourceSpan) -> Option<&SourceFile> {
        self.get(source_span.source_id)
    }

    pub fn same_package(&self, left: SourceSpan, right: SourceSpan) -> bool {
        match (self.file_at(left), self.file_at(right)) {
            (Some(left), Some(right)) => same_package_files(left, right),
            _ => false,
        }
    }

    pub fn same_checked_instance(&self, left: SourceSpan, right: SourceSpan) -> bool {
        match (self.file_at(left), self.file_at(right)) {
            (Some(left), Some(right)) => {
                left.dependency_scope == right.dependency_scope
                    && match (left.package_identity, right.package_identity) {
                        (Some(left), Some(right)) => left == right,
                        (None, None) => {
                            left.origin == right.origin && left.package_root == right.package_root
                        }
                        _ => false,
                    }
            }
            _ => false,
        }
    }

    pub fn reference_can_see_declaration(
        &self,
        reference: SourceSpan,
        declaration: SourceSpan,
    ) -> bool {
        if let SourceSelection::Explicit(_) = &self.selection
            && (!self.source_is_selected(declaration.source_id)
                || (reference.span.start != reference.span.end
                    && !self.source_is_selected(reference.source_id)))
        {
            return false;
        }
        if reference.span.start == reference.span.end {
            return true;
        }
        let Some(reference) = self.file_at(reference) else {
            return true;
        };
        let Some(declaration) = self.file_at(declaration) else {
            return true;
        };
        reference.dependency_scope == declaration.dependency_scope
            && self.reference_can_see_stratum_at(reference, declaration)
    }

    pub fn reference_can_see_stratum(
        &self,
        reference: SourceSpan,
        declaration: SourceSpan,
    ) -> bool {
        if reference.span.start == reference.span.end {
            return true;
        }
        match (self.file_at(reference), self.file_at(declaration)) {
            (Some(reference), Some(declaration)) => {
                self.reference_can_see_stratum_at(reference, declaration)
            }
            _ => true,
        }
    }

    fn reference_can_see_stratum_at(
        &self,
        reference: &SourceFile,
        declaration: &SourceFile,
    ) -> bool {
        match declaration.resolution_stratum {
            SourceResolutionStratum::Base => true,
            SourceResolutionStratum::CurrentActivationExtension => {
                reference.resolution_stratum == SourceResolutionStratum::CurrentActivationExtension
            }
            SourceResolutionStratum::DependencyActivationExtension => {
                reference.resolution_stratum != SourceResolutionStratum::Base
                    || !same_package_files(reference, declaration)
            }
        }
    }

    pub fn captured_reference_can_see_stratum(
        package: PackageKeyIdentity,
        stratum: SourceResolutionStratum,
        declaration: &SourceFile,
    ) -> bool {
        match declaration.resolution_stratum {
            SourceResolutionStratum::Base => true,
            SourceResolutionStratum::CurrentActivationExtension => {
                stratum == SourceResolutionStratum::CurrentActivationExtension
            }
            SourceResolutionStratum::DependencyActivationExtension => {
                stratum != SourceResolutionStratum::Base
                    || declaration.package_identity != Some(package)
            }
        }
    }

    pub fn resolution_strata_separate(&self, left: SourceSpan, right: SourceSpan) -> bool {
        match (self.file_at(left), self.file_at(right)) {
            (Some(left), Some(right)) => left.resolution_stratum != right.resolution_stratum,
            _ => false,
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter()
    }

    /*
        This map with every unselected file's path and text removed. Two
        compiles whose selected files, identities and routing agree have equal
        views, whatever else the invocation's catalogue holds.
    */
    pub fn selection_view(&self) -> Self {
        let mut view = self.clone();
        for file in &mut view.files {
            if !self.selection.includes(file.source_id) {
                file.path = PathBuf::new();
                file.package_root = PathBuf::new();
                file.source = Arc::from("");
            }
        }
        view
    }

    pub fn selected_files(&self) -> impl Iterator<Item = &SourceFile> {
        self.files
            .iter()
            .filter(|file| self.selection.includes(file.source_id))
    }

    pub fn text_at(&self, source_span: SourceSpan) -> &str {
        self.get(source_span.source_id)
            .map(|file| file.text_at(source_span.span))
            .unwrap_or("")
    }
}

fn same_package_files(left: &SourceFile, right: &SourceFile) -> bool {
    match (left.package_identity, right.package_identity) {
        (Some(left), Some(right)) => left == right,
        (None, None) => left.origin == right.origin && left.package_root == right.package_root,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use source_text::Span;
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{
        DependencyScope, SourceFile, SourceId, SourceMap, SourceOrigin, SourceResolutionStratum,
        SourceSpan,
    };
    use source_provenance::PackageKeyIdentity;

    #[test]
    fn source_span_carries_source_identity() {
        let file = SourceFile {
            source_id: SourceId(7),
            path: PathBuf::from("main.omg"),
            package_root: PathBuf::from("."),
            package_identity: None,
            dependency_scope: DependencyScope::Product,
            origin: SourceOrigin::User,
            resolution_stratum: SourceResolutionStratum::Base,
            source: Arc::from("machine main {}"),
        };
        let span = file.source_span(Span::new(8, 12));

        assert_eq!(span.source_id, SourceId(7));
        assert_eq!(file.text_at(span.span), "main");
    }

    #[test]
    fn query_aliases_stay_in_the_unowned_authors_captured_scope() {
        let mut sources = SourceMap::default();
        let root = PathBuf::from("query-root");
        let selected = PackageKeyIdentity::from_digest([3; 32]).unwrap();
        let foreign = PackageKeyIdentity::from_digest([5; 32]).unwrap();
        let foreign_target = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let local = sources
            .add_with_metadata(
                root.join("build.omg"),
                String::new(),
                root.clone(),
                None,
                SourceOrigin::User,
            )
            .source_id;
        let helper = sources
            .add_with_metadata(
                PathBuf::from("helper/build.omg"),
                String::new(),
                PathBuf::from("helper"),
                Some(foreign),
                SourceOrigin::User,
            )
            .source_id;
        let toolchain = sources
            .add_with_metadata(
                root.join("prelude.omg"),
                String::new(),
                root.clone(),
                None,
                SourceOrigin::Toolchain,
            )
            .source_id;
        let other = sources
            .add_with_metadata(
                PathBuf::from("unbound/build.omg"),
                String::new(),
                PathBuf::from("unbound"),
                None,
                SourceOrigin::User,
            )
            .source_id;
        sources.retain_product_dependency_scope([(foreign, "dep", foreign_target)]);
        sources
            .retain_query_dependencies(root.clone(), [("dep".into(), selected)])
            .unwrap();
        let at = |source| SourceSpan::new(source, Span::new(0, 0));
        assert_eq!(
            sources.product_dependency_at(at(local), "dep"),
            Some(selected)
        );
        assert_eq!(
            sources.product_dependency_at(at(helper), "dep"),
            Some(foreign_target)
        );
        assert_eq!(sources.product_dependency_at(at(toolchain), "dep"), None);
        assert_eq!(sources.product_dependency_at(at(other), "dep"), None);
        assert_eq!(sources.product_dependency_at(at(local), "missing"), None);
        assert_eq!(
            sources.get(local).unwrap().package_identity,
            None,
            "query routing must not assign a final declaration owner"
        );
        assert!(
            sources
                .retain_query_dependencies(root, [("dep".into(), foreign)])
                .is_err()
        );
        assert_eq!(
            sources.product_dependency_at(at(local), "dep"),
            Some(selected)
        );
    }

    #[test]
    fn invalid_source_span_resolves_to_empty_text() {
        let sources = SourceMap::default();
        let source_span = SourceSpan::new(SourceId(99), Span::new(0, 4));

        assert_eq!(sources.text_at(source_span), "");
    }

    #[test]
    fn source_free_reference_remains_permissive_with_an_extension_declaration() {
        let mut sources = SourceMap::default();
        let extension = sources
            .add_with_metadata_and_resolution_stratum(
                PathBuf::from(".omega/generated/extension.omg"),
                String::from("data Extension {}"),
                PathBuf::from("."),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::CurrentActivationExtension,
            )
            .source_span(Span::new(5, 14));
        let source_free = sources
            .add(PathBuf::from("main.omg"), String::new())
            .source_span(Span::new(0, 0));

        assert!(sources.reference_can_see_declaration(SourceSpan::default(), extension));
        assert!(sources.reference_can_see_declaration(source_free, extension));
    }

    #[test]
    fn dependency_generated_declaration_hides_from_its_own_authored_units_only() {
        let kit = PackageKeyIdentity::from_digest([3; 32]).expect("nonzero identity");
        let consumer = PackageKeyIdentity::from_digest([4; 32]).expect("nonzero identity");
        let mut sources = SourceMap::default();
        let authored = sources
            .add_with_metadata(
                PathBuf::from("kit/main.omg"),
                String::from("public machine value() -> i32 { generated_value() }"),
                PathBuf::from("kit"),
                Some(kit),
                SourceOrigin::User,
            )
            .source_span(Span::new(29, 44));
        let generated = sources
            .add_with_metadata_and_resolution_stratum(
                PathBuf::from("kit/.omega/generated/generated.omg"),
                String::from("public machine generated_value() -> i32 { seed() }"),
                PathBuf::from("kit"),
                Some(kit),
                SourceOrigin::User,
                SourceResolutionStratum::DependencyActivationExtension,
            )
            .source_span(Span::new(12, 27));
        let generated_reference = SourceSpan::new(generated.source_id, Span::new(39, 43));
        let importer = sources
            .add_with_metadata(
                PathBuf::from("app/main.omg"),
                String::from("use kit::main; machine entry() -> i32 { generated_value() }"),
                PathBuf::from("app"),
                Some(consumer),
                SourceOrigin::User,
            )
            .source_span(Span::new(40, 55));

        assert!(!sources.reference_can_see_declaration(authored, generated));
        assert!(sources.reference_can_see_declaration(importer, generated));
        assert!(sources.reference_can_see_declaration(generated_reference, authored));
        assert!(sources.reference_can_see_declaration(generated_reference, generated));
        assert!(sources.resolution_strata_separate(authored, generated));
    }

    #[test]
    fn reconciled_package_identity_supersedes_source_root_spelling() {
        let first_identity = PackageKeyIdentity::from_digest([1; 32]).expect("nonzero identity");
        let second_identity = PackageKeyIdentity::from_digest([2; 32]).expect("nonzero identity");
        let mut sources = SourceMap::default();
        let first = sources
            .add_with_metadata(
                PathBuf::from("cache/a.omg"),
                String::new(),
                PathBuf::from("cache"),
                Some(first_identity),
                source_admission::SourceOrigin::User,
            )
            .source_id;
        let second = sources
            .add_with_metadata(
                PathBuf::from("cache/b.omg"),
                String::new(),
                PathBuf::from("cache"),
                Some(second_identity),
                source_admission::SourceOrigin::User,
            )
            .source_id;
        let relocated = sources
            .add_with_metadata(
                PathBuf::from("other/c.omg"),
                String::new(),
                PathBuf::from("other"),
                Some(first_identity),
                source_admission::SourceOrigin::User,
            )
            .source_id;

        let span = |source_id| SourceSpan::new(source_id, Span::new(0, 0));
        assert!(!sources.same_package(span(first), span(second)));
        assert!(sources.same_package(span(first), span(relocated)));
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use source_admission::source_identity::*;
    use source_provenance::{ConsumedSourceUnit, ConsumedSourceUnitKind};

    fn toolchain(root: &str) -> SourceFile {
        let mut sources = SourceMap::default();
        let root = std::path::PathBuf::from(root).join("core");
        sources
            .add_with_metadata(
                root.join("numbers.omg"),
                "machine answer() -> u64 { 16 }".into(),
                root,
                None,
                SourceOrigin::Toolchain,
            )
            .clone()
    }

    #[test]
    fn toolchain_protocol_is_stable_across_physical_roots_and_source_ids() {
        let first = toolchain("first/toolchain");
        let mut relocated = toolchain("different/cache");
        relocated.source_id = source_text::SourceId(417);
        let digest = toolchain_source_identity_digest(&first).expect("identity");
        let hexadecimal = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hexadecimal,
            "8aa68ba24b8af7d4b20c3b58d6a447d0cd25214f14901a6956fb815578d4a4fb"
        );
        assert_eq!(
            digest,
            toolchain_source_identity_digest(&relocated).expect("relocated identity")
        );
        relocated.source = "machine answer() -> u64 { 17 }".into();
        assert_ne!(
            digest,
            toolchain_source_identity_digest(&relocated).expect("changed identity")
        );
    }

    #[test]
    fn closed_contract_identity_retains_role_without_granting_core_exposure() {
        let mut file = toolchain("toolchain");
        let core = toolchain_source_identity_digest(&file).expect("core identity");
        file.origin = SourceOrigin::ToolchainContract;
        let contract = consumed_source_unit(&file, false).expect("contract identity");
        assert_eq!(contract.kind(), ConsumedSourceUnitKind::ToolchainContract);
        let encoded = canonical_source_entry(&file).expect("canonical contract");
        assert_eq!(encoded[0], 5);
        let digest = toolchain_source_identity_digest(&file).expect("contract digest");
        assert_ne!(core, digest, "changing exposure changes admission identity");
        assert_eq!(
            toolchain_source_identities([&file]).unwrap(),
            vec![(file.source_id, digest)]
        );
        assert_eq!(
            ConsumedSourceUnit::from_retained(
                contract.kind(),
                contract.package(),
                contract.toolchain_namespace().map(str::to_owned),
                contract.relative_path().to_vec(),
                contract.byte_count(),
                contract.content_digest(),
            )
            .unwrap(),
            contract,
        );
        assert!(consumed_source_unit(&file, true).is_err());
        file.path = "<build-prelude>".into();
        assert!(consumed_source_unit(&file, false).is_err());
    }

    #[test]
    fn changing_toolchain_role_does_not_share_an_unbound_namespace() {
        let core = toolchain("toolchain");
        let mut contract = core.clone();
        contract.source_id = source_text::SourceId(1);
        contract.origin = SourceOrigin::ToolchainContract;
        let core_span = core.source_span(source_text::Span::new(0, 4));
        let contract_span = contract.source_span(source_text::Span::new(0, 4));
        let sources = SourceMap::from_files(vec![core, contract]);
        assert!(!sources.same_package(core_span, contract_span));
        assert!(!sources.same_checked_instance(core_span, contract_span));
    }

    #[test]
    fn source_identity_does_not_grant_toolchain_or_generated_authority() {
        let mut file = toolchain("toolchain");
        assert!(
            consumed_source_unit(&file, true)
                .expect_err("generated toolchain")
                .contains("generated-source custody")
        );
        file.origin = SourceOrigin::User;
        assert!(
            toolchain_source_identity_digest(&file)
                .expect_err("user source")
                .contains("non-toolchain")
        );
        assert!(
            consumed_source_unit(&file, false)
                .expect_err("missing package")
                .contains("no reconciled package identity")
        );
    }

    #[test]
    fn generated_coordinates_retain_checked_scope_but_authored_bytes_share() {
        let mut file = toolchain("package");
        file.origin = SourceOrigin::User;
        file.package_identity = source_provenance::PackageKeyIdentity::from_digest([1; 32]);
        let authored = consumed_source_unit(&file, false).expect("authored");
        let generated = consumed_source_unit(&file, true).expect("generated");
        file.dependency_scope = source_provenance::DependencyScope::Build;
        assert_eq!(
            authored,
            consumed_source_unit(&file, false).expect("authored Build")
        );
        assert_ne!(
            generated,
            consumed_source_unit(&file, true).expect("generated Build")
        );
    }
}
