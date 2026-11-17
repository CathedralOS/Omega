"""Arrange proposed operations on a vertical spine, with space for arrowheads."""
def layout_stages(model):
    width, inset = 432, 12
    nodes, frames = [], []
    y = 44
    positions = {node['id']:index for index,node in enumerate(model['nodes'])}
    for number, (_, first, last, _) in enumerate(model['phases']):
        x = inset * 2
        first_y = y
        for record in model['nodes'][positions[first]:positions[last]+1]:
            node = dict(record, x=x, y=y, width=width)
            notes = list(record.get('notes', []))
            if record['role'] == 'owner':
                notes = ['Work may repeat on demand'] if number in (1, 3) else []
            reference = ''
            if notes and notes[-1].startswith(('Current ', 'Same Psi 06')):
                reference = notes.pop().removeprefix('Current ')
            node.update(notes=notes, reference=reference, height=40 + 19 * len(notes))
            nodes.append(node)
            y += node['height'] + 24
        frames.append(dict(id=f'stage-{number}', x=x-inset, y=first_y-inset,
                           width=width+2*inset, height=y-first_y-24+2*inset,
                           caption=dict(lines=[])))
        y += 32

    edges = []
    by_id = {node['id']:node for node in nodes}
    for edge in model['edges']:
        start, end = by_id[edge['start']], by_id[edge['end']]
        middle = start['x'] + width / 2
        points = [(middle, start['y']+start['height']), (middle, end['y'])]
        edges.append(dict(edge, labels=[], sections=[dict(points=[dict(x=x,y=y) for x,y in points])]))
    return dict(model, nodes=nodes, edges=edges, groups=frames,
                width=frames[-1]['x']+frames[-1]['width']+inset,
                height=max(frame['y']+frame['height'] for frame in frames)+inset,
                startLabel=dict(x=nodes[0]['x'], y=0))
