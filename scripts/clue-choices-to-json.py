# Usage: python3 -I scripts/clue-choices-to-json.py <saved docs read result> NNN
# Reads the user's ticked clues for puzzle NNN from a saved full-doc read of the clue doc and writes
# puzzles/NNN/clues.json. Refuses (exit 1) on a word with no tick, several ticks, an empty own clue,
# or a word list that doesn't match puzzle.json.
import json, subprocess, sys
src, nr = sys.argv[1], sys.argv[2]
here = __file__.rsplit("/", 1)[0]
choices = json.loads(subprocess.run(
    [sys.executable, "-I", f"{here}/clue-doc-readback.py", src, nr],
    check=True, capture_output=True, text=True).stdout)
puzzle = json.load(open(f"puzzles/{nr}/puzzle.json", encoding="utf-8"))
out, problems = {}, []
for d, sub in (("across", "Waagerecht"), ("down", "Senkrecht")):
    words = choices.get(sub, [])
    if [w for w, _ in words] != [e["word"] for e in puzzle[d]]:
        problems.append(f"{sub}: words differ from puzzle.json")
        continue
    lst = []
    for w, items in words:
        ticked = [t for c, t in items if c]
        if len(ticked) != 1:
            problems.append(f"{w}: {len(ticked)} ticks")
            continue
        t = ticked[0]
        if t.startswith("Eigener Vorschlag:"):
            t = t.split(":", 1)[1].strip()
        if not t or t == "…":
            problems.append(f"{w}: own clue is empty")
            continue
        lst.append({"word": w, "clue": t})
    out[d] = lst
if problems:
    print("\n".join(problems)); sys.exit(1)
with open(f"puzzles/{nr}/clues.json", "w", encoding="utf-8") as f:
    json.dump(out, f, ensure_ascii=False, indent=2); f.write("\n")
print(f"puzzles/{nr}/clues.json")
