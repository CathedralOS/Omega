//! Control transfer, return, cleanup, and crash tags.

use super::super::AbstractSuccessor;
use super::{AbstractOperation, CanonicalBytes};
use crate::optimization_unit::identity::carrier_encoding::{
    encode_binding, encode_ids, encode_scalar_type, encode_structural_argument,
    encode_structural_binding, encode_structural_path_segment,
};
use crate::optimization_unit::identity::structural_encoding::{
    encode_cleanup, encode_crash_cause, encode_crash_predicate, encode_place_declaration,
    encode_structural_type,
};
pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::InvokeNoNormal {
            psi_edge,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => {
            bytes.u8(253);
            bytes.id(*psi_edge);
            bytes.id(*callee);
            encode_ids(bytes, arguments);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
                bytes.slice(&transfer.path, encode_structural_path_segment);
            });
            encode_ids(bytes, requirement_obligations);
            bytes.slice(
                crash_continuations,
                crate::optimization_unit::identity::structural_encoding::encode_crash_route_bucket,
            );
        }

        O::Jump {
            psi_edge,
            target,
            bindings,
            structural_bindings,
            cleanup_actions,
        } => {
            bytes.u8(42);
            bytes.id(*psi_edge);
            bytes.id(*target);
            bytes.slice(bindings, encode_binding);
            bytes.slice(structural_bindings, encode_structural_binding);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            bytes.u8(43);
            bytes.id(*condition);
            encode_successor(bytes, when_true);
            encode_successor(bytes, when_false);
        }
        O::StructuralCase {
            source,
            path,
            cases,
        } => {
            bytes.u8(48);
            bytes.id(*source);
            bytes.slice(path, encode_structural_path_segment);
            bytes.len(cases.len());
            for case in cases {
                bytes.id(case.psi_edge);
                bytes.id(case.target);
                bytes.id(case.case);
                bytes.len(case.payloads.len());
                for payload in &case.payloads {
                    bytes.id(payload.parameter);
                    bytes.id(payload.field);
                    encode_scalar_type(bytes, payload.scalar_type);
                }
                bytes.slice(&case.cleanup_actions, encode_cleanup);
            }
        }
        O::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        } => {
            bytes.u8(44);
            bytes.id(*psi_edge);
            bytes.id(*result);
            bytes.id(*value);
            encode_scalar_type(bytes, *scalar_type);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => {
            bytes.u8(45);
            bytes.id(*psi_edge);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_locals,
            cleanup_actions,
        } => {
            bytes.u8(46);
            bytes.id(*psi_edge);
            bytes.id(*source);
            bytes.slice(returned_claims, |bytes, binding| {
                bytes.id(binding.claim);
                bytes.slice(&binding.path, encode_structural_path_segment);
            });
            bytes.len(trivial_affine_locals.len());
            for (operation, place, structural_type) in trivial_affine_locals {
                bytes.id(*operation);
                encode_place_declaration(bytes, *place);
                encode_structural_type(bytes, structural_type);
            }
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
        } => {
            bytes.u8(101);
            bytes.id(*psi_edge);
            bytes.id(*boundary);
            match kind {
                semantic_vocabulary::ExternalCompletionKind::ProcessExit => bytes.u8(1),
            }
            encode_ids(bytes, arguments);
            bytes.slice(structural_arguments, encode_structural_argument);
            encode_ids(bytes, requirement_obligations);
            encode_ids(bytes, frontier_lower_bound);
        }
        O::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            bytes.u8(47);
            bytes.id(*psi_edge);
            encode_crash_cause(bytes, *cause);
            bytes.slice(site_guard, encode_crash_predicate);
            encode_ids(bytes, frontier_lower_bound);
        }
        _ => unreachable!("operation family routing admitted a non-control operation"),
    }
}

fn encode_successor(bytes: &mut CanonicalBytes, successor: &AbstractSuccessor) {
    bytes.id(successor.psi_edge);
    bytes.id(successor.target);
    bytes.slice(&successor.bindings, encode_binding);
    bytes.slice(&successor.structural_bindings, encode_structural_binding);
    bytes.slice(&successor.cleanup_actions, encode_cleanup);
}
