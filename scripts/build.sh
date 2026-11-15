#!/usr/bin/env bash
# Build puzzles/NNN/puzzle-NNN.pdf for every puzzle in puzzles/index.json, then sammlung.pdf.
# Puzzles without clues.json get placeholder clues (for layout review only).
set -euo pipefail
cd "$(dirname "$0")/.."

TYPST=${TYPST:-.tools/typst}
[ -x "$TYPST" ] || scripts/setup.sh

with_clues=()
for dir in puzzles/[0-9]*/; do
  nr=$(basename "$dir")
  [ -f "$dir/puzzle.json" ] || continue
  args=(--root . --input "nr=$nr")
  if [ -f "$dir/clues.json" ]; then
    args+=(--input clues=1)
    with_clues+=("$nr")
  fi
  "$TYPST" compile "${args[@]}" typst/einzeln.typ "$dir/puzzle-$nr.pdf"
  echo "$dir/puzzle-$nr.pdf"
done

clues_csv=$(IFS=,; echo "${with_clues[*]:-}")
"$TYPST" compile --root . --input "clues=$clues_csv" typst/sammlung.typ sammlung.pdf
echo sammlung.pdf
