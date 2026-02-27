use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::cell::Cell;

use super::PathGrid;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct FrontierNode {
    pos: Cell,
    f_score: i32,
    g_score: i32,
}

impl Ord for FrontierNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_score
            .cmp(&self.f_score)
            .then_with(|| other.g_score.cmp(&self.g_score))
            .then_with(|| self.pos.cmp(&other.pos))
    }
}

impl PartialOrd for FrontierNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn find_path(grid: &PathGrid, start: Cell, goal: Cell) -> Option<Vec<Cell>> {
    if !grid.in_bounds(start.x, start.z) || !grid.in_bounds(goal.x, goal.z) {
        return None;
    }
    if grid.is_blocked(goal.x, goal.z) {
        return None;
    }
    if start == goal {
        return Some(vec![start]);
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<Cell, Cell> = HashMap::new();
    let mut g_scores: HashMap<Cell, i32> = HashMap::new();

    g_scores.insert(start, 0);
    open.push(FrontierNode {
        pos: start,
        g_score: 0,
        f_score: heuristic(start, goal),
    });

    while let Some(current) = open.pop() {
        if current.pos == goal {
            return Some(reconstruct_path(came_from, goal));
        }

        for next in neighbors(current.pos) {
            if !grid.in_bounds(next.x, next.z) || grid.is_blocked(next.x, next.z) {
                continue;
            }

            let tentative_g = current.g_score + 1;
            let known_g = g_scores.get(&next).copied().unwrap_or(i32::MAX);
            if tentative_g < known_g {
                came_from.insert(next, current.pos);
                g_scores.insert(next, tentative_g);
                open.push(FrontierNode {
                    pos: next,
                    g_score: tentative_g,
                    f_score: tentative_g + heuristic(next, goal),
                });
            }
        }
    }

    None
}

fn heuristic(a: Cell, b: Cell) -> i32 {
    (a.x - b.x).abs() + (a.z - b.z).abs()
}

fn neighbors(pos: Cell) -> [Cell; 4] {
    [
        Cell::new(pos.x + 1, pos.z),
        Cell::new(pos.x - 1, pos.z),
        Cell::new(pos.x, pos.z + 1),
        Cell::new(pos.x, pos.z - 1),
    ]
}

fn reconstruct_path(mut came_from: HashMap<Cell, Cell>, mut current: Cell) -> Vec<Cell> {
    let mut out = vec![current];
    while let Some(prev) = came_from.remove(&current) {
        current = prev;
        out.push(current);
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use crate::cell::Cell;

    use super::find_path;
    use crate::path::PathGrid;

    #[test]
    fn finds_simple_path() {
        let grid = PathGrid::new(5, 5);
        let path = find_path(&grid, Cell::new(0, 0), Cell::new(4, 4)).expect("path");
        assert_eq!(path.first().copied(), Some(Cell::new(0, 0)));
        assert_eq!(path.last().copied(), Some(Cell::new(4, 4)));
    }

    #[test]
    fn avoids_blocked_cells() {
        let mut grid = PathGrid::new(6, 3);
        for z in 0..3 {
            grid.set_blocked(2, z, true);
        }
        grid.set_blocked(2, 1, false);

        let path = find_path(&grid, Cell::new(0, 1), Cell::new(5, 1)).expect("path around wall");
        assert!(path.contains(&Cell::new(2, 1)));
        assert!(!path.contains(&Cell::new(2, 0)));
        assert!(!path.contains(&Cell::new(2, 2)));
    }

    #[test]
    fn no_path_when_goal_blocked() {
        let mut grid = PathGrid::new(4, 4);
        grid.set_blocked(3, 3, true);
        assert!(find_path(&grid, Cell::new(0, 0), Cell::new(3, 3)).is_none());
    }
}
