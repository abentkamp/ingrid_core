# German puzzle setup

Notes on how the German crosswords in this working tree were generated.

## Word list

**`wordlist.txt` from [obrhubr/deutsche-kreuzwoerter](https://github.com/obrhubr/deutsche-kreuzwoerter)**

```
https://raw.githubusercontent.com/obrhubr/deutsche-kreuzwoerter/refs/heads/master/wordlist.txt
```

351,679 entries in the same `WORD;SCORE` format as Spread the Wordlist, so it
drops straight into `--wordlist`. Pure ASCII A-Z: umlauts are written AE/OE/UE,
and the generated grids spell them the same way (SPAET, NOETIG). Lengths run
1-21, within `MAX_SLOT_LENGTH`.

Scores are heavily bottom-weighted, which is what decides the usable
`--min-score`:

| min-score | entries | 2-letter | 3-letter |
| --- | --- | --- | --- |
| 15 | 351,679 | 99 | 502 |
| 40 | 96,379 | 98 | 482 |
| 45 | 32,636 | 96 | 412 |
| 50 | 23,497 | 90 | 360 |
| 55 | 5,588 | 46 | 164 |

Note that the embedded Spread the Wordlist has **no** 2-letter entries at all,
so any grid with a 2-letter slot is unfillable with the default list.

## Command line

```sh
ingrid_core --wordlist wordlist.txt --min-score 50 --seed <N> [-w|-s] <grid>
```

* `--min-score 50` is the practical ceiling for these grids. 55 leaves too few
  words and fails outright; 45 and below work but pull in weaker fill.
* `--seed` matters because the search is otherwise deterministic — the same
  grid, word list and min-score always produce the identical fill. Seeds were
  ranked by the mean score of the resulting entries.
* `-w` / `--print-words` for the entry list in grid order with positions,
  `-s` / `--shuffle-words` for the shuffled clue list used in the puzzles.

## Grids

### 17x8 (`kreuzwort_17x8.txt`)

The grid the puzzles were built on, given as run lengths of white squares per
row: `6#5#4 / 4#4#7 / 6#5#1#2 / 2#6#1#4# / #5#1#4#3 / 3#1#5#5 / 8#4#3 / 5#5#5`.

```
......#.....#....
....#....#.......
......#.....#.#..
..#......#.#....#
#.....#.#....#...
...#.#.....#.....
........#....#...
.....#.....#.....
```

Fills: `--seed 2` (mean score 53.48, 32/46 entries at the top tier) and
`--seed 24` (53.15, 29/46), in `kreuzwort_17x8_fill.txt` and
`kreuzwort_17x8_fill2.txt`, with clue suggestions in the matching
`kreuzwort_17x8_clues*.md`.

### 18x9 (`kreuzwort_18x9.txt`) — incomplete

A partial transcription of a newspaper "Kreuzwort für Tüftler" from a photo.
The dimensions are solid and the twelve marked blocks are forced by the
answers already written into the puzzle, but the remaining black squares
weren't legible at the angle the photo was taken, so the grid is not fillable
as it stands.

```
..................
....#.............
.......#.....#....
...#......#.#....#
#.................
...#........#.....
..................
..................
..........#......#
```

`kreuzwort_18x9_demo.txt` is an unrelated representative 18x9 pattern used only
to establish that `--min-score 50` is reachable at that size.
