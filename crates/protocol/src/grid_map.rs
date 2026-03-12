//! Grid map definition and parsing

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use crate::config::warehouse::SHELF_MAX_CAPACITY;

/// Tile types in the warehouse grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileType {
    Empty,      // ~ in layout
    Ground,     // .
    Wall,       // #
    Shelf(u8),  // xN (N = initial stock; max capacity is warehouse::SHELF_MAX_CAPACITY)
    Station,    // _ (charging station)
    Dropoff,    // v
}

/// A single tile with position and type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub x: usize,
    pub y: usize,
    pub tile_type: TileType,
}

/// The warehouse grid map - loaded from layout file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridMap {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Tile>,
    pub hash: u64, // For validation across crates
}

impl GridMap {
    /// Load map from layout.txt file
    pub fn load_from_file(path: &str) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read layout file: {}", e))?;
        Self::parse(&contents)
    }

    /// Parse layout from string (for testing or embedded data)
    pub fn parse(contents: &str) -> Result<Self, String> {
        let mut tiles = Vec::new();
        let mut width = 0;
        let mut height = 0;

        for (_y, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            
            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with('/') {
                continue;
            }

            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            width = width.max(tokens.len());

            for (x, token) in tokens.iter().enumerate() {
                let tile_type = Self::parse_token(token);
                tiles.push(Tile {
                    x,
                    y: height,
                    tile_type,
                });
            }
            height += 1;
        }

        let mut map = GridMap {
            width,
            height,
            tiles,
            hash: 0,
        };
        map.hash = map.calculate_hash();
        Ok(map)
    }

    fn parse_token(token: &str) -> TileType {
        match token {
            "~" => TileType::Empty,
            "." => TileType::Ground,
            "#" => TileType::Wall,
            "_" => TileType::Station,
            "v" => TileType::Dropoff,
            t if t.starts_with('x') && t.len() > 1 => {
                let capacity = match t[1..].parse() {
                    Ok(c) => c,
                    Err(e) => {
                        println!("Warning: failed to parse shelf capacity from token '{}': 
                            {}. Defaulting to 5.",t, e);
                        5
                    }
                };
                TileType::Shelf(capacity)
            }
            _ => TileType::Empty,
        }
    }

    /// Calculate hash for map validation
    fn calculate_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.width.hash(&mut hasher);
        self.height.hash(&mut hasher);
        for tile in &self.tiles {
            tile.x.hash(&mut hasher);
            tile.y.hash(&mut hasher);
            tile.tile_type.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Check if a position is walkable
    pub fn is_walkable(&self, x: usize, y: usize) -> bool {
        self.tiles.iter()
            .find(|t| t.x == x && t.y == y)
            .map(|t| matches!(t.tile_type, TileType::Ground | TileType::Station | TileType::Dropoff))
            .unwrap_or(false)
    }

    /// Get tile at position
    pub fn get_tile(&self, x: usize, y: usize) -> Option<&Tile> {
        self.tiles.iter().find(|t| t.x == x && t.y == y)
    }

    /// Get all tiles of a specific type
    pub fn get_tiles_of_type(&self, tile_type: TileType) -> Vec<&Tile> {
        self.tiles.iter().filter(|t| t.tile_type == tile_type).collect()
    }

    /// Get all station tiles
    pub fn get_stations(&self) -> Vec<&Tile> {
        self.tiles.iter().filter(|t| matches!(t.tile_type, TileType::Station)).collect()
    }

    /// Get all shelf tiles
    pub fn get_shelves(&self) -> Vec<&Tile> {
        self.tiles.iter().filter(|t| matches!(t.tile_type, TileType::Shelf(_))).collect()
    }

    /// Get all dropoff tiles
    pub fn get_dropoffs(&self) -> Vec<&Tile> {
        self.tiles.iter().filter(|t| matches!(t.tile_type, TileType::Dropoff)).collect()
    }
}

/// Map validation message sent on startup
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MapValidation {
    pub sender: String,     // "coordinator", "firmware", "renderer"
    pub map_hash: u64,
    pub map_dimensions: (usize, usize),
}

// ── Shelf inventory tracking ──

/// Tracks current stock levels for all shelf tiles.
///
/// Both the scheduler (reservation tracking) and coordinator (actual tracking)
/// can maintain independent instances initialized from the same GridMap.
/// Shelves start at full capacity.
#[derive(Debug, Clone)]
pub struct ShelfInventory {
    /// shelf grid (x, y) → (current_stock, max_capacity)
    shelves: HashMap<(usize, usize), (u8, u8)>,
}

impl ShelfInventory {
    /// Initialize from a GridMap. Each shelf starts with the stock defined by
    /// its layout token (xN = N items). Max capacity is warehouse::SHELF_MAX_CAPACITY.
    pub fn from_map(map: &GridMap) -> Self {
        let mut shelves = HashMap::new();
        for tile in &map.tiles {
            if let TileType::Shelf(initial_stock) = tile.tile_type {
                let max = SHELF_MAX_CAPACITY as u8;
                let stock = initial_stock.min(max);
                shelves.insert((tile.x, tile.y), (stock, max));
            }
        }
        ShelfInventory { shelves }
    }

    /// check if a position has stock available for pickup.
    /// Non-shelf positions always return true (no constraint).
    pub fn can_pickup(&self, pos: (usize, usize)) -> bool {
        self.shelves.get(&pos).map_or(true, |&(stock, _)| stock > 0)
    }

    /// check if a position has room for a delivery.
    /// Non-shelf positions always return true (no constraint).
    pub fn can_dropoff(&self, pos: (usize, usize)) -> bool {
        self.shelves.get(&pos).map_or(true, |&(stock, cap)| stock < cap)
    }

    /// decrement stock at a shelf (pickup or pickup reservation).
    /// Returns false if shelf is empty or position is not a shelf.
    pub fn pickup(&mut self, pos: (usize, usize)) -> bool {
        if let Some((stock, _)) = self.shelves.get_mut(&pos) {
            if *stock > 0 {
                *stock -= 1;
                return true;
            }
            return false;
        }
        true // not a shelf, no constraint
    }

    /// increment stock at a shelf (dropoff or dropoff reservation).
    /// Returns false if shelf is full or position is not a shelf.
    pub fn dropoff(&mut self, pos: (usize, usize)) -> bool {
        if let Some((stock, cap)) = self.shelves.get_mut(&pos) {
            if *stock < *cap {
                *stock += 1;
                return true;
            }
            return false;
        }
        true // not a shelf, no constraint
    }

    /// undo a pickup reservation (re-add stock)
    pub fn undo_pickup(&mut self, pos: (usize, usize)) {
        if let Some((stock, cap)) = self.shelves.get_mut(&pos) {
            if *stock < *cap {
                *stock += 1;
            }
        }
    }

    /// undo a dropoff reservation (remove stock)
    pub fn undo_dropoff(&mut self, pos: (usize, usize)) {
        if let Some((stock, _)) = self.shelves.get_mut(&pos) {
            if *stock > 0 {
                *stock -= 1;
            }
        }
    }

    /// get (current_stock, max_capacity) for a shelf position.
    /// Returns None if the position is not a shelf.
    pub fn stock_at(&self, pos: (usize, usize)) -> Option<(u8, u8)> {
        self.shelves.get(&pos).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_map() {
        let layout = "# # #\n# . #\n# # #";
        let map = GridMap::parse(layout).unwrap();
        
        assert_eq!(map.width, 3);
        assert_eq!(map.height, 3);
        assert_eq!(map.tiles.len(), 9);
    }

    #[test]
    fn test_parse_all_tile_types() {
        let layout = "# . ~ _ v x5";
        let map = GridMap::parse(layout).unwrap();
        
        assert_eq!(map.tiles[0].tile_type, TileType::Wall);
        assert_eq!(map.tiles[1].tile_type, TileType::Ground);
        assert_eq!(map.tiles[2].tile_type, TileType::Empty);
        assert_eq!(map.tiles[3].tile_type, TileType::Station);
        assert_eq!(map.tiles[4].tile_type, TileType::Dropoff);
        assert_eq!(map.tiles[5].tile_type, TileType::Shelf(5));
    }

    #[test]
    fn test_is_walkable() {
        let layout = "# . _ v x5";
        let map = GridMap::parse(layout).unwrap();
        
        assert!(!map.is_walkable(0, 0)); // Wall
        assert!(map.is_walkable(1, 0));  // Ground
        assert!(map.is_walkable(2, 0));  // Station
        assert!(map.is_walkable(3, 0));  // Dropoff
        assert!(!map.is_walkable(4, 0)); // Shelf (not walkable)
    }

    #[test]
    fn test_get_tile() {
        let layout = "# .\n. #";
        let map = GridMap::parse(layout).unwrap();
        
        assert_eq!(map.get_tile(0, 0).unwrap().tile_type, TileType::Wall);
        assert_eq!(map.get_tile(1, 0).unwrap().tile_type, TileType::Ground);
        assert_eq!(map.get_tile(0, 1).unwrap().tile_type, TileType::Ground);
        assert_eq!(map.get_tile(1, 1).unwrap().tile_type, TileType::Wall);
        assert!(map.get_tile(5, 5).is_none());
    }

    #[test]
    fn test_hash_consistency() {
        let layout = "# . #\n# . #";
        let map1 = GridMap::parse(layout).unwrap();
        let map2 = GridMap::parse(layout).unwrap();
        
        assert_eq!(map1.hash, map2.hash);
    }

    #[test]
    fn test_hash_differs_for_different_maps() {
        let map1 = GridMap::parse("# . #").unwrap();
        let map2 = GridMap::parse("# # #").unwrap();
        
        assert_ne!(map1.hash, map2.hash);
    }

    #[test]
    fn test_get_shelves_dropoffs_stations() {
        let layout = "x5 x3 . _ _ v";
        let map = GridMap::parse(layout).unwrap();
        
        assert_eq!(map.get_shelves().len(), 2);
        assert_eq!(map.get_stations().len(), 2);
        assert_eq!(map.get_dropoffs().len(), 1);
    }

    #[test]
    fn test_skip_comments_and_empty_lines() {
        let layout = "// This is a comment\n\n# . #\n\n// Another comment\n# . #";
        let map = GridMap::parse(layout).unwrap();
        
        assert_eq!(map.height, 2);
        assert_eq!(map.tiles.len(), 6);
    }

    // ── ShelfInventory tests ──

    #[test]
    fn inventory_from_map_starts_full() {
        let map = GridMap::parse("x5 . x3").unwrap();
        let inv = ShelfInventory::from_map(&map);
        // stock starts at parsed value, capacity is SHELF_MAX_CAPACITY (16)
        assert_eq!(inv.stock_at((0, 0)), Some((5, 16)));
        assert_eq!(inv.stock_at((2, 0)), Some((3, 16)));
        assert_eq!(inv.stock_at((1, 0)), None); // ground
    }

    #[test]
    fn inventory_can_pickup_full_shelf() {
        let map = GridMap::parse("x5").unwrap();
        let inv = ShelfInventory::from_map(&map);
        assert!(inv.can_pickup((0, 0)));
    }

    #[test]
    fn inventory_cannot_pickup_empty_shelf() {
        let map = GridMap::parse("x5").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        for _ in 0..5 { inv.pickup((0, 0)); }
        assert!(!inv.can_pickup((0, 0)));
    }

    #[test]
    fn inventory_can_dropoff_empty_shelf() {
        let map = GridMap::parse("x3").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        for _ in 0..3 { inv.pickup((0, 0)); }
        assert!(inv.can_dropoff((0, 0)));
    }

    #[test]
    fn inventory_cannot_dropoff_full_shelf() {
        // x16 starts at capacity (SHELF_MAX_CAPACITY = 16)
        let map = GridMap::parse("x16").unwrap();
        let inv = ShelfInventory::from_map(&map);
        assert!(!inv.can_dropoff((0, 0))); // starts full
    }

    #[test]
    fn inventory_pickup_decrements() {
        let map = GridMap::parse("x3").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        assert!(inv.pickup((0, 0)));
        assert_eq!(inv.stock_at((0, 0)), Some((2, 16)));
    }

    #[test]
    fn inventory_dropoff_increments() {
        let map = GridMap::parse("x3").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        inv.pickup((0, 0));
        assert!(inv.dropoff((0, 0)));
        assert_eq!(inv.stock_at((0, 0)), Some((3, 16)));
    }

    #[test]
    fn inventory_undo_pickup() {
        let map = GridMap::parse("x3").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        inv.pickup((0, 0));
        assert_eq!(inv.stock_at((0, 0)), Some((2, 16)));
        inv.undo_pickup((0, 0));
        assert_eq!(inv.stock_at((0, 0)), Some((3, 16)));
    }

    #[test]
    fn inventory_undo_dropoff() {
        let map = GridMap::parse("x3").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        inv.pickup((0, 0));
        inv.dropoff((0, 0));
        inv.undo_dropoff((0, 0));
        assert_eq!(inv.stock_at((0, 0)), Some((2, 16)));
    }

    #[test]
    fn inventory_non_shelf_always_allowed() {
        let map = GridMap::parse(". v _").unwrap();
        let mut inv = ShelfInventory::from_map(&map);
        assert!(inv.can_pickup((0, 0)));
        assert!(inv.can_dropoff((1, 0)));
        assert!(inv.pickup((0, 0)));
        assert!(inv.dropoff((1, 0)));
    }
}
