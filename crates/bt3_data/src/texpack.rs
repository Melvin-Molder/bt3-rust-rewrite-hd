//! The names PCSX2's texture replacement gives the game's textures, so packs made for the game under that emulator
//! (folder `SLUS-21678`) can replace the decoded textures. An independent implementation of the naming as observed
//! in PCSX2's dumps and in Tenkaichi3Decomp's matching against an 18,700-file pack (see `bt3-format-research.md`).
//!
//! A file is named `<texture hash>-<palette hash>-<bits>` (`.dds` or `.png`; `<texture hash>-<bits>` without a
//! palette), the hashes in hexadecimal (PCSX2 writes them without leading zeros), `bits` as 8 digits:
//! - texture hash: XXH3-64 (seed 0) over the texture's raw 256-byte GS memory blocks, block row by block row over
//!   its `2^TW x 2^TH` rectangle, as they lie in memory (swizzled);
//! - palette hash: XXH3-64 over the palette's 32-bit entries in index order (raw GS colours, alpha 0x80 = 1.0),
//!   for most files only from the lowest to the highest index the texture uses, for some the whole palette;
//! - bits: PSM | TW << 6 | TH << 10 | TCC << 14.
//!
//! Textures smaller than one block are hashed another way by PCSX2 and get no name here.

use twox_hash::XxHash3_64;

/// What a paletted texture's name takes from its pixels: the hash of its GS memory blocks, its format and size,
/// and the lowest and highest palette index it uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureKey {
    pub hash: u64,
    pub psm: u8,
    /// log2 of the width and height (TEX0 TW, TH).
    pub tw: u8,
    pub th: u8,
    pub used: [u8; 2],
}

/// A paletted texture's name with the palette it is drawn with, in the forms a pack may use.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackName {
    pub texture: u64,
    /// Per palette the texture may be drawn with (the first its own): the hash over the entries used, then over
    /// the whole palette.
    pub clut: Vec<u64>,
    /// The lowest and highest palette index the texture uses.
    pub used: [u8; 2],
    /// PSM, TW, TH (TCC is the draw's: both are looked up).
    pub bits: u32,
    /// The largest raw alpha (0x80 = 1.0) of the entries used: a replacement's alpha is kept at or below it (the
    /// game uses alpha bit 7 written into the frame as a mask).
    pub max_alpha: u8,
    /// How the decoded texture holds alpha, which a replacement (raw GS alpha) is converted to.
    pub alpha: PackAlpha,
}

/// The alpha of a decoded texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackAlpha {
    /// As the GS holds it (0x80 = 1.0).
    Raw,
    /// Doubled (0x80 = 0xFF), as the screens' textures hold it.
    Doubled,
    /// Opaque everywhere (a texture drawn without alpha).
    Opaque,
}

impl PackAlpha {
    /// A replacement's raw alpha `a` kept at or below `max` (the original's largest), as the texture holds it.
    pub fn convert(self, a: u8, max: u8) -> u8 {
        match self {
            PackAlpha::Raw => a.min(max),
            PackAlpha::Doubled => (u16::from(a.min(max)) * 2).min(255) as u8,
            PackAlpha::Opaque => 0xFF,
        }
    }
}

impl PackName {
    /// The name of the texture `key` drawn with `palette` (index order, raw GS colours), its alpha held `alpha`.
    pub fn new(key: &TextureKey, palette: &[[u8; 4]], alpha: PackAlpha) -> Self {
        let mut name = Self {
            texture: key.hash,
            clut: Vec::new(),
            used: key.used,
            bits: bits(key.psm, key.tw, key.th, false),
            max_alpha: used_entries(key.used, palette)
                .iter()
                .map(|c| c[3])
                .max()
                .unwrap_or(0),
            alpha,
        };
        name.add_palette(palette);
        name
    }

    /// Another palette the texture may be drawn with: its names are looked up after those before.
    pub fn add_palette(&mut self, palette: &[[u8; 4]]) {
        let bytes = |entries: &[[u8; 4]]| entries.iter().flatten().copied().collect::<Vec<u8>>();
        self.clut.extend([
            XxHash3_64::oneshot(&bytes(used_entries(self.used, palette))),
            XxHash3_64::oneshot(&bytes(palette)),
        ]);
    }

    /// The (texture hash, palette hash, bits) a pack may name the texture by, most likely first: TCC on (the
    /// game draws its textures with RGBA), then off; per palette the hash over the entries used, then the whole.
    pub fn candidates(&self) -> impl Iterator<Item = (u64, u64, u32)> + '_ {
        let tcc = 1 << 14;
        [self.bits | tcc, self.bits]
            .into_iter()
            .flat_map(move |bits| {
                self.clut
                    .iter()
                    .map(move |&clut| (self.texture, clut, bits))
            })
    }
}

/// The palette entries from the lowest to the highest index used.
fn used_entries(used: [u8; 2], palette: &[[u8; 4]]) -> &[[u8; 4]] {
    let [lo, hi] = used.map(usize::from);
    let hi = hi.min(palette.len().saturating_sub(1));
    palette.get(lo..=hi).unwrap_or(&[])
}

/// The `bits` part of a name.
pub fn bits(psm: u8, tw: u8, th: u8, tcc: bool) -> u32 {
    u32::from(psm & 0x3F)
        | u32::from(tw & 0xF) << 6
        | u32::from(th & 0xF) << 10
        | u32::from(tcc) << 14
}

/// XXH3-64 of a texture's blocks (each 256 bytes, in order).
pub fn blocks_hash(blocks: &[u8]) -> u64 {
    XxHash3_64::oneshot(blocks)
}

/// A file name's stem as (texture hash, palette hash (0 without), bits); `None` for other names (the region and
/// mip-level forms are not used by this game's packs).
pub fn parse_name(stem: &str) -> Option<(u64, u64, u32)> {
    let parts: Vec<&str> = stem.split('-').collect();
    let hex64 = |s: &str| {
        (!s.is_empty() && s.len() <= 16)
            .then(|| u64::from_str_radix(s, 16).ok())
            .flatten()
    };
    let hex32 = |s: &str| {
        (!s.is_empty() && s.len() <= 8)
            .then(|| u32::from_str_radix(s, 16).ok())
            .flatten()
    };
    match parts.as_slice() {
        [texture, bits] => Some((hex64(texture)?, 0, hex32(bits)?)),
        [texture, clut, bits] => Some((hex64(texture)?, hex64(clut)?, hex32(bits)?)),
        _ => None,
    }
}

/// The stem PCSX2 writes for a name: hashes without leading zeros, bits as 8 digits.
pub fn format_name(texture: u64, clut: u64, bits: u32) -> String {
    if clut == 0 {
        format!("{texture:x}-{bits:08x}")
    } else {
        format!("{texture:x}-{clut:x}-{bits:08x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh3_matches_the_reference_vectors() {
        // XXH3_64bits("") (seed 0), from the xxHash reference.
        assert_eq!(blocks_hash(b""), 0x2D06800538D394C2);
    }

    #[test]
    fn names_parse_and_format_as_pcsx2_writes_them() {
        // A file of the game's pack: a 512 x 128 PSMT4 sheet drawn with RGBA.
        let stem = "ab4d924a3991fead-c7a320092fc1176-00006254";
        let (texture, clut, b) = parse_name(stem).unwrap();
        assert_eq!(
            (texture, clut, b),
            (0xab4d924a3991fead, 0x0c7a320092fc1176, 0x6254)
        );
        assert_eq!(b, bits(0x14, 9, 8, true));
        assert_eq!(format_name(texture, clut, b), stem);
        assert_eq!(parse_name("1234-00000013"), Some((0x1234, 0, 0x13)));
        assert_eq!(parse_name("12-34-56-78"), None);
        assert_eq!(parse_name("zz-00000013"), None);
    }

    #[test]
    fn the_palette_hash_covers_the_used_entries_and_the_whole() {
        let palette: Vec<[u8; 4]> = (0..16u8).map(|i| [i, i, i, 0x40 + i]).collect();
        let key = TextureKey {
            hash: 7,
            psm: 0x14,
            tw: 5,
            th: 4,
            used: [2, 5],
        };
        let name = PackName::new(&key, &palette, PackAlpha::Doubled);
        let used: Vec<u8> = palette[2..=5].iter().flatten().copied().collect();
        let all: Vec<u8> = palette.iter().flatten().copied().collect();
        assert_eq!(name.clut, [blocks_hash(&used), blocks_hash(&all)]);
        assert_eq!(name.max_alpha, 0x45);
        // A replacement's alpha: kept at the original's largest, then held as the texture holds it.
        assert_eq!(PackAlpha::Raw.convert(0x8E, 0x80), 0x80);
        assert_eq!(PackAlpha::Doubled.convert(0x8E, 0x7F), 0xFE);
        assert_eq!(PackAlpha::Doubled.convert(0x20, 0x80), 0x40);
        assert_eq!(PackAlpha::Opaque.convert(0, 0), 0xFF);
        let tcc = 0x14 | 5 << 6 | 4 << 10 | 1 << 14;
        assert_eq!(name.candidates().next(), Some((7, name.clut[0], tcc)));
        assert_eq!(name.candidates().count(), 4);
        // Another palette (the fighters' CLUTs with their alpha slots): its names after the first's.
        let mut name = name;
        let mut other = palette.clone();
        other[3][3] = 9;
        name.add_palette(&other);
        assert_eq!(name.clut.len(), 4);
        let used: Vec<u8> = other[2..=5].iter().flatten().copied().collect();
        assert_eq!(name.candidates().nth(2), Some((7, blocks_hash(&used), tcc)));
        assert_eq!(name.candidates().count(), 8);
    }
}
