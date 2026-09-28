//! Substitutable-field inventory for the canonical Terminal module's proof
//! recursive-component roster.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `recursive_component_custody.rs`; keeping the inventory in its
//! own file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution of a
//! `TerminalModule` field that still canonicalizes under `encode_module`:
//! roster members that leave every edge's member join intact, source-free
//! custody strings that reidentify a member without moving its contract, and
//! site/path payloads that keep the (caller, site) ordering and the strict
//! member path's resolution back to the rank type. Count lies, roster-order
//! violations, emptied or unresolvable identities, unknown tags, and the
//! producer-side encode rejections stay authored in
//! `recursive_component_custody.rs` as decode- and encode-level rejections.
//!
//! Every declared leg is rejected by all three custody replays, asserted
//! inside the matrix's checker: the retained-manifest join
//! (`ArtifactManifestError::ManifestMismatch`), the verifier's reconstructed
//! recursive-component evidence join
//! (`MissingRecursiveComponentEvidence`/`UnknownRecursiveComponentEvidence`),
//! and the sealed proof section's subject join (`ProofSubjectMismatch`).

use mutation_matrix::{MutationOutcome, custody_field_inventory};
use semantic_vocabulary::ContractId;
use terminal_codec::ArtifactManifestError;
use terminal_psi::{
    TerminalModule, TerminalProofRecursiveCallSite, TerminalProofRecursiveTransitionLane,
};

custody_field_inventory! {
    /// One substitutable field of the canonical module's recursive-component
    /// custody; each leg's verdict is checked by the manifest, verifier, and
    /// sealed-proof-subject replays.
    pub enum RecursiveComponentCustodyFieldForTest {
        // --- component roster ---
        ComponentRosterCleared,
        NodeComponentDropped,
        TreeComponentDropped,
        // --- proof-type roster ---
        UnusedProofTypeDropped,
        UnusedProofTypeRenamed,
        // --- member roster ---
        MemberMachineIdentityRenamed,
        MemberRankParameterRenamed,
        TreeMemberMachineIdentityRenamed,
        // --- edge roster ---
        TransitionEdgeDropped,
        ExpressionEdgeDropped,
        EdgeCalleeSelfLoop,
        // --- call-site variants ---
        StatementSitePromotedToExpression,
        TransitionLaneFlipped,
        StatementStateIdentityRenamed,
        StatementIndexMoved,
        ExpressionOrdinalMoved,
        // --- strict member paths ---
        StrictPathSiblingField,
        TreeStrictPathSiblingField,
        StrictPathLengthened,
    }
}

/// Apply one declared substitution to the module.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor module declares no recursive components, so its fields have nothing
/// to lend this roster.
pub(crate) fn substitute_recursive_component_for_test(
    record: &mut TerminalModule,
    field: RecursiveComponentCustodyFieldForTest,
    donor: &TerminalModule,
) {
    let _ = donor;
    let components = &mut record.proof_recursive_components;
    let contract = |raw: u64| ContractId::new(raw).expect("nonzero proof contract identity");
    match field {
        RecursiveComponentCustodyFieldForTest::ComponentRosterCleared => components.clear(),
        RecursiveComponentCustodyFieldForTest::NodeComponentDropped => {
            components.remove(0);
        }
        RecursiveComponentCustodyFieldForTest::TreeComponentDropped => {
            components.remove(1);
        }
        RecursiveComponentCustodyFieldForTest::UnusedProofTypeDropped => {
            components[0].types.remove(0);
        }
        RecursiveComponentCustodyFieldForTest::UnusedProofTypeRenamed => {
            components[0].types[0].identity = "package::Leaa".into();
        }
        RecursiveComponentCustodyFieldForTest::MemberMachineIdentityRenamed => {
            components[0].members[0].machine_identity = "package::meft".into();
        }
        RecursiveComponentCustodyFieldForTest::MemberRankParameterRenamed => {
            components[0].members[0].rank_parameter_identity = "package::left::done".into();
        }
        RecursiveComponentCustodyFieldForTest::TreeMemberMachineIdentityRenamed => {
            components[1].members[0].machine_identity = "package::tree_laft".into();
        }
        RecursiveComponentCustodyFieldForTest::TransitionEdgeDropped => {
            components[0].edges.remove(5);
        }
        RecursiveComponentCustodyFieldForTest::ExpressionEdgeDropped => {
            components[0].edges.remove(2);
        }
        RecursiveComponentCustodyFieldForTest::EdgeCalleeSelfLoop => {
            components[0].edges[0].callee = contract(1001);
        }
        RecursiveComponentCustodyFieldForTest::StatementSitePromotedToExpression => {
            components[0].edges[1].site = TerminalProofRecursiveCallSite::Expression {
                state_identity: "package::left::step".into(),
                statement_index: 1,
                expression_ordinal: 9,
            };
        }
        RecursiveComponentCustodyFieldForTest::TransitionLaneFlipped => {
            let TerminalProofRecursiveCallSite::Transition { lane, .. } =
                &mut components[0].edges[5].site
            else {
                unreachable!("edge 5 carries the transition lane")
            };
            *lane = TerminalProofRecursiveTransitionLane::Target;
        }
        RecursiveComponentCustodyFieldForTest::StatementStateIdentityRenamed => {
            let TerminalProofRecursiveCallSite::Statement { state_identity, .. } =
                &mut components[0].edges[0].site
            else {
                unreachable!("edge 0 carries a statement site")
            };
            *state_identity = "package::left::stel".into();
        }
        RecursiveComponentCustodyFieldForTest::StatementIndexMoved => {
            let TerminalProofRecursiveCallSite::Statement {
                statement_index, ..
            } = &mut components[0].edges[1].site
            else {
                unreachable!("edge 1 carries a statement site")
            };
            *statement_index = 7;
        }
        RecursiveComponentCustodyFieldForTest::ExpressionOrdinalMoved => {
            let TerminalProofRecursiveCallSite::Expression {
                expression_ordinal, ..
            } = &mut components[0].edges[2].site
            else {
                unreachable!("edge 2 carries an expression site")
            };
            *expression_ordinal = 9;
        }
        RecursiveComponentCustodyFieldForTest::StrictPathSiblingField => {
            components[0].edges[0].strict_member_path[0] = "package::Node::right".into();
        }
        RecursiveComponentCustodyFieldForTest::TreeStrictPathSiblingField => {
            components[1].edges[0].strict_member_path[0] = "package::Tree::right".into();
        }
        RecursiveComponentCustodyFieldForTest::StrictPathLengthened => {
            components[0].edges[0].strict_member_path =
                vec!["package::Node::left".into(), "package::Node::left".into()];
        }
    }
}

/// Name the verdict a declared leg must produce at the retained-manifest
/// replay: every representable substitution diverges the semantic identity
/// the manifest binds, so the join always rejects with a manifest mismatch.
/// The verifier and sealed-proof-subject joins are asserted inside the
/// matrix's checker.
pub(crate) fn recursive_component_custody_outcome(
    _field: RecursiveComponentCustodyFieldForTest,
) -> MutationOutcome<ArtifactManifestError> {
    MutationOutcome::ExactError(ArtifactManifestError::ManifestMismatch)
}
