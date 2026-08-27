#!/usr/bin/env python3
"""Derive bounded scalar-call backend cases from the committed v28 fixture.

This helper is test plumbing, not a Terminal-Psi authority. It pins the exact
published fixture before applying local mutations and uses a small explicit
wire builder only for structurally complete capacity teeth.
"""

from __future__ import annotations

import pathlib
import struct
import sys


def u16(value: int) -> bytes:
    return struct.pack("<H", value)


def u32(value: int) -> bytes:
    return struct.pack("<I", value)


def u64(value: int) -> bytes:
    return struct.pack("<Q", value)


I32 = bytes((2, 1)) + u16(32)


def declaration(value_id: int) -> bytes:
    return u64(value_id) + I32


def constant(operation_id: int, result_id: int, value: int) -> bytes:
    return (
        u64(operation_id)
        + bytes((1,))
        + declaration(result_id)
        + bytes((1, 1))
        + int(value).to_bytes(16, "little", signed=True)
    )


def call(
    operation_id: int, result_id: int, callee: int, arguments: list[int]
) -> bytes:
    return (
        u64(operation_id)
        + bytes((1,))
        + declaration(result_id)
        + bytes((33,))
        + u64(callee)
        + u32(len(arguments))
        + b"".join(u64(argument) for argument in arguments)
        + u32(0)
        + u32(0)
    )


def machine(
    machine_id: int,
    parameters: list[int],
    result_declaration: int,
    operations: list[bytes],
    return_value: int,
) -> bytes:
    return (
        u64(machine_id)
        + bytes((0,))
        + u32(len(parameters))
        + b"".join(declaration(parameter) for parameter in parameters)
        + u32(0)
        + bytes((1,))
        + declaration(result_declaration)
        + u32(0) * 6
        + u64(machine_id)
        + u32(1)
        + u64(machine_id)
        + u32(0)
        + u32(len(operations))
        + b"".join(operations)
        + bytes((2,))
        + u64(machine_id)
        + u64(return_value)
        + u32(0)
        + u64(machine_id)
        + u32(0) * 3
    )


def module(entry: int, machines: list[bytes]) -> bytes:
    return (
        b"PSITERM\0"
        + u16(26)
        + u16(28)
        + u64(entry)
        + u32(0) * 15
        + u32(len(machines))
        + b"".join(machines)
    )


class Ids:
    def __init__(self) -> None:
        self.value = 1
        self.operation = 1

    def next_value(self) -> int:
        value = self.value
        self.value += 1
        return value

    def next_operation(self) -> int:
        operation = self.operation
        self.operation += 1
        return operation


def complete_machine_count_17() -> bytes:
    ids = Ids()
    machines = []
    for machine_id in range(1, 18):
        result = ids.next_value()
        operation = constant(ids.next_operation(), result, machine_id)
        declaration_id = ids.next_value()
        machines.append(machine(machine_id, [], declaration_id, [operation], result))
    return module(1, machines)


def complete_machine_count(count: int) -> bytes:
    ids = Ids()
    machines = []
    for machine_id in range(1, count + 1):
        result = ids.next_value()
        operation = constant(ids.next_operation(), result, machine_id)
        declaration_id = ids.next_value()
        machines.append(machine(machine_id, [], declaration_id, [operation], result))
    return module(1, machines)


def complete_parameter_count_5() -> bytes:
    ids = Ids()
    parameters = [ids.next_value() for _ in range(5)]
    declaration_id = ids.next_value()
    return module(1, [machine(1, parameters, declaration_id, [], parameters[0])])


def complete_operation_count_17() -> bytes:
    ids = Ids()
    operations = []
    result = 0
    for value in range(17):
        result = ids.next_value()
        operations.append(constant(ids.next_operation(), result, value))
    declaration_id = ids.next_value()
    return module(1, [machine(1, [], declaration_id, operations, result)])


def complete_argument_count_5() -> bytes:
    ids = Ids()
    arguments = []
    caller_operations = []
    for value in range(5):
        result = ids.next_value()
        arguments.append(result)
        caller_operations.append(constant(ids.next_operation(), result, value))
    call_result = ids.next_value()
    caller_operations.append(call(ids.next_operation(), call_result, 2, arguments))
    caller_declaration = ids.next_value()
    parameters = [ids.next_value() for _ in range(5)]
    callee_declaration = ids.next_value()
    return module(
        1,
        [
            machine(1, [], caller_declaration, caller_operations, call_result),
            machine(2, parameters, callee_declaration, [], parameters[0]),
        ],
    )


def complete_argument_count(count: int) -> bytes:
    ids = Ids()
    arguments = []
    caller_operations = []
    for value in range(count):
        result = ids.next_value()
        arguments.append(result)
        caller_operations.append(constant(ids.next_operation(), result, value))
    call_result = ids.next_value()
    caller_operations.append(call(ids.next_operation(), call_result, 2, arguments))
    caller_declaration = ids.next_value()
    parameters = [ids.next_value() for _ in range(count)]
    callee_declaration = ids.next_value()
    return module(
        1,
        [
            machine(1, [], caller_declaration, caller_operations, call_result),
            machine(2, parameters, callee_declaration, [], parameters[0]),
        ],
    )


def complete_operation_count(count: int) -> bytes:
    ids = Ids()
    operations = []
    result = 0
    for value in range(count):
        result = ids.next_value()
        operations.append(constant(ids.next_operation(), result, value))
    declaration_id = ids.next_value()
    return module(1, [machine(1, [], declaration_id, operations, result)])


def complete_code_calls_per_machine(calls_per_machine: int) -> bytes:
    ids = Ids()
    machines = []
    for machine_id in range(1, 16):
        arguments = []
        operations = []
        for value in range(4):
            result = ids.next_value()
            arguments.append(result)
            operations.append(constant(ids.next_operation(), result, value))
        result = arguments[0]
        for _ in range(calls_per_machine):
            result = ids.next_value()
            operations.append(call(ids.next_operation(), result, 16, arguments))
        declaration_id = ids.next_value()
        machines.append(machine(machine_id, [], declaration_id, operations, result))
    parameters = [ids.next_value() for _ in range(4)]
    declaration_id = ids.next_value()
    machines.append(machine(16, parameters, declaration_id, [], parameters[0]))
    return module(1, machines)


def complete_code_overflow() -> bytes:
    # With fifteen callers, five calls per caller yields 3,629 text bytes;
    # the sixth is the adjacent reachable 4,096-byte code-ceiling refusal.
    return complete_code_calls_per_machine(6)


def write(path: pathlib.Path, contents: bytes) -> None:
    path.write_bytes(contents)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: scalar-call-terminal-cases.py FIXTURE.hex OUT_DIR")
    fixture_path = pathlib.Path(sys.argv[1])
    output = pathlib.Path(sys.argv[2])
    accepted = output / "accepted"
    accepted_boundary = output / "accepted-boundary"
    malformed = output / "reject-251"
    exhausted = output / "reject-252"
    for directory in (accepted, accepted_boundary, malformed, exhausted):
        directory.mkdir(parents=True, exist_ok=True)

    source = bytes.fromhex(fixture_path.read_text(encoding="ascii"))
    assert len(source) == 431
    assert source[:8] == b"PSITERM\0"
    assert int.from_bytes(source[8:10], "little") == 26
    assert int.from_bytes(source[10:12], "little") == 28
    call_anchor = bytes.fromhex(
        "0200000000000000"  # OperationId 2
        "01" "0200000000000000" "02012000"  # scalar i32 ValueId 2
        "21" "0200000000000000"  # Call machine 2
        "01000000" "0100000000000000"  # one ValueId 1 argument
        "00000000" "00000000"  # empty requirements/crash routes
    )
    assert len(call_anchor) == 50
    assert source.count(call_anchor) == 1
    assert source.index(call_anchor) == 205

    write(accepted / "canonical.psi", source)
    write(accepted_boundary / "machine-count-16.psi", complete_machine_count(16))
    write(accepted_boundary / "parameter-and-argument-count-4.psi", complete_argument_count(4))
    write(accepted_boundary / "operation-count-16.psi", complete_operation_count(16))
    write(accepted_boundary / "code-near-limit.psi", complete_code_calls_per_machine(5))

    permuted = source[:0x54] + source[0x128:0x1AF] + source[0x54:0x128]
    assert len(permuted) == len(source)
    write(accepted / "machine-order-permutation.psi", permuted)

    renamed = bytearray(source)
    renamed[0x00C:0x014] = u64(11)
    renamed[0x054:0x05C] = u64(11)
    renamed[0x0E3:0x0EB] = u64(22)
    renamed[0x128:0x130] = u64(22)
    write(accepted / "arbitrary-machine-ids.psi", renamed)

    fixed_mutations = {
        "unknown-callee": (0x0E3, 3),
        "undefined-argument": (0x0EF, 99),
        "duplicate-operation-id": (0x0CD, 1),
        "duplicate-result-value-id": (0x0D6, 1),
        "unknown-entry": (0x00C, 3),
        "zero-machine-id": (0x054, 0),
        "wrong-call-result-width": (0x0E0, 16),
        "unsupported-operation-tag": (0x0E2, 32),
        "wrong-vocabulary": (0x00A, 27),
    }
    for name, (offset, byte) in fixed_mutations.items():
        mutated = bytearray(source)
        mutated[offset] = byte
        write(malformed / f"{name}.psi", mutated)

    high_id = bytearray(source)
    high_id[0x058] = 1
    write(malformed / "nonzero-machine-id-high-half.psi", high_id)
    write(malformed / "truncated.psi", source[:-1])
    write(malformed / "trailing.psi", source + b"\0")

    wrong_arity = bytearray(source)
    wrong_arity[0x0EB:0x0EF] = u32(0)
    del wrong_arity[0x0EF:0x0F7]
    write(malformed / "wrong-arity.psi", wrong_arity)

    parameterized_entry = Ids()
    entry_parameter = parameterized_entry.next_value()
    entry_result = parameterized_entry.next_value()
    write(
        malformed / "parameterized-entry.psi",
        module(1, [machine(1, [entry_parameter], entry_result, [], entry_parameter)]),
    )

    later_argument = bytearray(source)
    later_argument[0x0EF:0x0F7] = u64(2)
    write(malformed / "later-existing-argument.psi", later_argument)

    unknown_return = bytearray(source)
    unknown_return[0x108:0x110] = u64(99)
    write(malformed / "unknown-return-value.psi", unknown_return)

    duplicate_parameter = bytearray(source)
    duplicate_parameter[0x135:0x13D] = u64(1)
    write(malformed / "duplicate-parameter-value-id.psi", duplicate_parameter)

    wrong_parameter_type = bytearray(source)
    wrong_parameter_type[0x13F] = 16
    write(malformed / "wrong-parameter-width.psi", wrong_parameter_type)

    wrong_machine_result_type = bytearray(source)
    wrong_machine_result_type[0x070] = 16
    write(malformed / "wrong-machine-result-width.psi", wrong_machine_result_type)

    for name, offset in (
        ("duplicate-block-id", 0x176),
        ("duplicate-edge-id", 0x187),
        ("duplicate-contract-id", 0x19B),
    ):
        mutated = bytearray(source)
        mutated[offset : offset + 8] = u64(1)
        write(malformed / f"{name}.psi", mutated)

    cycle_operation = bytes.fromhex(
        "0300000000000000" "01" "0600000000000000" "02012000" "21"
        "0200000000000000" "01000000" "0400000000000000"
        "00000000" "00000000"
    )
    self_cycle = bytearray(source)
    self_cycle[0x182:0x186] = u32(1)
    self_cycle[0x186:0x186] = cycle_operation
    write(malformed / "self-cycle.psi", self_cycle)

    mutual_cycle_operation = bytearray(cycle_operation)
    mutual_cycle_operation[22:30] = u64(1)
    mutual_cycle = bytearray(source)
    mutual_cycle[0x182:0x186] = u32(1)
    mutual_cycle[0x186:0x186] = mutual_cycle_operation
    write(malformed / "mutual-cycle.psi", mutual_cycle)

    write(exhausted / "machine-count-17.psi", complete_machine_count_17())
    write(exhausted / "parameter-count-5.psi", complete_parameter_count_5())
    write(exhausted / "operation-count-17.psi", complete_operation_count_17())
    write(exhausted / "argument-count-5.psi", complete_argument_count_5())
    write(exhausted / "text-code-overflow.psi", complete_code_overflow())


if __name__ == "__main__":
    main()
