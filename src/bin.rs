use clap::Parser;
use ingrid_core::backtracking_search::find_fill_with_seed;
use ingrid_core::grid_config::{
    generate_grid_config_from_template_string, grid_entries, render_grid, Direction,
};
use ingrid_core::word_list::{WordList, WordListSourceConfig, WordListSourceConfigProvider};
use rand::prelude::*;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

const STWL_RAW: &str = include_str!("../resources/spreadthewordlist.dict");

/// ingrid_core: Command-line crossword generation tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the grid file, as ASCII with # representing blocks and . representing empty squares
    grid_path: String,

    /// Path to a scored wordlist file [default: (embedded copy of Spread the Wordlist)]
    #[arg(long)]
    wordlist: Option<String>,

    /// Minimum allowable word score
    #[arg(long, default_value_t = 50)]
    min_score: u16,

    /// Maximum shared substring length between entries [default: none]
    #[arg(long)]
    max_shared_substring: Option<usize>,

    /// Print timing information along with the grid
    #[arg(short, long, default_value_t = false)]
    time: bool,

    /// Print the list of words used in the filled grid
    #[arg(short = 'w', long, default_value_t = false)]
    print_words: bool,

    /// Shuffle the printed word list instead of listing entries in grid order, so it can be used
    /// as puzzle input. Across and down stay separate, and positions are omitted.
    #[arg(short = 's', long, default_value_t = false)]
    shuffle_words: bool,

    /// Seed for the fill search; different seeds give different fills for the same grid
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

struct Error(String);

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0) // Print error unquoted
    }
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let raw_grid_content = fs::read_to_string(&args.grid_path)
        .map_err(|_| Error(format!("Couldn't read file '{}'", args.grid_path)))?
        .trim()
        .lines()
        .map(|line| line.trim().to_lowercase().nfc().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    let height = raw_grid_content.lines().count();

    if height == 0 {
        return Err(Error("Grid must have at least one row".into()));
    }

    if raw_grid_content
        .lines()
        .map(|line| line.chars().count())
        .collect::<HashSet<_>>()
        .len()
        != 1
    {
        return Err(Error("Rows in grid must all be the same length".into()));
    }

    // `lines()` doesn't include the line terminators, so this is the true width. Getting it wrong
    // matters because `max_side` bounds the lengths of the words we load into the word list.
    let width = raw_grid_content.lines().next().unwrap().chars().count();
    let max_side = width.max(height);

    if !args
        .max_shared_substring
        .map_or(true, |mss| (3..=10).contains(&mss))
    {
        return Err(Error(
            "If given, max shared substring must be between 3 and 10".into(),
        ));
    }

    let start = Instant::now();

    let word_list = WordList::new(
        vec![match args.wordlist {
            Some(wordlist_path) => WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::File {
                    path: wordlist_path.into(),
                },
                normalization: None,
            },
            None => WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::FileContents { contents: STWL_RAW },
                normalization: None,
            },
        }],
        None,
        Some(max_side),
        args.max_shared_substring,
    );

    let word_list_time = start.elapsed();

    #[allow(clippy::comparison_chain)]
    if let Some(errors) = word_list.get_source_errors().get("0") {
        if errors.len() == 1 {
            return Err(Error(format!("{}", errors[0])));
        } else if errors.len() > 1 {
            let mut full_error: String = "".into();
            for error in errors {
                full_error.push_str(&format!("\n- {error}"));
            }
            return Err(Error(full_error));
        }
    }

    if word_list.word_id_by_string.is_empty() {
        return Err(Error("Word list is empty".into()));
    }

    let grid_config =
        generate_grid_config_from_template_string(word_list, &raw_grid_content, args.min_score);

    let result = find_fill_with_seed(&grid_config.to_config_ref(), None, None, args.seed)
        .map_err(|_| Error("Unfillable grid".into()))?;

    let fill_time = start.elapsed() - word_list_time;

    println!(
        "{}",
        render_grid(&grid_config.to_config_ref(), &result.choices).replace('.', "#")
    );

    if args.print_words || args.shuffle_words {
        let entries = grid_entries(&grid_config.to_config_ref(), &result.choices);

        for direction in [Direction::Across, Direction::Down] {
            let mut group: Vec<_> = entries
                .iter()
                .filter(|entry| entry.direction == direction)
                .collect();

            if args.shuffle_words {
                // Shuffling each direction separately keeps the two clue lists apart, the way a
                // "find the right place for the solutions" puzzle presents them.
                let mut rng: SmallRng = SeedableRng::seed_from_u64(args.seed);
                group.shuffle(&mut rng);
            }

            println!(
                "\n{}:",
                match direction {
                    Direction::Across => "Across",
                    Direction::Down => "Down",
                }
            );

            for entry in group {
                let word = entry.normalized_string.as_deref().unwrap_or("?");
                let note = if entry.is_in_word_list() {
                    ""
                } else {
                    " (not in word list)"
                };

                if args.shuffle_words {
                    // Positions would give the placement away, which is the point of the puzzle.
                    println!("  {word}{note}");
                } else {
                    // Coords are one-indexed row and column, to match the rendered grid.
                    let (x, y) = entry.start_cell;
                    println!("  r{}c{} {}{}", y + 1, x + 1, word, note);
                }
            }
        }
    }

    if args.time {
        eprintln!("{word_list_time:?} loading word list, {fill_time:?} finding fill");
    }

    Ok(())
}
