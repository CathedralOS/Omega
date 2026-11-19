use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{StaticDataShape, StaticValueDemand, TypedStage, ValueType};

impl DeclarationCatalog<'_> {
    pub(crate) fn publish_static_data_shapes(
        &self,
        demands: &[StaticValueDemand],
        stage: &mut TypedStage,
    ) {
        enum Step {
            Visit(ValueType),
            Leave,
        }
        let constant_carriers = stage
            .constants
            .iter()
            .map(|constant| &constant.initializer.carrier);
        let demand_carriers = demands.iter().filter_map(|demand| match demand {
            StaticValueDemand::Closed(request) => Some(&request.carrier),
            StaticValueDemand::Specialized { .. } => None,
        });
        let evaluated_carriers = stage
            .evaluated_indices
            .iter()
            .map(|(request, _)| &request.carrier);
        if stage.constants.is_empty() && demands.is_empty() && stage.evaluated_indices.is_empty() {
            return;
        }
        let mut steps: Vec<_> = constant_carriers
            .chain(demand_carriers)
            .chain(evaluated_carriers)
            // Local machine arguments and temporaries may use a different
            // aggregate than the value returned to the demanding expression.
            .chain(stage.expressions.iter().map(|expression| &expression.ty))
            .filter(|ty| matches!(ty, ValueType::Array(..) | ValueType::Nominal(_) | ValueType::Applied(..)))
            .cloned()
            .map(Step::Visit)
            .collect();
        let mut active: Vec<ValueType> = Vec::new();
        while let Some(step) = steps.pop() {
            let Step::Visit(carrier) = step else {
                active.pop();
                continue;
            };
            if let ValueType::Array(element, _) = carrier {
                steps.push(Step::Visit(*element));
                continue;
            }
            let Some((owner, args)) = carrier.nominal_parts() else { continue };
            if stage.static_data_shapes.contains_key(&carrier) {
                continue;
            }
            // An application can contain a smaller instance of the same
            // declaration through its type arguments. A declaration that
            // expands itself through a field can otherwise generate an
            // unbounded series of new applications.
            if active.iter().rev().any(|prior| {
                let (prior_owner, prior_args) = prior.nominal_parts().expect("active data application");
                prior_owner == owner
                    && !prior_args
                        .iter()
                        .flat_map(ValueType::components)
                        .any(|part| part == &carrier)
            }) {
                continue;
            }
            let Some(record) = self.fields.get(owner) else { continue };
            let specialize = |field: &crate::TypedField| self.types.apply_type_arguments(&field.ty, owner, args);
            let fields = record.ordered.iter().map(specialize).collect::<Vec<_>>();
            let cases: Vec<(_, Vec<ValueType>)> = self
                .cases
                .get(owner)
                .into_iter()
                .flatten()
                .map(|&case| {
                    let payload = self.fields[case].ordered.iter().map(specialize).collect();
                    (case.clone(), payload)
                })
                .collect::<Vec<_>>();
            steps.push(Step::Leave);
            for field in fields
                .iter()
                .chain(cases.iter().flat_map(|(_, payload)| payload))
            {
                steps.push(Step::Visit(field.clone()));
            }
            active.push(carrier.clone());
            stage
                .static_data_shapes
                .insert(carrier, StaticDataShape { fields, cases });
        }
    }
}
