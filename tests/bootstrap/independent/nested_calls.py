#!/usr/bin/env python3
"""Check nested expression calls through a supplied source-built Omega constructor.

The constructor consumes a closed-source-set packet and returns an Epsilon
observation. Successful tapes run on the audited Alpha seed. No host compiler
supplies Omega meaning in this test.
"""
import argparse
from pathlib import Path
import runpy
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from compiled import execute, stamp

encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]

CASES = (
    ("constant", """machine main() -> u8 { helper() machine helper() -> u8 { 42 } }""", 42),
    ("explicit-argument", """machine main() -> u8 {
        let value: u8 = 41;
        helper(42)
        machine helper(value: u8) -> u8 { value }
    }""", 42),
    ("lexical-before-global", """machine helper() -> u8 { 7 }
        machine main() -> u8 { helper() machine helper() -> u8 { 42 } }
    """, 42),
    ("sibling", """machine main() -> u8 {
        transition { _ -> start() }
        machine start() -> u8 { helper() }
        machine helper() -> u8 { 42 }
    }""", 42),
    ("descendant", """machine main() -> u8 {
        outer()
        machine outer() -> u8 { inner() machine inner() -> u8 { 42 } }
    }""", 42),
    ("helper-in-later-unit", (
        "machine main() -> u8 { later() }",
        "machine later() -> u8 { helper(42) machine helper(value: u8) -> u8 { value } }",
    ), 42),
    ("nested-predicate-effect", """data Counter { changed: bool; }
        machine main() -> u8 {
            let mutable counter: Counter;
            run(@mutable counter)
        }
        machine run(counter: @mutable Counter) -> u8 {
            transition { advance(counter) -> selected(counter.changed), _ -> 7 }
            machine advance(counter: @mutable Counter) -> bool {
                counter.changed = !counter.changed;
                counter.changed
            }
            machine selected(changed: bool) -> u8 {
                transition changed { true -> 42, _ -> 7 }
            }
        }
    """, 42),
    ("continuation", """machine main() -> u8 {
        let first: u8 = helper(41);
        let second: u8 = helper(42);
        first
        machine helper(value: u8) -> u8 { value }
    }""", 41),
    ("callee-tail-cycle", """machine main() -> u8 {
        helper(false)
        machine helper(done: bool) -> u8 { transition done { true -> 42, _ -> step(true) } }
        machine step(done: bool) -> u8 { transition { _ -> helper(done) } }
    }""", 42),
    ("proved-entry", """machine main() -> u8 {
        helper(42)
        machine helper(value: u8) -> u8 requires value > 0 { value }
    }""", 42),
    ("acknowledged-block", "machine main() -> u8 blocks { block helper() machine helper() -> u8 blocks { 42 } }", 42),
    ("shared-loan-forwarding", """data B { bytes: [u8; 2]; }
        machine read(source: @B) -> u8 {
            forward(source)
            machine forward(source: @B) -> u8 { source.bytes[0] }
        }
        machine main() -> u8 { let mutable b: B; b.bytes[0] = 51; read(@b) }
    """, 51),
    ("exclusive-loan-forwarding", """data C { value: u8; }
        machine update(cell: @mutable C) -> u8 {
            let result: u8 = write(cell);
            cell.value
            machine write(cell: @mutable C) -> u8 { cell.value = 42; 0 }
        }
        machine main() -> u8 { let mutable cell: C; update(@mutable cell) }
    """, 42),
    ("tail-cycle-without-suspended-caller", """machine main() -> u8 {
        transition false { true -> repeat(), _ -> 42 }
        machine repeat() -> u8 { transition { _ -> repeat() } }
    }""", 42),
)

REFUSALS = (
    ("argument-type", "machine main() -> u8 { helper(true) machine helper(value: u8) -> u8 { value } }"),
    ("argument-count", "machine main() -> u8 { helper() machine helper(value: u8) -> u8 { value } }"),
    ("scalar-shadow", "machine main() -> u8 { let helper: u8 = 42; helper() machine helper() -> u8 { 41 } }"),
    ("no-runtime-capture", "machine main() -> u8 { let value: u8 = 42; helper() machine helper() -> u8 { value } }"),
    ("private-descendant", """machine main() -> u8 {
        hidden()
        machine owner() -> u8 { hidden() machine hidden() -> u8 { 42 } }
    }"""),
    ("ordinary-self-cycle", "machine main() -> u8 { helper() machine helper() -> u8 { helper() } }"),
    ("unused-ordinary-cycle", "machine main() -> u8 { 42 machine helper() -> u8 { helper() } }"),
    ("ordinary-tail-cycle", """machine main() -> u8 {
        helper()
        machine helper() -> u8 { transition { _ -> next() } }
        machine next() -> u8 { helper() }
    }"""),
    ("unproved-entry", """machine main() -> u8 {
        helper(0)
        machine helper(value: u8) -> u8 requires value > 0 { value }
    }"""),
    ("unacknowledged-block", "machine main() -> u8 { helper() machine helper() -> u8 blocks { 42 } }"),
    ("forbidden-block", "machine main() -> u8 blocks false { block helper() machine helper() -> u8 blocks { 42 } }"),
    ("overlapping-call-arguments", """data C { value: u8; }
        machine victim(cell: @mutable C) -> u8 {
            write(cell, cell.value)
            machine write(cell: @mutable C, value: u8) -> u8 { value }
        }
        machine main() -> u8 { let mutable cell: C; victim(@mutable cell) }
    """),
    ("mutation-invalidates-fact", """data Cell requires length <= 2, { bytes: [u8; 2]; length: u64; }
        machine combine(first: u8, second: u8) -> u8 { second }
        machine victim(cell: @mutable Cell) -> u8 requires cell.length < 2 {
            combine(mutate(cell), cell.bytes[cell.length])
            machine mutate(cell: @mutable Cell) -> u8 { cell.length = 2; 0 }
        }
        machine main() -> u8 { 0 }
    """),
    ("forbidden-trap", "machine main() -> u8 crashes none { helper() machine helper() -> u8 crashes Trap { crash Trap; } }"),
)


REFUSAL_CODES = {
    "argument-type": 1,
    "argument-count": 1,
    "scalar-shadow": 1,
    "no-runtime-capture": 1,
    "private-descendant": 1,
    "ordinary-self-cycle": 2,
    "ordinary-tail-cycle": 2,
    "unused-ordinary-cycle": 2,
    "unproved-entry": 2,
    "unacknowledged-block": 1,
    "forbidden-block": 1,
    "overlapping-call-arguments": 1,
    "mutation-invalidates-fact": 2,
    "forbidden-trap": 2,
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("constructor", type=Path)
    parser.add_argument("output", type=Path, help="new directory for exact packets, observations and tapes")
    parser.add_argument("--seconds", type=int, help="optional limit per invocation")
    args = parser.parse_args()
    directory = args.output.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    producer = args.constructor.resolve(strict=True)
    for name, text, expected in (*CASES, *((name, source, None) for name, source in REFUSALS)):
        texts = text if isinstance(text, tuple) else (text,)
        units = [(0, part.encode("ascii")) for part in texts]
        paths = ["main.omg", *(f"part{index}.omg" for index in range(1, len(units)))]
        packet = encode([("app", 0)], [], units, 0, b"main", paths=paths)[0]
        (directory / (name + ".bin")).write_bytes(packet)
        result = subprocess.run([producer], input=packet, capture_output=True, timeout=args.seconds)
        observation = result.stdout
        (directory / (name + ".eeout")).write_bytes(observation)
        assert result.returncode == 0 and result.stderr == b"", (name, result.returncode, result.stderr)
        assert len(observation) >= 5 and observation[0] == 0, (name, observation[:40])
        status = struct.unpack_from("<i", observation, 1)[0]
        if expected is None:
            assert status == REFUSAL_CODES[name] and len(observation) == 9, (name, observation[:40])
        else:
            assert status == 0 and len(observation) > 5, (name, observation[:40])
            tape = directory / (name + ".tape")
            tape.write_bytes(observation[5:])
            image = directory / name
            stamp(tape, image)
            assert execute(image, timeout=args.seconds) == (expected, b""), name
        print(f"{name}: passed", flush=True)
    print(f"nested calls: {len(CASES)} executed products and {len(REFUSALS)} refusals")


if __name__ == "__main__":
    main()
