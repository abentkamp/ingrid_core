# Plan: Zuordnungs-Kreuzwort

## The puzzle
- A filled **17x8** grid with every block shown and no clue numbers.
- **Pattern:** 20–24 blocks (the original has 22). Every white cell belongs to an entry of at
  least 2 letters, all white cells are connected, and no entry is longer than **10 letters**.
- **Hints in the grid:**
  - One randomly chosen entry of 5 or more letters, written in full.
  - Every occurrence of one letter, picked at random from the grid's 4 most frequent letters.
  - The two hints may overlap.
- **Clues:** shuffled and split into *Waagerecht* and *Senkrecht*, with **no letter counts**.
  The solver has to work out which clue belongs to which slot.
- **Words:**
  - Umlauts are written AE/OE/UE, at `--min-score 50`.
  - Allowed: proper names, abbreviations, plurals, infinitives, adjective base forms,
    lexicalized participles, and ambiguous words if one reading qualifies.
  - Not allowed: any other inflected form, or the same word twice in one grid (Ingrid's dupe
    check already prevents that).
- No uniqueness check and no difficulty rating.

## 1. New binary `zkw` (Rust, `src/bin/zkw.rs`)

A separate binary on top of the library, so the existing `ingrid_core` CLI stays as it is. It
gets a `[[bin]]` entry in `Cargo.toml` and adds `serde_json` as a normal dependency.

```
zkw generate --count 3 --start 1 --seed 1000 --out puzzles/
zkw refill puzzles/002          # same pattern, new fill after blocklist changes
zkw pattern --seed 5            # print a pattern only, for debugging
```

Shared options: `--wordlist wordlist.txt`, `--blocklist german-blocklist.txt`,
`--min-score 50`, `--max-len 10`, `--blocks 20..=24`, `--fill-timeout 5s`.

All randomness comes from one `--seed` per puzzle. Puzzle *N* uses `seed + N`, and the pattern,
fill, hints and shuffle each draw from their own RNG stream derived from that seed. Rerunning the
same command gives byte-identical output.

### 1a. Pattern generator (`src/pattern.rs`, in the library so it can be tested)
- **Representation:** `Vec<bool>` with 136 cells.
- **Constraint check:**
  1. Compute horizontal and vertical runs.
  2. Every white cell must sit in at least one run of 2 or more letters.
  3. Every run must be at most `max_len` long.
  4. A flood fill from any white cell must reach all white cells.
- **Search** (local search, since plain rejection sampling would rarely hit all the constraints):
  1. Pick a block count *B* from the allowed range and place *B* blocks at random.
  2. Score the grid by its number of violations: unchecked cells, the excess length of runs over
     `max_len`, and the number of disconnected components minus one.
  3. Repeatedly move a random block to a random white cell. Keep the move if the score doesn't
     get worse, with a small chance of accepting a worse move to escape dead ends.
  4. Stop at score 0, or restart after about 20k steps.
- **Extra soft filters** (configurable), because the pool of short words at score ≥50 is small
  (90 two-letter and 360 three-letter words): at most 6 two-letter entries, and 40–52 entries
  in total (the original has 46).
- **Fill check:** run `find_fill_with_seed` with the timeout. If it fails or times out, generate
  the next pattern and log the attempt to stderr.
- **Tests:** unit tests for each constraint check, plus a property test that 200 generated
  patterns all pass.

### 1b. Word list and blocklist loading
- Read `wordlist.txt` and drop every word listed in `german-blocklist.txt` (one word per line,
  `#` comments allowed, e.g. `BEKAM  # Präteritum`).
- Pass the filtered text to `WordList` as `FileContents`, the same way the embedded word list is
  loaded today (that type may need a non-`'static` variant, or the string can be leaked).
- Because scores are only checked per fill, the blocklist only removes words. Correct words with
  too-low scores (HAEUSER, 30) are never found this way; a `score-overrides.txt` can be added
  later if that limits the fills.

### 1c. Hint selection
- **Hint word:** chosen uniformly from the entries of 5 or more letters, across and down
  together.
- **Hint letter:** count letters over the grid's cells, each cell once, and take the top 4. Ties
  are broken alphabetically. Pick one of the four uniformly.
- **Revealed cells:** the hint word's cells plus every cell with the hint letter, as one set. Its
  size is written to the JSON.

### 1d. JSON output (`puzzles/NNN/puzzle.json`)
```json
{
  "series": "Zuordnungs-Kreuzwort",
  "number": 1,
  "seed": 1001,
  "width": 17, "height": 8,
  "pattern": ["......#.....#....", "..."],
  "solution": ["NICHTE#SPAET#GABE", "..."],
  "hint_word": {"word": "VEREINEN", "row": 7, "col": 1, "dir": "across"},
  "hint_letter": "E",
  "revealed": [[1,5], [1,9], "..."],
  "across": [ {"word": "ATOME", "row": 8, "col": 7, "len": 5, "score": 50}, ... ],
  "down":   [ {"word": "PRAG",  "row": 1, "col": 9, "len": 4, "score": 50}, ... ]
}
```
- `across` and `down` are **shuffled at random** (seeded); that order is final and is used as is
  by the clue doc and the PDF.
- Positions stay in the JSON for the solution page and for checking; the puzzle page doesn't
  print them. Coordinates are 1-based, matching the existing `-w` output.
- `zkw` also writes `grid.txt` (the pattern) so `refill` can reuse it.

## 2. Word check (Claude, after each `generate`/`refill`)
1. Check every entry of each new puzzle against the allowed-forms rules.
2. Add rejected words to `german-blocklist.txt` with the reason.
3. Run `zkw refill puzzles/NNN` (same pattern, same seed, new fill and hints) and check again.
4. Repeat until all entries pass, then report the blocked words in a short list the user can
   veto.

## 3. Clue doc (Claude Docs, one doc per batch)
- Title "Zuordnungs-Kreuzwort – Hinweise Nr. 001–003", one section per puzzle, then
  *Waagerecht* and *Senkrecht* in the JSON order.
- For each word, three draft clues as checkboxes plus a blank line for an own clue:
  ```
  ATOME (5)
  ☐ Kleinste Bausteine der Materie
  ☐ Teilchen aus Kern und Hülle
  ☐ Gegenstand der Kernphysik
  ☐ Eigener Vorschlag: …
  ```
  The length is shown only in the doc, for editing; it never reaches the PDF. If the doc format
  doesn't support real checkboxes, fall back to "mark with ✓".
- The three drafts vary in difficulty: a plain definition, a paraphrase, and a cryptic or
  associative clue. Clues must not contain the answer or a word sharing its stem.
- **Reading it back:** the ticked clue wins, unless the own line is filled in, which then takes
  priority. If nothing or more than one is ticked, flag the word and don't guess.
- Result: `puzzles/NNN/clues.json`, `{"across": ["…", …], "down": ["…", …]}`, in the same order
  as `puzzle.json`.

## 4. PDF (Typst, A4 portrait)
- **Setup:** install the prebuilt Typst binary; pin the version in `scripts/setup.sh`.
- **`typst/zkw.typ`**, a template function reading `puzzle.json` and `clues.json` with `json()`:
  - **Puzzle page:** header "Zuordnungs-Kreuzwort Nr. 1"; the grid at about 10mm per cell
    (17 columns fit within the 180mm text width), blocks solid black, revealed letters in bold
    sans-serif centered in their cells; below it *Waagerecht* and *Senkrecht* as compact
    two-column lists in JSON order, with no numbers, positions or letter counts.
  - **Solution page:** "Lösung Nr. 1" with the filled grid, no special marking of the hints.
- **Build files:**
  - `typst/einzeln.typ` → `puzzles/NNN/puzzle-NNN.pdf` (puzzle on page 1, solution on page 2).
  - `typst/sammlung.typ` → `sammlung.pdf`: all puzzle pages in order, then all solutions at the
    end, two or three to a page.
- **`scripts/build.sh`:** `typst compile --input nr=NNN` per puzzle, then the collection.
- **Check:** render PNGs of the pages and look for layout problems before the user sees them.

## Repo layout
```
src/bin/zkw.rs, src/pattern.rs
german-blocklist.txt
puzzles/NNN/{grid.txt, puzzle.json, clues.json, puzzle-NNN.pdf}
typst/{zkw.typ, einzeln.typ, sammlung.typ}
scripts/{setup.sh, build.sh}
sammlung.pdf
```

## Milestones
1. Pattern generator with tests; show 5 sample patterns.
2. `zkw generate` with blocklist and JSON; generate 001–003 and run the word check.
3. Typst template with placeholder clues; user reviews the layout.
4. Clue doc for 001–003; user ticks or edits, Claude reads it back.
5. Final PDFs (single files and collection); review and adjust before scaling up.
