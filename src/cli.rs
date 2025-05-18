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
use std::path::PathBuf;
const LONG_ABOUT: &str = "\
Copyright (C) 2025  Chianti GALLY

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
    about = "Add watermark to images safely, and optionally generate PDFs.",
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

    /// Recursively apply watermark to all images in the specified directory
    #[arg(short, long, action)]
    pub recursive: bool,

    /// Create PDF of watermarked image(s) instead of an image
    #[arg(long, action)]
    pub pdf: bool,

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
