# Usage: python3 -I readback.py <docs-read-result.json> <NNN>
# Prints, per word of section "Nr. NNN", its items: [x]/[ ] + text.
import json, sys
import xml.etree.ElementTree as ET
d = json.load(open(sys.argv[1]))
obj, _ = json.JSONDecoder().raw_decode(d[1]["text"])
root = ET.fromstring(obj["data"]["xml"])
def text(el):
    return "".join(t.text or "" for t in el.iter() if t.tag in ("text",) for _ in [0]) if False else "".join(el.itertext())
section = None; sub = None; word = None
out = {}
for el in root:
    t = text(el).strip()
    if el.tag == "paragraph" and el.get("heading") == "2":
        section = t
        continue
    if section != f"Nr. {sys.argv[2]}":
        continue
    if el.tag == "paragraph" and el.get("heading") == "3":
        sub = t; continue
    if el.tag == "paragraph" and t.endswith(")") and "(" in t and t.split(" (")[0].isupper():
        word = t.split(" (")[0]; continue
    if el.tag == "list" and word:
        items = [(li.get("checked") == "true", text(li).strip()) for li in el.findall("listItem")]
        out.setdefault(sub, []).append((word, items))
        word = None
print(json.dumps(out, ensure_ascii=False, indent=1))
