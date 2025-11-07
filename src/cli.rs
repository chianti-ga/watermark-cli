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
use clap::{ Parser, ValueEnum };
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Pattern {
    Diagonal,
    Horizontal,
    Vertical,
    CrossDiagonal,
    Random,
}

const LONG_ABOUT: &str =
    "\
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
    /// Output directory
    #[arg(short = 'o', long, value_hint = clap::ValueHint::DirPath)]
    pub output_path: Option<PathBuf>,

    /// Watermark text
    #[arg(short = 'w', long, default_value = "WATERMARK")]
    pub watermark: String,

    /// Image quality 1–100
    #[arg(short = 'q', long, default_value_t = 85)]
    pub compression: u8,

    /// Watermark text scale
    #[arg(short = 't', long, default_value = "0.05")]
    pub text_scale: f32,

    /// Vertical spacing between watermarks
    #[arg(short = 's', long, default_value = "1.5")]
    pub space_scale: f32,

    /// Watermark text color in hex (e.g., FF0000 for red)
    #[arg(short = 'c', long, default_value = "808080")]
    pub color: String,

    /// Watermark opacity (0-255)
    #[arg(short = 'a', long, default_value_t = 150)]
    pub opacity: u8,

    /// Pattern of watermark
    #[arg(short = 'p', long, default_value_t = Pattern::Diagonal, value_enum)]
    pub pattern: Pattern,

    /// Recursively apply watermark to all images in the specified directory
    #[arg(short = 'r', long, action)]
    pub recursive: bool,

    /// Force GPU usage
    #[arg(short = 'g', long, default_value = "auto", value_parser = ["auto", "on", "off"])]
    pub gpu: String,

    /// Max CPU worker threads (Rayon). Default
    /// - when --gpu on = 1
    #[arg(short = 'k', long)]
    pub threads: Option<usize>,

    /// Input image file/directory
    #[arg(value_hint = clap::ValueHint::FilePath)]
    pub input_path: PathBuf,
}

// #[derive(Deserialize, Debug)]
// struct Tag { name: String }

// #[cfg(feature = "auto-update")]
// pub fn check_update() {
//     let config_file = dirs::home_dir()
//         .unwrap_or_default()
//         .join(".watermark-cli");

//     if !config_file.exists() {
//         println!("Would you like to enable automatic update checks? [Y/n]");
//         let mut input = String::new();
//         std::io::stdin().read_line(&mut input).unwrap_or_default();
//         let enable_updates = input.trim().to_lowercase() != "n";
//         fs::write(&config_file, if enable_updates { "1" } else { "0" }).unwrap_or_default();
//     }

//     if fs::read_to_string(&config_file).unwrap_or_default().trim() == "1" {
//         let current = env!("CARGO_PKG_VERSION");

//         match reqwest::blocking::Client::new()
//             .get("https://api.github.com/repos/chianti-ga/watermark-cli/tags")
//             .header(reqwest::header::USER_AGENT, format!("watermark-cli/{}", current))
//             .send()
//             .and_then(|response| response.json::<Vec<Tag>>())
//         {
//             Ok(tags) => {
//                 if let Some(latest_tag) = tags.first() {
//                     if latest_tag.name != format!("v{current}") {
//                         println!(
//                             "🎉 New version {} available! (Current version: v{})",
//                             latest_tag.name, current
//                         );
//                     }
//                 }
//             }
//             Err(_) => println!("Unable to check for updates"),
//         }
//     }
// }