//! Validated resource-content projections retained for conservation, backing,
//! access-footprint, and artifact consumers.

use arena::HandleSpan;
use language_semantics::content::{ContentConservationPlan, ContentProjectionPlan};
use language_semantics::{PermissionClaimIdentity, ReferenceAccess, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::name::Identifier;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentProjectionFacts {
    pub plans: Vec<ContentProjectionPlan>,
    /// Source-authored n-ary conservation contracts.
    pub conservation_plans: Vec<ContentConservationPlan>,
    /// Checker-derived one-to-one content equalities. Each row is justified by
    /// one exact input-relative claim outcome and never invents a partition
    /// between otherwise independent claims.
    pub identity_reshuffles: Vec<ContentIdentityReshuffleFact>,
    /// Checked wrappers instantiated from an already-authored partition
    /// theorem. These facts retain the exact source theorem, call site,
    /// transfer-stable input claims, and any exact result-identity rewrite rows
    /// used by the substitution; they never add a new `separate(...)` node.
    /// A row may hold only on selected alternatives. Consumers requiring an
    /// exit theorem must use `partition_returns`, which records full coverage.
    pub partition_compositions: Vec<ContentPartitionCompositionFact>,
    /// Independently proved body equations joined to exact entry and returned
    /// carrier claims. These rows prove the numeric relation, not permission
    /// to consume an input or establish an output domain. A custody transaction
    /// must additionally check live ownership, predicates and compatible lineage.
    pub body_conservations: arena::Arena<ContentBodyConservationFact>,
    pub body_claim_bindings: arena::Arena<ContentBodyClaimBinding>,
    pub body_available_inputs: arena::Arena<ContentBodyAvailableInput>,
    /// Atomic owned transformations selected at an actual value completion.
    /// Their derivations retain exact parent bindings and the complete existing
    /// root roster; root identity alone is not qualification or backing evidence.
    pub body_transformations: arena::Arena<ContentBodyTransformation>,
    pub body_derivations: arena::Arena<ContentBodyDerivation>,
    /// Conditional checked-provider custody laws. These are not theorem roots
    /// for ordinary calls and do not establish a result qualification.
    pub boundary_requirement_plans: Vec<ContentConservationPlan>,
    pub boundary_body_requirements: arena::Arena<ContentBoundaryBodyRequirement>,
    pub backing_addresses: arena::Arena<ContentBackingAddressTransport>,
    /// A theorem whose exact-call derivations cover every value alternative
    /// at this ordinary outgoing occurrence. Individual composition rows may
    /// be conditional and cannot independently establish an exit guarantee.
    pub partition_returns: Vec<ContentPartitionReturnFact>,
    /// Causal ownership replacements, appended only at successful completions.
    /// These retain exact claim occurrences, not inferred address relationships.
    pub claim_transformations: arena::Arena<ContentClaimTransformation>,
    /// Exact lifetime-bound shared custody admitted for a retained foreign
    /// result. This checked-only row never grants ownership: it binds one
    /// whole direct parameter loan to one whole result and retains both exact
    /// content projections so later consumers need not reselect by spelling
    /// or trust compact report fingerprints.
    pub retained_borrow_custodies: Vec<RetainedBorrowCustodyFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentClaimTransformation {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub expression: typed_trees::expression::ExpressionHandle,
    pub evaluation: crate::FlowPermissionEvaluation,
    pub source: ContentClaimTransformationSource,
    pub inputs: HandleSpan<ContentBodyClaimBinding>,
    pub outputs: HandleSpan<ContentBodyClaimBinding>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ContentClaimTransformationSource {
    #[default]
    Unknown,
    Call {
        call_ordinal: usize,
        /// Exact authored equation, still owed by the callee's checked body.
        plan_index: usize,
    },
    Body {
        conservation: arena::Handle<ContentBodyConservationFact>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBodyTransformation {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub completed_value: arena::Handle<crate::FlowOperandFact>,
    pub evaluation: crate::FlowPermissionEvaluation,
    /// Joint accounting: no claim/qualification input or output occurs in
    /// two rows. Different qualifications on a claim remain independent.
    pub derivations: HandleSpan<ContentBodyDerivation>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBodyDerivation {
    /// Exact proved equation, original input occurrences, live parent places,
    /// unchanged entry revisions and existing raw output claim identities.
    pub conservation: arena::Handle<ContentBodyConservationFact>,
    /// Complete existing qualified roots, never identities minted by arithmetic.
    /// Each root retains its qualification; a combination does not invent a
    /// common ancestor or replace its exact input/output correspondence.
    pub lineage_roots: HandleSpan<ContentBodyClaimBinding>,
    pub custody: ContentBodyCustody,
}

/// Same-owner derivation and checked-provider custody are different authority
/// judgments. The latter transports debt/backing into an existing raw result;
/// membership introduction remains at an admitted selected invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ContentBodyCustody {
    #[default]
    Derived,
    Boundary {
        requirements: HandleSpan<ContentBoundaryBodyRequirement>,
        addresses: HandleSpan<ContentBackingAddressTransport>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBoundaryBodyRequirement {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub realization_state: SymbolHandle,
    /// Index in the private boundary requirement plan table, never the public
    /// callable theorem table. Its exact normalized law is retained there.
    pub conservation_plan_index: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBackingAddressTransport {
    pub input: arena::Handle<ContentBodyAvailableInput>,
    pub output: arena::Handle<ContentBodyClaimBinding>,
    /// Address paths relative to those exact current claim occurrences.
    pub input_segments: HandleSpan<facts::PlaceSegment>,
    pub output_segments: HandleSpan<facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBodyConservationFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub returned_at: arena::Handle<crate::FlowStateOutgoingFact>,
    /// Exact completed producer. A later outgoing use discovers this demand,
    /// but supplies no premises to this earlier observation.
    pub completed_value: arena::Handle<crate::FlowOperandFact>,
    /// Selected invocation lane at that completion. Alternative transition
    /// targets in one statement do not share an ownership frontier.
    pub evaluation: crate::FlowPermissionEvaluation,
    /// Exact equation in this owner's immutable `conservation_plans` table.
    pub plan_index: usize,
    pub inputs: HandleSpan<ContentBodyClaimBinding>,
    pub outputs: HandleSpan<ContentBodyClaimBinding>,
    /// All inputs, or empty when the ownership walk could not establish their
    /// exact live membership, unchanged projected content and absence of
    /// conflicting loans at completion. This is not a consumption receipt:
    /// complete qualification coverage, lineage compatibility and atomic
    /// output establishment remain obligations of the transformation.
    /// This observation belongs to the captured value's completion, not exit.
    pub available_inputs: HandleSpan<ContentBodyAvailableInput>,
}

/// Current custody of one equation input, observed by the ordinary permission
/// walk. A move changes this place without changing the bound entry claim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBodyAvailableInput {
    pub binding: arena::Handle<ContentBodyClaimBinding>,
    /// Complete original input roster contributing to this current claim.
    /// This records causal ancestry, not a separate content equality for each
    /// origin. Boundary backing substitutions require their own correspondence.
    pub entry_bindings: HandleSpan<ContentBodyClaimBinding>,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    pub provenance: language_semantics::PermissionProvenance,
}

/// One exact projection occurrence and its existing nominal carrier claim.
/// Adding derived authority must not mint a second carrier claim for this value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentBodyClaimBinding {
    pub claim_identity: PermissionClaimIdentity,
    pub domain: SymbolHandle,
    pub semantic_domain: SemanticDomainId,
    pub projection_machine: SymbolHandle,
    /// Entry parameter for an input; zero for a callable result.
    pub parameter_symbol: SymbolHandle,
    /// Resolved claim path in `FlowOwnershipFacts::segments`.
    pub segments: HandleSpan<facts::PlaceSegment>,
    /// An actual argument's captured value when this binding belongs to a call
    /// replacement input. Its path is relative to that value, not a later read.
    pub observation: arena::Handle<crate::FlowOperandFact>,
}

pub struct ContentClaimFrontier {
    pub bindings: Vec<ContentFrontierBinding>,
    /// Exact expected snapshot and consumed input at every replacement. The
    /// source checker must prove their projected content is unchanged.
    pub replacements: Vec<(
        arena::Handle<ContentClaimTransformation>,
        arena::Handle<ContentBodyClaimBinding>,
        arena::Handle<ContentBodyClaimBinding>,
    )>,
}

/// One current frontier claim and the exact original inputs that contributed
/// to it. Construction scratch; published observations use arena spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFrontierBinding {
    pub binding: arena::Handle<ContentBodyClaimBinding>,
    pub entries: Vec<arena::Handle<ContentBodyClaimBinding>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedBorrowCustodyFact {
    pub callable: SymbolHandle,
    pub source: language_semantics::content::ContentStructuralPlace,
    pub result: language_semantics::content::ContentStructuralPlace,
    pub access: ReferenceAccess,
    pub lifetime: Identifier,
    pub callable_lifetime_parameter_ordinal: u32,
    pub result_data: SymbolHandle,
    pub result_lifetime_argument_ordinal: u32,
    pub retained_semantic_domain: SemanticDomainId,
    pub source_projection: ContentProjectionPlan,
    pub result_projection: ContentProjectionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentIdentityReshuffleFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub claim_identity: PermissionClaimIdentity,
    pub input_parameter_symbol: SymbolHandle,
    pub input_segments: HandleSpan<facts::PlaceSegment>,
    pub output_segments: HandleSpan<facts::PlaceSegment>,
    pub plan: ContentConservationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionReturnFact {
    pub returned_at: arena::Handle<crate::FlowStateOutgoingFact>,
    pub plan: ContentConservationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionCompositionFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source_callable: SymbolHandle,
    /// Non-authoritative report coordinate; source equality uses the retained
    /// plan rather than this compact fingerprint.
    pub source_report_fingerprint: u64,
    /// Number of grounded wrapper-composition edges before this source theorem.
    /// Authored obligations alone never seed this chain.
    pub source_derivation_depth: u32,
    /// The proved or admitted theorem before caller-place substitution.
    /// Checking composes these rows at exact outgoing occurrences; the current
    /// lowering route does not export them as Terminal composition records.
    pub source_plan: ContentConservationPlan,
    pub statement_index: usize,
    pub call_ordinal: usize,
    /// Exact caller-owned normal outgoing occurrence justified by this row.
    pub returned_at: arena::Handle<crate::FlowStateOutgoingFact>,
    pub input_claim_identities: Vec<PermissionClaimIdentity>,
    /// Exact caller-entry place bound to each transferred input claim. This is
    /// independent of any one-to-one output equality; a partition input need
    /// not have an identical whole output.
    pub input_claim_bindings: Vec<ContentPartitionInputClaimBinding>,
    /// Exact call-result to callable-result claim correspondence, accompanied
    /// by preserved captured contents. Selected tail calls have no intermediate
    /// caller-owned result and therefore need no result rewrite.
    pub result_rewrites: Vec<ContentPartitionResultRewrite>,
    pub substitutions: Vec<ContentPartitionPlaceSubstitution>,
    pub plan: ContentConservationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionInputClaimBinding {
    pub claim_identity: PermissionClaimIdentity,
    pub entry_place: language_semantics::content::ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionResultRewrite {
    pub claim_identity: PermissionClaimIdentity,
    pub source: language_semantics::content::ContentStructuralPlace,
    pub target: language_semantics::content::ContentStructuralPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPartitionPlaceSubstitution {
    pub source: language_semantics::content::ContentStructuralPlace,
    pub target: language_semantics::content::ContentStructuralPlace,
}

impl ContentProjectionFacts {
    /// Replay complete replacements of the equation's joint input frontier.
    /// Every replacement parent must already belong to that exact roster;
    /// unrelated roots cannot be pulled in by equal content. Empty-content
    /// claims remain nominal owners and stay in the roster too.
    pub fn descendant_frontier(
        &self,
        entries: HandleSpan<ContentBodyClaimBinding>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        evaluation: crate::FlowPermissionEvaluation,
        before: arena::Handle<ContentBodyConservationFact>,
    ) -> Result<ContentClaimFrontier, &'static str> {
        let same = |left: arena::Handle<ContentBodyClaimBinding>,
                    right: arena::Handle<ContentBodyClaimBinding>| {
            let left = self.body_claim_bindings.get(left);
            let right = self.body_claim_bindings.get(right);
            left.claim_identity == right.claim_identity
                && left.domain == right.domain
                && left.semantic_domain == right.semantic_domain
                && left.projection_machine == right.projection_machine
        };
        let mut frontier = entries
            .handles()
            .map(|entry| ContentFrontierBinding {
                binding: entry,
                entries: vec![entry],
            })
            .collect::<Vec<_>>();
        if frontier.is_empty()
            || frontier.iter().enumerate().any(|(position, claim)| {
                frontier[..position]
                    .iter()
                    .any(|previous| same(previous.binding, claim.binding))
            })
        {
            return Err("content frontier has no complete unique input roster");
        }
        let mut replacements = Vec::new();
        for (handle, transformation) in self.claim_transformations.iter() {
            if before.is_valid()
                && transformation.source
                    == (ContentClaimTransformationSource::Body {
                        conservation: before,
                    })
            {
                break;
            }
            if transformation.machine_symbol != machine
                || transformation.state_symbol != state
                || transformation.statement_index > statement
                || (transformation.statement_index == statement
                    && transformation.evaluation != evaluation)
                || transformation.source == ContentClaimTransformationSource::Unknown
            {
                continue;
            }
            let parents = transformation.inputs.handles().collect::<Vec<_>>();
            if !parents
                .iter()
                .any(|parent| frontier.iter().any(|claim| same(*parent, claim.binding)))
            {
                continue;
            }
            if parents.is_empty()
                || transformation.outputs.is_empty()
                || !parents
                    .iter()
                    .all(|parent| frontier.iter().any(|claim| same(*parent, claim.binding)))
            {
                return Err(
                    "a completed transformation consumes an input outside the joint frontier",
                );
            }
            let mut origins = Vec::new();
            for parent in &parents {
                let expected = frontier
                    .iter()
                    .find(|claim| same(*parent, claim.binding))
                    .ok_or("replacement parent is absent")?;
                replacements.push((handle, expected.binding, *parent));
                for origin in &expected.entries {
                    if !origins.contains(origin) {
                        origins.push(*origin);
                    }
                }
            }
            frontier.retain(|claim| !parents.iter().any(|parent| same(claim.binding, *parent)));
            for output in transformation.outputs.handles() {
                if frontier.iter().any(|claim| same(claim.binding, output)) {
                    return Err("a completed transformation duplicates a qualified claim");
                }
                frontier.push(ContentFrontierBinding {
                    binding: output,
                    entries: origins.clone(),
                });
            }
        }
        Ok(ContentClaimFrontier {
            bindings: frontier,
            replacements,
        })
    }

    /// Whether one exact construction occurrence receives this membership from
    /// its enclosing completed transaction. Follow only constructing child edges:
    /// retained children of a later alias must not backdate the judgment.
    pub fn publishes_formation_membership(
        &self,
        flow: &crate::FlowFacts,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
    ) -> bool {
        let at_path = |mut value: arena::Handle<crate::FlowOperandFact>,
                       mut path: &[facts::PlaceSegment]| {
            while !path.is_empty() {
                let capture = flow.control.operands.get(value);
                let mut children = flow
                    .control
                    .operand_children
                    .span_or_empty(capture.constructed_children())
                    .iter()
                    .filter_map(|child| {
                        let destination =
                            flow.control.operand_child_paths.span_or_empty(child.path);
                        (child.parent == capture.expression
                            && !destination.is_empty()
                            && path.starts_with(destination))
                        .then_some((child.value, destination.len()))
                    });
                let (selected, length) = children.next()?;
                if children.next().is_some() || !flow.control.operands.is_valid(selected) {
                    return None;
                }
                value = selected;
                path = &path[length..];
            }
            Some(value)
        };
        let mut matching = self.body_transformations.iter().filter(|(_, transaction)| {
            transaction.state_symbol == state
                && self
                    .body_derivations
                    .span_or_empty(transaction.derivations)
                    .iter()
                    .any(|derived| {
                        if !matches!(derived.custody, ContentBodyCustody::Derived) {
                            return false;
                        }
                        let equation = self.body_conservations.get(derived.conservation);
                        self.body_claim_bindings
                            .span_or_empty(equation.outputs)
                            .iter()
                            .any(|output| {
                                if output.domain != domain
                                    || output.semantic_domain != semantic_domain
                                {
                                    return false;
                                }
                                let path = flow.ownership.segments.span_or_empty(output.segments);
                                let Some(selected) = at_path(transaction.completed_value, path)
                                else {
                                    return false;
                                };
                                let capture = flow.control.operands.get(selected);
                                capture.state_symbol == state
                                    && capture.statement_index == statement
                                    && capture.expression == expression
                            })
                    })
        });
        // Do not choose one of multiple evaluation routes sharing coordinates.
        matching.next().is_some() && matching.next().is_none()
    }

    /// Membership published by this exact completion, not by an ancestor or
    /// a later use of the same source place. Paths are relative to its result.
    pub fn establishes_completed_membership(
        &self,
        flow: &crate::FlowFacts,
        value: arena::Handle<crate::FlowOperandFact>,
        path: &[facts::PlaceSegment],
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
    ) -> bool {
        self.body_transformations.iter().any(|(_, transaction)| {
            transaction.completed_value == value
                && self
                    .body_derivations
                    .span_or_empty(transaction.derivations)
                    .iter()
                    .any(|derived| {
                        if !matches!(derived.custody, ContentBodyCustody::Derived) {
                            return false;
                        }
                        let equation = self.body_conservations.get(derived.conservation);
                        self.body_claim_bindings
                            .span_or_empty(equation.outputs)
                            .iter()
                            .any(|output| {
                                output.domain == domain
                                    && output.semantic_domain == semantic_domain
                                    && flow.ownership.segments.span_or_empty(output.segments)
                                        == path
                            })
                    })
        })
    }

    pub fn for_semantic_domain(&self, domain: SemanticDomainId) -> Option<&ContentProjectionPlan> {
        domain
            .is_valid()
            .then(|| {
                self.plans
                    .iter()
                    .find(|plan| plan.semantic_domain == domain)
            })
            .flatten()
    }
}
