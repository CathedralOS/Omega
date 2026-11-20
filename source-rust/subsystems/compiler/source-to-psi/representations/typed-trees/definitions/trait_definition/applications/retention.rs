use super::{AppliedDeclarationRegionScope, AppliedTraitScope};
use crate::source_state::CallableSubstitution;
use crate::source_state::TypedTrees;
use crate::source_state::name::Identifier;
use crate::source_state::static_applications::StaticMachineLifetimeBinding;
use crate::source_state::type_identity;
use crate::source_state::types::TypeReferenceHandle;
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitApplicationEnvironment {
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    scope_starts: Vec<usize>,
    region_scopes: Vec<AppliedDeclarationRegionScope>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, record_storage::Record)]
pub struct RetainedTraitScope {
    declaration: SymbolHandle,
    argument_owner: SymbolHandle,
    frames: HandleSpan<RetainedTraitApplicationFrame>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub(crate) struct RetainedTraitApplicationFrame {
    declaration: SymbolHandle,
    arguments: HandleSpan<CallableSubstitution>,
    lifetimes: HandleSpan<StaticMachineLifetimeBinding>,
}

impl AppliedTraitScope<'_> {
    pub fn environment(&self) -> TraitApplicationEnvironment {
        TraitApplicationEnvironment {
            declaration: self.declaration.symbol,
            argument_owner: self.argument_owner,
            substitutions: self.substitutions.clone(),
            scope_starts: self.substitution_scope_starts.clone(),
            region_scopes: self.region_scopes.clone(),
        }
    }
}

impl TraitApplicationEnvironment {
    pub(crate) fn lifetime_identity(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        preceding_frames: usize,
        name: &str,
        lexical_owner: SymbolHandle,
    ) -> Option<(type_identity::LifetimeParameterIdentity, bool)> {
        let scope = self.scope(program)?;
        scope.with_identity_scopes(program, |frames| {
            let mut visible = frames.get(..preceding_frames)?;
            let recorded_owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            let retained_owner = if recorded_owner.is_valid() {
                recorded_owner
            } else {
                lexical_owner
            };
            let owner = if retained_owner.is_valid() {
                let region_owner =
                    type_identity::lifetime_parameter_identity(program, retained_owner, name)?
                        .declaration;
                let frame = visible.iter().rposition(|frame| {
                    program.lifetime_declaration_owner(frame.declaration)
                        == program.lifetime_declaration_owner(region_owner)
                });
                if let Some(ordinal) = frame {
                    visible = &visible[..=ordinal];
                    return type_identity::applied_lifetime_parameter_identity_with_supplier(
                        program,
                        retained_owner,
                        self.argument_owner,
                        name,
                        visible,
                    );
                } else {
                    visible = &[];
                    retained_owner
                }
            } else {
                self.argument_owner
            };
            type_identity::applied_lifetime_parameter_identity_with_supplier(
                program,
                owner,
                self.argument_owner,
                name,
                visible,
            )
        })
    }

    pub(crate) fn resolved_lifetime_identity(
        &self,
        program: &TypedTrees,
        region: type_identity::LifetimeParameterIdentity,
        preceding_frames: usize,
    ) -> Option<(type_identity::LifetimeParameterIdentity, bool)> {
        let scope = self.scope(program)?;
        scope.with_identity_scopes(program, |frames| {
            type_identity::apply_resolved_lifetime_parameter_identity(
                program,
                region,
                self.argument_owner,
                frames.get(..preceding_frames)?,
            )
        })
    }

    pub(crate) fn frame_count(&self) -> usize {
        self.scope_starts.len()
    }

    pub(crate) fn argument_owner(&self) -> SymbolHandle {
        self.argument_owner
    }

    pub fn scope<'program>(
        &self,
        program: &'program TypedTrees,
    ) -> Option<AppliedTraitScope<'program>> {
        Some(AppliedTraitScope {
            declaration: program.trait_definition_by_symbol(self.declaration)?,
            argument_owner: self.argument_owner,
            substitutions: self.substitutions.clone(),
            substitution_scope_starts: self.scope_starts.clone(),
            region_scopes: self.region_scopes.clone(),
        })
    }

    pub fn retain(&self, program: &mut TypedTrees) -> Result<RetainedTraitScope, &'static str> {
        if self.scope_starts.len() != self.region_scopes.len() {
            return Err("trait application lost its simultaneous region frames");
        }
        let mut frames = Vec::with_capacity(self.scope_starts.len());
        for (ordinal, region) in self.region_scopes.iter().enumerate() {
            let start = self.scope_starts[ordinal];
            let end = self
                .scope_starts
                .get(ordinal + 1)
                .copied()
                .unwrap_or(self.substitutions.len());
            let arguments = self
                .substitutions
                .get(start..end)
                .ok_or("trait application has an invalid substitution frame")?;
            let arguments =
                program
                    .callable_substitutions
                    .insert_many(arguments.iter().map(|(parameter, actual)| {
                        CallableSubstitution {
                            parameter: *parameter,
                            actual: *actual,
                        }
                    }));
            let lifetimes =
                program
                    .callable_lifetime_substitutions
                    .insert_many(region.lifetimes.iter().map(|(parameter, actual)| {
                        StaticMachineLifetimeBinding {
                            parameter: Identifier::generated(parameter),
                            argument: Identifier::generated(actual),
                        }
                    }));
            frames.push(RetainedTraitApplicationFrame {
                declaration: region.declaration,
                arguments,
                lifetimes,
            });
        }
        Ok(RetainedTraitScope {
            declaration: self.declaration,
            argument_owner: self.argument_owner,
            frames: program.trait_application_frames.insert_many(frames),
        })
    }
}

impl RetainedTraitScope {
    pub fn supplying_owner(self) -> SymbolHandle {
        self.argument_owner
    }

    pub(crate) fn visit_references(
        self,
        program: &TypedTrees,
        retained: &mut impl crate::source_state::references::TypedReferences,
    ) {
        retained.formation_owner(self.argument_owner);
        for frame in program.trait_application_frames.span_or_empty(self.frames) {
            retained.declaration_description_dependency(frame.declaration);
            for binding in program
                .callable_substitutions
                .span_or_empty(frame.arguments)
            {
                // A retained application can be visited from the
                // required trait rather than either binder's author.
                retained.enter_lexical_scope(frame.declaration);
                retained.declaration_description_dependency(binding.parameter);
                retained.leave_lexical_scope();

                // Copies retain an operand's original source scope.
                // The application supplier owns only operands without
                // a more specific retained author; target parentage
                // cannot supply missing lexical authority.
                let owner = program
                    .type_reference_table
                    .operand_lifetime_owner(binding.actual);
                let owner = if owner.is_valid() {
                    owner
                } else {
                    self.argument_owner
                };
                retained.enter_lexical_scope(owner);
                crate::source_state::references::visit_type_reference(
                    program,
                    retained,
                    binding.actual,
                );
                retained.leave_lexical_scope();
            }
        }
    }

    pub fn scope<'program>(
        self,
        program: &'program TypedTrees,
    ) -> Option<AppliedTraitScope<'program>> {
        let mut substitutions = Vec::new();
        let mut scope_starts = Vec::new();
        let mut region_scopes = Vec::new();
        for frame in program.trait_application_frames.span_or_empty(self.frames) {
            scope_starts.push(substitutions.len());
            substitutions.extend(
                program
                    .callable_substitutions
                    .span_or_empty(frame.arguments)
                    .iter()
                    .map(|binding| (binding.parameter, binding.actual)),
            );
            region_scopes.push(AppliedDeclarationRegionScope {
                declaration: frame.declaration,
                lifetimes: program
                    .callable_lifetime_substitutions
                    .span_or_empty(frame.lifetimes)
                    .iter()
                    .map(|binding| {
                        (
                            binding.parameter.as_str().to_owned(),
                            binding.argument.as_str().to_owned(),
                        )
                    })
                    .collect(),
            });
        }
        Some(AppliedTraitScope {
            declaration: program.trait_definition_by_symbol(self.declaration)?,
            argument_owner: self.argument_owner,
            substitutions,
            substitution_scope_starts: scope_starts,
            region_scopes,
        })
    }
}

#[cfg(test)]
mod retained_reference_tests {
    use super::*;
    use crate::source_state::references::TypedReferences;
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    struct ScopedReferences {
        owner: SymbolHandle,
        outer: Vec<SymbolHandle>,
        descriptions: Vec<(SymbolHandle, SymbolHandle)>,
        names: Vec<(SymbolHandle, SymbolHandle)>,
    }
    impl Extend<SymbolHandle> for ScopedReferences {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
            self.names
                .extend(symbols.into_iter().map(|symbol| (self.owner, symbol)));
        }
    }
    impl TypedReferences for ScopedReferences {
        fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
            self.outer.push(self.owner);
            self.owner = owner;
        }
        fn leave_lexical_scope(&mut self) {
            self.owner = self.outer.pop().expect("paired retained scopes");
        }
        fn declaration_description_dependency(&mut self, symbol: SymbolHandle) {
            self.descriptions.push((self.owner, symbol));
        }
    }

    #[test]
    fn retained_trait_bindings_keep_formal_and_actual_authors() {
        let mut symbols = SymbolTableBuilder::default();
        let observed = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Required"));
        let formal_owner = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Generic"));
        let nested_owner = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Nested"));
        let supplier = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Supplier"));
        let retained_supplier = symbols.insert_root(
            SymbolKind::Machine,
            SymbolNameRef::Static("OriginalSupplier"),
        );
        let formal = symbols
            .insert_children(
                formal_owner,
                [(
                    SymbolKind::ConformanceParameter,
                    SymbolNameRef::Static("Policy"),
                )],
            )
            .handle_at(0);
        let nested_formal = symbols
            .insert_children(
                nested_owner,
                [(SymbolKind::TypeParameter, SymbolNameRef::Static("Element"))],
            )
            .handle_at(0);
        let actual = symbols
            .insert_children(
                supplier,
                [(
                    SymbolKind::ConformanceParameter,
                    SymbolNameRef::Static("Selected"),
                )],
            )
            .handle_at(0);
        let nested_actual = symbols
            .insert_children(
                retained_supplier,
                [(
                    SymbolKind::TypeParameter,
                    SymbolNameRef::Static("OriginalElement"),
                )],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let actual_type = program.type_reference_table.insert(
            crate::source_state::types::TypeReferenceNode::Named {
                symbol: actual,
                name: Identifier::generated("Selected"),
            },
        );
        let nested_type = program.type_reference_table.insert(
            crate::source_state::types::TypeReferenceNode::Named {
                symbol: nested_actual,
                name: Identifier::generated("OriginalElement"),
            },
        );
        program
            .type_reference_table
            .retain_operand_lifetime_owner(nested_type, retained_supplier)
            .unwrap();
        let scope = TraitApplicationEnvironment {
            declaration: observed,
            argument_owner: supplier,
            substitutions: vec![(formal, actual_type), (nested_formal, nested_type)],
            scope_starts: vec![0, 1],
            region_scopes: vec![
                AppliedDeclarationRegionScope {
                    declaration: formal_owner,
                    lifetimes: Vec::new(),
                },
                AppliedDeclarationRegionScope {
                    declaration: nested_owner,
                    lifetimes: Vec::new(),
                },
            ],
        }
        .retain(&mut program)
        .unwrap();
        let mut references = ScopedReferences {
            owner: observed,
            outer: Vec::new(),
            descriptions: Vec::new(),
            names: Vec::new(),
        };
        scope.visit_references(&program, &mut references);
        assert!(references.descriptions.contains(&(observed, supplier)));
        assert!(references.descriptions.contains(&(observed, formal_owner)));
        assert!(references.descriptions.contains(&(observed, nested_owner)));
        assert!(references.descriptions.contains(&(formal_owner, formal)));
        assert!(
            references
                .descriptions
                .contains(&(nested_owner, nested_formal))
        );
        assert!(!references.descriptions.contains(&(observed, formal)));
        assert!(references.names.contains(&(supplier, actual)));
        assert!(
            references
                .names
                .contains(&(retained_supplier, nested_actual))
        );
        assert!(!references.names.contains(&(supplier, nested_actual)));
        assert_eq!(references.owner, observed);
        assert!(references.outer.is_empty());
    }
}
