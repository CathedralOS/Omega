"""Pack consecutive proposed stages into columns, keeping each stage intact."""
def layout_columns(model):
    # Put the two longest worklists (source and native planning) in adjacent
    # columns. The shorter stages share columns in pipeline order.
    column_starts = {0, 2, 4, 6, 9}
    width, gutter, inset = 288, 32, 8
    top = 32
    nodes = []
    frames = []
    column = -1
    positions = {node['id']:index for index,node in enumerate(model['nodes'])}
    for number, (_, first, last, _) in enumerate(model['phases']):
        if number in column_starts:
            column += 1
            y = top
        x = inset * 2 + column * (width + gutter)
        first_y = y
        for record in model['nodes'][positions[first]:positions[last]+1]:
            node = dict(record, x=x, y=y, width=width)
            notes = list(record.get('notes', []))
            if record['role'] == 'owner':
                notes = ['Work may repeat on demand'] if number in (1, 3) else []
            reference = ''
            if notes and notes[-1].startswith(('Current ', 'Same Psi 06')):
                reference_text = notes.pop().removeprefix('Current ').removeprefix('Same ')
                reference = ' '.join(reference_text.split()[:2])
            node.update(notes=notes, reference=reference, height=25 + 15 * len(notes))
            nodes.append(node)
            y += node['height'] + 7
        frames.append(dict(id=f'stage-{number}', x=x-inset, y=first_y-inset,
                           width=width+2*inset, height=y-first_y+inset,
                           caption=dict(lines=[])))
        y += 22

    edges = []
    for edge, start, end in zip(model['edges'], nodes, nodes[1:]):
        if start['x'] == end['x']:
            middle = start['x'] + width / 2
            points = [(middle, start['y']+start['height']), (middle, end['y'])]
        else:
            right = start['x'] + width
            channel = right + gutter / 2
            exit_y = start['y'] + start['height']/2
            entry_y = end['y'] + end['height']/2
            points = [(right, exit_y), (channel, exit_y), (channel, entry_y), (end['x'], entry_y)]
        edges.append(dict(edge, labels=[], sections=[dict(points=[dict(x=x,y=y) for x,y in points])]))
    return dict(model, nodes=nodes, edges=edges, groups=frames,
                width=frames[-1]['x']+frames[-1]['width']+inset,
                height=max(frame['y']+frame['height'] for frame in frames)+inset,
                startLabel=dict(x=nodes[0]['x'], y=0))
