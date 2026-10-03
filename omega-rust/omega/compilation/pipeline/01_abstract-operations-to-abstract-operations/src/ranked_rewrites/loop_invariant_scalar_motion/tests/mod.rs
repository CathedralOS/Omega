//! Optimizer module role: test leaf. Transitive member-parameter invariant discovery and place-observation admission.
//!
//! The families beside this root, one file per rewrite contract:
//! `computations`, `place_observations`, `byte_sequences`, `calls` and
//! `structural_establishments`; the lowering, coordinate-refresh and node
//! lookup helpers every family uses live here.

mod byte_sequences;
mod calls;
mod computations;
mod place_observations;
mod scheduled_step;
mod structural_establishments;

/// The component declines when no single preheader reaches every entry:
/// the shared-boundary refusal the evidence matrix exercises.
pub(crate) use computations::MULTI_SOURCE_ENTRY_SOURCE;

use crate::VerifiedPsiOptimizationSession;
use abstract_operations::{
    AbstractOperation, AbstractStructuralCallResult, IntegerArithmeticOperator,
    IntegerOverflowPolicy,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_unit::{
    PsiOptimizationUnit, PsiProvenance, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::ValueId;
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

/// Two-state unranked cycle: the entry state forwards `scale` to `step`, which
/// carries it back unchanged, and every traversal that leaves the component
/// passes through `step` — the loop's only exit is `step`'s own `finish` arm.
/// `s` is a parameter of a non-entry member block — provably invariant only
/// once member parameters resolve transitively through the component's
/// internal edges — so `s + s` is the relocated computation this family adds
/// over the entry-target-only discovery, and `step` dominating every exit
/// keeps the relocation inside the non-speculative gate.
pub(crate) const TRANSITIVE_MEMBER_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// The `s + s` computation inside a member block, its block, and the member
/// parameter it reads twice.
pub(super) fn member_addition<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
    ValueId,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::IntegerArithmetic {
                arithmetic,
                left,
                right,
                ..
            } = &node.operation
                && arithmetic.is(
                    IntegerOverflowPolicy::Wrapping,
                    IntegerArithmeticOperator::Add,
                )
                && left == right
                && block
                    .parameters
                    .iter()
                    .any(|parameter| parameter.value == *left)
            {
                return (block, node, *left);
            }
        }
    }
    panic!("the `s + s` computation lives in a member block")
}

pub(super) fn take_operation(
    unit: &mut PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
) -> optimization_unit::OptimizationNode {
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if let Some(index) = block.nodes.iter().position(|node| {
                node.provenance.first() == Some(&PsiProvenance::Operation(operation))
            }) {
                return block.nodes.remove(index);
            }
        }
    }
    panic!("operation {operation:?} exists")
}

pub(super) fn find_operation_mut(
    unit: &mut PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
) -> &mut optimization_unit::OptimizationNode {
    unit.functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| node.provenance.first() == Some(&PsiProvenance::Operation(operation)))
        .expect("operation exists")
}

pub(super) fn refresh_coordinates_and_effects(unit: &mut PsiOptimizationUnit) {
    for function in &mut unit.functions {
        let mut effect = 0u64;
        for block in &mut function.blocks {
            for (node_index, node) in block.nodes.iter_mut().enumerate() {
                let node_index = u32::try_from(node_index).expect("test fixture fits u32");
                for definition in &mut node.definitions {
                    definition.site = optimization_unit::ValueDefinitionSite::Node {
                        block: block.id,
                        node: node_index,
                    };
                }
                for value_use in &mut node.uses {
                    value_use.block = block.id;
                    value_use.node = node_index;
                }
                node.effect = optimization_unit::EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
            }
        }
        let operation_order = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .enumerate()
            .filter_map(|(position, node)| match node.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => Some((*operation, position)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        function.facts.sort_by_key(|fact| {
            let support = match fact {
                optimization_unit::OptimizationFact::OperationObligationReference {
                    support,
                    ..
                }
                | optimization_unit::OptimizationFact::BooleanConstant { support, .. }
                | optimization_unit::OptimizationFact::IntegerConstant { support, .. } => support,
            };
            operation_order.get(support).copied()
        });
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

/// The `ByteSequenceLength` observations inside a component's member blocks,
/// as `(member block, node)` pairs in member order.
pub(super) fn member_length_reads<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(node.operation, AbstractOperation::ByteSequenceLength { .. })
                })
                .map(move |node| (block, node))
        })
        .collect()
}

/// The operation identity of a source-owned node.
pub(super) fn operation_of(
    node: &optimization_unit::OptimizationNode,
) -> semantic_vocabulary::OperationId {
    match node.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the node carries its operation identity"),
    }
}

pub(super) fn lowered_session(source: &str, label: &str) -> VerifiedPsiOptimizationSession {
    lowered_session_entry(source, label, "Root::scan")
}

pub(super) fn lowered_session_entry(
    source: &str,
    label: &str,
    entry: &str,
) -> VerifiedPsiOptimizationSession {
    lowered_session_entry_with_module_edit(source, label, entry, |_| {})
}

pub(crate) fn lowered_unit(source: &str, label: &str) -> VerifiedPsiOptimizationUnit {
    lowered_unit_entry_with_module_edit(source, label, "Root::scan", |_| {})
}

/// Psi snapshots each copyable owned operand with a whole-root
/// `StructuralLeafCopy` at its evaluation point (1ed429ce993 for call
/// operands, 0c97e66393f for local copies), and this pass relocates no copy.
/// Respell the one owned whole call argument or record-field root each
/// snapshot feeds as naming the copied root directly, and drop the copy: the
/// Terminal shape those operands lowered to before, which Terminal still
/// admits. The verified artifact admission after the edit replays every
/// certificate and rejects any respelling that changes custody.
pub(super) fn pass_snapshot_roots_directly(module: &mut terminal_psi::TerminalModule) {
    use terminal_psi::{OperationKind, RecordFieldValue, StructuralAccess};
    for machine in &mut module.machines {
        let snapshots = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(
                |operation| match (&operation.kind, operation.result.structural()) {
                    (OperationKind::StructuralLeafCopy { source, path }, Some(result))
                        if path.is_empty() =>
                    {
                        Some((operation.id, result.place, *source))
                    }
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        // A local copy's snapshot can itself be copied again as a record
        // field operand; respell the later snapshot first so its use then
        // names the earlier one.
        for (copy, snapshot, root) in snapshots.into_iter().rev() {
            let mut uses = 0;
            for operation in machine
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
            {
                let arguments: Vec<&mut terminal_psi::StructuralArgument> =
                    match &mut operation.kind {
                        OperationKind::CallStructuralScalar {
                            structural_arguments,
                            ..
                        } => structural_arguments
                            .iter_mut()
                            .filter(|argument| argument.access == StructuralAccess::Owned)
                            .collect(),
                        OperationKind::EstablishRecord { fields } => fields
                            .iter_mut()
                            .filter_map(|field| match &mut field.value {
                                RecordFieldValue::Structural(argument) => Some(argument),
                                RecordFieldValue::Scalar { .. } => None,
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                for argument in arguments {
                    if argument.place == snapshot && argument.path.is_empty() {
                        argument.place = root;
                        uses += 1;
                    }
                }
            }
            assert_eq!(uses, 1, "each snapshot feeds exactly one respelled use");
            for block in &mut machine.blocks {
                block.operations.retain(|operation| operation.id != copy);
            }
            machine
                .structural_places
                .retain(|place| place.id != snapshot);
        }
    }
}

pub(super) fn lowered_session_entry_with_module_edit(
    source: &str,
    label: &str,
    entry: &str,
    edit: impl FnOnce(&mut terminal_psi::TerminalModule),
) -> VerifiedPsiOptimizationSession {
    VerifiedPsiOptimizationSession::new(lowered_unit_entry_with_module_edit(
        source, label, entry, edit,
    ))
    .unwrap_or_else(|error| panic!("verified {label} session: {error:?}"))
}

fn lowered_unit_entry_with_module_edit(
    source: &str,
    label: &str,
    entry: &str,
    edit: impl FnOnce(&mut terminal_psi::TerminalModule),
) -> VerifiedPsiOptimizationUnit {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let mut lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
    .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    edit(&mut lowered.semantic_module);
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("encode {label} semantics: {error:?}")),
            proof_bytes: &terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap_or_else(|error| panic!("encode {label} proof: {error:?}")),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"))
}

/// The `Root::bump`/`Root::spin` scalar call inside a member block and its
/// block — the caller-side counterpart of [`member_addition`].
pub(super) fn member_call<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::Call { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the scalar call lives in a member block")
}

/// A two-state cycle whose member materializes an immutable byte literal for
/// the `sink` call's argument: the `EstablishByteSequenceLiteral` declares a
/// fresh borrowed-view root over constant bytes every traversal — no scalar
/// uses, no observed root, no custody events — so the whole operation
/// relocates into the preheader byte-exact while the consuming Unit-result
/// `CallStructural` stays inside the loop still spelling the same place
/// identity.
pub(super) const INVARIANT_LITERAL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8]) {}
    machine Root::scan(remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(pending: u32 [0..=5]) {
            sink("lit");
            transition pending > 0 {
                true -> scan(pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// Same literal establishment inside `step`, but the entry state's `done` arm
/// can leave the component before `step` ever runs: relocating the literal
/// would perform its establishment work on traversals the source never
/// charged, so the non-speculative gate keeps it inside.
pub(super) const BYPASSED_LITERAL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8]) {}
    machine Root::scan(remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(remaining - 1)
            _ -> done()
        }
        state step(pending: u32 [0..=5]) {
            sink("lit");
            transition pending > 0 {
                true -> scan(pending - 1)
                _ -> finish()
            }
        }
        state done() {}
        state finish() {}
    }
"#;

/// The `EstablishByteSequenceLiteral` inside a member block and its block —
/// the byte-literal counterpart of [`member_call`].
pub(super) fn member_literal<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishByteSequenceLiteral { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the byte literal establishment lives in a member block")
}

/// The `EstablishPrimitiveLocal` node inside a member block and its block —
/// the primitive-local counterpart of [`member_call`].
pub(super) fn member_primitive_local<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishPrimitiveLocal { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the primitive-local establishment lives in a member block")
}

/// The scalar-result `CallStructural` node inside a member block and its block
/// — the shared-borrow counterpart of [`member_call`]. With more than one
/// structural call in the roster, [`member_structural_scalar_calls`] lists them
/// all.
pub(super) fn member_structural_scalar_call<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    let calls = member_structural_scalar_calls(function, component);
    let [(block, node)] = calls.as_slice() else {
        panic!("exactly one structural scalar call lives in a member block")
    };
    (block, node)
}

pub(super) fn member_structural_scalar_calls<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut calls = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Scalar(_),
                ..
            } = &node.operation
            {
                calls.push((block, node));
            }
        }
    }
    calls
}

/// Every `EstablishRecord` node inside `component`'s member blocks — the
/// record counterpart of [`member_primitive_local`].
pub(super) fn member_record_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut records = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishRecord { .. } = &node.operation {
                records.push((block, node));
            }
        }
    }
    records
}

/// A ranked scalar-machine cycle whose one member block establishes an
/// unrestricted scalar array from invariant elements and hands the fresh
/// root to `first` as an `Owned` argument: the `scale` member parameter
/// resolves transitively to the machine's `scale` anchor, so the
/// establishment relocates rebinding every element to that representative
/// while its declared place stays byte-exact. The owned-argument call is a
/// copy of the member-produced root — custody-preserving under the
/// whole-component bound — and relocates behind its producer in the same
/// run, still spelling the persistent place byte-exact.
pub(super) const MEMBER_SCALAR_ARRAY_SOURCE: &str = r#"
    machine first(row: [u64; 2]) -> u64 { 0 }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first([scale, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

/// Every `EstablishScalarArray` node inside `component`'s member blocks —
/// the array counterpart of [`member_record_establishments`].
pub(super) fn member_scalar_array_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut arrays = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishScalarArray { .. } = &node.operation {
                arrays.push((block, node));
            }
        }
    }
    arrays
}

/// Every `EstablishCase` node inside `component`'s member blocks —
/// the sum counterpart of [`member_scalar_array_establishments`].
pub(super) fn member_scalar_case_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut cases = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishCase { .. } = &node.operation {
                cases.push((block, node));
            }
        }
    }
    cases
}
