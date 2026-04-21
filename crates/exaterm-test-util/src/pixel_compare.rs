//! Pure-Rust pixel comparison, PNG I/O, and visual baseline management.

use std::io;
use std::path::{Path, PathBuf};

/// Raw RGBA pixel buffer.  Row-major, 4 bytes per pixel (R, G, B, A).
#[derive(Clone, Debug)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbaImage {
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), (width as usize) * (height as usize) * 4);
        Self {
            width,
            height,
            data,
        }
    }
}

/// Configuration for pixel-level image comparison.
#[derive(Clone, Debug)]
pub struct CompareConfig {
    pub channel_tolerance: u8,
    pub match_threshold: f64,
    pub generate_diff: bool,
    pub update_baseline: Option<bool>,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            channel_tolerance: 5,
            match_threshold: 0.98,
            generate_diff: true,
            update_baseline: None,
        }
    }
}

/// Result of comparing two images.
#[derive(Clone, Debug)]
pub struct CompareResult {
    pub matched_ratio: f64,
    pub mismatched_pixels: u32,
    pub passed: bool,
    pub diff_image: Option<RgbaImage>,
}

/// Error returned when a visual baseline comparison fails.
#[derive(Debug)]
pub struct VisualMismatch {
    pub baseline_name: String,
    pub result: CompareResult,
    pub actual_path: PathBuf,
    pub diff_path: Option<PathBuf>,
}

#[allow(clippy::float_arithmetic)]
impl std::fmt::Display for VisualMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "visual mismatch for '{}': {:.1}% match ({} mismatched pixels), actual saved to {}",
            self.baseline_name,
            self.result.matched_ratio * 100.0,
            self.result.mismatched_pixels,
            self.actual_path.display(),
        )
    }
}

impl std::error::Error for VisualMismatch {}

// ---------------------------------------------------------------------------
// Core operations
// ---------------------------------------------------------------------------

/// Extract the RGBA value of a single pixel.
///
/// Returns `None` if `(x, y)` is out of bounds.
#[must_use]
#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
pub fn pixel_at(image: &RgbaImage, x: u32, y: u32) -> Option<[u8; 4]> {
    if x >= image.width || y >= image.height {
        return None;
    }
    let idx = (y as usize)
        .checked_mul(image.width as usize)?
        .checked_add(x as usize)?
        .checked_mul(4)?;
    let red = *image.data.get(idx)?;
    let green = *image.data.get(idx + 1)?;
    let blue = *image.data.get(idx + 2)?;
    let alpha = *image.data.get(idx + 3)?;
    Some([red, green, blue, alpha])
}

/// Compare two images pixel-by-pixel with the given tolerance.
#[must_use]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_arithmetic
)]
pub fn compare(actual: &RgbaImage, expected: &RgbaImage, config: &CompareConfig) -> CompareResult {
    if actual.width != expected.width || actual.height != expected.height {
        return CompareResult {
            matched_ratio: 0.0,
            mismatched_pixels: u32::MAX,
            passed: false,
            diff_image: None,
        };
    }

    let total_pixels = u64::from(actual.width) * u64::from(actual.height);

    if total_pixels == 0 {
        return CompareResult {
            matched_ratio: 1.0,
            mismatched_pixels: 0,
            passed: true,
            diff_image: None,
        };
    }

    let required_bytes = match total_pixels.checked_mul(4) {
        Some(n) => n as usize,
        None => {
            return CompareResult {
                matched_ratio: 0.0,
                mismatched_pixels: u32::MAX,
                passed: false,
                diff_image: None,
            };
        }
    };
    if actual.data.len() < required_bytes || expected.data.len() < required_bytes {
        return CompareResult {
            matched_ratio: 0.0,
            mismatched_pixels: u32::MAX,
            passed: false,
            diff_image: None,
        };
    }

    let mut mismatched: u64 = 0;
    let tolerance = u16::from(config.channel_tolerance);

    let diff_data = if config.generate_diff {
        Some(Vec::with_capacity(actual.data.len()))
    } else {
        None
    };
    let mut diff_data = diff_data;

    for pixel_idx in 0..total_pixels {
        let byte_idx = (pixel_idx as usize) * 4;
        let actual_r = u16::from(*actual.data.get(byte_idx).unwrap_or(&0));
        let actual_g = u16::from(*actual.data.get(byte_idx + 1).unwrap_or(&0));
        let actual_b = u16::from(*actual.data.get(byte_idx + 2).unwrap_or(&0));
        let actual_a = u16::from(*actual.data.get(byte_idx + 3).unwrap_or(&0));

        let expect_r = u16::from(*expected.data.get(byte_idx).unwrap_or(&0));
        let expect_g = u16::from(*expected.data.get(byte_idx + 1).unwrap_or(&0));
        let expect_b = u16::from(*expected.data.get(byte_idx + 2).unwrap_or(&0));
        let expect_a = u16::from(*expected.data.get(byte_idx + 3).unwrap_or(&0));

        let diff_r = actual_r.abs_diff(expect_r);
        let diff_g = actual_g.abs_diff(expect_g);
        let diff_b = actual_b.abs_diff(expect_b);
        let diff_a = actual_a.abs_diff(expect_a);

        let is_match = diff_r <= tolerance
            && diff_g <= tolerance
            && diff_b <= tolerance
            && diff_a <= tolerance;

        if !is_match {
            mismatched += 1;
        }

        if let Some(ref mut buf) = diff_data {
            if is_match {
                buf.extend_from_slice(&[0, 255, 0, 255]);
            } else {
                buf.extend_from_slice(&[255, 0, 0, 255]);
            }
        }
    }

    let matched_ratio = if total_pixels > 0 {
        (total_pixels - mismatched) as f64 / total_pixels as f64
    } else {
        1.0
    };
    let passed = matched_ratio >= config.match_threshold;

    let diff_image = if mismatched > 0 {
        diff_data.map(|data| RgbaImage {
            width: actual.width,
            height: actual.height,
            data,
        })
    } else {
        None
    };

    let mismatched_u32 = if mismatched > u64::from(u32::MAX) {
        u32::MAX
    } else {
        mismatched as u32
    };

    CompareResult {
        matched_ratio,
        mismatched_pixels: mismatched_u32,
        passed,
        diff_image,
    }
}

// ---------------------------------------------------------------------------
// PNG I/O
// ---------------------------------------------------------------------------

/// Encode an [`RgbaImage`] as PNG and write it to `path`.
pub fn save_png(image: &RgbaImage, path: &Path) -> io::Result<()> {
    let file = std::fs::File::create(path)?;
    let w = io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(io::Error::other)?;
    writer
        .write_image_data(&image.data)
        .map_err(io::Error::other)?;
    Ok(())
}

/// Read a PNG file from `path` and decode it into an [`RgbaImage`].
#[allow(clippy::cast_possible_truncation)]
pub fn load_png(path: &Path) -> io::Result<RgbaImage> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(io::BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    buf.truncate(info.buffer_size());
    if info.color_type != png::ColorType::Rgba {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PNG is not RGBA format",
        ));
    }
    Ok(RgbaImage {
        width: info.width,
        height: info.height,
        data: buf,
    })
}

// ---------------------------------------------------------------------------
// Baseline management
// ---------------------------------------------------------------------------

/// Compare a captured image against a stored baseline.
pub fn assert_visual_match(
    actual: &RgbaImage,
    baseline_name: &str,
    baselines_dir: &Path,
    config: &CompareConfig,
) -> Result<(), VisualMismatch> {
    let baseline_path = baselines_dir.join(format!("{baseline_name}.png"));
    let actual_path = baselines_dir.join(format!("{baseline_name}.actual.png"));

    let should_update = config
        .update_baseline
        .unwrap_or_else(|| std::env::var("UPDATE_BASELINE").is_ok());

    if should_update {
        save_png(actual, &baseline_path).map_err(|_| VisualMismatch {
            baseline_name: baseline_name.to_owned(),
            result: CompareResult {
                matched_ratio: 0.0,
                mismatched_pixels: 0,
                passed: false,
                diff_image: None,
            },
            actual_path: actual_path.clone(),
            diff_path: None,
        })?;
        return Ok(());
    }

    let Ok(baseline) = load_png(&baseline_path) else {
        let _ = save_png(actual, &actual_path);
        return Err(VisualMismatch {
            baseline_name: baseline_name.to_owned(),
            result: CompareResult {
                matched_ratio: 0.0,
                mismatched_pixels: 0,
                passed: false,
                diff_image: None,
            },
            actual_path,
            diff_path: None,
        });
    };

    let result = compare(actual, &baseline, config);
    if result.passed {
        return Ok(());
    }

    let _ = save_png(actual, &actual_path);
    let diff_path = result.diff_image.as_ref().map(|diff| {
        let p = baselines_dir.join(format!("{baseline_name}.diff.png"));
        let _ = save_png(diff, &p);
        p
    });

    Err(VisualMismatch {
        baseline_name: baseline_name.to_owned(),
        result,
        actual_path,
        diff_path,
    })
}

/// Compute the fraction of pixels whose perceived luminance exceeds a threshold.
#[must_use]
#[allow(clippy::float_arithmetic, clippy::cast_lossless)]
pub fn bright_pixel_fraction(image: &RgbaImage, luminance_threshold: f64) -> f64 {
    let pixel_count = (image.width as usize) * (image.height as usize);
    if pixel_count == 0 {
        return 0.0;
    }
    if image.data.len() < pixel_count * 4 {
        return 0.0;
    }
    let mut bright = 0usize;
    for i in 0..pixel_count {
        let offset = i * 4;
        let r = image.data[offset] as f64 / 255.0;
        let g = image.data[offset + 1] as f64 / 255.0;
        let b = image.data[offset + 2] as f64 / 255.0;
        let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
        if luminance > luminance_threshold {
            bright += 1;
        }
    }
    bright as f64 / pixel_count as f64
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    clippy::float_arithmetic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;

    fn solid_2x2(r: u8, g: u8, b: u8, a: u8) -> RgbaImage {
        RgbaImage {
            width: 2,
            height: 2,
            data: vec![r, g, b, a, r, g, b, a, r, g, b, a, r, g, b, a],
        }
    }

    #[test]
    fn pixel_at_returns_correct_values() {
        let img = RgbaImage {
            width: 2,
            height: 1,
            data: vec![10, 20, 30, 255, 40, 50, 60, 128],
        };
        assert_eq!(pixel_at(&img, 0, 0), Some([10, 20, 30, 255]));
        assert_eq!(pixel_at(&img, 1, 0), Some([40, 50, 60, 128]));
    }

    #[test]
    fn pixel_at_out_of_bounds() {
        let img = solid_2x2(0, 0, 0, 255);
        assert_eq!(pixel_at(&img, 2, 0), None);
        assert_eq!(pixel_at(&img, 0, 2), None);
        assert_eq!(pixel_at(&img, 99, 99), None);
    }

    #[test]
    fn compare_identical_images_passes() {
        let a = solid_2x2(100, 150, 200, 255);
        let b = solid_2x2(100, 150, 200, 255);
        let result = compare(&a, &b, &CompareConfig::default());
        assert!(result.passed);
        assert_eq!(result.matched_ratio, 1.0);
        assert_eq!(result.mismatched_pixels, 0);
    }

    #[test]
    fn compare_within_tolerance_passes() {
        let a = solid_2x2(100, 150, 200, 255);
        let b = solid_2x2(103, 148, 200, 255);
        let config = CompareConfig {
            channel_tolerance: 5,
            update_baseline: Some(false),
            ..CompareConfig::default()
        };
        let result = compare(&a, &b, &config);
        assert!(result.passed);
        assert_eq!(result.mismatched_pixels, 0);
    }

    #[test]
    fn compare_beyond_tolerance_fails() {
        let a = solid_2x2(100, 150, 200, 255);
        let b = solid_2x2(200, 150, 200, 255);
        let config = CompareConfig {
            channel_tolerance: 5,
            match_threshold: 0.99,
            generate_diff: false,
            update_baseline: Some(false),
        };
        let result = compare(&a, &b, &config);
        assert!(!result.passed);
        assert_eq!(result.mismatched_pixels, 4);
    }

    #[test]
    fn compare_different_dimensions_fails() {
        let a = solid_2x2(0, 0, 0, 255);
        let b = RgbaImage {
            width: 3,
            height: 2,
            data: vec![0; 24],
        };
        let result = compare(&a, &b, &CompareConfig::default());
        assert!(!result.passed);
    }

    #[test]
    fn compare_empty_images_passes() {
        let a = RgbaImage {
            width: 0,
            height: 0,
            data: vec![],
        };
        let b = RgbaImage {
            width: 0,
            height: 0,
            data: vec![],
        };
        let result = compare(&a, &b, &CompareConfig::default());
        assert!(result.passed);
    }

    #[test]
    fn compare_generates_diff_image_on_mismatch() {
        let a = solid_2x2(0, 0, 0, 255);
        let b = solid_2x2(255, 255, 255, 255);
        let config = CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: true,
            update_baseline: Some(false),
        };
        let result = compare(&a, &b, &config);
        assert!(!result.passed);
        let diff = result.diff_image.as_ref().expect("should generate diff");
        assert_eq!(diff.width, 2);
        assert_eq!(diff.height, 2);
        assert_eq!(pixel_at(diff, 0, 0), Some([255, 0, 0, 255]));
    }

    #[test]
    fn png_save_load_round_trip() {
        let img = RgbaImage {
            width: 3,
            height: 2,
            data: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 128, 128, 128, 255, 0, 0, 0, 255,
                255, 255, 255, 255,
            ],
        };
        let dir = crate::test_tempdir();
        let path = dir.path().join("test.png");

        save_png(&img, &path).expect("save should succeed");
        assert!(path.exists());

        let loaded = load_png(&path).expect("load should succeed");
        assert_eq!(loaded.width, img.width);
        assert_eq!(loaded.height, img.height);
        assert_eq!(loaded.data, img.data);
    }

    #[test]
    fn assert_visual_match_update_baseline_writes_file() {
        let dir = crate::test_tempdir();
        let img = solid_2x2(42, 42, 42, 255);
        let config = CompareConfig {
            update_baseline: Some(true),
            ..CompareConfig::default()
        };
        let result = assert_visual_match(&img, "test_baseline", dir.path(), &config);
        assert!(result.is_ok());
        assert!(dir.path().join("test_baseline.png").exists());
    }

    #[test]
    fn assert_visual_match_matching_baseline_passes() {
        let dir = crate::test_tempdir();
        let img = solid_2x2(42, 42, 42, 255);
        save_png(&img, &dir.path().join("matching.png")).expect("save baseline");
        let config = CompareConfig {
            update_baseline: Some(false),
            ..CompareConfig::default()
        };
        let result = assert_visual_match(&img, "matching", dir.path(), &config);
        assert!(result.is_ok());
    }

    #[test]
    fn assert_visual_match_wrong_baseline_fails() {
        let dir = crate::test_tempdir();
        let baseline_img = solid_2x2(0, 0, 0, 255);
        let actual_img = solid_2x2(255, 255, 255, 255);
        save_png(&baseline_img, &dir.path().join("wrong.png")).expect("save baseline");
        let config = CompareConfig {
            channel_tolerance: 0,
            match_threshold: 1.0,
            generate_diff: true,
            update_baseline: Some(false),
        };
        let result = assert_visual_match(&actual_img, "wrong", dir.path(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn assert_visual_match_missing_baseline_fails() {
        let dir = crate::test_tempdir();
        let img = solid_2x2(42, 42, 42, 255);
        let config = CompareConfig {
            update_baseline: Some(false),
            ..CompareConfig::default()
        };
        let result = assert_visual_match(&img, "nonexistent", dir.path(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn bright_pixel_fraction_all_dark() {
        let img = solid_2x2(0, 0, 0, 255);
        assert_eq!(bright_pixel_fraction(&img, 0.5), 0.0);
    }

    #[test]
    fn bright_pixel_fraction_all_bright() {
        let img = solid_2x2(255, 255, 255, 255);
        assert_eq!(bright_pixel_fraction(&img, 0.5), 1.0);
    }

    #[test]
    fn bright_pixel_fraction_empty_image() {
        let img = RgbaImage::new(0, 0, vec![]);
        assert_eq!(bright_pixel_fraction(&img, 0.5), 0.0);
    }

    #[test]
    fn bright_pixel_fraction_threshold_boundary() {
        let img = solid_2x2(128, 128, 128, 255);
        assert!(
            bright_pixel_fraction(&img, 0.5) > 0.0,
            "mid-gray (luminance ~0.502) should be above 0.5 threshold"
        );
    }
}
