/*
 *     watermark-cli, a command-line tool for adding watermarks to images and PDFs
 *     Copyright (C) 2025-2026  Chianti GALLY
 *
 *     This program is free software: you can redistribute it and/or modify
 *     it under the terms of the GNU General Public License as published by
 *     the Free Software Foundation, either version 3 of the License, or
 *     (at your option) any later version.
 *
 *     This program is distributed in the hope that it will be useful,
 *     but WITHOUT ANY WARRANTY; without even the implied warranty of
 *     MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *     GNU General Public License for more details.
 *
 *     You should have received a copy of the GNU General Public License
 *     along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{render, RenderSettings};
use std::path::Path;
use std::sync::Arc;

pub fn convert_to_image(pdf_path: &Path, output_dir: &Path) {
    let file = std::fs::read(pdf_path).unwrap();

    let data = Arc::new(file);
    let pdf = Pdf::new(data).unwrap();

    let interpreter_settings = InterpreterSettings::default();

    let render_settings = RenderSettings {
        x_scale: 2.0,
        y_scale: 2.0,
        ..Default::default()
    };

    for (idx, page) in pdf.pages().iter().enumerate() {
        let pixmap = render(page, &interpreter_settings, &render_settings);
        let output_path = format!("{}/rendered_{idx}.png", output_dir.to_str().unwrap());
        std::fs::write(output_path, pixmap.into_png()).unwrap();
    }
}