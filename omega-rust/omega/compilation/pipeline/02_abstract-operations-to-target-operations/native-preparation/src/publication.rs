//! Retain source-site order and quoted bytes before the working body is dropped.

use std::sync::Arc;

use abstract_operations::{AbstractOperation as Operation, AbstractOperationPlan};
use native_vocabulary::provenance::PsiProvenance;
use target_vocabulary::{NativeByteLiteral, NativeFunctionPublication, NativePublicationFacts};

pub fn capture_source_publication(
    plan: &AbstractOperationPlan,
) -> Result<NativePublicationFacts, String> {
    let mut functions = Vec::with_capacity(plan.functions.len());
    for function in &plan.functions {
        let count = function
            .blocks
            .iter()
            .try_fold(0usize, |count, block| {
                count.checked_add(block.operations.len())
            })
            .ok_or("publication operation count overflows")?;
        let operation_count = u32::try_from(count)
            .map_err(|_| "publication operation count exceeds its wire range")?;
        let mut sites = Vec::with_capacity(count);
        let mut byte_literals = Vec::new();
        for (ordinal, operation) in function.operations().enumerate() {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| "publication source ordinal exceeds its wire range")?;
            if let Some(identity) = operation.psi_operation() {
                sites.push((PsiProvenance::Operation(identity), ordinal));
            } else {
                let before = sites.len();
                for (edge, _) in operation.cleanup_rosters() {
                    sites.push((PsiProvenance::Edge(edge), ordinal));
                }
                if sites.len() == before {
                    let edge = match operation {
                        Operation::InvokeNoNormal { psi_edge, .. }
                        | Operation::ExternalCompletion { psi_edge, .. }
                        | Operation::EliminateEmpty { psi_edge, .. }
                        | Operation::Crash { psi_edge, .. } => *psi_edge,
                        _ => return Err("publication operation has no semantic occurrence".into()),
                    };
                    sites.push((PsiProvenance::Edge(edge), ordinal));
                }
            }
            if let Operation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                bytes,
                ..
            } = operation
            {
                byte_literals.push(NativeByteLiteral {
                    operation: *psi_operation,
                    place: place.id,
                    bytes: Arc::from(bytes.as_slice()),
                });
            }
        }
        sites.sort_unstable_by_key(|row| row.0);
        // A conditional or case dispatch may repeat one edge within that same
        // source operation. Reusing it at another ordinal remains an error.
        sites.dedup();
        byte_literals.sort_unstable_by_key(|row| row.operation);
        functions.push(NativeFunctionPublication {
            machine: function.machine,
            operation_count,
            sites,
            byte_literals,
        });
    }
    let catalogs = target_vocabulary::NativeCodeCatalogs::seal(
        plan.dynamic_dispatch.clone(),
        plan.callables.clone(),
    )
    .map_err(|error| format!("publication code catalogs cannot be encoded: {error}"))?;
    NativePublicationFacts::seal(functions, catalogs).map_err(str::to_owned)
}

pub(crate) fn verify_source_publication(
    plan: &AbstractOperationPlan,
    facts: &NativePublicationFacts,
) -> Result<(), &'static str> {
    if facts.code_catalogs().dynamic_dispatch() != &plan.dynamic_dispatch
        || facts.code_catalogs().callables() != &plan.callables
    {
        return Err("publication code catalogs differ from their source");
    }
    if plan.functions.len() != facts.functions().len() {
        return Err("publication function roster differs from its source");
    }
    for (source, current) in plan.functions.iter().zip(facts.functions()) {
        let operations = source.operations().collect::<Vec<_>>();
        if source.machine != current.machine
            || usize::try_from(current.operation_count).ok() != Some(operations.len())
        {
            return Err("publication function or operation count differs from its source");
        }
        let mut expected_site_count = 0usize;
        let mut expected_literal_count = 0usize;
        let mut edges = Vec::new();
        for (ordinal, &operation) in operations.iter().enumerate() {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| "publication source ordinal exceeds its wire range")?;
            edges.clear();
            if let Some(identity) = operation.psi_operation() {
                if current.ordinal(PsiProvenance::Operation(identity)) != Some(ordinal) {
                    return Err("publication operation site differs from its source ordinal");
                }
                expected_site_count += 1;
            } else {
                match operation {
                    abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
                    | Operation::ReturnStructural { psi_edge, .. }
                    | Operation::Jump { psi_edge, .. }
                    | Operation::InvokeNoNormal { psi_edge, .. }
                    | Operation::ExternalCompletion { psi_edge, .. }
                    | Operation::EliminateEmpty { psi_edge, .. }
                    | Operation::Crash { psi_edge, .. } => edges.push(*psi_edge),
                    Operation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        edges.extend([when_true.psi_edge, when_false.psi_edge]);
                    }
                    Operation::StructuralCase { cases, .. } => {
                        edges.extend(cases.iter().map(|case| case.psi_edge));
                    }
                    _ => return Err("source operation has no publication occurrence"),
                }
                edges.sort_unstable();
                edges.dedup();
                for edge in &edges {
                    if current.ordinal(PsiProvenance::Edge(*edge)) != Some(ordinal) {
                        return Err("publication edge site differs from its source ordinal");
                    }
                }
                expected_site_count += edges.len();
            }
            if let Operation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                bytes,
                ..
            } = operation
            {
                let row = current
                    .byte_literal(*psi_operation)
                    .ok_or("publication literal is missing from its source occurrence")?;
                if row.place != place.id || row.bytes.as_ref() != bytes.as_slice() {
                    return Err("publication literal differs from its source place or bytes");
                }
                expected_literal_count += 1;
            }
        }
        if expected_site_count != current.sites.len()
            || expected_literal_count != current.byte_literals.len()
        {
            return Err("publication adds or drops a source site or literal");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use abstract_operations::{AbstractBlock, AbstractFunction, AbstractFunctionResult};
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, ValueId};

    fn source() -> AbstractOperationPlan {
        let machine = MachineId::new(1).unwrap();
        AbstractOperationPlan {
            psi: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
            },
            entry: machine,
            structural_types: Vec::new().into(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            callables: Default::default(),
            dynamic_dispatch: Default::default(),
            identities: terminal_psi::TerminalIdentityTableBuilder::default().build(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                block_entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                blocks: vec![AbstractBlock {
                    block: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: vec![
                        Operation::BooleanConstant {
                            psi_operation: OperationId::new(1).unwrap(),
                            result: ValueId::new(1).unwrap(),
                            value: true,
                        },
                        Operation::ReturnUnit {
                            psi_edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            }],
        }
    }

    #[test]
    fn resealed_callable_catalogs_still_need_original_correspondence() {
        let mut source = source();
        source
            .callables
            .machine_envelopes
            .push(Some(terminal_psi::TerminalMachineEnvelope {
                machine: source.entry,
                envelope: terminal_psi::TerminalCallableEnvelope {
                    service_reach: Vec::new(),
                    invocations: Vec::new(),
                    suspends: false,
                    blocks: false,
                    termination: terminal_psi::TerminalCallableTermination::NotGuaranteed,
                },
            }));
        let original = capture_source_publication(&source).unwrap();
        verify_source_publication(&source, &original).unwrap();
        let mut changed_catalog = source.callables.clone();
        changed_catalog.machine_envelopes[0]
            .as_mut()
            .unwrap()
            .envelope
            .blocks = true;
        let changed = NativePublicationFacts::seal(
            original.functions().to_vec(),
            target_vocabulary::NativeCodeCatalogs::seal(
                source.dynamic_dispatch.clone(),
                changed_catalog,
            )
            .unwrap(),
        )
        .unwrap();
        assert_ne!(original.identity_bytes(), changed.identity_bytes());
        assert_eq!(
            verify_source_publication(&source, &changed),
            Err("publication code catalogs differ from their source")
        );
        let missing =
            NativePublicationFacts::seal(original.functions().to_vec(), Default::default())
                .unwrap();
        assert_eq!(
            verify_source_publication(&source, &missing),
            Err("publication code catalogs differ from their source")
        );
    }

    #[test]
    fn resealed_source_ordinals_still_need_exact_original_correspondence() {
        let source = source();
        let facts = capture_source_publication(&source).unwrap();
        verify_source_publication(&source, &facts).unwrap();
        let mut changed = facts.functions().to_vec();
        changed[0].sites[1].1 = 0;
        let changed = NativePublicationFacts::seal(changed, facts.code_catalogs().clone()).unwrap();
        assert_ne!(facts.identity_bytes(), changed.identity_bytes());
        assert_eq!(
            verify_source_publication(&source, &changed),
            Err("publication edge site differs from its source ordinal")
        );
        let mut missing = facts.functions().to_vec();
        missing[0].sites.pop();
        let missing = NativePublicationFacts::seal(missing, facts.code_catalogs().clone()).unwrap();
        assert_eq!(
            verify_source_publication(&source, &missing),
            Err("publication edge site differs from its source ordinal")
        );
        let mut extra = facts.functions().to_vec();
        extra[0].sites.insert(
            1,
            (PsiProvenance::Operation(OperationId::new(2).unwrap()), 1),
        );
        let extra = NativePublicationFacts::seal(extra, facts.code_catalogs().clone()).unwrap();
        assert_eq!(
            verify_source_publication(&source, &extra),
            Err("publication adds or drops a source site or literal")
        );
    }

    #[test]
    fn resealed_quoted_bytes_and_missing_literals_still_need_the_original_source() {
        let mut source = source();
        let mut names = terminal_psi::TerminalIdentityTableBuilder::default();
        let structural_type = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
        let view = terminal_psi::StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            quotient_carrier: None,
            id: structural_type,
            identity: names.intern("quoted bytes"),
            owns_linear_claim: false,
            copyable: true,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access: None },
            ),
            scalar_restriction: None,
            requires: Vec::new(),
        };
        source.identities = names.build();
        source.structural_types = vec![view.clone()].into();
        let operation = OperationId::new(2).unwrap();
        source.functions[0].blocks[0].operations.insert(
            1,
            Operation::EstablishByteSequenceLiteral {
                psi_operation: operation,
                place: terminal_psi::StructuralPlaceDeclaration {
                    id: semantic_vocabulary::PlaceId::new(1).unwrap(),
                    kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal: 0,
                        structural_type,
                    },
                },
                structural_type: view,
                bytes: b"quoted\0octets".to_vec(),
                qualifications: Vec::new(),
            },
        );
        let facts = capture_source_publication(&source).unwrap();
        verify_source_publication(&source, &facts).unwrap();
        assert_eq!(
            facts.functions()[0]
                .byte_literal(operation)
                .unwrap()
                .bytes
                .as_ref(),
            b"quoted\0octets"
        );
        let mut changed = facts.functions().to_vec();
        changed[0].byte_literals[0].bytes = Arc::from(b"other".as_slice());
        let changed = NativePublicationFacts::seal(changed, facts.code_catalogs().clone()).unwrap();
        assert_eq!(
            verify_source_publication(&source, &changed),
            Err("publication literal differs from its source place or bytes")
        );
        let mut missing = facts.functions().to_vec();
        missing[0].byte_literals.clear();
        let missing = NativePublicationFacts::seal(missing, facts.code_catalogs().clone()).unwrap();
        assert_eq!(
            verify_source_publication(&source, &missing),
            Err("publication literal is missing from its source occurrence")
        );
    }
}
