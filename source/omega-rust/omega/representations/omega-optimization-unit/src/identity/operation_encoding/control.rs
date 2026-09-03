//! Control transfer, return, cleanup, and crash tags.

use super::*;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => {
            bytes.u8(42);
            bytes.id(*psi_edge);
            bytes.id(*target);
            bytes.slice(bindings, encode_binding);
            encode_ids(bytes, trivial_affine_discards);
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
        O::StructuralCase { source, cases } => {
            bytes.u8(48);
            bytes.id(*source);
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
                encode_ids(bytes, &case.trivial_affine_discards);
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
            trivial_affine_discards,
        } => {
            bytes.u8(46);
            bytes.id(*psi_edge);
            bytes.id(*source);
            encode_ids(bytes, returned_claims);
            bytes.len(trivial_affine_locals.len());
            for (operation, place, structural_type) in trivial_affine_locals {
                bytes.id(*operation);
                encode_place_declaration(bytes, *place);
                encode_structural_type(bytes, structural_type);
            }
            encode_ids(bytes, trivial_affine_discards);
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
    encode_ids(bytes, &successor.trivial_affine_discards);
}
