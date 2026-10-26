//! Static type operands belong to their supplying declaration, including a
//! constant whose initializer has already been consumed. A constant adds no
//! binders; only its actual enclosing declarations supply types and regions.
//! Keep that telescope separate from the constant's own lexical identity so
//! checking cannot borrow a consumer's context or admit an undeclared lifetime.

use super::StaticTypeArgumentOwner;
use std::borrow::Cow;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{data::TypeParameter, name::Identifier};

impl<'program> StaticTypeArgumentOwner<'program> {
    pub(crate) fn from_lexical_owner(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
    ) -> Option<Self> {
        if let Some(machine) = program.machine_declaration_for_lexical_scope(owner) {
            return Some(Self::Machine(machine));
        }
        if let Some(data) = program.data_definition_by_symbol(owner) {
            return Some(Self::Data(data));
        }
        // Const symbols deliberately outlive value substitution and retain
        // source provenance even when an executable projection omits the value.
        (owner.is_valid() && program.symbols.get(owner).kind == SymbolKind::Const).then(|| {
            Self::Constant {
                symbol: owner,
                name: program.symbols.name(owner),
            }
        })
    }

    pub(super) fn parameters(
        self,
        program: &'program dyn crate::CheckingInput,
    ) -> (
        Cow<'program, [TypeParameter]>,
        Cow<'program, [Identifier]>,
        SymbolHandle,
    ) {
        match self {
            Self::Machine(machine) => {
                let (types, lifetimes) = program
                    .machine_type_reference_scope(machine, |left, right| left.name == right.name);
                (Cow::Owned(types), Cow::Owned(lifetimes), machine.symbol)
            }
            Self::Data(data) => (
                Cow::Borrowed(program.data_type_parameters(data)),
                Cow::Borrowed(data.lifetime_parameters.as_slice()),
                data.symbol,
            ),
            Self::Constant { symbol, .. } => {
                let (types, lifetimes) = constant_parameters(program, symbol);
                (Cow::Owned(types), Cow::Owned(lifetimes), symbol)
            }
        }
    }
}

fn constant_parameters(
    program: &dyn crate::CheckingInput,
    constant: SymbolHandle,
) -> (Vec<TypeParameter>, Vec<Identifier>) {
    let mut types = Vec::new();
    let mut lifetimes = Vec::new();
    let mut enclosing = program.declaration_scope_parent(constant);
    while enclosing.is_valid() {
        if let Some(machine) = program.machine_declaration_for_lexical_scope(enclosing) {
            let (outer_types, outer_lifetimes) = program
                .machine_type_reference_scope(machine, |left, right| left.name == right.name);
            append_scope(&mut types, &mut lifetimes, &outer_types, &outer_lifetimes);
            break;
        }
        if let Some(data) = program.data_definition_by_symbol(enclosing) {
            append_scope(
                &mut types,
                &mut lifetimes,
                program.data_type_parameters(data),
                &data.lifetime_parameters,
            );
        } else if let Some(conformance) = program.conformance_by_symbol(enclosing) {
            append_scope(
                &mut types,
                &mut lifetimes,
                program.conformance_type_parameters(conformance),
                &conformance.lifetime_parameters,
            );
        } else if let Some(trait_definition) = program.trait_definition_by_symbol(enclosing) {
            append_scope(
                &mut types,
                &mut lifetimes,
                program.trait_type_parameters(trait_definition),
                &trait_definition.lifetime_parameters,
            );
        } else if let Some(domain) = program.domain_definition(enclosing) {
            append_scope(
                &mut types,
                &mut lifetimes,
                program.domain_type_parameters(domain),
                &[],
            );
        }
        enclosing = program.declaration_scope_parent(enclosing);
    }
    (types, lifetimes)
}

fn append_scope(
    types: &mut Vec<TypeParameter>,
    lifetimes: &mut Vec<Identifier>,
    outer_types: &[TypeParameter],
    outer_lifetimes: &[Identifier],
) {
    for parameter in outer_types {
        if !types.iter().any(|existing| existing.name == parameter.name) {
            types.push(parameter.clone());
        }
    }
    for lifetime in outer_lifetimes {
        if !lifetimes.contains(lifetime) {
            lifetimes.push(lifetime.clone());
        }
    }
}
