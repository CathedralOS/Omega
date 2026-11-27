//! A quotient declared for every type `T` (OWNER_QUESTIONS.md, "Generic
//! quotients name the applied relation and its proof"):
//!
//! ```omega
//! data SequenceLength<T> = Seq<T> % same_length<T>
//!     requires same_length<T> satisfies MereEquivalence<Seq<T>, same_length<T>>
//!     as SameLength<T>;
//! ```
//!
//! Each closed application, `SequenceLength<u8>`, is its own data
//! declaration from resolution on, whose carrier, relation and conformance
//! are applied at `u8`. Here the application selects what they name: the
//! relation `same_length` at `u8` ([`super::propositions`]), and the
//! conformance `SameLength` at `u8`, a closed conformance whose rows are the
//! machines `SameLength`'s rows are with `u8` in place of `T`. Its schema
//! arguments are `SameLength`'s, applied at `u8`, so the lowering reader can
//! reject a conformance that proves its laws for another carrier or
//! relation. A conformance that does not close at the written arguments
//! (another count, a binder other than a type, an argument that is not a
//! named type) is unserved and refused by name on the application.

use super::propositions::{self, GeneratedPropositions};
use super::{Operand, Served, Stop, copied_symbol, copy_declarations, unserved};
use crate::source_state::TypedTrees;
use crate::source_state::data::{QuotientDefinition, TypeParameterKind};
use crate::source_state::expression::StaticMachineArgument;
use crate::source_state::proposition::{
    PropositionApplication, PropositionBinderArgument, PropositionBinderArgumentKind,
};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};

/// Closes every quotient application in the program. A quotient whose
/// application cannot be closed keeps its arguments, and the lowering
/// reader refuses it by name.
pub(super) fn close(program: &mut TypedTrees, generated: &mut GeneratedPropositions) {
    // Every declaration, the applications resolution made among them.
    let handles = program
        .tables
        .data_definitions
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let definition = program.tables.data_definitions.get(handle);
        let Some(quotient) = definition.quotient.clone() else {
            continue;
        };
        if !definition.type_parameters.is_empty()
            || (quotient.relation_arguments.is_empty()
                && quotient
                    .equivalence
                    .as_ref()
                    .is_none_or(|selection| selection.conformance_arguments.is_empty()))
        {
            continue;
        }
        let symbol = definition.symbol;
        if let Ok(quotient) = closed(program, symbol, quotient, generated) {
            program.tables.data_definitions.get_mut(handle).quotient = Some(quotient);
        }
    }
}

fn closed(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    mut quotient: QuotientDefinition,
    generated: &mut GeneratedPropositions,
) -> Served<QuotientDefinition> {
    quotient.relation_symbol = relation(
        program,
        quotient.relation_symbol,
        quotient.relation_arguments,
        generated,
    )?;
    quotient.relation_arguments = HandleSpan::empty();
    let Some(selection) = &mut quotient.equivalence else {
        return Ok(quotient);
    };
    selection.relation_symbol = relation(
        program,
        selection.relation_symbol,
        selection.relation_arguments,
        generated,
    )?;
    selection.relation_arguments = HandleSpan::empty();
    if !selection.conformance_arguments.is_empty() {
        selection.conformance_symbol = conformance(
            program,
            owner,
            selection.conformance_symbol,
            selection.conformance_arguments,
            generated,
        )?;
        selection.conformance_arguments = HandleSpan::empty();
    }
    Ok(quotient)
}

/// The proposition a relation applied at `arguments` selects.
fn relation(
    program: &mut TypedTrees,
    relation: SymbolHandle,
    arguments: HandleSpan<TypeReferenceHandle>,
    generated: &mut GeneratedPropositions,
) -> Served<SymbolHandle> {
    if arguments.is_empty() {
        return Ok(relation);
    }
    let name = program
        .proposition_definition_by_symbol(relation)
        .map(|definition| definition.name.clone())
        .ok_or(unserved(
            "a generic quotient's relation is no declared proposition",
        ))?;
    let mut binder_arguments = Vec::new();
    for &argument in program
        .type_reference_table
        .type_reference_handles(arguments)
    {
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(argument)
        else {
            return Err(unserved(
                "a generic quotient's relation is applied at a type that is not a closed named type",
            ));
        };
        binder_arguments.push(PropositionBinderArgument {
            kind: PropositionBinderArgumentKind::Type,
            path: vec![name.clone()].into_boxed_slice(),
            const_literal: None,
            evidence_projection: None,
            symbol: *symbol,
        });
    }
    let application = PropositionApplication {
        proposition: relation,
        name,
        binder_arguments: binder_arguments.into_boxed_slice(),
        arguments: HandleSpan::empty(),
    };
    let (instance, formula) = propositions::select(program, &application, generated)?;
    // The formula's calls are served with the first clause applying it.
    if let Some(formula) = formula {
        generated.defer(instance, formula);
    }
    Ok(instance)
}

/// The closed conformance `declared` applied at `arguments` selects.
fn conformance(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    declared: SymbolHandle,
    arguments: HandleSpan<TypeReferenceHandle>,
    generated: &mut GeneratedPropositions,
) -> Served<SymbolHandle> {
    let template = program
        .conformance_by_symbol(declared)
        .cloned()
        .ok_or(unserved(
            "a generic quotient's equivalence is no declared conformance",
        ))?;
    let parameters = program.conformance_type_parameters(&template).to_vec();
    let actuals = program
        .type_reference_table
        .type_reference_handles(arguments)
        .to_vec();
    if parameters.len() != actuals.len() || !template.lifetime_parameters.is_empty() {
        return Err(unserved(
            "a generic quotient's conformance does not close at the arguments written for it",
        ));
    }
    let mut operands = Vec::with_capacity(parameters.len());
    let mut substitutions = Vec::with_capacity(parameters.len());
    for (parameter, &actual) in parameters.iter().zip(&actuals) {
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(actual).clone()
        else {
            return Err(unserved(
                "a generic quotient's conformance is applied at a type that is not a closed named type",
            ));
        };
        if parameter.kind != TypeParameterKind::Type {
            return Err(unserved(
                "a generic quotient's conformance has a binder other than a type",
            ));
        }
        let argument = StaticMachineArgument {
            type_reference: actual,
            path: vec![name].into_boxed_slice(),
            symbol,
            ..StaticMachineArgument::default()
        };
        operands.push(Operand::read(program, parameter, &argument, owner)?);
        substitutions.push((parameter.symbol, actual));
    }
    // Its schema arguments, at the written types; a relation among them
    // names the proposition its application selects.
    let mut schema = Vec::new();
    for &argument in program
        .type_reference_table
        .type_reference_handles(template.arguments)
        .to_vec()
        .iter()
    {
        let applied = program
            .substitute_type_parameters(argument, template.symbol, &substitutions)
            .map_err(unserved)?;
        let applied = match program.type_reference_table.type_reference(applied).clone() {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if program.symbols.symbols().nodes().is_valid(base_symbol)
                && program.symbols.get(base_symbol).kind == SymbolKind::Proposition =>
            {
                let instance = relation(program, base_symbol, arguments, generated)?;
                let name = program
                    .proposition_definition_by_symbol(instance)
                    .map(|definition| definition.name.clone())
                    .ok_or(Stop::Defect("a selected relation lost its declaration"))?;
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: instance,
                        name,
                    })
            }
            _ => applied,
        };
        schema.push(applied);
    }
    let mut closed = template.clone();
    let name = format!(
        "{}$specialized${}",
        program.symbols.display_path(template.symbol, "$"),
        program.symbols.symbols().len() + 1
    );
    closed.symbol =
        program
            .symbols
            .insert_generated_root_from(template.symbol, SymbolKind::Conformance, &name);
    closed.type_parameters = HandleSpan::empty();
    closed.arguments = program
        .type_reference_table
        .insert_type_reference_handles(schema);
    closed.value_identity = Default::default();
    for row in &mut closed.rows {
        if !row.source.owns_realization() {
            return Err(unserved(
                "a generic quotient's conformance has a row it does not realize itself",
            ));
        }
        let copied = copy_declarations(program, row.realization_machine, &operands)?;
        row.realization_state = copied_symbol(&copied.symbols, row.realization_state)?;
        row.realization_machine = copied_symbol(&copied.symbols, row.realization_machine)?;
    }
    let symbol = closed.symbol;
    program.push_conformance(closed);
    Ok(symbol)
}
