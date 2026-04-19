//! Link-drawer and terrain-edge types, mirroring RimWorld's `Verse.LinkDrawer*`
//! and `Verse.TerrainEdgeType` enums. Pure data + arithmetic; no I/O.

/// Matches RimWorld's `LinkFlags` `[Flags]` enum. The values are the same on
/// both sides: a wall's `linkFlags` lists *both* what it is and what it
/// matches against. `MAP_EDGE` is the special sentinel that treats
/// out-of-bounds cells as a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinkFlags(u32);

impl LinkFlags {
    pub const EMPTY: Self = Self(0);
    pub const MAP_EDGE: Self = Self(0x01);
    pub const ROCK: Self = Self(0x02);
    pub const WALL: Self = Self(0x04);
    pub const SANDBAGS: Self = Self(0x08);
    pub const POWER_CONDUIT: Self = Self(0x10);
    pub const BARRICADES: Self = Self(0x20);
    pub const FENCES: Self = Self(0x40);

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Parse a single `<li>` value from `<linkFlags>`. Case-sensitive, matches
    /// RimWorld spelling. Returns `None` for unknown tokens so callers can
    /// warn and continue.
    pub fn from_token(name: &str) -> Option<Self> {
        match name {
            "None" => Some(Self::EMPTY),
            "MapEdge" => Some(Self::MAP_EDGE),
            "Rock" => Some(Self::ROCK),
            "Wall" => Some(Self::WALL),
            "Sandbags" => Some(Self::SANDBAGS),
            "PowerConduit" => Some(Self::POWER_CONDUIT),
            "Barricades" => Some(Self::BARRICADES),
            "Fences" => Some(Self::FENCES),
            _ => None,
        }
    }
}

impl std::ops::BitOr for LinkFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for LinkFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkDrawerType {
    #[default]
    None,
    Basic,
    CornerFiller,
    /// Parsed but not yet rendered; treated as `Basic` at emission time with
    /// a log-warn.
    Transmitter,
    /// Parsed but not yet rendered; treated as `Basic` at emission time with
    /// a log-warn.
    TransmitterOverlay,
    /// Parsed but not yet rendered; treated as `Basic` at emission time with
    /// a log-warn.
    Asymmetric,
}

impl LinkDrawerType {
    pub fn from_token(name: &str) -> Option<Self> {
        match name {
            "None" => Some(Self::None),
            "Basic" => Some(Self::Basic),
            "CornerFiller" => Some(Self::CornerFiller),
            "Transmitter" => Some(Self::Transmitter),
            "TransmitterOverlay" => Some(Self::TransmitterOverlay),
            "Asymmetric" => Some(Self::Asymmetric),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerrainEdgeType {
    #[default]
    None,
    Hard,
    FadeRough,
    Water,
}

impl TerrainEdgeType {
    pub fn from_token(name: &str) -> Option<Self> {
        match name {
            "None" => Some(Self::None),
            "Hard" => Some(Self::Hard),
            "FadeRough" => Some(Self::FadeRough),
            "Water" => Some(Self::Water),
            _ => None,
        }
    }
}

/// 4×4 atlas layout: each subimage is 3/16 wide with a 1/32 margin inside its
/// 1/4 quarter of the texture. Index 0 = bottom-left (Unity UV origin); index
/// 15 = top-right.
pub const ATLAS_SUBIMAGE_SIZE: f32 = 0.1875; // 3 / 16
pub const ATLAS_SUBIMAGE_MARGIN: f32 = 0.03125; // 1 / 32

/// UV sub-rect `[u_min, v_min, u_max, v_max]` for one of the 16 atlas cells.
/// Index layout: `(N ? 1 : 0) | (E ? 2 : 0) | (S ? 4 : 0) | (W ? 8 : 0)`.
pub fn atlas_uv_rect(index: u8) -> [f32; 4] {
    assert!(index < 16, "atlas index out of range: {index}");
    let col = (index % 4) as f32;
    let row = (index / 4) as f32;
    let u_min = col * 0.25 + ATLAS_SUBIMAGE_MARGIN;
    let v_min = row * 0.25 + ATLAS_SUBIMAGE_MARGIN;
    [
        u_min,
        v_min,
        u_min + ATLAS_SUBIMAGE_SIZE,
        v_min + ATLAS_SUBIMAGE_SIZE,
    ]
}

/// NESW bitmask for link-atlas lookup. `neighbor_flags` is indexed in the same
/// order as `crate::world::CARDINAL_OFFSETS`: N, E, S, W. `None` means
/// out-of-bounds, which only counts as a match if `self_flags` contains
/// `MAP_EDGE`.
pub fn link_index(self_flags: LinkFlags, neighbor_flags: [Option<LinkFlags>; 4]) -> u8 {
    let mut idx = 0u8;
    for (bit, flags) in neighbor_flags.iter().enumerate() {
        let links = match flags {
            Some(n) => n.intersects(self_flags),
            None => self_flags.contains(LinkFlags::MAP_EDGE),
        };
        if links {
            idx |= 1 << bit;
        }
    }
    idx
}

/// For each diagonal (NE, SE, SW, NW) return whether a corner-filler quad
/// should be emitted at that corner. A filler is emitted iff both orthogonal
/// neighbors link AND the diagonal cell itself links. Inputs are in the same
/// orderings as `CARDINAL_OFFSETS` (N, E, S, W) and `DIAGONAL_OFFSETS` (NE,
/// SE, SW, NW).
pub fn corner_filler_positions(
    cardinal_links: [bool; 4],
    diagonal_links: [bool; 4],
) -> [bool; 4] {
    // NE needs N(0) & E(1); SE needs S(2) & E(1); SW needs S(2) & W(3); NW needs N(0) & W(3).
    [
        cardinal_links[0] && cardinal_links[1] && diagonal_links[0],
        cardinal_links[2] && cardinal_links[1] && diagonal_links[1],
        cardinal_links[2] && cardinal_links[3] && diagonal_links[2],
        cardinal_links[0] && cardinal_links[3] && diagonal_links[3],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_flags_union_and_intersect() {
        let wall_rock = LinkFlags::WALL | LinkFlags::ROCK;
        assert!(wall_rock.contains(LinkFlags::WALL));
        assert!(wall_rock.contains(LinkFlags::ROCK));
        assert!(!wall_rock.contains(LinkFlags::SANDBAGS));
        assert!(wall_rock.intersects(LinkFlags::WALL));
        assert!(!wall_rock.intersects(LinkFlags::SANDBAGS));
    }

    #[test]
    fn link_flags_from_token_roundtrip() {
        assert_eq!(LinkFlags::from_token("Wall"), Some(LinkFlags::WALL));
        assert_eq!(LinkFlags::from_token("MapEdge"), Some(LinkFlags::MAP_EDGE));
        assert_eq!(
            LinkFlags::from_token("PowerConduit"),
            Some(LinkFlags::POWER_CONDUIT)
        );
        assert_eq!(LinkFlags::from_token("wall"), None); // case-sensitive
        assert_eq!(LinkFlags::from_token("Unknown"), None);
    }

    #[test]
    fn link_drawer_type_parse() {
        assert_eq!(LinkDrawerType::from_token("None"), Some(LinkDrawerType::None));
        assert_eq!(LinkDrawerType::from_token("Basic"), Some(LinkDrawerType::Basic));
        assert_eq!(
            LinkDrawerType::from_token("CornerFiller"),
            Some(LinkDrawerType::CornerFiller)
        );
        assert_eq!(
            LinkDrawerType::from_token("Transmitter"),
            Some(LinkDrawerType::Transmitter)
        );
        assert!(LinkDrawerType::from_token("bogus").is_none());
    }

    #[test]
    fn terrain_edge_type_parse() {
        assert_eq!(
            TerrainEdgeType::from_token("FadeRough"),
            Some(TerrainEdgeType::FadeRough)
        );
        assert_eq!(
            TerrainEdgeType::from_token("Water"),
            Some(TerrainEdgeType::Water)
        );
        assert_eq!(TerrainEdgeType::from_token("None"), Some(TerrainEdgeType::None));
        assert!(TerrainEdgeType::from_token("bogus").is_none());
    }

    #[test]
    fn atlas_uv_rect_has_expected_size_and_margin() {
        let rect = atlas_uv_rect(0);
        assert!((rect[0] - ATLAS_SUBIMAGE_MARGIN).abs() < 1e-6);
        assert!((rect[1] - ATLAS_SUBIMAGE_MARGIN).abs() < 1e-6);
        assert!((rect[2] - rect[0] - ATLAS_SUBIMAGE_SIZE).abs() < 1e-6);
        assert!((rect[3] - rect[1] - ATLAS_SUBIMAGE_SIZE).abs() < 1e-6);

        // Index 15 is top-right in a 4x4 grid (col=3, row=3).
        let last = atlas_uv_rect(15);
        assert!((last[0] - (0.75 + ATLAS_SUBIMAGE_MARGIN)).abs() < 1e-6);
        assert!((last[1] - (0.75 + ATLAS_SUBIMAGE_MARGIN)).abs() < 1e-6);
    }

    #[test]
    #[should_panic]
    fn atlas_uv_rect_rejects_out_of_range() {
        let _ = atlas_uv_rect(16);
    }

    #[test]
    fn link_index_nesw_bits() {
        let self_flags = LinkFlags::WALL;
        let w = LinkFlags::WALL;
        // No neighbors link.
        assert_eq!(
            link_index(self_flags, [Some(LinkFlags::EMPTY); 4]),
            0
        );
        // All four cardinals link -> 0b1111 = 15.
        assert_eq!(link_index(self_flags, [Some(w); 4]), 15);
        // Only N -> 1.
        assert_eq!(
            link_index(self_flags, [Some(w), None, None, None]),
            1
        );
        // Only E -> 2.
        assert_eq!(
            link_index(self_flags, [None, Some(w), None, None]),
            2
        );
        // Only S -> 4.
        assert_eq!(
            link_index(self_flags, [None, None, Some(w), None]),
            4
        );
        // Only W -> 8.
        assert_eq!(
            link_index(self_flags, [None, None, None, Some(w)]),
            8
        );
    }

    #[test]
    fn link_index_map_edge_counts_oob_as_link() {
        let rock = LinkFlags::ROCK | LinkFlags::MAP_EDGE;
        // All four neighbors are out-of-bounds, and we have MapEdge -> 15.
        assert_eq!(link_index(rock, [None; 4]), 15);
        // Same but without MapEdge -> 0.
        assert_eq!(link_index(LinkFlags::ROCK, [None; 4]), 0);
    }

    #[test]
    fn link_index_mismatched_flags_dont_link() {
        // Wall next to Sandbags: no overlap -> no link.
        let idx = link_index(
            LinkFlags::WALL,
            [Some(LinkFlags::SANDBAGS); 4],
        );
        assert_eq!(idx, 0);

        // Wall next to Wall|Rock: overlap on WALL -> all link.
        let idx = link_index(
            LinkFlags::WALL,
            [Some(LinkFlags::WALL | LinkFlags::ROCK); 4],
        );
        assert_eq!(idx, 15);
    }

    #[test]
    fn corner_filler_all_four_when_plus_surrounded() {
        // N, E, S, W all link, and all four diagonals link -> all four corners fill.
        assert_eq!(
            corner_filler_positions([true; 4], [true; 4]),
            [true; 4]
        );
    }

    #[test]
    fn corner_filler_requires_both_orthogonals_and_diagonal() {
        // N=t, E=f, S=t, W=t with all diagonals present:
        //   NE needs E -> no; SE needs E -> no; SW needs S+W -> yes; NW needs N+W -> yes.
        assert_eq!(
            corner_filler_positions([true, false, true, true], [true; 4]),
            [false, false, true, true]
        );
        // All orthogonals link but NE diagonal is missing -> NE no, others yes.
        assert_eq!(
            corner_filler_positions([true; 4], [false, true, true, true]),
            [false, true, true, true]
        );
        // No orthogonals link -> no corner fills even with diagonals present.
        assert_eq!(
            corner_filler_positions([false; 4], [true; 4]),
            [false; 4]
        );
    }
}
