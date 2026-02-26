use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use super::PathGrid;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct FrontierNode {
    pos: (i32, i32),
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

pub fn find_path(grid: &PathGrid, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if !grid.in_bounds(start.0, start.1) || !grid.in_bounds(goal.0, goal.1) {
        return None;
    }
    if grid.is_blocked(goal.0, goal.1) {
        return None;
    }
    if start == goal {
        return Some(vec![start]);
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_scores: HashMap<(i32, i32), i32> = HashMap::new();

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
            if !grid.in_bounds(next.0, next.1) || grid.is_blocked(next.0, next.1) {
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

fn heuristic(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

fn neighbors(pos: (i32, i32)) -> [(i32, i32); 4] {
    let (x, z) = pos;
    [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)]
}

fn reconstruct_path(
    mut came_from: HashMap<(i32, i32), (i32, i32)>,
    mut current: (i32, i32),
) -> Vec<(i32, i32)> {
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
    use super::find_path;
    use crate::path::PathGrid;

    #[test]
    fn finds_simple_path() {
        let grid = PathGrid::new(5, 5);
        let path = find_path(&grid, (0, 0), (4, 4)).expect("path");
        assert_eq!(path.first().copied(), Some((0, 0)));
        assert_eq!(path.last().copied(), Some((4, 4)));
    }

    #[test]
    fn avoids_blocked_cells() {
        let mut grid = PathGrid::new(6, 3);
        for z in 0..3 {
            grid.set_blocked(2, z, true);
        }
        grid.set_blocked(2, 1, false);

        let path = find_path(&grid, (0, 1), (5, 1)).expect("path around wall");
        assert!(path.contains(&(2, 1)));
        assert!(!path.contains(&(2, 0)));
        assert!(!path.contains(&(2, 2)));
    }

    #[test]
    fn no_path_when_goal_blocked() {
        let mut grid = PathGrid::new(4, 4);
        grid.set_blocked(3, 3, true);
        assert!(find_path(&grid, (0, 0), (3, 3)).is_none());
    }
}
