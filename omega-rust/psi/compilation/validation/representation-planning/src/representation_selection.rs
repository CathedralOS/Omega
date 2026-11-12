/*
Opaque carrier formation (`form_opaque_representation_request`), authoritative
build selection harvesting (`collect_opaque_representation_selections`), final
typed-graph rederivation (`rederive_opaque_representation_selections`), and
complete carrier inertness and copy closure (`carrier_closure`).
*/

use crate::representation_trait::is_compiler_owned_opaque_representation_trait;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};
use representation_selections::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
    OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
};
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::product_operand::{ProductOperandKind, resolve_product_path};
use typed_trees::{TypedTrees, typed_trees::ClosedConformanceApplication};

pub(crate) mod carrier_closure {
    use diagnostics::Diagnostic;
    use language_semantics::{DataSupplyMode, Multiplicity};
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::TypedTrees;
    use typed_trees::data::{DataDefinition, DataMember};
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    use representation_selections::OpaqueRepresentationCopyDisposition;

    pub(crate) fn validate_inert_carrier(
        program: &TypedTrees,
        opaque: &DataDefinition,
        carrier: TypeReferenceHandle,
    ) -> Result<OpaqueRepresentationCopyDisposition, Diagnostic> {
        let copy_disposition = if opaque.properties.multiplicity == Multiplicity::Unrestricted {
            OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        } else {
            OpaqueRepresentationCopyDisposition::PlacementOnly
        };
        CarrierClosure {
            program,
            copy_disposition,
            visiting: Vec::new(),
            validated: Vec::new(),
            scopes: validation::DeclarationScopes::default(),
            applications: Vec::new(),
        }
        .validate_type(carrier, 0, &program.display_type_reference(carrier))?;
        Ok(copy_disposition)
    }

    pub(crate) struct CarrierClosure<'program> {
        program: &'program TypedTrees,
        copy_disposition: OpaqueRepresentationCopyDisposition,
        visiting: Vec<typed_trees::type_identity::TypeIdentityId>,
        validated: Vec<typed_trees::type_identity::TypeIdentityId>,
        scopes: validation::DeclarationScopes,
        applications: Vec<(SymbolHandle, TypeReferenceHandle, usize)>,
    }

    impl CarrierClosure<'_> {
        fn validate_definition(
            &mut self,
            definition: &DataDefinition,
            identity: typed_trees::type_identity::TypeIdentityId,
            scope: usize,
            path: &str,
        ) -> Result<(), Diagnostic> {
            if self.validated.contains(&identity) {
                return Ok(());
            }
            if self.visiting.contains(&identity) {
                return Err(self.reject(path, "contains a recursive by-value carrier cycle"));
            }
            if definition.supply_mode != DataSupplyMode::CheckedShape {
                return Err(self.reject(
                    path,
                    "contains boundary-opaque data without its own joined representation",
                ));
            }
            if definition.properties.multiplicity == Multiplicity::Linear {
                return Err(self.reject(path, "contains live linear ownership debt"));
            }
            if self.copy_disposition == OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
                && definition.properties.multiplicity != Multiplicity::Unrestricted
            {
                return Err(self.reject(
                path,
                "is affine; a copyable opaque datum requires every carrier declaration to be structurally `[copy]`",
            ));
            }
            if definition.cleanup_selection.is_some() {
                return Err(self.reject(path, "contains independently invoked nominal cleanup"));
            }

            self.visiting.push(identity);
            for member in self.program.data_members(definition) {
                match member {
                    DataMember::Field(field) if !field.relevance.is_erased() => {
                        self.validate_type(
                            field.type_reference,
                            scope,
                            &format!("{path}.{}", field.name),
                        )?;
                    }
                    DataMember::Variant(variant) => {
                        for field in self.program.data_payload_fields(variant) {
                            if field.relevance.is_erased() {
                                continue;
                            }
                            self.validate_type(
                                field.type_reference,
                                scope,
                                &format!("{path}::{}({})", variant.name, field.name),
                            )?;
                        }
                    }
                    DataMember::Field(_) => {}
                }
            }
            let removed = self.visiting.pop();
            debug_assert_eq!(removed, Some(identity));
            self.validated.push(identity);
            Ok(())
        }

        fn validate_type(
            &mut self,
            type_reference: TypeReferenceHandle,
            scope: usize,
            path: &str,
        ) -> Result<(), Diagnostic> {
            let (type_reference, scope) = self.scopes.head(self.program, type_reference, scope);
            if self
                .program
                .primitive_type_reference(type_reference)
                .is_some()
            {
                return Ok(());
            }
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Constrained { base_type, .. } => {
                    self.validate_type(*base_type, scope, path)
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    if !matches!(length, FixedArrayLength::Literal(_)) {
                        return Err(self.reject(path, "contains an unresolved array extent"));
                    }
                    self.validate_type(*element_type, scope, &format!("{path}[]"))
                }
                TypeReferenceNode::Named { symbol, .. }
                    if self.program.symbols.get(*symbol).kind == SymbolKind::BuiltinType =>
                {
                    Ok(())
                }
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    let identity = self
                        .scopes
                        .type_identity(self.program, type_reference, scope);
                    let (definition, field_scope) = self
                        .scopes
                        .definition(self.program, type_reference, scope)
                        .ok_or_else(|| {
                            self.reject(path, "contains data without an exact closed application")
                        })?;
                    let ancestors = std::mem::take(&mut self.applications);
                    self.applications.extend(ancestors.iter().copied().filter(
                        |(_, parent, parent_scope)| {
                            !self.scopes.is_strict_argument(
                                self.program,
                                *parent,
                                *parent_scope,
                                identity,
                            )
                        },
                    ));
                    let result = if self
                        .applications
                        .iter()
                        .any(|(symbol, _, _)| *symbol == definition.symbol)
                    {
                        Err(self.reject(path, "contains a recursive by-value carrier family"))
                    } else {
                        self.applications
                            .push((definition.symbol, type_reference, scope));
                        self.validate_definition(definition, identity, field_scope, path)
                    };
                    self.applications = ancestors;
                    result
                }
                TypeReferenceNode::Reference { .. } => {
                    Err(self.reject(path, "contains borrowed or external storage"))
                }
                TypeReferenceNode::Slice { .. } => {
                    Err(self.reject(path, "contains dynamically sized external storage"))
                }
                TypeReferenceNode::DynamicTrait { .. } => {
                    Err(self.reject(path, "contains dynamic external provider state"))
                }
                TypeReferenceNode::ConstExpression(_) => Err(self.reject(
                    path,
                    "contains a proof-static expression as runtime storage",
                )),
                TypeReferenceNode::Unit => Ok(()),
            }
        }

        fn reject(&self, path: &str, reason: &str) -> Diagnostic {
            Diagnostic::error(format!(
                "opaque representation carrier graph at `{path}` {reason}; v1 carriers must be inert storage"
            ))
        }
    }
}

#[derive(Clone)]
pub struct FormedOpaqueRepresentation<'program> {
    _program: &'program TypedTrees,
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    carrier_type: typed_trees::types::TypeReferenceHandle,
    application: ClosedConformanceApplication,
    copy_disposition: OpaqueRepresentationCopyDisposition,
}

impl FormedOpaqueRepresentation<'_> {
    fn activate(self, activation: SymbolHandle, span: SourceSpan) -> OpaqueRepresentationSelection {
        OpaqueRepresentationSelection::from_validated_application(
            self.opaque,
            self.carrier,
            self.carrier_type,
            self.application,
            OpaqueRepresentationLifecycleDisposition::Inert,
            self.copy_disposition,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
            activation,
            span,
        )
    }
}

// A failed name lookup must show which checked source supplied its names.
// These rows explain the refusal only; they never choose a declaration or
// turn a dependency's private declaration into a visible one.
fn representation_resolution_context(
    typed: &TypedTrees,
    path: &str,
    occurrence: SourceSpan,
) -> String {
    use std::fmt::Write;

    const ROW_LIMIT: usize = 4;
    const TEXT_LIMIT: usize = 4096;
    let program = typed.product_declarations();
    let selected = program.symbols.rejoin_product_occurrence(occurrence);
    let alias = path.split_once("::").map(|(alias, _)| alias);
    let target = alias.and_then(|alias| program.symbols.product_dependency_target(selected, alias));
    let author = program.symbols.source_file(selected);
    let mut text = format!(
        "\nrepresentation lookup context: occurrence={occurrence:?} rejoined={selected:?} alias={alias:?} target={target:?} lookup_conformances={} current_conformances={} requester={:?}",
        program.conformances().len(),
        typed.conformances().len(),
        author.map(|file| (
            file.package_identity,
            file.dependency_scope,
            file.resolution_stratum,
            file.origin,
            &file.path,
        )),
    );
    let leaf = path.rsplit("::").next().unwrap_or(path);
    let mut shown = 0;
    let mut omitted = false;
    let current = (!std::ptr::eq(program, typed)).then_some(("current", typed));
    'tables: for (label, table) in std::iter::once(("lookup", program)).chain(current) {
        for conformance in table.conformances() {
            if !conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str().rsplit("::").next() == Some(leaf))
            {
                continue;
            }
            if shown == ROW_LIMIT {
                omitted = true;
                break 'tables;
            }
            shown += 1;
            let symbol = conformance.symbol;
            let source = symbol
                .is_valid()
                .then(|| table.symbols.symbol_provenance_source_span(symbol))
                .flatten()
                .and_then(|span| table.symbols.source_file(span));
            let _ = write!(
                text,
                "\ncandidate table={label} symbol={symbol:?} path={:?} public={} source_selected={} product_package={:?} source={:?}",
                table.symbols.display_path(symbol, "::"),
                conformance.is_public,
                symbol.is_valid() && table.symbols.symbol_source_is_selected(symbol),
                symbol
                    .is_valid()
                    .then(|| table.symbols.symbol_product_package_identity(symbol))
                    .flatten(),
                source.map(|file| (
                    file.package_identity,
                    file.dependency_scope,
                    file.resolution_stratum,
                    file.origin,
                    &file.path,
                )),
            );
        }
    }
    let candidate_rows = shown;
    let mut inventory_counts = String::new();
    // A declaration can be missing because its source was never opened or
    // because a later declaration projection omitted it. Report the retained
    // source and symbol inventories without loading or resolving anything.
    let source_stem = path
        .rsplit_once("::")
        .map(|(namespace, _)| namespace.rsplit("::").next().unwrap_or(namespace));
    let current = (!std::ptr::eq(program, typed)).then_some(("current", typed));
    for (label, table) in std::iter::once(("lookup", program)).chain(current) {
        let mut package_sources = 0;
        let mut matching_sources = 0;
        for file in table.symbols.source_files() {
            if target.is_none() || file.package_identity != target {
                continue;
            }
            package_sources += 1;
            if file.path.file_stem().and_then(|stem| stem.to_str()) != source_stem {
                continue;
            }
            matching_sources += 1;
            if shown == ROW_LIMIT {
                omitted = true;
                continue;
            }
            shown += 1;
            let _ = write!(
                text,
                "\nsource table={label} id={:?} package={:?} scope={:?} stratum={:?} origin={:?} path={:?}",
                file.source_id,
                file.package_identity,
                file.dependency_scope,
                file.resolution_stratum,
                file.origin,
                file.path,
            );
        }
        let mut matching_symbols = 0;
        for (symbol, declaration) in table.symbols.symbols().nodes().iter() {
            if table.symbols.name(symbol) != leaf {
                continue;
            }
            matching_symbols += 1;
            if shown == ROW_LIMIT {
                omitted = true;
                continue;
            }
            shown += 1;
            let active_rows = table
                .conformances()
                .iter()
                .filter(|conformance| conformance.symbol == symbol)
                .count();
            let stored_rows = table
                .tables
                .conformances
                .iter()
                .filter(|(_, conformance)| conformance.symbol == symbol)
                .count();
            let _ = write!(
                inventory_counts,
                "\nconformance_membership table={label} symbol={symbol:?} active_rows={active_rows} stored_rows={stored_rows}"
            );
            let source = table
                .symbols
                .symbol_provenance_source_span(symbol)
                .and_then(|span| table.symbols.source_file(span));
            let _ = write!(
                text,
                "\nsymbol table={label} handle={symbol:?} kind={:?} parent={:?} generated_from={:?} path={:?} source_selected={} source={:?}",
                declaration.kind,
                declaration.parent,
                declaration.generated_from,
                table.symbols.display_path(symbol, "::"),
                table.symbols.symbol_source_is_selected(symbol),
                source.map(|file| (
                    file.source_id,
                    file.package_identity,
                    file.dependency_scope,
                    &file.path
                )),
            );
        }
        let _ = write!(
            inventory_counts,
            "\ninventory table={label} target_selected_sources={package_sources} matching_sources={matching_sources} exact_leaf_symbols={matching_symbols}"
        );
    }
    let _ = write!(
        text,
        "\ncandidate_rows={candidate_rows} inventory_and_candidate_rows={shown} row_limit={ROW_LIMIT} additional_rows_omitted={omitted}"
    );
    // Keep the complete counts visible even when detailed rows fill the text limit.
    text.insert_str(0, &inventory_counts);
    if text.len() > TEXT_LIMIT {
        const MARKER: &str = "\n[representation lookup context text truncated]";
        let mut end = TEXT_LIMIT - MARKER.len();
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(MARKER);
    }
    text
}

pub fn form_opaque_representation_request<'p>(
    typed: &'p TypedTrees,
    request: &ProductDeclarationRequest,
) -> Result<FormedOpaqueRepresentation<'p>, Diagnostic> {
    if request.operation != BuildOperation::RepresentationSelection {
        return Err(Diagnostic::error(
            "opaque formation requires its exact representation request",
        ));
    }
    let [opaque_operand, conformance_operand] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "opaque representation request lost its two declaration operands",
        ));
    };
    let opaque = resolve_product_path(
        typed,
        &opaque_operand.path,
        opaque_operand.source_span,
        ProductOperandKind::Data,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "opaque representation subject `{}` does not resolve to one visible data declaration",
            opaque_operand.path
        ))
        .with_source_span(opaque_operand.source_span)
    })?;
    let conformance = resolve_product_path(
        typed,
        &conformance_operand.path,
        conformance_operand.source_span,
        ProductOperandKind::Representation,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "opaque representation `{}` does not resolve to one visible named conformance{}",
            conformance_operand.path,
            representation_resolution_context(
                typed,
                &conformance_operand.path,
                conformance_operand.source_span,
            )
        ))
        .with_source_span(conformance_operand.source_span)
    })?;
    let conformance_argument = typed_trees::expression::StaticMachineArgument {
        symbol: conformance,
        path: conformance_operand
            .path
            .split("::")
            .map(typed_trees::name::Identifier::from)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        ..Default::default()
    };
    let opaque_name = &opaque_operand.path;
    let Some(opaque_definition) = typed.data_definition_by_symbol(opaque) else {
        return Err(Diagnostic::error(format!(
            "opaque representation subject `{opaque_name}` does not resolve to an exact data declaration"
        )));
    };
    if opaque_definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
        return Err(Diagnostic::error(format!(
            "representation subject `{opaque_name}` is not boundary-opaque data"
        )));
    }

    if typed.symbols.get(conformance_argument.symbol).kind != SymbolKind::Conformance {
        return Err(Diagnostic::error(format!(
            "opaque representation `{}` does not resolve to an exact named conformance",
            conformance_argument.display_name(),
        )));
    }
    let application = validation::conformance_applications::close_conformance_application(
        typed,
        &conformance_argument,
    )?;
    let conformance = typed
        .conformance_by_symbol(application.declaration)
        .ok_or_else(|| Diagnostic::error("selected opaque conformance disappeared"))?;
    if !is_compiler_owned_opaque_representation_trait(typed, conformance.trait_symbol)
        || !is_compiler_owned_opaque_representation_trait(typed, application.trait_definition)
    {
        return Err(Diagnostic::error(format!(
            "conformance `{}` does not satisfy the exact compiler-owned `OpaqueRepresentation` trait",
            conformance_argument.display_name(),
        )));
    }
    let trait_arguments = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    let [opaque_trait_argument, representation_trait_argument] = trait_arguments else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` does not retain exactly one opaque argument and one representation argument",
            conformance_argument.display_name(),
        )));
    };
    if typed
        .type_reference_table
        .type_symbol(*opaque_trait_argument)
        != opaque
    {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{}`, not selected opaque declaration `{opaque_name}`",
            conformance_argument.display_name(),
            typed.display_type_reference(*opaque_trait_argument),
        )));
    }
    let carrier = typed
        .type_reference_table
        .type_symbol(*representation_trait_argument);
    let Some(carrier_definition) = typed.data_definition_by_symbol(carrier) else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{opaque_name}` as `{}`, which is not one exact concrete data declaration",
            conformance_argument.display_name(),
            typed.display_type_reference(*representation_trait_argument),
        )));
    };
    if carrier_definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
        return Err(Diagnostic::error(format!(
            "opaque representation carrier `{}` must be ordinary checked-shape data",
            carrier_definition.name,
        )));
    }
    let copy_disposition = carrier_closure::validate_inert_carrier(
        typed,
        opaque_definition,
        *representation_trait_argument,
    )?;

    Ok(FormedOpaqueRepresentation {
        _program: typed,
        opaque,
        carrier,
        carrier_type: *representation_trait_argument,
        application,
        copy_disposition,
    })
}

pub fn collect_opaque_representation_selections(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    let has_requests = declared
        .iter()
        .any(|request| request.operation == BuildOperation::RepresentationSelection)
        || executed
            .iter()
            .any(|event| event.request.operation == BuildOperation::RepresentationSelection);
    let Some(activation) = activation else {
        return if has_requests {
            Err(vec![Diagnostic::error(
                "representation requests have no authoritative Build activation",
            )])
        } else {
            Ok(Vec::new())
        };
    };
    if typed.machine(activation).is_none() {
        return Err(vec![Diagnostic::error(
            "representation requests lost their exact authoritative Build machine",
        )]);
    }
    let mut formed = Vec::new();
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::RepresentationSelection)
    {
        let declaration = form_opaque_representation_request(typed, request)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
        formed.push((request, declaration));
    }
    let mut selections: Vec<OpaqueRepresentationSelection> = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::RepresentationSelection)
    {
        if event.composition.is_some() {
            return Err(vec![
                Diagnostic::error("representation choice has an unauthorized composition argument")
                    .with_source_span(event.request.source_span),
            ]);
        }
        let Some((_, declaration)) = formed
            .iter()
            .find(|(request, _)| request.as_ref() == event.request.as_ref())
        else {
            return Err(vec![
                Diagnostic::error(
                    "executed representation choice has no retained authored formation request",
                )
                .with_source_span(event.request.source_span),
            ]);
        };
        if selections
            .iter()
            .any(|selection| selection.opaque() == declaration.opaque)
        {
            return Err(vec![
                Diagnostic::error(format!(
                    "build selects opaque representation `{}` more than once",
                    typed.symbols.display_path(declaration.opaque, "::")
                ))
                .with_source_span(event.request.source_span),
            ]);
        }
        selections.push(
            declaration.clone().activate(
                activation,
                typed
                    .symbols
                    .rejoin_product_occurrence(event.request.source_span),
            ),
        );
    }
    Ok(selections)
}

pub fn rederive_opaque_representation_selections(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    retained: &[OpaqueRepresentationSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    if retained.iter().any(|selection| {
        Some(selection.selecting_machine()) != activation
            || selection.selected_application_commitment()
                != selection.rederived_selected_application_commitment()
    }) {
        return Err(vec![Diagnostic::error(
            "opaque representation selection has stale activation or application custody",
        )]);
    }
    let derived = collect_opaque_representation_selections(typed, activation, declared, executed)?;
    if derived != retained {
        return Err(vec![Diagnostic::error(format!(
            "final opaque-representation rederivation disagrees with executed Build custody (retained {} rows, derived {} rows)",
            retained.len(),
            derived.len()
        ))]);
    }
    Ok(derived)
}
