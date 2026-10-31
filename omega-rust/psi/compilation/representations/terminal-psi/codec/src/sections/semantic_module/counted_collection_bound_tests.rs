//! The counted-collection read contract at the public wire entrances: a count
//! the remaining input cannot hold is refused before any of its elements are
//! read, so no roster is ever sized from an unvalidated count. Wire-format
//! units only; language behavior stays in `tests/omega`.

use semantic_vocabulary::Proposition;
use terminal_psi::{CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

use super::wire::{Reader, bounded_count, decode_bytes};
use super::{CodecError, decode_module, encode_module};
use crate::{decode_crash_route_buckets, encode_crash_route_buckets};

use super::target_family_tests::unit_machine;

#[test]
fn bounded_count_refuses_a_count_the_remaining_input_cannot_hold() {
    // One byte remains behind a declared count of two: the second element
    // cannot exist, so the read is truncation, refused before a caller
    // reserves capacity for it.
    let mut reader = Reader::new(&[2, 0, 0, 0, 0x7F]);
    assert_eq!(
        bounded_count(&mut reader, 1),
        Err(CodecError::UnexpectedEnd)
    );

    // A family's own encoded minimum tightens the same refusal: two four-byte
    // elements fit in eight remaining bytes, three cannot.
    let mut reader = Reader::new(&[2, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(bounded_count(&mut reader, 4), Ok(2));
    let mut reader = Reader::new(&[3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        bounded_count(&mut reader, 4),
        Err(CodecError::UnexpectedEnd)
    );

    // A zero count reads with no payload and stays outside the bound.
    let mut reader = Reader::new(&[0, 0, 0, 0]);
    assert_eq!(bounded_count(&mut reader, 1), Ok(0));
}

#[test]
fn over_declared_crash_route_counts_are_refused_before_their_elements() {
    // Two buckets declared behind one byte. That byte is not a bucket cause,
    // so a decoder that reads its elements first reports the stray byte's
    // tag; the counted-collection bound refuses the collection as truncation
    // before any element is read.
    assert_eq!(
        decode_crash_route_buckets(&[2, 0, 0, 0, 0x09]),
        Err(CodecError::UnexpectedEnd)
    );
    // A count no input could hold is the same refusal; it never reaches a
    // reservation sized from the count.
    assert_eq!(
        decode_crash_route_buckets(&[0xFF, 0xFF, 0xFF, 0xFF, 0x09]),
        Err(CodecError::UnexpectedEnd)
    );
}

#[test]
fn valid_crash_route_rosters_round_trip() {
    let routes = vec![
        CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        },
        CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                Proposition::Truth,
            ))],
        },
    ];
    let bytes = encode_crash_route_buckets(&routes).unwrap();
    assert_eq!(decode_crash_route_buckets(&bytes).unwrap(), routes);
}

#[test]
fn over_declared_string_rosters_and_byte_payloads_are_refused() {
    // Three strings declared behind one byte. Every string carries a
    // four-byte length prefix, so the roster is refused before it is sized
    // from the count.
    let mut reader = Reader::new(&[3, 0, 0, 0, 0x2A]);
    assert_eq!(
        reader.strings("test roster"),
        Err(CodecError::UnexpectedEnd)
    );

    // A byte payload whose length exceeds the remaining input is the same
    // refusal: the length and the count share the one bound.
    let mut reader = Reader::new(&[9, 0, 0, 0, 1, 2]);
    assert_eq!(decode_bytes(&mut reader), Err(CodecError::UnexpectedEnd));
    let mut reader = Reader::new(&[2, 0, 0, 0, 1, 2]);
    assert_eq!(decode_bytes(&mut reader).unwrap(), vec![1, 2]);
}

#[test]
fn every_proper_prefix_of_a_canonical_module_is_refused() {
    let machine = semantic_vocabulary::MachineId::new(1).unwrap();
    let mut module = terminal_psi::TerminalModule::for_entry(machine);
    module.machines.push(unit_machine(machine));
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    for length in 0..bytes.len() {
        assert!(
            decode_module(&bytes[..length]).is_err(),
            "a module prefix of {length} bytes must not decode"
        );
    }
}
