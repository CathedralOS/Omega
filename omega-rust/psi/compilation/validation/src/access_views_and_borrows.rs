/*
Access-view formation and atomic observing results (`access_views`) alongside
write-only borrow establishment and discharge (`write_only_borrows`).
*/
pub mod access_views {
    use crate::CheckingInput;
    use access_plans::{AccessExposure, AtomicPermissions, FieldAccess};
    use diagnostics::Diagnostic;
    use language_core::atomic::AtomicOrderingPlan;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TableAssignment};
    use typed_trees::typed_trees::AccessFieldPlan;
    use typed_trees::typed_trees::AccessViewPlan;

    pub(crate) mod access_custody {
        use crate::CheckingInput;
        use std::path::Path;

        use diagnostics::Diagnostic;
        use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};
        use typed_trees::data::{DataField, DataMember};
        use typed_trees::trait_definition::TraitDefinition;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_agreements(
            program: &dyn CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for conformance in program.conformances() {
                let Some(trait_definition) =
                    program.trait_definition_by_symbol(conformance.trait_symbol)
                else {
                    continue;
                };
                if !is_core_access_custody(program, trait_definition) {
                    continue;
                }

                let arguments = program
                    .type_reference_table
                    .type_reference_handles(conformance.arguments);
                let [custody_argument, policy_argument, schema_argument] = arguments else {
                    continue;
                };
                let (Some(custody_symbol), Some(policy_symbol), Some(schema_symbol)) = (
                    concrete_named_symbol(program, *custody_argument),
                    concrete_named_symbol(program, *policy_argument),
                    concrete_named_symbol(program, *schema_argument),
                ) else {
                    continue;
                };
                let Some(plan) = program.access_view_plans.iter().find(|plan| {
                    plan.policy_symbol == policy_symbol && plan.schema_symbol == schema_symbol
                }) else {
                    continue;
                };
                let Some(schema) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    schema_symbol,
                ) else {
                    continue;
                };
                let Some(custody) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    custody_symbol,
                ) else {
                    continue;
                };
                let plan_name = program
                    .symbols
                    .display_path(plan.policy_plan_machine_symbol, "::");
                let schema_fields = program
                    .data_members(schema)
                    .iter()
                    .filter_map(field)
                    .collect::<Vec<_>>();
                let custody_fields = program
                    .data_members(custody)
                    .iter()
                    .filter_map(field)
                    .collect::<Vec<_>>();

                if custody_fields.len() != program.data_members(custody).len() {
                    diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: `{}` must be one ordinary record, but it declares case members",
                program.symbols.display_path(conformance.symbol, "::"),
                custody.name,
            )));
                    continue;
                }

                for schema_field in &schema_fields {
                    let custody_field = custody_fields
                        .iter()
                        .copied()
                        .find(|candidate| same_canonical_field(schema_field, candidate));
                    if !schema_field.relevance.is_erased() {
                        let Some(layout_entry) = exact_layout_entry(plan, schema_field) else {
                            continue;
                        };
                        let Some(nested_schema) =
                            direct_nested_custody_record(program, schema_field, schema.symbol)
                        else {
                            if type_contains_erased_descendant(
                                program,
                                schema_field.type_reference,
                                &mut vec![schema.symbol],
                            ) {
                                diagnostics.push(unsupported_nested_custody_diagnostic(
                                    program,
                                    conformance,
                                    schema,
                                    schema_field,
                                    &plan_name,
                                    layout_entry,
                                ));
                            } else if custody_field.is_some() {
                                diagnostics.push(represented_field_diagnostic(
                                    program,
                                    conformance,
                                    schema,
                                    custody,
                                    schema_field,
                                    &plan_name,
                                    layout_entry,
                                ));
                            }
                            continue;
                        };
                        validate_nested_record(
                            program,
                            conformance,
                            schema,
                            custody,
                            schema_field,
                            nested_schema,
                            custody_field,
                            &plan_name,
                            layout_entry,
                            diagnostics,
                        );
                        continue;
                    }
                    let layout_entry = exact_layout_entry(plan, schema_field);
                    if let Some(layout_entry) = layout_entry {
                        if custody_field.is_some() {
                            diagnostics.push(represented_field_diagnostic(
                                program,
                                conformance,
                                schema,
                                custody,
                                schema_field,
                                &plan_name,
                                layout_entry,
                            ));
                        }
                        continue;
                    }
                    validate_custody_leaf(
                        program,
                        conformance,
                        custody,
                        schema_field,
                        custody_field,
                        &canonical_path(schema, schema_field),
                        &plan_name,
                        diagnostics,
                    );
                }

                for custody_field in custody_fields {
                    if !schema_fields
                        .iter()
                        .any(|candidate| same_canonical_field(candidate, custody_field))
                    {
                        diagnostics.push(Diagnostic::error(format!(
                    "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{}` path, but `{}` declares extra canonical field path `{}`",
                    program.symbols.display_path(conformance.symbol, "::"),
                    canonical_path(custody, custody_field),
                    custody.name,
                    canonical_path(custody, custody_field),
                )));
                        continue;
                    }
                }
            }
        }

        const MAX_CUSTODY_RECORD_DEPTH: usize = 24;

        #[allow(clippy::too_many_arguments)]
        fn validate_nested_record(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            schema: &typed_trees::data::DataDefinition,
            custody: &typed_trees::data::DataDefinition,
            outer_field: &DataField,
            nested_schema: &typed_trees::data::DataDefinition,
            custody_field: Option<&DataField>,
            plan_name: &str,
            outer_entry: &LayoutFieldEntryReport,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            validate_nested_record_at_path(
                program,
                conformance,
                schema,
                custody,
                outer_field,
                nested_schema,
                custody_field,
                &canonical_path(schema, outer_field),
                &[nested_schema.symbol],
                1,
                plan_name,
                outer_entry,
                diagnostics,
            );
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_nested_record_at_path(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            root_schema: &typed_trees::data::DataDefinition,
            custody: &typed_trees::data::DataDefinition,
            outer_field: &DataField,
            nested_schema: &typed_trees::data::DataDefinition,
            custody_field: Option<&DataField>,
            path: &str,
            ancestors: &[symbols::SymbolHandle],
            record_depth: usize,
            plan_name: &str,
            outer_entry: &LayoutFieldEntryReport,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let schema_fields = program
                .data_members(nested_schema)
                .iter()
                .filter_map(field)
                .collect::<Vec<_>>();
            let Some(custody_field) = custody_field else {
                push_missing_nested_paths(
                    program,
                    conformance,
                    custody,
                    nested_schema,
                    path,
                    ancestors,
                    record_depth,
                    plan_name,
                    diagnostics,
                );
                return;
            };
            let Some(nested_custody) = plain_record_for_type(program, custody_field.type_reference)
            else {
                diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: canonical path `{path}` requires one authored ordinary projection record, but `{}` has type `{}`",
            program.symbols.display_path(conformance.symbol, "::"),
            canonical_path(custody, custody_field),
            program.normalized_type_identity_text(custody_field.type_reference),
        )));
                return;
            };
            let custody_fields = program
                .data_members(nested_custody)
                .iter()
                .filter_map(field)
                .collect::<Vec<_>>();

            for schema_field in &schema_fields {
                let custody_leaf = custody_fields
                    .iter()
                    .copied()
                    .find(|candidate| same_canonical_field(schema_field, candidate));
                let field_path = format!("{path}.{}", schema_field.path_identity());
                if schema_field.relevance.is_erased() {
                    validate_custody_leaf(
                        program,
                        conformance,
                        custody,
                        schema_field,
                        custody_leaf,
                        &field_path,
                        plan_name,
                        diagnostics,
                    );
                    continue;
                }
                let Some(child_schema) =
                    nested_custody_record(program, schema_field, ancestors, record_depth + 1, true)
                else {
                    if custody_leaf.is_some() {
                        diagnostics.push(nested_path_represented_field_diagnostic(
                            program,
                            conformance,
                            &field_path,
                            root_schema,
                            outer_field,
                            custody,
                            plan_name,
                            outer_entry,
                        ));
                    }
                    continue;
                };
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.push(child_schema.symbol);
                validate_nested_record_at_path(
                    program,
                    conformance,
                    root_schema,
                    custody,
                    outer_field,
                    child_schema,
                    custody_leaf,
                    &field_path,
                    &child_ancestors,
                    record_depth + 1,
                    plan_name,
                    outer_entry,
                    diagnostics,
                );
            }

            for custody_leaf in custody_fields {
                if !schema_fields
                    .iter()
                    .any(|candidate| same_canonical_field(candidate, custody_leaf))
                {
                    diagnostics.push(Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized custody projection has no `{path}.{}` path, but `{}` declares that extra canonical field path",
                program.symbols.display_path(conformance.symbol, "::"),
                custody_leaf.path_identity(),
                custody.name,
            )));
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn validate_custody_leaf(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            custody: &typed_trees::data::DataDefinition,
            schema_field: &DataField,
            custody_field: Option<&DataField>,
            path: &str,
            plan_name: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(custody_field) = custody_field else {
                push_missing_custody_path(
                    program,
                    conformance,
                    custody,
                    plan_name,
                    schema_field,
                    path,
                    diagnostics,
                );
                return;
            };
            let expected_multiplicity = program.type_multiplicity(schema_field.type_reference);
            let actual_multiplicity = program.type_multiplicity(custody_field.type_reference);
            if actual_multiplicity != expected_multiplicity {
                diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with multiplicity {expected_multiplicity:?}, but `{}` uses multiplicity {actual_multiplicity:?}",
            program.symbols.display_path(conformance.symbol, "::"),
            custody.name,
        )));
                return;
            }
            let expected_type = program.normalized_type_identity(schema_field.type_reference);
            let actual_type = program.normalized_type_identity(custody_field.type_reference);
            if actual_type != expected_type {
                let expected_type = program
                    .type_identity_text(expected_type)
                    .unwrap_or_default();
                let actual_type = program.type_identity_text(actual_type).unwrap_or_default();
                diagnostics.push(Diagnostic::error(format!(
            "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{expected_type}`, but `{}` uses `{actual_type}`",
            program.symbols.display_path(conformance.symbol, "::"),
            custody.name,
        )));
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn push_missing_nested_paths(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            custody: &typed_trees::data::DataDefinition,
            nested_schema: &typed_trees::data::DataDefinition,
            path: &str,
            ancestors: &[symbols::SymbolHandle],
            record_depth: usize,
            plan_name: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for schema_field in program.data_members(nested_schema).iter().filter_map(field) {
                let field_path = format!("{path}.{}", schema_field.path_identity());
                if schema_field.relevance.is_erased() {
                    push_missing_custody_path(
                        program,
                        conformance,
                        custody,
                        plan_name,
                        schema_field,
                        &field_path,
                        diagnostics,
                    );
                    continue;
                }
                let Some(child_schema) =
                    nested_custody_record(program, schema_field, ancestors, record_depth + 1, true)
                else {
                    continue;
                };
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.push(child_schema.symbol);
                push_missing_nested_paths(
                    program,
                    conformance,
                    custody,
                    child_schema,
                    &field_path,
                    &child_ancestors,
                    record_depth + 1,
                    plan_name,
                    diagnostics,
                );
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn push_missing_custody_path(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            custody: &typed_trees::data::DataDefinition,
            plan_name: &str,
            field: &DataField,
            path: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            diagnostics.push(Diagnostic::error(format!(
        "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{path}` is custody-carried with exact type `{}` and multiplicity {:?}, but `{}` omits canonical field path `{path}`",
        program.symbols.display_path(conformance.symbol, "::"),
        program.normalized_type_identity_text(field.type_reference),
        program.type_multiplicity(field.type_reference),
        custody.name,
    )));
        }

        fn direct_nested_custody_record<'program>(
            program: &'program dyn CheckingInput,
            schema_field: &DataField,
            owner_symbol: symbols::SymbolHandle,
        ) -> Option<&'program typed_trees::data::DataDefinition> {
            nested_custody_record(program, schema_field, &[owner_symbol], 1, false)
        }

        fn nested_custody_record<'program>(
            program: &'program dyn CheckingInput,
            schema_field: &DataField,
            ancestors: &[symbols::SymbolHandle],
            record_depth: usize,
            require_nonzero_layout: bool,
        ) -> Option<&'program typed_trees::data::DataDefinition> {
            if record_depth > MAX_CUSTODY_RECORD_DEPTH {
                return None;
            }
            let record = plain_record_for_type(program, schema_field.type_reference)?;
            if ancestors.contains(&record.symbol)
                || (require_nonzero_layout
                    && fixed_type_width_bytes(program, schema_field.type_reference)
                        .is_none_or(|width| width == 0))
            {
                return None;
            }
            let mut visiting = ancestors.to_vec();
            visiting.push(record.symbol);
            let mut has_custody = false;
            for field in program.data_members(record).iter().filter_map(field) {
                if field.relevance.is_erased() {
                    has_custody = true;
                    continue;
                }
                if !type_contains_erased_descendant(
                    program,
                    field.type_reference,
                    &mut visiting.clone(),
                ) {
                    continue;
                }
                nested_custody_record(program, field, &visiting, record_depth + 1, true)?;
                has_custody = true;
            }
            has_custody.then_some(record)
        }

        fn type_contains_erased_descendant(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> bool {
            let mut memo = Vec::new();
            let mut visited_records = 0usize;
            type_contains_erased_descendant_bounded(
                program,
                type_reference,
                visiting,
                &mut memo,
                &mut visited_records,
                0,
            )
        }

        const MAX_CUSTODY_DESCENDANT_TYPE_DEPTH: usize = 64;
        const MAX_CUSTODY_DESCENDANT_RECORDS: usize = 256;

        fn type_contains_erased_descendant_bounded(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
            visiting: &mut Vec<symbols::SymbolHandle>,
            memo: &mut Vec<(symbols::SymbolHandle, bool)>,
            visited_records: &mut usize,
            depth: usize,
        ) -> bool {
            if !type_reference.is_valid() {
                return true;
            }
            if depth > MAX_CUSTODY_DESCENDANT_TYPE_DEPTH {
                return true;
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { symbol, .. } => {
                    let Some(record) = crate::declarations::data_queries::data_definition_by_symbol(
                        program, *symbol,
                    ) else {
                        return false;
                    };
                    if visiting.contains(symbol) {
                        return true;
                    }
                    if let Some((_, result)) =
                        memo.iter().find(|(candidate, _)| candidate == symbol)
                    {
                        return *result;
                    }
                    *visited_records = match visited_records.checked_add(1) {
                        Some(count) if count <= MAX_CUSTODY_DESCENDANT_RECORDS => count,
                        _ => return true,
                    };
                    visiting.push(*symbol);
                    let mut result = false;
                    for member in program.data_members(record) {
                        let fields = program.data_member_fields(std::slice::from_ref(member));
                        for field in fields {
                            if field.relevance.is_erased()
                                || type_contains_erased_descendant_bounded(
                                    program,
                                    field.type_reference,
                                    visiting,
                                    memo,
                                    visited_records,
                                    depth + 1,
                                )
                            {
                                result = true;
                                break;
                            }
                        }
                        if result {
                            break;
                        }
                    }
                    visiting.pop();
                    memo.push((*symbol, result));
                    result
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    type_contains_erased_descendant_bounded(
                        program,
                        *element_type,
                        visiting,
                        memo,
                        visited_records,
                        depth + 1,
                    )
                }
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => type_contains_erased_descendant_bounded(
                    program,
                    *base_type,
                    visiting,
                    memo,
                    visited_records,
                    depth + 1,
                ),
                TypeReferenceNode::Generic { .. } => true,
                TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {
                    true
                }
                TypeReferenceNode::Unit => false,
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn plain_record_for_type(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<&typed_trees::data::DataDefinition> {
            let symbol = &program
                .type_reference_table
                .type_reference(type_reference)
                .as_named_symbol()?;
            let record =
                crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
            (record.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                && record.generic_instance.is_none()
                && record.type_parameters.is_empty()
                && record.lifetime_parameters.is_empty()
                && record.quotient.is_none()
                && program
                    .data_members(record)
                    .iter()
                    .all(|member| matches!(member, DataMember::Field(_))))
            .then_some(record)
        }

        fn is_core_access_custody(
            program: &dyn CheckingInput,
            definition: &TraitDefinition,
        ) -> bool {
            definition.name.as_str() == "AccessCustody"
                && !definition.is_boundary
                && program
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .and_then(|span| program.symbols.source_file(span))
                    .is_some_and(|source| {
                        source.origin == source::SourceOrigin::Toolchain
                            && source.path.ends_with(Path::new("core/layout.omg"))
                    })
        }

        fn concrete_named_symbol(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<symbols::SymbolHandle> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                _ => None,
            }
        }

        fn field(member: &DataMember) -> Option<&DataField> {
            match member {
                DataMember::Field(field) => Some(field),
                DataMember::Variant(_) => None,
            }
        }

        fn same_canonical_field(left: &DataField, right: &DataField) -> bool {
            match (left.identity, right.identity) {
                (Some(left), Some(right)) => left == right,
                (None, None) => left.name.as_str() == right.name.as_str(),
                _ => false,
            }
        }

        fn exact_layout_entry<'plan>(
            plan: &'plan typed_trees::typed_trees::AccessViewPlan,
            field: &DataField,
        ) -> Option<&'plan LayoutFieldEntryReport> {
            plan.placement.layout().entries.iter().find(|entry| {
                match (field.identity, entry.member_identity) {
                    (Some(field), Some(entry)) => field == entry,
                    (None, None) => field.name.as_str() == entry.field,
                    _ => false,
                }
            })
        }

        fn canonical_path(owner: &typed_trees::data::DataDefinition, field: &DataField) -> String {
            format!("{}.{}", owner.name, field.path_identity())
        }

        fn represented_field_diagnostic(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            schema: &typed_trees::data::DataDefinition,
            custody: &typed_trees::data::DataDefinition,
            schema_field: &DataField,
            plan_name: &str,
            entry: &LayoutFieldEntryReport,
        ) -> Diagnostic {
            Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is {}, so represented field `{}` must be absent from `{}`",
                program.symbols.display_path(conformance.symbol, "::"),
                canonical_path(schema, schema_field),
                represented_decision(program, schema_field.type_reference, &entry.placement),
                canonical_path(schema, schema_field),
                custody.name,
            ))
        }

        #[allow(clippy::too_many_arguments)]
        fn nested_path_represented_field_diagnostic(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            path: &str,
            schema: &typed_trees::data::DataDefinition,
            outer_field: &DataField,
            custody: &typed_trees::data::DataDefinition,
            plan_name: &str,
            outer_entry: &LayoutFieldEntryReport,
        ) -> Diagnostic {
            Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is contained in `{}`, which is {}, so represented field `{}` must be absent from `{}`",
                program.symbols.display_path(conformance.symbol, "::"),
                path,
                canonical_path(schema, outer_field),
                represented_decision(program, outer_field.type_reference, &outer_entry.placement),
                path,
                custody.name,
            ))
        }

        fn unsupported_nested_custody_diagnostic(
            program: &dyn CheckingInput,
            conformance: &typed_trees::trait_definition::Conformance,
            schema: &typed_trees::data::DataDefinition,
            field: &DataField,
            plan_name: &str,
            entry: &LayoutFieldEntryReport,
        ) -> Diagnostic {
            Diagnostic::error(format!(
                "custody conformance `{}` disagrees with `{plan_name}`: normalized decision for `{}` is {}, but its represented type contains non-runtime custody outside the exact twenty-four-record acyclic, non-generic, case-free projection spine",
                program.symbols.display_path(conformance.symbol, "::"),
                canonical_path(schema, field),
                represented_decision(program, field.type_reference, &entry.placement),
            ))
        }

        fn represented_decision(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
            placement: &LayoutPlacementReport,
        ) -> String {
            match placement {
                LayoutPlacementReport::At { offset } => {
                    fixed_type_width_bytes(program, type_reference).map_or_else(
                        || format!("represented at offset {offset} with its exact semantic width"),
                        |width| format!("represented at offset {offset} with width {width}"),
                    )
                }
                LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width,
                    interpretation,
                } => format!(
                    "represented at offset {offset} with stored width {} ({interpretation:?})",
                    stored_width / 8
                ),
                LayoutPlacementReport::Bits {
                    container,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
                } => format!(
                    "represented in container {container} (width {container_width}) at destination bit {destination_lsb} from source bit {source_lsb} with width {width} bits"
                ),
            }
        }

        fn fixed_type_width_bytes(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<usize> {
            fixed_type_layout(program, type_reference, &mut Vec::new()).map(|(size, _)| size)
        }

        fn fixed_type_layout(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> Option<(usize, usize)> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some(primitive) =
                        crate::value_custody::recasts::exact_primitive_type(program, type_reference)
                    {
                        let size = primitive.scalar_byte_size()?;
                        return Some((size, size));
                    }
                    if visiting.contains(symbol) {
                        return None;
                    }
                    let record = plain_record_for_type(program, type_reference)?;
                    visiting.push(*symbol);
                    let mut size = 0usize;
                    let mut aggregate_align = 1usize;
                    for field in program.data_members(record).iter().filter_map(field) {
                        if field.relevance.is_erased() {
                            continue;
                        }
                        let (field_size, field_align) =
                            fixed_type_layout(program, field.type_reference, visiting)?;
                        size = align_up(size, field_align)?.checked_add(field_size)?;
                        aggregate_align = aggregate_align.max(field_align);
                    }
                    visiting.pop();
                    Some((align_up(size, aggregate_align)?, aggregate_align))
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } => {
                    let (element_size, element_align) =
                        fixed_type_layout(program, *element_type, visiting)?;
                    Some((element_size.checked_mul(*length)?, element_align))
                }
                _ => None,
            }
        }

        fn align_up(value: usize, align: usize) -> Option<usize> {
            value
                .checked_add(align.checked_sub(1)?)
                .map(|value| value / align * align)
        }
    }
    pub(crate) mod checked_atomic_resident {
        use typed_trees::typed_trees::AccessAtomicObservingResultContract;

        pub fn access_atomic_observing_results(
            operations: access_plans::AtomicPermissions,
        ) -> Vec<AccessAtomicObservingResultContract> {
            let mut observing_results = Vec::new();
            if operations.compare_exchange {
                let operation =
                    language_core::atomic::AtomicObservingCompareExchangeOperation::Decisive;
                observing_results.push(AccessAtomicObservingResultContract {
                    operation,
                    result_shape: operation.result_shape(),
                });
            }
            if operations.compare_exchange_once {
                let operation =
                    language_core::atomic::AtomicObservingCompareExchangeOperation::SingleAttempt;
                observing_results.push(AccessAtomicObservingResultContract {
                    operation,
                    result_shape: operation.result_shape(),
                });
            }
            observing_results
        }
    }
    pub(crate) mod plan_replay {
        use crate::CheckingInput;
        use access_plans::{AccessFieldEntry, FieldAccess};
        use diagnostics::Diagnostic;
        use language_semantics::Multiplicity;

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum RetainedFieldIdentity {
            Numbered(u64),
            Positional(String),
        }

        fn retained_field_identity(name: &str, identity: Option<u64>) -> RetainedFieldIdentity {
            match identity {
                Some(identity) => RetainedFieldIdentity::Numbered(identity),
                None => RetainedFieldIdentity::Positional(name.to_owned()),
            }
        }

        pub(crate) fn exact_access_entry_for_field<'view>(
            view: &'view typed_trees::typed_trees::AccessViewPlan,
            field: &typed_trees::typed_trees::AccessFieldPlan,
        ) -> Option<&'view AccessFieldEntry> {
            let field_identity = retained_field_identity(&field.field_name, field.member_identity);
            let mut canonical_layout_identities = view
                .placement
                .layout()
                .entries
                .iter()
                .map(|entry| retained_field_identity(&entry.field, entry.member_identity))
                .collect::<Vec<_>>();
            canonical_layout_identities.sort();
            canonical_layout_identities.dedup();
            canonical_layout_identities
                .iter()
                .position(|identity| identity == &field_identity)
                .and_then(|slot| u32::try_from(slot).ok())
                .and_then(|slot| {
                    view.placement
                        .access()
                        .plan()
                        .entries()
                        .iter()
                        .find(|entry| entry.key().slot() == slot)
                })
        }

        pub(crate) fn validate_plans(
            program: &dyn CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for view in &program.access_view_plans {
                macro_rules! view {
                    ($detail:literal) => {
                        diagnostics.push(Diagnostic::error(format!(
                            concat!("access view `{}` ", $detail),
                            view.data_name
                        )))
                    };
                }
                let Some(view_data) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    view.data_symbol,
                ) else {
                    view!("no longer names its exact synthesized data identity");
                    continue;
                };
                let Some(schema) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    view.schema_symbol,
                ) else {
                    view!("no longer names its exact source schema identity");
                    continue;
                };
                let Some(policy) = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    view.policy_symbol,
                ) else {
                    view!("no longer names its exact placement-policy identity");
                    continue;
                };
                let Some(policy_plan_machine) = program.machine(view.policy_plan_machine_symbol)
                else {
                    view!("no longer names its exact placement-policy plan machine");
                    continue;
                };
                if policy_plan_machine
                    .attached_data
                    .as_ref()
                    .is_none_or(|attached| attached.as_str() != policy.name.as_str())
                    || policy_plan_machine.name.as_str()
                        != format!("{}::plan", policy.name.as_str())
                {
                    view!("changed its exact placement-policy binding");
                    continue;
                }
                let accessible_field_count = view
                    .placement
                    .access()
                    .plan()
                    .entries()
                    .iter()
                    .filter(|entry| !matches!(entry.access(), FieldAccess::Inaccessible))
                    .count();
                if view.fields.len() != accessible_field_count
                    || program.data_members(view_data).len() != accessible_field_count
                    || program
                        .data_members(view_data)
                        .iter()
                        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
                {
                    view!("changed its exact accessible field inventory");
                    continue;
                }
                let mut field_symbols = Vec::with_capacity(view.fields.len());
                let mut accessor_data_symbols = Vec::with_capacity(view.fields.len());
                for field in &view.fields {
                    macro_rules! field {
                        ($detail:literal) => {
                            diagnostics.push(Diagnostic::error(format!(
                                concat!("access view `{}` field `{}` ", $detail),
                                view.data_name, field.field_name
                            )))
                        };
                    }
                    if field_symbols.contains(&field.field_symbol)
                        || accessor_data_symbols.contains(&field.accessor_data_symbol)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "access view `{}` repeats field or accessor identity for `{}`",
                            view.data_name, field.field_name
                        )));
                        continue;
                    }
                    field_symbols.push(field.field_symbol);
                    accessor_data_symbols.push(field.accessor_data_symbol);
                    if matches!(field.access, FieldAccess::Region { .. })
                        && let Err(diagnostic) =
                            field.scalar_projection_leaf(program.typed_program())
                    {
                        diagnostics.push(diagnostic);
                        continue;
                    }

                    let Some(schema_field) = program
                        .data_members(schema)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Field(field) => Some(field),
                            typed_trees::data::DataMember::Variant(_) => None,
                        })
                        .find(|candidate| candidate.symbol == field.field_symbol)
                    else {
                        field!("no longer names its exact source field identity");
                        continue;
                    };
                    if schema_field.identity != field.member_identity
                        || (field.member_identity.is_none()
                            && schema_field.name.as_str() != field.field_name)
                        || schema_field.type_reference != field.value_type
                    {
                        field!("changed its exact source member binding");
                        continue;
                    }

                    let field_identity =
                        retained_field_identity(&field.field_name, field.member_identity);
                    let exact_layout_entry = view.placement.layout().entries.iter().find(|entry| {
                        retained_field_identity(&entry.field, entry.member_identity)
                            == field_identity
                    });
                    if exact_layout_entry.is_none() {
                        field!("changed its retained layout member identity");
                        continue;
                    }

                    let exact_access = exact_access_entry_for_field(view, field);
                    if !exact_access.is_some_and(|entry| entry.access() == &field.access) {
                        field!("changed its admitted access decision");
                        continue;
                    }

                    let expected_atomic_resident = match &field.access {
                        FieldAccess::Atomic {
                            transfer_width_bits,
                            operations,
                            ..
                        } if operations.compare_exchange || operations.compare_exchange_once => {
                            let multiplicity =
                                program.type_multiplicity(schema_field.type_reference);
                            if multiplicity != Multiplicity::Unrestricted {
                                field!(
                                    "no longer has an unrestricted resident for observing compare-exchange"
                                );
                                continue;
                            }
                            let observing_results =
                                super::access_atomic_observing_results(*operations);
                            Some(typed_trees::typed_trees::AccessAtomicResidentContract {
                                field_symbol: schema_field.symbol,
                                resident_type: schema_field.type_reference,
                                multiplicity,
                                transfer_width_bits: *transfer_width_bits,
                                compare_exchange: operations.compare_exchange,
                                compare_exchange_once: operations.compare_exchange_once,
                                observing_results,
                            })
                        }
                        _ => None,
                    };
                    if field.atomic_resident != expected_atomic_resident {
                        field!("changed its checked Atomic resident/result contract");
                        continue;
                    }

                    let exact_view_field = program
                        .data_members(view_data)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Field(field) => Some(field),
                            typed_trees::data::DataMember::Variant(_) => None,
                        })
                        .find(|candidate| {
                            candidate.identity == field.member_identity
                                && (field.member_identity.is_some()
                                    || candidate.name.as_str() == field.field_name)
                        });
                    if !exact_view_field
                        .is_some_and(|candidate| candidate.type_reference == field.accessor_type)
                    {
                        field!("changed its exact synthesized accessor binding");
                    }

                    let has_accessor_data =
                        crate::declarations::data_queries::data_definition_by_symbol(
                            program,
                            field.accessor_data_symbol,
                        )
                        .is_some();
                    if !has_accessor_data {
                        field!("no longer names its exact generated accessor data identity");
                        continue;
                    }
                    if exact_view_field.is_some() {
                        let type_symbol = program
                            .type_reference_table
                            .type_symbol(field.accessor_type);
                        if (type_symbol.is_valid() && type_symbol != field.accessor_data_symbol)
                            || (!type_symbol.is_valid()
                                && !matches!(field.access, FieldAccess::Atomic { .. }))
                        {
                            field!("changed its exact generated accessor data binding");
                            continue;
                        }
                    }

                    let expected_operations = field.access.operation_names();
                    if field
                        .accessor_targets
                        .iter()
                        .map(|target| target.operation.as_str())
                        .ne(expected_operations.iter().copied())
                    {
                        field!("changed its generated accessor operation set");
                        continue;
                    }
                    for target in &field.accessor_targets {
                        let Some(machine) = program.machine(target.machine_symbol) else {
                            diagnostics.push(Diagnostic::error(format!(
                        "access view `{}` field `{}` operation `{}` no longer names its exact generated accessor machine",
                        view.data_name, field.field_name, target.operation
                    )));
                            continue;
                        };
                        let exact_machine = machine
                            .attached_data
                            .as_ref()
                            .is_some_and(|attached| attached.as_str() == field.accessor_name)
                            && machine
                                .name
                                .as_str()
                                .rsplit("::")
                                .next()
                                .is_some_and(|operation| operation == target.operation);
                        let states = program.machine_states(machine);
                        if !exact_machine
                            || !matches!(states, [state] if state.symbol == target.state_symbol)
                        {
                            diagnostics.push(Diagnostic::error(format!(
                        "access view `{}` field `{}` operation `{}` changed its exact generated accessor target",
                        view.data_name, field.field_name, target.operation
                    )));
                        }
                    }
                }
            }
        }
    }
    pub use checked_atomic_resident::access_atomic_observing_results;

    pub(crate) fn validate_plans(program: &dyn CheckingInput, diagnostics: &mut Vec<Diagnostic>) {
        plan_replay::validate_plans(program, diagnostics);
        access_custody::validate_agreements(program, diagnostics);
    }

    pub(crate) fn validate_statement(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        statement: &StatementNode,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if let Some(assignment) = statement.as_assignment() {
            validate_assignment(program, machine, state, assignment, diagnostics);
        }
        if let Some(call) = statement.as_call()
            && let Some((view, field)) =
                access_view_field_for_statement_call(program, machine, state, call)
        {
            validate_binding_private_use(program, machine, view, field, diagnostics);
        }
        for expression in statement_expression_roots(program, statement, false) {
            validate_expression(program, machine, state, expression, false, diagnostics);
        }
    }

    pub(crate) fn assignment_is_access_atomic_operation(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        assignment: &TableAssignment,
    ) -> bool {
        access_atomic_field_for_place(program, machine, state, assignment.target).is_some()
            && matches!(
                program.expression_table.expression(assignment.value),
                ExpressionNode::Atomic(atomic)
                    if !matches!(atomic.ordering, AtomicOrderingPlan::Load(_))
            )
    }

    fn validate_assignment(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        assignment: &TableAssignment,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some((view, field)) =
            access_view_field_for_place(program, machine, state, assignment.target)
        else {
            return;
        };
        validate_binding_private_use(program, machine, view, field, diagnostics);
        if !matches!(&field.access, FieldAccess::Atomic { .. }) {
            return;
        }
        let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment.value)
        else {
            diagnostics.push(Diagnostic::error(format!(
            "atomic accessor field `{}` cannot be assigned directly; use one admitted atomic operation",
            field.field_name
        )));
            return;
        };
        let (operation, admitted) = match atomic.ordering {
            AtomicOrderingPlan::Store(_) => ("store", permissions(field).store),
            AtomicOrderingPlan::ReadModifyWrite(_) => {
                let operation = fetch_operation(program, atomic.value);
                let admitted = operation.is_some_and(|operation| match operation {
                    "fetch_add" => permissions(field).fetch_add,
                    "fetch_sub" => permissions(field).fetch_sub,
                    "fetch_xor" => permissions(field).fetch_xor,
                    "fetch_or" => permissions(field).fetch_or,
                    "fetch_and" => permissions(field).fetch_and,
                    _ => false,
                });
                (operation.unwrap_or("unknown fetch operation"), admitted)
            }
            AtomicOrderingPlan::Swap(_) => ("swap", permissions(field).swap),
            ordering @ (AtomicOrderingPlan::CompareExchange { .. }
            | AtomicOrderingPlan::CompareExchangeOnce { .. }) => {
                observing_compare_exchange_permission(permissions(field), ordering)
                    .expect("compare-exchange ordering has one observing permission row")
            }
            AtomicOrderingPlan::Load(_) => ("load", false),
        };
        if !admitted {
            report_unauthorized(field, operation, diagnostics);
        }
    }

    fn validate_expression(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        atomic_place_allowed: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !expression.is_valid() {
            return;
        }
        if let Some(formation) =
            crate::value_custody::recasts::reference_formation_source(program, expression)
            && matches!(
                formation.interpretation,
                crate::value_custody::recasts::ReferenceFormationInterpretation::ResidentAccess { .. }
            )
            && let Some(plan) = program.access_view_plans.iter().find(|plan| {
                plan.data_symbol
                    == program.type_reference_table.type_symbol(
                        program
                            .type_reference_table
                            .peeled_type_reference(formation.view_type),
                    )
            })
            && plan
                .fields
                .iter()
                .any(|field| field.access.has_destructive_read())
        {
            diagnostics.push(Diagnostic::error(
            "resident access view with destructive read requires tracked content restoration before loan return",
        ));
            return;
        }
        if let Some((view, field)) =
            access_view_field_for_place(program, machine, state, expression)
        {
            validate_binding_private_use(program, machine, view, field, diagnostics);
            if !atomic_place_allowed && matches!(&field.access, FieldAccess::Atomic { .. }) {
                diagnostics.push(Diagnostic::error(format!(
                "atomic accessor field `{}` is an accessor, not an ordinary value; use one admitted atomic operation",
                field.field_name
            )));
                return;
            }
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                for child in
                    crate::queries::expression_traversal::match_children(program, *dispatch)
                {
                    validate_expression(program, machine, state, child, false, diagnostics);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                if matches!(atomic.ordering, AtomicOrderingPlan::Load(_)) {
                    if let Some(field) =
                        access_atomic_field_for_place(program, machine, state, atomic.value)
                    {
                        if !permissions(field).load {
                            report_unauthorized(field, "load", diagnostics);
                        }
                    } else if !crate::place_is_atomic_storage(
                        program,
                        machine,
                        Some(state),
                        atomic.value,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                        "atomic load in machine `{}` state `{}` requires dedicated atomic storage or an admitted placed atomic accessor",
                        machine.name, state.name,
                    )));
                    }
                    validate_expression(program, machine, state, atomic.value, true, diagnostics);
                } else {
                    validate_expression(program, machine, state, atomic.value, false, diagnostics);
                }
                if atomic.result.is_valid() {
                    validate_expression(program, machine, state, atomic.result, false, diagnostics);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for element in program.expression_table.expression_handles(*elements) {
                    validate_expression(program, machine, state, *element, false, diagnostics);
                }
            }
            ExpressionNode::Binary(binary) => {
                validate_expression(program, machine, state, binary.left, false, diagnostics);
                validate_expression(program, machine, state, binary.right, false, diagnostics);
            }
            ExpressionNode::Cast(cast) => {
                validate_expression(program, machine, state, cast.value, false, diagnostics);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    validate_expression(program, machine, state, call.receiver, false, diagnostics);
                }
                for argument in program.expression_table.expression_handles(call.arguments) {
                    validate_expression(program, machine, state, *argument, false, diagnostics);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                validate_expression(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    false,
                    diagnostics,
                );
                validate_expression(program, machine, state, indexed.index, false, diagnostics);
            }
            ExpressionNode::Member(member) => {
                validate_expression(program, machine, state, member.receiver, false, diagnostics);
            }
            ExpressionNode::Borrow(inner) => {
                validate_expression(program, machine, state, inner.target, false, diagnostics);
            }
            ExpressionNode::Unary(unary) => {
                validate_expression(program, machine, state, unary.operand, false, diagnostics);
            }
            ExpressionNode::Range(range) => {
                validate_expression(program, machine, state, range.start, false, diagnostics);
                validate_expression(program, machine, state, range.end, false, diagnostics);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    validate_expression(program, machine, state, field.value, false, diagnostics);
                }
            }
            ExpressionNode::TypeExpression(_)
            | ExpressionNode::SavedSubject(_)
            | typed_trees::literal_expression_pattern!() => {}
        }
    }

    fn access_atomic_field_for_place<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        state: &State,
        place: ExpressionHandle,
    ) -> Option<&'program AccessFieldPlan> {
        let (_, field) = access_view_field_for_place(program, machine, state, place)?;
        matches!(&field.access, FieldAccess::Atomic { .. }).then_some(field)
    }

    fn access_view_field_for_place<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        state: &State,
        place: ExpressionHandle,
    ) -> Option<(&'program AccessViewPlan, &'program AccessFieldPlan)> {
        if program.access_view_plans.is_empty() {
            return None;
        }
        let type_reference = crate::value_custody::places::declared_place_type(
            program,
            machine,
            Some(state),
            place,
        )?;
        program.access_view_field_plan_for_type_reference(type_reference)
    }

    fn access_view_field_for_call_target(
        program: &dyn CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<(&AccessViewPlan, &AccessFieldPlan)> {
        if !target.is_valid() || program.access_view_plans.is_empty() {
            return None;
        }
        program.access_view_plans.iter().find_map(|view| {
            view.fields
                .iter()
                .find(|field| {
                    field
                        .accessor_targets
                        .iter()
                        .any(|accessor| accessor.state_symbol == target)
                })
                .map(|field| (view, field))
        })
    }

    fn access_view_field_for_statement_call<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        state: &State,
        call: &typed_trees::statement::TableCall,
    ) -> Option<(&'program AccessViewPlan, &'program AccessFieldPlan)> {
        access_view_field_for_call_target(program, call.target_symbol).or_else(|| {
            let receiver =
                crate::machine_calls::calls::signature_queries::call_receiver_member_names(
                    program, call,
                );
            let type_reference = crate::value_custody::places::declared_member_path_type(
                program,
                machine,
                Some(state),
                &receiver,
            )?;
            program.access_view_field_plan_for_type_reference(type_reference)
        })
    }

    pub(crate) fn statement_call_target(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        call: &typed_trees::statement::TableCall,
    ) -> Option<symbols::SymbolHandle> {
        let members = crate::machine_calls::calls::signature_queries::call_receiver_member_names(
            program, call,
        );
        crate::machine_calls::calls::accessor_target(
            program,
            machine,
            state,
            &members,
            call.target_symbol,
            call.target.as_str(),
        )
    }

    fn validate_binding_private_use(
        program: &dyn CheckingInput,
        machine: &Machine,
        view: &AccessViewPlan,
        field: &AccessFieldPlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if access_exposure(&field.access) != Some(AccessExposure::BindingPrivate)
            || program
                .symbols
                .same_symbol_source_package(machine.symbol, view.policy_symbol)
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "binding-private accessor `{}` belongs to access policy `{}`'s package",
            field.field_name, view.policy_name
        )));
    }

    fn access_exposure(access: &FieldAccess) -> Option<AccessExposure> {
        match access {
            FieldAccess::Inaccessible => None,
            FieldAccess::Stable { exposure, .. }
            | FieldAccess::Region { exposure, .. }
            | FieldAccess::External { exposure, .. }
            | FieldAccess::Atomic { exposure, .. } => Some(*exposure),
        }
    }

    fn permissions(field: &AccessFieldPlan) -> AtomicPermissions {
        match &field.access {
            FieldAccess::Atomic { operations, .. } => *operations,
            _ => AtomicPermissions::default(),
        }
    }

    fn observing_compare_exchange_permission(
        permissions: AtomicPermissions,
        ordering: AtomicOrderingPlan,
    ) -> Option<(&'static str, bool)> {
        match ordering {
            AtomicOrderingPlan::CompareExchange { .. } => {
                Some(("compare_exchange", permissions.compare_exchange))
            }
            AtomicOrderingPlan::CompareExchangeOnce { .. } => {
                Some(("compare_exchange_once", permissions.compare_exchange_once))
            }
            _ => None,
        }
    }

    fn fetch_operation(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<&'static str> {
        let binary = program.expression_table.binary_node(expression)?;
        match binary.operator {
            BinaryOperator::Add => Some("fetch_add"),
            BinaryOperator::Subtract => Some("fetch_sub"),
            BinaryOperator::BitwiseXor => Some("fetch_xor"),
            BinaryOperator::BitwiseOr => Some("fetch_or"),
            BinaryOperator::BitwiseAnd => Some("fetch_and"),
            _ => None,
        }
    }

    fn report_unauthorized(
        field: &AccessFieldPlan,
        operation: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "atomic accessor field `{}` does not admit `{operation}`",
            field.field_name
        )));
    }

    pub(crate) fn statement_expression_roots(
        program: &dyn CheckingInput,
        statement: &StatementNode,
        include_assignment_targets: bool,
    ) -> Vec<ExpressionHandle> {
        use typed_trees::statement::StatementRootRole as Role;
        program
            .tables
            .expression_roots(statement)
            .into_iter()
            .filter(|(role, _)| match role {
                Role::AssignmentTarget => include_assignment_targets,
                _ => true,
            })
            .map(|(_, root)| root)
            .collect()
    }
}

pub mod write_only_borrows {
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::ReferenceAccess;
    use language_semantics::declaration_selection::CollectionMeasure;
    use symbols::SymbolHandle;
    use typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use typed_trees::types::{
        FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) mod local_formation {
        use super::{
            ExpressionNode, Machine, ReferenceAccess, State, TypeReferenceHandle,
            TypeReferenceNode, WriteOnlyRoot, receiver,
        };
        use crate::frozen_program::expression_table;
        use typed_trees::statement::TableLocalData;

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            local: &TableLocalData,
            roots: &[WriteOnlyRoot],
            diagnostics: &mut Vec<diagnostics::Diagnostic>,
        ) -> bool {
            let Some((actual, referee)) =
                capture(program, machine, state, local, roots, diagnostics)
            else {
                return false;
            };
            if crate::value_custody::type_references::type_references_match(
                program, actual, referee,
            ) {
                return true;
            }
            diagnostics.push(diagnostics::Diagnostic::error(format!(
        "machine `{}` state `{}` captures `{}` of type `{}` as `&write {}`; a write-only formation must preserve the declared constraint atoms exactly, so a weaker, stronger, or different referee remains rejected",
        machine.name,
        state.name,
        expression_table(program).display_name(
            match expression_table(program).expression(local.initial_value) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => local.initial_value,
            }
        ),
        program.display_type_reference_with_constraints(actual),
        program.display_type_reference_with_constraints(referee),
    )));
            true
        }

        fn capture(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            local: &TableLocalData,
            roots: &[WriteOnlyRoot],
            diagnostics: &mut Vec<diagnostics::Diagnostic>,
        ) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
            if local.is_mutable
                || !expression_table(program).expression_is_valid(local.initial_value)
            {
                return None;
            }
            let TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::WriteOnly,
                ..
            } = program
                .type_reference_table
                .type_reference(local.type_reference)
            else {
                return None;
            };
            let borrow = expression_table(program).borrow_expression(local.initial_value)?;
            if borrow.access != ReferenceAccess::WriteOnly
                || !crate::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    borrow.target,
                )
            {
                return None;
            }
            let mut sources = roots.to_vec();
            sources.extend(super::projection_sources(
                program,
                state,
                Some(local.symbol),
            ));
            let projection = receiver::nonobserving_projection(
                program,
                machine,
                state,
                borrow.target,
                &sources,
                roots,
                diagnostics,
            )?;
            (!projection.attached_root
                && super::is_admitted_checked_referee(program, projection.type_reference))
            .then_some((projection.type_reference, *referee))
        }
    }
    pub(crate) mod receiver {
        use super::{
            DataDefinition, DataShapeKind, Diagnostic, ExpressionHandle, ExpressionNode, Machine,
            SymbolHandle, WriteOnlyRoot, closed_write_only_data_by_symbol,
            closed_write_only_data_shape_by_symbol, write_only_record_shape,
        };
        use crate::frozen_program::expression_table;
        use typed_trees::data::DataField;
        use typed_trees::expression::TableNamePath;
        use typed_trees::statement::TableCall;

        pub(crate) mod projections {
            use super::super::{
                TypeReferenceHandle, direct_write_only_root,
                fixed_discardable_write_only_array_shape,
            };
            use super::{
                ExpressionHandle, ExpressionNode, WriteOnlyRoot, bare_field,
                write_only_record_shape,
            };

            pub(crate) struct ResolvedProjection {
                pub(crate) type_reference: TypeReferenceHandle,
                pub(crate) attached_root: bool,
                pub(crate) access: language_semantics::ReferenceAccess,
            }

            #[derive(Clone, Copy)]
            pub(crate) enum ProjectionEndpoint {
                Storage,
                Referent,
            }

            pub(crate) fn storage_projection(
                program: &dyn crate::CheckingInput,
                machine: &super::super::Machine,
                state: &super::super::State,
                expression: ExpressionHandle,
                sources: &[WriteOnlyRoot],
                observation_roots: &[WriteOnlyRoot],
                diagnostics: &mut Vec<super::super::Diagnostic>,
            ) -> Option<ResolvedProjection> {
                resolve_projection(
                    program,
                    expression,
                    sources,
                    ProjectionEndpoint::Storage,
                    &mut |selector, _| {
                        super::super::validate_expression(
                            program,
                            machine,
                            state,
                            selector,
                            observation_roots,
                            diagnostics,
                        );
                        true
                    },
                )
            }

            pub(crate) fn nonobserving_projection(
                program: &dyn crate::CheckingInput,
                machine: &super::super::Machine,
                state: &super::super::State,
                expression: ExpressionHandle,
                sources: &[WriteOnlyRoot],
                observation_roots: &[WriteOnlyRoot],
                diagnostics: &mut Vec<super::super::Diagnostic>,
            ) -> Option<ResolvedProjection> {
                projected(program, expression, sources, &mut |selector, _| {
                    super::super::validate_expression(
                        program,
                        machine,
                        state,
                        selector,
                        observation_roots,
                        diagnostics,
                    );
                    true
                })
            }

            pub(crate) fn projected(
                program: &dyn crate::CheckingInput,
                expression: ExpressionHandle,
                roots: &[WriteOnlyRoot],
                admit_selector: &mut dyn FnMut(ExpressionHandle, usize) -> bool,
            ) -> Option<ResolvedProjection> {
                resolve_projection(
                    program,
                    expression,
                    roots,
                    ProjectionEndpoint::Referent,
                    admit_selector,
                )
            }

            fn resolve_projection(
                program: &dyn crate::CheckingInput,
                expression: ExpressionHandle,
                roots: &[WriteOnlyRoot],
                endpoint: ProjectionEndpoint,
                admit_selector: &mut dyn FnMut(ExpressionHandle, usize) -> bool,
            ) -> Option<ResolvedProjection> {
                let (projections, cursor) = crate::queries::expression_traversal::projection_climb(
                    program,
                    expression,
                    usize::MAX,
                )?;
                let (root, mut referee, mut attached_root) =
                    if let Some(root) = direct_write_only_root(program, cursor, roots) {
                        (root, root.referee, root.receiver_machine.is_valid())
                    } else {
                        let (root, field) = bare_field(program, cursor, roots)?;
                        if field.relevance.is_erased() {
                            return None;
                        }
                        (root, field.type_reference, false)
                    };
                let mut access = root.access;
                for projection in projections.into_iter().rev() {
                    referee = projected_referee(program, referee, &mut access)?;
                    match program.expression_table.expression(projection) {
                        ExpressionNode::Member(member) => {
                            let field = if attached_root {
                                super::field(program, root, projection)?
                            } else {
                                let field =
                                    crate::value_custody::places::applied_data_member_field(
                                        program, referee, member,
                                    )?;
                                if access == language_semantics::ReferenceAccess::WriteOnly {
                                    write_only_record_shape(program, referee)?;
                                }
                                let owner = crate::value_custody::places::data_definition_for_type(
                                    program, referee,
                                )?;
                                if matches!(endpoint, ProjectionEndpoint::Referent)
                        && !crate::declarations::data_queries::data_field_is_requires_independent(
                            program, owner, field,
                        )
                    {
                        return None;
                    }
                                field
                            };
                            if field.relevance.is_erased() {
                                return None;
                            }
                            referee = field.type_reference;
                        }
                        ExpressionNode::Indexed(indexed) => {
                            let (element, length) =
                                if access == language_semantics::ReferenceAccess::WriteOnly {
                                    fixed_discardable_write_only_array_shape(program, referee)?
                                } else {
                                    let array = program
                                        .type_reference_table
                                        .unconstrained_type_reference(referee);
                                    let typed_trees::types::TypeReferenceNode::FixedArray {
                                        element_type,
                                        ..
                                    } = program.type_reference_table.type_reference(array)
                                    else {
                                        return None;
                                    };
                                    (
                                        *element_type,
                                        usize::try_from(
                                            program.declared_array_extent(array)?.literal()?,
                                        )
                                        .ok()?,
                                    )
                                };
                            if !program.expression_table.expression_is_valid(indexed.index)
                                || !admit_selector(indexed.index, length)
                            {
                                return None;
                            }
                            let (machine, state) =
                                crate::machine_calls::calls::machine_state_by_symbol(
                                    program,
                                    program.symbols.get(root.symbol).parent,
                                )?;
                            if !crate::place_has_builtin_coordinates(
                                program,
                                machine,
                                Some(state),
                                projection,
                            ) {
                                return None;
                            }
                            referee = element;
                        }
                        _ => return None,
                    }
                    attached_root = false;
                }
                if matches!(endpoint, ProjectionEndpoint::Referent) {
                    referee = projected_referee(program, referee, &mut access)?;
                }
                Some(ResolvedProjection {
                    type_reference: referee,
                    attached_root,
                    access,
                })
            }

            fn projected_referee(
                program: &dyn crate::CheckingInput,
                mut referee: TypeReferenceHandle,
                access: &mut language_semantics::ReferenceAccess,
            ) -> Option<TypeReferenceHandle> {
                use language_semantics::ReferenceAccess;
                use typed_trees::types::TypeReferenceNode;
                while let TypeReferenceNode::Reference {
                    referee: target,
                    access: target_access,
                    ..
                } = program.type_reference_table.type_reference(referee)
                {
                    if *access != ReferenceAccess::Mutable || !target_access.is_exclusive() {
                        return None;
                    }
                    *access = *target_access;
                    referee = *target;
                }
                Some(referee)
            }
        }
        pub(crate) use projections::nonobserving_projection;
        pub(crate) use projections::projected;
        pub(crate) use projections::storage_projection;

        pub(crate) fn record<'program>(
            program: &'program dyn crate::CheckingInput,
            root: &WriteOnlyRoot,
        ) -> Option<&'program DataDefinition> {
            let machine = machine(program, root)?;
            let definition =
                closed_write_only_data_by_symbol(program, machine.attached_data_symbol)?;
            (DataDefinition::shape_kind_from_members(program.data_members(definition))
                == DataShapeKind::Record)
                .then_some(definition)
        }

        pub(crate) fn field<'program>(
            program: &'program dyn crate::CheckingInput,
            root: &WriteOnlyRoot,
            expression: ExpressionHandle,
        ) -> Option<&'program DataField> {
            let definition = projection_record(program, root)?;
            let field = crate::exact_self_field(program, machine(program, root)?, expression)?;
            crate::declarations::data_queries::data_field_is_requires_independent(
                program, definition, field,
            )
            .then_some(field)
        }

        fn projection_record<'program>(
            program: &'program dyn crate::CheckingInput,
            root: &WriteOnlyRoot,
        ) -> Option<&'program DataDefinition> {
            let definition = closed_write_only_data_shape_by_symbol(
                program,
                machine(program, root)?.attached_data_symbol,
            )?;
            (DataDefinition::shape_kind_from_members(program.data_members(definition))
                == DataShapeKind::Record)
                .then_some(definition)
        }

        fn machine<'program>(
            program: &'program dyn crate::CheckingInput,
            root: &WriteOnlyRoot,
        ) -> Option<&'program Machine> {
            if !root.receiver_machine.is_valid() {
                return None;
            }
            program.machine(root.receiver_machine)
        }

        pub(crate) fn matches_name(root: &WriteOnlyRoot, path: &TableNamePath) -> bool {
            path.head_symbol == root.symbol
        }

        pub(crate) fn mentions_name(
            program: &dyn crate::CheckingInput,
            root: &WriteOnlyRoot,
            path: &TableNamePath,
        ) -> bool {
            matches_name(root, path)
                || expression_table(program)
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| {
                        attached_field(program, root, path.head_symbol, name.as_str()).is_some()
                    })
        }

        pub(crate) fn bare_field<'program, 'roots>(
            program: &'program dyn crate::CheckingInput,
            expression: ExpressionHandle,
            roots: &'roots [WriteOnlyRoot],
        ) -> Option<(&'roots WriteOnlyRoot, &'program DataField)> {
            let (path, name) = expression_table(program).single_name_member(expression)?;
            roots.iter().find_map(|root| {
                attached_field(program, root, path.head_symbol, name.as_str())
                    .map(|field| (root, field))
            })
        }

        fn attached_field<'program>(
            program: &'program dyn crate::CheckingInput,
            root: &WriteOnlyRoot,
            symbol: SymbolHandle,
            name: &str,
        ) -> Option<&'program DataField> {
            if !symbol.is_valid() {
                return None;
            }
            let definition = projection_record(program, root)?;
            let field = crate::value_custody::places::exact_attached_field(
                program,
                machine(program, root)?,
                symbol,
                name,
            )?;
            crate::declarations::data_queries::data_field_is_requires_independent(
                program, definition, field,
            )
            .then_some(field)
        }

        pub(crate) fn validate_statement_call(
            machine: &str,
            state: &str,
            call: &TableCall,
            roots: &[WriteOnlyRoot],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for root in roots {
                if call.receiver_root_symbol == root.symbol {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{machine}` state `{state}` calls through write-only parameter `{}`; dispatch requires an exact checked `&write self` target on the whole receiver or an eligible record-field projection",
                root.name,
            )));
                }
            }
        }
    }

    #[derive(Clone)]
    pub(crate) struct WriteOnlyRoot {
        symbol: SymbolHandle,
        receiver_machine: SymbolHandle,
        name: String,
        referee: TypeReferenceHandle,
        access: ReferenceAccess,
    }

    fn declared_reference_shell(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        reference: TypeReferenceHandle,
    ) -> Option<crate::ReferenceShell> {
        let substitutions =
            crate::value_custody::places::declaration_type_equations(program, machine.symbol);
        crate::value_custody::reference_shells::reference_shell_with_substitutions(
            program,
            reference,
            &substitutions,
        )
    }

    fn has_checked_write_only_implementation(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
    ) -> bool {
        machine.supplies_checked_body()
            || typed_trees::typed_trees::AccessExecutionDeclaration::authenticate(
                program,
                machine.symbol,
            )
            .is_ok_and(|contract| contract.is_some())
            || crate::machine_calls::calls::target_family_members(program, machine).is_some_and(
                |members| {
                    members.iter().all(|member| {
                        program
                            .machine(member.machine)
                            .is_some_and(Machine::supplies_checked_body)
                    })
                },
            )
            || satisfied_by_checked_implementations(program, machine.symbol)
    }

    fn satisfied_by_checked_implementations(
        program: &dyn crate::CheckingInput,
        requirement: symbols::SymbolHandle,
    ) -> bool {
        let requirement = program
            .closed_requirement_application(requirement)
            .map_or(requirement, |application| application.template);
        let satisfiers =
            crate::machine_calls::denotational_calls::requirement_satisfiers(program, requirement);
        let named = program
            .conformances()
            .iter()
            .flat_map(|conformance| conformance.rows.iter())
            .filter(|row| row.requirement == requirement);
        let mut has_implementation = !satisfiers.is_empty();
        let checked_machines = satisfiers.iter().all(|satisfier| {
            program
                .machine(*satisfier)
                .is_some_and(Machine::supplies_checked_body)
        });
        let checked_named = named.into_iter().all(|row| {
            has_implementation = true;
            program
                .machine(row.realization_machine)
                .is_some_and(Machine::supplies_checked_body)
        });
        has_implementation && checked_machines && checked_named
    }

    pub(crate) fn validate_checked_write_only_slice(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut bound_lookup = None;
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut roots = program
                    .state_parameters(state)
                    .iter()
                    .filter_map(|parameter| {
                        let shell =
                            declared_reference_shell(program, machine, parameter.type_reference)?;
                        if shell.access != ReferenceAccess::WriteOnly {
                            return None;
                        }
                        Some(WriteOnlyRoot {
                            symbol: parameter.symbol,
                            receiver_machine: { SymbolHandle::invalid() },
                            name: parameter.name.as_str().to_owned(),
                            referee: shell.referee,
                            access: ReferenceAccess::WriteOnly,
                        })
                    })
                    .collect::<Vec<_>>();
                roots.extend(
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter_map(|statement| {
                            let local = statement.as_local_data()?;
                            let shell =
                                declared_reference_shell(program, machine, local.type_reference)?;
                            if shell.access != ReferenceAccess::WriteOnly {
                                return None;
                            }
                            Some(WriteOnlyRoot {
                                symbol: local.symbol,
                                receiver_machine: SymbolHandle::invalid(),
                                name: local.name.as_str().to_owned(),
                                referee: shell.referee,
                                access: ReferenceAccess::WriteOnly,
                            })
                        }),
                );
                let opaque_write_only = !has_checked_write_only_implementation(program, machine)
                    && program.state_parameters(state).iter().any(|parameter| {
                        matches!(
                            crate::borrow_contracts::input_write_only_access(
                                program,
                                parameter.type_reference,
                                program.machine_type_parameters(machine),
                                &[],
                            ),
                            crate::borrow_contracts::InputWriteOnlyAccess::Present
                                | crate::borrow_contracts::InputWriteOnlyAccess::Incomplete
                        )
                    });
                if !roots.is_empty() || opaque_write_only {
                    if !has_checked_write_only_implementation(program, machine) {
                        diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` declares `&write`, but the current milestone proves non-observation only for checked Omega bodies; bodyless boundary, accepted, requirement, and external-provider declarations require an admitted write-only boundary claim",
                        machine.name,
                        state.name,
                    )));
                    }

                    for root in &roots {
                        if !is_admitted_checked_referee(program, root.referee)
                            && receiver::record(program, root).is_none()
                        {
                            diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` state `{}` parameter `{}` uses `&write` with `{}`; the current checked slice supports unrestricted primitive scalars, integer scalars qualified only by closed literal ranges or a lone arithmetic policy, carriers qualified only by plain declared domains, recursively literal fixed arrays whose ultimate elements are unrestricted primitive scalars or statically discardable material records or sums, forwarding-only byte slices, closed invariant-free checked records, and closed statically discardable material sums as atomic whole values",
                            machine.name,
                            state.name,
                            root.name,
                            program.display_type_reference_with_constraints(root.referee),
                        )));
                        }
                    }
                }

                for statement in program.statement_table.statements(state.statement_nodes) {
                    validate_statement(
                        program,
                        machine,
                        state,
                        statement,
                        &roots,
                        &mut bound_lookup,
                        diagnostics,
                    );
                }
            }
        }
    }

    fn is_unrestricted_scalar(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let TypeReferenceNode::Named { name, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        !name.as_str().starts_with("Atomic")
            && program.primitive_type_reference(type_reference).is_some()
    }

    fn fixed_discardable_write_only_array_shape(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, usize)> {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        is_discardable_write_only_array_element(program, *element_type)
            .then_some((*element_type, *length))
    }

    fn fixed_discardable_write_only_array_length(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<usize> {
        fixed_discardable_write_only_array_shape(program, type_reference).map(|(_, length)| length)
    }

    fn is_admitted_checked_referee(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        is_discardable_write_only_field_leaf(program, type_reference)
            || is_qualified_write_only_leaf(program, type_reference)
            || is_byte_slice(program, type_reference)
            || write_only_record(program, type_reference).is_some()
            || is_discardable_write_only_sum(program, type_reference)
    }

    fn is_qualified_write_only_leaf(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        crate::is_arithmetic_policy_only_integer(program, type_reference)
            || is_plain_domain_qualified_leaf(program, type_reference)
    }

    fn is_byte_slice(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Slice { element_type }
                if program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
        )
    }

    fn is_write_only_length_metadata(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> bool {
        crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            member.receiver,
        )
        .and_then(|reference| {
            crate::collection_operations::collection_measure_member(
                program,
                member,
                crate::collection_operations::MeasureReceiver::Declared(reference),
            )
        }) == Some(CollectionMeasure::Length)
    }

    fn closed_write_only_data(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&DataDefinition> {
        let definition = write_only_data_shape(program, type_reference)?;
        definition.requires_facts.is_empty().then_some(definition)
    }

    fn write_only_data_shape(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&DataDefinition> {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        let definition = program.closed_data_definition(carrier)?;
        checked_write_only_data_shape(program, definition)
    }

    fn closed_write_only_data_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&DataDefinition> {
        let definition = closed_write_only_data_shape_by_symbol(program, symbol)?;
        definition.requires_facts.is_empty().then_some(definition)
    }

    fn closed_write_only_data_shape_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&DataDefinition> {
        let definition =
            crate::declarations::data_queries::data_definition_by_symbol(program, symbol)?;
        checked_write_only_data_shape(program, definition)
    }

    fn checked_write_only_data_shape<'program>(
        program: &dyn crate::CheckingInput,
        definition: &'program DataDefinition,
    ) -> Option<&'program DataDefinition> {
        (definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
            && program.data_type_parameters(definition).is_empty()
            && definition.quotient.is_none()
            && !definition.zero_gated)
            .then_some(definition)
    }

    fn write_only_record_shape(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&DataDefinition> {
        let definition = write_only_data_shape(program, type_reference)?;
        (DataDefinition::shape_kind_from_members(program.data_members(definition))
            == DataShapeKind::Record)
            .then_some(definition)
    }

    fn write_only_record(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&DataDefinition> {
        let definition = closed_write_only_data(program, type_reference)?;
        (DataDefinition::shape_kind_from_members(program.data_members(definition))
            == DataShapeKind::Record)
            .then_some(definition)
    }

    fn is_discardable_write_only_data(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
    ) -> bool {
        definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
            || crate::data_has_contents(
                program,
                definition,
                &[],
                crate::ContentsRequirement::RepresentedOwned,
            )
            || crate::data_has_contents(
                program,
                definition,
                &[],
                crate::ContentsRequirement::DescriptorCleanup,
            )
    }

    fn is_discardable_write_only_record(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        write_only_record_shape(program, type_reference)
            .is_some_and(|definition| is_discardable_write_only_data(program, definition))
    }

    fn is_discardable_write_only_sum(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        closed_write_only_data(program, type_reference).is_some_and(|definition| {
            is_discardable_write_only_data(program, definition)
                && DataDefinition::shape_kind_from_members(program.data_members(definition))
                    == DataShapeKind::Enum
                && program.data_members(definition).iter().all(|member| {
                    let Some(variant) = member.as_variant() else {
                        return false;
                    };
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .all(|field| !field.relevance.is_erased())
                })
        })
    }

    fn is_discardable_write_only_array_element(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        is_unrestricted_scalar(program, type_reference)
            || is_qualified_write_only_leaf(program, type_reference)
            || fixed_discardable_write_only_array_shape(program, type_reference).is_some()
            || write_only_record(program, type_reference).is_some_and(|definition| {
                is_discardable_write_only_data(program, definition)
                && program.data_members(definition).iter().all(|member| {
                    matches!(member, DataMember::Field(field) if !field.relevance.is_erased())
                })
            })
            || is_discardable_write_only_sum(program, type_reference)
    }

    fn whole_root_replacement_is_admitted(
        program: &dyn crate::CheckingInput,
        root: &WriteOnlyRoot,
    ) -> bool {
        write_only_assignment_leaf(program, root.referee)
            || receiver::record(program, root)
                .is_some_and(|definition| is_discardable_write_only_data(program, definition))
    }

    fn write_only_record_field_type(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
    ) -> Option<TypeReferenceHandle> {
        if direct_write_only_root(program, expression, roots).is_some() {
            return None;
        }
        receiver::projected(program, expression, roots, &mut |_, _| false)
            .map(|projection| projection.type_reference)
    }

    fn is_discardable_write_only_field_leaf(
        program: &dyn crate::CheckingInput,
        field_type: TypeReferenceHandle,
    ) -> bool {
        is_unrestricted_scalar(program, field_type)
            || fixed_discardable_write_only_array_length(program, field_type).is_some()
            || is_discardable_write_only_record(program, field_type)
            || is_discardable_write_only_sum(program, field_type)
    }

    fn write_only_assignment_leaf(
        program: &dyn crate::CheckingInput,
        field_type: TypeReferenceHandle,
    ) -> bool {
        is_discardable_write_only_field_leaf(program, field_type)
            || is_qualified_write_only_leaf(program, field_type)
            || is_stored_reference_leaf(program, field_type)
    }

    fn is_stored_reference_leaf(
        program: &dyn crate::CheckingInput,
        field_type: TypeReferenceHandle,
    ) -> bool {
        matches!(
            program.type_reference_table.type_reference(field_type),
            TypeReferenceNode::Reference { .. }
        )
    }

    fn is_plain_domain_qualified_leaf(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        let constraints = program.type_reference_table.constraints(*constraints);
        !constraints.is_empty()
            && constraints.iter().all(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::Domain(domain)
                        if crate::value_custody::storage_contents::is_plain_value_domain(
                            program, domain,
                        )
                )
            })
            && write_only_assignment_leaf(program, *base_type)
    }

    fn projection_sources(
        program: &dyn crate::CheckingInput,
        state: &State,
        stop_before_local: Option<SymbolHandle>,
    ) -> Vec<WriteOnlyRoot> {
        let mut sources = Vec::new();
        sources.extend(
            program
                .state_parameters(state)
                .iter()
                .filter_map(|parameter| {
                    let (referee, receiver_machine) = match program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                    {
                        TypeReferenceNode::Reference { referee, .. } => {
                            (*referee, SymbolHandle::invalid())
                        }
                        _ if !parameter.is_const && !parameter.relevance.is_erased() => {
                            (parameter.type_reference, { SymbolHandle::invalid() })
                        }
                        _ => return None,
                    };
                    Some(WriteOnlyRoot {
                        symbol: parameter.symbol,
                        receiver_machine,
                        name: parameter.name.as_str().to_owned(),
                        referee,
                        access: crate::reference_shell(program, parameter.type_reference)
                            .map_or(ReferenceAccess::Mutable, |shell| shell.access),
                    })
                }),
        );
        sources.extend(
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take_while(|statement| {
                    !matches!(
                        statement,
                        StatementNode::LocalData(candidate)
                            if Some(candidate.symbol) == stop_before_local
                    )
                })
                .filter_map(|statement| {
                    let source = statement.as_local_data()?;
                    if !source.symbol.is_valid() || source.relevance.is_erased() {
                        return None;
                    }
                    let referee = match program
                        .type_reference_table
                        .type_reference(source.type_reference)
                    {
                        TypeReferenceNode::Reference {
                            referee,
                            access: ReferenceAccess::Mutable,
                            ..
                        } => *referee,
                        TypeReferenceNode::Reference { .. } => return None,
                        _ => source.type_reference,
                    };
                    Some(WriteOnlyRoot {
                        symbol: source.symbol,
                        receiver_machine: SymbolHandle::invalid(),
                        name: source.name.as_str().to_owned(),
                        referee,
                        access: crate::reference_shell(program, source.type_reference)
                            .map_or(ReferenceAccess::Mutable, |shell| shell.access),
                    })
                }),
        );
        sources
    }

    fn write_only_call_subloan(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        target: ExpressionHandle,
        declared_parameter: Option<TypeReferenceHandle>,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let declared_referee = declared_parameter.and_then(|parameter| {
            match program.type_reference_table.type_reference(parameter) {
                TypeReferenceNode::Reference {
                    referee,
                    access: ReferenceAccess::WriteOnly,
                    ..
                } => Some(*referee),
                _ => None,
            }
        });

        let observation_roots = roots;
        let mut sources = roots.to_vec();
        sources.extend(projection_sources(program, state, None));
        let roots = &sources;

        if let Some(root) = direct_write_only_root(program, target, roots) {
            return match declared_referee {
                Some(declared) => {
                    write_only_referee_storage_matches(program, root.referee, declared)
                        || reject_subloan_referee_mismatch(
                            program,
                            machine,
                            state,
                            target,
                            root.referee,
                            declared,
                            diagnostics,
                        )
                }
                None => true,
            };
        }

        if let Some(projection) = receiver::nonobserving_projection(
            program,
            machine,
            state,
            target,
            roots,
            observation_roots,
            diagnostics,
        ) {
            if projection.attached_root {
                return false;
            }
            let referee = projection.type_reference;
            return match declared_referee {
                Some(declared) => {
                    is_admitted_checked_referee(program, referee)
                        && (write_only_referee_storage_matches(program, referee, declared)
                            || reject_subloan_referee_mismatch(
                                program,
                                machine,
                                state,
                                target,
                                referee,
                                declared,
                                diagnostics,
                            ))
                }
                None => is_discardable_write_only_field_leaf(program, referee),
            };
        }

        false
    }

    fn write_only_referee_storage_matches(
        program: &dyn crate::CheckingInput,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
    ) -> bool {
        actual.is_valid()
            && required.is_valid()
            && crate::stored_structural_type_identity(program, actual, &[], &[])
                == crate::stored_structural_type_identity(program, required, &[], &[])
    }

    fn reject_subloan_referee_mismatch(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        target: ExpressionHandle,
        actual: TypeReferenceHandle,
        declared: TypeReferenceHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` lends `{}` of type `{}` as `&write` to a parameter declared `&write {}`; a write-only subloan requires compatible stored carriers without changing scalar storage constraints or nested reference access",
        machine.name,
        state.name,
        expression_table(program).display_name(target),
        program.display_type_reference_with_constraints(actual),
        program.display_type_reference_with_constraints(declared),
    )));
        true
    }

    fn validate_projected_store(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let mut sources = roots.to_vec();
        sources.extend(projection_sources(program, state, None));
        let Some(projection) = receiver::storage_projection(
            program,
            machine,
            state,
            expression,
            &sources,
            roots,
            diagnostics,
        ) else {
            return false;
        };
        if projection.access == ReferenceAccess::WriteOnly
            && !write_only_assignment_leaf(program, projection.type_reference)
        {
            diagnose_unsupported_write_only_assignment_target(
                program,
                machine.name.as_str(),
                state.name.as_str(),
                expression,
                roots,
                diagnostics,
            );
        }
        true
    }

    fn validate_statement<'p>(
        program: &'p dyn crate::CheckingInput,
        machine_definition: &Machine,
        state_definition: &State,
        statement: &StatementNode,
        roots: &[WriteOnlyRoot],
        bound_lookup: &mut Option<
            crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>,
        >,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let machine = machine_definition.name.as_str();
        let state = state_definition.name.as_str();
        match statement {
            StatementNode::Assembly(_) => {}
            StatementNode::AssemblyFact(fact) => {
                for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                    validate_expression(
                        program,
                        machine_definition,
                        state_definition,
                        expression,
                        roots,
                        diagnostics,
                    );
                }
            }
            StatementNode::Assignment(assignment) => {
                if let Some(root) = direct_write_only_root(program, assignment.target, roots) {
                    if !whole_root_replacement_is_admitted(program, root) {
                        diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine}` state `{state}` replaces whole write-only aggregate `{}`; whole-root replacement requires a freely discardable supported root, so replace one eligible leaf through an invariant-free record path or declare and prove a statically discardable material aggregate instead",
                        root.name,
                    )));
                    }
                } else if validate_write_only_fixed_array_range_assignment(
                    program,
                    machine_definition,
                    state_definition,
                    assignment.target,
                    roots,
                    bound_lookup,
                    diagnostics,
                ) || validate_projected_store(
                    program,
                    machine_definition,
                    state_definition,
                    assignment.target,
                    roots,
                    diagnostics,
                ) {
                } else if let Some(index) =
                    write_only_element_assignment_index(program, assignment.target, roots)
                {
                    validate_expression(
                        program,
                        machine_definition,
                        state_definition,
                        index,
                        roots,
                        diagnostics,
                    );
                } else if mentioned_write_only_root(program, assignment.target, roots).is_some() {
                    diagnose_unsupported_write_only_assignment_target(
                        program,
                        machine,
                        state,
                        assignment.target,
                        roots,
                        diagnostics,
                    );
                } else {
                    validate_expression(
                        program,
                        machine_definition,
                        state_definition,
                        assignment.target,
                        roots,
                        diagnostics,
                    );
                }
                let receiving =
                    crate::value_custody::field_domains::assignment_target_type_reference(
                        program,
                        machine_definition,
                        state_definition,
                        assignment.target,
                    )
                    .map(|destination| {
                        crate::value_custody::places::assignment_value_type(program, destination)
                    });
                validate_received_value(
                    program,
                    machine_definition,
                    state_definition,
                    assignment.value,
                    receiving,
                    roots,
                    diagnostics,
                );
            }
            StatementNode::Call(call) => {
                validate_opaque_call_inputs(
                    program,
                    machine_definition,
                    state_definition,
                    call.target_symbol,
                    &call.machine_arguments,
                    program.statement_table.expression_handles(call.arguments),
                    diagnostics,
                );
                receiver::validate_statement_call(machine, state, call, roots, diagnostics);
                let declared_parameters = call_receiving_parameters(
                    program,
                    machine_definition,
                    state_definition,
                    call.target_symbol,
                    program.statement_table.expression_handles(call.arguments),
                );
                for (index, argument) in program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .enumerate()
                {
                    validate_received_value(
                        program,
                        machine_definition,
                        state_definition,
                        *argument,
                        declared_parameters.get(index).copied(),
                        roots,
                        diagnostics,
                    );
                }
            }
            StatementNode::Expression(expression) => validate_received_value(
                program,
                machine_definition,
                state_definition,
                *expression,
                Some(state_definition.return_type),
                roots,
                diagnostics,
            ),
            StatementNode::LocalData(local)
                if local_formation::validate(
                    program,
                    machine_definition,
                    state_definition,
                    local,
                    roots,
                    diagnostics,
                ) => {}
            StatementNode::LocalData(local) => validate_received_value(
                program,
                machine_definition,
                state_definition,
                local.initial_value,
                Some(local.type_reference),
                roots,
                diagnostics,
            ),
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    validate_expression(
                        program,
                        machine_definition,
                        state_definition,
                        guard,
                        roots,
                        diagnostics,
                    );
                }
                validate_transition_target(
                    program,
                    machine_definition,
                    state_definition,
                    transition.target,
                    roots,
                    diagnostics,
                );
                validate_transition_target(
                    program,
                    machine_definition,
                    state_definition,
                    transition.continuation,
                    roots,
                    diagnostics,
                );
            }
        }
    }

    fn validate_write_only_fixed_array_range_assignment<'p>(
        program: &'p dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        bound_lookup: &mut Option<
            crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>,
        >,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(indexed) = expression_table(program).indexed_node(expression) else {
            return false;
        };
        let collection_type =
            if let Some(root) = direct_write_only_root(program, indexed.collection, roots) {
                root.referee
            } else {
                let Some(collection_type) =
                    write_only_record_field_type(program, indexed.collection, roots)
                else {
                    return false;
                };
                let Some(_) = mentioned_write_only_root(program, indexed.collection, roots) else {
                    return false;
                };
                collection_type
            };
        let Some((_, collection_len)) =
            fixed_discardable_write_only_array_shape(program, collection_type)
        else {
            return false;
        };
        let bound_lookup = bound_lookup.get_or_insert_with(|| {
            crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup::new(program)
        });
        let Some(range) = expression_table(program).range_node(indexed.index) else {
            return false;
        };

        if range.start.is_valid() {
            validate_expression(program, machine, state, range.start, roots, diagnostics);
        }
        if range.end.is_valid() {
            validate_expression(program, machine, state, range.end, roots, diagnostics);
        }

        let start = if range.start.is_valid() {
            crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.start)
        } else {
            Some(0)
        };
        let end = if range.end.is_valid() {
            crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.end)
                .and_then(|end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                })
        } else {
            Some(collection_len)
        };
        let (Some(start), Some(end)) = (start, end) else {
            diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` replaces a write-only fixed-array range whose bounds are not statically known; exact range replacement currently requires literal bounds or finite immutable local-copy aliases",
            machine.name, state.name,
        )));
            return true;
        };

        if start > end || end > collection_len {
            return true;
        }
        true
    }

    fn write_only_element_assignment_index(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
    ) -> Option<ExpressionHandle> {
        let indexed = expression_table(program).indexed_node(expression)?;
        let (collection_type, direct_byte_slice) =
            if let Some(root) = direct_write_only_root(program, indexed.collection, roots) {
                (root.referee, is_byte_slice(program, root.referee))
            } else {
                (
                    write_only_record_field_type(program, indexed.collection, roots)?,
                    false,
                )
            };
        if direct_byte_slice {
            return (!matches!(
                expression_table(program).expression(indexed.index),
                ExpressionNode::Range(_)
            ))
            .then_some(indexed.index);
        }
        let length = fixed_discardable_write_only_array_length(program, collection_type)
            .or_else(|| predicate_byte_array_length(program, collection_type))?;
        match expression_table(program).expression(indexed.index) {
            ExpressionNode::Range(_) => None,
            ExpressionNode::Integer(index) => {
                let index = usize::try_from(index.value_i64()?).ok()?;
                (index < length).then_some(indexed.index)
            }
            _ => Some(indexed.index),
        }
    }

    fn predicate_byte_array_length(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<usize> {
        if !is_plain_domain_qualified_leaf(program, type_reference) {
            return None;
        }
        let domains =
            crate::value_custody::field_domains::domain_constraint_symbols(program, type_reference);
        if domains.is_empty()
            || domains.iter().any(|domain| {
                typed_trees::byte_predicates::domain_byte_predicate(program, *domain).is_none()
            })
        {
            return None;
        }
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(
            program
                .type_reference_table
                .unconstrained_type_reference(type_reference),
        )
        else {
            return None;
        };
        (program.primitive_type_reference(*element_type)
            == Some(typed_trees::types::PrimitiveType::U8))
        .then_some(*length)
    }

    fn diagnose_unsupported_write_only_assignment_target(
        program: &dyn crate::CheckingInput,
        machine: &str,
        state: &str,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if let Some(indexed) = expression_table(program).indexed_node(expression)
            && let Some(root) = direct_write_only_root(program, indexed.collection, roots)
            && fixed_discardable_write_only_array_length(program, root.referee).is_some()
        {
            let detail = match expression_table(program).expression(indexed.index) {
                ExpressionNode::Range(_) => "range projection is not implemented",
                ExpressionNode::Integer(index) => match index.value_i64() {
                    Some(value) if value < 0 => "the index must be non-negative",
                    Some(_) => "the literal index is outside the fixed array",
                    None => "the literal index is outside the supported index range",
                },
                _ => "the index expression is not an admissible element place",
            };
            diagnostics.push(Diagnostic::error(format!(
            "machine `{machine}` state `{state}` writes through unsupported projection of write-only fixed array `{}`; {detail}; whole-array replacement, proven-in-bounds element replacement, and statically normalized closed-range replacement are accepted",
            root.name,
        )));
            return;
        }

        diagnostics.push(Diagnostic::error(format!(
        "machine `{machine}` state `{state}` writes through an unsupported write-only projection; the destination must have content-independent coordinates, compatible write authority, and a supported replaceable leaf. Stores retain their complete type, bounds, default-domain and displaced-custody obligations; locating a destination grants no permission to read its contents"
    )));
    }

    fn validate_transition_target(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        target: typed_trees::statement::TransitionTargetHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match program.statement_table.transition_target(target) {
            TransitionTargetNode::Named {
                path, arguments, ..
            } => {
                let declared_parameters =
                    crate::declarations::transitions::resolved_transition_target_state(
                        program,
                        path.symbol,
                    )
                    .map(|(_, callee_state)| declared_state_parameters(program, callee_state))
                    .unwrap_or_default();
                validate_arguments_against_parameters(
                    program,
                    machine,
                    state,
                    program.statement_table.expression_handles(*arguments),
                    &declared_parameters,
                    roots,
                    diagnostics,
                );
            }
            TransitionTargetNode::Value(value) => validate_received_value(
                program,
                machine,
                state,
                *value,
                Some(state.return_type),
                roots,
                diagnostics,
            ),
            TransitionTargetNode::Terminal => {}
        }
    }

    fn validate_arguments_against_parameters(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        argument_handles: &[ExpressionHandle],
        declared_parameters: &[TypeReferenceHandle],
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (index, argument) in argument_handles.iter().enumerate() {
            validate_received_value(
                program,
                machine,
                state,
                *argument,
                declared_parameters.get(index).copied(),
                roots,
                diagnostics,
            );
        }
    }

    fn call_receiving_parameters(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        target: symbols::SymbolHandle,
        arguments: &[ExpressionHandle],
    ) -> Vec<TypeReferenceHandle> {
        if let Some(parameters) = crate::compiler_operations::global_descriptor_table_operand_types(
            program, machine, state, target, arguments,
        ) {
            return parameters.to_vec();
        }
        crate::machine_calls::calls::machine_state_by_symbol(program, target)
            .map(|(_, callee_state)| declared_state_parameters(program, callee_state))
            .unwrap_or_default()
    }

    fn declared_state_parameters(
        program: &dyn crate::CheckingInput,
        state: &State,
    ) -> Vec<TypeReferenceHandle> {
        program
            .state_parameters(state)
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect()
    }

    macro_rules! walk {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr) => {
            validate_expression($a, $b, $c, $d, $e, $f)
        };
    }

    fn validate_received_value(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        receiving_type: Option<TypeReferenceHandle>,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let constructed = expression_table(program).constructed_value_type_reference(expression);
        let receiving = constructed
            .is_valid()
            .then_some(constructed)
            .or(receiving_type);
        match expression_table(program).expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                for field in expression_table(program).struct_fields(literal.fields) {
                    let required = receiving
                        .and_then(|reference| {
                            program
                                .closed_data_constructor_field(
                                    reference,
                                    literal.type_symbol,
                                    field.field_symbol,
                                )
                                .map(|selected| selected.type_reference)
                        })
                        .or_else(|| {
                            crate::selected_construction_field_type(
                                program,
                                expression,
                                literal,
                                field.field_symbol,
                            )
                        });
                    validate_received_value(
                        program,
                        machine,
                        state,
                        field.value,
                        required,
                        roots,
                        diagnostics,
                    );
                }
                return;
            }
            ExpressionNode::ArrayLiteral(values) => {
                let element = receiving.and_then(|reference| {
                    match program.type_reference_table.type_reference(
                        program
                            .type_reference_table
                            .unconstrained_type_reference(reference),
                    ) {
                        TypeReferenceNode::FixedArray { element_type, .. }
                        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                        _ => None,
                    }
                });
                for value in expression_table(program).expression_handles(*values) {
                    validate_received_value(
                        program,
                        machine,
                        state,
                        *value,
                        element,
                        roots,
                        diagnostics,
                    );
                }
                return;
            }
            _ => {}
        }
        if receiving_type.is_some_and(|reference| {
            declared_reference_shell(program, machine, reference)
                .is_some_and(|shell| shell.access == ReferenceAccess::WriteOnly)
        }) && let ExpressionNode::Match(dispatch) =
            expression_table(program).expression(expression)
        {
            walk!(program => machine => state => dispatch.subject => roots => diagnostics);
            for arm in expression_table(program).match_arms(dispatch.arms) {
                for pattern in expression_table(program).match_pattern_operands(arm.pattern) {
                    validate_expression(program, machine, state, pattern, roots, diagnostics);
                }
                validate_received_value(
                    program,
                    machine,
                    state,
                    arm.value,
                    receiving_type,
                    roots,
                    diagnostics,
                );
            }
            return;
        }
        if let Some(borrow) = expression_table(program).borrow_expression(expression)
            && borrow.access == ReferenceAccess::WriteOnly
            && write_only_call_subloan(
                program,
                machine,
                state,
                borrow.target,
                receiving_type,
                roots,
                diagnostics,
            )
        {
            return;
        }
        if let Some(required) = receiving_type
            && let Some(produced) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            )
            && let Some(required) = declared_reference_shell(program, machine, required)
            && let Some(produced) = declared_reference_shell(program, machine, produced)
            && required.access == ReferenceAccess::WriteOnly
            && produced.access == ReferenceAccess::WriteOnly
            && crate::value_custody::type_references::type_references_match(
                program,
                produced.referee,
                required.referee,
            )
        {
            match expression_table(program).expression(expression) {
                ExpressionNode::Call(call) => {
                    validate_call_expression(program, machine, state, call, roots, diagnostics);
                    return;
                }
                ExpressionNode::Name(_) => return,
                ExpressionNode::Member(member) => {
                    validate_expression(
                        program,
                        machine,
                        state,
                        member.receiver,
                        roots,
                        diagnostics,
                    );
                    return;
                }
                ExpressionNode::Indexed(indexed) => {
                    walk!(program => machine => state => indexed.collection => roots => diagnostics);
                    validate_expression(program, machine, state, indexed.index, roots, diagnostics);
                    return;
                }
                _ => {}
            }
        }
        validate_expression(program, machine, state, expression, roots, diagnostics);
    }

    fn validate_opaque_call_inputs(
        program: &dyn crate::CheckingInput,
        caller: &Machine,
        state: &State,
        target: SymbolHandle,
        selected: &[typed_trees::expression::StaticMachineArgument],
        arguments: &[ExpressionHandle],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut parameters = program.machine_type_parameters(caller).to_vec();
        let (name, inputs, bindings) = if let Some((callee, entry)) =
            crate::machine_calls::calls::machine_state_by_symbol(program, target)
        {
            if has_checked_write_only_implementation(program, callee) {
                return;
            }
            let Some(bindings) = crate::machine_calls::calls::call_type_arguments(
                program,
                caller,
                Some(state),
                callee,
                entry,
                selected,
                arguments,
            ) else {
                return;
            };
            parameters.extend_from_slice(program.machine_type_parameters(callee));
            (
                callee.name.as_str(),
                program.state_parameters(entry),
                bindings,
            )
        } else if let Some((declaration, signature)) =
            crate::machine_calls::calls::selected_requirement_signature(program, target)
                .filter(|(declaration, _)| declaration.is_boundary)
        {
            if satisfied_by_checked_implementations(program, signature.symbol) {
                return;
            }
            let local_parameters = program.state_signature_type_parameters(signature);
            let bindings = if local_parameters.is_empty() {
                Some(Vec::new())
            } else {
                crate::machine_calls::calls::callable_type_arguments(
                    program,
                    caller,
                    Some(state),
                    local_parameters,
                    program.state_signature_parameters(signature),
                    selected,
                    arguments,
                )
            };
            let Some(bindings) = bindings else {
                return;
            };
            parameters.extend_from_slice(program.trait_type_parameters(declaration));
            parameters.extend_from_slice(program.state_signature_type_parameters(signature));
            (
                signature.name.as_str(),
                program.state_signature_parameters(signature),
                bindings,
            )
        } else {
            return;
        };
        if inputs.iter().any(|parameter| {
            matches!(
                crate::borrow_contracts::input_write_only_access(
                    program,
                    parameter.type_reference,
                    &parameters,
                    &bindings,
                ),
                crate::borrow_contracts::InputWriteOnlyAccess::Present
                    | crate::borrow_contracts::InputWriteOnlyAccess::Incomplete
            )
        }) {
            diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` calls `{}` with write-only input authority; an opaque selected implementation requires an admitted write-only boundary claim",
            caller.name, state.name, name,
        )));
        }
    }

    fn validate_call_expression(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        call: &typed_trees::expression::TableCallExpression,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_opaque_call_inputs(
            program,
            machine,
            state,
            call.target_symbol,
            &call.machine_arguments,
            expression_table(program).expression_handles(call.arguments),
            diagnostics,
        );
        if call.receiver.is_valid() {
            validate_expression(program, machine, state, call.receiver, roots, diagnostics);
        }
        let declared_parameters = call_receiving_parameters(
            program,
            machine,
            state,
            call.target_symbol,
            expression_table(program).expression_handles(call.arguments),
        );
        validate_arguments_against_parameters(
            program,
            machine,
            state,
            expression_table(program).expression_handles(call.arguments),
            &declared_parameters,
            roots,
            diagnostics,
        );
    }

    fn validate_expression(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let machine_name = machine.name.as_str();
        let state_name = state.name.as_str();
        let metadata = expression_table(program)
            .member_node(expression)
            .is_some_and(|member| is_write_only_length_metadata(program, machine, state, member));
        let observes_write_only = !metadata
            && validate_observation_access(program, machine, state, expression, roots, diagnostics);
        match expression_table(program).expression(expression) {
            ExpressionNode::Match(dispatch) => {
                walk!(program => machine => state => dispatch.subject => roots => diagnostics);
                for arm in expression_table(program).match_arms(dispatch.arms) {
                    for pattern in expression_table(program).match_pattern_operands(arm.pattern) {
                        validate_expression(program, machine, state, pattern, roots, diagnostics);
                    }
                    validate_expression(program, machine, state, arm.value, roots, diagnostics);
                }
            }
            ExpressionNode::SavedSubject(_) | ExpressionNode::Name(_) => {}
            ExpressionNode::Borrow(borrow) => match borrow.access {
                ReferenceAccess::WriteOnly => {
                    if let Some(binding) =
                        binding_without_write_authority(program, state, borrow.target)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine_name}` state `{state_name}` forms `&write` on `{binding}`, a binding without mutable authority; `&write` formation requires a declared `&write` root, a `&mut` place, a `mut` value binding, or a consuming `self` receiver — a plain `let` or immutable parameter cannot lend write access"
                    )));
                    } else if !is_direct_name(program, borrow.target) {
                        diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine_name}` state `{state_name}` forms `&write` from an unsupported projection or computed expression; the current checked slice supports explicit attenuation of a whole parameter, an exact direct-reborrow local, or a content-independent path through eligible relevant record fields and proven-in-bounds fixed-array indexes with non-observing selectors whose declared `&write` referee carries exactly the same type"
                    )));
                    }
                }
                ReferenceAccess::Mutable => {
                    if let Some(root) = mentioned_write_only_root(program, borrow.target, roots) {
                        diagnostics.push(Diagnostic::error(format!(
                        "machine `{machine_name}` state `{state_name}` widens write-only parameter `{}` to `&mut`; forward it explicitly as `&write {}` instead",
                        root.name, root.name,
                    )));
                    } else {
                        validate_expression(
                            program,
                            machine,
                            state,
                            borrow.target,
                            roots,
                            diagnostics,
                        );
                    }
                }
                ReferenceAccess::Shared => {
                    validate_expression(program, machine, state, borrow.target, roots, diagnostics)
                }
            },
            ExpressionNode::Member(member) => {
                if metadata {
                    validate_metadata_receiver(
                        program,
                        machine,
                        state,
                        member.receiver,
                        roots,
                        diagnostics,
                    );
                } else if !observes_write_only {
                    validate_expression(
                        program,
                        machine,
                        state,
                        member.receiver,
                        roots,
                        diagnostics,
                    );
                }
            }
            ExpressionNode::Indexed(indexed) => {
                if !observes_write_only {
                    walk!(program => machine => state => indexed.collection => roots => diagnostics);
                }
                validate_expression(program, machine, state, indexed.index, roots, diagnostics);
            }
            ExpressionNode::ArrayLiteral(_) | ExpressionNode::StructLiteral(_) => {
                validate_received_value(
                    program,
                    machine,
                    state,
                    expression,
                    None,
                    roots,
                    diagnostics,
                );
            }
            ExpressionNode::Atomic(atomic) => {
                validate_expression(program, machine, state, atomic.value, roots, diagnostics);
                validate_expression(program, machine, state, atomic.result, roots, diagnostics);
            }
            ExpressionNode::Binary(binary) => {
                validate_expression(program, machine, state, binary.left, roots, diagnostics);
                validate_expression(program, machine, state, binary.right, roots, diagnostics);
            }
            ExpressionNode::Cast(cast)
                if cast.form.recast_access() == Some(ReferenceAccess::WriteOnly) =>
            {
                validate_reference_address(program, machine, state, cast.value, roots, diagnostics)
            }
            ExpressionNode::Cast(cast) => {
                validate_expression(program, machine, state, cast.value, roots, diagnostics)
            }
            ExpressionNode::Call(call) => {
                validate_call_expression(program, machine, state, call, roots, diagnostics);
            }
            ExpressionNode::Range(range) => {
                validate_expression(program, machine, state, range.start, roots, diagnostics);
                validate_expression(program, machine, state, range.end, roots, diagnostics);
            }
            ExpressionNode::Unary(unary) => {
                validate_expression(program, machine, state, unary.operand, roots, diagnostics)
            }
            ExpressionNode::TypeExpression(_)
            | typed_trees::spelled_literal_expression_pattern!() => {}
        }
    }

    fn validate_metadata_receiver(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(_) => {}
            ExpressionNode::Member(member) => {
                reject_stored_pointer_metadata_receiver(
                    program,
                    machine,
                    state,
                    member.receiver,
                    roots,
                    diagnostics,
                );
                validate_metadata_receiver(
                    program,
                    machine,
                    state,
                    member.receiver,
                    roots,
                    diagnostics,
                );
            }
            ExpressionNode::Indexed(indexed) => {
                reject_stored_pointer_metadata_receiver(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    roots,
                    diagnostics,
                );
                validate_metadata_receiver(
                    program,
                    machine,
                    state,
                    indexed.collection,
                    roots,
                    diagnostics,
                );
                validate_expression(program, machine, state, indexed.index, roots, diagnostics);
            }
            _ => validate_expression(program, machine, state, expression, roots, diagnostics),
        }
    }

    fn reject_stored_pointer_metadata_receiver(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        receiver: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if matches!(
            expression_table(program).expression(receiver),
            ExpressionNode::Name(_)
        ) {
            return;
        }
        let Some(root) = write_only_place_root(program, receiver, roots) else {
            return;
        };
        let Some(reference) = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            receiver,
        ) else {
            return;
        };
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        ) {
            diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` reaches `.len` metadata below a stored pointer of write-only parameter `{}`; the observed descriptor lives in another referent's storage — `&write` admits descriptor metadata only below its own referent",
            machine.name, state.name, root.name,
        )));
        }
    }

    fn validate_reference_address(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut sources = roots.to_vec();
        sources.extend(projection_sources(program, state, None));
        if receiver::nonobserving_projection(
            program,
            machine,
            state,
            expression,
            &sources,
            roots,
            diagnostics,
        )
        .is_none()
        {
            validate_expression(program, machine, state, expression, roots, diagnostics);
        }
    }

    fn validate_observation_access(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        roots: &[WriteOnlyRoot],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let node = expression_table(program).expression(expression);
        if matches!(node, ExpressionNode::Borrow(_))
            || matches!(node, ExpressionNode::Cast(cast) if cast.form.is_recast())
        {
            return false;
        }
        let receiver = match node {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => ExpressionHandle::invalid(),
        };
        let root = write_only_place_root(program, expression, roots);
        let declared_write_only = [expression, receiver].into_iter().any(|value| {
            if !value.is_valid() {
                return false;
            }
            let Some(mut reference) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                value,
            ) else {
                return false;
            };
            let mut visited = Vec::new();
            while !visited.contains(&reference) {
                visited.push(reference);
                let Some(shell) = declared_reference_shell(program, machine, reference) else {
                    return false;
                };
                if shell.access == ReferenceAccess::WriteOnly {
                    return true;
                }
                reference = shell.referee;
            }
            false
        });
        if !declared_write_only && root.is_none() {
            return false;
        }
        let message = match (node, root) {
        (ExpressionNode::Name(_), Some(root)) => format!(
            "reads write-only parameter `{}`; `&write` permits replacement or exact `&write` forwarding, never observation", root.name,
        ),
        (ExpressionNode::Member(member), Some(root)) => format!(
            "reads field `{}` from write-only parameter `{}`; an eligible record-field path may be replaced as an assignment target, but write-only projection never grants observation", member.member, root.name,
        ),
        (ExpressionNode::Indexed(_), Some(root)) => format!(
            "reads through index projection of write-only parameter `{}`; `&write` permits admitted fixed-array element replacement but never observation", root.name,
        ),
        _ => "observes write-only referent through a selected value; `&write` permits replacement or exact `&write` forwarding, never observation".to_owned(),
    };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` {message}",
            machine.name, state.name,
        )));
        true
    }

    fn write_only_place_root<'roots>(
        program: &dyn crate::CheckingInput,
        mut expression: ExpressionHandle,
        roots: &'roots [WriteOnlyRoot],
    ) -> Option<&'roots WriteOnlyRoot> {
        loop {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    return roots
                        .iter()
                        .find(|root| receiver::mentions_name(program, root, path));
                }
                ExpressionNode::Member(member) => expression = member.receiver,
                ExpressionNode::Indexed(indexed) => expression = indexed.collection,
                _ => return None,
            }
        }
    }

    fn is_direct_name(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
        matches!(
            expression_table(program).expression(expression),
            ExpressionNode::Name(path)
                if program.expression_table.name_path_members(path.members).len() == 1
        )
    }

    fn binding_without_write_authority(
        program: &dyn crate::CheckingInput,
        state: &State,
        target: ExpressionHandle,
    ) -> Option<String> {
        let path = expression_table(program).name_path_node(target)?;
        if expression_table(program)
            .name_path_members(path.members)
            .len()
            != 1
            || !path.head_symbol.is_valid()
        {
            return None;
        }
        if let Some(parameter) = program.state_parameter_by_symbol(state, path.head_symbol) {
            return (!parameter.is_mutable
                && !matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { .. }
                ))
            .then(|| parameter.name.as_str().to_owned());
        }
        let (_, local) = program.state_local_declaration(state, path.head_symbol)?;
        (!local.is_mutable
            && !matches!(
                program
                    .type_reference_table
                    .type_reference(local.type_reference),
                TypeReferenceNode::Reference { .. }
            ))
        .then(|| local.name.as_str().to_owned())
    }

    fn direct_write_only_root<'a>(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        roots: &'a [WriteOnlyRoot],
    ) -> Option<&'a WriteOnlyRoot> {
        let path = expression_table(program).name_path_node(expression)?;
        (expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1)
            .then(|| roots.iter().find(|root| receiver::matches_name(root, path)))
            .flatten()
    }

    fn mentioned_write_only_root<'a>(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        roots: &'a [WriteOnlyRoot],
    ) -> Option<&'a WriteOnlyRoot> {
        roots
            .iter()
            .find(|root| expression_mentions_root(program, expression, root))
    }

    fn expression_mentions_root(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        root: &WriteOnlyRoot,
    ) -> bool {
        match expression_table(program).expression(expression) {
            ExpressionNode::SavedSubject(saved) => program
                .saved_value_source(saved)
                .is_some_and(|subject| expression_mentions_root(program, subject, root)),
            ExpressionNode::Match(dispatch) => {
                crate::queries::expression_traversal::match_children(program, *dispatch)
                    .any(|child| expression_mentions_root(program, child, root))
            }
            ExpressionNode::Name(path) => receiver::mentions_name(program, root, path),
            ExpressionNode::Borrow(value) => expression_mentions_root(program, value.target, root),
            ExpressionNode::ArrayLiteral(values) => expression_table(program)
                .expression_handles(*values)
                .iter()
                .any(|value| expression_mentions_root(program, *value, root)),
            ExpressionNode::Atomic(atomic) => {
                expression_mentions_root(program, atomic.value, root)
                    || expression_mentions_root(program, atomic.result, root)
            }
            ExpressionNode::Binary(binary) => {
                expression_mentions_root(program, binary.left, root)
                    || expression_mentions_root(program, binary.right, root)
            }
            ExpressionNode::Cast(cast) => expression_mentions_root(program, cast.value, root),
            ExpressionNode::Call(call) => {
                (call.receiver.is_valid() && expression_mentions_root(program, call.receiver, root))
                    || expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| expression_mentions_root(program, *argument, root))
            }
            ExpressionNode::Indexed(indexed) => {
                expression_mentions_root(program, indexed.collection, root)
                    || expression_mentions_root(program, indexed.index, root)
            }
            ExpressionNode::Member(member) => {
                expression_mentions_root(program, member.receiver, root)
            }
            ExpressionNode::Range(range) => {
                expression_mentions_root(program, range.start, root)
                    || expression_mentions_root(program, range.end, root)
            }
            ExpressionNode::StructLiteral(literal) => expression_table(program)
                .struct_fields(literal.fields)
                .iter()
                .any(|field| expression_mentions_root(program, field.value, root)),
            ExpressionNode::Unary(unary) => expression_mentions_root(program, unary.operand, root),
            ExpressionNode::TypeExpression(_)
            | typed_trees::spelled_literal_expression_pattern!() => false,
        }
    }
}

pub use access_views::access_atomic_observing_results;
