#!/usr/bin/env python3
"""Focused regressions for the independent OMGRFN16 Python owners."""

from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "source/on-ramp/omega-bootstrap/gates"
CASES = GATES / "fixtures/ckir14-arithmetic-cases"
sys.path[:0] = [str(HERE), str(GATES)]

import checked_ir_v14_test_support as support  # noqa: E402
from omgrfn16_ckir import check_expression_join, decode  # noqa: E402
from omgrfn16_frame import RefinementError, RefinementResourceError  # noqa: E402
from omgrfn16_source import execute, selected_run, witness_leaf_names  # noqa: E402


class SourceOwnerTests(unittest.TestCase):
    def source(self, name: str) -> bytes:
        return (CASES / f"{name}.omg").read_bytes()

    def test_representative_contexts_are_independently_selected(self) -> None:
        source = self.source("representative-contexts")
        program = selected_run(source)
        self.assertEqual(
            tuple(site.context for site in program.sites),
            ("assignment", "guard", "call-argument", "transition-argument"),
        )
        self.assertEqual(execute(program), 70)

        transition = b"receive(prefix, (self.byte0 - 192) * 64 + (self.byte1 - 128), 9)"
        changed = source.replace(transition, transition[:-2] + b"8)", 1)
        self.assertEqual(len(changed), len(source))
        self.assertEqual(execute(selected_run(changed)), 0)

    def test_expression_depth_boundary(self) -> None:
        self.assertEqual(execute(selected_run(self.source("depth-8-boundary"))), 70)
        with self.assertRaisesRegex(RefinementResourceError, "expression-depth"):
            selected_run(self.source("depth-9-exhausted"))

    def test_view_source_observations(self) -> None:
        program = selected_run(self.source("ckir12-view-plus-arithmetic"))
        self.assertEqual(program.view_literals, (b"F",))
        self.assertEqual(
            (program.view_nonempty, program.view_heads, program.view_tails),
            (1, 1, 1),
        )
        self.assertEqual(execute(program), 70)

    def test_malformed_witness_name_input_is_an_owner_error(self) -> None:
        with self.assertRaisesRegex(RefinementError, "OMGRSW7 leaf-name framing"):
            witness_leaf_names(b"", b"OMGRSW6\0")


class LoweringOwnerTests(unittest.TestCase):
    SOURCE = b"""data Probe { result: u32 in Trapping; }
machine Probe::run(&mut self, value: u32 in Trapping) -> u8 {
    self.result = value + 69;
    transition self.result == 69 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
    NAMES = {
        "fields": {}, "machine_parameters": {},
        "block_parameters": {0: b"value"},
    }

    def test_exact_parameter_name_and_operand_order_join(self) -> None:
        program = selected_run(self.SOURCE)
        module = decode(support.parameter_arithmetic())
        check_expression_join(module, program, self.NAMES)

        wrong_name = copy.deepcopy(self.NAMES)
        wrong_name["block_parameters"][0] = b"other"
        with self.assertRaises(RefinementError):
            check_expression_join(module, program, wrong_name)

        tables = copy.deepcopy(module.tables)
        tables["operands"][0], tables["operands"][1] = (
            tables["operands"][1], tables["operands"][0]
        )
        swapped = decode(support.encode(tables, values=4))
        with self.assertRaises(RefinementError):
            check_expression_join(swapped, program, self.NAMES)


if __name__ == "__main__":
    unittest.main()
