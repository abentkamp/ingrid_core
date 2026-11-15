// Layout for the Zuordnungs-Kreuzwort series. Puzzle data comes from zkw's puzzle.json, clues
// from clues.json (same order as puzzle.json). See PLAN.md, section 4.

#let text-font = "Libertinus Serif"
#let grid-font = "DejaVu Sans Mono"

#let setup(body) = {
  set document(title: "Zuordnungs-Kreuzwort")
  set page(paper: "a4", margin: (x: 15mm, top: 15mm, bottom: 15mm))
  set text(font: text-font, lang: "de", size: 10pt)
  set par(justify: false)
  body
}

// Puzzle number as printed: 1, not 001.
#let nr-text(p) = str(p.number)

// The grid. With `solution: true` every letter is printed; otherwise only the revealed cells.
#let puzzle-grid(p, cell: 10mm, solution: false) = {
  let revealed = (:)
  for rc in p.revealed {
    revealed.insert(str(rc.at(0) - 1) + "," + str(rc.at(1) - 1), true)
  }
  let cells = ()
  for r in range(p.height) {
    let row = p.solution.at(r).clusters()
    for c in range(p.width) {
      let ch = row.at(c)
      if ch == "#" {
        cells.push(grid.cell(fill: black)[])
      } else if solution or revealed.at(str(r) + "," + str(c), default: false) {
        cells.push(text(font: grid-font, weight: "bold", size: cell * 0.55, ch))
      } else {
        cells.push([])
      }
    }
  }
  grid(
    columns: (cell,) * p.width,
    rows: (cell,) * p.height,
    stroke: 0.6pt + black,
    align: center + horizon,
    ..cells,
  )
}

// Clue texts for one direction: from clues.json if given, otherwise placeholders of realistic
// length (for layout review only; placeholders never contain the answer).
#let clue-texts(p, clues, dir) = {
  let entries = p.at(dir)
  if clues == none {
    entries.enumerate().map(((i, e)) => lorem(3 + calc.rem(i * 7, 4)))
  } else {
    let given = clues.at(dir)
    let expected = entries.map(e => e.word)
    let got = given.map(c => c.word)
    assert(
      got == expected,
      message: "clues.json does not match puzzle.json (" + dir + "): " + repr(got) + " vs " + repr(expected),
    )
    given.map(c => c.clue)
  }
}

// Index of the hint word's clue in one direction's list, or none. Matched by position, so a word
// that happens to occur in both directions can't be confused.
#let hint-index(p, dir) = {
  let h = p.hint_word
  if h.dir != dir { return none }
  let entries = p.at(dir)
  for (i, e) in entries.enumerate() {
    if e.row == h.row and e.col == h.col { return i }
  }
  panic("hint word not found in the " + dir + " list")
}

// One direction's clues as a single column under its heading. The hint word's clue is struck
// through, as if a solver had already filled it in.
#let clue-list(title, texts, struck: none) = {
  block(below: 3mm, text(weight: "bold", size: 11pt, title))
  set par(leading: 0.45em, hanging-indent: 3mm)
  texts
    .enumerate()
    .map(((i, t)) => block(
      above: 0pt,
      below: 1.6mm,
      breakable: false,
      if i == struck { strike(stroke: 0.8pt)[– #t] } else [– #t],
    ))
    .join()
}

#let puzzle-page(p, clues: none, cell: 10mm) = {
  align(center, text(size: 18pt, weight: "bold", [#p.series Nr. #nr-text(p)]))
  v(1mm)
  align(center, text(size: 9.5pt, style: "italic", [
    Finden Sie die Lösungswörter zu den Hinweisen und finden Sie die passende Stelle im Gitter,
    um diese einzutragen.
  ]))
  v(4mm)
  align(center, puzzle-grid(p, cell: cell))
  v(7mm)
  // Waagerecht on the left, Senkrecht on the right.
  grid(
    columns: (1fr, 1fr),
    column-gutter: 8mm,
    clue-list("Waagerecht", clue-texts(p, clues, "across"), struck: hint-index(p, "across")),
    clue-list("Senkrecht", clue-texts(p, clues, "down"), struck: hint-index(p, "down")),
  )
}

#let solution-block(p, cell: 10mm) = {
  block(breakable: false, {
    align(center, text(size: 13pt, weight: "bold", [Lösung Nr. #nr-text(p)]))
    v(2mm)
    align(center, puzzle-grid(p, cell: cell, solution: true))
  })
}

#let solution-page(p, cell: 10mm) = solution-block(p, cell: cell)
