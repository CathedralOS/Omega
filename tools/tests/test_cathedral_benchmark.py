import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

BENCHMARKS = Path(__file__).resolve().parents[1] / 'benchmark'
sys.path.insert(0, str(BENCHMARKS))
specification = importlib.util.spec_from_file_location('cathedral_benchmark', BENCHMARKS / 'cathedral.py')
benchmark = importlib.util.module_from_spec(specification)
specification.loader.exec_module(benchmark)


class CathedralBenchmarkTests(unittest.TestCase):
    def test_success_requires_a_fresh_valid_image(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            entry = dict(name='kernel', target='uefi_x86_64', source='main.omg', kind='uefi')
            def successful_compile(command, working):
                output = directory / 'product'
                output.mkdir()
                contents = bytearray(68)
                contents[:2] = b'MZ'
                contents[60:64] = (64).to_bytes(4, 'little')
                contents[64:] = b'PE\0\0'
                (output / 'omega-program.exe').write_bytes(contents)
                return 0, 123.0, 1024, 'published'
            with patch.object(benchmark, 'capture_compile', successful_compile):
                result = benchmark.compile_entry(directory / 'compiler', directory, entry, directory)
            self.assertTrue(result['artifact_valid'])
            self.assertEqual(result['artifacts']['omega-program.exe']['bytes'], 68)
            self.assertEqual(benchmark.compiler_wall_sum_ms([entry], [dict(name='kernel', compile=result)]), 123)
            (directory / 'product/omega-program.exe').write_bytes(b'not an executable')
            with self.assertRaises(ValueError):
                benchmark.validate_artifacts(directory, entry, directory / 'product')

    def test_failed_compiler_with_leftover_image_cannot_qualify(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            entry = dict(name='kernel', target='uefi_x86_64', source='main.omg', kind='uefi')
            def failed_compile(command, working):
                output = directory / 'product'
                output.mkdir()
                (output / 'omega-program.exe').write_bytes(b'MZ' + b'old bytes' * 20)
                return 1, 123.0, 1024, 'compiler rejected the source'
            with patch.object(benchmark, 'capture_compile', failed_compile):
                result = benchmark.compile_entry(directory / 'compiler', directory, entry, directory)
            self.assertFalse(result['artifact_valid'])
            self.assertIsNone(benchmark.compiler_wall_sum_ms([entry], [dict(name='kernel', compile=result)]))
            with self.assertRaises(ValueError):
                benchmark.compile_entry(directory / 'compiler', directory, entry, directory)

    def test_partial_composition_has_no_full_compile_metric(self):
        entries = [dict(name='kernel'), dict(name='matrix')]
        rows = [dict(name='kernel', compile=dict(exit_code=0, artifact_valid=True, wall_ms=12))]
        self.assertIsNone(benchmark.compiler_wall_sum_ms(entries, rows))
        rows.append(dict(name='matrix', compile=dict(exit_code=0, artifact_valid=True, wall_ms=13)))
        self.assertEqual(benchmark.compiler_wall_sum_ms(entries, rows), 25)


if __name__ == '__main__':
    unittest.main()
