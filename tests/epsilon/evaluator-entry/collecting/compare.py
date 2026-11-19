#!/usr/bin/env python3
"""Compare ordinary and collecting Gamma programs on the frozen evaluator benchmark."""
import argparse
import hashlib
from pathlib import Path
import runpy
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from large_input import compile_with, measure, replace_member


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('frozen', type=Path, help='b957 checkout with its source-built native compiler')
    parser.add_argument('cursor_run', type=Path, help='large_input.py output for the 720b cursor evaluator')
    parser.add_argument('output', type=Path, help='new directory for the private compiler and customers')
    parser.add_argument('--whole', action='store_true', help='run the complete frozen request under the resident-memory guard')
    args = parser.parse_args()
    frozen = args.frozen.resolve()
    work = args.output.resolve()
    work.mkdir()
    vm_sources = frozen / 'bootstrap/compiled_vm'
    sys.path.insert(0, str(vm_sources))
    vm = runpy.run_path(str(vm_sources / 'test.py'))
    stock = frozen / 'build/independent-native-reference/compiler2'
    assert sha(stock.read_bytes()) == '15831075c8059252e707957e3880b66a42b3e1df240a30803bbe348333690961'

    # Apply the four lowering changes to a separate copy of the frozen compiler.
    hooks = work / 'compiler.gamma'
    subprocess.run(['patch', '--quiet', '--fuzz=0', '--output', hooks, vm_sources / 'compiler.gamma',
                    HERE / 'lowering.patch'], check=True)
    collector = (HERE / 'collector.gamma').read_bytes()
    capacity = b'(def gc_space_size () Int 268435456)'

    def compiler_source(size, backend):
        tuned = replace_member(collector, capacity, f'(def gc_space_size () Int {size})'.encode())
        return b'\n'.join((hooks.read_bytes(), tuned, (vm_sources / 'frontend.gamma').read_bytes(), backend.read_bytes()))

    native = frozen / 'bootstrap/independent/native.gamma'
    source = compiler_source(1536 * 1024**2, native)
    assert sha(source) == '3f970157de1cf021d466f631ca40eba526371c911cadc2409ab5b4f01defe8a9'
    compiler = stock
    for generation in range(3):
        successor = work / f'compiler{generation}'
        compile_with(compiler, source, successor)
        compiler = successor
    assert (work / 'compiler1').read_bytes() == compiler.read_bytes()
    assert sha(compiler.read_bytes()) == '242a3353102502d18b08fc625051172c92349d6a0c588ccd9af3708994f249b7'

    # With just 32 cells the small programs repeatedly move live pairs. The
    # Alpha tape and native program must agree even when the tiny heap fills.
    tiny_native = work / 'tiny-native'
    tiny_alpha = work / 'tiny-alpha'
    compile_with(compiler, compiler_source(1024, native), tiny_native)
    compile_with(stock, compiler_source(1024, vm_sources / 'alpha_backend.gamma'), tiny_alpha)
    for name, ordinary, tiny in [
        ('both-operands', (0, bytes((11, 12, 21, 22))), (0, bytes((11, 12, 21, 22)))),
        ('caller-shared-and-tail', (0, b'*'), (0, b'*')),
        ('shared-dag-and-scalar-address', (0, b'*'), (0, b'*')),
        ('unestablished-locals', (0, b'*'), (0, b'*')),
        ('exact-live', (0, b'*'), (0, b'*')),
        ('adjacent-live', (0, b'*'), (3, b'')),
        ('departed-lexical-scope', (0, b'*'), (3, b'')),
        ('application-capacity', (0, b'A'), (252, b'')),
    ]:
        customer = (HERE / 'cases' / f'{name}.gamma').read_bytes()
        for profile, producer, expected in [('stock', stock, ordinary), ('native', tiny_native, tiny), ('alpha', tiny_alpha, tiny)]:
            executable = work / f'{name}-{profile}'
            if profile == 'alpha':
                # The stamped Alpha seed is large. Reuse it after each observed tape.
                executable = work / 'alpha-customer'
                tape = work / f'{name}.tape'
                with tape.open('xb') as stream:
                    subprocess.run([producer], input=customer, stdout=stream, check=True)
                vm['stamp'](tape, executable)
            else:
                compile_with(producer, customer, executable)
            assert vm['execute'](executable) == expected, (name, profile)

    receipt = (args.cursor_run / 'extended.gamma').read_bytes()
    request = args.cursor_run / 'whole.ereq'
    assert sha(receipt) == '5c9425d50aa62ca233f30e123a48ffcd424d6b6b451475c79cf4e82e3e934fbb'
    assert sha(request.read_bytes()) == 'eec989b92850e3d18f14586a22111e3dbad1a7cf3ac15bc1580ba4870c94ee34'
    evaluator = work / 'evaluator'
    compile_with(compiler, receipt, evaluator)
    assert sha(evaluator.read_bytes()) == 'c44852d8006ab8f78cc1d5f01096f53ead740c34eb01e0313d3f4ef35d1472be'
    print('Eight stock/native/Alpha controls passed. Reconstructed the complete request and its collecting evaluator.', flush=True)
    if args.whole:
        output = work / 'whole.out'
        result = measure(evaluator, request, output)
        assert result['raw_status'] == 0 and result['guard'] is None and result['stderr_bytes'] == 0, result
        assert output.read_bytes() == bytes.fromhex('00020000007f030000'), 'unexpected compiler diagnostic'


if __name__ == '__main__':
    main()
