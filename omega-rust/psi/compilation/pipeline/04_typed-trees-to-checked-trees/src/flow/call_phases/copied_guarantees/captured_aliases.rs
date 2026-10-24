//! A captured reference has its own value namespace and still denotes live
//! storage. Physical writes must invalidate observations in that namespace;
//! neither this association nor its inverse projection grants a storage write.

use super::ArgumentSources;
use crate::flow::mutation::CallMutationWrite;
use validation::CanonicalPlace;

impl ArgumentSources<'_, '_> {
    pub(in crate::flow) fn mutation_observation_aliases(
        &self,
        write: &CallMutationWrite,
    ) -> Vec<CanonicalPlace> {
        let physical = write.region.anchor();
        let mut aliases = Vec::new();
        for capture in &self.captures {
            if !crate::flow::operand_binds_live_view(self.program, capture.type_reference) {
                continue;
            }
            let Some(alias) = validation::canonical_place_from_expression_in_state(
                self.program,
                self.state,
                self.statement,
                capture.expression,
            ) else {
                continue;
            };
            let Some(binding) = validation::ownership_place_from_expression_in_state(
                self.program,
                self.state,
                self.statement,
                capture.expression,
            ) else {
                continue;
            };
            let Some(later) = &capture.later_writes else {
                continue;
            };
            if later.iter().any(|write| {
                write.root == binding.root
                    && crate::flow::operand_contexts::binding_overwrite(
                        self.program,
                        &binding,
                        &write.segments,
                    )
            }) {
                continue;
            }
            for referent in &capture.referents {
                if referent.root != physical.root
                    || !crate::flow::canonical_place_segments_may_overlap(
                        self.program,
                        &referent.segments,
                        &physical.segments,
                    )
                {
                    continue;
                }
                let mut projected = alias.clone();
                if let Some(suffix) = crate::flow::mutation::captured_alias_projection(
                    &referent.segments,
                    &physical.segments,
                ) {
                    projected.segments.extend(suffix);
                }
                if !aliases.contains(&projected) {
                    aliases.push(projected);
                }
            }
        }
        // A callee-relative write also retains the path through an input's
        // nested references. Demand only its first reference boundary, rather
        // than enumerating a potentially recursive referent type graph.
        if let Some(source) = &write.source
            && let Some(root) = source.anchor().root.as_symbol()
            && let Some(position) =
                typed_trees::signature::state_parameter_position(self.parameters, root)
            && let Some(capture) = self
                .captures
                .iter()
                .find(|capture| capture.position == position && capture.destination.is_empty())
        {
            let path = &source.anchor().segments;
            let mut reference = self.parameters[position].type_reference;
            let mut boundary = validation::reference_shell(self.program, reference).map(|_| 0usize);
            if boundary.is_none() {
                for (ordinal, segment) in path.iter().enumerate() {
                    let Some(projected) = validation::project_type_reference_from_segments(
                        self.program,
                        reference,
                        std::slice::from_ref(segment),
                    ) else {
                        break;
                    };
                    reference = projected;
                    if validation::reference_shell(self.program, reference).is_some() {
                        boundary = Some(ordinal + 1);
                        break;
                    }
                }
            }
            if let Some(boundary) = boundary
                && let Some(mut alias) = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement,
                    capture.expression,
                )
                && let Some(mut binding) = validation::ownership_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement,
                    capture.expression,
                )
                && let Some(later) = &capture.later_writes
            {
                binding.segments.extend_from_slice(&path[..boundary]);
                if !later.iter().any(|write| {
                    write.root == binding.root
                        && crate::flow::operand_contexts::binding_overwrite(
                            self.program,
                            &binding,
                            &write.segments,
                        )
                }) {
                    let projected = path
                        .iter()
                        .map(|segment| {
                            segment.erase_runtime_selector(|expression| {
                                self.program
                                    .expression_table
                                    .range_node(expression)
                                    .is_some()
                            })
                        })
                        .collect::<Vec<_>>();
                    if let Some(projected) =
                        crate::flow::operand_values::project_captured_view_place(
                            alias.clone(),
                            &projected,
                        )
                    {
                        alias = projected;
                    }
                    if !aliases.contains(&alias) {
                        aliases.push(alias);
                    }
                }
            }
        }
        aliases
    }
}
