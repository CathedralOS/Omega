//! When lowering copies a helper body into a loop, its calls still refer to
//! the original callback parameter. Source call records preserve that connection.
use crate::read::refuse_at;

use crate::source_state::SourceCallCorrespondence;
use crate::source_state::data::TypeParameter;
use semantic_vocabulary::{InvocationSite, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use terminal_psi::static_reach::StaticReachParameter;

use super::Program;
use crate::Unserved;

struct Origin {
    owner: MachineId,
    declaration: TypeParameter,
    requirements: Vec<MachineId>,
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

fn unavailable(owner: MachineId, reason: &'static str) -> crate::Refusal {
    refuse_at(owner, Unserved::Declaration(reason))
}

impl Program<'_> {
    fn static_parameter(&self, owner: MachineId, position: u32) -> Checked<TypeParameter> {
        let symbol = self
            .symbols
            .get(owner)
            .ok_or_else(|| unavailable(owner, "static callback context lost its source machine"))?;
        self.typed
            .declaration_type_parameters(*symbol)
            .get(position as usize)
            .cloned()
            .ok_or_else(|| {
                unavailable(owner, "static callback context lost its original parameter")
            })
    }

    fn same_static_parameter(
        &self,
        origin: &Origin,
        owner: MachineId,
        declaration: &TypeParameter,
    ) -> Checked<bool> {
        let first = &origin.declaration;
        if first == declaration {
            return Ok(true);
        }
        if first.symbol != declaration.symbol
            || first.name != declaration.name
            || first.bounds != declaration.bounds
        {
            return Ok(false);
        }
        let source = |machine| self.machine_symbol(machine);
        self.typed
            .copied_parameter_kinds_match(
                &first.kind,
                source(origin.owner)?,
                &declaration.kind,
                source(owner)?,
            )
            .map_err(|reason| unavailable(owner, reason))
    }

    pub(crate) fn static_reach_contexts(
        &self,
        module: &mut terminal_psi::TerminalModule,
        calls: &[SourceCallCorrespondence],
    ) -> Checked<Vec<StaticReachParameter>> {
        if self.static_reach_parameters.is_empty() {
            return Ok(Vec::new());
        }
        let mut declared = Vec::with_capacity(self.static_reach_parameters.len());
        for row in &self.static_reach_parameters {
            let declaration = self.static_parameter(row.owner, row.parameter)?;
            declared.push((declaration, row.requirement, row.owner));
        }
        declared.sort_unstable_by_key(|(declaration, requirement, _)| {
            (symbol_key(declaration.symbol), *requirement)
        });
        let mut origins: Vec<Origin> = Vec::new();
        for (declaration, requirement, owner) in declared {
            if origins
                .last()
                .is_none_or(|origin| origin.declaration.symbol != declaration.symbol)
            {
                origins.push(Origin {
                    owner,
                    declaration: declaration.clone(),
                    requirements: Vec::new(),
                });
            }
            let origin = origins.last_mut().expect("the new origin exists");
            if !self.same_static_parameter(origin, owner, &declaration)? {
                return Err(unavailable(
                    owner,
                    "static callback context changed its original declaration",
                ));
            }
            if origin.requirements.last() != Some(&requirement) {
                origin.requirements.push(requirement);
            }
        }

        // A wrapper retains its declared parameter even if only a helper calls
        // through it. This records where the callback came from; selecting its
        // body still requires the ordinary contract checks.
        let mut positions = Vec::new();
        let mut parameters = Vec::new();
        for (owner, symbol) in self.symbols.iter() {
            for (position, declaration) in self
                .typed
                .declaration_type_parameters(*symbol)
                .iter()
                .enumerate()
            {
                let Ok(origin) = origins
                    .binary_search_by_key(&symbol_key(declaration.symbol), |origin| {
                        symbol_key(origin.declaration.symbol)
                    })
                else {
                    continue;
                };
                let origin = &origins[origin];
                if !self.same_static_parameter(origin, owner, declaration)? {
                    return Err(unavailable(
                        owner,
                        "static callback context changed an inherited declaration",
                    ));
                }
                let position = u32::try_from(position).map_err(|_| {
                    unavailable(owner, "static callback context has too many parameters")
                })?;
                positions.push(((owner, symbol_key(declaration.symbol)), position));
                parameters.extend(origin.requirements.iter().map(|&requirement| {
                    StaticReachParameter {
                        owner,
                        parameter: position,
                        requirement,
                    }
                }));
            }
        }
        parameters.sort_unstable();
        parameters.dedup();
        positions.sort_unstable_by_key(|(key, _)| *key);

        let mut source_calls = calls.iter().collect::<Vec<_>>();
        source_calls.sort_unstable_by_key(|call| (call.machine, call.site));
        for pair in source_calls.windows(2) {
            if (pair[0].machine, pair[0].site) == (pair[1].machine, pair[1].site) {
                return Err(unavailable(
                    pair[1].machine,
                    "static callback context repeats a source call",
                ));
            }
        }
        let normalize = |owner, site, callee, binding| -> Checked<u32> {
            let source = source_calls
                .binary_search_by_key(&(owner, site), |call| (call.machine, call.site))
                .map(|index| source_calls[index])
                .map_err(|_| {
                    unavailable(owner, "static callback context lost its actual source call")
                })?;
            let declaration = self.static_parameter(source.source_member, binding)?;
            let original = StaticReachParameter {
                owner: source.source_member,
                parameter: binding,
                requirement: callee,
            };
            if self
                .static_reach_parameters
                .binary_search(&original)
                .is_err()
            {
                return Err(unavailable(
                    owner,
                    "static callback call differs from its original requirement",
                ));
            }
            positions
                .binary_search_by_key(&(owner, symbol_key(declaration.symbol)), |(key, _)| *key)
                .map(|index| positions[index].1)
                .map_err(|_| {
                    unavailable(
                        owner,
                        "copied callback call is outside its declared context",
                    )
                })
        };
        let mut requirements = self
            .static_reach_parameters
            .iter()
            .map(|row| row.requirement)
            .collect::<Vec<_>>();
        requirements.sort_unstable();
        requirements.dedup();
        for machine in &mut module.machines {
            for block in &mut machine.blocks {
                for operation in &mut block.operations {
                    let Some(binding) = operation.static_reach_binding else {
                        continue;
                    };
                    let Some(inputs) = operation.module_invocation() else {
                        return Err(unavailable(
                            machine.id,
                            "static callback binding is attached to a non-call operation",
                        ));
                    };
                    if requirements.binary_search(&inputs.callee).is_ok() {
                        operation.static_reach_binding = Some(normalize(
                            machine.id,
                            InvocationSite::Operation(operation.id),
                            inputs.callee,
                            binding,
                        )?);
                    }
                }
                if let terminal_psi::Terminator::InvokeNoNormal {
                    edge,
                    callee,
                    static_reach_binding: Some(binding),
                    ..
                } = &mut block.terminator
                    && requirements.binary_search(callee).is_ok()
                {
                    *binding = normalize(
                        machine.id,
                        InvocationSite::InvokeNoNormal(*edge),
                        *callee,
                        *binding,
                    )?;
                }
            }
        }
        Ok(parameters)
    }
}
