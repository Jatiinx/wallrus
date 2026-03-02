/// Palette image handling — extract colors from 1x4 palette images
/// and list available palette images organized by category (subfolder).
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gtk4::glib;
use image::{ImageBuffer, Rgb};

/// A category name mapped to its palette image paths (sorted by filename).
pub type PaletteCategories = BTreeMap<String, Vec<PathBuf>>;

/// Extract 4 colors from a palette image.
///
/// The image is expected to be 1x4px (one pixel per color, top to bottom).
/// For backward compatibility, larger images are also supported — the image
/// is divided into 4 equal horizontal bands and the center pixel of each
/// band is sampled.
pub fn extract_colors_from_image(path: &Path) -> Result<[[f32; 3]; 4], String> {
    let img = image::open(path).map_err(|e| format!("Failed to load image: {}", e))?;
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();

    if width == 0 || height == 0 {
        return Err("Image has zero dimensions".to_string());
    }

    let cx = width / 2;
    let band_height = height / 4;

    let mut colors = [[0.0f32; 3]; 4];
    for i in 0..4 {
        let cy = band_height * i as u32 + band_height / 2;
        let cy = cy.min(height - 1);
        let pixel = rgb.get_pixel(cx, cy);
        colors[i] = [
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
        ];
    }

    Ok(colors)
}

/// List all palette images organized by category.
///
/// Categories are subfolders inside the user palettes directory.
/// Bundled palettes are expected to have been copied there already by
/// `sync_bundled_palettes()`.
/// Images directly in the root (not in a subfolder) go into an "Uncategorized" category.
/// Categories are sorted alphabetically; images within each are sorted by filename.
pub fn list_palette_categories() -> PaletteCategories {
    let mut categories: PaletteCategories = BTreeMap::new();

    let user_dir = user_palettes_dir();
    collect_categorized_images(&user_dir, &mut categories);

    // Sort images within each category by filename
    for images in categories.values_mut() {
        images.sort_by(|a, b| {
            a.file_name()
                .unwrap_or_default()
                .cmp(b.file_name().unwrap_or_default())
        });
    }

    categories
}

/// Copy bundled palette images into the user-writable palettes directory.
///
/// On first run, all bundled palettes are copied. On subsequent runs, only
/// newly added bundled palettes (e.g. from app updates) are copied. Palettes
/// the user has deleted are tracked in a manifest file and will not be
/// re-copied.
///
/// The manifest file (`.bundled_manifest`) lives in the user palettes directory
/// and contains one relative path per line (e.g. `earth/ffaa00ff8800.png`)
/// for every bundled palette that has been seen.
pub fn sync_bundled_palettes() {
    let bundled_dir = match bundled_palettes_dir() {
        Some(d) => d,
        None => return, // No bundled palettes found (shouldn't happen in practice)
    };

    let user_dir = user_palettes_dir();
    let manifest_path = user_dir.join(".bundled_manifest");

    // Load existing manifest entries
    let mut manifest: BTreeSet<String> = match std::fs::read_to_string(&manifest_path) {
        Ok(contents) => contents.lines().map(|l| l.to_string()).collect(),
        Err(_) => BTreeSet::new(),
    };

    let mut manifest_changed = false;

    // Walk bundled palettes: each subfolder is a category
    let entries = match std::fs::read_dir(&bundled_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let category = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let sub_entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for sub_entry in sub_entries.flatten() {
            let src = sub_entry.path();
            if !src.is_file() || !is_image_file(&src) {
                continue;
            }

            let filename = match src.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let relative = format!("{}/{}", category, filename);

            if manifest.contains(&relative) {
                // Already copied (or was copied and then deleted by user) — skip
                continue;
            }

            // Copy to user directory
            let dest_dir = user_dir.join(&category);
            if !dest_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                    eprintln!(
                        "Failed to create palette category dir '{}': {}",
                        dest_dir.display(),
                        e
                    );
                    continue;
                }
            }

            let dest = dest_dir.join(&filename);
            if let Err(e) = std::fs::copy(&src, &dest) {
                eprintln!("Failed to copy bundled palette '{}': {}", relative, e);
                continue;
            }

            manifest.insert(relative);
            manifest_changed = true;
        }
    }

    // Write updated manifest back to disk
    if manifest_changed {
        let contents: String = manifest
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(e) = std::fs::write(&manifest_path, contents) {
            eprintln!("Failed to write bundled manifest: {}", e);
        }
    }
}

/// Save 4 colors as a 1x4px palette PNG in the user data directory.
///
/// The image is saved under the given `category` subfolder (lowercased) with
/// a timestamp-based name. Returns the path of the saved file.
pub fn save_palette_image(colors: &[[f32; 3]; 4], category: &str) -> Result<PathBuf, String> {
    let cat_dir = user_palettes_dir().join(category.to_lowercase());
    if !cat_dir.exists() {
        std::fs::create_dir_all(&cat_dir)
            .map_err(|e| format!("Failed to create palettes dir: {}", e))?;
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let filename = format!("palette_{}.png", timestamp);
    let path = cat_dir.join(&filename);

    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(1, 4);
    for (i, color) in colors.iter().enumerate() {
        let r = (color[0] * 255.0).round() as u8;
        let g = (color[1] * 255.0).round() as u8;
        let b = (color[2] * 255.0).round() as u8;
        img.put_pixel(0, i as u32, Rgb([r, g, b]));
    }

    img.save(&path)
        .map_err(|e| format!("Failed to save palette image: {}", e))?;

    Ok(path)
}

/// Delete a palette image.
///
/// All palettes live in the user-writable directory (bundled palettes are
/// copied there by `sync_bundled_palettes()`), so the safety check simply
/// ensures the path is inside that directory.
pub fn delete_palette_image(path: &Path) -> Result<(), String> {
    let user_dir = user_palettes_dir();
    if !path.starts_with(&user_dir) {
        return Err("Cannot delete bundled palettes".to_string());
    }

    std::fs::remove_file(path).map_err(|e| format!("Failed to delete palette: {}", e))
}

/// Get the user palettes directory inside the sandbox data dir.
///
/// In Flatpak this is `~/.var/app/io.github.megakode.Wallrus/data/palettes/`.
/// Outside Flatpak this is `~/.local/share/palettes/` (via `g_get_user_data_dir()`).
/// Creates the directory if it doesn't exist.
pub fn user_palettes_dir() -> PathBuf {
    let dir = glib::user_data_dir().join("palettes");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

/// Get the bundled palettes directory.
///
/// Looks for palettes relative to the executable, then falls back to
/// common installation paths. During development this is `data/palettes/`
/// relative to the project root.
pub fn bundled_palettes_dir() -> Option<PathBuf> {
    // During development: look relative to the executable
    if let Ok(exe) = std::env::current_exe() {
        // target/debug/wallrus -> project_root/data/palettes
        if let Some(project_root) = exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            let dev_path = project_root.join("data").join("palettes");
            if dev_path.is_dir() {
                return Some(dev_path);
            }
        }
    }

    // Installed (prefix-relative): <prefix>/bin/wallrus -> <prefix>/share/wallrus/palettes
    if let Ok(exe) = std::env::current_exe() {
        if let Some(prefix) = exe.parent().and_then(|p| p.parent()) {
            let prefix_path = prefix.join("share").join("wallrus").join("palettes");
            if prefix_path.is_dir() {
                return Some(prefix_path);
            }
        }
    }

    // Installed: /usr/share/wallrus/palettes
    let system_path = PathBuf::from("/usr/share/wallrus/palettes");
    if system_path.is_dir() {
        return Some(system_path);
    }

    // Flatpak or local: /app/share/wallrus/palettes
    let flatpak_path = PathBuf::from("/app/share/wallrus/palettes");
    if flatpak_path.is_dir() {
        return Some(flatpak_path);
    }

    None
}

/// Scan a palette root directory for categorized images.
///
/// - Subfolders become categories (folder name with first letter capitalized).
/// - Image files directly in the root go into "Uncategorized".
fn collect_categorized_images(root: &Path, categories: &mut PaletteCategories) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Subfolder = category
            let category_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(capitalize_first)
                .unwrap_or_else(|| "Uncategorized".to_string());

            let sub_entries = match std::fs::read_dir(&path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for sub_entry in sub_entries.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.is_file() && is_image_file(&sub_path) {
                    categories
                        .entry(category_name.clone())
                        .or_default()
                        .push(sub_path);
                }
            }
        } else if path.is_file() && is_image_file(&path) {
            // Files directly in root go to "Uncategorized"
            categories
                .entry("Uncategorized".to_string())
                .or_default()
                .push(path);
        }
    }
}

fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
        .unwrap_or(false)
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
