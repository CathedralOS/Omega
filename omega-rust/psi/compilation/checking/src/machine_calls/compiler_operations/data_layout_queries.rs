//! Formation of an exact data-query subject is independent of evaluation.
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::DataLayoutQueryRequest;
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::references::TypedReferences;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub fn validate_data_layout_query_subject(
    program: &typed_trees::TypedTrees,
    classification: &ProofOnlyClassification,
    query: &DataLayoutQueryRequest,
    span: source::SourceSpan,
) -> Result<(), Diagnostic> {
    if !program
        .type_reference_table
        .contains_type_reference(query.carrier)
    {
        return Err(
            Diagnostic::error("layout query has no formed data carrier").with_source_span(span)
        );
    }
    if let Some(held) = classification.proof_only_mention(program, query.carrier) {
        return Err(Diagnostic::error(format!(
            "`{}` cannot measure a proof-only carrier: `{held}` has no runtime layout",
            query.kind.target_name(),
        ))
        .with_source_span(span));
    }
    struct Subject<'program> {
        program: &'program typed_trees::TypedTrees,
        visited: std::collections::HashSet<TypeReferenceHandle>,
        missing: Option<TypeReferenceHandle>,
    }
    impl Extend<SymbolHandle> for Subject<'_> {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for Subject<'_> {
        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            if !self.visited.insert(reference) {
                return false;
            }
            if let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = self.program.type_reference_table.type_reference(reference)
                && !arguments.is_empty()
                && self
                    .program
                    .data_definition_by_symbol(*base_symbol)
                    .is_some_and(|definition| definition.type_parameters.is_empty())
                && self
                    .program
                    .data_application_definition(reference)
                    .is_none()
            {
                self.missing.get_or_insert(reference);
                return false;
            }
            true
        }
        fn expression(&mut self, _: typed_trees::expression::ExpressionHandle) -> bool {
            false
        }
    }
    let mut subject = Subject {
        program,
        visited: Default::default(),
        missing: None,
    };
    typed_trees::references::visit_type_reference(program, &mut subject, query.carrier);
    if let Some(reference) = subject.missing {
        let TypeReferenceNode::Generic { base_name, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            unreachable!()
        };
        return Err(Diagnostic::error(format!(
            "`{base_name}<...>` has no bound data application: its schema instance has no runtime layout"
        )).with_source_span(span));
    }
    Ok(())
}
