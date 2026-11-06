//! Synthetic dataset generator for the fictional "Cascade City".
//!
//! `casefile datagen [--out data/seed] [--seed 7] [--cases 2000]`
//!
//! Deterministic (splitmix64). Places come from the public Seattle points extract in
//! `data/public/seattle_burglary_points.csv`; everything else (offenders, MOs, items, pawn
//! records, narratives, arrestee refs) is made up. Local time is Pacific standard (UTC-8) and the
//! dataset ends at 2026-11-05 08:00 local.
//!
//! Outputs under `--out`: `dataset.json`, `ground_truth.json`, `demo.json`.

use anyhow::{bail, Context};
use casefile_core::geo::haversine_km;
use casefile_core::{
    Case, CaseStatus, Dataset, Decoy, EntryMethod, EntryPoint, GeoPoint, GroundTruth, Mo, PawnRecord,
    Premise, PropertyType, SolvabilityFacts, StolenItem, TimeBand, Ts,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const TZ: i64 = 8 * 3600;
const DAY: i64 = 86_400;
const N_SERIES: usize = 40;
const N_DECOYS: usize = 10;
const N_QUEUE: usize = 36;
const CLEARANCE_RATE: f64 = 0.13;
const PAWN_TARGET: usize = 1500;
const MIN_CASES: usize = 400;

fn today() -> i64 {
    days_from_civil(2026, 11, 5)
}
/// "Now" in local seconds (2026-11-05 08:00 local).
fn now_local() -> i64 {
    today() * DAY + 8 * 3600
}
fn now_ts() -> Ts {
    now_local() + TZ
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Monday-Friday? (epoch day 0 was a Thursday)
fn is_weekday(day: i64) -> bool {
    (day + 3).rem_euclid(7) < 5
}

const DOW: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
const MON: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

fn date_str(l: i64) -> String {
    let d = l.div_euclid(DAY);
    let (_, m, dd) = civil_from_days(d);
    let dow = DOW[(d + 3).rem_euclid(7) as usize];
    let mon = MON[(m - 1).clamp(0, 11) as usize];
    format!("{dow} {dd:02}{mon}")
}
fn hhmm(l: i64) -> String {
    let s = l.rem_euclid(DAY);
    format!("{:02}{:02}", s / 3600, (s % 3600) / 60)
}
fn hhmm_end(l: i64) -> String {
    if l.rem_euclid(DAY) == 0 {
        "2400".to_string()
    } else {
        hhmm(l)
    }
}

fn band_of_hour(h: i64) -> TimeBand {
    match h {
        0..=5 => TimeBand::Night,
        6..=9 => TimeBand::Morning,
        10..=15 => TimeBand::Midday,
        16..=19 => TimeBand::Evening,
        _ => TimeBand::Late,
    }
}
fn band_hours(b: TimeBand) -> (i64, i64) {
    match b {
        TimeBand::Night | TimeBand::Unknown => (0, 6),
        TimeBand::Morning => (6, 10),
        TimeBand::Midday => (10, 16),
        TimeBand::Evening => (16, 20),
        TimeBand::Late => (20, 24),
    }
}

// ---------------------------------------------------------------------------------------------
// PRNG
// ---------------------------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    /// Inclusive range.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as usize) as i64
    }
    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
    fn weighted<T: Copy>(&mut self, xs: &[(T, u32)]) -> T {
        let total: u32 = xs.iter().map(|x| x.1).sum();
        let mut r = self.below(total.max(1) as usize) as u32;
        for (v, w) in xs {
            if r < *w {
                return *v;
            }
            r -= *w;
        }
        xs[0].0
    }
}

// ---------------------------------------------------------------------------------------------
// Public points
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Pt {
    block: String,
    lat: f64,
    lon: f64,
    beat: String,
    neigh: String,
}

pub fn load_points(path: &Path) -> anyhow::Result<Vec<Pt>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {} (see data/public/SOURCE.md)", path.display()))?;
    let mut out = vec![];
    for line in text.lines().skip(1) {
        let parts: Vec<&str> = line.rsplitn(5, ',').collect();
        if parts.len() != 5 {
            continue;
        }
        let (Ok(lat), Ok(lon)) = (parts[3].trim().parse::<f64>(), parts[2].trim().parse::<f64>()) else {
            continue;
        };
        out.push(Pt {
            block: parts[4].trim().trim_matches('"').to_string(),
            lat,
            lon,
            beat: parts[1].trim().to_string(),
            neigh: parts[0].trim().to_string(),
        });
    }
    if out.is_empty() {
        bail!("no usable points in {}", path.display());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Item / narrative vocab
// ---------------------------------------------------------------------------------------------

struct CatInfo {
    brands: &'static [&'static str],
    models: &'static [&'static str],
    noun: &'static str,
    serial_p: f64,
    value: (i64, i64),
}

fn cat_info(c: PropertyType) -> CatInfo {
    match c {
        PropertyType::Laptop => CatInfo {
            brands: &["Norvik", "Kestrel", "Aldane"],
            models: &["K14", "K15 Pro", "Air 13", "Ultra 16"],
            noun: "laptop",
            serial_p: 0.7,
            value: (500, 2200),
        },
        PropertyType::Phone => CatInfo {
            brands: &["Lumo", "Pyxa", "Tessel"],
            models: &["P9", "P10 Max", "Edge 5", "Nova 3"],
            noun: "cell phone",
            serial_p: 0.5,
            value: (200, 1100),
        },
        PropertyType::Tablet => CatInfo {
            brands: &["Slatewave", "Pyxa"],
            models: &["Tab 8", "Tab 11", "Slate Mini"],
            noun: "tablet",
            serial_p: 0.6,
            value: (150, 900),
        },
        PropertyType::Camera => CatInfo {
            brands: &["Optara", "Fenwick"],
            models: &["D50", "Mirror 7", "Pocket Z"],
            noun: "camera",
            serial_p: 0.7,
            value: (250, 2500),
        },
        PropertyType::GameConsole => CatInfo {
            brands: &["Playnova", "Quarrion"],
            models: &["Core 5", "Core 5 Slim", "Portable 2"],
            noun: "game console",
            serial_p: 0.65,
            value: (200, 600),
        },
        PropertyType::Electronics => CatInfo {
            brands: &["Sonique", "Vantel"],
            models: &["SB-40 soundbar", "TV 55in", "Speaker S2", "Monitor 27"],
            noun: "electronics",
            serial_p: 0.55,
            value: (80, 900),
        },
        PropertyType::Firearm => CatInfo {
            brands: &["Brannock"],
            models: &["B9", "Compact 380"],
            noun: "handgun",
            serial_p: 0.85,
            value: (300, 900),
        },
        PropertyType::Tools => CatInfo {
            brands: &["Ridgeback", "Torvane"],
            models: &["18V drill set", "circular saw", "tool chest", "nailer"],
            noun: "power tools",
            serial_p: 0.25,
            value: (100, 1500),
        },
        PropertyType::Bicycle => CatInfo {
            brands: &["Cyclade", "Verdant"],
            models: &["Ridge 29", "Commuter 7", "Gravel X"],
            noun: "bicycle",
            serial_p: 0.4,
            value: (200, 3000),
        },
        PropertyType::Jewelry => CatInfo {
            brands: &[],
            models: &[],
            noun: "jewelry",
            serial_p: 0.0,
            value: (150, 6000),
        },
        PropertyType::Cash => CatInfo { brands: &[], models: &[], noun: "cash", serial_p: 0.0, value: (50, 1500) },
        PropertyType::Other => CatInfo {
            brands: &[],
            models: &[],
            noun: "misc property",
            serial_p: 0.0,
            value: (20, 400),
        },
    }
}

const JEWELRY: [&str; 4] = ["gold chain necklace", "assorted rings and earrings", "silver bracelet set", "wristwatch"];
const OTHER_ITEMS: [&str; 4] = ["backpack with clothing", "designer handbag", "coin collection", "luggage set"];
const COLORS: [&str; 6] = ["black", "silver", "grey", "blue", "red", "white"];
const COMMON_MARKS: [&str; 6] = [
    "dragon sticker on lid",
    "blue duct tape on corner",
    "cracked rear corner",
    "paw print sticker",
    "red paint dot on case",
    "scratched initials on bottom",
];
const DEALERS: [&str; 12] = [
    "Ridgeline Pawn",
    "Copper Anchor Loan",
    "Evergreen Exchange",
    "Fifth and Pine Pawn",
    "Harborview Gold and Loan",
    "Northgate Trade Post",
    "Cascade Coin and Pawn",
    "Blue Heron Resale",
    "Lakeside Pawn Co",
    "Union Bay Buy and Sell",
    "Summit Pawn",
    "Rainier Loan Office",
];
const SIGS: [(&str, &str, &str); 6] = [
    ("ransacked_master_bedroom", "Ransacked MBR.", "MBR ransacked, drawers dumped."),
    ("ignored_electronics", "Electronics left behind, TV and computer untouched.", "Susp ignored electronics (TV left in place)."),
    ("used_pillowcase", "Susp used pillowcase from bed to carry property.", "Pillowcase from bed used as bag."),
    ("disabled_doorbell_cam", "Doorbell cam disabled before entry.", "Doorbell cam unplugged/covered by susp."),
    ("targeted_safe", "Small safe targeted and removed.", "Safe targeted."),
    ("took_vehicle_keys", "Vehicle keys taken but veh not moved.", "Susp took veh keys, veh left."),
];
const ELECTRONIC: [PropertyType; 6] = [
    PropertyType::Laptop,
    PropertyType::Phone,
    PropertyType::Tablet,
    PropertyType::Camera,
    PropertyType::GameConsole,
    PropertyType::Electronics,
];

// ---------------------------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Opt {
    Normal,
    /// Force the first item to be a serialed laptop (hero pawn case).
    Laptop,
    /// No leads at all (hero weak case).
    Weak,
}

struct Gen {
    rng: Rng,
    pts: Vec<Pt>,
    ctr: u32,
}

impl Gen {
    fn geo(&self, i: usize) -> GeoPoint {
        GeoPoint { lat: self.pts[i].lat, lon: self.pts[i].lon }
    }

    fn jitter(&mut self, i: usize) -> GeoPoint {
        let r = self.rng.unit().sqrt() * 60.0;
        let a = self.rng.unit() * std::f64::consts::TAU;
        let p = &self.pts[i];
        GeoPoint {
            lat: p.lat + r * a.cos() / 111_320.0,
            lon: p.lon + r * a.sin() / (111_320.0 * p.lat.to_radians().cos().max(0.2)),
        }
    }

    fn serial(&mut self) -> String {
        const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ0123456789";
        let mut s = String::new();
        for i in 0..8 {
            if i == 3 {
                s.push('-');
            }
            s.push(A[self.rng.below(A.len())] as char);
        }
        s
    }

    fn variant(&mut self, s: &str) -> String {
        match self.rng.below(5) {
            0 => s.to_string(),
            1 => s.replace('-', "").to_lowercase(),
            2 => s.replace('-', ""),
            3 => s.replace('-', " "),
            _ => {
                if s.contains('0') {
                    s.replacen('0', "O", 1)
                } else {
                    s.replace('-', "").to_lowercase()
                }
            }
        }
    }

    // ---- MO ------------------------------------------------------------------------------

    fn random_mo(&mut self, min_props: usize, min_sigs: usize, premise: Option<Premise>, band: Option<TimeBand>) -> Mo {
        let premise = premise.unwrap_or_else(|| {
            self.rng.weighted(&[
                (Premise::Residential, 60),
                (Premise::Commercial, 15),
                (Premise::Garage, 15),
                (Premise::Storage, 10),
            ])
        });
        let entry_point = match premise {
            Premise::Residential => self.rng.weighted(&[
                (EntryPoint::FrontDoor, 25),
                (EntryPoint::RearDoor, 25),
                (EntryPoint::SlidingDoor, 20),
                (EntryPoint::Window, 25),
                (EntryPoint::Other, 3),
                (EntryPoint::Unknown, 4),
            ]),
            Premise::Commercial => self.rng.weighted(&[
                (EntryPoint::FrontDoor, 25),
                (EntryPoint::RearDoor, 25),
                (EntryPoint::Window, 25),
                (EntryPoint::Other, 20),
                (EntryPoint::Unknown, 5),
            ]),
            Premise::Garage => self.rng.weighted(&[
                (EntryPoint::GarageDoor, 50),
                (EntryPoint::RearDoor, 15),
                (EntryPoint::Window, 15),
                (EntryPoint::Other, 15),
                (EntryPoint::Unknown, 5),
            ]),
            Premise::Storage => {
                self.rng.weighted(&[(EntryPoint::Other, 70), (EntryPoint::GarageDoor, 10), (EntryPoint::Unknown, 20)])
            }
        };
        let mut entry_method = self.rng.weighted(&[
            (EntryMethod::Pry, 30),
            (EntryMethod::Smash, 20),
            (EntryMethod::Kick, 15),
            (EntryMethod::Unlocked, 12),
            (EntryMethod::KeyOrCode, 8),
            (EntryMethod::CutScreen, 10),
            (EntryMethod::Unknown, 5),
        ]);
        let screenish = matches!(entry_point, EntryPoint::Window | EntryPoint::SlidingDoor);
        if entry_method == EntryMethod::CutScreen && !screenish {
            entry_method = EntryMethod::Pry;
        }
        if entry_method == EntryMethod::Kick && screenish {
            entry_method = EntryMethod::Smash;
        }
        let time_band = band.unwrap_or_else(|| {
            if premise == Premise::Commercial {
                self.rng.weighted(&[
                    (TimeBand::Night, 50),
                    (TimeBand::Late, 15),
                    (TimeBand::Evening, 10),
                    (TimeBand::Morning, 15),
                    (TimeBand::Midday, 10),
                ])
            } else {
                self.rng.weighted(&[
                    (TimeBand::Midday, 40),
                    (TimeBand::Morning, 20),
                    (TimeBand::Evening, 15),
                    (TimeBand::Late, 12),
                    (TimeBand::Night, 13),
                ])
            }
        });
        let weekday = self.rng.chance(0.75);

        // signatures
        let mut sig_pool: Vec<usize> = if premise == Premise::Residential { (0..SIGS.len()).collect() } else { vec![1, 3, 4] };
        self.rng.shuffle(&mut sig_pool);
        let n_sigs = self.rng.range(min_sigs as i64, 2) as usize;
        sig_pool.truncate(n_sigs);
        let signatures: Vec<String> = sig_pool.iter().map(|&i| SIGS[i].0.to_string()).collect();

        // property types
        let mut pool: Vec<PropertyType> = match premise {
            Premise::Residential => vec![
                PropertyType::Laptop,
                PropertyType::Phone,
                PropertyType::Tablet,
                PropertyType::Camera,
                PropertyType::GameConsole,
                PropertyType::Jewelry,
                PropertyType::Cash,
                PropertyType::Electronics,
                PropertyType::Tools,
            ],
            Premise::Commercial => vec![
                PropertyType::Cash,
                PropertyType::Laptop,
                PropertyType::Electronics,
                PropertyType::Tools,
                PropertyType::Phone,
            ],
            Premise::Garage => {
                vec![PropertyType::Tools, PropertyType::Bicycle, PropertyType::Electronics, PropertyType::Other]
            }
            Premise::Storage => vec![
                PropertyType::Tools,
                PropertyType::Bicycle,
                PropertyType::Electronics,
                PropertyType::Other,
                PropertyType::Firearm,
            ],
        };
        if signatures.iter().any(|s| s == "ignored_electronics") {
            pool.retain(|p| !ELECTRONIC.contains(p));
        }
        self.rng.shuffle(&mut pool);
        let n_props = self.rng.range(min_props.max(1) as i64, 3) as usize;
        pool.truncate(n_props);

        Mo { premise, entry_point, entry_method, time_band, weekday, property_types: pool, signatures }
    }

    // ---- windows ------------------------------------------------------------------------

    /// Offense window inside one local day, starting in `band`. Returns local seconds.
    fn window(&mut self, day: i64, band: TimeBand, premise: Premise, cap_today: bool) -> (i64, i64) {
        let (bs, be) = band_hours(band);
        let start = bs * 60 + self.rng.below(((be - bs) * 60) as usize) as i64;
        let mut start = start / 15 * 15;
        let hours = match (premise, band) {
            (Premise::Residential, TimeBand::Midday | TimeBand::Morning) => self.rng.range(6, 10),
            (_, TimeBand::Night) => self.rng.range(4, 8),
            (Premise::Residential, _) => self.rng.range(2, 5),
            _ => self.rng.range(2, 6),
        };
        let cap = if cap_today { 7 * 60 } else { 24 * 60 };
        start = start.min(cap - 60);
        let end = (start + hours * 60).min(cap);
        (day * DAY + start * 60, day * DAY + end * 60)
    }

    fn report_delay(&mut self) -> i64 {
        if self.rng.chance(0.7) {
            self.rng.range(20, 360) * 60
        } else {
            self.rng.range(6, 72) * 3600
        }
    }

    /// Reported time for a non-queue case (never inside the last 12 hours).
    fn reported_for(&mut self, we: i64) -> i64 {
        (we + self.report_delay()).min(now_local() - 12 * 3600 - 1).max(we)
    }

    // ---- items --------------------------------------------------------------------------

    fn item(&mut self, cat: PropertyType, force_serial: bool) -> StolenItem {
        let info = cat_info(cat);
        let brand = if info.brands.is_empty() { None } else { Some((*self.rng.pick(info.brands)).to_string()) };
        let model = if info.models.is_empty() { None } else { Some((*self.rng.pick(info.models)).to_string()) };
        let serial = if force_serial || self.rng.chance(info.serial_p) { Some(self.serial()) } else { None };
        let value = (self.rng.range(info.value.0, info.value.1) / 5 * 5).max(5) as u32;
        let description = match cat {
            PropertyType::Cash => format!("cash, approx ${value}"),
            PropertyType::Jewelry => (*self.rng.pick(&JEWELRY)).to_string(),
            PropertyType::Other => (*self.rng.pick(&OTHER_ITEMS)).to_string(),
            _ => {
                let color = *self.rng.pick(&COLORS);
                format!("{} {} {}, {}", brand.clone().unwrap_or_default(), model.clone().unwrap_or_default(), info.noun, color)
            }
        };
        let mut markings = vec![];
        if cat != PropertyType::Cash && self.rng.chance(0.15) {
            if self.rng.chance(0.5) {
                self.ctr += 1;
                markings.push(format!("etched ZX-{:04} on back", self.ctr));
            } else {
                markings.push((*self.rng.pick(&COMMON_MARKS)).to_string());
            }
        }
        StolenItem { id: String::new(), category: cat, brand, model, serial, description, markings, value_usd: value }
    }

    fn make_items(&mut self, mo: &Mo, opt: Opt) -> Vec<StolenItem> {
        if opt == Opt::Weak {
            return vec![self.item(PropertyType::Cash, false)];
        }
        let mut items = vec![];
        if opt == Opt::Laptop {
            items.push(self.item(PropertyType::Laptop, true));
        }
        for &c in &mo.property_types {
            if !items.iter().any(|i: &StolenItem| i.category == c) {
                items.push(self.item(c, false));
            }
        }
        let want = self.rng.weighted(&[(1usize, 35), (2, 35), (3, 20), (4, 10)]).min(4).max(items.len());
        while items.len() < want && !mo.property_types.is_empty() {
            let c = *self.rng.pick(&mo.property_types);
            items.push(self.item(c, false));
        }
        items
    }

    // ---- narrative ----------------------------------------------------------------------

    fn narrative(&mut self, mo: &Mo, items: &[StolenItem], f: &SolvabilityFacts, ws: i64, we: i64) -> String {
        let (noun, abbr) = match mo.premise {
            Premise::Residential => ("residence", "RESD"),
            Premise::Commercial => ("business", "BUS"),
            Premise::Garage => ("detached garage", "DET GAR"),
            Premise::Storage => ("storage unit", "STOR UNIT"),
        };
        let same_day = ws.div_euclid(DAY) == (we - 1).div_euclid(DAY);
        let s1 = if same_day {
            let (date, t1, t2) = (date_str(ws), hhmm(ws), hhmm_end(we));
            match self.rng.below(3) {
                0 => format!("{abbr} burglary rpt'd. RP out of loc {date} between {t1}-{t2}, UNK SUSP made entry."),
                1 => format!("RP returned {t2} {date} to find {noun} burglarized. Last secure {t1}."),
                _ => format!("{abbr} B&E, offense window {t1}-{t2} on {date}."),
            }
        } else {
            let a = format!("{} {}", date_str(ws), hhmm(ws));
            let b = format!("{} {}", date_str(we), hhmm_end(we));
            if self.rng.chance(0.5) {
                format!("{abbr} burglary rpt'd after RP absence. Offense window {a} to {b}.")
            } else {
                format!("RP returned from travel to find {noun} burglarized, window {a} - {b}.")
            }
        };
        let ep = match mo.entry_point {
            EntryPoint::FrontDoor => *self.rng.pick(&["front door (FD)", "FD"]),
            EntryPoint::RearDoor => *self.rng.pick(&["rear door", "rear door (RD)"]),
            EntryPoint::SlidingDoor => *self.rng.pick(&["rear slider", "sliding door"]),
            EntryPoint::Window => *self.rng.pick(&["window", "window (rear)"]),
            EntryPoint::GarageDoor => "garage door",
            EntryPoint::Other => "other opening (roof/vent)",
            EntryPoint::Unknown => "UNK",
        };
        let method = match mo.entry_method {
            EntryMethod::Pry => "pried (pry marks on frame)",
            EntryMethod::Smash => "glass smashed",
            EntryMethod::Kick => "kicked in",
            EntryMethod::Unlocked => "unlocked, no force",
            EntryMethod::CutScreen => "screen cut",
            EntryMethod::KeyOrCode => "key/code used, no force",
            EntryMethod::Unknown => "method undetermined",
        };
        let s2 = if mo.entry_point == EntryPoint::Unknown {
            format!("POE UNK, {method}.")
        } else if self.rng.chance(0.5) {
            format!("POE {ep}, {method}.")
        } else {
            format!("UNK SUSP(s) entered via {ep}; {method}.")
        };
        let mut parts = vec![];
        for it in items {
            let mut p = it.description.clone();
            if let (Some(s), true) = (&it.serial, self.rng.chance(0.6)) {
                p.push_str(&format!(" (S/N {s})"));
            }
            if let Some(m) = it.markings.first() {
                p.push_str(&format!(" marked \"{m}\""));
            }
            parts.push(p);
        }
        let lead = *self.rng.pick(&["Taken:", "Missing:", "Items taken:", "Loss:"]);
        let total: u32 = items.iter().map(|i| i.value_usd).sum();
        let s3 = format!("{lead} {}. Est. loss ${total}.", parts.join("; "));
        let mut out = vec![s1, s2, s3];
        for s in &mo.signatures {
            if let Some((_, a, b)) = SIGS.iter().find(|x| x.0 == s) {
                out.push(if self.rng.chance(0.5) { (*a).to_string() } else { (*b).to_string() });
            }
        }
        if mo.signatures.len() < 2 {
            let mut bits = vec![];
            if f.suspect_description {
                bits.push("Witness saw M/UNK subj in dark hoodie leaving on foot");
            } else if f.witness {
                bits.push("Neighbor heard glass break, saw no susp");
            }
            if f.vehicle_description {
                bits.push("veh described as older dk sedan, plate unk");
            }
            if f.video_available {
                bits.push("video available from nearby cam");
            }
            if f.physical_evidence {
                bits.push("latents/footwear collected by CSI");
            }
            out.push(if bits.is_empty() {
                "No wit, no video, no susp info.".to_string()
            } else {
                format!("{}.", bits.join("; "))
            });
        }
        out.join(" ")
    }

    // ---- cases --------------------------------------------------------------------------

    /// Build a case from local-second times. `id` and item ids are filled in at finalisation.
    fn build_case(&mut self, pi: usize, mo: &Mo, ws: i64, we: i64, reported: i64, opt: Opt) -> Case {
        let location = self.jitter(pi);
        let items = self.make_items(mo, opt);
        let weak = opt == Opt::Weak;
        let residential = mo.premise == Premise::Residential;
        let witness = !weak && self.rng.chance(if residential { 0.12 } else { 0.2 });
        let suspect_description = witness && self.rng.chance(0.6);
        let vehicle_description = !weak && self.rng.chance(0.07);
        let cam_off = mo.signatures.iter().any(|s| s == "disabled_doorbell_cam");
        let video_available =
            !weak && !cam_off && self.rng.chance(if mo.premise == Premise::Commercial { 0.5 } else { 0.18 });
        let physical_evidence = !weak && self.rng.chance(0.15);
        let traceable_property = items.iter().any(|i| i.serial.is_some() || !i.markings.is_empty());
        let facts = SolvabilityFacts {
            witness,
            suspect_description,
            vehicle_description,
            video_available,
            physical_evidence,
            traceable_property,
            window_hours: (we - ws) as f64 / 3600.0,
        };
        let narrative = self.narrative(mo, &items, &facts, ws, we);
        let p = &self.pts[pi];
        Case {
            id: String::new(),
            reported_at: reported + TZ,
            window_start: ws + TZ,
            window_end: we + TZ,
            location,
            block: p.block.clone(),
            beat: p.beat.clone(),
            neighborhood: p.neigh.clone(),
            narrative,
            mo: Some(mo.clone()),
            items,
            facts,
            status: CaseStatus::Open,
        }
    }

    fn pick_points(&mut self, size: usize) -> Vec<usize> {
        let home = self.rng.below(self.pts.len());
        let radius = 0.5 + self.rng.unit() * 2.5;
        let hp = self.geo(home);
        let mut d: Vec<(f64, usize)> = (0..self.pts.len()).map(|i| (haversine_km(hp, self.geo(i)), i)).collect();
        let mut within: Vec<usize> = d.iter().filter(|x| x.0 <= radius).map(|x| x.1).collect();
        if within.len() < size {
            d.sort_by(|a, b| a.0.total_cmp(&b.0));
            within = d.iter().take(size).map(|x| x.1).collect();
        }
        self.rng.shuffle(&mut within);
        within.truncate(size);
        within
    }

    /// One synthetic offender's series. Cases are returned in chronological order.
    /// `hero`: weekday night series whose last case happened in the final hours before "now".
    fn plant(&mut self, size: usize, hero: bool) -> (Vec<Case>, Mo) {
        let (premise, band) = if hero { (Some(Premise::Residential), Some(TimeBand::Night)) } else { (None, None) };
        let mut mo = self.random_mo(2, 1, premise, band);
        mo.weekday = hero || mo.weekday;
        let weekday = mo.weekday;
        let first = today() - 364;
        let last = today() - 2;
        let mut days: Vec<i64>;
        if hero {
            days = (today() - 49..=last).filter(|&d| is_weekday(d)).collect();
            self.rng.shuffle(&mut days);
            days.truncate(size.saturating_sub(1));
            days.sort_unstable();
            days.push(today());
        } else {
            let mut period = self.rng.range(3, 10) * 7;
            loop {
                let hi = (last - period).max(first);
                let s = first + self.rng.below((hi - first + 1) as usize) as i64;
                let mut c: Vec<i64> = (s..(s + period).min(last + 1)).filter(|&d| is_weekday(d) == weekday).collect();
                if c.len() >= size || period > 364 {
                    self.rng.shuffle(&mut c);
                    c.truncate(size);
                    c.sort_unstable();
                    days = c;
                    break;
                }
                period += 7;
            }
        }
        let pts = self.pick_points(days.len());
        let mut cases = vec![];
        for (k, &day) in days.iter().enumerate() {
            let is_today = day == today();
            let (ws, we) = self.window(day, mo.time_band, mo.premise, is_today);
            let reported = if is_today { we + self.rng.range(20, 60) * 60 } else { self.reported_for(we) };
            let pi = pts.get(k).copied().unwrap_or(0);
            cases.push(self.build_case(pi, &mo, ws, we, reported, Opt::Normal));
        }
        (cases, mo)
    }

    fn single_case(&mut self) -> Case {
        let pi = self.rng.below(self.pts.len());
        let mut mo = self.random_mo(1, 0, None, None);
        let day = self.rng.range(today() - 364, today() - 2);
        mo.weekday = is_weekday(day);
        let (ws, we) = self.window(day, mo.time_band, mo.premise, false);
        let reported = self.reported_for(we);
        self.build_case(pi, &mo, ws, we, reported, Opt::Normal)
    }

    fn queue_case(&mut self, opt: Opt) -> Case {
        let pi = self.rng.below(self.pts.len());
        let now = now_local();
        let (ws, we, reported);
        let mut mo;
        match opt {
            Opt::Laptop => {
                mo = self.random_mo(1, 0, Some(Premise::Residential), None);
                mo.property_types = vec![PropertyType::Laptop, PropertyType::Jewelry];
                mo.signatures.retain(|s| s != "ignored_electronics");
                we = now - 5 * DAY - 12 * 3600;
                ws = we - 66 * 3600;
                reported = now - 2 * 3600;
                mo.time_band = TimeBand::Unknown;
            }
            Opt::Weak => {
                mo = self.random_mo(1, 0, Some(Premise::Residential), None);
                mo.property_types = vec![PropertyType::Cash];
                mo.signatures.clear();
                reported = now - 3 * 3600;
                we = reported - 3600;
                ws = we - 9 * 3600;
                mo.time_band = band_of_hour(ws.rem_euclid(DAY) / 3600);
            }
            Opt::Normal => {
                mo = self.random_mo(1, 0, None, None);
                reported = now - self.rng.range(15, 12 * 60 - 1) * 60;
                we = reported - self.rng.range(10, 180) * 60;
                let hours = if mo.premise == Premise::Residential { self.rng.range(2, 6) } else { self.rng.range(1, 4) };
                ws = we - hours * 3600;
                mo.time_band = band_of_hour(ws.rem_euclid(DAY) / 3600);
            }
        }
        mo.weekday = is_weekday(ws.div_euclid(DAY));
        self.build_case(pi, &mo, ws, we, reported, opt)
    }

    /// A case with the same MO whose window overlaps `anchor` but sits >= 12 km away.
    /// Two cases with the same MO and the SAME 30-minute window (e.g. alarm-confirmed) at least
    /// 15.5 km apart. One offender needs > 15.5/40 h + 10 min = 33 min, so even the best-case
    /// timing is infeasible. Long windows would not work: within a 6-10 h window one offender
    /// can legitimately do both jobs.
    fn infeasible_pair(&mut self, anchor_ws_local: i64, mo: &Mo) -> (Case, Case) {
        const MIN_KM: f64 = 15.5;
        let ai = self.rng.below(self.pts.len());
        let a = self.geo(ai);
        let mut far: Vec<usize> = (0..self.pts.len()).filter(|&i| haversine_km(a, self.geo(i)) >= MIN_KM).collect();
        if far.is_empty() {
            let mut best = (0.0, 0usize);
            for i in 0..self.pts.len() {
                let d = haversine_km(a, self.geo(i));
                if d > best.0 {
                    best = (d, i);
                }
            }
            far.push(best.1);
        }
        let fi = *self.rng.pick(&far);
        let (ws, we) = (anchor_ws_local, anchor_ws_local + 30 * 60);
        let rep_a = self.reported_for(we);
        let first = self.build_case(ai, mo, ws, we, rep_a, Opt::Normal);
        let rep_b = self.reported_for(we);
        let second = self.build_case(fi, mo, ws, we, rep_b, Opt::Normal);
        (first, second)
    }

    // ---- pawn ---------------------------------------------------------------------------

    fn pawn_rec(&mut self, it: &StolenItem, serial: Option<String>, received: Ts, seller: String) -> PawnRecord {
        let pi = self.rng.below(self.pts.len());
        let loc = self.jitter(pi);
        let desc = match (&it.brand, &it.model) {
            (Some(b), Some(m)) => format!("{b} {m} {}", cat_info(it.category).noun),
            _ => it.description.clone(),
        };
        PawnRecord {
            id: String::new(),
            dealer: (*self.rng.pick(&DEALERS)).to_string(),
            received_at: received,
            location: loc,
            category: it.category,
            brand: it.brand.clone(),
            model: it.model.clone(),
            serial,
            description: desc,
            markings: it.markings.clone(),
            seller_ref: seller,
        }
    }

    fn seller(&mut self) -> String {
        format!("SLR-{:08X}", self.rng.next() as u32)
    }

    fn gen_pawn(&mut self, cases: &[Case], sellers: &HashMap<String, String>, queue: &HashSet<String>, hero_pawn: &str) -> Vec<PawnDraft> {
        let mut out: Vec<PawnDraft> = vec![];
        let now = now_ts();
        let case_serials: HashSet<String> =
            cases.iter().flat_map(|c| c.items.iter().filter_map(|i| i.serial.clone())).collect();
        let mut used: HashSet<usize> = HashSet::new();

        // Hero: laptop resurfaced 3 days after the offense window, plus a same-model different-serial trap.
        if let Some((ci, c)) = cases.iter().enumerate().find(|(_, c)| c.id == hero_pawn) {
            if let Some(it) = c.items.first() {
                let sv = it.serial.clone().map(|s| self.variant(&s));
                let seller = self.seller();
                let rec = self.pawn_rec(it, sv, c.window_end + 3 * DAY, seller);
                out.push(PawnDraft { rec, truth: Some((c.id.clone(), it.id.clone())), tag: 1 });
                let other = self.other_serial(it.serial.as_deref());
                let seller = self.seller();
                let rec = self.pawn_rec(it, Some(other), c.window_end + 2 * DAY, seller);
                out.push(PawnDraft { rec, truth: None, tag: 2 });
            }
            used.insert(ci);
        }

        // Resurfacing: ~10% of traceable cases.
        for (ci, c) in cases.iter().enumerate() {
            if used.contains(&ci) || queue.contains(&c.id) || !c.facts.traceable_property || !self.rng.chance(0.10) {
                continue;
            }
            let it = match c.items.iter().find(|i| i.serial.is_some()).or_else(|| c.items.iter().find(|i| !i.markings.is_empty())) {
                Some(i) => i,
                None => continue,
            };
            let received = c.window_end + self.rng.range(2, 30) * DAY + self.rng.range(0, 12) * 3600;
            if received > now {
                continue;
            }
            let sv = it.serial.clone().map(|s| self.variant(&s));
            let seller = match sellers.get(&c.id) {
                Some(s) if self.rng.chance(0.7) => s.clone(),
                _ => self.seller(),
            };
            let rec = self.pawn_rec(it, sv, received, seller);
            out.push(PawnDraft { rec, truth: Some((c.id.clone(), it.id.clone())), tag: 0 });
            used.insert(ci);
        }

        // Traps.
        let mut serialed: Vec<usize> = (0..cases.len())
            .filter(|i| !used.contains(i) && !queue.contains(&cases[*i].id) && cases[*i].items.iter().any(|x| x.serial.is_some()))
            .collect();
        self.rng.shuffle(&mut serialed);
        let mut marked: Vec<usize> = (0..cases.len())
            .filter(|i| !used.contains(i) && !queue.contains(&cases[*i].id) && cases[*i].items.iter().any(|x| !x.markings.is_empty()))
            .collect();
        self.rng.shuffle(&mut marked);
        // kind: 0 same model different serial, 1 same model no serial, 2 pawned before the burglary
        for (n, kind) in [0u8, 0, 1, 1, 2].into_iter().enumerate() {
            let Some(&ci) = serialed.get(n) else { continue };
            let c = &cases[ci];
            let Some(it) = c.items.iter().find(|x| x.serial.is_some()) else { continue };
            let (serial, received) = match kind {
                0 => (Some(self.other_serial(it.serial.as_deref())), c.window_end + self.rng.range(3, 20) * DAY),
                1 => (None, c.window_end + self.rng.range(3, 20) * DAY),
                _ => (it.serial.clone(), c.window_start - self.rng.range(3, 20) * DAY),
            };
            let seller = self.seller();
            let rec = self.pawn_rec(it, serial, received.min(now), seller);
            out.push(PawnDraft { rec, truth: None, tag: 0 });
        }
        // shared marking on an unrelated item
        for &ci in marked.iter().take(2) {
            let c = &cases[ci];
            let Some(it) = c.items.iter().find(|x| !x.markings.is_empty()) else { continue };
            let mut other = it.clone();
            other.category = PropertyType::Other;
            other.brand = None;
            other.model = None;
            other.description = "misc property".to_string();
            let received = (c.window_end + self.rng.range(3, 25) * DAY).min(now);
            let seller = self.seller();
            let rec = self.pawn_rec(&other, None, received, seller);
            out.push(PawnDraft { rec, truth: None, tag: 0 });
        }

        // Noise listings.
        let cats = [
            PropertyType::Laptop,
            PropertyType::Phone,
            PropertyType::Tablet,
            PropertyType::Camera,
            PropertyType::GameConsole,
            PropertyType::Electronics,
            PropertyType::Tools,
            PropertyType::Bicycle,
            PropertyType::Jewelry,
            PropertyType::Firearm,
        ];
        let remaining = PAWN_TARGET.saturating_sub(out.len());
        for _ in 0..remaining {
            let cat = *self.rng.pick(&cats);
            let mut it = self.item(cat, false);
            it.markings.retain(|m| COMMON_MARKS.contains(&m.as_str()));
            let mut serial = it.serial.take();
            while serial.as_ref().is_some_and(|s| case_serials.contains(s)) {
                serial = Some(self.serial());
            }
            let received = now - self.rng.range(0, 365 * DAY - 1);
            let seller = self.seller();
            let rec = self.pawn_rec(&it, serial, received, seller);
            out.push(PawnDraft { rec, truth: None, tag: 0 });
        }
        out
    }

    fn other_serial(&mut self, not: Option<&str>) -> String {
        loop {
            let s = self.serial();
            if Some(s.as_str()) != not {
                return s;
            }
        }
    }
}

struct PawnDraft {
    rec: PawnRecord,
    truth: Option<(String, String)>,
    /// 1 = hero truth record, 2 = hero same-model different-serial trap.
    tag: u8,
}

// ---------------------------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------------------------

pub struct Generated {
    pub ds: Dataset,
    pub truth: GroundTruth,
    pub demo: serde_json::Value,
}

pub fn generate(pts: &[Pt], seed: u64, n_cases: usize) -> anyhow::Result<Generated> {
    if pts.is_empty() {
        bail!("no geography points");
    }
    if n_cases < MIN_CASES {
        bail!("--cases must be at least {MIN_CASES}");
    }
    let mut g = Gen { rng: Rng(seed), pts: pts.to_vec(), ctr: 0 };
    let mut all: Vec<Case> = vec![];
    let mut series: Vec<Vec<usize>> = vec![];
    let mut queue: Vec<usize> = vec![];

    // Planted series.
    for s in 0..N_SERIES {
        let size = if s == 0 { 7 } else { (2 + (10.0 * g.rng.unit().powi(2)) as usize).clamp(2, 12) };
        let (cs, _) = g.plant(size, s == 0);
        let start = all.len();
        all.extend(cs);
        let idxs: Vec<usize> = (start..all.len()).collect();
        if let (0, Some(&l)) = (s, idxs.last()) {
            queue.push(l);
        }
        series.push(idxs);
    }

    // Decoys: 5 spatiotemporal, 5 conflicting clearance.
    let mut decoy_groups: Vec<(Vec<usize>, &'static str)> = vec![];
    for d in 0..N_DECOYS {
        let size = g.rng.range(3, 5) as usize;
        let spatial = d < N_DECOYS / 2;
        let (mut cs, mo) = g.plant(if spatial { size - 1 } else { size }, false);
        if spatial {
            let ws = cs.first().map_or(0, |a| a.window_start - TZ);
            let (first, far) = g.infeasible_pair(ws, &mo);
            if let Some(slot) = cs.first_mut() {
                *slot = first;
            }
            cs.push(far);
        }
        let start = all.len();
        all.extend(cs);
        let idxs: Vec<usize> = (start..all.len()).collect();
        decoy_groups.push((idxs, if spatial { "spatiotemporal_infeasible" } else { "conflicting_clearance" }));
    }

    // Overnight queue singletons: hero pawn, hero weak, then the rest.
    let hero_pawn_i = all.len();
    let c = g.queue_case(Opt::Laptop);
    all.push(c);
    let hero_weak_i = all.len();
    let c = g.queue_case(Opt::Weak);
    all.push(c);
    queue.extend([hero_pawn_i, hero_weak_i]);
    while queue.len() < N_QUEUE {
        let c = g.queue_case(Opt::Normal);
        queue.push(all.len());
        all.push(c);
    }

    // Non-queue singletons.
    if all.len() > n_cases {
        bail!("--cases {n_cases} is too small for the planted structure ({} needed)", all.len());
    }
    let mut singles: Vec<usize> = vec![];
    while all.len() < n_cases {
        let c = g.single_case();
        singles.push(all.len());
        all.push(c);
    }

    // Clearance.
    let mut arr = 0u32;
    let mut next_ref = || {
        arr += 1;
        format!("ARR-{arr:05}")
    };
    let mut order: Vec<usize> = (1..N_SERIES).collect();
    g.rng.shuffle(&mut order);
    let mut cleared_series: Vec<usize> = vec![0];
    cleared_series.extend(order.into_iter().take((N_SERIES as f64 * 0.3).round() as usize - 1));
    let mut cleared = 0usize;
    for &s in &cleared_series {
        let idxs = &series[s];
        let pool = if s == 0 { idxs.len().saturating_sub(1) } else { idxs.len() };
        let ci = idxs[if s == 0 { 2.min(pool.saturating_sub(1)) } else { g.rng.below(pool) }];
        all[ci].status = CaseStatus::ClearedByArrest { arrestee_ref: next_ref() };
        cleared += 1;
    }
    for (idxs, kind) in &decoy_groups {
        if *kind == "conflicting_clearance" {
            for &ci in [idxs[0], idxs[idxs.len() - 1]].iter() {
                all[ci].status = CaseStatus::ClearedByArrest { arrestee_ref: next_ref() };
                cleared += 1;
            }
        }
    }
    let target = (n_cases as f64 * CLEARANCE_RATE).round() as usize;
    g.rng.shuffle(&mut singles);
    for &ci in singles.iter().take(target.saturating_sub(cleared)) {
        all[ci].status = CaseStatus::ClearedByArrest { arrestee_ref: next_ref() };
    }
    let left = singles.get(target.saturating_sub(cleared)..).unwrap_or(&[]).to_vec();
    for ci in left {
        if g.rng.chance(0.06) {
            all[ci].status = CaseStatus::Suspended;
        }
    }

    // Finalise: sort by report time, assign ids.
    let n = all.len();
    let mut ord: Vec<usize> = (0..n).collect();
    ord.sort_by_key(|&i| (all[i].reported_at, i));
    let mut pos = vec![0usize; n];
    for (p, &i) in ord.iter().enumerate() {
        pos[i] = p;
    }
    let mut cases: Vec<Case> = ord.iter().map(|&i| all[i].clone()).collect();
    for (p, c) in cases.iter_mut().enumerate() {
        let (y, _, _) = civil_from_days((c.reported_at - TZ).div_euclid(DAY));
        c.id = format!("CC-{:02}-{:06}", y % 100, p + 1);
        for (k, it) in c.items.iter_mut().enumerate() {
            it.id = format!("{}-I{}", c.id, k + 1);
        }
    }
    let id_of = |i: usize| cases[pos[i]].id.clone();
    let ids = |v: &[usize]| -> Vec<String> { v.iter().map(|&i| id_of(i)).collect() };

    let mut truth = GroundTruth {
        series: series.iter().map(|s| ids(s)).collect(),
        decoys: decoy_groups.iter().map(|(v, k)| Decoy { case_ids: ids(v), violates: (*k).to_string() }).collect(),
        ..GroundTruth::default()
    };
    for c in &cases {
        if let Some(mo) = &c.mo {
            truth.mo_truth.insert(c.id.clone(), mo.clone());
        }
    }
    let queue_ids: Vec<String> = {
        let mut q = ids(&queue);
        q.sort();
        q
    };
    let queue_set: HashSet<String> = queue_ids.iter().cloned().collect();
    let hero_pawn = id_of(hero_pawn_i);
    let hero_weak = id_of(hero_weak_i);
    let mut sellers: HashMap<String, String> = HashMap::new();
    for s in &truth.series {
        let r = g.seller();
        for id in s {
            sellers.insert(id.clone(), r.clone());
        }
    }

    // Pawn ledger.
    let mut drafts = g.gen_pawn(&cases, &sellers, &queue_set, &hero_pawn);
    drafts.sort_by_key(|d| d.rec.received_at);
    let mut pawn = vec![];
    let (mut hero_pawn_id, mut hero_trap_id) = (String::new(), String::new());
    for (k, d) in drafts.into_iter().enumerate() {
        let mut rec = d.rec;
        let (y, _, _) = civil_from_days((rec.received_at - TZ).div_euclid(DAY));
        rec.id = format!("PW-{:02}-{:06}", y % 100, k + 1);
        if let Some(t) = d.truth {
            truth.pawn_truth.insert(rec.id.clone(), t);
        }
        match d.tag {
            1 => hero_pawn_id = rec.id.clone(),
            2 => hero_trap_id = rec.id.clone(),
            _ => {}
        }
        pawn.push(rec);
    }

    // Demo picks (read before clearing queue MOs).
    let hero_series_ids = truth.series.first().cloned().unwrap_or_default();
    let find = |id: &str| cases.iter().find(|c| c.id == id);
    let cleared_member = hero_series_ids.iter().find_map(|id| match find(id).map(|c| &c.status) {
        Some(CaseStatus::ClearedByArrest { arrestee_ref }) => Some((id.clone(), arrestee_ref.clone())),
        _ => None,
    });
    let hero_queue_case = hero_series_ids.iter().find(|id| queue_set.contains(*id)).cloned();
    let hero_decoy = truth.decoys.iter().find(|d| d.violates == "spatiotemporal_infeasible").cloned();
    let hero_item = find(&hero_pawn).and_then(|c| c.items.first().map(|i| i.id.clone()));
    let demo = json!({
        "queue": queue_ids,
        "hero_series": {
            "case_ids": hero_series_ids,
            "queue_case": hero_queue_case,
            "cleared_case": cleared_member.as_ref().map(|x| x.0.clone()),
            "arrestee_ref": cleared_member.as_ref().map(|x| x.1.clone()),
        },
        "hero_pawn": {
            "case_id": hero_pawn,
            "item_id": hero_item,
            "pawn_id": hero_pawn_id,
            "trap_pawn_id": hero_trap_id,
            "note": "offense window ended ~5.5 days before the report; laptop pawned 3 days after the window",
        },
        "hero_decoy": hero_decoy.as_ref().map(|d| json!({
            "case_ids": d.case_ids,
            "violates": d.violates,
            "offending_case": d.case_ids.last(),
        })),
        "hero_weak": { "case_id": hero_weak },
    });

    // Overnight queue cases await extraction.
    for c in cases.iter_mut() {
        if queue_set.contains(&c.id) {
            c.mo = None;
        }
    }
    Ok(Generated { ds: Dataset { cases, pawn }, truth, demo })
}

pub fn write_all(dir: &Path, g: &Generated) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(dir.join("dataset.json"), serde_json::to_vec(&g.ds)?)?;
    std::fs::write(dir.join("ground_truth.json"), serde_json::to_vec(&g.truth)?)?;
    std::fs::write(dir.join("demo.json"), serde_json::to_vec_pretty(&g.demo)?)?;
    Ok(())
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let seed: u64 = match crate::flag(args, "--seed") {
        Some(s) => s.parse().context("--seed must be an integer")?,
        None => 7,
    };
    let n: usize = match crate::flag(args, "--cases") {
        Some(s) => s.parse().context("--cases must be an integer")?,
        None => 2000,
    };
    let out = crate::flag(args, "--out").unwrap_or_else(|| "data/seed".to_string());
    let root = crate::root();
    let pts = load_points(&root.join("data/public/seattle_burglary_points.csv"))?;
    let g = generate(&pts, seed, n)?;
    let dir = root.join(out);
    write_all(&dir, &g)?;
    let cleared = g.ds.cases.iter().filter(|c| matches!(c.status, CaseStatus::ClearedByArrest { .. })).count();
    println!(
        "datagen: {} cases ({} cleared), {} pawn records, {} series, {} decoys, {} queue -> {}",
        g.ds.cases.len(),
        cleared,
        g.ds.pawn.len(),
        g.truth.series.len(),
        g.truth.decoys.len(),
        g.demo["queue"].as_array().map_or(0, Vec::len),
        dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use casefile_core::verify::{MAX_URBAN_SPEED_KMH, MIN_ON_SCENE_MIN};

    fn points() -> Vec<Pt> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/public/seattle_burglary_points.csv");
        load_points(&p).unwrap()
    }

    fn mk(seed: u64) -> Generated {
        generate(&points(), seed, 2000).unwrap()
    }

    fn by_id<'a>(ds: &'a Dataset, id: &str) -> &'a Case {
        ds.cases.iter().find(|c| c.id == id).unwrap()
    }

    fn feasible(a: &Case, b: &Case) -> bool {
        let gap = if a.window_end < b.window_start {
            b.window_start - a.window_end
        } else if b.window_end < a.window_start {
            a.window_start - b.window_end
        } else {
            0
        };
        let need = haversine_km(a.location, b.location) / MAX_URBAN_SPEED_KMH * 60.0 + MIN_ON_SCENE_MIN;
        gap as f64 / 60.0 >= need
    }

    #[test]
    fn deterministic() {
        let (a, b, c) = (mk(7), mk(7), mk(8));
        let bytes = |g: &Generated| {
            (
                serde_json::to_vec(&g.ds).unwrap(),
                serde_json::to_vec(&g.truth).unwrap(),
                serde_json::to_vec(&g.demo).unwrap(),
            )
        };
        assert_eq!(bytes(&a), bytes(&b));
        assert_ne!(bytes(&a).0, bytes(&c).0);
    }

    #[test]
    fn counts_and_clearance() {
        let g = mk(7);
        assert_eq!(g.ds.cases.len(), 2000);
        assert_eq!(g.truth.series.len(), 40);
        assert_eq!(g.truth.decoys.len(), 10);
        assert!((1400..=1600).contains(&g.ds.pawn.len()), "pawn {}", g.ds.pawn.len());
        assert!(g.truth.series.iter().all(|s| (2..=12).contains(&s.len())));
        let cleared = g.ds.cases.iter().filter(|c| matches!(c.status, CaseStatus::ClearedByArrest { .. })).count();
        let pct = cleared as f64 / 20.0;
        assert!((12.0..=14.0).contains(&pct), "clearance {pct}%");
        let ids: HashSet<_> = g.ds.cases.iter().map(|c| &c.id).collect();
        assert_eq!(ids.len(), 2000);
        assert!(g.truth.pawn_truth.len() >= 10);
        for c in &g.ds.cases {
            assert!((1..=4).contains(&c.items.len()));
            assert!(c.narrative.len() > 60);
        }
    }

    #[test]
    fn planted_series_are_feasible() {
        let g = mk(7);
        for s in &g.truth.series {
            let cs: Vec<&Case> = s.iter().map(|id| by_id(&g.ds, id)).collect();
            for i in 0..cs.len() {
                for j in i + 1..cs.len() {
                    assert!(feasible(cs[i], cs[j]), "{} vs {}", cs[i].id, cs[j].id);
                    assert!((cs[i].window_start - cs[j].window_start).abs() >= 12 * 3600);
                }
            }
            let arrestees: HashSet<_> = cs
                .iter()
                .filter_map(|c| match &c.status {
                    CaseStatus::ClearedByArrest { arrestee_ref } => Some(arrestee_ref.clone()),
                    _ => None,
                })
                .collect();
            assert!(arrestees.len() <= 1);
        }
    }

    #[test]
    fn decoys_violate_their_constraint() {
        let g = mk(7);
        for d in &g.truth.decoys {
            assert!((3..=5).contains(&d.case_ids.len()));
            let cs: Vec<&Case> = d.case_ids.iter().map(|id| by_id(&g.ds, id)).collect();
            match d.violates.as_str() {
                "spatiotemporal_infeasible" => {
                    // Best-case (largest separation the windows allow) must still be too short.
                    let bad = cs.iter().enumerate().any(|(i, a)| {
                        cs[i + 1..].iter().any(|b| {
                            let sep = (b.window_end - a.window_start).max(a.window_end - b.window_start) as f64 / 60.0;
                            let need = haversine_km(a.location, b.location) / MAX_URBAN_SPEED_KMH * 60.0
                                + MIN_ON_SCENE_MIN;
                            haversine_km(a.location, b.location) >= 12.0 && sep < need
                        })
                    });
                    assert!(bad, "{:?}", d.case_ids);
                }
                "conflicting_clearance" => {
                    let refs: HashSet<_> = cs
                        .iter()
                        .filter_map(|c| match &c.status {
                            CaseStatus::ClearedByArrest { arrestee_ref } => Some(arrestee_ref.clone()),
                            _ => None,
                        })
                        .collect();
                    assert!(refs.len() >= 2, "{:?}", d.case_ids);
                }
                other => panic!("unknown violation {other}"),
            }
        }
    }

    #[test]
    fn queue_and_heroes() {
        let g = mk(7);
        let q: Vec<String> = serde_json::from_value(g.demo["queue"].clone()).unwrap();
        assert_eq!(q.len(), 36);
        let qs: HashSet<_> = q.iter().cloned().collect();
        let now = now_ts();
        for c in &g.ds.cases {
            if qs.contains(&c.id) {
                assert!(c.mo.is_none(), "{} has mo", c.id);
                assert!(c.reported_at > now - 12 * 3600 && c.reported_at <= now);
            } else {
                assert!(c.mo.is_some());
                assert!(c.reported_at <= now - 12 * 3600);
            }
        }
        // hero series: 7 cases, one queue case, one cleared case.
        let hs: Vec<String> = serde_json::from_value(g.demo["hero_series"]["case_ids"].clone()).unwrap();
        assert_eq!(hs.len(), 7);
        assert!(hs.iter().any(|i| qs.contains(i)));
        let cleared = g.demo["hero_series"]["cleared_case"].as_str().unwrap();
        assert!(matches!(by_id(&g.ds, cleared).status, CaseStatus::ClearedByArrest { .. }));
        // hero pawn: laptop serial resurfaced 3 days after the window, with a same-model trap.
        let hp = g.demo["hero_pawn"]["case_id"].as_str().unwrap();
        assert!(qs.contains(hp));
        let case = by_id(&g.ds, hp);
        let item = &case.items[0];
        assert_eq!(item.category, PropertyType::Laptop);
        let pawn = |k: &str| g.ds.pawn.iter().find(|p| p.id == g.demo["hero_pawn"][k].as_str().unwrap()).unwrap();
        let (hit, trap) = (pawn("pawn_id"), pawn("trap_pawn_id"));
        assert_eq!(hit.received_at, case.window_end + 3 * DAY);
        let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase().replace('O', "0");
        assert_eq!(norm(hit.serial.as_deref().unwrap()), norm(item.serial.as_deref().unwrap()));
        assert_eq!((&trap.brand, &trap.model), (&item.brand, &item.model));
        assert_ne!(norm(trap.serial.as_deref().unwrap()), norm(item.serial.as_deref().unwrap()));
        assert_eq!(g.truth.pawn_truth.get(&hit.id), Some(&(hp.to_string(), item.id.clone())));
        assert!(!g.truth.pawn_truth.contains_key(&trap.id));
        // hero decoy and weak.
        let hd: Vec<String> = serde_json::from_value(g.demo["hero_decoy"]["case_ids"].clone()).unwrap();
        assert!(g.truth.decoys.iter().any(|d| d.case_ids == hd));
        let weak = by_id(&g.ds, g.demo["hero_weak"]["case_id"].as_str().unwrap());
        assert!(qs.contains(&weak.id));
        assert!(!weak.facts.traceable_property && !weak.facts.witness && !weak.facts.video_available);
    }

    #[test]
    fn window_hours_and_narrative_shape() {
        let g = mk(7);
        let res_mid: Vec<&Case> = g
            .ds
            .cases
            .iter()
            .filter(|c| c.mo.as_ref().is_some_and(|m| m.premise == Premise::Residential && m.time_band == TimeBand::Midday))
            .collect();
        assert!(!res_mid.is_empty());
        // Spatiotemporal decoys deliberately have a confirmed 30-minute window.
        assert!(res_mid
            .iter()
            .all(|c| (c.facts.window_hours >= 5.9 && c.facts.window_hours <= 10.1) || c.facts.window_hours == 0.5));
        assert!(g.ds.cases.iter().any(|c| c.narrative.contains("UNK SUSP") || c.narrative.contains("POE")));
    }

    #[test]
    fn written_files_load_in_store() {
        let g = mk(7);
        let root = std::env::temp_dir().join(format!("casefile-datagen-test-{}", std::process::id()));
        write_all(&root.join("data/seed"), &g).unwrap();
        let store = crate::store::Store::load(&root).unwrap();
        assert_eq!(store.ds.cases.len(), 2000);
        assert!(store.truth.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
