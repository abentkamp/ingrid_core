# Usage: python3 -I mkmd.py puzzle.json drafts.txt
# drafts.txt: lines "WORD | clue1 | clue2 | clue3"
import json, sys
p = json.load(open(sys.argv[1]))
drafts = {}
for line in open(sys.argv[2], encoding="utf-8"):
    line = line.strip()
    if not line or line.startswith("#"):
        continue
    parts = [x.strip() for x in line.split("|")]
    drafts[parts[0]] = parts[1:]
out = [f"## Nr. {p['number']:03d}"]
for d, title in (("across", "Waagerecht"), ("down", "Senkrecht")):
    out.append(f"\n### {title}\n")
    for e in p[d]:
        w = e["word"]
        c = drafts[w]
        assert len(c) == 3, w
        for clue in c:
            norm = clue.lower().replace("ä","ae").replace("ö","oe").replace("ü","ue").replace("ß","ss")
            if len(w) <= 3:
                import re
                assert w.lower() not in re.findall(r"\w+", norm), (w, clue)
            else:
                assert w.lower() not in norm, (w, clue)
        out.append(f"**{w}** ({e['len']})\n")
        h = p["hint_word"]
        if h["dir"] == d and (h["row"], h["col"]) == (e["row"], e["col"]):
            out.append("*Lösungshilfe: Dieses Wort steht im Gitter, sein Hinweis erscheint im PDF durchgestrichen.*\n")
        out += [f"- [ ] {x}" for x in c] + ["- [ ] Eigener Vorschlag: …", ""]
missing = [e["word"] for d in ("across","down") for e in p[d] if e["word"] not in drafts]
assert not missing, missing
print("\n".join(out))
