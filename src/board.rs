//! Minesweeper rules, ported from omamine `Game.js`.
//!
//! First-click safety, flood-fill via an explicit queue, win when every
//! non-mine cell is revealed (not when flags match).

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Expert,
}

impl Difficulty {
    pub fn id(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Intermediate => "intermediate",
            Self::Expert => "expert",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Beginner => "Beginner",
            Self::Intermediate => "Inter.",
            Self::Expert => "Expert",
        }
    }

    pub fn spec(self) -> (u8, u8, u16) {
        match self {
            Self::Beginner => (9, 9, 10),
            Self::Intermediate => (16, 16, 40),
            Self::Expert => (30, 16, 99),
        }
    }

    pub fn parse(id: &str) -> Self {
        match id {
            "intermediate" => Self::Intermediate,
            "expert" => Self::Expert,
            _ => Self::Beginner,
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Beginner, Self::Intermediate, Self::Expert]
    }
}

/// [0, 1) source used by mine placement (matches `Math.random` in omamine).
pub trait Rand {
    fn next_f64(&mut self) -> f64;
}

/// Linear congruential generator used by omamine's tests.
pub struct Lcg {
    i: u32,
}

impl Lcg {
    pub fn new(seed: u32) -> Self {
        Self { i: seed }
    }
}

impl Rand for Lcg {
    fn next_f64(&mut self) -> f64 {
        self.i = self.i.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        f64::from(self.i % 233_280) / 233_280.0
    }
}

pub struct ThreadRng {
    s: u64,
}

impl Default for ThreadRng {
    fn default() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self { s: nanos | 1 }
    }
}

impl Rand for ThreadRng {
    fn next_f64(&mut self) -> f64 {
        let mut s = self.s;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        self.s = s;
        (s as f64) / (u64::MAX as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub x: u8,
    pub y: u8,
    pub mine: bool,
    pub adj: u8,
    pub revealed: bool,
    pub flagged: bool,
    pub exploded: bool,
}

#[derive(Debug, Clone)]
pub struct Game {
    pub difficulty: Difficulty,
    pub width: u8,
    pub height: u8,
    pub mines: u16,
    pub cells: Vec<Cell>,
    pub placed: bool,
    pub over: bool,
    pub won: bool,
    pub flags: i16,
    pub revealed_count: u16,
    pub cursor_x: u8,
    pub cursor_y: u8,
}

impl Game {
    pub fn new(difficulty: Difficulty) -> Self {
        let (width, height, mines) = difficulty.spec();
        let mut cells = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            for x in 0..width {
                cells.push(Cell {
                    x,
                    y,
                    mine: false,
                    adj: 0,
                    revealed: false,
                    flagged: false,
                    exploded: false,
                });
            }
        }
        Self {
            difficulty,
            width,
            height,
            mines,
            cells,
            placed: false,
            over: false,
            won: false,
            flags: 0,
            revealed_count: 0,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    pub fn remaining(&self) -> i16 {
        self.mines as i16 - self.flags
    }

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= i32::from(self.width) || y >= i32::from(self.height) {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    pub fn cell(&self, x: i32, y: i32) -> Option<&Cell> {
        self.index(x, y).map(|i| &self.cells[i])
    }

    pub fn cell_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        let i = self.index(x, y)?;
        Some(&mut self.cells[i])
    }

    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        let x = (i32::from(self.cursor_x) + dx).clamp(0, i32::from(self.width) - 1);
        let y = (i32::from(self.cursor_y) + dy).clamp(0, i32::from(self.height) - 1);
        self.cursor_x = x as u8;
        self.cursor_y = y as u8;
    }

    pub fn set_cursor(&mut self, x: u8, y: u8) {
        self.cursor_x = x.min(self.width.saturating_sub(1));
        self.cursor_y = y.min(self.height.saturating_sub(1));
    }

    pub fn neighbor_indices(&self, x: u8, y: u8) -> Vec<usize> {
        let mut out = Vec::with_capacity(8);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(i) = self.index(i32::from(x) + dx, i32::from(y) + dy) {
                    out.push(i);
                }
            }
        }
        out
    }

    pub fn place_mines<R: Rand>(&mut self, safe_x: u8, safe_y: u8, rng: &mut R) {
        let mut forbidden = vec![false; self.cells.len()];
        if let Some(i) = self.index(i32::from(safe_x), i32::from(safe_y)) {
            forbidden[i] = true;
        }
        for i in self.neighbor_indices(safe_x, safe_y) {
            forbidden[i] = true;
        }

        let mut candidates: Vec<usize> = (0..self.cells.len()).filter(|&i| !forbidden[i]).collect();
        if candidates.len() < self.mines as usize {
            candidates = (0..self.cells.len())
                .filter(|&i| {
                    let c = &self.cells[i];
                    !(c.x == safe_x && c.y == safe_y)
                })
                .collect();
        }
        shuffle(&mut candidates, rng);
        let count = self.mines.min(candidates.len() as u16);
        for &i in candidates.iter().take(count as usize) {
            self.cells[i].mine = true;
        }
        self.mines = count;

        for i in 0..self.cells.len() {
            if self.cells[i].mine {
                self.cells[i].adj = 0;
                continue;
            }
            let (x, y) = (self.cells[i].x, self.cells[i].y);
            let adj = self
                .neighbor_indices(x, y)
                .into_iter()
                .filter(|&n| self.cells[n].mine)
                .count();
            self.cells[i].adj = adj as u8;
        }
        self.placed = true;
    }

    pub fn reveal<R: Rand>(&mut self, x: u8, y: u8, rng: &mut R) {
        if self.over {
            return;
        }
        let Some(i) = self.index(i32::from(x), i32::from(y)) else {
            return;
        };
        if self.cells[i].revealed || self.cells[i].flagged {
            return;
        }
        if !self.placed {
            self.place_mines(x, y, rng);
        }
        self.reveal_index(i);
        self.check_win();
    }

    fn reveal_index(&mut self, i: usize) {
        if self.cells[i].revealed || self.cells[i].flagged || self.over {
            return;
        }
        self.cells[i].revealed = true;
        self.revealed_count += 1;
        if self.cells[i].mine {
            self.cells[i].exploded = true;
            self.explode();
            return;
        }
        if self.cells[i].adj == 0 {
            self.flood(i);
        }
    }

    fn flood(&mut self, start: usize) {
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            let (x, y) = (self.cells[i].x, self.cells[i].y);
            let around = self.neighbor_indices(x, y);
            for n in around {
                if self.cells[n].revealed || self.cells[n].flagged || self.cells[n].mine {
                    continue;
                }
                self.cells[n].revealed = true;
                self.revealed_count += 1;
                if self.cells[n].adj == 0 {
                    queue.push_back(n);
                }
            }
        }
    }

    fn explode(&mut self) {
        self.over = true;
        self.won = false;
        for cell in &mut self.cells {
            if cell.mine {
                cell.revealed = true;
            }
        }
    }

    fn check_win(&mut self) {
        if self.over {
            return;
        }
        let safe = u16::from(self.width) * u16::from(self.height) - self.mines;
        if self.revealed_count < safe {
            return;
        }
        self.over = true;
        self.won = true;
        self.flags = self.mines as i16;
        for cell in &mut self.cells {
            if cell.mine {
                cell.flagged = true;
            }
        }
    }

    pub fn flag(&mut self, x: u8, y: u8) {
        if self.over {
            return;
        }
        let Some(i) = self.index(i32::from(x), i32::from(y)) else {
            return;
        };
        if self.cells[i].revealed {
            return;
        }
        self.cells[i].flagged = !self.cells[i].flagged;
        self.flags += if self.cells[i].flagged { 1 } else { -1 };
    }

    pub fn chord<R: Rand>(&mut self, x: u8, y: u8, rng: &mut R) {
        if self.over {
            return;
        }
        let Some(i) = self.index(i32::from(x), i32::from(y)) else {
            return;
        };
        if !self.cells[i].revealed {
            self.reveal(x, y, rng);
            return;
        }
        let adj = self.cells[i].adj;
        if adj == 0 {
            return;
        }
        let around = self.neighbor_indices(x, y);
        let flagged = around.iter().filter(|&&n| self.cells[n].flagged).count();
        if flagged != adj as usize {
            return;
        }
        let pending: Vec<(u8, u8)> = around
            .into_iter()
            .filter(|&n| !self.cells[n].flagged && !self.cells[n].revealed)
            .map(|n| (self.cells[n].x, self.cells[n].y))
            .collect();
        for (nx, ny) in pending {
            self.reveal(nx, ny, rng);
            if self.over && !self.won {
                return;
            }
        }
        self.check_win();
    }
}

fn shuffle<R: Rand>(values: &mut [usize], rng: &mut R) {
    for i in (1..values.len()).rev() {
        let j = (rng.next_f64() * (i + 1) as f64).floor() as usize;
        values.swap(i, j.min(i));
    }
}

pub fn pad3(n: i16) -> String {
    if n < 0 {
        let mag = (-n).min(99);
        format!("-{mag:02}")
    } else {
        format!("{:03}", n.max(0))
    }
}

pub fn format_time(seconds: u32) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{m}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(game: &Game, pred: impl Fn(&Cell) -> bool) -> usize {
        game.cells.iter().filter(|c| pred(c)).count()
    }

    #[test]
    fn beginner_and_expert_specs() {
        let b = Game::new(Difficulty::Beginner);
        assert_eq!((b.width, b.height, b.cells.len(), b.mines), (9, 9, 81, 10));
        assert!(!b.placed);
        let e = Game::new(Difficulty::Expert);
        assert_eq!(
            (e.width, e.height, e.cells.len(), e.mines),
            (30, 16, 480, 99)
        );
        assert_eq!(Difficulty::parse("nope"), Difficulty::Beginner);
        assert_eq!(pad3(99), "099");
        assert_eq!(pad3(-2), "-02");
        assert_eq!(format_time(65), "1:05");
    }

    #[test]
    fn first_click_is_safe_and_places_mines() {
        let mut s = Game::new(Difficulty::Beginner);
        s.reveal(4, 4, &mut Lcg::new(1));
        assert!(s.placed);
        let cell = s.cell(4, 4).unwrap();
        assert!(!cell.mine && cell.revealed);
        assert_eq!(count(&s, |c| c.mine), 10);
        for i in s.neighbor_indices(4, 4) {
            assert!(!s.cells[i].mine);
        }
    }

    #[test]
    fn flag_toggle_and_blocks_reveal() {
        let mut s = Game::new(Difficulty::Beginner);
        s.reveal(4, 4, &mut Lcg::new(1));
        let hidden = *s
            .cells
            .iter()
            .find(|c| !c.revealed && !c.mine)
            .expect("covered safe cell");
        s.flag(hidden.x, hidden.y);
        assert_eq!(s.flags, 1);
        assert!(s.cell(hidden.x as i32, hidden.y as i32).unwrap().flagged);
        assert_eq!(s.remaining(), 9);
        s.flag(hidden.x, hidden.y);
        assert_eq!(s.flags, 0);
        s.flag(hidden.x, hidden.y);
        s.reveal(hidden.x, hidden.y, &mut Lcg::new(1));
        let cell = s.cell(hidden.x as i32, hidden.y as i32).unwrap();
        assert!(cell.flagged && !cell.revealed);
    }

    #[test]
    fn hitting_a_mine_loses_and_shows_all() {
        let mut lose = Game::new(Difficulty::Beginner);
        lose.place_mines(0, 0, &mut Lcg::new(1));
        let mine = *lose.cells.iter().find(|c| c.mine).unwrap();
        lose.reveal(mine.x, mine.y, &mut Lcg::new(1));
        assert!(
            lose.over && !lose.won && lose.cell(mine.x as i32, mine.y as i32).unwrap().exploded
        );
        assert_eq!(count(&lose, |c| c.mine && c.revealed), lose.mines as usize);
    }

    #[test]
    fn clearing_safe_cells_wins() {
        let mut win = Game::new(Difficulty::Beginner);
        win.reveal(2, 2, &mut Lcg::new(1));
        let pending: Vec<(u8, u8)> = win
            .cells
            .iter()
            .filter(|c| !c.mine && !c.revealed)
            .map(|c| (c.x, c.y))
            .collect();
        for (x, y) in pending {
            win.reveal(x, y, &mut Lcg::new(1));
        }
        assert!(win.won && win.over);
        assert_eq!(win.flags as u16, win.mines);
    }

    #[test]
    fn correct_chord_does_not_lose() {
        let mut chord = Game::new(Difficulty::Beginner);
        chord.place_mines(0, 0, &mut Lcg::new(1));
        let numbered = *chord.cells.iter().find(|c| !c.mine && c.adj == 1).unwrap();
        chord.reveal(numbered.x, numbered.y, &mut Lcg::new(1));
        let around = chord.neighbor_indices(numbered.x, numbered.y);
        let mine = around.into_iter().find(|&i| chord.cells[i].mine).unwrap();
        let (mx, my) = (chord.cells[mine].x, chord.cells[mine].y);
        chord.flag(mx, my);
        chord.chord(numbered.x, numbered.y, &mut Lcg::new(1));
        assert!(!chord.over || chord.won);
        chord.move_cursor(100, 100);
        assert_eq!(chord.cursor_x, chord.width - 1);
        assert_eq!(chord.cursor_y, chord.height - 1);
    }
}
