// One puzzle: puzzle page, then solution page.
// typst compile --root . --input nr=001 [--input clues=1] typst/einzeln.typ puzzles/001/puzzle-001.pdf
#import "zkw.typ": *

#let nr = sys.inputs.at("nr")
#let p = json("/puzzles/" + nr + "/puzzle.json")
#let clues = if sys.inputs.at("clues", default: "0") == "1" {
  json("/puzzles/" + nr + "/clues.json")
} else { none }

#show: setup
#puzzle-page(p, clues: clues)
#pagebreak()
#solution-page(p)
