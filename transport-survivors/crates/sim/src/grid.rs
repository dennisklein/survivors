//! Spatial hash with one bucket per tile, for neighbour and hit queries.

use crate::math::V2;

#[derive(Clone, Debug)]
pub struct Grid {
    w: i32,
    h: i32,
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            start: vec![0; (w * h + 1) as usize],
            items: Vec::new(),
        }
    }

    fn cell_xy(&self, p: V2) -> (i32, i32) {
        (
            (p.x.floor() as i32).clamp(0, self.w - 1),
            (p.y.floor() as i32).clamp(0, self.h - 1),
        )
    }

    fn cell(&self, p: V2) -> usize {
        let (x, y) = self.cell_xy(p);
        (y * self.w + x) as usize
    }

    /// Sorts the points into buckets (counting sort).
    pub fn build(&mut self, points: &[V2]) {
        self.start.fill(0);
        for &p in points {
            let c = self.cell(p);
            self.start[c + 1] += 1;
        }
        for i in 1..self.start.len() {
            self.start[i] += self.start[i - 1];
        }
        let mut next = self.start.clone();
        self.items.clear();
        self.items.resize(points.len(), 0);
        for (i, &p) in points.iter().enumerate() {
            let c = self.cell(p);
            self.items[next[c] as usize] = i as u32;
            next[c] += 1;
        }
    }

    /// Calls `f` with the index of every point in the buckets within `r` of `p`.
    /// The caller checks the exact distance.
    pub fn query(&self, p: V2, r: f32, mut f: impl FnMut(usize)) {
        let (x0, y0) = self.cell_xy(p - V2::new(r, r));
        let (x1, y1) = self.cell_xy(p + V2::new(r, r));
        for y in y0..=y1 {
            let row = (y * self.w) as usize;
            let a = self.start[row + x0 as usize] as usize;
            let b = self.start[row + x1 as usize + 1] as usize;
            for &i in &self.items[a..b] {
                f(i as usize);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_finds_near_points_only() {
        let mut g = Grid::new(16, 16);
        let pts = [
            V2::new(1.5, 1.5),
            V2::new(2.2, 1.1),
            V2::new(10.0, 10.0),
            V2::new(-5.0, 1.0),
        ];
        g.build(&pts);
        let mut found = Vec::new();
        g.query(V2::new(1.8, 1.4), 1.0, |i| found.push(i));
        found.sort();
        assert_eq!(
            found,
            vec![0, 1, 3],
            "point outside the map is clamped to the edge"
        );
        let mut far = Vec::new();
        g.query(V2::new(10.2, 10.2), 0.5, |i| far.push(i));
        assert_eq!(far, vec![2]);
    }
}
