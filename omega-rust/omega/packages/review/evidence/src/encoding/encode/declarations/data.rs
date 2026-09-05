use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::contracts::{encode_callable_contract, encode_contract_fact};
use super::super::values::crashes::encode_crash_route;
use super::super::values::effects::{encode_synchronous_invocation, encode_termination};
use super::super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewDataField, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataProperties, PackageReviewDataShape, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewTypeIdentity, PackageReviewTypeParameter,
    PackageReviewTypeParameterKind,
};

pub(crate) fn encode_data_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDataShape,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &shape.identity)
    })?;
    encoder.field("kind", |encoder| {
        match &shape.kind {
            PackageReviewDataKind::Ordinary => encoder.tag("ordinary", 0),
            PackageReviewDataKind::Quotient { carrier, relation } => {
                encoder.tag("quotient", 1);
                encoder.field("carrier", |encoder| encode_type_identity(encoder, carrier))?;
                encoder.field("relation", |encoder| encode_nominal(encoder, relation))?;
            }
        };
        Ok(())
    })?;
    encoder.field("supply", |encoder| {
        match shape.supply {
            psi_language_semantics::DataSupplyMode::CheckedShape => encoder.tag("checked_shape", 0),
            psi_language_semantics::DataSupplyMode::BoundaryOpaque => {
                encoder.tag("boundary_opaque", 1)
            }
        };
        Ok(())
    })?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(shape.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&shape.type_parameters, encode_type_parameter)
    })?;
    encoder.field("properties", |encoder| {
        encode_data_properties(encoder, shape.properties);
        Ok(())
    })?;
    encoder.field("zero_gated", |encoder| {
        encoder.boolean(shape.zero_gated);
        Ok(())
    })?;
    encoder.field("invariants", |encoder| {
        encoder.sequence(&shape.invariants, encode_contract_fact)
    })?;
    encoder.field("retired_identities", |encoder| {
        encoder.sequence(&shape.retired_identities, |encoder, identity| {
            encoder.field("identity", |encoder| {
                encoder.u64(*identity);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("members", |encoder| {
        encoder.sequence(&shape.members, encode_data_member)
    })
}

pub(crate) fn encode_type_parameter(
    encoder: &mut Encoder,
    parameter: &PackageReviewTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match &parameter.kind {
            PackageReviewTypeParameterKind::Type => encoder.tag("type", 0),
            PackageReviewTypeParameterKind::Const(type_identity) => {
                encoder.tag("const", 1);
                encoder.field("type_identity", |encoder| {
                    encode_type_identity(encoder, type_identity)
                })?;
            }
            PackageReviewTypeParameterKind::Machine(contract) => {
                encoder.tag("machine", 2);
                encoder.field("contract", |encoder| {
                    encode_machine_parameter_contract(encoder, contract)
                })?;
            }
            PackageReviewTypeParameterKind::Proposition(signature) => {
                encoder.tag("proposition", 3);
                encoder.field("parameters", |encoder| {
                    encoder.sequence(&signature.parameters, |encoder, parameter| {
                        encoder.field("type_identity", |encoder| {
                            encode_type_identity(encoder, &parameter.type_identity)
                        })
                    })
                })?;
            }
        };
        Ok(())
    })?;
    encoder.field("bounds", |encoder| {
        encode_data_properties(encoder, parameter.bounds);
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn encode_machine_parameter_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewMachineParameterContract,
) -> Result<(), PackageReviewEncodingError> {
    encoder.nested(|encoder| match contract {
        PackageReviewMachineParameterContract::Structural(signature) => {
            encoder.tag("structural", 0);
            encoder.field("signature", |encoder| {
                encode_machine_parameter_signature(encoder, signature)
            })
        }
        PackageReviewMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => {
            encoder.tag("nominal", 1);
            encoder.field("trait_identity", |encoder| {
                encode_nominal(encoder, trait_identity)
            })?;
            encoder.field("requirement_identity", |encoder| {
                encode_nominal(encoder, requirement_identity)
            })
        }
        PackageReviewMachineParameterContract::RequirementIdentity => {
            encoder.tag("requirement_identity", 2);
            Ok(())
        }
    })
}

pub(crate) fn encode_machine_parameter_signature(
    encoder: &mut Encoder,
    signature: &PackageReviewMachineParameterSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(signature.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&signature.type_parameters, encode_type_parameter)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&signature.parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("type_identity", |encoder| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
            encoder.field("is_const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("is_mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("is_self", |encoder| {
                encoder.boolean(parameter.is_self);
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("return_type", |encoder| {
        encode_type_identity(encoder, &signature.return_type)
    })?;
    encoder.field("contracts", |encoder| {
        encoder.sequence(&signature.contracts, encode_callable_contract)
    })?;
    encoder.field("published_crash", |encoder| {
        encoder.sequence(&signature.published_crash, encode_crash_route)
    })?;
    encoder.field("service_reach", |encoder| {
        encoder.sequence(&signature.service_reach, encode_nominal)
    })?;
    encoder.field("service_reach_is_installation_bound", |encoder| {
        encoder.boolean(signature.service_reach_is_installation_bound);
        Ok(())
    })?;
    encoder.field("synchronous_invocations", |encoder| {
        encoder.sequence(
            &signature.synchronous_invocations,
            encode_synchronous_invocation,
        )
    })?;
    encoder.field("suspends", |encoder| {
        encoder.boolean(signature.suspends);
        Ok(())
    })?;
    encoder.field("blocks", |encoder| {
        encoder.boolean(signature.blocks);
        Ok(())
    })?;
    encoder.field("termination", |encoder| {
        encode_termination(encoder, &signature.termination)
    })
}

pub(crate) fn encode_data_properties(
    encoder: &mut Encoder,
    properties: PackageReviewDataProperties,
) {
    let _ = encoder.field("multiplicity", |encoder| {
        match properties.multiplicity {
            psi_language_semantics::Multiplicity::Unrestricted => encoder.tag("unrestricted", 0),
            psi_language_semantics::Multiplicity::Affine => encoder.tag("affine", 1),
            psi_language_semantics::Multiplicity::Linear => encoder.tag("linear", 2),
        };
        Ok(())
    });
    let _ = encoder.field("carry", |encoder| {
        match properties.carry {
            None => encoder.tag("none", 0),
            Some(carry) => {
                encoder.tag("some", 1);
                let _ = encoder.field("suspension", |encoder| {
                    match carry.suspension {
                        psi_language_semantics::CarrySuspension::Forbidden => {
                            encoder.tag("forbidden", 0)
                        }
                        psi_language_semantics::CarrySuspension::Allowed => {
                            encoder.tag("allowed", 1)
                        }
                    };
                    Ok(())
                });
                let _ = encoder.field("cpu", |encoder| {
                    match carry.cpu {
                        psi_language_semantics::CarryCpu::Origin => encoder.tag("origin", 0),
                        psi_language_semantics::CarryCpu::Any => encoder.tag("any", 1),
                    };
                    Ok(())
                });
                let _ = encoder.field("host_thread", |encoder| {
                    match carry.host_thread {
                        psi_language_semantics::CarryHostThread::Origin => encoder.tag("origin", 0),
                        psi_language_semantics::CarryHostThread::Any => encoder.tag("any", 1),
                    };
                    Ok(())
                });
                let _ = encoder.field("address", |encoder| {
                    match carry.address {
                        psi_language_semantics::CarryAddress::Stable => encoder.tag("stable", 0),
                        psi_language_semantics::CarryAddress::Movable => encoder.tag("movable", 1),
                    };
                    Ok(())
                });
            }
        };
        Ok(())
    });
}

pub(crate) fn encode_data_member(
    encoder: &mut Encoder,
    member: &PackageReviewDataMember,
) -> Result<(), PackageReviewEncodingError> {
    match member {
        PackageReviewDataMember::Field(field) => {
            encoder.tag("field", 0);
            encoder.field("field", |encoder| encode_data_field(encoder, field))?;
        }
        PackageReviewDataMember::Variant {
            identity,
            name,
            payload,
            retired_payload_identities,
        } => {
            encoder.tag("variant", 1);
            encoder.field("identity", |encoder| {
                encode_optional_u64(encoder, *identity);
                Ok(())
            })?;
            encoder.field("name", |encoder| encoder.string(name))?;
            encoder.field("payload", |encoder| {
                encoder.sequence(payload, encode_data_field)
            })?;
            encoder.field("retired_payload_identities", |encoder| {
                encoder.sequence(retired_payload_identities, |encoder, identity| {
                    encoder.field("identity", |encoder| {
                        encoder.u64(*identity);
                        Ok(())
                    })?;
                    Ok(())
                })
            })?;
        }
    }
    Ok(())
}

pub(crate) fn encode_data_field(
    encoder: &mut Encoder,
    field: &PackageReviewDataField,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("identity", |encoder| {
        encode_optional_u64(encoder, field.identity);
        Ok(())
    })?;
    encoder.field("name", |encoder| encoder.string(&field.name))?;
    encoder.field("relevance", |encoder| {
        encode_relevance(encoder, field.relevance);
        Ok(())
    })?;
    encoder.field("type_identity", |encoder| {
        encode_type_identity(encoder, &field.type_identity)
    })
}

pub(crate) fn encode_type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("canonical", |encoder| encoder.string(&identity.canonical))
}

pub(crate) fn encode_relevance(
    encoder: &mut Encoder,
    relevance: psi_language_core::BindingRelevance,
) {
    let _ = encoder.field("relevance", |encoder| {
        match relevance {
            psi_language_core::BindingRelevance::Relevant => encoder.tag("relevant", 0),
            psi_language_core::BindingRelevance::Erased => encoder.tag("erased", 1),
        };
        Ok(())
    });
}

pub(crate) fn encode_optional_u64(encoder: &mut Encoder, value: Option<u64>) {
    match value {
        None => encoder.tag("none", 0),
        Some(value) => {
            encoder.tag("some", 1);
            let _ = encoder.field("value", |encoder| {
                encoder.u64(value);
                Ok(())
            });
        }
    }
}
