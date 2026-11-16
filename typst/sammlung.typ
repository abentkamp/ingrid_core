// All puzzles from puzzles/index.json, then all solutions, three per page.
// typst compile --root . [--input clues=001,003] [--input solutions=0] typst/sammlung.typ sammlung.pdf
// With solutions=0 the solution pages are left out.
#import "zkw.typ": *

#let index = json("/puzzles/index.json")
#let with-clues = sys.inputs.at("clues", default: "").split(",")
#let with-solutions = sys.inputs.at("solutions", default: "1") != "0"

#show: setup

#for (i, entry) in index.puzzles.enumerate() {
  let p = json(entry.puzzle)
  let clues = if entry.dir in with-clues { json(entry.clues) } else { none }
  if i > 0 { pagebreak() }
  puzzle-page(p, clues: clues)
}

#if with-solutions {
  pagebreak()
  for (i, entry) in index.puzzles.enumerate() {
    if i > 0 and calc.rem(i, 3) == 0 { pagebreak() } else if i > 0 { v(8mm) }
    solution-block(json(entry.puzzle), cell: 8mm)
  }
}
