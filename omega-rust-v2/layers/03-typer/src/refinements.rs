use super::*;
pub(super) fn read(
    unit: &Unit<'_>,
    types: &Types<'_>,
    schema: &DeclarationId,
    n: usize,
) -> Result<TraitRefinement, TypeError> {
    let f = &unit.file;
    let mut refinement = TraitRefinement {
        site: Site::at(f, n),
        schema: schema.clone(),
        prohibits_block: false,
        prohibits_suspend: false,
        reach: effects::reach(unit, types, n)?,
        crash_ceiling: effects::crashes(unit, n)?,
        terminates: false,
    };
    for &part in &f.nodes[n].children {
        match f.nodes[part].kind {
            NodeKind::Blocks => refinement.prohibits_block |= f.nodes[part].words().any(|w| w.text == "false"),
            NodeKind::Suspends => refinement.prohibits_suspend |= f.nodes[part].words().any(|w| w.text == "false"),
            NodeKind::Terminates if f.nodes[part].children.is_empty() => refinement.terminates = true,
            NodeKind::Reaches | NodeKind::Crashes => {}
            _ => return Err(TypeError::unsupported(f, part)),
        }
    }
    Ok(refinement)
}
impl Catalog<'_> {
    pub(super) fn evidence_refinements(
        &self,
        evidence: Option<&ValueType>,
    ) -> Vec<(&TraitRefinement, RequirementSlot)> {
        let Some(ValueType::Evidence(slot)) = evidence.and_then(|e| self.types.parameter_carrier(e)) else {
            return Vec::new();
        };
        self.trait_lineage(slot)
            .into_iter()
            .flat_map(|s| {
                self.refinements
                    .get(&s.declaration)
                    .into_iter()
                    .flatten()
                    .map(move |r| (r, s.clone()))
            })
            .collect()
    }
    pub(super) fn admits_conformance(
        &self,
        actual: &RequirementSlot,
        expected: &RequirementSlot,
    ) -> bool {
        let target = self
            .trait_lineage(expected)
            .into_iter()
            .find(|s| !self.refinements.contains_key(&s.declaration));
        target.is_some_and(|t| self.trait_lineage(actual).contains(&t))
    }
}
pub(super) fn finish(
    catalog: &Catalog<'_>,
    out: &mut TypedStage,
) -> Result<(), TypeError> {
    let mut all: Vec<_> = catalog.refinements.values().flatten().cloned().collect();
    all.sort_by_cached_key(|r| (r.site.package.clone(), r.site.file.clone(), r.site.span.get_start_offset()));
    out.refinements = all;
    for obligation in &out.obligations {
        let ObligationKind::RefineEvidence { binder, implementation } = &obligation.kind else {
            continue;
        };
        for owner in catalog.trait_lineage(binder) {
            for r in catalog
                .refinements
                .get(&owner.declaration)
                .into_iter()
                .flatten()
            {
                let reach = r.reach.as_ref().map(|v| {
                    v.iter()
                        .map(|s| ServiceApplication {
                            service: s.service.clone(),
                            arguments: s
                                .arguments
                                .iter()
                                .map(|a| a.substitute(&owner.declaration, &owner.trait_arguments))
                                .collect(),
                        })
                        .collect::<Vec<_>>()
                });
                for row in out
                    .conformance_rows
                    .iter()
                    .filter(|row| &row.conformance == implementation)
                {
                    let effects = out
                        .machine_effects
                        .iter()
                        .find(|e| e.machine == row.implementation)
                        .unwrap();
                    if r.prohibits_block && effects.pause.block
                        || r.prohibits_suspend && effects.pause.suspend
                        || reach
                            .as_ref()
                            .is_some_and(|allowed| effects.reach.iter().any(|s| !allowed.contains(s)))
                    {
                        return Err(TypeError {
                            package: obligation.site.package.clone(),
                            file: obligation.site.file.clone(),
                            span: obligation.site.span,
                            message: format!("{} exceeds the refined contract of {}", row.implementation.path, r.schema.path),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}
