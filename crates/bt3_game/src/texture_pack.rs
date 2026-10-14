//! `--textures DIR`: textures replaced from a pack made for the game under PCSX2 (the folder named after the
//! serial, SLUS-21678; its `replacements` folder is read if it has one). An enhancement the console does not have,
//! off by default. A decoded texture's PCSX2 name (`bt3_data::texpack`) is looked up when it is uploaded; a file
//! found takes its place, its UVs unchanged (they are normalised, so a larger picture maps the same). The pack's
//! alpha is the GS's (0x80 = 1.0): it is kept at or below the original's largest and held as the texture holds
//! alpha. Reads DDS (DXT1 / DXT3 / DXT5 or 32-bit, also under a DX10 header, with its mip levels) and PNG (mip levels made here).
//! `BT3_TEX_LOG=1` prints each lookup, found or missing, with the name the pack would need.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler,
    ImageSamplerDescriptor, ImageType,
};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bt3_data::gs::RgbaTexture;
use bt3_data::texpack::{PackName, format_name, parse_name};

/// The pack's files by name (texture hash, palette hash, bits); empty without `--textures`.
#[derive(Resource, Default)]
pub struct TexturePack {
    files: HashMap<(u64, u64, u32), PathBuf>,
    log: bool,
}

/// A replacement picture: RGBA8 levels, the first `width` x `height`, each half the one before.
pub struct Replacement {
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<u8>>,
}

impl TexturePack {
    /// Indexes the pack in `dir` (its `replacements` folder if it has one), at any depth. Where a name is found
    /// twice the first path in order wins.
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        let root = dir.join("replacements");
        let root = if root.is_dir() {
            root
        } else {
            dir.to_path_buf()
        };
        let mut paths = Vec::new();
        collect(&root, &mut paths)?;
        paths.sort();
        let mut files = HashMap::new();
        for path in paths {
            let (Some(stem), Some(ext)) = (path.file_stem(), path.extension()) else {
                continue;
            };
            let ext = ext.to_string_lossy().to_ascii_lowercase();
            if (ext == "dds" || ext == "png")
                && let Some(name) = parse_name(&stem.to_string_lossy())
            {
                files.entry(name).or_insert(path);
            }
        }
        Ok(Self {
            files,
            log: std::env::var_os("BT3_TEX_LOG").is_some(),
        })
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// The file for a texture of this name, if the pack has one.
    pub fn find(&self, name: &PackName) -> Option<&Path> {
        name.candidates()
            .find_map(|key| self.files.get(&key))
            .map(PathBuf::as_path)
    }

    /// The replacement for `texture`, its alpha as the texture holds it; `None` without one (or if it cannot be
    /// read, which is logged).
    pub fn replacement(&self, texture: &RgbaTexture) -> Option<Replacement> {
        if self.files.is_empty() {
            return None;
        }
        let name = texture.pack.as_ref()?;
        let path = self.find(name);
        if self.log {
            let (t, c, b) = name.candidates().next()?;
            info!(
                "texture pack: {} {}x{} {:?} {}{}",
                if path.is_some() { "found  " } else { "missing" },
                texture.width,
                texture.height,
                name.alpha,
                format_name(t, c, b),
                path.map(|p| format!(" {}", p.display()))
                    .unwrap_or_default()
            );
        }
        let path = path?;
        let mut picture = match read_picture(path) {
            Ok(picture) => picture,
            Err(error) => {
                warn!("texture pack: cannot read {}: {error}", path.display());
                return None;
            }
        };
        for level in &mut picture.levels {
            for pixel in level.chunks_exact_mut(4) {
                pixel[3] = name.alpha.convert(pixel[3], name.max_alpha);
            }
        }
        Some(picture)
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

impl Replacement {
    /// The picture as a texture: `format` (the texture's own), sampled bilinearly with `address`; with `mips` its
    /// smaller levels too (the 3D textures; the screens' sheets keep only the first, so a level never averages
    /// neighbouring pieces of a sheet).
    pub fn image(&self, format: TextureFormat, address: ImageAddressMode, mips: bool) -> Image {
        let levels = if mips { self.levels.len() } else { 1 };
        let mut image = Image::new_uninit(
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            format,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_descriptor.mip_level_count = levels as u32;
        image.data = Some(self.levels[..levels].concat());
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address,
            address_mode_v: address,
            mipmap_filter: ImageFilterMode::Linear,
            ..ImageSamplerDescriptor::linear()
        });
        image
    }
}

/// A DDS or PNG file's levels as RGBA8.
fn read_picture(path: &Path) -> Result<Replacement, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.starts_with(b"DDS ") {
        return read_dds(&bytes);
    }
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD,
    )
    .map_err(|e| e.to_string())?;
    let image = if image.texture_descriptor.format == TextureFormat::Rgba8Unorm {
        image
    } else {
        image
            .convert(TextureFormat::Rgba8Unorm)
            .ok_or("unsupported PNG format")?
    };
    let (width, height) = (image.width(), image.height());
    let first = image.data.ok_or("PNG without pixels")?;
    Ok(Replacement {
        width,
        height,
        levels: with_smaller_levels(width, height, first),
    })
}

/// The level and its halves down to 1 x 1, each pixel the average of 2 x 2 of the one before.
fn with_smaller_levels(width: u32, height: u32, first: Vec<u8>) -> Vec<Vec<u8>> {
    let mut levels = vec![first];
    let (mut w, mut h) = (width as usize, height as usize);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let prev = levels.last().unwrap();
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |sx: usize, sy: usize| {
                        u32::from(prev[((sy.min(h - 1)) * w + sx.min(w - 1)) * 4 + c])
                    };
                    let sum = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    next[(y * nw + x) * 4 + c] = ((sum + 2) / 4) as u8;
                }
            }
        }
        levels.push(next);
        (w, h) = (nw, nh);
    }
    levels
}

/// A DDS file: DXT1 / DXT3 / DXT5 or 32-bit RGBA by masks, with its mip levels.
fn read_dds(bytes: &[u8]) -> Result<Replacement, String> {
    let u32_at = |o: usize| {
        bytes
            .get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or("DDS header is truncated")
    };
    if u32_at(4)? != 124 {
        return Err("not a DDS header".into());
    }
    let height = u32_at(12)?;
    let width = u32_at(16)?;
    let mips = u32_at(28)?.max(1);
    let pf_flags = u32_at(80)?;
    let fourcc = bytes.get(84..88).ok_or("DDS header is truncated")?;
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err("bad DDS size".into());
    }
    enum Kind {
        Bc(usize),
        Rgba([u32; 4]),
    }
    let mut offset = 128;
    let kind = if pf_flags & 4 != 0 && fourcc == b"DX10" {
        // DDS_HEADER_DXT10: the DXGI format, the data after its 20 bytes.
        offset = 148;
        match u32_at(128)? {
            70..=72 => Kind::Bc(1),
            73..=75 => Kind::Bc(3),
            76..=78 => Kind::Bc(5),
            28 | 29 => Kind::Rgba([0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000]),
            87 | 91 => Kind::Rgba([0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000]),
            format => return Err(format!("unsupported DXGI format {format}")),
        }
    } else if pf_flags & 4 != 0 {
        match fourcc {
            b"DXT1" => Kind::Bc(1),
            b"DXT3" => Kind::Bc(3),
            b"DXT5" => Kind::Bc(5),
            _ => return Err(format!("unsupported DDS format {fourcc:?}")),
        }
    } else if pf_flags & 0x40 != 0 && u32_at(88)? == 32 {
        Kind::Rgba([u32_at(92)?, u32_at(96)?, u32_at(100)?, u32_at(104)?])
    } else {
        return Err("unsupported DDS pixel format".into());
    };
    let mut levels = Vec::new();
    for level in 0..mips {
        let (w, h) = ((width >> level).max(1), (height >> level).max(1));
        let (w, h) = (w as usize, h as usize);
        let size = match kind {
            Kind::Bc(n) => w.div_ceil(4) * h.div_ceil(4) * if n == 1 { 8 } else { 16 },
            Kind::Rgba(_) => w * h * 4,
        };
        let Some(data) = bytes.get(offset..offset + size) else {
            if level == 0 {
                return Err("DDS data is truncated".into());
            }
            break;
        };
        offset += size;
        levels.push(match kind {
            Kind::Bc(n) => decode_bc(data, w, h, n),
            Kind::Rgba(masks) => data
                .chunks_exact(4)
                .flat_map(|p| {
                    let v = u32::from_le_bytes(p.try_into().unwrap());
                    masks.map(|m| channel(v, m))
                })
                .collect(),
        });
    }
    Ok(Replacement {
        width,
        height,
        levels,
    })
}

/// A channel of a 32-bit pixel by its mask, as 8 bits (0xFF without a mask: an opaque alpha).
fn channel(v: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0xFF;
    }
    let shift = mask.trailing_zeros();
    let max = mask >> shift;
    (u64::from((v & mask) >> shift) * 255 / u64::from(max)) as u8
}

/// BC1 (DXT1), BC2 (DXT3) or BC3 (DXT5) blocks to RGBA8.
fn decode_bc(data: &[u8], width: usize, height: usize, kind: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let block_size = if kind == 1 { 8 } else { 16 };
    let blocks_wide = width.div_ceil(4);
    for (i, block) in data.chunks_exact(block_size).enumerate() {
        let (bx, by) = ((i % blocks_wide) * 4, (i / blocks_wide) * 4);
        let colour = &block[block_size - 8..];
        let alpha = &block[..block_size - 8];
        let rgb = |c: u16| {
            let (r, g, b) = (c >> 11, (c >> 5) & 0x3F, c & 0x1F);
            [
                (r << 3 | r >> 2) as u8,
                (g << 2 | g >> 4) as u8,
                (b << 3 | b >> 2) as u8,
            ]
        };
        let c0 = u16::from_le_bytes([colour[0], colour[1]]);
        let c1 = u16::from_le_bytes([colour[2], colour[3]]);
        let (p0, p1) = (rgb(c0), rgb(c1));
        let mix = |a: u8, b: u8, wa: u16, wb: u16| {
            ((u16::from(a) * wa + u16::from(b) * wb) / (wa + wb)) as u8
        };
        let mut palette = [[0u8; 4]; 4];
        palette[0] = [p0[0], p0[1], p0[2], 255];
        palette[1] = [p1[0], p1[1], p1[2], 255];
        if kind != 1 || c0 > c1 {
            let a = [0, 1, 2].map(|k| mix(p0[k], p1[k], 2, 1));
            let b = [0, 1, 2].map(|k| mix(p0[k], p1[k], 1, 2));
            palette[2] = [a[0], a[1], a[2], 255];
            palette[3] = [b[0], b[1], b[2], 255];
        } else {
            let m = [0, 1, 2].map(|k| mix(p0[k], p1[k], 1, 1));
            palette[2] = [m[0], m[1], m[2], 255];
            palette[3] = [0, 0, 0, 0];
        }
        let colour_bits = u32::from_le_bytes(colour[4..8].try_into().unwrap());
        // DXT5's alpha: two end points and eight steps between them (or six, 0 and 255).
        let alphas = if kind == 5 {
            let (a0, a1) = (alpha[0], alpha[1]);
            let mut a = [0u8; 8];
            a[0] = a0;
            a[1] = a1;
            if a0 > a1 {
                for k in 1..7u16 {
                    a[k as usize + 1] = ((u16::from(a0) * (7 - k) + u16::from(a1) * k) / 7) as u8;
                }
            } else {
                for k in 1..5u16 {
                    a[k as usize + 1] = ((u16::from(a0) * (5 - k) + u16::from(a1) * k) / 5) as u8;
                }
                a[6] = 0;
                a[7] = 255;
            }
            Some(a)
        } else {
            None
        };
        let alpha_bits = u64::from_le_bytes([
            alpha.get(2).copied().unwrap_or(0),
            alpha.get(3).copied().unwrap_or(0),
            alpha.get(4).copied().unwrap_or(0),
            alpha.get(5).copied().unwrap_or(0),
            alpha.get(6).copied().unwrap_or(0),
            alpha.get(7).copied().unwrap_or(0),
            0,
            0,
        ]);
        for py in 0..4 {
            for px in 0..4 {
                let (x, y) = (bx + px, by + py);
                if x >= width || y >= height {
                    continue;
                }
                let k = py * 4 + px;
                let mut p = palette[((colour_bits >> (2 * k)) & 3) as usize];
                match kind {
                    3 => {
                        let nibble = (alpha[k / 2] >> ((k & 1) * 4)) & 0xF;
                        p[3] = nibble << 4 | nibble;
                    }
                    5 => p[3] = alphas.unwrap()[((alpha_bits >> (3 * k)) & 7) as usize],
                    _ => {}
                }
                out[(y * width + x) * 4..][..4].copy_from_slice(&p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dxt5_block_decodes_its_colours_and_alphas() {
        // c0 pure red, c1 pure blue; colour index 1 everywhere but pixel 0 (0); alphas 0x80 / 0x00 with eight
        // steps, index 1 (0x00) everywhere but pixel 0 (0: 0x80).
        let mut block = [0u8; 16];
        block[0] = 0x80;
        block[1] = 0x00;
        let mut ab: u64 = 0;
        for k in 1..16 {
            ab |= 1 << (3 * k);
        }
        block[2..8].copy_from_slice(&ab.to_le_bytes()[..6]);
        block[8..10].copy_from_slice(&0xF800u16.to_le_bytes());
        block[10..12].copy_from_slice(&0x001Fu16.to_le_bytes());
        let mut cb: u32 = 0;
        for k in 1..16 {
            cb |= 1 << (2 * k);
        }
        block[12..16].copy_from_slice(&cb.to_le_bytes());
        let px = decode_bc(&block, 4, 4, 5);
        assert_eq!(&px[0..4], &[255, 0, 0, 0x80]);
        assert_eq!(&px[4..8], &[0, 0, 255, 0]);
    }

    #[test]
    fn smaller_levels_halve_down_to_one_pixel() {
        let first = vec![200u8; 8 * 2 * 4];
        let levels = with_smaller_levels(8, 2, first);
        let sizes: Vec<usize> = levels.iter().map(Vec::len).collect();
        assert_eq!(sizes, [64, 16, 8, 4]);
        assert!(levels.iter().flatten().all(|&v| v == 200));
    }

    #[test]
    fn a_32_bit_channel_reads_by_its_mask() {
        assert_eq!(channel(0xAABBCCDD, 0x00FF0000), 0xBB);
        assert_eq!(channel(0, 0), 0xFF);
    }
}
