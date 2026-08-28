#!/usr/bin/env python3
"""Ownership checks for untrusted Beta refinement reconstruction."""

import ast
import os
from pathlib import Path
import sys
import unittest


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
WITNESSES = HERE.parent / 'admission/witnesses'
REFERENCE = Path(
    os.environ.get(
        'OMEGA_PATH_BETA_REFERENCE',
        ROOT / 'source/beta/reference',
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
        self.assertEqual((HERE / 'alpha_symbolic.py').resolve().parent, HERE)
        self.assertEqual((HERE / 'alpha_refinement_check.py').resolve().parent, HERE)

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

    def test_block_control_mapper_is_only_an_untrusted_witness_builder(self):
        tree = ast.parse((WITNESSES / 'bc_block_control_map.py').read_text())
        imported = {
            alias.name.split('.')[0]
            for node in ast.walk(tree)
            if isinstance(node, (ast.Import, ast.ImportFrom))
            for alias in node.names
        }
        self.assertNotIn('bc2', imported)
        self.assertNotIn('beta_backend', imported)
        self.assertNotIn('subprocess', imported)
        self.assertFalse(any(
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in {'exec', 'eval'}
            for node in ast.walk(tree)
        ))

    def test_retired_compiler_facade_is_absent(self):
        self.assertFalse((ROOT / 'compiler/beta-lang-py').exists())

    def test_alpha_has_no_validation_compatibility_aliases(self):
        alpha = ROOT / 'source/alpha'
        for entry in (
            'REFINEMENT.md',
            'alpha_refinement_check.py',
            'alpha_symbolic.py',
            'refinement.sh',
            'refinement-cert-diamond.sh',
            'refinement-samples',
        ):
            self.assertFalse((alpha / entry).exists(), entry)

        self.assertTrue((alpha / 'SEMANTICS.md').is_file())
        self.assertTrue((alpha / 'alpha_ref.py').is_file())
        self.assertFalse((HERE / 'SEMANTICS.md').exists())
        self.assertFalse((HERE / 'alpha_ref.py').exists())


if __name__ == '__main__':
    unittest.main()
