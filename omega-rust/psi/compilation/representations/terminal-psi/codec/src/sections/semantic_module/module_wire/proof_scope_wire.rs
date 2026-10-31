//! Portable proof scope: declaration coordinates and retained call relationships.
//! Arena positions are private. Spans are encoded by content, never generation.

use super::super::machine_declaration_wire;
use super::super::wire::{Reader, Writer, decode_counted};
use super::super::{CodecError, MAX_PROPOSITION_DEPTH};
use arena::{Arena, HandleSpan};
use semantic_vocabulary::proof_applications::{
    ProofApplicationCall, ProofApplicationMachine, ProofCallGraph, ProofCallMachine,
    ProofDeclarationDependencies, ProofStaticBinding,
};
use terminal_psi::{
    ProofApplicationScope, ProofDeclarationId, ProofScopeDeclaration, TerminalIdentityIndex,
};

fn references(
    writer: &mut Writer,
    rows: &Arena<ProofDeclarationId>,
    span: HandleSpan<ProofDeclarationId>,
) -> Result<(), CodecError> {
    let entries = rows
        .span(span)
        .ok_or(CodecError::MalformedStructuralFoundation(
            "proof scope lost its reference span",
        ))?;
    writer.len("proof scope references", entries.len())?;
    for entry in entries {
        writer.u32(entry.0);
    }
    Ok(())
}

fn read_references(
    reader: &mut Reader<'_>,
    rows: &mut Arena<ProofDeclarationId>,
) -> Result<HandleSpan<ProofDeclarationId>, CodecError> {
    let entries = decode_counted(reader, |reader| Ok(ProofDeclarationId(reader.u32()?)))?;
    Ok(rows.insert_many(entries))
}

pub(super) fn encode(writer: &mut Writer, scope: &ProofApplicationScope) -> Result<(), CodecError> {
    writer.len("proof scope declarations", scope.declarations.len())?;
    for (_, declaration) in scope.declarations.iter() {
        match declaration {
            ProofScopeDeclaration::Missing => {
                return Err(CodecError::MalformedStructuralFoundation(
                    "proof scope declaration is absent",
                ));
            }
            ProofScopeDeclaration::Nominal(identity) => {
                writer.u8(1);
                writer.u32(identity.0);
            }
            ProofScopeDeclaration::Callable(declaration) => {
                writer.u8(2);
                machine_declaration_wire::encode(writer, declaration)?;
            }
            ProofScopeDeclaration::Unavailable(reason) => {
                writer.u8(3);
                writer.string("unavailable proof declaration", reason)?;
            }
            ProofScopeDeclaration::Operator(requirement) => {
                writer.u8(4);
                super::boundary_application_wire::encode_requirement(writer, requirement)?;
            }
        }
    }
    writer.len("proof scope callables", scope.callables.len())?;
    for (_, row) in scope.callables.iter() {
        let terminal_psi::ProofCallableAssociation::Callable {
            declaration,
            callable,
        } = row
        else {
            return Err(CodecError::MalformedStructuralFoundation(
                "proof scope callable association is absent",
            ));
        };
        writer.u32(declaration.0);
        super::callable_wire::encode_callable(writer, *callable);
    }
    let plan = &scope.plan;
    writer.boolean(plan.has_obligations);
    writer.boolean(plan.unscoped_recipe);
    writer.len("proof scope machines", plan.machines.len())?;
    for (_, row) in plan.machines.iter() {
        writer.u32(row.machine.0);
        references(writer, &plan.symbols, row.states)?;
        references(writer, &plan.symbols, row.binders)?;
        references(writer, &plan.symbols, row.dynamic_realizations)?;
        writer.boolean(row.product_selected);
    }
    writer.len(
        "proof scope declaration dependencies",
        plan.declaration_dependencies.len(),
    )?;
    for (_, row) in plan.declaration_dependencies.iter() {
        writer.u32(row.declaration.0);
        references(writer, &plan.symbols, row.dependencies)?;
        references(writer, &plan.symbols, row.signatures)?;
    }
    references(writer, &plan.symbols, plan.declarations)?;
    references(writer, &plan.symbols, plan.obligation_owners)?;
    writer.len(
        "proof scope candidate providers",
        plan.provider_candidates.len(),
    )?;
    for (_, (requirement, realization)) in plan.provider_candidates.iter() {
        writer.u32(requirement.0);
        writer.u32(realization.0);
    }
    let graph = &scope.calls;
    writer.len("proof scope call summaries", graph.machines.len())?;
    for (_, row) in graph.machines.iter() {
        writer.u32(row.machine.0);
        let calls =
            graph
                .calls
                .span(row.calls)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "proof scope lost its call span",
                ))?;
        writer.len("proof scope calls", calls.len())?;
        for call in calls {
            writer.u32(call.binder.0);
            writer.u32(call.callee.0);
            writer.u32(call.state.0);
            writer.u32(call.operator.0);
            bindings(writer, graph, call.arguments, 0)?;
        }
        references(writer, &graph.cleanup, row.cleanup)?;
    }
    Ok(())
}

fn bindings(
    writer: &mut Writer,
    graph: &ProofCallGraph<ProofDeclarationId>,
    span: HandleSpan<ProofStaticBinding<ProofDeclarationId>>,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::InvalidTag("ProofStaticBindingDepth", 0));
    }
    let entries = graph
        .bindings
        .span(span)
        .ok_or(CodecError::MalformedStructuralFoundation(
            "proof scope lost its binding span",
        ))?;
    writer.len("proof scope static bindings", entries.len())?;
    for entry in entries {
        writer.u32(entry.parameter.0);
        writer.u32(entry.selected.0);
        bindings(writer, graph, entry.arguments, depth + 1)?;
    }
    Ok(())
}

fn read_bindings(
    reader: &mut Reader<'_>,
    graph: &mut ProofCallGraph<ProofDeclarationId>,
    depth: usize,
) -> Result<HandleSpan<ProofStaticBinding<ProofDeclarationId>>, CodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(CodecError::InvalidTag("ProofStaticBindingDepth", 0));
    }
    let entries = decode_counted(reader, |reader| {
        Ok(ProofStaticBinding {
            parameter: ProofDeclarationId(reader.u32()?),
            selected: ProofDeclarationId(reader.u32()?),
            arguments: read_bindings(reader, graph, depth + 1)?,
        })
    })?;
    Ok(graph.bindings.insert_many(entries))
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<ProofApplicationScope, CodecError> {
    let mut result = ProofApplicationScope::default();
    for _ in 0..reader.count()? {
        let declaration = match reader.u8()? {
            1 => ProofScopeDeclaration::Nominal(TerminalIdentityIndex(reader.u32()?)),
            2 => ProofScopeDeclaration::Callable(machine_declaration_wire::decode(reader)?),
            3 => {
                ProofScopeDeclaration::Unavailable(reader.string("unavailable proof declaration")?)
            }
            4 => ProofScopeDeclaration::Operator(
                super::boundary_application_wire::decode_requirement(reader)?,
            ),
            tag => return Err(CodecError::InvalidTag("ProofScopeDeclaration", tag)),
        };
        result.declarations.insert(declaration);
    }
    for _ in 0..reader.count()? {
        result
            .callables
            .insert(terminal_psi::ProofCallableAssociation::Callable {
                declaration: ProofDeclarationId(reader.u32()?),
                callable: super::callable_wire::decode_callable(reader)?,
            });
    }
    let plan = &mut result.plan;
    plan.has_obligations = reader.boolean()?;
    plan.unscoped_recipe = reader.boolean()?;
    for _ in 0..reader.count()? {
        let machine = ProofDeclarationId(reader.u32()?);
        let states = read_references(reader, &mut plan.symbols)?;
        let binders = read_references(reader, &mut plan.symbols)?;
        let dynamic_realizations = read_references(reader, &mut plan.symbols)?;
        let product_selected = reader.boolean()?;
        plan.machines.insert(ProofApplicationMachine {
            machine,
            states,
            binders,
            dynamic_realizations,
            product_selected,
        });
    }
    for _ in 0..reader.count()? {
        let declaration = ProofDeclarationId(reader.u32()?);
        let dependencies = read_references(reader, &mut plan.symbols)?;
        let signatures = read_references(reader, &mut plan.symbols)?;
        plan.declaration_dependencies
            .insert(ProofDeclarationDependencies {
                declaration,
                dependencies,
                signatures,
            });
    }
    plan.declarations = read_references(reader, &mut plan.symbols)?;
    plan.obligation_owners = read_references(reader, &mut plan.symbols)?;
    for _ in 0..reader.count()? {
        plan.provider_candidates.insert((
            ProofDeclarationId(reader.u32()?),
            ProofDeclarationId(reader.u32()?),
        ));
    }
    let graph = &mut result.calls;
    for _ in 0..reader.count()? {
        let machine = ProofDeclarationId(reader.u32()?);
        let calls = decode_counted(reader, |reader| {
            Ok(ProofApplicationCall {
                binder: ProofDeclarationId(reader.u32()?),
                callee: ProofDeclarationId(reader.u32()?),
                state: ProofDeclarationId(reader.u32()?),
                operator: ProofDeclarationId(reader.u32()?),
                arguments: read_bindings(reader, graph, 0)?,
            })
        })?;
        let calls = graph.calls.insert_many(calls);
        let cleanup = read_references(reader, &mut graph.cleanup)?;
        graph.machines.insert(ProofCallMachine {
            machine,
            calls,
            cleanup,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> ProofApplicationScope {
        let mut scope = ProofApplicationScope::default();
        for _ in 0..8 {
            // Equal nominal text is deliberately not graph identity.
            scope
                .declarations
                .insert(ProofScopeDeclaration::Nominal(TerminalIdentityIndex(1)));
        }
        scope
            .declarations
            .insert(ProofScopeDeclaration::Unavailable(
                "symbolic declaration".into(),
            ));
        for ordinal in [1, 2, 3, 4, 7, 8] {
            scope.plan.machines.insert(ProofApplicationMachine {
                machine: ProofDeclarationId(ordinal),
                product_selected: ordinal != 8,
                ..Default::default()
            });
        }
        let dependency = scope.plan.symbols.insert_many([ProofDeclarationId(9)]);
        scope
            .plan
            .declaration_dependencies
            .insert(ProofDeclarationDependencies {
                declaration: ProofDeclarationId(4),
                dependencies: dependency,
                ..Default::default()
            });
        scope.plan.has_obligations = true;
        scope.plan.obligation_owners = scope.plan.symbols.insert_many([ProofDeclarationId(4)]);
        let nested = scope.calls.bindings.insert_many([ProofStaticBinding {
            parameter: ProofDeclarationId(6),
            selected: ProofDeclarationId(4),
            ..Default::default()
        }]);
        let bindings = scope.calls.bindings.insert_many([ProofStaticBinding {
            parameter: ProofDeclarationId(5),
            selected: ProofDeclarationId(3),
            arguments: nested,
        }]);
        for (owner, callee, binder, arguments) in [
            (1, 2, 0, bindings),
            (2, 0, 5, HandleSpan::default()),
            (3, 0, 6, HandleSpan::default()),
        ] {
            let calls = scope.calls.calls.insert_many([ProofApplicationCall {
                callee: ProofDeclarationId(callee),
                binder: ProofDeclarationId(binder),
                arguments,
                ..Default::default()
            }]);
            let cleanup = if owner == 1 {
                scope.calls.cleanup.insert_many([ProofDeclarationId(7)])
            } else {
                HandleSpan::default()
            };
            scope.calls.machines.insert(ProofCallMachine {
                machine: ProofDeclarationId(owner),
                calls,
                cleanup,
            });
        }
        scope
    }

    #[test]
    fn proof_scope_round_trip_preserves_distinct_declarations_and_nested_bindings() {
        let mut scope = scope();
        for (declaration, callable) in [
            (
                1,
                terminal_psi::TerminalCallable::Machine(
                    semantic_vocabulary::MachineId::new(41).unwrap(),
                ),
            ),
            (
                2,
                terminal_psi::TerminalCallable::Machine(
                    semantic_vocabulary::MachineId::new(42).unwrap(),
                ),
            ),
            (
                9,
                terminal_psi::TerminalCallable::Boundary(
                    semantic_vocabulary::BoundaryMachineId::new(41).unwrap(),
                ),
            ),
        ] {
            scope
                .callables
                .insert(terminal_psi::ProofCallableAssociation::Callable {
                    declaration: ProofDeclarationId(declaration),
                    callable,
                });
        }
        scope.declarations.insert(ProofScopeDeclaration::Operator(
            terminal_psi::BoundaryOperatorRequirement::new(
                terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
                "operator::compare(u64,u64)->bool".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]),
            )
            .unwrap(),
        ));
        let mut writer = Writer::default();
        encode(&mut writer, &scope).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode(&mut reader).unwrap();
        assert_eq!(decoded.declarations, scope.declarations);
        assert_eq!(decoded.callables, scope.callables);
        assert_eq!(reader.remaining(), 0);
        let before = scope.applications().membership(&[]);
        let after = decoded.applications().membership(&[]);
        assert_eq!(before, after);
        let mut machines = after.0;
        machines.sort_unstable();
        assert_eq!(machines, [1, 2, 3, 4, 7].map(ProofDeclarationId));
        assert_eq!(after.1, [ProofDeclarationId(9)]);
        assert!(!machines.contains(&ProofDeclarationId(8)));
        let mut writer = Writer::default();
        encode(&mut writer, &decoded).unwrap();
        assert_eq!(writer.finish(), bytes);
        assert!(decode(&mut Reader::new(&bytes[..bytes.len() - 1])).is_err());
    }

    #[test]
    fn proof_scope_refuses_lost_spans_instead_of_erasing_edges() {
        let mut scope = scope();
        scope.plan.obligation_owners =
            HandleSpan::from_parts(arena::Handle::from_parts(u32::MAX, 1), 1);
        assert!(matches!(
            encode(&mut Writer::default(), &scope),
            Err(CodecError::MalformedStructuralFoundation(
                "proof scope lost its reference span"
            ))
        ));
    }
}
