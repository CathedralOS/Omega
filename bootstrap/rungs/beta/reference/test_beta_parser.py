#!/usr/bin/env python3
"""Focused ownership and grammar checks for Beta's executable reference."""

import ast
from pathlib import Path
import sys
import subprocess
import unittest

import beta_parser


class BetaParserTests(unittest.TestCase):
    def test_reference_import_does_not_load_backend_or_refinement(self):
        completed = subprocess.run(
            [
                sys.executable,
                '-c',
                'import beta_parser, beta_interp, sys; '
                'assert "bc2" not in sys.modules; '
                'assert "beta_symbolic" not in sys.modules',
            ],
            cwd=__file__.rsplit('/', 1)[0],
            check=False,
        )
        self.assertEqual(completed.returncode, 0)

    def test_complete_reference_surface_has_stable_tuple_ast(self):
        source = r'''
            proc helper(a, b) { return word[a] + byte[b] }
            proc main() {
                let x = 3
                word[x] = 9
                byte[x] = '\n'
                emit("ok\\n")
                helper(x, 1)
                state loop { to done when (x >= 3) }
                state done { return helper(x, 1) }
            }
        '''
        ast = beta_parser.Parser(beta_parser.lex(source)).parse()
        self.assertEqual([proc[1] for proc in ast], ['helper', 'main'])
        body = ast[1][3]
        self.assertEqual(body[0], ('let', 'x', ('num', 3)))
        self.assertEqual(body[1], ('memset', 'word', ('var', 'x'), ('num', 9)))
        self.assertEqual(body[2], ('memset', 'byte', ('var', 'x'), ('num', 10)))
        self.assertEqual(body[3], ('emit', r'ok\\n'))
        self.assertEqual(body[4], ('callstmt', ('call', 'helper', [('var', 'x'), ('num', 1)])))
        self.assertEqual(
            body[5],
            ('state', 'loop', [('goto', 'done', ('bin', '>=', ('var', 'x'), ('num', 3)))])
        )

    def test_reference_sources_have_no_backend_or_refinement_import(self):
        here = Path(__file__).resolve().parent
        forbidden = {'bc2', 'beta_symbolic', 'symbolic_loop_check'}
        for name in ('beta_parser.py', 'beta_interp.py', 'io-verify.py'):
            tree = ast.parse((here / name).read_text(), filename=name)
            imported = {
                alias.name.split('.')[0]
                for node in ast.walk(tree)
                if isinstance(node, (ast.Import, ast.ImportFrom))
                for alias in node.names
            }
            self.assertTrue(forbidden.isdisjoint(imported), (name, imported))

    def test_retired_second_compiler_is_absent(self):
        root = Path(__file__).resolve().parents[4]
        facade = root / 'compiler/beta-lang-py'
        self.assertFalse((facade / 'bc2.py').exists())
        self.assertFalse((facade / 'independent-floor.sh').exists())

    def test_legacy_facade_contains_forwarders_only(self):
        root = Path(__file__).resolve().parents[4]
        facade = root / 'compiler/beta-lang-py'
        expected = {
            'README.md',
            'beta-correctness-fuzz.sh',
            'beta-fuzz-gen.py',
            'beta-io-exhaust.sh',
            'beta_interp.py',
            'beta_parser.py',
            'beta_symbolic.py',
            'io-fuzz-gen.py',
            'io-verify.py',
            'symbolic-loops.sh',
            'symbolic_loop_check.py',
            'test_beta_parser.py',
        }
        files = {path.name for path in facade.iterdir() if path.is_file()}
        self.assertEqual(files, expected)
        for path in facade.iterdir():
            if path.is_file() and path.name != 'README.md':
                self.assertIn('Compatibility', path.read_text())


if __name__ == '__main__':
    unittest.main()
