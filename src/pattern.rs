//! Random generation of block patterns for dense, newspaper-style grids.
//!
//! A pattern is only the arrangement of blocks; filling it is the job of the backtracking search.
//! The generator uses simulated annealing over block moves. It minimizes a weighted sum of hard
//! constraint violations (which must reach zero) and soft penalties (which describe what tends
//! to make dense grids fillable), and keeps going until the soft penalty is low, rather than
//! stopping at the first legal grid. Grids that only just satisfy every limit turned out to be
//! much harder to fill than ones that resemble hand-made grids.

use rand::prelude::*;

/// Weight of one hard violation relative to one unit of soft penalty.
const HARD_WEIGHT: f64 = 100.0;

/// Constraints and targets for generated patterns.
#[derive(Debug, Clone)]
pub struct PatternConfig {
    pub width: usize,
    pub height: usize,

    /// Inclusive range for the number of blocks.
    pub min_blocks: usize,
    pub max_blocks: usize,

    /// Hard limit on entry length.
    pub max_len: usize,

    /// Hard rule: no two blocks may share an edge.
    pub forbid_adjacent_blocks: bool,

    /// Entries longer than this are "long": allowed, but penalized.
    pub long_threshold: usize,

    /// Soft limit on the number of long entries.
    pub max_long_entries: usize,

    /// Soft limit on the number of 2-letter entries.
    pub max_two_letter_entries: usize,

    /// Soft range for the total number of entries.
    pub min_entries: usize,
    pub max_entries: usize,

    /// Soft minimum for the number of white cells that belong to only one entry.
    pub min_single_entry_cells: usize,

    /// The search stops once hard violations are zero and the soft penalty is at most this.
    pub target_soft_penalty: f64,

    /// Annealing steps per restart.
    pub steps_per_restart: usize,

    /// Restarts before giving up.
    pub max_restarts: usize,
}

impl PatternConfig {
    /// Settings modelled on the hand-made 17x8 grid: 22 blocks, no adjacent blocks, longest entry
    /// 8, 46 entries, 4 of them 2-letter, 17 cells in only one entry.
    #[must_use]
    pub fn like_original_17x8() -> PatternConfig {
        PatternConfig {
            width: 17,
            height: 8,
            min_blocks: 20,
            max_blocks: 24,
            max_len: 10,
            forbid_adjacent_blocks: true,
            long_threshold: 8,
            max_long_entries: 2,
            max_two_letter_entries: 4,
            min_entries: 40,
            max_entries: 52,
            min_single_entry_cells: 12,
            target_soft_penalty: 2.0,
            steps_per_restart: 5000,
            max_restarts: 200,
        }
    }
}

/// Direction of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dir {
    Across,
    Down,
}

/// An entry (run of 2 or more white cells) in a pattern, with zero-indexed coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternEntry {
    pub row: usize,
    pub col: usize,
    pub dir: Dir,
    pub len: usize,
}

/// A block pattern, stored row-major with `true` for blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub width: usize,
    pub height: usize,
    pub blocks: Vec<bool>,
}

/// Measurements of a pattern, used both for scoring and for reporting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatternStats {
    pub blocks: usize,
    pub entries: usize,
    pub two_letter_entries: usize,
    pub long_entries: usize,
    pub longest_entry: usize,

    /// Sum over all runs of how far they exceed `max_len`.
    pub excess_length: usize,

    /// White cells that belong to no entry.
    pub orphan_cells: usize,

    /// White cells that belong to exactly one entry.
    pub single_entry_cells: usize,

    /// Number of connected components of white cells.
    pub components: usize,

    /// Pairs of blocks sharing an edge.
    pub adjacent_block_pairs: usize,
}

impl Pattern {
    /// Parse a template with `#` for blocks and anything else for white cells.
    pub fn from_template(template: &str) -> Result<Pattern, String> {
        let rows: Vec<&str> = template
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let height = rows.len();
        if height == 0 {
            return Err("Pattern must have at least one row".into());
        }
        let width = rows[0].chars().count();
        if rows.iter().any(|row| row.chars().count() != width) {
            return Err("Rows in pattern must all be the same length".into());
        }
        let blocks = rows
            .iter()
            .flat_map(|row| row.chars().map(|ch| ch == '#'))
            .collect();
        Ok(Pattern {
            width,
            height,
            blocks,
        })
    }

    /// Render as a template with `#` for blocks and `.` for white cells, one row per line.
    #[must_use]
    pub fn to_template(&self) -> String {
        self.blocks
            .chunks(self.width)
            .map(|row| {
                row.iter()
                    .map(|&block| if block { '#' } else { '.' })
                    .collect::<String>()
                    + "\n"
            })
            .collect()
    }

    #[must_use]
    pub fn is_block(&self, row: usize, col: usize) -> bool {
        self.blocks[row * self.width + col]
    }

    /// All runs of white cells as `(row, col, dir, len)`, including 1-letter runs.
    fn runs(&self) -> Vec<PatternEntry> {
        let mut runs = vec![];
        for row in 0..self.height {
            let mut col = 0;
            while col < self.width {
                if self.is_block(row, col) {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < self.width && !self.is_block(row, col) {
                    col += 1;
                }
                runs.push(PatternEntry {
                    row,
                    col: start,
                    dir: Dir::Across,
                    len: col - start,
                });
            }
        }
        for col in 0..self.width {
            let mut row = 0;
            while row < self.height {
                if self.is_block(row, col) {
                    row += 1;
                    continue;
                }
                let start = row;
                while row < self.height && !self.is_block(row, col) {
                    row += 1;
                }
                runs.push(PatternEntry {
                    row: start,
                    col,
                    dir: Dir::Down,
                    len: row - start,
                });
            }
        }
        runs
    }

    /// The entries of the pattern: runs of 2 or more white cells, matching how Ingrid's template
    /// parser defines slots. Across entries come first, each direction in reading order.
    #[must_use]
    pub fn entries(&self) -> Vec<PatternEntry> {
        let mut entries: Vec<_> = self.runs().into_iter().filter(|run| run.len >= 2).collect();
        entries.sort_by_key(|entry| (entry.dir, entry.row, entry.col));
        entries
    }

    #[must_use]
    pub fn stats(&self, max_len: usize, long_threshold: usize) -> PatternStats {
        let mut stats = PatternStats {
            blocks: self.blocks.iter().filter(|&&block| block).count(),
            ..PatternStats::default()
        };

        let mut entry_count_by_cell = vec![0u8; self.blocks.len()];
        for run in self.runs() {
            stats.excess_length += run.len.saturating_sub(max_len);
            if run.len < 2 {
                continue;
            }
            stats.entries += 1;
            stats.longest_entry = stats.longest_entry.max(run.len);
            if run.len == 2 {
                stats.two_letter_entries += 1;
            }
            if run.len > long_threshold {
                stats.long_entries += 1;
            }
            for i in 0..run.len {
                let (row, col) = match run.dir {
                    Dir::Across => (run.row, run.col + i),
                    Dir::Down => (run.row + i, run.col),
                };
                entry_count_by_cell[row * self.width + col] += 1;
            }
        }
        for (idx, &block) in self.blocks.iter().enumerate() {
            if block {
                continue;
            }
            match entry_count_by_cell[idx] {
                0 => stats.orphan_cells += 1,
                1 => stats.single_entry_cells += 1,
                _ => {}
            }
        }

        // Connected components of white cells (4-neighbourhood).
        let mut seen = vec![false; self.blocks.len()];
        let mut stack = vec![];
        for start in 0..self.blocks.len() {
            if self.blocks[start] || seen[start] {
                continue;
            }
            stats.components += 1;
            seen[start] = true;
            stack.push(start);
            while let Some(idx) = stack.pop() {
                let (row, col) = (idx / self.width, idx % self.width);
                let mut neighbours = [None; 4];
                if row > 0 {
                    neighbours[0] = Some(idx - self.width);
                }
                if row + 1 < self.height {
                    neighbours[1] = Some(idx + self.width);
                }
                if col > 0 {
                    neighbours[2] = Some(idx - 1);
                }
                if col + 1 < self.width {
                    neighbours[3] = Some(idx + 1);
                }
                for next in neighbours.into_iter().flatten() {
                    if !self.blocks[next] && !seen[next] {
                        seen[next] = true;
                        stack.push(next);
                    }
                }
            }
        }

        for row in 0..self.height {
            for col in 0..self.width {
                if !self.is_block(row, col) {
                    continue;
                }
                if col + 1 < self.width && self.is_block(row, col + 1) {
                    stats.adjacent_block_pairs += 1;
                }
                if row + 1 < self.height && self.is_block(row + 1, col) {
                    stats.adjacent_block_pairs += 1;
                }
            }
        }

        stats
    }
}

/// Number of hard-constraint violations; zero means the pattern is legal.
#[must_use]
pub fn hard_violations(stats: &PatternStats, config: &PatternConfig) -> usize {
    let block_range_violation = if stats.blocks < config.min_blocks {
        config.min_blocks - stats.blocks
    } else {
        stats.blocks.saturating_sub(config.max_blocks)
    };
    stats.orphan_cells
        + stats.excess_length
        + stats.components.saturating_sub(1)
        + if config.forbid_adjacent_blocks {
            stats.adjacent_block_pairs
        } else {
            0
        }
        + block_range_violation
}

/// Soft penalty: how far the pattern is from the targets that make dense grids fillable.
#[must_use]
pub fn soft_penalty(stats: &PatternStats, config: &PatternConfig) -> f64 {
    let mut penalty = 0.0;

    // Each long entry costs a little, and going over the limit costs a lot.
    penalty += stats.long_entries as f64;
    penalty += 10.0 * stats.long_entries.saturating_sub(config.max_long_entries) as f64;

    penalty += 3.0
        * stats
            .two_letter_entries
            .saturating_sub(config.max_two_letter_entries) as f64;

    if stats.entries < config.min_entries {
        penalty += 2.0 * (config.min_entries - stats.entries) as f64;
    } else if stats.entries > config.max_entries {
        penalty += 2.0 * (stats.entries - config.max_entries) as f64;
    }

    penalty += 0.5
        * config
            .min_single_entry_cells
            .saturating_sub(stats.single_entry_cells) as f64;

    penalty
}

/// Does the pattern satisfy every hard constraint?
#[must_use]
pub fn is_legal(pattern: &Pattern, config: &PatternConfig) -> bool {
    hard_violations(&pattern.stats(config.max_len, config.long_threshold), config) == 0
}

fn energy(pattern: &Pattern, config: &PatternConfig) -> (usize, f64) {
    let stats = pattern.stats(config.max_len, config.long_threshold);
    let hard = hard_violations(&stats, config);
    let soft = soft_penalty(&stats, config);
    (hard, soft)
}

/// Generate a pattern that satisfies the hard constraints and has a soft penalty of at most
/// `config.target_soft_penalty`, or `None` if that wasn't reached within `max_restarts`.
pub fn generate_pattern<R: Rng>(config: &PatternConfig, rng: &mut R) -> Option<Pattern> {
    let cell_count = config.width * config.height;
    assert!(config.min_blocks <= config.max_blocks && config.max_blocks < cell_count);

    for _ in 0..config.max_restarts {
        let block_count = rng.random_range(config.min_blocks..=config.max_blocks);
        let mut cells: Vec<usize> = (0..cell_count).collect();
        cells.shuffle(rng);
        let mut blocks = vec![false; cell_count];
        for &idx in &cells[..block_count] {
            blocks[idx] = true;
        }
        let mut pattern = Pattern {
            width: config.width,
            height: config.height,
            blocks,
        };

        let (mut hard, mut soft) = energy(&pattern, config);
        for step in 0..config.steps_per_restart {
            if hard == 0 && soft <= config.target_soft_penalty {
                return Some(pattern);
            }

            // Temperature falls linearly from 3.0 to 0.05 over the restart.
            let progress = step as f64 / config.steps_per_restart as f64;
            let temperature = 3.0 * (1.0 - progress) + 0.05 * progress;

            // Move a random block to a random white cell.
            let block_cells: Vec<usize> = (0..cell_count).filter(|&i| pattern.blocks[i]).collect();
            let white_cells: Vec<usize> =
                (0..cell_count).filter(|&i| !pattern.blocks[i]).collect();
            let from = *block_cells.choose(rng).unwrap();
            let to = *white_cells.choose(rng).unwrap();
            pattern.blocks[from] = false;
            pattern.blocks[to] = true;

            let (new_hard, new_soft) = energy(&pattern, config);
            let delta = HARD_WEIGHT * (new_hard as f64 - hard as f64) + (new_soft - soft);
            if delta <= 0.0 || rng.random::<f64>() < (-delta / temperature).exp() {
                hard = new_hard;
                soft = new_soft;
            } else {
                pattern.blocks[to] = false;
                pattern.blocks[from] = true;
            }
        }

        if hard == 0 && soft <= config.target_soft_penalty {
            return Some(pattern);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    const ORIGINAL: &str = indoc! {"
        ......#.....#....
        ....#....#.......
        ......#.....#.#..
        ..#......#.#....#
        #.....#.#....#...
        ...#.#.....#.....
        ........#....#...
        .....#.....#.....
    "};

    #[test]
    fn original_grid_stats() {
        let pattern = Pattern::from_template(ORIGINAL).unwrap();
        let stats = pattern.stats(10, 8);
        assert_eq!(stats.blocks, 22);
        assert_eq!(stats.entries, 46);
        assert_eq!(stats.two_letter_entries, 4);
        assert_eq!(stats.longest_entry, 8);
        assert_eq!(stats.long_entries, 0);
        assert_eq!(stats.single_entry_cells, 17);
        assert_eq!(stats.orphan_cells, 0);
        assert_eq!(stats.components, 1);
        assert_eq!(stats.adjacent_block_pairs, 0);
    }

    #[test]
    fn original_grid_passes_all_constraints() {
        let config = PatternConfig::like_original_17x8();
        let pattern = Pattern::from_template(ORIGINAL).unwrap();
        let stats = pattern.stats(config.max_len, config.long_threshold);
        assert_eq!(hard_violations(&stats, &config), 0);
        assert!(soft_penalty(&stats, &config) <= config.target_soft_penalty);
    }

    #[test]
    fn template_round_trip() {
        let pattern = Pattern::from_template(ORIGINAL).unwrap();
        assert_eq!(pattern.to_template(), ORIGINAL);
    }

    #[test]
    fn detects_orphan_cells() {
        // The middle cell of the top row is boxed in horizontally and vertically.
        let pattern = Pattern::from_template("..#.#..\n...#...\n").unwrap();
        let stats = pattern.stats(10, 8);
        assert_eq!(stats.orphan_cells, 1);
    }

    #[test]
    fn detects_long_runs() {
        let pattern = Pattern::from_template("............\n").unwrap();
        let stats = pattern.stats(10, 8);
        assert_eq!(stats.longest_entry, 12);
        assert_eq!(stats.excess_length, 2);
    }

    #[test]
    fn detects_disconnected_regions() {
        let pattern = Pattern::from_template("..#..\n..#..\n").unwrap();
        assert_eq!(pattern.stats(10, 8).components, 2);
    }

    #[test]
    fn detects_adjacent_blocks() {
        let pattern = Pattern::from_template("..##..\n...#..\n").unwrap();
        assert_eq!(pattern.stats(10, 8).adjacent_block_pairs, 2);
    }

    #[test]
    fn entries_skip_one_letter_runs() {
        let pattern = Pattern::from_template(".#..\n").unwrap();
        assert_eq!(
            pattern.entries(),
            vec![PatternEntry {
                row: 0,
                col: 2,
                dir: Dir::Across,
                len: 2
            }]
        );
    }

    #[test]
    fn generated_patterns_are_legal_and_deterministic() {
        let config = PatternConfig::like_original_17x8();
        for seed in 0..20 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let pattern = generate_pattern(&config, &mut rng).expect("pattern");
            assert!(is_legal(&pattern, &config), "seed {seed}");
            let mut rng = SmallRng::seed_from_u64(seed);
            assert_eq!(generate_pattern(&config, &mut rng), Some(pattern));
        }
    }
}
