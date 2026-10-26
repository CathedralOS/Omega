//! The proof-only fence (math roster N1): every RUNTIME consumption face
//! refuses proof-only data with the classification named. Legal homes for
//! proof-only types are fact positions (contracts, lemmas -- N2 evaluates
//! them); explicit parameters, locals, returns, and wire schemas
//! are runtime faces. Declaring proof-only data is always legal; the
//! contagion (a runtime-looking holder BECOMES proof-only) is computed in
//! `typed_trees::proof_only`, not refused here.

use diagnostics::Diagnostic;
use typed_trees::proof_only::ProofOnlyClassification;

pub(crate) fn validate_proof_only_consumption(
    program: &dyn crate::CheckingInput,
    classification: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Queries in unused constants and open templates still owe formation.
    // Their physical evaluation may remain deferred or never be demanded.
    for expression in typed_trees::references::collect_program_references(program).expressions {
        if let Some(query) = program
            .expression_table
            .call_node(expression)
            .and_then(|call| call.layout_query.as_ref())
            && let Err(reason) = crate::compiler_operations::validate_data_layout_query_subject(
                program,
                classification,
                query,
                program.expression_table.source_span(expression),
            )
        {
            diagnostics.push(reason);
        }
    }
    // N7 machine-indexed data is compile-time schema metadata. It has no
    // runtime callable field and therefore only makes sense on a carrier that
    // already has no layout (recursive proof data or an opaque boundary
    // carrier). A finite checked-shape record would otherwise let the static
    // argument appear to vary a runtime layout without a representation.
    for definition in program.data_definitions() {
        let has_machine_parameter =
            program
                .data_type_parameters(definition)
                .iter()
                .any(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Machine { .. }
                    )
                });
        if has_machine_parameter
            && definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
            && !classification.is_proof_only(definition.symbol)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` is parameterized by a static machine but has a runtime layout; `<machine ...>` on data is reserved for proof-only families (recursive data has no layout)",
                definition.name
            )));
        }
    }

    // Data properties are runtime claims (`[copy]`/`[carry(...)]` speak about
    // values in memory); a proof-only carrier cannot honor them.
    for definition in program.data_definitions() {
        let properties = definition.properties;
        if !(properties.multiplicity != language_semantics::Multiplicity::Affine
            || properties.carry.is_some())
        {
            continue;
        }
        if let Some(reason) = classification.describe(definition.name.as_str(), definition.symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares runtime properties, but {reason}; properties speak \
                 about values in memory, which proof-only data never becomes",
                definition.name
            )));
        }
    }

    for machine in program.machines() {
        // `Content<Owner, A>::project` is compiler-normalized proof material. It may
        // symbolically embed runtime carrier fields into a proof-only algebra,
        // but the machine is never emitted or called at runtime.
        if crate::value_custody::content_projections::is_content_projection_machine(
            program, machine,
        ) {
            continue;
        }
        // Computed proof machines emit no runtime code. A declaration's
        // namespace contributes no storage; only explicit value occurrences
        // below are runtime consumption faces.
        if classification.is_proof_machine(program, machine) {
            continue;
        }

        // Machine-owned data slots.
        for owned_data in program.machine_owned_data(machine) {
            if let Some(held) =
                classification.proof_only_mention(program, owned_data.type_reference)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` owned data `{}` holds proof-only `{held}`, which has no \
                     runtime layout",
                    machine.name, owned_data.name
                )));
            }
        }

        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                // An erased parameter is a proof-side binding occurrence with
                // no runtime transfer, so a proof-only type there is legal --
                // the same exemption as an erased data field below.
                if parameter.relevance.is_erased() {
                    continue;
                }
                if let Some(held) =
                    classification.proof_only_mention(program, parameter.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` parameter `{}` mentions proof-only \
                         `{held}`, which has no runtime layout",
                        machine.name, state.name, parameter.name
                    )));
                }
            }

            if state.return_type.is_valid()
                && let Some(held) = classification.proof_only_mention(program, state.return_type)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` returns proof-only `{held}`, which has no \
                     runtime layout",
                    machine.name, state.name
                )));
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local_data) = statement.as_local_data() else {
                    continue;
                };
                // `let proof [erased]: T` owns no runtime storage either.
                if local_data.relevance.is_erased() {
                    continue;
                }
                if let Some(held) =
                    classification.proof_only_mention(program, local_data.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` local `{}` mentions proof-only `{held}`, \
                         which has no runtime layout",
                        machine.name, state.name, local_data.name
                    )));
                }
            }
        }
    }

    // Wire schemas serialize runtime bytes; a proof-only mention has no
    // encoding.
    for schema in program.wire_schemas() {
        for member in program.wire_members(schema.members) {
            let typed_trees::wire::WireMember::Field(field) = member else {
                continue;
            };
            // Erased numbered fields remain semantic schema/history facts but
            // have no current codec placement or runtime bytes.
            if field.relevance.is_erased() {
                continue;
            }
            if let Some(held) = classification.proof_only_mention(program, field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}` mentions proof-only `{held}`, which has no \
                     wire encoding",
                    schema.name, field.name
                )));
            }
        }
    }

    // A RUNTIME data definition may not view proof-only values through
    // indirection either (`next: &Nat`, `view: [Nat]`): the pointee never
    // materializes. Inline containment is not an error -- it makes the
    // holder proof-only (contagion), which the machine faces then fence.
    for definition in program.data_definitions() {
        if classification.is_proof_only(definition.symbol) {
            continue;
        }
        for field in program.data_member_fields(program.data_members(definition)) {
            // An erased occurrence has no runtime pointer or inline
            // storage. Its proof-only mention is legal here and remains
            // subject to proof and multiplicity checking elsewhere.
            if field.relevance == language_core::BindingRelevance::Erased {
                continue;
            }
            if let Some(held) = classification.proof_only_mention(program, field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` field `{}` views proof-only `{held}` through an \
                     indirection, but proof-only values never materialize at runtime \
                     (there is nothing to point at)",
                    definition.name, field.name
                )));
            }
        }
    }
}
