//! CS50-style knowledge solver, ported from omamine `Solver.js`.
//!
//! Pure deduction: sentences of the form "exactly N of these cells are
//! mines", subset differences, and remaining-mine global constraints.
//! Never guesses. `hint` returns one certain reveal or flag, or `None`.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::board::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    None,
    Reveal { x: u8, y: u8 },
    Flag { x: u8, y: u8 },
}

impl Hint {
    pub fn kind(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Reveal { .. } => "reveal",
            Self::Flag { .. } => "flag",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Sentence {
    cells: BTreeSet<usize>,
    count: i32,
}

#[derive(Default)]
struct Knowledge {
    mines: HashSet<usize>,
    safes: HashSet<usize>,
    sentences: Vec<Sentence>,
}

fn add_sentence(kb: &mut Knowledge, cells: BTreeSet<usize>, count: i32) -> bool {
    if cells.is_empty() {
        return false;
    }
    let sentence = Sentence {
        cells,
        count: count.max(0),
    };
    if kb.sentences.iter().any(|s| s == &sentence) {
        return false;
    }
    kb.sentences.push(sentence);
    true
}

fn mark_mine(kb: &mut Knowledge, key: usize) -> bool {
    if kb.safes.contains(&key) || kb.mines.contains(&key) {
        return false;
    }
    kb.mines.insert(key);
    let mut changed = false;
    for sentence in &mut kb.sentences {
        if sentence.cells.remove(&key) {
            sentence.count = (sentence.count - 1).max(0);
            changed = true;
        }
    }
    changed
}

fn mark_safe(kb: &mut Knowledge, key: usize) -> bool {
    if kb.safes.contains(&key) || kb.mines.contains(&key) {
        return false;
    }
    kb.safes.insert(key);
    let mut changed = false;
    for sentence in &mut kb.sentences {
        if sentence.cells.remove(&key) {
            changed = true;
        }
    }
    changed
}

fn is_proper_subset(inner: &BTreeSet<usize>, outer: &BTreeSet<usize>) -> bool {
    !inner.is_empty() && inner.len() < outer.len() && inner.iter().all(|c| outer.contains(c))
}

pub fn infer(game: &Game) -> (HashSet<usize>, HashSet<usize>) {
    let mut kb = Knowledge::default();
    let mut hidden = Vec::new();
    let mut flagged = 0i32;

    for (i, cell) in game.cells.iter().enumerate() {
        if cell.revealed {
            kb.safes.insert(i);
        } else if cell.flagged {
            kb.mines.insert(i);
            flagged += 1;
        } else {
            hidden.push(i);
        }
    }

    for src in &game.cells {
        if !src.revealed || src.mine {
            continue;
        }
        let mut undetermined = BTreeSet::new();
        let mut count = i32::from(src.adj);
        for n in game.neighbor_indices(src.x, src.y) {
            if game.cells[n].flagged || kb.mines.contains(&n) {
                count -= 1;
            } else if !game.cells[n].revealed && !kb.safes.contains(&n) {
                undetermined.insert(n);
            }
        }
        add_sentence(&mut kb, undetermined, count);
    }

    let remain = i32::from(game.mines) - flagged;
    if remain == 0 {
        for &h in &hidden {
            kb.safes.insert(h);
        }
    } else if remain > 0 && remain == hidden.len() as i32 {
        for &h in &hidden {
            kb.mines.insert(h);
        }
    }

    for _ in 0..400 {
        let mut changed = false;

        let inferred_mines = kb.mines.iter().filter(|&&i| !game.cells[i].flagged).count() as i32;
        let still_hidden: Vec<usize> = hidden
            .iter()
            .copied()
            .filter(|i| !kb.mines.contains(i) && !kb.safes.contains(i))
            .collect();
        let remain = i32::from(game.mines) - flagged;
        let unknown_remain = (remain - inferred_mines).max(0);
        if unknown_remain == 0 && !still_hidden.is_empty() {
            for h in still_hidden {
                if mark_safe(&mut kb, h) {
                    changed = true;
                } else {
                    kb.safes.insert(h);
                }
            }
        } else if unknown_remain > 0 && unknown_remain == still_hidden.len() as i32 {
            for h in still_hidden {
                if mark_mine(&mut kb, h) {
                    changed = true;
                } else {
                    kb.mines.insert(h);
                }
            }
        }

        let mut mine_keys = Vec::new();
        let mut safe_keys = Vec::new();
        for sentence in &kb.sentences {
            if sentence.cells.is_empty() {
                continue;
            }
            if sentence.cells.len() as i32 == sentence.count && sentence.count > 0 {
                mine_keys.extend(sentence.cells.iter().copied());
            } else if sentence.count == 0 {
                safe_keys.extend(sentence.cells.iter().copied());
            }
        }
        for k in mine_keys {
            if mark_mine(&mut kb, k) {
                changed = true;
            }
        }
        for k in safe_keys {
            if mark_safe(&mut kb, k) {
                changed = true;
            }
        }

        let mut derived = Vec::new();
        for a in 0..kb.sentences.len() {
            for b in 0..kb.sentences.len() {
                if a == b {
                    continue;
                }
                let s1 = &kb.sentences[a];
                let s2 = &kb.sentences[b];
                if !is_proper_subset(&s1.cells, &s2.cells) {
                    continue;
                }
                let cells: BTreeSet<usize> = s2.cells.difference(&s1.cells).copied().collect();
                let count = s2.count - s1.count;
                if !cells.is_empty() && count >= 0 {
                    derived.push((cells, count));
                }
            }
        }
        for (cells, count) in derived {
            if add_sentence(&mut kb, cells, count) {
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    (kb.mines, kb.safes)
}

pub fn hint(game: &Game) -> Hint {
    if game.over {
        return Hint::None;
    }
    if !game.placed {
        return Hint::Reveal {
            x: game.width / 2,
            y: game.height / 2,
        };
    }
    let (mines, safes) = infer(game);
    for (i, cell) in game.cells.iter().enumerate() {
        if !cell.revealed && !cell.flagged && safes.contains(&i) {
            return Hint::Reveal {
                x: cell.x,
                y: cell.y,
            };
        }
    }
    for (i, cell) in game.cells.iter().enumerate() {
        if !cell.revealed && !cell.flagged && mines.contains(&i) {
            return Hint::Flag {
                x: cell.x,
                y: cell.y,
            };
        }
    }
    Hint::None
}

/// Build a board for solver tests: mines, then revealed, then flagged.
pub fn layout(
    width: u8,
    height: u8,
    mines: &[[u8; 2]],
    revealed: &[[u8; 2]],
    flagged: &[[u8; 2]],
) -> Game {
    let mut game = Game::new(crate::board::Difficulty::Beginner);
    game.width = width;
    game.height = height;
    game.mines = mines.len() as u16;
    game.cells.clear();
    let mine_set: HashSet<(u8, u8)> = mines.iter().map(|p| (p[0], p[1])).collect();
    for y in 0..height {
        for x in 0..width {
            game.cells.push(crate::board::Cell {
                x,
                y,
                mine: mine_set.contains(&(x, y)),
                adj: 0,
                revealed: false,
                flagged: false,
                exploded: false,
            });
        }
    }
    for i in 0..game.cells.len() {
        if game.cells[i].mine {
            continue;
        }
        let (x, y) = (game.cells[i].x, game.cells[i].y);
        let adj = game
            .neighbor_indices(x, y)
            .into_iter()
            .filter(|&n| game.cells[n].mine)
            .count();
        game.cells[i].adj = adj as u8;
    }
    let mut revealed_set: HashMap<(u8, u8), ()> = HashMap::new();
    for p in revealed {
        revealed_set.insert((p[0], p[1]), ());
    }
    game.flags = 0;
    game.revealed_count = 0;
    for cell in &mut game.cells {
        if revealed_set.contains_key(&(cell.x, cell.y)) {
            cell.revealed = true;
            game.revealed_count += 1;
        }
    }
    for p in flagged {
        if let Some(i) = game.index(i32::from(p[0]), i32::from(p[1])) {
            game.cells[i].flagged = true;
            game.flags += 1;
        }
    }
    game.placed = true;
    game
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Difficulty, Game, Lcg};

    #[test]
    fn over_is_none() {
        let mut g = Game::new(Difficulty::Beginner);
        g.over = true;
        g.placed = true;
        assert_eq!(hint(&g), Hint::None);
    }

    #[test]
    fn unstarted_hint_is_center() {
        let g = Game::new(Difficulty::Beginner);
        assert_eq!(hint(&g), Hint::Reveal { x: 4, y: 4 });
    }

    #[test]
    fn last_hidden_cell_is_the_mine() {
        let g = layout(
            3,
            3,
            &[[0, 0]],
            &[
                [1, 0],
                [2, 0],
                [0, 1],
                [1, 1],
                [2, 1],
                [0, 2],
                [1, 2],
                [2, 2],
            ],
            &[],
        );
        assert_eq!(hint(&g), Hint::Flag { x: 0, y: 0 });
    }

    #[test]
    fn remaining_mines_zero_marks_hidden_safe() {
        let g = layout(
            3,
            3,
            &[[0, 0]],
            &[[2, 0], [1, 1], [2, 1], [0, 2], [1, 2], [2, 2]],
            &[[0, 0]],
        );
        match hint(&g) {
            Hint::Reveal { x, y } => {
                let cell = g.cell(x as i32, y as i32).unwrap();
                assert!(!cell.mine && !cell.revealed);
            }
            other => panic!("expected reveal, got {other:?}"),
        }
    }

    #[test]
    fn certain_hints_do_not_hit_a_mine() {
        let mut play = Game::new(Difficulty::Beginner);
        play.reveal(4, 4, &mut Lcg::new(7));
        let mut steps = 0;
        let mut guesses = 0;
        while !play.over && steps < 200 {
            match hint(&play) {
                Hint::None => {
                    guesses += 1;
                    break;
                }
                Hint::Reveal { x, y } => play.reveal(x, y, &mut Lcg::new(7)),
                Hint::Flag { x, y } => play.flag(x, y),
            }
            steps += 1;
        }
        assert!(guesses == 1 || play.won || play.over);
        assert!(!(play.over && !play.won && guesses == 0));
    }
}
