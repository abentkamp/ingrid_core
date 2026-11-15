# Zuordnungs-Kreuzwort: how to make puzzles

This is the working guide for producing puzzles in the series. Read it in full before generating,
checking or publishing a puzzle. Design decisions and their reasons are in `PLAN.md`; background on
the German word list is in `german-puzzle-setup.md`.

## The puzzle

- A filled 17x8 grid. All blocks are shown, and there are no clue numbers.
- Two hints are printed in the grid:
  - one random entry of 5 or more letters, written in full;
  - every occurrence of one letter, picked at random from the grid's 4 most frequent letters.
- The clues are shuffled and split into *Waagerecht* and *Senkrecht*, with no letter counts. The
  solver has to work out which clue belongs to which slot.
- Umlauts are written AE/OE/UE in the grid (one letter per cell).

## Rules

**Grid patterns** (enforced by `src/pattern.rs`; `zkw` applies them automatically):
- 20–24 blocks, no two blocks sharing an edge.
- Every white cell is part of an entry of at least 2 letters; all white cells are connected.
- No entry longer than 10 letters.
- Both letters of every 2-letter entry must also belong to a crossing entry.
- Soft targets, modelled on the hand-made original grid: at most 2 entries of 9–10 letters, at most 4
  two-letter entries, 40–52 entries in total, enough cells that belong to only one entry.

**Words** (checked by the agent, see step 2):
- Allowed: base forms (nominative singular), plurals, verb infinitives, adjective base forms,
  lexicalized participles (ANGESAGT, ABGELEGEN, GELASSEN), proper names, abbreviations, and
  ambiguous words if one reading qualifies (BOT = *der Bot*, RAN = *heran*, MIR = the space station).
- Not allowed: any other inflected form. That means conjugated verbs (SIEHT, BIN), past tense (LAG,
  GAB), subjunctive (SEI), imperative (GIB), non-lexicalized participles (GEWIDMET), *zu*-infinitives,
  comparatives and superlatives (AELTER, GROESSTE), case forms of nouns (KUEHEN, STAEDTEN), inflected
  articles and pronouns (DIE, DEM, IHN, JEDE, ANDERE), and words that only occur inside a fixed phrase or
  name (HOC, ANGELES, JEHER) or only as a verb particle (DAR, EINHER). Non-words such as AE or UE are
  rejected too.
- No taste rule: offensive but valid words (NAZI, HURE, KZ) are allowed.
- Contractions are allowed (AM, IM, ZUR, BEIM).
- The same word must not appear twice in one grid. Ingrid blocks exact duplicates. Variants (GEN and
  GENE, STIEL and STIELE) must be caught by hand and handled with a per-puzzle `exclude.txt` (step 2).
- `german-blocklist.txt` records every rejection with its reason. Use it as precedent for new
  judgment calls and stay consistent with it.

## Setup

```bash
cargo build --release
scripts/setup.sh        # installs the pinned Typst version into .tools/
```

The word list `wordlist.txt` (`WORD;SCORE`) is committed. `zkw` uses words with score ≥ 50.

## Files

```
wordlist.txt               scored word list (source data, do not edit)
german-blocklist.txt       rejected words, one per line, "# reason" comments
german-approved.txt        words already checked and accepted (cache, sorted)
puzzles/index.json         list of puzzles for the collection (rebuilt by zkw)
puzzles/NNN/grid.txt       block pattern
puzzles/NNN/puzzle.json    solution, hints, shuffled across/down lists, seeds, settings
puzzles/NNN/exclude.txt    optional: words excluded for this puzzle only (variant pairs)
puzzles/NNN/clue-drafts.txt  three draft clues per word: "WORD | clue1 | clue2 | clue3"
puzzles/NNN/clues.json     final clues, same order as puzzle.json
puzzles/NNN/puzzle-NNN.pdf puzzle page + solution page
sammlung.pdf               all puzzles, then all solutions
typst/                     PDF templates
```

## Step by step

### 1. Generate

```bash
./target/release/zkw generate --seed 1000 --start 4 --count 3
```

- `--start` is the first puzzle number; continue after the highest existing one.
- Use the series base seed 1000 unless told otherwise. Everything is reproducible from the seed, the
  word list and the blocklist.
- A failed pattern costs about 80 s before `zkw` moves on, so a puzzle takes between a few seconds and
  a few minutes. Run long batches in the background.
- `generate` refuses to overwrite an existing puzzle directory unless you pass `--force`.

### 2. Word check (loop until clean)

```bash
./target/release/zkw unchecked puzzles/004
```

This lists the puzzle's words that are in neither `german-approved.txt` nor the blocklist. For each
unchecked word, decide by the rules above. Then:

1. Append rejected words to `german-blocklist.txt`, aligned, with a short German reason, e.g.
   `GALT      # Präteritum von gelten`.
2. Append accepted words to `german-approved.txt`, then sort it: `sort -u -o german-approved.txt german-approved.txt`.
3. Also look at the puzzle's full word list for variant pairs (singular/plural or other forms of one
   word). If there is one, add one of the two to `puzzles/NNN/exclude.txt` with a comment.
4. If anything was rejected or excluded, refill:
   `./target/release/zkw refill puzzles/NNN`
   This fills the same pattern again with the updated lists. The fill changes substantially, so run
   `unchecked` again and repeat.
5. When all words pass and there are no variant pairs:
   `./target/release/zkw approve puzzles/NNN`
   This sets `words_checked: true` and refuses if any word is blocked.

Notes:
- Expect many rounds: about 8% of new words get rejected, and puzzles have needed 3 to 22 refills.
  The approved cache makes later rounds shorter.
- `refill` stops after 10 rounds and asks for a human decision. Ask the user before continuing;
  they have so far chosen to continue with `--force`.
- `refill` refuses once a puzzle is approved or has `clues.json`, unless you pass `--force`. Never
  refill a puzzle whose clues exist without the user's consent: the clues would no longer match.
- `refill --new-pattern` discards the pattern and generates a new one, e.g. after a pattern rule
  changed. `python3` can check old grids against a new rule before deciding.
- Report every rejected word and every judgment call to the user, so they can veto them.

### 3. Draft clues

For every approved puzzle, write `puzzles/NNN/clue-drafts.txt` with three German drafts per word, in
the order of `puzzle.json` (`across` first, then `down`):

```
ATOME | Kleinste Bausteine der Materie | Teilchen aus Kern und Hülle | Gegenstand der Kernphysik
```

- Vary the three: a plain definition, a paraphrase, and a cryptic or associative clue.
- At most about 40 characters, so a clue fits on one line in the PDF.
- A clue must not contain the answer or a word sharing its stem (Rentier for REN, Haut for HAEUTEN,
  Hadrian for ADRIAN). Mark abbreviations "(Abk.)" and plurals "(Mz.)" where the clue doesn't show it.
- Avoid two words in one puzzle defining each other (ALT "Gegenteil von neu" next to NEU).

### 4. Clue doc for the user

Put the drafts in a Claude Docs document so the user can choose. The pilot doc is
"Zuordnungs-Kreuzwort – Hinweise Nr. 001–003". Use one doc per batch, one `## Nr. NNN` section per
puzzle, and `### Waagerecht` / `### Senkrecht` subsections in `puzzle.json` order. Each word looks like this:

```
**ATOME** (5)

- [ ] Kleinste Bausteine der Materie
- [ ] Teilchen aus Kern und Hülle
- [ ] Gegenstand der Kernphysik
- [ ] Eigener Vorschlag: …
```

The length is shown only in the doc, never in the PDF. Ticked items show struck through in the doc;
the doc's intro explains that this means "selected".

### 5. Read the choices back

When the user says they are done, read the doc and write `puzzles/NNN/clues.json`:

```json
{"across": [{"word": "ATOME", "clue": "…"}, …], "down": [{"word": "…", "clue": "…"}, …]}
```

- Same order as `puzzle.json`; the Typst build asserts that the words match.
- A ticked "Eigener Vorschlag" with text wins over a ticked draft. The user may also have edited
  draft text in place; use the text as it stands.
- If a word has no tick or more than one, don't guess: list those words for the user.

### 6. Build the PDFs

```bash
scripts/build.sh
```

This writes `puzzles/NNN/puzzle-NNN.pdf` (puzzle page and solution page) and `sammlung.pdf`. Puzzles
without `clues.json` get placeholder text, which is for layout review only and must never be
published. Render a page to PNG and look at it before handing PDFs over:

```bash
.tools/typst compile --root . --input nr=001 --format png --ppi 80 typst/einzeln.typ "/tmp/p-{p}.png"
```

### 7. Commit

Commit the puzzle directories, the updated block and approved lists and the PDFs together, and
push. `Cargo.lock` is tracked on purpose (reproducible fills).

## Other commands

- `zkw pattern --seed S --count N [--show K]`: generate N patterns and test-fill each, to measure the
  fill rate after changing pattern rules.
- `zkw index`: rebuild `puzzles/index.json`.
- `cargo test --release`: library and `zkw` tests, including golden seed values. If a seed test
  fails, existing puzzles would no longer reproduce; don't change the seed derivation.
