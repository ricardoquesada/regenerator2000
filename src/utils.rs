use image::DynamicImage;
use ratatui_image::picker::Picker;
use std::fs;
use std::path::{Path, PathBuf};

pub fn list_files(dir: &Path, extensions: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if extensions.iter().any(|e| e.to_lowercase() == ext_lower) {
                    files.push(path);
                }
            }
        }
    }

    // Sort: directories first, then files alpha
    files.sort_by(|a, b| {
        let a_is_dir = a.is_dir();
        let b_is_dir = b.is_dir();
        if a_is_dir && !b_is_dir {
            std::cmp::Ordering::Less
        } else if !a_is_dir && b_is_dir {
            std::cmp::Ordering::Greater
        } else {
            a.file_name().cmp(&b.file_name())
        }
    });

    files
}

pub fn load_logo() -> Option<DynamicImage> {
    let path = Path::new("docs/regenerator2000_logo.png");
    if path.exists() {
        if let Ok(img) = image::open(path) {
            return Some(img);
        }
    }
    None
}
pub fn create_picker() -> Option<Picker> {
    let font_size = (8, 16);
    Some(Picker::from_fontsize(font_size))
}
