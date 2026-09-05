use super::*;

pub(super) fn encode_contract_expression_body(
    encoder: &mut Encoder,
    expression: &PackageReviewContractExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewContractExpression::Boolean(value) => {
            encoder.tag("boolean", 0);
            encoder.field("value", |encoder| {
                encoder.boolean(*value);
                Ok(())
            })?;
        }
        PackageReviewContractExpression::Integer(value) => {
            encoder.tag("integer", 1);
            encoder.field("value", |encoder| encoder.string(value))?;
        }
        PackageReviewContractExpression::Float(value) => {
            encoder.tag("float", 19);
            encoder.field("literal", |encoder| {
                match value {
                    PackageReviewFloatLiteral::F32(bits) => {
                        encoder.tag("f32", 0);
                        encoder.field("bits", |encoder| {
                            encoder.u32(*bits);
                            Ok(())
                        })?;
                    }
                    PackageReviewFloatLiteral::F64(bits) => {
                        encoder.tag("f64", 1);
                        encoder.field("bits", |encoder| {
                            encoder.u64(*bits);
                            Ok(())
                        })?;
                    }
                };
                Ok(())
            })?;
        }
        PackageReviewContractExpression::Array(values) => {
            encoder.tag("array", 14);
            encoder.field("values", |encoder| {
                encoder.sequence(values, encode_contract_expression)
            })?;
        }
        PackageReviewContractExpression::Constructor { data, case, fields } => {
            encoder.tag("constructor", 15);
            encoder.field("data", |encoder| encode_nominal(encoder, data))?;
            encoder.field("case", |encoder| {
                encoder.option(case.as_ref(), encode_nominal)
            })?;
            encoder.field("fields", |encoder| {
                encoder.sequence(fields, |encoder, field| {
                    encoder.field("field", |encoder| encode_nominal(encoder, &field.field))?;
                    encoder.field("value", |encoder| {
                        encode_contract_expression(encoder, &field.value)
                    })
                })
            })?;
        }
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        } => {
            encoder.tag("indexed", 16);
            encoder.field("meaning", |encoder| {
                encode_contract_operator_meaning(encoder, meaning)
            })?;
            encoder.field("collection", |encoder| {
                encode_contract_expression(encoder, collection)
            })?;
            encoder.field("index", |encoder| {
                encode_contract_expression(encoder, index)
            })?;
        }
        PackageReviewContractExpression::Range {
            start,
            end,
            end_inclusive,
        } => {
            encoder.tag("range", 17);
            encoder.field("start", |encoder| {
                encoder.option(start.as_deref(), encode_contract_expression)
            })?;
            encoder.field("end", |encoder| {
                encoder.option(end.as_deref(), encode_contract_expression)
            })?;
            encoder.field("end_inclusive", |encoder| {
                encoder.boolean(*end_inclusive);
                Ok(())
            })?;
        }
        PackageReviewContractExpression::ByteSequence(value) => {
            encoder.tag("byte_sequence", 12);
            encoder.field("value", |encoder| encoder.bytes(value))?;
        }
        PackageReviewContractExpression::DomainSubject => encoder.tag("domain_subject", 10),
        PackageReviewContractExpression::Parameter(position) => {
            encoder.tag("parameter", 2);
            encoder.field("position", |encoder| {
                encoder.u32(*position);
                Ok(())
            })?;
        }
        PackageReviewContractExpression::Result => encoder.tag("result", 3),
        PackageReviewContractExpression::GenericBinder(position) => {
            encoder.tag("generic_binder", 4);
            encoder.field("position", |encoder| {
                encoder.u32(*position);
                Ok(())
            })?;
        }
        PackageReviewContractExpression::Nominal(identity) => {
            encoder.tag("nominal", 5);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
        }
        PackageReviewContractExpression::Reference { access, target } => {
            encoder.tag("reference", 20);
            encoder.field("access", |encoder| {
                match access {
                    PackageReviewReferenceAccess::Shared => encoder.tag("shared", 0),
                    PackageReviewReferenceAccess::Mutable => encoder.tag("mutable", 1),
                    PackageReviewReferenceAccess::WriteOnly => encoder.tag("write_only", 2),
                };
                Ok(())
            })?;
            encoder.field("target", |encoder| {
                encode_contract_expression(encoder, target)
            })?;
        }
        PackageReviewContractExpression::AtomicLoad { value, ordering } => {
            encoder.tag("atomic_load", 21);
            encoder.field("ordering", |encoder| {
                match ordering {
                    PackageReviewAtomicLoadOrdering::NoOrdering => encoder.tag("no_ordering", 0),
                    PackageReviewAtomicLoadOrdering::Receive => encoder.tag("receive", 1),
                    PackageReviewAtomicLoadOrdering::GlobalOrder => encoder.tag("global_order", 2),
                };
                Ok(())
            })?;
            encoder.field("value", |encoder| {
                encode_contract_expression(encoder, value)
            })?;
        }
        PackageReviewContractExpression::ZeroValue(type_identity) => {
            encoder.tag("zero_value", 13);
            encoder.field("type_identity", |encoder| {
                encode_type_identity(encoder, type_identity)
            })?;
        }
        PackageReviewContractExpression::CollectionLength { collection } => {
            encoder.tag("collection_length", 18);
            encoder.field("collection", |encoder| {
                encode_contract_expression(encoder, collection)
            })?;
        }
        PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } => {
            encoder.tag("member", 8);
            encoder.field("receiver", |encoder| {
                encode_contract_expression(encoder, receiver)
            })?;
            encoder.field("member", |encoder| encode_nominal(encoder, member))?;
            encoder.field("case_variant", |encoder| {
                encoder.option(case_variant.as_ref(), encode_nominal)
            })?;
        }
        PackageReviewContractExpression::Cast {
            value,
            target,
            arithmetic_domain,
            semantic_domain,
            semantic_domain_arguments,
            form,
        } => {
            encoder.tag("cast", 9);
            encoder.field("value", |encoder| {
                encode_contract_expression(encoder, value)
            })?;
            encoder.field("target", |encoder| encode_type_identity(encoder, target))?;
            encoder.field("arithmetic_domain", |encoder| {
                match arithmetic_domain {
                    PackageReviewArithmeticDomain::Exact => encoder.tag("exact", 0),
                    PackageReviewArithmeticDomain::Wrapping => encoder.tag("wrapping", 1),
                    PackageReviewArithmeticDomain::Saturating => encoder.tag("saturating", 2),
                    PackageReviewArithmeticDomain::Trapping => encoder.tag("trapping", 3),
                };
                Ok(())
            })?;
            encoder.field("semantic_domain", |encoder| {
                encoder.option(semantic_domain.as_ref(), encode_nominal)
            })?;
            encoder.field("semantic_domain_arguments", |encoder| {
                encoder.sequence(semantic_domain_arguments, encode_type_identity)
            })?;
            encoder.field("form", |encoder| {
                match form {
                    PackageReviewCastForm::Value => encoder.tag("value", 0),
                    PackageReviewCastForm::RecastShared => encoder.tag("recast_shared", 1),
                    PackageReviewCastForm::RecastMutable => encoder.tag("recast_mutable", 2),
                };
                Ok(())
            })?;
        }
        PackageReviewContractExpression::Call {
            receiver,
            target,
            static_arguments,
            evidence_arguments,
            arguments,
        } => {
            encoder.tag("call", 11);
            encoder.field("receiver", |encoder| {
                encoder.option(receiver.as_deref(), encode_contract_expression)
            })?;
            encoder.field("target", |encoder| {
                match target {
                    PackageReviewContractCallTarget::Nominal(identity) => {
                        encoder.tag("nominal", 0);
                        encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
                    }
                    PackageReviewContractCallTarget::BuiltinFunction(function) => {
                        encoder.tag("builtin_function", 2);
                        encoder.field("u16", |encoder| {
                            encoder.u16(u16::try_from(function.ordinal()).map_err(|_| {
                        PackageReviewEncodingError::new(
                            "compiler builtin-function ordinal exceeds the portable encoding range",
                        )
                    })?);
                            Ok(())
                        })?;
                    }
                    PackageReviewContractCallTarget::ByteSequencePredicate(predicate) => {
                        encoder.tag("byte_sequence_predicate", 1);
                        encoder.field("predicate", |encoder| {
                            match predicate {
                                PackageReviewByteSequencePredicate::ValidUtf8 => {
                                    encoder.tag("valid_utf8", 0)
                                }
                                PackageReviewByteSequencePredicate::NoNul => {
                                    encoder.tag("no_nul", 1)
                                }
                                PackageReviewByteSequencePredicate::AsciiOnly => {
                                    encoder.tag("ascii_only", 2)
                                }
                                PackageReviewByteSequencePredicate::NonEmpty => {
                                    encoder.tag("non_empty", 3)
                                }
                            };
                            Ok(())
                        })?;
                    }
                    PackageReviewContractCallTarget::CollectionView(operation) => {
                        encoder.tag("collection_view", 3);
                        encoder.field("operation", |encoder| {
                            match operation {
                                PackageReviewCollectionViewOperation::SharedSlice => {
                                    encoder.tag("shared_slice", 0)
                                }
                                PackageReviewCollectionViewOperation::MutableSlice => {
                                    encoder.tag("mutable_slice", 1)
                                }
                                PackageReviewCollectionViewOperation::TextView => {
                                    encoder.tag("text_view", 2)
                                }
                                PackageReviewCollectionViewOperation::Bytes => {
                                    encoder.tag("bytes", 3)
                                }
                            };
                            Ok(())
                        })?;
                    }
                };
                Ok(())
            })?;
            encoder.field("static_arguments", |encoder| {
                encoder.sequence(static_arguments, encode_contract_static_argument)
            })?;
            encoder.field("evidence_arguments", |encoder| {
                encoder.sequence(evidence_arguments, encode_contract_evidence_argument)
            })?;
            encoder.field("arguments", |encoder| {
                encoder.sequence(arguments, encode_contract_expression)
            })?;
        }
        PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        } => {
            encoder.tag("binary", 6);
            encoder.field("meaning", |encoder| {
                encode_contract_operator_meaning(encoder, meaning)
            })?;
            encoder.field("operator", |encoder| {
                match operator {
                    PackageReviewContractBinaryOperator::Add => encoder.tag("add", 0),
                    PackageReviewContractBinaryOperator::And => encoder.tag("and", 1),
                    PackageReviewContractBinaryOperator::BitwiseAnd => {
                        encoder.tag("bitwise_and", 2)
                    }
                    PackageReviewContractBinaryOperator::BitwiseOr => encoder.tag("bitwise_or", 3),
                    PackageReviewContractBinaryOperator::BitwiseXor => {
                        encoder.tag("bitwise_xor", 4)
                    }
                    PackageReviewContractBinaryOperator::Divide => encoder.tag("divide", 5),
                    PackageReviewContractBinaryOperator::Equal => encoder.tag("equal", 6),
                    PackageReviewContractBinaryOperator::Greater => encoder.tag("greater", 7),
                    PackageReviewContractBinaryOperator::GreaterOrEqual => {
                        encoder.tag("greater_or_equal", 8)
                    }
                    PackageReviewContractBinaryOperator::Less => encoder.tag("less", 9),
                    PackageReviewContractBinaryOperator::LessOrEqual => {
                        encoder.tag("less_or_equal", 10)
                    }
                    PackageReviewContractBinaryOperator::Modulo => encoder.tag("modulo", 11),
                    PackageReviewContractBinaryOperator::Multiply => encoder.tag("multiply", 12),
                    PackageReviewContractBinaryOperator::NotEqual => encoder.tag("not_equal", 13),
                    PackageReviewContractBinaryOperator::Or => encoder.tag("or", 14),
                    PackageReviewContractBinaryOperator::ShiftLeft => encoder.tag("shift_left", 15),
                    PackageReviewContractBinaryOperator::ShiftRight => {
                        encoder.tag("shift_right", 16)
                    }
                    PackageReviewContractBinaryOperator::Subtract => encoder.tag("subtract", 17),
                };
                Ok(())
            })?;
            encoder.field("left", |encoder| encode_contract_expression(encoder, left))?;
            encoder.field("right", |encoder| {
                encode_contract_expression(encoder, right)
            })?;
        }
        PackageReviewContractExpression::Unary { operator, operand } => {
            encoder.tag("unary", 7);
            encoder.field("operator", |encoder| {
                match operator {
                    PackageReviewContractUnaryOperator::BitwiseNot => encoder.tag("bitwise_not", 0),
                    PackageReviewContractUnaryOperator::LogicalNot => encoder.tag("logical_not", 1),
                };
                Ok(())
            })?;
            encoder.field("operand", |encoder| {
                encode_contract_expression(encoder, operand)
            })?;
        }
    }
    Ok(())
}
