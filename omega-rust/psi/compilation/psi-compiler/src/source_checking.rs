/*
    Whole-program and library-slice preparation, retained-judgment matching, and
    Psi 05 checking under fresh invocation authority.

    We admit opaque representations and source-completion inputs, match retained
    checked programs and library slices against exact preparation inputs, and
    produce `CheckedSource` and `SelectedCheckedSource` records prior to product
    pruning.
*/

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    ExecutedProductSelection, ProductDeclarationRequest,
};
use representation_selections::OpaqueRepresentationSelection;
use std::sync::Arc;

mod constant_entries {
    pub(super) fn retain_evaluation_entries(
        program: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let mut entries = Vec::new();
        for declaration in program.const_declarations() {
            if declaration.evaluated_value.is_some() || !declaration.evaluation_entry.is_valid() {
                continue;
            }
            if !program
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                return Err(vec![diagnostics::Diagnostic::error(
                    "retained constant lost its complete evaluation entry",
                )]);
            }
            let entry = program.tables.machines.get(declaration.evaluation_entry);
            if program.machine_by_symbol(entry.symbol).is_none() {
                entries.push(entry.clone());
            }
        }
        for entry in entries {
            program.push_machine(entry);
        }
        Ok(())
    }
}
mod library_slice {
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceFile, SourceOrigin};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct LibrarySliceRequest {
        pub root_package: Option<PackageKeyIdentity>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LibrarySliceKey([u8; 32]);

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct LibrarySliceTotals {
        pub distinct: u64,
        pub computed: u64,
    }

    static COMPUTED: AtomicU64 = AtomicU64::new(0);
    static DISTINCT: Mutex<Vec<LibrarySliceKey>> = Mutex::new(Vec::new());

    pub fn library_slice_totals() -> LibrarySliceTotals {
        let distinct = DISTINCT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        LibrarySliceTotals {
            distinct: distinct as u64,
            computed: COMPUTED.load(Ordering::Relaxed),
        }
    }

    pub(super) fn record_computed(key: LibrarySliceKey) {
        COMPUTED.fetch_add(1, Ordering::Relaxed);
        let mut distinct = DISTINCT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(position) = distinct.binary_search(&key) {
            distinct.insert(position, key);
        }
    }

    pub(super) struct SliceAdmission<'input> {
        pub completion: &'input validation::source_completion::SourceCompletionAdmission<'input>,
        pub layout_scopes: &'input [build_time_evaluation::FixedLayoutTargetScope],
        pub representations: &'input [representation_selections::OpaqueRepresentationSelection],
        pub asm_authority: Option<validation::AsmAuthorityAdmission>,
        pub wire_demands: &'input [language_semantics::wire::WireCompatibilityDemand],
    }

    impl LibrarySliceRequest {
        pub(super) fn key(
            &self,
            program: &typed_trees::TypedTrees,
            admission: &SliceAdmission<'_>,
        ) -> Option<LibrarySliceKey> {
            let sources = program.symbols.source_map()?;
            let mut slice: Vec<SliceSource> = sources
                .selected_files()
                .filter(|file| self.in_slice(file))
                .map(SliceSource::of)
                .collect();
            if slice.is_empty() {
                return None;
            }
            slice.sort();
            let mut digest =
                optimization::digest_encoder::DomainDigestEncoder::new(b"omega-library-slice-v1\0");
            digest.len(slice.len());
            for source in &slice {
                source.encode(&mut digest);
            }
            let in_slice = |symbol: symbols::SymbolHandle| {
                program
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .and_then(|span| program.symbols.source_file(span))
                    .is_some_and(|file| self.in_slice(file))
            };
            encode_completion(&mut digest, admission.completion);
            digest.len(admission.layout_scopes.len());
            for scope in admission.layout_scopes {
                digest.bytes(scope.target.as_bytes());
                match &scope.facts {
                    Some(facts) => {
                        digest.byte(1);
                        digest.bytes(facts.profile.as_bytes());
                        digest.u64(facts.pointer_size);
                        digest.u64(facts.pointer_alignment);
                        digest.u64(facts.case_tag_size);
                        digest.u64(facts.case_tag_alignment);
                        digest.byte(u8::from(facts.little_endian));
                    }
                    None => digest.byte(0),
                }
                match &scope.entry_declarations {
                    Some(entries) => {
                        digest.byte(1);
                        let mut paths: Vec<String> = entries
                            .iter()
                            .map(|entry| portable_symbol(program, *entry))
                            .collect();
                        paths.sort();
                        digest.len(paths.len());
                        for path in &paths {
                            digest.bytes(path.as_bytes());
                        }
                    }
                    None => digest.byte(0),
                }
                let mut providers: Vec<Vec<u8>> = scope
                    .provider_declarations
                    .iter()
                    .map(portable_provider_selection)
                    .collect();
                providers.sort();
                digest.len(providers.len());
                for provider in &providers {
                    digest.bytes(provider);
                }
            }
            let mut representations: Vec<([u8; 32], String)> = admission
                .representations
                .iter()
                .filter(|selection| in_slice(selection.opaque()))
                .map(|selection| {
                    (
                        selection.rederived_selected_application_commitment(),
                        portable_symbol(program, selection.opaque()),
                    )
                })
                .collect();
            representations.sort();
            digest.len(representations.len());
            for (commitment, opaque) in &representations {
                digest.raw(commitment);
                digest.bytes(opaque.as_bytes());
            }
            match admission.asm_authority {
                Some(authority) => {
                    digest.byte(1);
                    digest.bytes(format!("{authority:?}").as_bytes());
                }
                None => digest.byte(0),
            }
            let mut wire: Vec<String> = admission
                .wire_demands
                .iter()
                .map(|demand| format!("{demand:?}"))
                .collect();
            wire.sort();
            digest.len(wire.len());
            for demand in &wire {
                digest.bytes(demand.as_bytes());
            }
            Some(LibrarySliceKey(digest.finish()))
        }

        pub(super) fn in_slice(&self, file: &SourceFile) -> bool {
            file.origin == SourceOrigin::Toolchain
                || file
                    .package_identity
                    .is_some_and(|package| Some(package) != self.root_package)
        }
    }

    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    pub(super) struct SliceSource {
        package: [u8; 32],
        path: String,
        scope: u8,
        stratum: u8,
        content: [u8; 32],
    }

    impl SliceSource {
        pub(super) fn of(file: &SourceFile) -> Self {
            let relative = file
                .path
                .strip_prefix(&file.package_root)
                .unwrap_or(&file.path);
            let mut content = optimization::digest_encoder::DomainDigestEncoder::new(
                b"omega-library-source-v1\0",
            );
            content.bytes(file.source.as_bytes());
            Self {
                package: file
                    .package_identity
                    .map(PackageKeyIdentity::digest)
                    .unwrap_or([0; 32]),
                path: relative.to_string_lossy().replace('\\', "/"),
                scope: file.dependency_scope as u8,
                stratum: file.resolution_stratum as u8,
                content: content.finish(),
            }
        }

        pub(super) fn encode(&self, digest: &mut optimization::digest_encoder::DomainDigestEncoder) {
            digest.raw(&self.package);
            digest.bytes(self.path.as_bytes());
            digest.byte(self.scope);
            digest.byte(self.stratum);
            digest.raw(&self.content);
        }
    }

    fn encode_completion(
        digest: &mut optimization::digest_encoder::DomainDigestEncoder,
        completion: &validation::source_completion::SourceCompletionAdmission<'_>,
    ) {
        use validation::source_completion::SourceCompletionAdmission;
        match completion {
            SourceCompletionAdmission::None => digest.byte(0),
            SourceCompletionAdmission::AcceptedProcessExit {
                package,
                declaration_path,
            } => {
                digest.byte(1);
                digest.raw(&package.digest());
                digest.bytes(declaration_path.as_bytes());
            }
            SourceCompletionAdmission::DiscoverProcessExit => digest.byte(2),
        }
    }

    fn portable_symbol(program: &typed_trees::TypedTrees, symbol: symbols::SymbolHandle) -> String {
        let owner = program
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .map(|file| match file.package_identity {
                Some(package) => hex(&package.digest()),
                None => format!("{:?}", file.origin),
            })
            .unwrap_or_default();
        format!("{owner}::{}", program.symbols.display_path(symbol, "::"))
    }

    fn portable_provider_selection(
        selection: &build_time_evaluation::provider_declarations::ProviderSelection,
    ) -> Vec<u8> {
        use build_time_evaluation::provider_declarations::{
            ProviderSelectionIdentity, ProviderSelectionSubject,
        };
        fn identity(text: &mut String, identity: &ProviderSelectionIdentity) {
            let package = identity
                .package
                .map(|package| hex(&package.digest()))
                .unwrap_or_default();
            text.push_str(&format!(
                "{package}|{}|{:?}|{}\n",
                identity.canonical_path, identity.application_identity, identity.applied
            ));
        }
        let mut text = String::new();
        text.push_str(
            selection
                .target
                .map(|target| target.canonical_name())
                .unwrap_or("portable"),
        );
        text.push('\n');
        match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(subject) => {
                text.push_str("trait\n");
                identity(&mut text, subject);
            }
            ProviderSelectionSubject::BoundaryRequirement(subject) => {
                text.push_str("requirement\n");
                identity(&mut text, subject);
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                let package = family
                    .package
                    .map(|package| hex(&package.digest()))
                    .unwrap_or_default();
                text.push_str(&format!("family\n{package}|{}\n", family.canonical_path));
                for coordinate in family.coordinates() {
                    text.push_str(&format!(
                        "{}|{}\n",
                        coordinate.requirement_identity, coordinate.static_parameter_count
                    ));
                }
            }
        }
        identity(&mut text, &selection.provider_type);
        text.push_str(&format!("{:?}", selection.composition_mode));
        text.into_bytes()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::{LibrarySliceRequest, SliceSource};
        use semantic_vocabulary::PackageKeyIdentity;
        use source::{
            DependencyScope, SourceFile, SourceId, SourceOrigin, SourceResolutionStratum,
        };
        use std::path::PathBuf;
        use std::sync::Arc;

        fn file(
            root: &str,
            path: &str,
            origin: SourceOrigin,
            package: Option<PackageKeyIdentity>,
            text: &str,
        ) -> SourceFile {
            SourceFile {
                source_id: SourceId(0),
                path: PathBuf::from(root).join(path),
                package_root: PathBuf::from(root),
                package_identity: package,
                dependency_scope: DependencyScope::Product,
                origin,
                resolution_stratum: SourceResolutionStratum::Base,
                source: Arc::from(text),
            }
        }

        #[test]
        fn the_slice_is_every_source_except_the_root_package() {
            let root = PackageKeyIdentity::from_digest([1; 32]);
            let dependency = PackageKeyIdentity::from_digest([2; 32]);
            let request = LibrarySliceRequest { root_package: root };
            let core = file("/core", "option.omg", SourceOrigin::Toolchain, None, "");
            let std = file("/std", "console.omg", SourceOrigin::User, dependency, "");
            let main = file("/app", "main.omg", SourceOrigin::User, root, "");
            assert!(request.in_slice(&core));
            assert!(request.in_slice(&std));
            assert!(!request.in_slice(&main));
            let standalone = LibrarySliceRequest::default();
            let loose = file("/app", "main.omg", SourceOrigin::User, None, "");
            assert!(standalone.in_slice(&core));
            assert!(!standalone.in_slice(&loose));
        }

        #[test]
        fn a_slice_source_is_named_by_package_relative_path_and_bytes() {
            let package = PackageKeyIdentity::from_digest([2; 32]);
            let here = SliceSource::of(&file("/a/std", "x.omg", SourceOrigin::User, package, "m"));
            let moved = SliceSource::of(&file("/b/std", "x.omg", SourceOrigin::User, package, "m"));
            let edited =
                SliceSource::of(&file("/a/std", "x.omg", SourceOrigin::User, package, "n"));
            assert!(here == moved);
            assert!(here != edited);
        }
    }
}
/*
    Checked dependency slices kept on local disk between compiles.

    A compile may load a check it did not run only when the check judged
    nothing but dependency source and the stored result was produced for
    exactly the same input (`wiki/spec/build/validation_subjects.md`, "What
    this rule does not do"). The key binds the encoded program the check would
    judge, its declaration authority and package labels, the content of every
    source it contains, the producing compiler's identity and the target. A
    program holding any source of the consumer's own package has no key and is
    always checked. A record that is missing, truncated, carries another key,
    fails its digest or does not decode is a miss, and the ordinary check runs.

    The host owns where records live and who produced them, so this module
    only frames bytes; `CheckedSliceStore` performs the reads and writes.
*/
mod checked_slice {
    use super::library_slice::{LibrarySliceRequest, SliceSource};
    use semantic_vocabulary::PackageKeyIdentity;

    const RECORD_MAGIC: &[u8; 8] = b"OMGCSLC\x01";
    const KEY_DOMAIN: &[u8] = b"omega-checked-slice-key-v1\0";
    const PAYLOAD_DOMAIN: &[u8] = b"omega-checked-slice-payload-v1\0";
    const LIMITS: compiler_storage::RecordLimits = compiler_storage::RecordLimits {
        bytes: 256 * 1024 * 1024,
        allocation: 1024 * 1024 * 1024,
        elements: 64 * 1024 * 1024,
        depth: 512,
    };

    /// Local storage for checked dependency slices. Implementations must not
    /// publish a partially written record under its final name.
    pub trait CheckedSliceStore: Send + Sync {
        /// Identity of the compiler that produces and loads these records.
        fn producer_identity(&self) -> [u8; 32];
        fn load(&self, key: &[u8; 32]) -> Option<Vec<u8>>;
        fn save(&self, key: &[u8; 32], record: &[u8]);
    }

    /// One check's permission to use a store.
    pub struct CheckedSliceReuse<'store> {
        pub store: &'store dyn CheckedSliceStore,
        /// The target the program was typed for.
        pub target: &'store str,
        pub owner: CheckedSliceOwner,
    }

    /// Whose source the checked program can hold.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CheckedSliceOwner {
        /// A dependency's own Build, run for its consumer. Its program holds
        /// that dependency and the toolchain only, even before package
        /// admission names its files.
        Dependency,
        /// A program that may hold the consumer's source. It is reusable only
        /// when every file belongs to the library slice outside this package;
        /// `None` names a consumer without a package, whose loose sources are
        /// never reused.
        Consumer(Option<PackageKeyIdentity>),
    }

    pub(super) struct SliceKey([u8; 32]);

    /*
        Only a check whose admission is the Build check's (a declaration
        authority and nothing else) is keyed. Product checks carry roots,
        layout scopes and representation selections and always judge the
        consumer's own package, so they never reach a store.
    */
    pub(super) fn key(
        program: &typed_trees::TypedTrees,
        admission: &super::SourceAdmission<'_>,
        reuse: &CheckedSliceReuse<'_>,
    ) -> Option<SliceKey> {
        let representations = &admission.representations;
        if !admission.external_roots.is_empty()
            || !matches!(
                admission.completion,
                validation::source_completion::SourceCompletionAdmission::None
            )
            || representations.activation.is_some()
            || !representations.declared.is_empty()
            || !representations.executed.is_empty()
            || !representations.retained.is_empty()
            || !admission.layout_scopes.is_empty()
            || admission.layout_selection_authority.is_some()
            || admission.asm_authority.is_some()
            || !admission.wire_demands.is_empty()
        {
            return None;
        }
        let sources = program.symbols.source_map()?;
        let mut files = Vec::new();
        for file in sources.selected_files() {
            if let CheckedSliceOwner::Consumer(root_package) = reuse.owner
                && !(LibrarySliceRequest { root_package }).in_slice(file)
            {
                return None;
            }
            files.push(SliceSource::of(file));
        }
        if files.is_empty() {
            return None;
        }
        files.sort();
        let authority =
            validation::RecordedDeclarationAuthority::capture(admission.declarations).ok()?;
        let mut digest = optimization::digest_encoder::DomainDigestEncoder::new(KEY_DOMAIN);
        digest.raw(&reuse.store.producer_identity());
        digest.bytes(reuse.target.as_bytes());
        digest.bytes(&compiler_storage::encode(&authority, LIMITS).ok()?);
        if let (validation::RecordedDeclarationAuthority::Packages(scope), Some(labels)) =
            (&authority, admission.declarations)
        {
            let mut packages: Vec<PackageKeyIdentity> = scope
                .iter()
                .flat_map(|(requester, owners)| std::iter::once(*requester).chain(owners.iter().copied()))
                .collect();
            packages.sort_unstable();
            packages.dedup();
            digest.len(packages.len());
            for package in packages {
                digest.raw(&package.digest());
                digest.bytes(labels.package_label(package).as_bytes());
            }
        }
        digest.len(files.len());
        for file in &files {
            file.encode(&mut digest);
        }
        digest.bytes(&compiler_storage::encode(program, LIMITS).ok()?);
        Some(SliceKey(digest.finish()))
    }

    pub(super) fn load(
        reuse: &CheckedSliceReuse<'_>,
        key: &SliceKey,
    ) -> Option<checked_trees::CheckedTrees> {
        let record = reuse.store.load(&key.0)?;
        let rest = record.strip_prefix(RECORD_MAGIC.as_slice())?;
        let (stored_key, rest) = rest.split_first_chunk::<32>()?;
        if *stored_key != key.0 {
            return None;
        }
        let (length, rest) = rest.split_first_chunk::<8>()?;
        let length = usize::try_from(u64::from_le_bytes(*length)).ok()?;
        if rest.len() != length.checked_add(32)? {
            return None;
        }
        let (payload, payload_digest) = rest.split_at(length);
        if payload_digest != payload_identity(&key.0, payload) {
            return None;
        }
        compiler_storage::decode(payload, LIMITS).ok()
    }

    pub(super) fn save(
        reuse: &CheckedSliceReuse<'_>,
        key: &SliceKey,
        checked: &checked_trees::CheckedTrees,
    ) {
        let Ok(payload) = compiler_storage::encode(checked, LIMITS) else {
            return;
        };
        let mut record = Vec::with_capacity(RECORD_MAGIC.len() + 32 + 8 + payload.len() + 32);
        record.extend_from_slice(RECORD_MAGIC);
        record.extend_from_slice(&key.0);
        record.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        record.extend_from_slice(&payload);
        record.extend_from_slice(&payload_identity(&key.0, &payload));
        reuse.store.save(&key.0, &record);
    }

    fn payload_identity(key: &[u8; 32], payload: &[u8]) -> [u8; 32] {
        let mut digest = optimization::digest_encoder::DomainDigestEncoder::new(PAYLOAD_DOMAIN);
        digest.raw(key);
        digest.bytes(payload);
        digest.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{CheckedSliceReuse, CheckedSliceStore, SliceKey};
        use std::sync::Mutex;

        #[derive(Default)]
        struct Memory(Mutex<Vec<([u8; 32], Vec<u8>)>>);

        impl CheckedSliceStore for Memory {
            fn producer_identity(&self) -> [u8; 32] {
                [7; 32]
            }
            fn load(&self, key: &[u8; 32]) -> Option<Vec<u8>> {
                let records = self.0.lock().unwrap();
                records.iter().find(|(stored, _)| stored == key).map(|(_, bytes)| bytes.clone())
            }
            fn save(&self, key: &[u8; 32], record: &[u8]) {
                self.0.lock().unwrap().push((*key, record.to_vec()));
            }
        }

        fn reuse(store: &Memory) -> CheckedSliceReuse<'_> {
            CheckedSliceReuse {
                store,
                target: "linux_x86_64",
                owner: super::CheckedSliceOwner::Dependency,
            }
        }

        #[test]
        fn a_saved_slice_loads_under_its_own_key_only() {
            let store = Memory::default();
            let checked = checked_trees::CheckedTrees::default();
            super::save(&reuse(&store), &SliceKey([1; 32]), &checked);
            assert_eq!(super::load(&reuse(&store), &SliceKey([1; 32])), Some(checked));
            assert_eq!(super::load(&reuse(&store), &SliceKey([2; 32])), None);
        }

        #[test]
        fn a_damaged_or_misfiled_record_is_a_miss() {
            let store = Memory::default();
            super::save(
                &reuse(&store),
                &SliceKey([1; 32]),
                &checked_trees::CheckedTrees::default(),
            );
            let record = store.0.lock().unwrap()[0].1.clone();
            let mut damaged = record.clone();
            let middle = damaged.len() / 2;
            damaged[middle] ^= 1;
            let truncated = record[..record.len() - 1].to_vec();
            for bytes in [damaged, truncated, Vec::new()] {
                let other = Memory::default();
                other.save(&[1; 32], &bytes);
                assert_eq!(super::load(&reuse(&other), &SliceKey([1; 32])), None);
            }
            let misfiled = Memory::default();
            misfiled.save(&[3; 32], &record);
            assert_eq!(super::load(&reuse(&misfiled), &SliceKey([3; 32])), None);
        }
    }
}
mod retained_check {
    use build_time_evaluation::FixedLayoutTargetScope;
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    pub struct RetainedCheck {
        pub(super) judgment: typed_trees_to_checked_trees::RetainedCheck,
        pub(super) layout_evaluation: Option<Arc<LayoutEvaluationInput>>,
    }

    pub(super) struct LayoutEvaluationInput {
        pub(super) program: typed_trees::TypedTrees,
        pub(super) scopes: Vec<FixedLayoutTargetScope>,
        pub(super) authority: validation::RecordedDeclarationAuthority,
    }

    impl std::fmt::Debug for LayoutEvaluationInput {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("LayoutEvaluationInput")
                .field("scopes", &self.scopes.len())
                .finish_non_exhaustive()
        }
    }

    impl LayoutEvaluationInput {
        pub(super) fn capture(
            program: &typed_trees::TypedTrees,
            scopes: &[FixedLayoutTargetScope],
            authority: Option<&dyn validation::DeclarationSelectionAuthority>,
        ) -> Option<Self> {
            Some(Self {
                program: program.clone(),
                scopes: scopes.to_vec(),
                authority: validation::RecordedDeclarationAuthority::capture(authority).ok()?,
            })
        }

        pub(super) fn matches(
            &self,
            program: &typed_trees::TypedTrees,
            scopes: &[FixedLayoutTargetScope],
            authority: Option<&dyn validation::DeclarationSelectionAuthority>,
        ) -> bool {
            self.scopes == scopes
                && validation::RecordedDeclarationAuthority::capture(authority)
                    .is_ok_and(|current| current == self.authority)
                && self.program == *program
        }
    }

    impl RetainedCheck {
        pub fn same_judgment(&self, other: &Self) -> bool {
            self.judgment.same_judgment(&other.judgment)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::LayoutEvaluationInput;
        use build_time_evaluation::FixedLayoutTargetScope;
        use semantic_vocabulary::PackageKeyIdentity;
        use std::collections::BTreeMap;

        struct Scoped(BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>);

        impl validation::DeclarationSelectionAuthority for Scoped {
            fn allows_declaration_selection(
                &self,
                requester: PackageKeyIdentity,
                owner: PackageKeyIdentity,
            ) -> bool {
                requester == owner
                    || self
                        .0
                        .get(&requester)
                        .is_some_and(|owners| owners.contains(&owner))
            }
            fn package_label(&self, _identity: PackageKeyIdentity) -> String {
                String::new()
            }
            fn declaration_selection_scope(
                &self,
            ) -> Option<BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
                Some(self.0.clone())
            }
        }

        struct Opaque;

        impl validation::DeclarationSelectionAuthority for Opaque {
            fn allows_declaration_selection(
                &self,
                _requester: PackageKeyIdentity,
                _owner: PackageKeyIdentity,
            ) -> bool {
                true
            }
            fn package_label(&self, _identity: PackageKeyIdentity) -> String {
                String::new()
            }
        }

        fn scope(target: &str, pointer_size: u64) -> FixedLayoutTargetScope {
            FixedLayoutTargetScope {
                target: target.to_owned(),
                facts: Some(layout_plans::LayoutTargetFacts {
                    profile: target.to_owned(),
                    pointer_size,
                    pointer_alignment: pointer_size,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                }),
                entry_declarations: None,
                provider_declarations: Vec::new(),
            }
        }

        fn package(marker: u8) -> PackageKeyIdentity {
            PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero digest")
        }

        #[test]
        fn layout_evaluation_input_matches_only_the_same_targets_and_authority() {
            let program = typed_trees::TypedTrees::default();
            let scopes = [scope("linux_x86_64", 8), scope("macos_arm64", 8)];
            let authority = Scoped(BTreeMap::from([(package(1), vec![package(1), package(2)])]));
            let input = LayoutEvaluationInput::capture(&program, &scopes, Some(&authority))
                .expect("a predicate with an exported scope is comparable");
            assert!(input.matches(&program, &scopes, Some(&authority)));

            let another_target = [scope("linux_x86_64", 8), scope("linux_arm64", 8)];
            assert!(!input.matches(&program, &another_target, Some(&authority)));
            let other_facts = [scope("linux_x86_64", 8), scope("macos_arm64", 4)];
            assert!(!input.matches(&program, &other_facts, Some(&authority)));
            assert!(!input.matches(&program, &scopes[..1], Some(&authority)));

            let wider = Scoped(BTreeMap::from([(
                package(1),
                vec![package(1), package(2), package(3)],
            )]));
            assert!(!input.matches(&program, &scopes, Some(&wider)));
            assert!(!input.matches(&program, &scopes, None));
            assert!(!input.matches(&program, &scopes, Some(&Opaque)));
        }

        #[test]
        fn another_program_under_the_same_targets_and_authority_misses() {
            let program = typed_trees::TypedTrees::default();
            let scopes = [scope("linux_x86_64", 8)];
            let authority = Scoped(BTreeMap::from([(package(1), vec![package(1)])]));
            let input = LayoutEvaluationInput::capture(&program, &scopes, Some(&authority))
                .expect("a predicate with an exported scope is comparable");
            let mut other = typed_trees::TypedTrees::default();
            other.push_data_definition(Default::default());
            assert!(!input.matches(&other, &scopes, Some(&authority)));
            assert!(input.matches(&program, &scopes, Some(&authority)));
        }

        #[test]
        fn a_predicate_without_a_comparison_scope_is_never_captured() {
            let program = typed_trees::TypedTrees::default();
            let scopes = [scope("linux_x86_64", 8)];
            assert!(LayoutEvaluationInput::capture(&program, &scopes, Some(&Opaque)).is_none());
            let absent = LayoutEvaluationInput::capture(&program, &scopes, None)
                .expect("an absent predicate is comparable");
            assert!(absent.matches(&program, &scopes, None));
            assert!(!absent.matches(&program, &scopes, Some(&Opaque)));
        }
    }
}

pub use library_slice::{
    LibrarySliceKey, LibrarySliceRequest, LibrarySliceTotals, library_slice_totals,
};

pub use checked_slice::{CheckedSliceOwner, CheckedSliceReuse, CheckedSliceStore};
pub use retained_check::RetainedCheck;
pub use typed_trees_to_checked_trees::CheckedTreeProductSelection;

static WHOLE_PROGRAM_REUSE_HITS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn whole_program_reuse_hits() -> u64 {
    WHOLE_PROGRAM_REUSE_HITS.load(std::sync::atomic::Ordering::Relaxed)
}

fn record_whole_program_reuse() {
    WHOLE_PROGRAM_REUSE_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

static CHECKED_SLICE_LOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CHECKED_SLICE_SAVES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Checks this process loaded from a `CheckedSliceStore` and checks it saved.
pub fn checked_slice_totals() -> (u64, u64) {
    (
        CHECKED_SLICE_LOADS.load(std::sync::atomic::Ordering::Relaxed),
        CHECKED_SLICE_SAVES.load(std::sync::atomic::Ordering::Relaxed),
    )
}

fn record_checked_slice_load() {
    CHECKED_SLICE_LOADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn record_checked_slice_save() {
    CHECKED_SLICE_SAVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

static RETAINED_LAYOUT_MATCH_HITS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn retained_layout_match_hits() -> u64 {
    RETAINED_LAYOUT_MATCH_HITS.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub external_roots: &'request [symbols::SymbolHandle],
    pub completion: validation::source_completion::SourceCompletionAdmission<'request>,
    pub representations: ProductRepresentationInput<'request>,
    pub layout_scopes: &'request [build_time_evaluation::FixedLayoutTargetScope],
    pub layout_selection_authority:
        Option<Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>>,
    pub asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

#[derive(Default)]
pub struct ProductRepresentationInput<'request> {
    pub activation: Option<symbols::SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub retained: &'request [OpaqueRepresentationSelection],
}

struct AdmittedRepresentations {
    selections: Vec<OpaqueRepresentationSelection>,
    copy_properties: Vec<validation::OpaqueDataPropertyReceipt>,
}

impl ProductRepresentationInput<'_> {
    fn admit(
        &self,
        program: &typed_trees::TypedTrees,
    ) -> Result<AdmittedRepresentations, Vec<Diagnostic>> {
        let selections = representation_planning::rederive_opaque_representation_selections(
            program,
            self.activation,
            self.declared,
            self.executed,
            self.retained,
        )?;
        let copy_properties = selections
            .iter()
            .filter(|selection| {
                selection.copy_disposition()
                    == representation_selections::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
            })
            .map(|selection| validation::OpaqueDataPropertyReceipt::copy(selection.opaque()))
            .collect();
        Ok(AdmittedRepresentations {
            selections,
            copy_properties,
        })
    }
}

pub struct CheckRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub admission: SourceAdmission<'request>,
    pub previous: &'request [&'request RetainedCheck],
    pub retain: bool,
    pub library_slice: LibrarySliceRequest,
    pub checked_slices: Option<CheckedSliceReuse<'request>>,
}

pub struct CheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub reused: bool,
}

#[derive(Clone)]
pub struct SelectedCheckedSource {
    pub program: Arc<checked_trees::CheckedTrees>,
    pub provider_defaults:
        Arc<[build_time_evaluation::provider_declarations::TargetProviderDefaults]>,
    pub retained: Option<RetainedCheck>,
    pub selection: Option<CheckedTreeProductSelection>,
}

impl CheckRequest<'_> {
    pub fn check(self) -> Result<CheckedSource, Vec<Diagnostic>> {
        self.check_keeping_typed_frontier(None)
    }

    /*
        The same check, also copying out the frozen typed trees Psi 05 judged.

        Psi 04's output is moved into Psi 05 and is gone when that stage
        rejects, so a caller that wants to run a second checker on a rejected
        program has nothing to give it. With `frontier` supplied, the trees
        are cloned into it before Psi 05 takes them. The slot is filled exactly
        when Psi 04 completed (or a retained judgment supplied its trees), no
        matter what this function then returns; it stays `None` when the
        request was refused earlier. It normally holds Psi 05's exact input. If
        the lifecycle-plan step between the two stages refuses, it holds Psi
        04's output, which Psi 05 never saw. `check` passes no slot and clones
        nothing.
    */
    pub fn check_keeping_typed_frontier(
        self,
        mut frontier: Option<&mut Option<typed_trees::FrozenTypedTrees>>,
    ) -> Result<CheckedSource, Vec<Diagnostic>> {
        let stored_slice = self
            .checked_slices
            .as_ref()
            .and_then(|reuse| checked_slice::key(&self.program, &self.admission, reuse));
        if let (Some(reuse), Some(key)) = (self.checked_slices.as_ref(), stored_slice.as_ref())
            && let Some(checked) = checked_slice::load(reuse, key)
        {
            record_checked_slice_load();
            build_time_evaluation::admit_access_lifecycle_plans(
                &checked.typed,
                self.admission.declarations,
            )?;
            if let Some(frontier) = frontier.as_deref_mut() {
                *frontier = Some(checked.typed.clone());
            }
            return Ok(CheckedSource {
                program: Arc::new(checked),
                retained: None,
                reused: true,
            });
        }
        let mut program = self.program;
        let representations = self.admission.representations.admit(&program)?;
        let slice_key = {
            self.library_slice.key(
                &program,
                &library_slice::SliceAdmission {
                    completion: &self.admission.completion,
                    layout_scopes: self.admission.layout_scopes,
                    representations: &representations.selections,
                    asm_authority: self.admission.asm_authority,
                    wire_demands: self.admission.wire_demands,
                },
            )
        };
        validation::source_completion::prepare_source_completion(
            &mut program,
            self.admission.completion,
        )?;
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_opaque_property_receipts(&representations.copy_properties)
            .with_opaque_representation_selections(&representations.selections)
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let checking = typed_trees_to_checked_trees::CheckingRequest::new()
            .with_declaration_authority(self.admission.declarations)
            .with_external_roots(self.admission.external_roots);
        let admission = preparation.admission();
        let layout_authority = self.admission.layout_selection_authority.as_deref();
        let evaluated = if self.admission.layout_scopes.is_empty() {
            None
        } else {
            self.previous.iter().copied().find(|prior| {
                prior.layout_evaluation.as_ref().is_some_and(|input| {
                    input.matches(&program, self.admission.layout_scopes, layout_authority)
                }) && prior
                    .judgment
                    .judges_preparation_admission(&admission, self.admission.external_roots)
            })
        };
        let layout_evaluation = match evaluated {
            Some(_) => None,
            None if self.retain
                && !self.admission.layout_scopes.is_empty()
                && build_time_evaluation::has_fixed_layout_applications(&program) =>
            {
                retained_check::LayoutEvaluationInput::capture(
                    &program,
                    self.admission.layout_scopes,
                    layout_authority,
                )
                .map(Arc::new)
            }
            None => None,
        };
        if evaluated.is_none() {
            build_time_evaluation::evaluate_fixed_layout_applications(
                &mut program,
                self.admission.layout_scopes,
                self.admission.layout_selection_authority.clone(),
            )?;
            constant_entries::retain_evaluation_entries(&mut program)?;
        }
        let matching = evaluated.or_else(|| {
            {
                self.previous.iter().copied().find(|prior| {
                    prior.judgment.judges_preparation_input(
                        &program,
                        &admission,
                        self.admission.external_roots,
                    )
                })
            }
        });
        let outcome = if let Some(prior) = matching {
            if evaluated.is_some() {
                RETAINED_LAYOUT_MATCH_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            record_whole_program_reuse();
            if let Some(frontier) = frontier.as_deref_mut() {
                *frontier = Some(prior.judgment.judged_typed_trees().clone());
            }
            let outcome = typed_trees_to_checked_trees::reuse_retained_check(
                &prior.judgment,
                &checking,
                self.retain,
            )?;
            build_time_evaluation::admit_access_lifecycle_plans(
                &outcome.checked.typed,
                self.admission.declarations,
            )?;
            (
                outcome,
                prior.layout_evaluation.clone().or(layout_evaluation),
            )
        } else {
            if let Some(key) = slice_key {
                library_slice::record_computed(key);
            }
            let retained_input = self.retain.then(|| (Arc::new(program.clone()), admission));
            let (program, preparation) =
                typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)?;
            let prepared = frontier.is_some().then(|| program.clone());
            let program = build_time_evaluation::retain_access_lifecycle_plans(
                program,
                self.admission.declarations,
                self.admission.layout_scopes,
            );
            if let Some(frontier) = frontier {
                *frontier = match &program {
                    Ok(program) => Some(program.clone()),
                    Err(_) => prepared,
                };
            }
            let program = program?;
            let prior_dependency_bodies: Vec<_> = self
                .previous
                .iter()
                .flat_map(|prior| prior.judgment.strict_dependency_bodies().iter().cloned())
                .collect();
            let prior_frozen_bodies: Vec<_> = self
                .previous
                .iter()
                .flat_map(|prior| {
                    prior
                        .judgment
                        .strict_frozen_dependency_bodies()
                        .iter()
                        .cloned()
                })
                .collect();
            let outcome = typed_trees_to_checked_trees::lower_typed_trees_reusing(
                program,
                preparation,
                retained_input,
                &checking,
                &prior_dependency_bodies,
                &prior_frozen_bodies,
                self.retain,
            )?;
            (outcome, layout_evaluation)
        };
        let (outcome, layout_evaluation) = outcome;
        if let (Some(reuse), Some(key)) = (self.checked_slices.as_ref(), stored_slice.as_ref()) {
            checked_slice::save(reuse, key, &outcome.checked);
            record_checked_slice_save();
        }
        Ok(CheckedSource {
            program: outcome.checked,
            retained: outcome.retained.map(|judgment| RetainedCheck {
                judgment,
                layout_evaluation,
            }),
            reused: outcome.took_prior,
        })
    }
}

impl CheckedSource {
    pub fn select_product(
        self,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        let provider_defaults =
            build_time_evaluation::provider_declarations::retain_provider_defaults(
                build_time_evaluation::provider_declarations::ProviderPlanningContext::new(
                    &self.program.typed,
                    source::DependencyScope::Product,
                ),
            )?;
        let (program, selection) =
            typed_trees_to_checked_trees::select_checked_tree_product(self.program, roots)?;
        Ok(SelectedCheckedSource {
            program,
            provider_defaults: provider_defaults.into(),
            retained: self.retained,
            selection,
        })
    }
}

#[derive(Default)]
pub struct ProductSelections {
    entries: Vec<SelectedProduct>,
    productions: Arc<crate::PortableProductions>,
}

struct SelectedProduct {
    input: Arc<checked_trees::CheckedTrees>,
    roots: Option<Vec<symbols::SymbolHandle>>,
    output: SelectedCheckedSource,
}

impl ProductSelections {
    pub fn portable_productions(&self) -> Arc<crate::PortableProductions> {
        Arc::clone(&self.productions)
    }

    pub fn select(
        &mut self,
        checked: CheckedSource,
        roots: Option<Vec<symbols::SymbolHandle>>,
    ) -> Result<SelectedCheckedSource, Vec<Diagnostic>> {
        if let Some(prior) = self
            .entries
            .iter()
            .find(|prior| Arc::ptr_eq(&prior.input, &checked.program) && prior.roots == roots)
        {
            let mut selected = prior.output.clone();
            selected.retained = checked.retained;
            return Ok(selected);
        }
        let input = checked.program.clone();
        let output = checked.select_product(roots.clone())?;
        self.entries.push(SelectedProduct {
            input,
            roots,
            output: output.clone(),
        });
        Ok(output)
    }
}
