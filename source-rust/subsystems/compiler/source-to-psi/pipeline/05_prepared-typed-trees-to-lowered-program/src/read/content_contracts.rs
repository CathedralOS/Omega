//! Read an authored equality over content projections before the terminal domain catalogue is allocated.
use super::Program;
use super::contracts::{ClauseScope, ResultTerm};
use crate::body::{
    ContentEquation, ProjectedContent, Qualification, StorageShape, ValueType,
};
use crate::{SourceOrigin, Unserved, refuse};
use crate::source_state::domain::ProofFact;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use crate::source_state::types::TypeReferenceNode;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Step, ContentAlgebra, ContentPlaceSegment,
    ContentPlaceVersion, MachineId, StructuralTypeId,
};
use terminal_check_tables::{Checked, Construct};

impl Program<'_> {
    pub(super) fn content_equation(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        fact: &ProofFact,
    ) -> Checked<Option<ContentEquation>> {
        let typed = self.typed;
        let ProofFact::Expression(expression) = fact else {
            return Ok(None);
        };
        let ExpressionNode::Call(call) = typed.expression_table.expression(*expression) else {
            return Ok(None);
        };
        if typed
            .symbols
            .fixed_core_declaration_for_symbol(call.target_symbol)
            != Some(symbols::FixedCoreDeclaration::Identity)
        {
            return Ok(None);
        }
        let [argument] = call.machine_arguments.as_ref() else {
            return Ok(None);
        };
        let reference = argument.type_reference;
        if !typed.contains_type_reference(reference) {
            return Ok(None);
        }
        let TypeReferenceNode::Generic { base_symbol, .. } =
            typed.type_reference_table.type_reference(reference)
        else {
            return Ok(None);
        };
        if self.content_algebra_kind(*base_symbol).is_none() {
            return Ok(None);
        }
        if !typed
            .trait_machine_signature(scope.owner)
            .is_some_and(|(owner, _)| owner.is_boundary)
        {
            return Err(unavailable(
                id,
                "checked content conservation requires a body judgment over the actual normal return",
            ));
        }
        let (_, [left, right], _) = self.identity_inputs(id, scope, *expression, call)?;
        let expected = crate::source_state::reflection::construction::schema_type_identity(
            typed,
            reference,
            scope.owner,
        )
        .map_err(|_| unavailable(id, "content equation lost the written algebra application"))?;
        let (left, algebra) = self.content_term(id, scope, &expected, left)?;
        let (right, other) = self.content_term(id, scope, &expected, right)?;
        if algebra != other {
            return Err(unavailable(
                id,
                "content equation mixes distinct coordinate spaces or units",
            ));
        }
        Ok(Some(ContentEquation {
            algebra,
            left,
            right,
        }))
    }

    fn content_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expected: &str,
        expression: ExpressionHandle,
    ) -> Checked<(ProjectedContent, ContentAlgebra)> {
        let typed = self.typed;
        let table = &typed.expression_table;
        let ExpressionNode::Call(call) = table.expression(expression) else {
            return Err(unavailable(
                id,
                "content equation requires an owner projection or separation",
            ));
        };
        self.content_call(id, call)?;
        let arguments = table.expression_handles(call.arguments);
        if self.builtin_call(call, symbols::BuiltinFunction::ContentSeparate) {
            if arguments.len() < 2 {
                return Err(unavailable(
                    id,
                    "content separation needs at least two operands",
                ));
            }
            let mut terms = Vec::with_capacity(arguments.len());
            let mut common = None;
            for &argument in arguments {
                let (term, algebra) = self.content_term(id, scope, expected, argument)?;
                if common.as_ref().is_some_and(|previous| previous != &algebra) {
                    return Err(unavailable(
                        id,
                        "separation operands must share their content algebra application",
                    ));
                }
                common = Some(algebra);
                terms.push(term);
            }
            return Ok((
                ProjectedContent::Separate(terms),
                common.expect("at least two operands"),
            ));
        }
        let [subject] = arguments else {
            return Err(unavailable(id, "an owner projection requires one subject"));
        };
        let (domain, reference, algebra) =
            self.contract_projection(id, call.target_symbol, expected)?;
        let (version, borrow) = match table.expression(*subject) {
            ExpressionNode::Call(old)
                if self.builtin_call(old, symbols::BuiltinFunction::ContentOld) =>
            {
                self.content_call(id, old)?;
                let [input] = table.expression_handles(old.arguments) else {
                    return Err(unavailable(id, "old requires exactly one invocation input"));
                };
                (ContentPlaceVersion::Entry, *input)
            }
            _ => (ContentPlaceVersion::Current, *subject),
        };
        let ExpressionNode::Borrow(borrow) = table.expression(borrow) else {
            return Err(unavailable(
                id,
                "a content projection requires a shared structural borrow",
            ));
        };
        if borrow.access != language_core::ReferenceAccess::Shared {
            return Err(unavailable(
                id,
                "a content projection requires a shared structural borrow",
            ));
        }
        let place_scope = ClauseScope {
            bound: &[],
            formal_fields: true,
            ..*scope
        };
        let Some((root, path, actual @ ValueType::Record(carrier, ..))) =
            self.projected(id, &place_scope, borrow.target)?
        else {
            return Err(unavailable(
                id,
                "content observation needs an exact structural parameter or result place",
            ));
        };
        if version == ContentPlaceVersion::Entry
            && matches!(scope.result, ResultTerm::Structural(result, _) if result == root)
        {
            return Err(unavailable(id, "result has no entry content"));
        }
        let bare = actual
            .qualified(Qualification::NONE)
            .ok_or_else(|| unavailable(id, "content observation lost its carrier"))?;
        self.declared_domain(
            id,
            domain,
            bare,
            reference,
            SourceOrigin::of(table.source_span(expression)),
        )?;
        let base = match scope.result {
            ResultTerm::Structural(result, ValueType::Record(base, ..)) if result == root => base,
            _ => {
                let mut base = None;
                for (position, slot) in scope.slots.iter().enumerate() {
                    if let ValueType::Record(carrier, ..) = slot.value_type
                        && super::contracts::formal_place(id, scope.slots, position)? == root
                    {
                        base = Some(carrier);
                        break;
                    }
                }
                base.ok_or_else(|| {
                    unavailable(id, "content subject lost its structural input root")
                })?
            }
        };
        let (_, segments) = self
            .content_path(id, base, &path)?
            .filter(|(tip, _)| *tip == carrier)
            .ok_or_else(|| {
                unavailable(
                    id,
                    "content subject lost its exact named field, case, or fixed index",
                )
            })?;
        let expression = self
            .declared_domains
            .iter()
            .find(|declared| declared.instance == domain && declared.carrier == bare)
            .and_then(|declared| declared.content_projection.as_ref())
            .filter(|projection| projection.algebra == algebra)
            .ok_or_else(|| unavailable(id, "content subject has no retained checked projection"))?
            .expression
            .clone();
        let subject = semantic_vocabulary::ContentStructuralPlace {
            root,
            version,
            segments,
        };
        Ok((
            ProjectedContent::Projection {
                domain,
                carrier,
                expression,
                subject,
            },
            algebra,
        ))
    }

    /// Resolve each globally named selector through its actual containing record.
    /// Terminal report paths use names, while stored membership facts use selector IDs.
    fn content_path(
        &self,
        id: MachineId,
        mut carrier: StructuralTypeId,
        steps: &[Step],
    ) -> Checked<Option<(StructuralTypeId, Vec<ContentPlaceSegment>)>> {
        let mut names = Vec::with_capacity(steps.len());
        let mut selected_case = None;
        for step in steps {
            let record = self.data_type(id, carrier)?;
            match step {
                Step::Case(id) if selected_case.is_none() => {
                    let Some(case) = record.cases().iter().find(|case| case.id == *id) else {
                        return Ok(None);
                    };
                    names.push(ContentPlaceSegment::Case(case.identity.clone()));
                    selected_case = Some(case);
                }
                Step::Field(id) => {
                    let fields = selected_case
                        .take()
                        .map_or_else(|| record.fields(), |case| case.fields.as_slice());
                    let Some(field) = fields.iter().find(|field| field.id == *id) else {
                        return Ok(None);
                    };
                    let ValueType::Record(child, ..) = field.value_type else {
                        return Ok(None);
                    };
                    names.push(ContentPlaceSegment::Field(field.identity.clone()));
                    carrier = child;
                }
                Step::FixedIndex(index) if selected_case.is_none() => {
                    let StorageShape::FixedArray {
                        element,
                        length: terminal_psi::ArrayExtent::Literal(length),
                        ..
                    } = record.shape
                    else {
                        return Ok(None);
                    };
                    if *index >= length {
                        return Ok(None);
                    }
                    names.push(ContentPlaceSegment::FixedIndex(*index));
                    carrier = element;
                }
                _ => return Ok(None),
            }
        }
        Ok(selected_case.is_none().then_some((carrier, names)))
    }

    fn content_call(&self, id: MachineId, call: &TableCallExpression) -> Checked<()> {
        if !call.is_ordinary_positional_call()
            || call.build_selection.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.requirement_application.is_valid()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(unavailable(
                id,
                "content term has a special invocation or extra operand",
            ));
        }
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )
    }
}

fn unavailable(id: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    refuse(id, Construct::Machine, Unserved::Contract(detail))
}
