//! zkw: generator for the "Zuordnungs-Kreuzwort" puzzle series.
//!
//! Generates block patterns, fills them with Ingrid, chooses the hints (one given word plus every
//! occurrence of one letter), shuffles the across/down entries, and writes one `puzzle.json` per
//! puzzle. Also supports the word-check workflow: listing unchecked words, approving a puzzle,
//! and refilling a puzzle after words were added to the blocklist. See PLAN.md.

use clap::{Args, Parser, Subcommand};
use ingrid_core::backtracking_search::{find_fill_capped, FillFailure};
use ingrid_core::grid_config::{
    generate_grid_config_from_template_string, grid_entries, Direction, OwnedGridConfig,
};
use ingrid_core::pattern::{generate_pattern, Pattern, PatternConfig};
use ingrid_core::word_list::{WordList, WordListSourceConfig, WordListSourceConfigProvider};
use rand::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const SERIES: &str = "Zuordnungs-Kreuzwort";

/// Stream ids for seed derivation, so each random decision has its own independent seed.
const STREAM_PATTERN: u64 = 1;
const STREAM_FILL: u64 = 2;
const STREAM_HINTS: u64 = 3;
const STREAM_SHUFFLE: u64 = 4;

/// Refills allowed per puzzle before `refill` asks for a human decision.
const MAX_REFILL_ROUNDS: u64 = 10;

#[derive(Parser, Debug)]
#[command(about = "Generator for the Zuordnungs-Kreuzwort puzzle series")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Generate new puzzles
    Generate {
        /// Number of puzzles to generate
        #[arg(long, default_value_t = 1)]
        count: u64,
        /// Number of the first puzzle
        #[arg(long, default_value_t = 1)]
        start: u64,
        /// Base seed of the series
        #[arg(long)]
        seed: u64,
        /// Directory holding the puzzle directories
        #[arg(long, default_value = "puzzles")]
        out: PathBuf,
        /// Overwrite existing puzzle directories
        #[arg(long)]
        force: bool,
        #[command(flatten)]
        opts: FillOpts,
    },
    /// Refill a puzzle's pattern after the blocklist changed
    Refill {
        /// Puzzle directory, e.g. puzzles/002
        dir: PathBuf,
        /// Refill even if the words were approved, clues exist, or the round cap is reached
        #[arg(long)]
        force: bool,
        /// Discard the current pattern and generate a new one (e.g. after a pattern rule changed)
        #[arg(long)]
        new_pattern: bool,
        #[command(flatten)]
        opts: FillOpts,
    },
    /// List a puzzle's words that are neither approved nor blocked yet
    Unchecked {
        /// Puzzle directories
        dirs: Vec<PathBuf>,
        #[command(flatten)]
        lists: ListOpts,
    },
    /// Mark a puzzle's words as checked and add them to the approved list
    Approve {
        /// Puzzle directory
        dir: PathBuf,
        #[command(flatten)]
        lists: ListOpts,
    },
    /// Generate patterns and test-fill them, for measuring the fill success rate
    Pattern {
        /// Base seed
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Number of patterns
        #[arg(long, default_value_t = 1)]
        count: u64,
        /// Print the patterns (and fills) of the first N
        #[arg(long, default_value_t = 0)]
        show: usize,
        #[command(flatten)]
        opts: FillOpts,
    },
    /// Rebuild puzzles/index.json from the puzzle directories
    Index {
        #[arg(long, default_value = "puzzles")]
        out: PathBuf,
    },
}

#[derive(Args, Debug, Clone)]
struct ListOpts {
    /// Words that break the rules, one per line, `#` starts a comment
    #[arg(long, default_value = "german-blocklist.txt")]
    blocklist: PathBuf,
    /// Words that were checked and accepted
    #[arg(long, default_value = "german-approved.txt")]
    approved: PathBuf,
}

#[derive(Args, Debug, Clone)]
struct FillOpts {
    /// Scored word list (WORD;SCORE per line)
    #[arg(long, default_value = "wordlist.txt")]
    wordlist: PathBuf,
    #[command(flatten)]
    lists: ListOpts,
    /// Minimum word score
    #[arg(long, default_value_t = 50)]
    min_score: u16,
    /// Minimum score for short words (up to --short-len letters), which are scarce
    #[arg(long, default_value_t = 40)]
    short_min_score: u16,
    /// Words up to this length count as short
    #[arg(long, default_value_t = 3)]
    short_len: usize,
    /// Extra scored words (WORD;SCORE) merged into the word list; the higher score wins
    #[arg(long, default_value = "german-extra-words.txt")]
    extra_words: PathBuf,
    /// Maximum entry length
    #[arg(long, default_value_t = 10)]
    max_len: usize,
    /// Minimum number of blocks
    #[arg(long, default_value_t = 20)]
    min_blocks: usize,
    /// Maximum number of blocks
    #[arg(long, default_value_t = 24)]
    max_blocks: usize,
    /// Fill tries per pattern (each try allows ~10% more backtracks than the last, from 500)
    #[arg(long, default_value_t = 30)]
    fill_tries: usize,
    /// Pattern candidates per puzzle before giving up
    #[arg(long, default_value_t = 30)]
    max_patterns: u64,
    /// Safety net per fill in seconds: hitting it aborts with an error instead of moving on
    #[arg(long, default_value_t = 600)]
    safety_timeout: u64,
    /// Allow words that already appear in other puzzles of the series
    #[arg(long)]
    allow_repeats: bool,
}

struct Error(String);

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error(message)
    }
}

type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------------------------
// Seeds and hashes (explicit algorithms, so values are stable across Rust and crate versions).

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derive the seed for one random stream of one puzzle attempt.
fn derive_seed(base_seed: u64, number: u64, stream: u64, attempt: u64) -> u64 {
    [base_seed, number, stream, attempt]
        .iter()
        .fold(0x5A4B_5700_0000_0001, |acc, &part| {
            splitmix64(acc ^ splitmix64(part))
        })
}

/// 64-bit FNV-1a, as a hex string.
fn fnv1a(chunks: &[&[u8]]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for chunk in chunks {
        for &byte in *chunk {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // Separator, so that chunk boundaries matter.
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

// ---------------------------------------------------------------------------------------------
// Word lists.

/// Read a word-per-line file (blocklist or approved list), lowercased. Missing files are empty.
fn read_word_set(path: &Path) -> Result<BTreeSet<String>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(Error(format!("Couldn't read '{}': {err}", path.display()))),
    };
    Ok(contents
        .lines()
        .map(|line| line.split('#').next().unwrap().trim().to_lowercase())
        .filter(|word| !word.is_empty())
        .collect())
}

#[derive(Clone)]
struct LoadedWords {
    /// Words usable as fill: score >= min_score, length <= max_len, not blocked.
    words: Vec<(String, u16)>,
    /// Hash of word list plus blocklist file contents.
    hash: String,
}

fn load_words(opts: &FillOpts) -> Result<LoadedWords> {
    let raw = fs::read(&opts.wordlist)
        .map_err(|err| Error(format!("Couldn't read '{}': {err}", opts.wordlist.display())))?;
    let extra_raw = fs::read(&opts.extra_words).unwrap_or_default();
    let blocklist_raw = fs::read(&opts.lists.blocklist).unwrap_or_default();
    let blocklist = read_word_set(&opts.lists.blocklist)?;

    // Word list first, in file order; extra words are merged in (higher score wins) or appended,
    // so the order of existing words stays stable.
    let mut entries: Vec<(String, u16)> = vec![];
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for text in [String::from_utf8_lossy(&raw), String::from_utf8_lossy(&extra_raw)] {
        for line in text.lines() {
            let line = line.split('#').next().unwrap().trim();
            let Some((word, score)) = line.rsplit_once(';') else {
                continue;
            };
            let Ok(score) = score.trim().parse::<u16>() else {
                continue;
            };
            let word = word.trim().to_uppercase();
            if word.is_empty() {
                continue;
            }
            match index.get(&word) {
                Some(&i) => entries[i].1 = entries[i].1.max(score),
                None => {
                    index.insert(word.clone(), entries.len());
                    entries.push((word, score));
                }
            }
        }
    }

    let words: Vec<(String, u16)> = entries
        .into_iter()
        .filter(|(word, score)| {
            let len = word.chars().count();
            let min_score = if len <= opts.short_len {
                opts.short_min_score.min(opts.min_score)
            } else {
                opts.min_score
            };
            *score >= min_score && len <= opts.max_len && !blocklist.contains(&word.to_lowercase())
        })
        .collect();
    if words.is_empty() {
        return Err(Error("No usable words in word list".into()));
    }

    Ok(LoadedWords {
        words,
        hash: fnv1a(&[&raw, &extra_raw, &blocklist_raw]),
    })
}

/// Words used in the other puzzles of the series (every puzzle directory in `out` except
/// number `number`), lowercased.
fn words_in_other_puzzles(out: &Path, number: u64) -> Result<BTreeSet<String>> {
    let mut used = BTreeSet::new();
    let Ok(read_dir) = fs::read_dir(out) else {
        return Ok(used);
    };
    for dir_entry in read_dir.flatten() {
        let Ok(puzzle) = read_puzzle(&dir_entry.path()) else {
            continue;
        };
        if puzzle["number"].as_u64() == Some(number) {
            continue;
        }
        used.extend(puzzle_words(&puzzle).iter().map(|w| w.to_lowercase()));
    }
    Ok(used)
}

/// The word list for puzzle `number`: without words used in other puzzles of the series (unless
/// `--allow-repeats`), so no word repeats across puzzles. Returns the list and the number of
/// distinct words excluded.
fn words_for_puzzle(
    words: &LoadedWords,
    out: &Path,
    number: u64,
    opts: &FillOpts,
) -> Result<(LoadedWords, usize)> {
    if opts.allow_repeats {
        return Ok((words.clone(), 0));
    }
    let used = words_in_other_puzzles(out, number)?;
    let mut words = words.clone();
    words
        .words
        .retain(|(word, _)| !used.contains(&word.to_lowercase()));
    let used_text: String = used.iter().map(|w| format!("{w}\n")).collect();
    words.hash = fnv1a(&[words.hash.as_bytes(), used_text.as_bytes()]);
    Ok((words, used.len()))
}

/// The last commit touching the blocklist, with "+dirty" if it has uncommitted changes.
fn blocklist_commit(path: &Path) -> Value {
    let commit = Command::new("git")
        .args(["log", "-1", "--format=%H", "--"])
        .arg(path)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
    let Some(commit) = commit.filter(|commit| !commit.is_empty()) else {
        return Value::Null;
    };
    let dirty = Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .arg(path)
        .status()
        .map_or(false, |status| !status.success());
    json!(if dirty { format!("{commit}+dirty") } else { commit })
}

fn build_grid_config(words: &[(String, u16)], template: &str, opts: &FillOpts) -> OwnedGridConfig {
    let word_list = WordList::new(
        vec![WordListSourceConfig {
            id: "0".into(),
            enabled: true,
            provider: WordListSourceConfigProvider::Memory {
                words: words.to_vec(),
            },
            normalization: None,
        }],
        None,
        Some(opts.max_len),
        None,
    );
    generate_grid_config_from_template_string(
        word_list,
        template,
        opts.min_score.min(opts.short_min_score),
    )
}

// ---------------------------------------------------------------------------------------------
// Filling.

struct Entry {
    word: String,
    row: usize,
    col: usize,
    dir: Direction,
    score: u16,
}

impl Entry {
    fn cells(&self) -> Vec<(usize, usize)> {
        (0..self.word.chars().count())
            .map(|i| match self.dir {
                Direction::Across => (self.row, self.col + i),
                Direction::Down => (self.row + i, self.col),
            })
            .collect()
    }

    fn to_json(&self) -> Value {
        json!({
            "word": self.word,
            "row": self.row + 1,
            "col": self.col + 1,
            "len": self.word.chars().count(),
            "score": self.score,
        })
    }
}

struct Fill {
    entries: Vec<Entry>,
    solution: Vec<String>,
}

enum FillOutcome {
    Filled(Fill, Duration),
    Unfillable(Duration),
}

fn try_fill(
    words: &[(String, u16)],
    pattern: &Pattern,
    fill_seed: u64,
    opts: &FillOpts,
) -> Result<FillOutcome> {
    let start = Instant::now();
    let grid_config = build_grid_config(words, &pattern.to_template(), opts);
    let config = grid_config.to_config_ref();
    match find_fill_capped(
        &config,
        fill_seed,
        opts.fill_tries,
        Some(Duration::from_secs(opts.safety_timeout)),
    ) {
        Ok(success) => {
            let mut solution: Vec<Vec<char>> = pattern
                .blocks
                .chunks(pattern.width)
                .map(|row| row.iter().map(|&b| if b { '#' } else { '?' }).collect())
                .collect();
            let mut entries = vec![];
            for entry in grid_entries(&config, &success.choices) {
                let word = entry
                    .normalized_string
                    .ok_or_else(|| Error("Fill left an entry incomplete".into()))?
                    .to_uppercase();
                let (col, row) = entry.start_cell;
                let entry = Entry {
                    word,
                    row,
                    col,
                    dir: entry.direction,
                    score: entry.score.unwrap_or(0),
                };
                for ((r, c), ch) in entry.cells().into_iter().zip(entry.word.chars()) {
                    solution[r][c] = ch;
                }
                entries.push(entry);
            }
            Ok(FillOutcome::Filled(
                Fill {
                    entries,
                    solution: solution.into_iter().map(String::from_iter).collect(),
                },
                start.elapsed(),
            ))
        }
        Err(FillFailure::Timeout) => Err(Error(format!(
            "Fill hit the {}s safety timeout; raise --safety-timeout or lower --fill-tries",
            opts.safety_timeout
        ))),
        Err(_) => Ok(FillOutcome::Unfillable(start.elapsed())),
    }
}

fn pattern_config(opts: &FillOpts) -> PatternConfig {
    PatternConfig {
        min_blocks: opts.min_blocks,
        max_blocks: opts.max_blocks,
        max_len: opts.max_len,
        ..PatternConfig::like_original_17x8()
    }
}

fn make_pattern(base_seed: u64, number: u64, attempt: u64, opts: &FillOpts) -> Result<Pattern> {
    let mut rng = SmallRng::seed_from_u64(derive_seed(base_seed, number, STREAM_PATTERN, attempt));
    generate_pattern(&pattern_config(opts), &mut rng)
        .ok_or_else(|| Error("Pattern generator found no pattern meeting the targets".into()))
}

// ---------------------------------------------------------------------------------------------
// Puzzle assembly.

struct Hints {
    word: usize,
    letter: char,
    revealed: BTreeSet<(usize, usize)>,
}

fn choose_hints(fill: &Fill, seed: u64) -> Result<Hints> {
    let mut rng = SmallRng::seed_from_u64(seed);

    let candidates: Vec<usize> = (0..fill.entries.len())
        .filter(|&i| fill.entries[i].word.chars().count() >= 5)
        .collect();
    let word = *candidates
        .choose(&mut rng)
        .ok_or_else(|| Error("No entry with 5 or more letters".into()))?;

    // Count each cell once.
    let mut counts: BTreeMap<char, u32> = BTreeMap::new();
    for row in &fill.solution {
        for ch in row.chars().filter(|&ch| ch != '#') {
            *counts.entry(ch).or_default() += 1;
        }
    }
    let mut by_frequency: Vec<(char, u32)> = counts.into_iter().collect();
    by_frequency.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let top: Vec<char> = by_frequency.iter().take(4).map(|&(ch, _)| ch).collect();
    let letter = *top.choose(&mut rng).unwrap();

    let mut revealed: BTreeSet<(usize, usize)> = fill.entries[word].cells().into_iter().collect();
    for (r, row) in fill.solution.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == letter {
                revealed.insert((r, c));
            }
        }
    }

    Ok(Hints {
        word,
        letter,
        revealed,
    })
}

struct PuzzleMeta {
    number: u64,
    base_seed: u64,
    pattern_attempt: u64,
    refill_round: u64,
}

fn puzzle_json(
    meta: &PuzzleMeta,
    pattern: &Pattern,
    fill: &Fill,
    fill_seed: u64,
    words: &LoadedWords,
    opts: &FillOpts,
) -> Result<Value> {
    let hints = choose_hints(
        fill,
        derive_seed(meta.base_seed, meta.number, STREAM_HINTS, meta.pattern_attempt),
    )?;
    let hint_entry = &fill.entries[hints.word];

    let mut rng = SmallRng::seed_from_u64(derive_seed(
        meta.base_seed,
        meta.number,
        STREAM_SHUFFLE,
        meta.pattern_attempt,
    ));
    let mut across: Vec<&Entry> = fill
        .entries
        .iter()
        .filter(|e| e.dir == Direction::Across)
        .collect();
    let mut down: Vec<&Entry> = fill
        .entries
        .iter()
        .filter(|e| e.dir == Direction::Down)
        .collect();
    across.shuffle(&mut rng);
    down.shuffle(&mut rng);

    let solution_text = fill.solution.join("\n");
    Ok(json!({
        "series": SERIES,
        "number": meta.number,
        "base_seed": meta.base_seed,
        "pattern_attempt": meta.pattern_attempt,
        "fill_seed": fill_seed,
        "refill_round": meta.refill_round,
        "blocklist_commit": blocklist_commit(&opts.lists.blocklist),
        "wordlist_hash": words.hash,
        "words_checked": false,
        "solution_hash": fnv1a(&[solution_text.as_bytes()]),
        "settings": {
            "min_score": opts.min_score,
            "short_min_score": opts.short_min_score,
            "short_len": opts.short_len,
            "max_len": opts.max_len,
            "min_blocks": opts.min_blocks,
            "max_blocks": opts.max_blocks,
            "fill_tries": opts.fill_tries,
            "require_crossed_two_letter_entries": true,
        },
        "width": pattern.width,
        "height": pattern.height,
        "pattern": pattern.to_template().lines().collect::<Vec<_>>(),
        "solution": fill.solution,
        "hint_word": {
            "word": hint_entry.word,
            "row": hint_entry.row + 1,
            "col": hint_entry.col + 1,
            "dir": match hint_entry.dir { Direction::Across => "across", Direction::Down => "down" },
        },
        "hint_letter": hints.letter.to_string(),
        "revealed_count": hints.revealed.len(),
        "revealed": hints.revealed.iter().map(|&(r, c)| json!([r + 1, c + 1])).collect::<Vec<_>>(),
        "across": across.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
        "down": down.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
    }))
}

/// Starting at `first_attempt`, generate pattern candidates until one fills.
fn find_puzzle(
    base_seed: u64,
    number: u64,
    first_attempt: u64,
    words: &LoadedWords,
    opts: &FillOpts,
) -> Result<(u64, Pattern, Fill, u64)> {
    for attempt in first_attempt..first_attempt + opts.max_patterns {
        let pattern = make_pattern(base_seed, number, attempt, opts)?;
        let fill_seed = derive_seed(base_seed, number, STREAM_FILL, attempt);
        match try_fill(&words.words, &pattern, fill_seed, opts)? {
            FillOutcome::Filled(fill, elapsed) => {
                eprintln!("  Nr. {number}: pattern attempt {attempt} filled in {elapsed:.2?}");
                return Ok((attempt, pattern, fill, fill_seed));
            }
            FillOutcome::Unfillable(elapsed) => {
                eprintln!("  Nr. {number}: pattern attempt {attempt} unfillable ({elapsed:.2?})");
            }
        }
    }
    Err(Error(format!(
        "Nr. {number}: no fillable pattern in {} candidates",
        opts.max_patterns
    )))
}

fn write_puzzle(dir: &Path, pattern: &Pattern, puzzle: &Value) -> Result<()> {
    fs::create_dir_all(dir)
        .map_err(|err| Error(format!("Couldn't create '{}': {err}", dir.display())))?;
    fs::write(dir.join("grid.txt"), pattern.to_template())
        .map_err(|err| Error(format!("Couldn't write grid.txt: {err}")))?;
    let text = serde_json::to_string_pretty(puzzle).unwrap() + "\n";
    fs::write(dir.join("puzzle.json"), text)
        .map_err(|err| Error(format!("Couldn't write puzzle.json: {err}")))?;
    Ok(())
}

fn read_puzzle(dir: &Path) -> Result<Value> {
    let path = dir.join("puzzle.json");
    let text = fs::read_to_string(&path)
        .map_err(|err| Error(format!("Couldn't read '{}': {err}", path.display())))?;
    serde_json::from_str(&text).map_err(|err| Error(format!("Invalid puzzle.json: {err}")))
}

fn puzzle_words(puzzle: &Value) -> Vec<String> {
    ["across", "down"]
        .iter()
        .flat_map(|dir| puzzle[dir].as_array().cloned().unwrap_or_default())
        .filter_map(|entry| entry["word"].as_str().map(str::to_string))
        .collect()
}

fn rebuild_index(out: &Path) -> Result<()> {
    let mut puzzles = vec![];
    if let Ok(read_dir) = fs::read_dir(out) {
        for dir_entry in read_dir.flatten() {
            let dir = dir_entry.path();
            if let Ok(puzzle) = read_puzzle(&dir) {
                if let Some(number) = puzzle["number"].as_u64() {
                    let name = dir.file_name().unwrap().to_string_lossy().to_string();
                    puzzles.push((number, name));
                }
            }
        }
    }
    puzzles.sort();
    let index: Vec<Value> = puzzles
        .iter()
        .map(|(number, name)| {
            json!({
                "number": number,
                "dir": name,
                "puzzle": format!("/{}/{name}/puzzle.json", out.display()),
                "clues": format!("/{}/{name}/clues.json", out.display()),
            })
        })
        .collect();
    fs::create_dir_all(out).map_err(|err| Error(format!("Couldn't create output dir: {err}")))?;
    fs::write(
        out.join("index.json"),
        serde_json::to_string_pretty(&json!({ "series": SERIES, "puzzles": index })).unwrap()
            + "\n",
    )
    .map_err(|err| Error(format!("Couldn't write index.json: {err}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Commands.

fn cmd_generate(
    count: u64,
    start: u64,
    seed: u64,
    out: &Path,
    force: bool,
    opts: &FillOpts,
) -> Result<()> {
    let all_words = load_words(opts)?;
    for number in start..start + count {
        let dir = out.join(format!("{number:03}"));
        if dir.join("puzzle.json").exists() && !force {
            return Err(Error(format!(
                "'{}' already exists; use --force or refill",
                dir.display()
            )));
        }
        // Recomputed per puzzle, so puzzles generated earlier in this run count too.
        let (words, repeats_excluded) = words_for_puzzle(&all_words, out, number, opts)?;
        let (attempt, pattern, fill, fill_seed) = find_puzzle(seed, number, 0, &words, opts)?;
        let meta = PuzzleMeta {
            number,
            base_seed: seed,
            pattern_attempt: attempt,
            refill_round: 0,
        };
        let mut puzzle = puzzle_json(&meta, &pattern, &fill, fill_seed, &words, opts)?;
        puzzle["repeats_excluded"] = json!(repeats_excluded);
        write_puzzle(&dir, &pattern, &puzzle)?;
        println!("{}", dir.display());
    }
    rebuild_index(out)
}

fn cmd_refill(dir: &Path, force: bool, new_pattern: bool, opts: &FillOpts) -> Result<()> {
    let old = read_puzzle(dir)?;
    if !force {
        if old["words_checked"].as_bool() == Some(true) {
            return Err(Error("Words are already approved; use --force to refill anyway".into()));
        }
        if dir.join("clues.json").exists() {
            return Err(Error("clues.json exists; use --force to refill anyway".into()));
        }
        if old["refill_round"].as_u64().unwrap_or(0) >= MAX_REFILL_ROUNDS {
            return Err(Error(format!(
                "Reached {MAX_REFILL_ROUNDS} refill rounds; decide by hand, or use --force"
            )));
        }
    }
    let number = old["number"].as_u64().ok_or("puzzle.json lacks number".to_string())?;
    let base_seed = old["base_seed"]
        .as_u64()
        .ok_or("puzzle.json lacks base_seed".to_string())?;
    let old_attempt = old["pattern_attempt"].as_u64().unwrap_or(0);
    let refill_round = old["refill_round"].as_u64().unwrap_or(0) + 1;

    let out = dir.parent().unwrap_or(Path::new("."));
    let (mut words, repeats_excluded) = words_for_puzzle(&load_words(opts)?, out, number, opts)?;

    // Words excluded for this puzzle only, e.g. one half of a variant pair like GEN/GENE that the
    // dupe check doesn't catch. They stay usable in other puzzles.
    let exclude_path = dir.join("exclude.txt");
    let excluded = read_word_set(&exclude_path)?;
    if !excluded.is_empty() {
        words
            .words
            .retain(|(word, _)| !excluded.contains(&word.to_lowercase()));
        let exclude_raw = fs::read(&exclude_path).unwrap_or_default();
        words.hash = fnv1a(&[words.hash.as_bytes(), &exclude_raw]);
    }

    let template = fs::read_to_string(dir.join("grid.txt"))
        .map_err(|err| Error(format!("Couldn't read grid.txt: {err}")))?;
    let pattern = Pattern::from_template(&template)?;
    let fill_seed = derive_seed(base_seed, number, STREAM_FILL, old_attempt);

    let (attempt, pattern, fill, fill_seed) = if new_pattern {
        eprintln!("  Nr. {number}: generating a new pattern");
        find_puzzle(base_seed, number, old_attempt + 1, &words, opts)?
    } else {
        match try_fill(&words.words, &pattern, fill_seed, opts)? {
            FillOutcome::Filled(fill, elapsed) => {
                eprintln!("  Nr. {number}: same pattern refilled in {elapsed:.2?}");
                (old_attempt, pattern, fill, fill_seed)
            }
            FillOutcome::Unfillable(_) => {
                eprintln!("  Nr. {number}: pattern no longer fills, generating a new one");
                find_puzzle(base_seed, number, old_attempt + 1, &words, opts)?
            }
        }
    };

    let meta = PuzzleMeta {
        number,
        base_seed,
        pattern_attempt: attempt,
        refill_round,
    };
    let mut puzzle = puzzle_json(&meta, &pattern, &fill, fill_seed, &words, opts)?;
    puzzle["repeats_excluded"] = json!(repeats_excluded);
    if !excluded.is_empty() {
        puzzle["excluded"] = json!(excluded
            .iter()
            .map(|w| w.to_uppercase())
            .collect::<Vec<_>>());
    }
    write_puzzle(dir, &pattern, &puzzle)?;
    println!("{} (round {refill_round})", dir.display());
    rebuild_index(dir.parent().unwrap_or(Path::new(".")))
}

fn cmd_unchecked(dirs: &[PathBuf], lists: &ListOpts) -> Result<()> {
    let blocklist = read_word_set(&lists.blocklist)?;
    let approved = read_word_set(&lists.approved)?;
    for dir in dirs {
        let puzzle = read_puzzle(dir)?;
        let words = puzzle_words(&puzzle);
        let blocked: Vec<&String> = words
            .iter()
            .filter(|w| blocklist.contains(&w.to_lowercase()))
            .collect();
        let unchecked: BTreeSet<&String> = words
            .iter()
            .filter(|w| {
                let w = w.to_lowercase();
                !approved.contains(&w) && !blocklist.contains(&w)
            })
            .collect();
        println!(
            "{} ({} words, {} unchecked{}):",
            dir.display(),
            words.len(),
            unchecked.len(),
            if blocked.is_empty() {
                String::new()
            } else {
                format!(", BLOCKED: {}", blocked.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "))
            }
        );
        println!("  {}", unchecked.into_iter().cloned().collect::<Vec<_>>().join(" "));
    }
    Ok(())
}

fn cmd_approve(dir: &Path, lists: &ListOpts) -> Result<()> {
    let mut puzzle = read_puzzle(dir)?;
    let blocklist = read_word_set(&lists.blocklist)?;
    let words = puzzle_words(&puzzle);
    let blocked: Vec<&String> = words
        .iter()
        .filter(|w| blocklist.contains(&w.to_lowercase()))
        .collect();
    if !blocked.is_empty() {
        return Err(Error(format!(
            "Puzzle contains blocked words: {}; refill it first",
            blocked.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
        )));
    }

    let approved = read_word_set(&lists.approved)?;
    let new: BTreeSet<String> = words
        .iter()
        .map(|w| w.to_lowercase())
        .filter(|w| !approved.contains(w))
        .collect();
    if !new.is_empty() {
        let mut all: BTreeSet<String> = approved;
        all.extend(new.iter().cloned());
        let text: String = all.iter().map(|w| w.to_uppercase() + "\n").collect();
        fs::write(&lists.approved, text)
            .map_err(|err| Error(format!("Couldn't write approved list: {err}")))?;
    }

    puzzle["words_checked"] = json!(true);
    fs::write(
        dir.join("puzzle.json"),
        serde_json::to_string_pretty(&puzzle).unwrap() + "\n",
    )
    .map_err(|err| Error(format!("Couldn't write puzzle.json: {err}")))?;
    println!(
        "{}: approved, {} new words added: {}",
        dir.display(),
        new.len(),
        new.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join(" ")
    );
    Ok(())
}

fn cmd_pattern(seed: u64, count: u64, show: usize, opts: &FillOpts) -> Result<()> {
    let words = load_words(opts)?;
    let config = pattern_config(opts);
    let mut successes = 0;
    let mut since_last_success = Duration::ZERO;
    let mut per_puzzle: Vec<Duration> = vec![];
    let total = Instant::now();

    for i in 0..count {
        let generated = Instant::now();
        let pattern = make_pattern(seed, i, 0, opts)?;
        let generation_time = generated.elapsed();
        let stats = pattern.stats(config.max_len, config.long_threshold);
        let fill_seed = derive_seed(seed, i, STREAM_FILL, 0);
        let outcome = try_fill(&words.words, &pattern, fill_seed, opts)?;
        let (filled, elapsed) = match &outcome {
            FillOutcome::Filled(_, elapsed) => (true, *elapsed),
            FillOutcome::Unfillable(elapsed) => (false, *elapsed),
        };
        since_last_success += generation_time + elapsed;
        if filled {
            successes += 1;
            per_puzzle.push(since_last_success);
            since_last_success = Duration::ZERO;
        }
        println!(
            "#{i:>3} blocks {:>2} entries {:>2} 2-letter {} long {} longest {:>2} single-cells {:>2}  {} {:.2?}",
            stats.blocks,
            stats.entries,
            stats.two_letter_entries,
            stats.long_entries,
            stats.longest_entry,
            stats.single_entry_cells,
            if filled { "FILLED    " } else { "unfillable" },
            elapsed,
        );
        if (i as usize) < show {
            match &outcome {
                FillOutcome::Filled(fill, _) => println!("{}\n", fill.solution.join("\n")),
                FillOutcome::Unfillable(_) => println!("{}", pattern.to_template()),
            }
        }
    }

    per_puzzle.sort();
    let median = per_puzzle.get(per_puzzle.len() / 2).copied();
    println!(
        "\n{successes}/{count} filled ({:.0}%), median time per successful puzzle incl. failed candidates: {}, total {:.1?}",
        100.0 * successes as f64 / count.max(1) as f64,
        median.map_or("-".into(), |m| format!("{m:.2?}")),
        total.elapsed()
    );
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Cmd::Generate {
            count,
            start,
            seed,
            out,
            force,
            opts,
        } => cmd_generate(*count, *start, *seed, out, *force, opts),
        Cmd::Refill {
            dir,
            force,
            new_pattern,
            opts,
        } => cmd_refill(dir, *force, *new_pattern, opts),
        Cmd::Unchecked { dirs, lists } => cmd_unchecked(dirs, lists),
        Cmd::Approve { dir, lists } => cmd_approve(dir, lists),
        Cmd::Pattern {
            seed,
            count,
            show,
            opts,
        } => cmd_pattern(*seed, *count, *show, opts),
        Cmd::Index { out } => rebuild_index(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_reference_values() {
        // First outputs of the reference SplitMix64 generator seeded with 0.
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(
            splitmix64(0x9E37_79B9_7F4A_7C15),
            0x6E78_9E6A_A1B9_65F4
        );
    }

    #[test]
    fn derived_seeds_are_stable_and_distinct() {
        // Golden value: changing it would silently change every existing puzzle.
        assert_eq!(derive_seed(1000, 1, STREAM_FILL, 0), GOLDEN_FILL_SEED);
        let seeds: BTreeSet<u64> = (0..4)
            .flat_map(|stream| (0..4).map(move |attempt| derive_seed(1000, 1, stream, attempt)))
            .collect();
        assert_eq!(seeds.len(), 16);
        assert_ne!(derive_seed(1000, 1, 0, 0), derive_seed(1000, 2, 0, 0));
    }

    const GOLDEN_FILL_SEED: u64 = 8_246_128_913_712_181_991;

    #[test]
    fn fnv1a_depends_on_chunk_boundaries() {
        assert_ne!(fnv1a(&[b"ab", b"c"]), fnv1a(&[b"a", b"bc"]));
        assert_eq!(fnv1a(&[b"x"]), fnv1a(&[b"x"]));
    }
}
