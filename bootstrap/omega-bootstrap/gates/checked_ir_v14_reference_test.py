#!/usr/bin/env python3
"""Focused tests for CKIR14 independent validation and execution meaning."""

from __future__ import annotations

import unittest

import checked_ir_v5_reference as v5
import checked_ir_v12_reference as v12
import checked_ir_v14_reference as v14
import checked_ir_v14_test_support as support


class Ckir14ReferenceTests(unittest.TestCase):
    def test_nested_full_width_arithmetic_and_high_bits(self) -> None:
        module = v14.decode(support.arithmetic())
        self.assertEqual(v14.selected_counts(module), {8: 1, 26: 1, 27: 1})
        self.assertEqual(v14.interpret(module), 70)

    def test_distinct_high_bit_range_rows_survive_legacy_projection(self) -> None:
        tables = support.arithmetic_tables()
        tables["types"].append(
            (2, 2, 1, 0, 0, 0, 0x8000_0000, 0xFFFF_FFFE)
        )
        module = v14.decode(support.encode(tables, values=7))
        self.assertEqual(module.tables["types"][2][6:],
                         (0x8000_0000, 0xFFFF_FFFE))

    def test_node_level_add_overflow_traps(self) -> None:
        module = v14.decode(support.arithmetic("add-overflow"))
        with self.assertRaisesRegex(v5.Ckir5Error, "runtime add range"):
            v14.interpret(module)

    def test_node_level_subtract_underflow_traps(self) -> None:
        module = v14.decode(support.arithmetic("subtract-underflow"))
        with self.assertRaisesRegex(v5.Ckir5Error, "runtime subtract range"):
            v14.interpret(module)

    def test_node_level_multiply_overflow_traps(self) -> None:
        module = v14.decode(support.arithmetic("multiply-overflow"))
        with self.assertRaisesRegex(v5.Ckir5Error, "runtime multiply range"):
            v14.interpret(module)

    def test_each_selected_opcode_is_sufficient(self) -> None:
        cases = ((8, 0xFFFF_FFFE, 1, 0xFFFF_FFFF),
                 (26, 0xFFFF_FFFF, 5, 0xFFFF_FFFA),
                 (27, 65_535, 65_537, 0xFFFF_FFFF))
        for opcode, left, right, expected in cases:
            with self.subTest(opcode=opcode):
                module = v14.decode(support.single_arithmetic(opcode, left, right))
                self.assertEqual(v14.interpret(module), expected)

    def test_contextual_high_word_const_immediates_are_exact(self) -> None:
        for value in (0x8000_0000, 0xFFFF_FFFE):
            with self.subTest(value=value):
                module = v14.decode(support.single_arithmetic(8, value, 0))
                immediates = [
                    operation[10] for operation in module.tables["operations"]
                    if operation[3] == 1
                ]
                self.assertIn(value, immediates)
                self.assertEqual(v14.interpret(module), value)

    def test_requires_at_least_one_selected_arithmetic_opcode(self) -> None:
        with self.assertRaisesRegex(v5.Ckir5Error, "requires selected full-width"):
            v14.decode(support.view_only())

    def test_block_parameter_is_an_admitted_typed_leaf(self) -> None:
        module = v14.decode(support.parameter_arithmetic())
        self.assertEqual(v14.selected_counts(module), {8: 1, 26: 0, 27: 0})
        self.assertEqual(v14.interpret(module), 70)

    def test_exact_integer_widen_is_an_admitted_arithmetic_leaf(self) -> None:
        module = v14.decode(support.widen_arithmetic())
        self.assertEqual(v14.selected_counts(module), {8: 1, 26: 0, 27: 0})
        self.assertEqual(v14.interpret(module), 70)

    def test_integer_widen_rejects_the_frozen_ckir10_target_in_ckir14(self) -> None:
        tables = support.widen_arithmetic_tables()
        tables["types"].append((3, 2, 1, 0, 0, 0, 0, 0x7FFF_FFFF))
        tables["operations"][1] = support.replace(tables["operations"][1], 7, 3)
        with self.assertRaisesRegex(v5.Ckir5Error,
                                    "IntegerWiden canonical u32 Trapping result"):
            v14.decode(support.encode(tables, values=4))

    def test_call_result_is_not_an_arithmetic_leaf(self) -> None:
        with self.assertRaisesRegex(v5.Ckir5Error,
                                    "full-width arithmetic leaf custody"):
            v14.decode(support.call_custody_violation())

    def test_kind7_type_selects_complete_inherited_view_family(self) -> None:
        tables = support.arithmetic_tables()
        tables["types"].extend([
            (2, 1, 0, 0, 0, 0, 0, 255),
            (3, 7, 0, 0, 2, 0, 0, 0),
        ])
        with self.assertRaisesRegex(v5.Ckir5Error, "partial byte-view relation"):
            v14.decode(support.encode(tables, values=7))

    def test_synthetic_block_flag_selects_complete_inherited_view_family(self) -> None:
        tables = support.arithmetic_tables()
        tables["blocks"][0] = support.replace(tables["blocks"][0], 3, 1)
        with self.assertRaisesRegex(v5.Ckir5Error, "partial byte-view relation"):
            v14.decode(support.encode(tables, values=7))

    def test_rejects_noncanonical_subtract_carrier(self) -> None:
        tables = support.arithmetic_tables()
        tables["types"][1] = support.replace(tables["types"][1], 2, 0)
        with self.assertRaisesRegex(v5.Ckir5Error, "canonical full-width"):
            v14.decode(support.encode(tables, values=7))

    def test_composes_inherited_ckir12_views_with_arithmetic(self) -> None:
        module = v14.decode(support.composed_view_and_arithmetic())
        self.assertEqual(v12.selected_counts(module), {22: 1, 23: 2, 24: 1, 25: 1})
        self.assertEqual(v14.selected_counts(module), {8: 1, 26: 1, 27: 1})
        self.assertEqual(v14.interpret(module), 70)

    def test_rejects_partial_inherited_view_family(self) -> None:
        tables = support.composed_view_and_arithmetic_tables()
        tables["operations"][3] = support.replace(tables["operations"][3], 3, 24)
        with self.assertRaises(v5.Ckir5Error):
            v14.decode(support.encode(tables, values=18))

    def test_ckir14_does_not_require_views(self) -> None:
        self.assertEqual(v14.interpret(v14.decode(support.arithmetic())), 70)

    def test_old_major_cannot_acquire_ckir14_capabilities(self) -> None:
        with self.assertRaisesRegex(v5.Ckir5Error, "bad CKIR schema"):
            v14.decode(support.encode(support.arithmetic_tables(), values=7, major=12))

        with self.assertRaises(v5.Ckir5Error):
            v12.decode(support.encode(support.arithmetic_tables(), values=7, major=12))


if __name__ == "__main__":
    unittest.main()
