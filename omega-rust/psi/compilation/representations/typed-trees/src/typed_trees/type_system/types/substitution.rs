use super::*;

impl TypeReferenceTable {
    pub fn substitute_node(&mut self, handle: TypeReferenceHandle, node: TypeReferenceNode) {
        let replaced = std::mem::replace(self.type_references.get_mut(handle), node);
        let replaced_named = match replaced {
            TypeReferenceNode::Named { symbol, .. } => Some(symbol),
            _ => None,
        };
        self.renamed_row(handle, replaced_named);
    }

    /// Replace an occurrence with another row in this same arena. Child handles
    /// remain shared; both the receiving use's origins and the selected
    /// argument's origins survive the structural substitution.
    pub fn substitute_from(
        &mut self,
        occurrence: TypeReferenceHandle,
        replacement: TypeReferenceHandle,
    ) -> Result<(), &'static str> {
        self.constant_origins(occurrence)?;
        let origins = self.constant_origins(replacement)?.to_vec();
        let node = self.type_reference(replacement).clone();
        self.attach_constant_origins(occurrence, origins)?;
        self.substitute_node(occurrence, node);
        Ok(())
    }

    /// Keep a built named-row index exact after the row at `handle`, which
    /// was a `Named` row for `replaced_named` if any, was rewritten in place.
    pub(super) fn renamed_row(
        &mut self,
        handle: TypeReferenceHandle,
        replaced_named: Option<SymbolHandle>,
    ) {
        if let Some(symbol) = replaced_named
            && self
                .named_rows
                .0
                .get()
                .is_some_and(|rows| rows.get(&symbol) == Some(&handle))
        {
            self.rescan_named_row(symbol);
        }
        if let TypeReferenceNode::Named { symbol, .. } = self.type_references.get(handle) {
            let symbol = *symbol;
            self.note_named_row(symbol, handle);
        }
    }

    /// Rewire monomorphization-clone nodes (`arena_index >= type_start`)
    /// whose direct child still names `parameter_symbol`. Copied compounds
    /// share child handles: `copy_from` reuses the template's interned
    /// `Named` leaf, which sits below `type_start` and is never rewritten in
    /// place (it is the template's own row). The parent therefore keeps a
    /// dangling parameter reference unless its field is repointed at the
    /// argument's fresh node. Call after the in-place `Named` occurrence
    /// substitution so only shared (unsubstituted) leaves remain named.
    /// Returns the number of rewired parent fields.
    pub fn substitute_shared_children(
        &mut self,
        type_start: usize,
        parameter_symbol: SymbolHandle,
        replacement: TypeReferenceHandle,
    ) -> usize {
        let parents: Vec<TypeReferenceHandle> = self
            .type_references
            .iter()
            .filter_map(|(handle, _)| {
                (handle.arena_index() as usize >= type_start).then_some(handle)
            })
            .collect();
        let mut rewritten = 0;
        for parent in parents {
            match self.type_references.get(parent).clone() {
                TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime,
                } if self.named_child_matches(referee, parameter_symbol) => {
                    self.substitute_node(
                        parent,
                        TypeReferenceNode::Reference {
                            referee: replacement,
                            access,
                            lifetime,
                        },
                    );
                    rewritten += 1;
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } if self.named_child_matches(base_type, parameter_symbol) => {
                    self.substitute_node(
                        parent,
                        TypeReferenceNode::Constrained {
                            base_type: replacement,
                            constraints,
                        },
                    );
                    rewritten += 1;
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } if self.named_child_matches(element_type, parameter_symbol) => {
                    self.substitute_node(
                        parent,
                        TypeReferenceNode::FixedArray {
                            element_type: replacement,
                            length,
                        },
                    );
                    rewritten += 1;
                }
                TypeReferenceNode::Slice { element_type }
                    if self.named_child_matches(element_type, parameter_symbol) =>
                {
                    self.substitute_node(
                        parent,
                        TypeReferenceNode::Slice {
                            element_type: replacement,
                        },
                    );
                    rewritten += 1;
                }
                TypeReferenceNode::Generic { arguments, .. } => {
                    for (offset, child) in self
                        .type_reference_handles(arguments)
                        .to_vec()
                        .into_iter()
                        .enumerate()
                    {
                        if self.named_child_matches(child, parameter_symbol) {
                            self.set_type_reference_handle_at_offset(
                                arguments,
                                u32::try_from(offset).expect("generic argument offset"),
                                replacement,
                            );
                            rewritten += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        rewritten
    }

    fn named_child_matches(&self, child: TypeReferenceHandle, symbol: SymbolHandle) -> bool {
        matches!(
            self.type_references.get(child),
            TypeReferenceNode::Named {
                symbol: named,
                ..
            } if *named == symbol
        )
    }

    /// Replace a fixed-array type reference's length with a concrete literal
    /// (the comptime const-eval substitution). Panics if `handle` is not a
    /// fixed-array node -- callers obtain handles from `fixed_array_lengths`.
    /// The type `handle` denotes after replacing each binder in
    /// `substitutions` by its argument type.
    ///
    /// Only rows whose subtree names a binder are rebuilt; every other row,
    /// including the arguments themselves, is shared, so the authored
    /// declaration is never rewritten. A binder inside a domain argument, a
    /// fixed-array length or a const expression has no structural
    /// substitution here and is refused rather than left generic.
    pub fn instantiate(
        &mut self,
        handle: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Result<TypeReferenceHandle, &'static str> {
        if !handle.is_valid() {
            return Ok(handle);
        }
        let rebuilt = match self.type_reference(handle).clone() {
            TypeReferenceNode::Named { symbol, .. } => {
                return Ok(substitutions
                    .iter()
                    .find(|(binder, _)| *binder == symbol)
                    .map_or(handle, |(_, argument)| *argument));
            }
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let instantiated = self.instantiate(referee, substitutions)?;
                (instantiated != referee).then_some(TypeReferenceNode::Reference {
                    referee: instantiated,
                    access,
                    lifetime,
                })
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let mentions_binder = self.constraints(constraints).iter().any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(domain)
                    if domain.arguments.iter().any(|argument| {
                        self.names_any_binder(*argument, substitutions)
                    }))
                });
                if mentions_binder {
                    return Err("a trait parameter inside a domain argument has no instantiation");
                }
                let instantiated = self.instantiate(base_type, substitutions)?;
                (instantiated != base_type).then_some(TypeReferenceNode::Constrained {
                    base_type: instantiated,
                    constraints,
                })
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if let FixedArrayLength::ConstParameter { symbol, .. } = &length
                    && substitutions.iter().any(|(binder, _)| binder == symbol)
                {
                    return Err("a trait parameter as an array length has no instantiation");
                }
                let instantiated = self.instantiate(element_type, substitutions)?;
                (instantiated != element_type).then_some(TypeReferenceNode::FixedArray {
                    element_type: instantiated,
                    length,
                })
            }
            TypeReferenceNode::Slice { element_type } => {
                let instantiated = self.instantiate(element_type, substitutions)?;
                (instantiated != element_type).then_some(TypeReferenceNode::Slice {
                    element_type: instantiated,
                })
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => self
                .instantiate_arguments(arguments, substitutions)?
                .map(|arguments| TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    lifetime_arguments,
                    arguments,
                }),
            TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => self
                .instantiate_arguments(arguments, substitutions)?
                .map(|arguments| TypeReferenceNode::DynamicTrait {
                    symbol,
                    name,
                    arguments,
                }),
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => None,
        };
        Ok(rebuilt.map_or(handle, |node| self.insert(node)))
    }

    /// A rebuilt argument span when any argument changed.
    fn instantiate_arguments(
        &mut self,
        arguments: HandleSpan<TypeReferenceHandle>,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Result<Option<HandleSpan<TypeReferenceHandle>>, &'static str> {
        let original = self.type_reference_handles(arguments).to_vec();
        let mut instantiated = Vec::with_capacity(original.len());
        for argument in &original {
            instantiated.push(self.instantiate(*argument, substitutions)?);
        }
        Ok((instantiated != original).then(|| self.insert_type_reference_handles(instantiated)))
    }

    fn names_any_binder(
        &self,
        handle: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        if !handle.is_valid() {
            return false;
        }
        match self.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => {
                substitutions.iter().any(|(binder, _)| binder == symbol)
            }
            TypeReferenceNode::Reference { referee, .. } => {
                self.names_any_binder(*referee, substitutions)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.names_any_binder(*base_type, substitutions)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                self.names_any_binder(*element_type, substitutions)
            }
            TypeReferenceNode::Generic { arguments, .. }
            | TypeReferenceNode::DynamicTrait { arguments, .. } => self
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| self.names_any_binder(*argument, substitutions)),
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
        }
    }
}
