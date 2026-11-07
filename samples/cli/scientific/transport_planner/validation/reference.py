"""Independent cell-by-cell enumeration: no four-free-cell formula."""
from itertools import product

def optimum(values):
    supplies, demands, costs = values[:3], values[3:6], values[6:]
    if sum(supplies) != sum(demands):
        return None
    best = None
    def visit(cell, remaining_supply, remaining_demand, table, cost):
        nonlocal best
        if cell == 9:
            if any(remaining_supply) or any(remaining_demand):
                return
            candidate = (cost, tuple(table))
            if best is None or candidate < best:
                best = candidate
            return
        row, column = divmod(cell, 3)
        for quantity in range(min(remaining_supply[row], remaining_demand[column]) + 1):
            supply = remaining_supply.copy()
            demand = remaining_demand.copy()
            supply[row] -= quantity
            demand[column] -= quantity
            visit(cell + 1, supply, demand, table + [quantity], cost + quantity * costs[cell])
    visit(0, supplies.copy(), demands.copy(), [], 0)
    return best

def expected(values):
    result = optimum(values)
    if result is None:
        return 3, f"INFEASIBLE: unequal totals\nsupply {sum(values[:3]):+05d}\ndemand {sum(values[3:6]):+05d}\n"
    cost, table = result
    rows = [" ".join(map(str, table[index:index+3])) for index in range(0, 9, 3)]
    return 0, "OPTIMAL: shipments\n" + "\n".join(rows) + f"\ntotal_cost {cost:+05d}\n"

def tiny_cases():
    # ALL 2x2 margins in 0..2 and ALL four costs from {-1,0,2}.
    # Embedded with a zero third source and destination.
    for margins in product(range(3), repeat=4):
        if sum(margins[:2]) != sum(margins[2:]):
            continue
        for costs in product((-1, 0, 2), repeat=4):
            yield list(margins[:2]) + [0] + list(margins[2:]) + [0] + [costs[0], costs[1], 0, costs[2], costs[3], 0, 0, 0, 0]
