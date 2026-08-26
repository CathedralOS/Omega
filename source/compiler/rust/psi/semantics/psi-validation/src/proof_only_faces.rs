//! The proof-only fence (math roster N1): every RUNTIME consumption face
//! refuses proof-only data with the classification named. Legal homes for
//! proof-only types are fact positions (contracts, lemmas -- N2 evaluates
//! them); a machine's storage, params, locals, returns, and wire schemas
//! are runtime faces. Declaring proof-only data is always legal; the
//! contagion (a runtime-looking holder BECOMES proof-only) is computed in
//! `psi_typed_trees::proof_only`, not refused here.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::proof_only::ProofOnlyClassification;
use psi_typed_trees::statement::StatementNode;

pub(crate) fn validate_proof_only_consumption(
    program: &TypedTrees,
    classification: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
                        psi_typed_trees::data::TypeParameterKind::Machine { .. }
                    )
                });
        if has_machine_parameter
            && definition.supply_mode != psi_language_semantics::DataSupplyMode::BoundaryOpaque
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
        if !(properties.multiplicity != psi_language_semantics::Multiplicity::Affine
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
        // `Content<A>::project` is compiler-normalized proof material. It may
        // symbolically embed runtime carrier fields into a proof-only algebra,
        // but the machine is never emitted or called at runtime.
        if crate::content_projections::is_content_projection_machine(program, machine) {
            continue;
        }
        // A computed proof machine emits no runtime code. This includes both
        // free machines whose signatures mention proof-only values and
        // by-value operations attached directly to a proof-only carrier; the
        // latter's receiver is a proof term, never storage or runtime
        // dispatch. Borrowed or mutable receivers intentionally fall through
        // to the attached-storage fence below.
        if classification.is_proof_machine(program, machine) {
            continue;
        }

        // The machine's own storage: `machine Main::main` runs ON `Main`.
        if let Some(attached) = machine.attached_data.as_ref()
            && let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached.as_str())
            && let Some(reason) = classification.describe(attached.as_str(), definition.symbol)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` runs on data `{attached}`, but {reason}; proof-only data \
                 lives in facts and lemmas, never at runtime",
                machine.name
            )));
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
                if parameter.is_self {
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
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
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
            let psi_typed_trees::wire::WireMember::Field(field) = member else {
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
        for member in program.data_members(definition) {
            let fields = match member {
                psi_typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    program.data_payload_fields(variant)
                }
            };
            for field in fields {
                // An erased occurrence has no runtime pointer or inline
                // storage. Its proof-only mention is legal here and remains
                // subject to proof and multiplicity checking elsewhere.
                if field.relevance == psi_language_core::BindingRelevance::Erased {
                    continue;
                }
                if let Some(held) = classification.proof_only_mention(program, field.type_reference)
                {
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
}
