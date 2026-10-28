//! Starting a new game: a generated New World, its native peoples, and the
//! four colonial powers arriving by sea.

use crate::game::*;
use crate::rules::Rules;
use crate::types::{Control, Tech, Tile, UnitType};

pub const MAP_W: usize = 58;
pub const MAP_H: usize = 72;

/// Settings for a generated New World.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct World {
    /// 0 small, 1 moderate, 2 large.
    pub land: u8,
    /// 0 islands, 1 one great continent.
    pub shape: u8,
    /// 0 cool, 1 temperate, 2 warm.
    pub temperature: u8,
    /// 0 arid, 1 normal, 2 wet.
    pub climate: u8,
}

impl Default for World {
    fn default() -> Self {
        World { land: 1, shape: 1, temperature: 1, climate: 1 }
    }
}

impl World {
    pub fn describe(&self) -> String {
        format!(
            "{} {}, {}, {}",
            ["Small", "Moderate", "Large"][self.land.min(2) as usize],
            ["islands", "continent"][self.shape.min(1) as usize],
            ["cool", "temperate", "warm"][self.temperature.min(2) as usize],
            ["arid", "normal", "wet"][self.climate.min(2) as usize]
        )
    }
}

pub struct NewGame {
    pub human: u8,
    pub difficulty: u8,
    pub world: World,
    pub seed: u64,
}

// raw tile codes
const OCEAN: u8 = 25;
const SEA_LANE: u8 = 26;
const ARCTIC: u8 = 24;
const T_TUNDRA: u8 = 0;
const T_DESERT: u8 = 1;
const T_PLAINS: u8 = 2;
const T_PRAIRIE: u8 = 3;
const T_GRASS: u8 = 4;
const T_SAVANNAH: u8 = 5;
const T_MARSH: u8 = 6;
const T_SWAMP: u8 = 7;
const HILLS: u8 = 1 << 5;
const MINOR_RIVER: u8 = 2 << 5;
const MOUNTAINS: u8 = 5 << 5;
const MAJOR_RIVER: u8 = 6 << 5;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn n(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { (self.next() % n as u64) as u32 }
    }
}

/// Smooth value noise in 0..1 with grid spacing `cell`.
fn value_noise(rng: &mut Rng, w: usize, h: usize, cell: usize) -> Vec<f32> {
    let gw = w / cell + 2;
    let gh = h / cell + 2;
    let grid: Vec<f32> = (0..gw * gh).map(|_| rng.f()).collect();
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / cell as f32, y as f32 / cell as f32);
            let (ix, iy) = (fx as usize, fy as usize);
            let (tx, ty) = (fx - ix as f32, fy - iy as f32);
            let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
            let g = |gx: usize, gy: usize| grid[gy.min(gh - 1) * gw + gx.min(gw - 1)];
            let a = g(ix, iy) * (1.0 - sx) + g(ix + 1, iy) * sx;
            let b = g(ix, iy + 1) * (1.0 - sx) + g(ix + 1, iy + 1) * sx;
            out[y * w + x] = a * (1.0 - sy) + b * sy;
        }
    }
    out
}

/// Generate a New World with the default settings.
pub fn random_map(seed: u64) -> Vec<Tile> {
    random_world(seed, World::default())
}

/// Generate a New World: a long continent (or a scatter of islands), ocean
/// on both sides and high seas along the east and west edges.
pub fn random_world(seed: u64, world: World) -> Vec<Tile> {
    let (w, h) = (MAP_W, MAP_H);
    let mut rng = Rng(seed | 1);
    let n1 = value_noise(&mut rng, w, h, 9);
    let n2 = value_noise(&mut rng, w, h, 4);
    let wet = value_noise(&mut rng, w, h, 7);
    let wobble = value_noise(&mut rng, 1, h, 10);
    let width_n = value_noise(&mut rng, 1, h, 12);
    let n3 = value_noise(&mut rng, w, h, 6);

    let islands = world.shape == 0;
    let mut elev = vec![0.0f32; w * h];
    for y in 0..h {
        let cx = w as f32 * (0.42 + 0.22 * (wobble[y] - 0.5) * 2.0);
        let half = 6.0 + 10.0 * width_n[y];
        for x in 0..w {
            let i = y * w + x;
            let e = if islands {
                // broken land everywhere, a little thicker towards the middle
                let mid = 1.0 - ((x as f32 - w as f32 * 0.45).abs() / (w as f32 * 0.5));
                0.45 * n3[i] + 0.45 * n2[i] + 0.15 * n1[i] + 0.2 * mid
            } else {
                let bias = 1.0 - ((x as f32 - cx).abs() / half);
                0.55 * n1[i] + 0.3 * n2[i] + 0.55 * bias
            };
            elev[i] = e;
        }
    }
    // pick the sea level that gives the wanted amount of land
    let inside = |x: usize, y: usize| x >= 3 && x < w - 4 && y >= 1 && y < h - 1;
    let mut heights: Vec<f32> = (0..w * h).filter(|i| inside(i % w, i / w)).map(|i| elev[i]).collect();
    heights.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let share = [0.24, 0.33, 0.43][world.land.min(2) as usize] * if islands { 0.85 } else { 1.0 };
    let sea = heights[((heights.len() as f32 * share) as usize).min(heights.len() - 1)];
    let mut land = vec![false; w * h];
    for i in 0..w * h {
        elev[i] -= sea;
        land[i] = elev[i] > 0.0 && inside(i % w, i / w);
    }
    // rescale so mountains and rivers see the same range as before
    let top = elev.iter().cloned().fold(0.01f32, f32::max);
    for e in elev.iter_mut() {
        *e = *e / top * 0.6;
    }
    // a few island chains
    for _ in 0..[6, 10, 12][world.land.min(2) as usize] {
        let (ix, iy) = (4 + rng.n(w as u32 - 9) as i32, 4 + rng.n(h as u32 - 8) as i32);
        for _ in 0..(2 + rng.n(5)) {
            let (x, y) = (ix + rng.n(5) as i32 - 2, iy + rng.n(5) as i32 - 2);
            if x >= 3 && x < w as i32 - 4 && y >= 2 && y < h as i32 - 2 {
                land[y as usize * w + x as usize] = true;
            }
        }
    }

    let mut tiles = vec![Tile { raw: OCEAN, mask: 0, path: 0xF0, seen: 0 }; w * h];
    for y in 0..h {
        // where the continent starts on this row, for marking the far ocean
        let west_land = (0..w).find(|&x| land[y * w + x]).unwrap_or(w);
        for x in 0..w {
            let i = y * w + x;
            let t = &mut tiles[i];
            if !land[i] {
                t.raw = if x < 2 || x >= w - 3 { SEA_LANE } else { OCEAN };
                if x < west_land {
                    t.mask |= 0x20;
                }
                continue;
            }
            let lat = ((y as f32 - h as f32 / 2.0).abs() / (h as f32 / 2.0)).min(1.0);
            // cooler worlds behave as if further from the equator
            let lat = (lat + [0.16, 0.0, -0.16][world.temperature.min(2) as usize]).clamp(0.0, 1.0);
            let m = (wet[i] + [-0.22, 0.0, 0.2][world.climate.min(2) as usize]).clamp(0.0, 1.0);
            if y <= 2 || y >= h - 3 {
                t.raw = ARCTIC;
                continue;
            }
            let base = if lat > 0.82 {
                T_TUNDRA
            } else if lat > 0.45 {
                if m < 0.3 { T_PRAIRIE } else if m < 0.55 { T_PLAINS } else if m < 0.8 { T_GRASS } else { T_MARSH }
            } else if lat > 0.22 {
                if m < 0.22 { T_DESERT } else if m < 0.45 { T_PRAIRIE } else if m < 0.7 { T_SAVANNAH } else { T_MARSH }
            } else if m < 0.3 {
                T_SAVANNAH
            } else if m < 0.6 {
                T_GRASS
            } else {
                T_SWAMP
            };
            let forest = m > 0.3 && rng.f() < 0.45 + m * 0.35 && base != T_DESERT;
            t.raw = base + if forest { 8 } else { 0 };
        }
    }

    // mountains along the west coast and on high ground, hills around them
    for y in 3..h - 3 {
        let west_land = (0..w).find(|&x| land[y * w + x] && tiles[y * w + x].raw != ARCTIC);
        if let Some(wx) = west_land {
            for dx in 1..4 {
                let x = wx + dx;
                if x < w && land[y * w + x] && rng.f() < 0.55 {
                    tiles[y * w + x].raw = (tiles[y * w + x].raw & 0x1F & !8) | MOUNTAINS;
                }
            }
        }
    }
    for i in 0..w * h {
        if !land[i] || tiles[i].raw == ARCTIC || tiles[i].raw >> 5 != 0 {
            continue;
        }
        if elev[i] > 0.42 && rng.f() < 0.5 {
            tiles[i].raw = (tiles[i].raw & 0x17) | MOUNTAINS;
        } else if (elev[i] > 0.28 && rng.f() < 0.45) || rng.f() < 0.04 {
            tiles[i].raw = (tiles[i].raw & 0x17) | HILLS;
        }
    }

    // rivers run downhill from the high ground to the sea
    let sources: Vec<usize> = (0..w * h)
        .filter(|&i| land[i] && matches!(tiles[i].raw >> 5, 1 | 5) && tiles[i].raw != ARCTIC)
        .collect();
    for r in 0..14 {
        if sources.is_empty() {
            break;
        }
        let mut cur = sources[rng.n(sources.len() as u32) as usize];
        let major = r < 4;
        let mut visited = Vec::new();
        for step in 0..40 {
            let (x, y) = ((cur % w) as i32, (cur / w) as i32);
            visited.push(cur);
            let mut best: Option<(f32, usize)> = None;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let ni = ny as usize * w + nx as usize;
                if visited.contains(&ni) {
                    continue;
                }
                let score = elev[ni] + rng.f() * 0.08;
                if best.is_none_or(|(b, _)| score < b) {
                    best = Some((score, ni));
                }
            }
            let Some((_, next)) = best else { break };
            if !land[next] {
                break;
            }
            let t = &mut tiles[next];
            if t.raw == ARCTIC || t.raw >> 5 == 5 {
                cur = next;
                continue;
            }
            let hills = t.raw >> 5 == 1;
            let kind = if major && step > 4 { MAJOR_RIVER } else if hills { HILLS | MINOR_RIVER } else { MINOR_RIVER };
            t.raw = (t.raw & 0x1F) | kind;
            cur = next;
        }
    }
    tiles
}

impl Game {
    /// Start a fresh game.
    pub fn new_game(rules: Rules, opts: &NewGame) -> Game {
        let mut rng = Rng(opts.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1);
        let tiles = random_world(opts.seed, opts.world);
        let (w, h) = (MAP_W, MAP_H);
        let mut g = Game {
            width: w,
            height: h,
            tiles,
            resource: vec![255; w * h],
            rumour: vec![false; w * h],
            year: 1500,
            autumn: false,
            turn: 0,
            human: opts.human,
            difficulty: opts.difficulty,
            players: Vec::new(),
            units: Vec::new(),
            colonies: Vec::new(),
            villages: Vec::new(),
            tribes: Vec::new(),
            next_id: 1,
            rng: opts.seed ^ 0x5DEE_CE66_D1CE_4E5B,
            messages: Vec::new(),
            crown_force: [
                20 + 5 * opts.difficulty as i32,
                8 + 3 * opts.difficulty as i32,
                3 + opts.difficulty as i32,
                6 + 2 * opts.difficulty as i32,
            ],
            rebellion: None,
            game_over: None,
            milestones: 0,
            met_tribes: 0,
            events: Vec::new(),
            far_ocean_cache: Vec::new(),
            sounds: Vec::new(),
            ai_settlers: Vec::new(),
            tax_demand: None,
            land_settled: false,
            mercenary_turn: 0,
            rules,
        };

        // native tribes
        for i in 0..8usize {
            let tech = g.rules.tribes.get(i).map(|t| t.1).unwrap_or(0);
            let tech = match tech {
                0 => Tech::SemiNomadic,
                1 => Tech::Agrarian,
                2 => Tech::Advanced,
                _ => Tech::Civilized,
            };
            g.tribes.push(Tribe { tech, alarm: [0; 4], extinct: false });
        }
        let sites = g.random_village_sites(&mut rng);
        let mut placed_tribe = [false; 12];
        for (tribe, x, y) in sites {
            if g.is_water(x, y) || g.village_at(x, y).is_some() {
                continue;
            }
            if DIRS8.iter().any(|(dx, dy)| g.village_at(x + dx, y + dy).is_some()) {
                continue;
            }
            let capital = !placed_tribe[tribe as usize];
            placed_tribe[tribe as usize] = true;
            let tech = g.tribe_tech(tribe) as u8;
            let teaches = Some(g.village_skill(x, y, tribe));
            g.villages.push(Village {
                x: x as i16,
                y: y as i16,
                tribe,
                pop: (2 + rng.n(3) + tech as u32 * 2) as u8 + if capital { 2 } else { 0 },
                capital,
                scouted: 0,
                teaches,
                mission: None,
                wealth: 50 + 100 * (tech as u32 + 1),
            });
        }

        // the four colonial powers, approaching from the east
        let mut rows: Vec<i32> = vec![h as i32 * 22 / 100, h as i32 * 38 / 100, h as i32 * 55 / 100, h as i32 * 70 / 100];
        for i in (1..4).rev() {
            let j = rng.n(i as u32 + 1) as usize;
            rows.swap(i, j);
        }
        let leaders = g.rules.leaders.clone();
        let difficulty_gold = [1000, 600, 300, 100, 0][opts.difficulty.min(4) as usize];
        for n in 0..4u8 {
            let mut prices = [0u8; 16];
            for (gd, p) in prices.iter_mut().enumerate() {
                let r = &g.rules.cargo[gd];
                *p = (r.start_lo as u32 + rng.n((r.start_hi.saturating_sub(r.start_lo)) as u32 + 1)) as u8;
                *p = (*p).max(1);
            }
            let y = rows[n as usize];
            let x = (0..w as i32).rev().find(|&x| g.tile(x, y).is_some_and(|t| t.raw & 0x1F == SEA_LANE)).unwrap_or(w as i32 - 2);
            let recruits = [g.random_immigrant(), g.random_immigrant(), g.random_immigrant()];
            g.players.push(Player {
                name: leaders.get(n as usize).cloned().unwrap_or_else(|| format!("Leader {n}")),
                control: if n == opts.human { Control::Human } else { Control::Ai },
                gold: difficulty_gold,
                tax: 0,
                bells: 0,
                crosses: 0,
                crosses_needed: 40 + 8 * opts.difficulty as i32,
                recruits,
                prices,
                volume: [0; 16],
                entry: (x as i16, y as i16),
                artillery_bought: 0,
                colonies_founded: 0,
                at_war: [false; 12],
                tension: [0; 4],
                war_turns: [0; 4],
                boycott: [false; 16],
                assembly: Assembly::default(),
            });
            let ship_kind = if n == 3 { UnitType::Merchantman } else { UnitType::Caravel };
            let ship = g.spawn(ship_kind, n, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
            let soldier = g.spawn(UnitType::Soldier, n, P_VETERAN, Loc::Aboard(ship));
            let pioneer = g.spawn(UnitType::Pioneer, n, P_FREE, Loc::Aboard(ship));
            for id in [soldier, pioneer] {
                g.set_orders(id, Orders::Sentry);
            }
            if n == opts.human {
                g.spawn(UnitType::Colonist, n, P_FREE, Loc::Homeland);
            }
            g.reveal(n, x, y, 1);
        }
        for u in &mut g.units {
            u.moves = g.rules.units.get(u.kind as usize).map(|r| r.movement * 3).unwrap_or(3);
        }
        g.place_resources(rng.next());
        // no rumours or prime resources under villages
        for v in &g.villages {
            let i = v.y as usize * w + v.x as usize;
            g.rumour[i] = false;
        }
        g.milestone(crate::events::MS_NEW_WORLD);
        g
    }

    /// Village sites for a random map: each tribe settles around its own region.
    fn random_village_sites(&self, rng: &mut Rng) -> Vec<(u8, i32, i32)> {
        let (w, h) = (self.width as f32, self.height as f32);
        // (tribe, relative position of its homeland)
        let homes: [(u8, f32, f32); 8] = [
            (10, 0.30, 0.13), // Shanoka, plains riders of the north
            (7, 0.55, 0.16),  // Onaweh, northern farmers
            (9, 0.30, 0.28),  // Durrak, the dry west
            (8, 0.52, 0.30),  // Kessani, the river valleys
            (5, 0.35, 0.42),  // Itzari, the council cities
            (6, 0.62, 0.45),  // Marawe, the coasts and islands
            (4, 0.32, 0.70),  // Ankaru, the southern mountains
            (11, 0.60, 0.66), // Ilvae, the southern forests
        ];
        let mut out = Vec::new();
        for (tribe, hx, hy) in homes {
            let (cx, cy) = ((hx * w) as i32, (hy * h) as i32);
            let count = 4 + rng.n(5);
            let mut placed = 0;
            for _ in 0..200 {
                if placed >= count {
                    break;
                }
                let x = cx + rng.n(17) as i32 - 8;
                let y = cy + rng.n(13) as i32 - 6;
                let Some(t) = self.tile(x, y) else { continue };
                if t.terrain().is_water() || t.raw & 0x1F == ARCTIC || t.mountains() {
                    continue;
                }
                if out.iter().any(|&(_, ox, oy): &(u8, i32, i32)| (ox - x).abs() <= 2 && (oy - y).abs() <= 2) {
                    continue;
                }
                out.push((tribe, x, y));
                placed += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_maps_have_land_and_sea_lanes() {
        for seed in 1..20 {
            let t = random_map(seed);
            let land = t.iter().filter(|t| !t.terrain().is_water()).count();
            let lanes = t.iter().filter(|t| t.raw & 0x1F == SEA_LANE).count();
            assert!(land > 600 && land < 2500, "seed {seed}: {land} land squares");
            assert!(lanes > 100);
        }
    }
}
