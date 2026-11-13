# Ingrid Core

This crate contains the core crossword-solving code used in the Ingrid
construction app, as well as a standalone binary that can be used to solve
grids from the command line.

### Usage

After [setting up Rust](https://rustup.rs), you can install the Ingrid Core CLI
tool with `cargo`:
```
$ cargo install ingrid_core
```

Then you just need to provide a grid as an input file:

```
$ cat example_grid.txt
....#.....#....
....#.....#....
...............
......##.......
###.....#......
............###
.....#.....#...
....#.....#....
...#.....#.....
###cremebrulees
......#.....###
.......##......
...............
....#.....#....
....#.....#....
$ ingrid_core example_grid.txt
bile#seeit#slaw
room#lasso#pone
intimateapparel
garret##whirred
###amens#easels
wisterialane###
aloes#nuevo#tnt
ssns#betty#ciao
pas#wipes#pelts
###cremebrulees
dealin#deere###
imgonna##aesops
goingintodetail
utne#anise#atta
pegs#lemur#shay
```

You can also use a custom word list (the default is [Spread the
Wordlist](https://www.spreadthewordlist.com)) or customize various other
options:
```
$ ingrid_core --help
ingrid_core: Command-line crossword generation tool

Usage: ingrid_core [OPTIONS] <GRID_PATH>

Arguments:
  <GRID_PATH>  Path to the grid file, as ASCII with # representing blocks and . representing empty squares

Options:
      --wordlist <WORDLIST>
          Path to a scored wordlist file [default: (embedded copy of Spread the Wordlist)]
      --min-score <MIN_SCORE>
          Minimum allowable word score [default: 50]
      --max-shared-substring <MAX_SHARED_SUBSTRING>
          Maximum shared substring length between entries [default: none]
  -t, --time
          Print timing information along with the grid
  -w, --print-words
          Print the list of words used in the filled grid
  -s, --shuffle-words
          Shuffle the printed word list instead of listing entries in grid order, so it can be
          used as puzzle input. Across and down stay separate, and positions are omitted
      --seed <SEED>
          Seed for the fill search; different seeds give different fills for the same grid
          [default: 0]
  -h, --help
          Print help information
  -V, --version
          Print version information
```

With `--print-words`, the entries in the finished grid are listed by direction and
position. Entries that aren't in the word list -- which can only happen when they
were pre-filled in the input grid -- are marked as such:

```
$ ingrid_core --print-words example_grid.txt
clam#altar#poem
[...]

Across:
  r1c1 clam
  r1c6 altar
  [...]
  r10c4 cremebrulees (not in word list)
  [...]

Down:
  r1c1 crop
  [...]
```

### Acknowledgments

* The backtracking search implementation in this library owes a lot to
  "Adaptive Strategies for Solving Constraint Satisfaction Problems" by
  Thanasis Balafoutis, which was helpful both as an overview of the CSP space
  and a source of specific implementation ideas.

* The CLI tool includes a copy of the free [Spread the
  Wordlist](https://www.spreadthewordlist.com) dictionary published by Brooke
  Husic and Enrique Henestroza Anguiano.
