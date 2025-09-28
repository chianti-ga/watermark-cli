/*
 * Copyright (C) 2025  Chianti GALLY
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

const LONG_ABOUT: &str = "\
A command-line tool for adding watermarks to images with support for batch processing and various watermark patterns.
Designed to prevent identity theft and unauthorized copying of official documents through visible watermarking.


Copyright (C) 2025 Chianti GALLY

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
";

#[derive(Parser, Debug)]
#[command(
    version,
    about = "A command-line tool for adding watermarks to images with support for batch processing and various watermark patterns.\nDesigned to prevent identity theft and unauthorized copying of official documents through visible watermarking.",
    long_about = LONG_ABOUT
)]
pub struct Cli {
    /// Input image file/directory
    #[arg(value_hint = clap::ValueHint::FilePath)]
    pub input_path: PathBuf,

    /// Watermark text
    pub watermark: String,

    /// JPEG quality 1–100
    #[arg(default_value_t = 90)]
    pub compression: u8,

    /// Vertical spacing between watermark
    #[arg(default_value = "1.5", short, long)]
    pub space_scale: f32,

    /// Watermark text scale
    #[arg(default_value = "0.05", short, long)]
    pub text_scale: f32,

    /// Recursively apply watermark to all images in the specified directory
    #[arg(short, long, action)]
    pub recursive: bool,

    /// Pattern of watermark
    #[arg(short, long, default_value = "diagonal")]
    pub pattern: Pattern,
}

#[derive(Debug, Clone, clap::ValueEnum)]

pub enum Pattern {
    Diagonal,
    Horizontal,
    Vertical,
    Random,
    CrossDiagonal,
}

#[derive(Deserialize, Debug)]
struct Tag {
    name: String,
}
#[cfg(feature = "auto-update")]
pub fn check_update() {
    let config_file = std::env::home_dir().unwrap_or_default().join(".watermark-cli");
    if !config_file.exists() {
        println!("Would you like to enable automatic update checks? [Y/n]");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .unwrap_or_default();
        let enable_updates = input.trim().to_lowercase() != "n";
        fs::write(&config_file, if enable_updates { "1" } else { "0" }).unwrap_or_default();
    }

    if fs::read_to_string(&config_file).unwrap_or_default().trim() == "1" {
        let current = env!("CARGO_PKG_VERSION");

        match reqwest::blocking::Client::new()
            .get("https://api.github.com/repos/chianti-ga/watermark-cli/tags")
            .header(reqwest::header::USER_AGENT, format!("watermark-cli/{}", current))
            .send()
            .and_then(|response| response.json::<Vec<Tag>>())
        {
            Ok(tags) => {
                if let Some(latest_tag) = tags.first() {
                    if latest_tag.name != format!("v{current}") {
                        println!("🎉 New version {} available! (Current version: v{})",
                                 latest_tag.name, current);
                    }
                }
            }
            Err(_) => println!("Unable to check for updates"),
        }
    }
}