"""Format theme files in the house style (one key per line, short arrays inline): python3 tools/fmt_themes.py crates/keylume-profiles/themes/*.json"""
import json, sys
def inline(v): return json.dumps(v, ensure_ascii=False, separators=(", ", ": ")).replace("{", "{ ", 1)[:-1] + " }" if isinstance(v, dict) else json.dumps(v, ensure_ascii=False, separators=(", ", ": "))
for p in sys.argv[1:]:
    d = json.load(open(p, encoding="utf-8"))
    out = ["{", f'  "collection": {json.dumps(d["collection"], ensure_ascii=False)},', f'  "prefix": {json.dumps(d["prefix"])},']
    if "section" in d:
        out.append(f'  "section": {json.dumps(d["section"])},')
    if "keys" in d:
        out.append('  "keys": [')
        out.append(",\n".join(f"    {inline(r)}" for r in d["keys"]))
        out.append("  ],")
    extra = set(d) - {"collection", "prefix", "section", "keys", "themes"}
    assert not extra, f"{p}: unknown fields {extra}"
    out.append('  "themes": [')
    for i, t in enumerate(d["themes"]):
        lines = [f'      {json.dumps(k)}: {inline(v)}' for k, v in t.items()]
        out += ["    {", ",\n".join(lines), "    }" + ("," if i < len(d["themes"]) - 1 else "")]
    out += ["  ]", "}", ""]
    open(p, "w", encoding="utf-8").write("\n".join(out))
    json.load(open(p, encoding="utf-8"))  # still valid
    print("formatted", p)
