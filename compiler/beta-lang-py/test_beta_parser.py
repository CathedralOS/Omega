#!/usr/bin/env python3
"""Focused ownership and grammar checks for the shared Beta parser."""

import sys
import subprocess
import unittest

import beta_parser


class BetaParserTests(unittest.TestCase):
    def test_parser_import_does_not_load_optional_compiler(self):
        completed = subprocess.run(
            [
                sys.executable,
                '-c',
                'import beta_parser, sys; assert "bc2" not in sys.modules',
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

    def test_bc2_reexports_the_same_parser_for_compatibility(self):
        import bc2

        self.assertIs(bc2.lex, beta_parser.lex)
        self.assertIs(bc2.Parser, beta_parser.Parser)


if __name__ == '__main__':
    unittest.main()
