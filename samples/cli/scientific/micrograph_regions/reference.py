"""Independent union-find oracle; never used by the Omega executable."""
from pathlib import Path
import hashlib
import json
import random

ROOT = Path(__file__).resolve().parent
ERROR = 'ERROR invalid image: width height 1..8, threshold 0..255, connectivity 4 or 8, exact pixels 0..255\n'

def reference(width, height, threshold, connectivity, pixels):
    parent = list(range(len(pixels)))
    def root(index):
        while parent[index] != index:
            index = parent[index]
        return index
    # Union only already encountered adjacent pixels; this differs from FIFO flood.
    for index, value in enumerate(pixels):
        if value < threshold:
            continue
        x, y = index % width, index // width
        for dx, dy in ((-1, 0), (0, -1), (-1, -1), (1, -1)):
            if connectivity == 4 and dx and dy:
                continue
            nx, ny = x + dx, y + dy
            if 0 <= nx < width and 0 <= ny < height:
                previous = ny * width + nx
                if pixels[previous] >= threshold:
                    parent[root(index)] = root(previous)
    names = {}
    labels = []
    for index, value in enumerate(pixels):
        if value < threshold:
            labels.append(0)
        else:
            representative = root(index)
            names.setdefault(representative, len(names) + 1)
            labels.append(names[representative])
    pgm = 'P2\n# micrograph region labels; zero is background\n%d %d\n64\n' % (width, height)
    pgm += ''.join(' '.join(map(str, labels[y*width:(y+1)*width])) + '\n' for y in range(height))
    report = 'regions %d\nlabel area centroid_x centroid_y min_x min_y max_x max_y\n' % len(names)
    for label in range(1, len(names) + 1):
        points = [(i % width, i // width) for i, value in enumerate(labels) if value == label]
        xs, ys = zip(*points)
        area = len(points)
        report += f'{label} {area} {sum(xs)}/{area} {sum(ys)}/{area} {min(xs)} {min(ys)} {max(xs)} {max(ys)}\n'
    return pgm, report

def freeze():
    cases = []
    def add(name, width, height, threshold, connectivity, pixels):
        text = f'{width} {height} {threshold} {connectivity}\n' + '\n'.join(' '.join(map(str, pixels[y*width:(y+1)*width])) for y in range(height)) + '\n'
        pgm, report = reference(width, height, threshold, connectivity, pixels)
        cases.append((name, text, 0, pgm + report))
        (ROOT/'expected'/f'{name}.pgm').write_text(pgm)
        (ROOT/'expected'/f'{name}.regions.txt').write_text(report)
    synthetic = [200,200,0,0,0,255, 200,0,0,128,0,255, 0,0,128,0,0,0, 0,127,0,0,180,180, 128,0,0,0,180,0]
    add('synthetic4',6,5,128,4,synthetic)
    add('synthetic8',6,5,128,8,synthetic)
    # Two lobes share one orthogonal bridge; they must become one region.
    add('touching',5,3,100,4,[200,200,0,200,200, 200,200,200,200,200, 0,0,0,0,0])
    add('empty',4,3,1,4,[0]*12)
    add('ties',5,1,128,4,[127,128,129,0,128])
    add('max_tie',2,2,255,8,[254,255,255,0])
    add('zero_threshold',3,2,0,4,[0]*6)
    add('single',1,1,255,4,[255])
    add('full_capacity',8,8,1,4,[255]*64)
    checker = [255 if (x+y)%2==0 else 0 for y in range(8) for x in range(8)]
    add('checker4',8,8,1,4,checker)
    add('checker8',8,8,1,8,checker)
    add('row_edges',4,2,1,4,[0,0,0,255,255,0,0,0])
    add('vertical',1,8,128,4,[128,128,0,255,255,0,128,128])
    generator = random.Random(23023)
    for index in range(6):
        width,height=generator.randint(2,8),generator.randint(2,8)
        pixels=[generator.choice([0,127,128,129,255]) for _ in range(width*height)]
        add(f'variant{index}',width,height,128,4 if index%2==0 else 8,pixels)
    invalid = {
        'bad_width':'0 2 128 4\n', 'bad_height':'2 9 128 4\n',
        'bad_threshold':'1 1 256 4 255\n','bad_connectivity':'1 1 128 6 255\n',
        'bad_pixel':'1 1 128 4 256\n', 'truncated':'2 2 128 4 255 0 0\n',
        'extra':'1 1 128 4 255 0\n','negative':'1 1 128 4 -1\n',
        'nondecimal':'1 1 128 4 x\n','long_token':'0001 1 128 4 255\n',
        'empty_input':'', 'malformed_suffix':'1 1 128 4 255x\n'
    }
    cases += [(name,text,2,ERROR) for name,text in invalid.items()]
    manifest = {'oracle':'union-find independent of Omega FIFO implementation', 'tolerance':0,
                'centroids':'exact unreduced sum/area, zero-based pixel centers; inclusive bounds', 'cases':[]}
    for name, text, code, output in cases:
        input_path=ROOT/'data'/f'{name}.txt'; output_path=ROOT/'expected'/f'{name}.stdout.txt'
        input_path.write_text(text); output_path.write_text(output)
        manifest['cases'].append({'name':name,'exit':code,'input_sha256':hashlib.sha256(text.encode()).hexdigest(), 'stdout_sha256':hashlib.sha256(output.encode()).hexdigest()})
    (ROOT/'expected/frozen.json').write_text(json.dumps(manifest,indent=2)+'\n')

def analytic_checks():
    # Closed-form checks challenge the reference independently of its unions.
    _, report = reference(8, 8, 1, 4, [255] * 64)
    assert report.endswith('1 64 224/64 224/64 0 0 7 7\n')
    _, report = reference(8, 8, 1, 4, [255 if (x+y)%2==0 else 0 for y in range(8) for x in range(8)])
    assert report.startswith('regions 32\n')
    assert all(line.split()[1] == '1' for line in report.splitlines()[2:])
    _, report = reference(4, 2, 1, 4, [0,0,0,255,255,0,0,0])
    assert report.endswith('1 1 3/1 0/1 3 0 3 0\n2 1 0/1 1/1 0 1 0 1\n')
    _, report = reference(5, 1, 128, 4, [127,128,129,0,128])
    assert report.endswith('1 2 3/2 0/2 1 0 2 0\n2 1 4/1 0/1 4 0 4 0\n')

if __name__ == '__main__':
    # Explicit authoring operation only; validate.py never regenerates these.
    analytic_checks()
    freeze()
