#!/usr/bin/env python3
"""Print per-key profiles as letter grids, one letter per colour, to check flags and
patterns without screenshots.

    npm run fixtures
    python3 tools/flag_grid.py flag-seychelles flag-saint-lucia
"""
import json
import sys

lib = json.load(open("public/mock/library.json"))
keys = lib["layout"]["keys"]
byid = {p["id"]: p for p in lib["profiles"]}
for pid in sys.argv[1:]:
    packed = byid[pid]["lighting"]["packed"]
    colours = [packed[i:i + 6] for i in range(0, len(packed), 6)]
    legend = {}
    print(pid)
    for row in range(5):
        cells = sorted((k["x"], k["w"], c) for k, c in zip(keys, colours) if int(k["y"]) == row)
        # wide keys get wider cells, so the rows line up roughly as on the board
        print("  " + " ".join(legend.setdefault(c, chr(65 + len(legend))) * max(1, round(w * 2)) for _, w, c in cells))
    print("  " + "  ".join(f"{v}=#{k}" for k, v in legend.items()))
