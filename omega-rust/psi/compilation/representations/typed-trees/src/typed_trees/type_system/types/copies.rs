use super::*;

impl TypeReferenceTable {
    pub fn copy_from(
        &mut self,
        source: &TypeReferenceTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        type_reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        let copied = match source.type_reference(type_reference) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let referee =
                    self.copy_from(source, source_expressions, target_expressions, *referee);
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    access: *access,
                    lifetime: lifetime.clone(),
                })
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_type =
                    self.copy_from(source, source_expressions, target_expressions, *base_type);
                let constraints = self.copy_constraints_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *constraints,
                );
                self.insert(TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                })
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_type = self.copy_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *element_type,
                );
                let length = match length {
                    FixedArrayLength::Expression(reference) => FixedArrayLength::Expression(
                        self.copy_from(source, source_expressions, target_expressions, *reference),
                    ),
                    other => other.clone(),
                };
                self.insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                })
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = self.copy_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *element_type,
                );
                self.insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let arguments = self.copy_type_reference_handles_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *arguments,
                );
                self.insert(TypeReferenceNode::Generic {
                    base_symbol: *base_symbol,
                    base_name: base_name.clone(),
                    lifetime_arguments: lifetime_arguments.clone(),
                    arguments,
                })
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let expression = target_expressions.copy_from(source_expressions, *expression);
                self.insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => {
                let arguments = self.copy_type_reference_handles_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *arguments,
                );
                self.insert(TypeReferenceNode::DynamicTrait {
                    symbol: *symbol,
                    name: name.clone(),
                    arguments,
                })
            }
            TypeReferenceNode::Named { symbol, name } => self.insert(TypeReferenceNode::Named {
                symbol: *symbol,
                name: name.clone(),
            }),
            TypeReferenceNode::Unit => self.insert(TypeReferenceNode::Unit),
        };
        // Copying retains the same authored declaration identities, like the
        // expression table's occurrence metadata. Lexical remapping below must
        // not turn these original selections into generated local selections.
        let source_owner =
            Handle::from_parts(type_reference.arena_index(), type_reference.generation());
        let target_owner = Handle::from_parts(copied.arena_index(), copied.generation());
        let span = *source.constant_origin_spans.get(source_owner);
        *self.constant_origin_spans.get_mut(target_owner) = self
            .constant_origins
            .insert_many(source.constant_origins.span_or_empty(span).iter().copied());
        *self.operand_lifetime_owners.get_mut(Handle::from_parts(
            copied.arena_index(),
            copied.generation(),
        )) = source.operand_lifetime_owner(type_reference);
        self.set_operand_origin(copied, source.operand_origin(type_reference));
        copied
    }

    /// Remap lexical symbols reachable from a type reference copied into a
    /// fresh scope. Constraint expressions remain owned by the expression
    /// table and are remapped there.
    pub fn remap_symbols_in(
        &mut self,
        type_reference: TypeReferenceHandle,
        expressions: &mut crate::expression::ExpressionTable,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        if !type_reference.is_valid() {
            return;
        }
        let node = self.type_reference(type_reference).clone();
        match node {
            TypeReferenceNode::Reference { referee, .. } => {
                self.remap_symbols_in(referee, expressions, symbols);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.remap_symbols_in(base_type, expressions, symbols);
                for constraint in self.constraints(constraints).to_vec() {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            for argument in domain.arguments {
                                self.remap_symbols_in(argument, expressions, symbols);
                            }
                        }
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            expressions.remap_symbols_in(minimum, symbols);
                            expressions.remap_symbols_in(maximum, symbols);
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                self.remap_symbols_in(element_type, expressions, symbols);
                if let FixedArrayLength::Expression(reference) = length {
                    self.remap_symbols_in(reference, expressions, symbols);
                }
                if let FixedArrayLength::ConstParameter { symbol, .. } = length
                    && let TypeReferenceNode::FixedArray { length, .. } =
                        self.type_references.get_mut(type_reference)
                    && let FixedArrayLength::ConstParameter {
                        symbol: current, ..
                    } = length
                {
                    *current = remapped(symbol, symbols);
                }
            }
            TypeReferenceNode::Slice { element_type } => {
                self.remap_symbols_in(element_type, expressions, symbols);
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                for argument in self.type_reference_handles(arguments).to_vec() {
                    self.remap_symbols_in(argument, expressions, symbols);
                }
                let TypeReferenceNode::Generic {
                    base_symbol: current,
                    ..
                } = self.type_references.get_mut(type_reference)
                else {
                    unreachable!();
                };
                *current = remapped(base_symbol, symbols);
            }
            TypeReferenceNode::ConstExpression(expression) => {
                expressions.remap_symbols_in(expression, symbols);
            }
            TypeReferenceNode::DynamicTrait {
                symbol, arguments, ..
            } => {
                for argument in self.type_reference_handles(arguments).to_vec() {
                    self.remap_symbols_in(argument, expressions, symbols);
                }
                let TypeReferenceNode::DynamicTrait {
                    symbol: current, ..
                } = self.type_references.get_mut(type_reference)
                else {
                    unreachable!();
                };
                *current = remapped(symbol, symbols);
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let replaced_named = matches!(
                    self.type_references.get(type_reference),
                    TypeReferenceNode::Named { .. }
                )
                .then_some(symbol);
                let TypeReferenceNode::Named {
                    symbol: current, ..
                } = self.type_references.get_mut(type_reference)
                else {
                    unreachable!();
                };
                *current = remapped(symbol, symbols);
                self.renamed_row(type_reference, replaced_named);
            }
            TypeReferenceNode::Unit => {}
        }
    }

    fn copy_type_reference_handles_from(
        &mut self,
        source: &TypeReferenceTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        type_references: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        let source_type_references = source.type_reference_handles(type_references);
        let copied = self.reserve_type_reference_handles(
            source_type_references
                .len()
                .try_into()
                .expect("type reference handle span count overflow"),
        );

        for (offset, type_reference) in source_type_references.iter().enumerate() {
            let type_reference = self.copy_from(
                source,
                source_expressions,
                target_expressions,
                *type_reference,
            );
            self.set_type_reference_handle_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("type reference handle span count overflow"),
                type_reference,
            );
        }

        copied
    }

    fn copy_constraints_from(
        &mut self,
        source: &TypeReferenceTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        let source_constraints = source.constraints(constraints);
        let copied = self.reserve_constraints(
            source_constraints
                .len()
                .try_into()
                .expect("type constraint span count overflow"),
        );

        for (offset, constraint) in source_constraints.iter().enumerate() {
            let copied_constraint = match constraint {
                TypeConstraintNode::Domain(domain) => {
                    let mut domain = domain.clone();
                    domain.arguments = domain
                        .arguments
                        .iter()
                        .map(|argument| {
                            self.copy_from(
                                source,
                                source_expressions,
                                target_expressions,
                                *argument,
                            )
                        })
                        .collect();
                    TypeConstraintNode::Domain(domain)
                }
                _ => constraint.copy_from(source_expressions, target_expressions),
            };
            self.set_constraint_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("type constraint span count overflow"),
                copied_constraint,
            );
        }

        copied
    }
}
