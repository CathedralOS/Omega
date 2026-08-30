#!/usr/bin/env python3
"""Focused ownership and grammar checks for Beta's executable reference."""

import ast
from pathlib import Path
import sys
import subprocess
import unittest

import beta_parser
import beta_interp


class BetaParserTests(unittest.TestCase):
    def parse(self, source):
        return beta_parser.Parser(beta_parser.lex(source)).parse()

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
                state loop { to done when x >= 3 }
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

    def test_guard_parentheses_are_optional(self):
        bare = 'proc main() { state start { to done when 1 + 2 == 3 } state done { return 0 } }'
        grouped = 'proc main() { state start { to done when (1 + 2 == 3) } state done { return 0 } }'
        bare_ast = beta_parser.Parser(beta_parser.lex(bare)).parse()
        grouped_ast = beta_parser.Parser(beta_parser.lex(grouped)).parse()
        self.assertEqual(bare_ast, grouped_ast)

    def test_lists_comparisons_and_literals_follow_the_canonical_grammar(self):
        invalid = [
            'proc f(a b) { return a } proc main() { return f(1, 2) }',
            'proc f(a,) { return a } proc main() { return f(1) }',
            'proc f(a, b) { return a } proc main() { return f(1 2) }',
            'proc f(a) { return a } proc main() { return f(1,) }',
            'proc main() { return 1 < 2 < 3 }',
            r'proc main() { emit("bad\x") return 0 }',
            r'''proc main() { emit("bad\'") return 0 }''',
            r'''proc main() { return '\"' }''',
        ]
        for source in invalid:
            with self.subTest(source=source):
                with self.assertRaises(SyntaxError):
                    self.parse(source)

    def test_source_envelope_is_byte_exact_before_tokenization(self):
        valid = b'; comment\rproc main() {\treturn \'A\'\r}\n'
        self.assertEqual(self.parse(valid)[0][1], 'main')
        invalid = [
            (b'; hidden\x00\nproc main() { return 0 }', 8),
            (b'proc\vmain() { return 0 }', 4),
            (b'; hidden\x7f\nproc main() { return 0 }', 8),
            (b'; hidden\xc3\xa9\nproc main() { return 0 }', 8),
        ]
        for source, offset in invalid:
            with self.subTest(source=source):
                with self.assertRaisesRegex(
                    SyntaxError, f'invalid source byte at offset {offset}'
                ):
                    self.parse(source)

    def test_recursive_states_preserve_dfs_tuple_shape_and_fallthrough(self):
        source = '''
            proc main() {
                state outer {
                    state child { let x = 7 }
                }
                state next { return x }
            }
        '''
        procs = self.parse(source)
        self.assertEqual(procs[0][3][0][0:2], ('state', 'outer'))
        self.assertEqual(procs[0][3][0][2][0][0:2], ('state', 'child'))
        self.assertEqual(beta_interp.interpret(procs, b''), (7, b''))

    def test_block_shape_rejects_interleaving_and_post_terminator_statements(self):
        invalid = [
            'proc main() { state child { } return 0 }',
            'proc main() { state child { state nested { } return 0 } }',
            'proc main() { return 0 let x = 1 }',
            'proc main() { to done let x = 1 state done { return 0 } }',
        ]
        for source in invalid:
            with self.subTest(source=source):
                with self.assertRaises(SyntaxError):
                    self.parse(source)

        self.parse('proc main() { return 7 state dead { return 0 } }')
        self.parse('proc main() { to done state done { return 0 } }')

    def test_every_path_initialization_accepts_alternate_assignments_and_loops(self):
        alternate = '''
            proc main() {
                to assigned when read_byte()
                state initialize { let x = 1 to join }
                state assigned { x = 2 to join }
                state join { return x }
            }
        '''
        loop = '''
            proc main() {
                let x = 0
                state loop { to body when x < 3 return x }
                state body { x = x + 1 to loop }
            }
        '''
        self.assertEqual(beta_interp.interpret(self.parse(alternate), b'\0'), (1, b''))
        self.assertEqual(beta_interp.interpret(self.parse(alternate), b'x'), (2, b''))
        self.assertEqual(beta_interp.interpret(self.parse(loop), b''), (3, b''))

    def test_every_path_initialization_rejects_skips_and_traversal_order_bias(self):
        skipped = '''
            proc main() {
                to bypass when read_byte()
                state initialize { let x = 1 to join }
                state bypass { to join }
                state join { return x }
            }
        '''
        traversal_order = '''
            proc main() {
                to head
                state initialize { let x = 1 to head }
                state head { to initialize when read_byte() return x }
            }
        '''
        for source in (skipped, traversal_order):
            with self.subTest(source=source):
                with self.assertRaisesRegex(SyntaxError, 'uninitialized'):
                    self.parse(source)

    def test_unreachable_blocks_skip_only_the_initialization_judgment(self):
        accepted = '''
            proc main() {
                return 7
                state declared { let x = 1 }
                state dead { return x }
            }
        '''
        self.assertEqual(beta_interp.interpret(self.parse(accepted), b''), (7, b''))
        with self.assertRaisesRegex(SyntaxError, 'unresolved local'):
            self.parse('proc main() { return 7 state dead { return x } }')

    def test_let_initializer_and_assignment_require_prior_declaration(self):
        for source in (
            'proc main() { let x = x return 0 }',
            'proc main() { x = 1 let x = 2 return x }',
        ):
            with self.subTest(source=source):
                with self.assertRaises(SyntaxError):
                    self.parse(source)

    def test_reference_sources_have_no_backend_diagnostic_import(self):
        here = Path(__file__).resolve().parent
        forbidden = {'bc2', 'beta_symbolic', 'symbolic_differential'}
        for name in ('beta_parser.py', 'beta_interp.py', 'io-verify.py'):
            tree = ast.parse((here / name).read_text(), filename=name)
            imported = {
                alias.name.split('.')[0]
                for node in ast.walk(tree)
                if isinstance(node, (ast.Import, ast.ImportFrom))
                for alias in node.names
            }
            self.assertTrue(forbidden.isdisjoint(imported), (name, imported))

    def test_retired_compiler_facade_is_absent(self):
        root = Path(__file__).resolve().parents[4]
        self.assertFalse((root / 'compiler/beta-lang-py').exists())


if __name__ == '__main__':
    unittest.main()
