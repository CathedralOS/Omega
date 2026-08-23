#!/usr/bin/env python3
"""Ownership checks for untrusted Beta refinement reconstruction."""

import ast
import os
from pathlib import Path
import sys
import unittest


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
REFERENCE = Path(
    os.environ.get(
        'OMEGA_PATH_BETA_REFERENCE',
        ROOT / 'bootstrap/rungs/beta/reference',
    )
).resolve()
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REFERENCE))

import beta_parser
import beta_symbolic


class BetaRefinementOwnershipTests(unittest.TestCase):
    def test_reconstruction_and_parser_have_distinct_canonical_owners(self):
        self.assertEqual(Path(beta_symbolic.__file__).resolve().parent, HERE)
        self.assertEqual(Path(beta_parser.__file__).resolve().parent, REFERENCE)

    def test_reconstruction_does_not_import_a_compiler_backend(self):
        tree = ast.parse((HERE / 'beta_symbolic.py').read_text())
        imported = {
            alias.name.split('.')[0]
            for node in ast.walk(tree)
            if isinstance(node, (ast.Import, ast.ImportFrom))
            for alias in node.names
        }
        self.assertNotIn('bc2', imported)
        self.assertNotIn('beta_backend', imported)
        self.assertNotIn('bc', sys.modules)
        self.assertNotIn('bc2', sys.modules)

    def test_legacy_facade_contains_no_second_compiler(self):
        facade = ROOT / 'compiler/beta-lang-py'
        self.assertFalse((facade / 'bc2.py').exists())
        self.assertFalse((facade / 'independent-floor.sh').exists())


if __name__ == '__main__':
    unittest.main()
