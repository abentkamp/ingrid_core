// All puzzles from puzzles/index.json, then all solutions, three per page.
// typst compile --root . [--input clues=001,003] typst/sammlung.typ sammlung.pdf
#import "zkw.typ": *

#let index = json("/puzzles/index.json")
#let with-clues = sys.inputs.at("clues", default: "").split(",")

#show: setup

#for (i, entry) in index.puzzles.enumerate() {
  let p = json(entry.puzzle)
  let clues = if entry.dir in with-clues { json(entry.clues) } else { none }
  if i > 0 { pagebreak() }
  puzzle-page(p, clues: clues)
}

#pagebreak()
#for (i, entry) in index.puzzles.enumerate() {
  if i > 0 and calc.rem(i, 3) == 0 { pagebreak() } else if i > 0 { v(8mm) }
  solution-block(json(entry.puzzle), cell: 8mm)
}
