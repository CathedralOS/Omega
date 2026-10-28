//! Canonical fresh Build outputs used to qualify a retained native component.
//!
//! The enclosing cache key already binds compiler/library, complete package
//! sources, dependency contexts, snapshot and generated bundles. This digest
//! binds what the current activation actually returned, including source-issued
//! requests whose Product declarations have not yet been admitted. It is only
//! an equality key: none of its coordinates supplies declaration authority.

use super::PreparedPackageBuild;
use crate::{BuildAdmission, BuildConfig, ComputedBuildConfig};
use build_time_evaluation::provider_declarations::{
    ProviderSelectionIdentity, ProviderSelectionSubject,
};
use language_semantics::declaration_selection::{
    BuildOperation, CompositionMode, ExclusionVocabulary, ProductDeclarationRequest,
    RootBindingSelection,
};
use optimization_core::digest_encoder::{DomainDigestEncoder, content_identity};
use source::{DependencyScope, SourceOrigin, SourceResolutionStratum, SourceSpan};
use std::collections::HashMap;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;

impl PreparedPackageBuild {
    /// Exact semantic output of this freshly settled Build. Invocation-local
    /// arena numbers, source numbers and execution telemetry never enter it.
    pub fn native_product_input_identity(&self) -> Result<[u8; 32], String> {
        let mut identity = BuildOutputIdentity {
            program: self.prepared.built.source.typed(),
            digest: DomainDigestEncoder::new(b"omega-native-product-build-outputs-v8\0"),
            source_contents: HashMap::new(),
        };
        identity.computed(&self.prepared.built.computed_build_config)?;
        identity
            .digest
            .len(self.inputs.dependency_build_handoff_instances().count());
        for (scope, bundle) in self.inputs.dependency_build_handoff_instances() {
            identity.digest.byte(match scope {
                build_declarations::DependencyPurpose::Product => 1,
                build_declarations::DependencyPurpose::Build => 2,
            });
            identity.package(Some(bundle.package()));
            identity
                .digest
                .raw(&package_compilation::native_output_codec::digest(bundle));
        }
        identity.admission(&self.prepared.built.admission);
        Ok(identity.digest.finish())
    }
}

struct BuildOutputIdentity<'program> {
    program: &'program TypedTrees,
    digest: DomainDigestEncoder,
    // Construction scratch only. Retained declarations can come from distinct
    // forests with equal numeric SourceIds, so use their immutable source text.
    // Only the content commitment is written to the durable key.
    source_contents: HashMap<std::sync::Arc<str>, [u8; 32]>,
}

impl BuildOutputIdentity<'_> {
    fn computed(&mut self, value: &ComputedBuildConfig) -> Result<(), String> {
        let ComputedBuildConfig {
            config,
            root_binding_requests,
            declared_product_selections,
            product_selection_requests,
            native_binding_requests,
            behavior_exclusion_requests,
            project_declaration,
            optimization_report_request,
            // These are fresh execution receipts, not native Product choices.
            // The caller has already admitted/charged this activation; a cache
            // hit must never replace its observations or sponsor accounting.
            evaluation_usage: _,
            observation_summary: _,
            selected_build_machine_symbol,
            generated_sources,
        } = value;
        self.config(config)?;
        self.digest.len(root_binding_requests.len());
        for request in root_binding_requests {
            let language_semantics::declaration_selection::ExecutedRootBinding {
                source_span,
                slot,
                implementation,
            } = request;
            self.occurrence(*source_span)?;
            self.digest.string(slot);
            match implementation {
                RootBindingSelection::AuthoredPath(path) => {
                    self.digest.byte(0);
                    self.digest.string(path);
                }
                RootBindingSelection::Described(description) => {
                    self.digest.byte(1);
                    let language_semantics::declaration_selection::DescribedProductEntry {
                        declaration,
                        machine_name,
                        slot,
                    } = description;
                    self.digest.byte(symbol_kind_tag(declaration.kind()));
                    self.source_coordinate(declaration.source(), declaration.span())?;
                    // Consume the producer-issued exact declaration join. A
                    // display path or matching source span is never a fallback.
                    let selected = declaration
                        .rejoin(
                            &self.program.symbols,
                            self.program.machines().iter().map(|machine| machine.symbol),
                        )
                        .map_err(str::to_owned)?;
                    self.symbol(selected)?;
                    self.digest.string(machine_name);
                    self.digest.string(slot);
                }
            }
        }
        self.digest.len(declared_product_selections.len());
        for request in declared_product_selections {
            self.product_request(request)?;
        }
        self.digest.len(product_selection_requests.len());
        for selection in product_selection_requests {
            let language_semantics::declaration_selection::ExecutedProductSelection {
                request,
                selecting_machine,
                composition,
                target,
            } = selection;
            self.product_request(request)?;
            self.product_target(*target);
            self.optional_occurrence(*selecting_machine)?;
            self.digest.bool(composition.is_some());
            if let Some(mode) = composition {
                self.composition(*mode);
            }
        }
        self.digest.len(native_binding_requests.len());
        for event in native_binding_requests {
            self.product_request(&event.request)?;
            self.occurrence(event.selecting_machine)?;
            self.product_target(Some(event.target));
            match &event.static_environment {
                None => self.digest.byte(0),
                Some(environment) => {
                    if environment.identity.is_empty() {
                        return Err(
                            "native helper environment lost its specialization identity".into()
                        );
                    }
                    self.digest.byte(1);
                    self.digest.bytes(&environment.identity);
                }
            }
            for width in event.widths {
                self.digest.u64(width);
            }
            self.native_locator(&event.locator);
        }
        self.digest.len(behavior_exclusion_requests.len());
        for request in behavior_exclusion_requests {
            let language_semantics::declaration_selection::ExecutedEnumExclusion {
                request,
                code,
                selecting_machine,
            } = request;
            let language_semantics::declaration_selection::EnumExclusionRequest {
                source_span,
                vocabulary,
            } = request;
            self.occurrence(*source_span)?;
            self.digest.byte(match vocabulary {
                ExclusionVocabulary::CrashCause => 0,
                ExclusionVocabulary::PhysicalAuthorityClass => 1,
            });
            self.digest.u64(*code);
            self.occurrence(*selecting_machine)?;
        }
        self.declaration(project_declaration.as_ref());
        self.digest.byte(match optimization_report_request {
            optimization_core::OptimizationReportRequest::Suppressed => 0,
            optimization_core::OptimizationReportRequest::EmitHumanText => 1,
        });
        self.digest.bool(selected_build_machine_symbol.is_some());
        if let Some(symbol) = selected_build_machine_symbol {
            self.symbol(*symbol)?;
        }
        // The enclosing key also retains each sealed handoff's dependency and
        // activation custody. Preserve this returned roster and its order too.
        self.digest.len(generated_sources.len());
        for source in generated_sources {
            self.digest.bytes(source.relative_path());
            self.digest.raw(&source.digest());
        }
        Ok(())
    }

    fn config(&mut self, config: &BuildConfig) -> Result<(), String> {
        let BuildConfig {
            settings,
            grants,
            provider_selections,
            admitted_native_bindings,
            native_binding_evaluation,
            package_provider_selections,
            assumed_bindings,
            opaque_representation_selections,
            wire_compatibility_demands,
            root_bindings,
            behavior_exclusions,
            service_audits,
            accepted_component_assumptions,
        } = config;
        self.settings(settings);
        self.digest.len(grants.len());
        for grant in grants {
            let trust_model::AuthoredRootGrant {
                selector,
                selecting_machine,
                source_span,
            } = grant;
            self.digest.string(selector);
            self.symbol(*selecting_machine)?;
            self.occurrence(*source_span)?;
        }
        for selections in [provider_selections, package_provider_selections] {
            self.digest.len(selections.len());
            for selection in selections {
                let provider_planning::ProviderSelection {
                    subject,
                    provider_type,
                    composition_mode,
                    target,
                    origin,
                } = selection;
                match subject {
                    ProviderSelectionSubject::BoundaryTrait(identity) => {
                        self.digest.byte(0);
                        self.provider_identity(identity)?;
                    }
                    ProviderSelectionSubject::BoundaryRequirement(identity) => {
                        self.digest.byte(1);
                        self.provider_identity(identity)?;
                    }
                    ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                        self.digest.byte(2);
                        self.package(family.package);
                        self.digest.string(&family.canonical_path);
                        self.digest.string(&family.authored_path);
                        self.digest.len(family.coordinates().len());
                        for coordinate in family.coordinates() {
                            self.symbol(coordinate.symbol)?;
                            self.digest.string(&coordinate.requirement_identity);
                            self.digest.len(coordinate.static_parameter_count);
                        }
                    }
                }
                self.provider_identity(provider_type)?;
                self.product_target(*target);
                self.composition(*composition_mode);
                for declaration in [origin.activation(), origin.author()] {
                    self.digest.byte(symbol_kind_tag(declaration.kind()));
                    self.source_coordinate(declaration.source(), declaration.span())?;
                }
                self.digest.len(origin.occurrence().start);
                self.digest.len(origin.occurrence().end);
                self.digest.bool(origin.event_ordinal().is_some());
                if let Some(ordinal) = origin.event_ordinal() {
                    self.digest.u64(ordinal);
                }
            }
        }
        self.digest.bool(native_binding_evaluation.is_some());
        if let Some(evaluation) = native_binding_evaluation {
            self.digest.raw(&evaluation.identity_digest());
        }
        self.digest.len(admitted_native_bindings.len());
        for row in admitted_native_bindings {
            self.symbol(row.requirement)?;
            self.digest.bool(row.selecting_machine.is_some());
            if let Some(author) = row.selecting_machine {
                self.symbol(author)?;
            }
            self.occurrence(row.source_span)?;
            if let Some(retained) = &row.retained {
                self.digest.raw(
                    &package_compilation::native_selection_codec::package_native_bindings_digest(
                        std::slice::from_ref(retained.as_ref()),
                        None,
                    ),
                );
            }
            self.digest.len(row.event_ordinal);
            self.digest.string(row.target.target_name());
            self.digest.raw(&row.receipt().identity_digest());
        }
        self.digest.len(assumed_bindings.len());
        for binding in assumed_bindings {
            let build_time_evaluation::assumed_bindings::AssumedBindingContract {
                required,
                supplied,
                selecting_machine,
                source_span,
            } = binding;
            self.symbol(*required)?;
            self.symbol(*supplied)?;
            self.symbol(*selecting_machine)?;
            self.occurrence(*source_span)?;
        }
        self.digest.len(opaque_representation_selections.len());
        for selection in opaque_representation_selections {
            self.symbol(selection.opaque())?;
            self.symbol(selection.carrier())?;
            let carrier = self
                .program
                .package_qualified_type_identity(selection.carrier_type());
            let carrier = self
                .program
                .type_identity_text(carrier)
                .ok_or("selected representation has no complete carrier type identity")?;
            self.digest.string(&carrier);
            self.symbol(selection.application().declaration)?;
            self.symbol(selection.application().trait_definition)?;
            self.digest
                .raw(&selection.application().commitment.as_bytes());
            self.digest.u16(selection.schema_version());
            self.digest
                .raw(&selection.selected_application_commitment());
            self.digest
                .raw(&selection.rederived_selected_application_commitment());
            self.symbol(selection.selecting_machine())?;
            self.occurrence(selection.source_span())?;
        }
        self.digest.len(wire_compatibility_demands.len());
        for demand in wire_compatibility_demands {
            let build_time_evaluation::WireCompatibilityDemand {
                edge,
                lineage,
                local_schema,
                peer_schema,
                require_readable,
                require_writable,
                require_unknown_preservation,
                require_canonical,
                require_complete_migration,
            } = demand;
            self.digest.string(edge);
            self.digest.string(lineage);
            self.digest.string(local_schema);
            self.digest.string(peer_schema);
            for flag in [
                require_readable,
                require_writable,
                require_unknown_preservation,
                require_canonical,
                require_complete_migration,
            ] {
                self.digest.bool(*flag);
            }
        }
        self.digest.len(root_bindings.len());
        for binding in root_bindings {
            let provider_planning::program_entry::RootBinding {
                slot,
                implementation,
                implementation_symbol,
            } = binding;
            self.digest.string(slot);
            self.digest.string(implementation);
            self.symbol(*implementation_symbol)?;
        }
        self.digest.len(behavior_exclusions.len());
        for exclusion in behavior_exclusions {
            let effects::AuthoredBehaviorExclusion {
                kind,
                selecting_machine,
                source_span,
            } = exclusion;
            match kind {
                effects::AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol } => {
                    self.digest.byte(0);
                    self.digest.byte(match cause {
                        terminal_psi::CrashCause::Trap => 0,
                        terminal_psi::CrashCause::Abort => 1,
                    });
                    self.symbol(*case_symbol)?;
                }
                effects::AuthoredBehaviorExclusionKind::PhysicalAuthorityClass {
                    class,
                    case_symbol,
                } => {
                    self.digest.byte(1);
                    self.digest.byte(class.canonical_tag());
                    self.symbol(*case_symbol)?;
                }
                effects::AuthoredBehaviorExclusionKind::Service { identity, name } => {
                    self.digest.byte(2);
                    self.digest.string(identity);
                    self.digest.string(name);
                }
            }
            self.symbol(*selecting_machine)?;
            self.occurrence(*source_span)?;
        }
        self.digest.len(service_audits.len());
        for audit in service_audits {
            let crate::ServiceAuditRegistration { identity, name } = audit;
            self.digest.string(identity);
            self.digest.string(name);
        }
        self.digest.len(accepted_component_assumptions.len());
        for acceptance in accepted_component_assumptions {
            let trust_model::AuthoredComponentAssumptionAcceptance {
                digest,
                selecting_machine,
                source_span,
            } = acceptance;
            self.digest.raw(digest);
            self.symbol(*selecting_machine)?;
            self.occurrence(*source_span)?;
        }
        Ok(())
    }

    fn settings(
        &mut self,
        settings: &package_compilation::build_execution::build_value::EvaluatedBuildSettings,
    ) {
        let package_compilation::build_execution::build_value::EvaluatedBuildSettings {
            application_intent,
            application_identifier,
            subsystem,
            freestanding,
            optimizations,
            x86_deployment_claim,
            privileged_services,
            elf_interpreters,
        } = settings;
        self.digest.bool(application_intent.is_some());
        if let Some(intent) = application_intent {
            self.digest.byte(match intent {
                target::HostedApplicationIntent::Console => 0,
                target::HostedApplicationIntent::Gui => 1,
            });
        }
        self.digest.optional_string(
            application_identifier
                .as_ref()
                .map(|identity| identity.as_str()),
        );
        self.digest.u16(*subsystem);
        self.digest.bool(*freestanding);
        self.digest.bytes(&optimizations.encode());
        // This is the source enum's value discriminant, not its memory layout
        // or an invocation handle. The enclosing compiler identity pins this
        // private vocabulary, including its discriminant assignments.
        self.digest.u32(*x86_deployment_claim as u32);
        let package_compilation::build_execution::build_value::PrivilegedServicesGrants {
            port_io,
            interrupt_table,
        } = privileged_services;
        self.digest.bool(*port_io);
        self.digest.bool(*interrupt_table);
        // The owner constructs explicit rows only by iterating this complete
        // catalog, in this order. Do not collapse authored rows with defaults.
        self.digest
            .len(target::ELF_INTERPRETER_TARGET_PROFILES.len());
        for target in target::ELF_INTERPRETER_TARGET_PROFILES {
            self.digest.string(target.target_name());
            let selection = elf_interpreters.explicit(target);
            self.digest.bool(selection.is_some());
            if let Some(selection) = selection {
                self.digest.string(selection.target().target_name());
                self.digest.bytes(selection.interpreter_path());
            }
        }
    }

    fn admission(&mut self, admission: &BuildAdmission) {
        let BuildAdmission {
            application,
            selected_build_machine_identity,
            restricted_build_requests,
        } = admission;
        self.digest.bool(application.is_some());
        if let Some(application) = application {
            let build_declarations::ApplicationDeclaration {
                name,
                artifact_only,
            } = application;
            self.digest.string(name.as_str());
            self.digest.bool(*artifact_only);
        }
        self.digest
            .optional_string(selected_build_machine_identity.as_deref());
        self.digest.len(restricted_build_requests.len());
        let mut encoded = Vec::new();
        for request in restricted_build_requests {
            encoded.clear();
            package_compilation::build_execution::build_record_codec::encode_restricted_build_request(request, &mut encoded);
            self.digest.bytes(&encoded);
        }
    }

    fn declaration(&mut self, declaration: Option<&build_declarations::BuildDeclaration>) {
        use build_declarations::BuildDeclaration;
        self.digest.bool(declaration.is_some());
        match declaration {
            None => {}
            Some(BuildDeclaration::Package(package)) => {
                self.digest.byte(0);
                let build_declarations::PackageDeclaration { name } = package;
                self.digest.string(name.as_str());
            }
            Some(BuildDeclaration::Application(application)) => {
                self.digest.byte(1);
                let build_declarations::ApplicationDeclaration {
                    name,
                    artifact_only,
                } = application;
                self.digest.string(name.as_str());
                self.digest.bool(*artifact_only);
            }
            Some(BuildDeclaration::Workspace(workspace)) => {
                self.digest.byte(2);
                let build_declarations::WorkspaceDeclaration { members } = workspace;
                self.digest.len(members.len());
                for member in members {
                    self.digest.string(member.as_str());
                }
            }
        }
    }

    fn product_request(&mut self, request: &ProductDeclarationRequest) -> Result<(), String> {
        let ProductDeclarationRequest {
            source_span,
            operation,
            operands,
        } = request;
        self.occurrence(*source_span)?;
        self.digest.byte(match operation {
            BuildOperation::ProviderReadiness => 0,
            BuildOperation::ProviderSelection => 1,
            BuildOperation::AssumedBindingContract => 2,
            BuildOperation::RepresentationSelection => 3,
            BuildOperation::ServiceExclusion => 4,
            BuildOperation::ServiceAudit => 5,
            BuildOperation::OptimizationSelection => 6,
            BuildOperation::OptimizationReportRequest => 7,
            BuildOperation::BoundaryAcceptance => 8,
            BuildOperation::WireCompatibilityRequest => 9,
            BuildOperation::IncludedSourceHandoff => 10,
            BuildOperation::LogWriteLine => 11,
            BuildOperation::NativeBinding => 12,
            BuildOperation::NativeTableFunction => 13,
            BuildOperation::NativeVtableField => 14,
            BuildOperation::NativeSameStackDemand => 15,
        });
        self.digest.len(operands.len());
        for operand in operands {
            let language_semantics::declaration_selection::ProductDeclarationOperand {
                path,
                source_span,
                application,
            } = operand;
            self.digest.string(path);
            // The occurrence commits its immutable complete source bytes; the
            // graph is a deterministic source capture, not another authority.
            self.digest.byte(u8::from(application.is_some()));
            self.occurrence(*source_span)?;
        }
        Ok(())
    }

    fn native_locator(
        &mut self,
        value: &language_semantics::declaration_selection::NativeBindingValue,
    ) {
        use language_semantics::declaration_selection::NativeBindingValue as V;
        match value {
            V::PeByName { library, export } => {
                self.digest.byte(0);
                self.digest.bytes(library);
                self.digest.bytes(export);
            }
            V::PeByOrdinal { library, ordinal } => {
                self.digest.byte(1);
                self.digest.bytes(library);
                self.digest.u64(u64::from(*ordinal));
            }
            V::ElfVersioned {
                object,
                symbol,
                version,
            } => {
                self.digest.byte(2);
                self.digest.bytes(object);
                self.digest.bytes(symbol);
                self.digest.bytes(version);
            }
            V::MachODylibSymbol {
                install_name,
                symbol,
            } => {
                self.digest.byte(3);
                self.digest.bytes(install_name);
                self.digest.bytes(symbol);
            }
            V::Syscall { number } => {
                self.digest.byte(4);
                self.digest.u64(*number);
            }
            V::TableFunction => self.digest.byte(5),
            V::VtableField => self.digest.byte(6),
            V::SameStackDemand { bytes } => {
                self.digest.byte(7);
                self.digest.u64(*bytes);
            }
        }
    }

    fn provider_identity(&mut self, identity: &ProviderSelectionIdentity) -> Result<(), String> {
        let ProviderSelectionIdentity {
            symbol,
            package,
            canonical_path,
            authored_path,
        } = identity;
        self.symbol(*symbol)?;
        self.package(*package);
        self.digest.string(canonical_path);
        self.digest.string(authored_path);
        Ok(())
    }

    fn product_target(
        &mut self,
        target: Option<language_semantics::declaration_selection::ProductTarget>,
    ) {
        self.digest.bool(target.is_some());
        if let Some(target) = target {
            self.digest.string(target.canonical_name());
        }
    }

    fn composition(&mut self, mode: CompositionMode) {
        self.digest.byte(match mode {
            CompositionMode::Fused => 0,
            CompositionMode::Independent => 1,
        });
    }

    fn package(&mut self, package: Option<semantic_vocabulary::PackageKeyIdentity>) {
        self.digest.bool(package.is_some());
        if let Some(package) = package {
            self.digest.raw(&package.digest());
        }
    }

    fn optional_occurrence(&mut self, span: Option<SourceSpan>) -> Result<(), String> {
        self.digest.bool(span.is_some());
        if let Some(span) = span {
            self.occurrence(span)?;
        }
        Ok(())
    }

    fn occurrence(&mut self, span: SourceSpan) -> Result<(), String> {
        // Rejoin only through the producer's explicit SourceMap correspondence.
        // Retained Build requests can precede anonymous-to-final Product sealing.
        let program = self.program;
        let span = program.symbols.rejoin_product_occurrence(span);
        let file = program
            .symbols
            .source_file(span)
            .ok_or("Build output occurrence has no retained source owner")?;
        self.source_coordinate(file, span.span)
    }

    fn source_coordinate(
        &mut self,
        file: &source::SourceFile,
        span: source::Span,
    ) -> Result<(), String> {
        let source::SourceFile {
            source_id: _,
            path,
            package_root,
            package_identity,
            dependency_scope,
            origin,
            resolution_stratum,
            source,
        } = file;
        if span.start > span.end || span.end > source.len() {
            return Err("Build output occurrence is outside its retained source bytes".to_owned());
        }
        self.package(*package_identity);
        self.digest.byte(match origin {
            SourceOrigin::User => 0,
            SourceOrigin::Toolchain => 1,
        });
        self.digest.byte(match dependency_scope {
            DependencyScope::Product => 0,
            DependencyScope::Build => 1,
        });
        self.digest.byte(match resolution_stratum {
            SourceResolutionStratum::Base => 0,
            SourceResolutionStratum::CurrentActivationExtension => 1,
            SourceResolutionStratum::DependencyActivationExtension => 2,
        });
        let relative = path
            .strip_prefix(package_root)
            .map_err(|_| "Build output source is outside its retained package root")?;
        self.digest.bytes(relative.as_os_str().as_encoded_bytes());
        let content = self
            .source_contents
            .entry(source.clone())
            .or_insert_with(|| content_identity(source.as_bytes()));
        self.digest.raw(content);
        self.digest.len(span.start);
        self.digest.len(span.end);
        Ok(())
    }

    fn symbol(&mut self, symbol: SymbolHandle) -> Result<(), String> {
        self.digest.bool(symbol.is_valid());
        if !symbol.is_valid() {
            return Ok(());
        }
        let nominal = self.program.normalized_hermetic_symbol_identity(symbol)?;
        self.digest.string(&nominal);
        self.digest
            .byte(symbol_kind_tag(self.program.symbols.get(symbol).kind));
        let occurrence = self.program.symbols.symbol_provenance_source_span(symbol);
        self.optional_occurrence(occurrence)?;
        // A nominal machine name alone does not identify its overload/static
        // application. Use the same canonical callable identity as admission.
        if let Some(machine) = self.program.machine(symbol) {
            self.digest.byte(1);
            let callable = self
                .program
                .machine_callable_identity_text(machine)
                .ok_or("Build output machine has no complete callable identity")?;
            self.digest.string(&callable);
        } else {
            self.digest.byte(0);
        }
        Ok(())
    }
}

fn symbol_kind_tag(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Unknown => 0,
        SymbolKind::Root => 1,
        SymbolKind::Module => 2,
        SymbolKind::BuiltinType => 3,
        SymbolKind::BuiltinFunction => 4,
        SymbolKind::Data => 5,
        SymbolKind::Domain => 6,
        SymbolKind::Field => 7,
        SymbolKind::Variant => 8,
        SymbolKind::Machine => 9,
        SymbolKind::Operator => 10,
        SymbolKind::Measure => 11,
        SymbolKind::Proposition => 12,
        SymbolKind::MathematicalDefinition => 13,
        SymbolKind::State => 14,
        SymbolKind::Trait => 15,
        SymbolKind::Conformance => 16,
        SymbolKind::Const => 17,
        SymbolKind::ConformanceParameter => 18,
        SymbolKind::WireSchema => 19,
        SymbolKind::Parameter => 20,
        SymbolKind::TypeParameter => 21,
        SymbolKind::MachineParameter => 22,
        SymbolKind::PropositionParameter => 23,
        SymbolKind::PropositionMachineParameter => 24,
        SymbolKind::Local => 25,
        SymbolKind::HostCapability => 26,
        SymbolKind::Object => 27,
        SymbolKind::Function => 28,
        SymbolKind::Section => 29,
        SymbolKind::Import => 30,
    }
}
