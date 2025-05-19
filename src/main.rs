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

mod cli;

use crate::cli::Cli;
use ab_glyph::FontRef;
use clap::Parser;
use colored::Colorize;
use image::{DynamicImage, ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;
use imageproc::geometric_transformations::{rotate, Interpolation};
use imageproc::image;
use imageproc::image::codecs::jpeg::JpegEncoder;
use imageproc::image::codecs::png::PngEncoder;
use imageproc::image::codecs::webp::WebPEncoder;
use imageproc::image::imageops::overlay;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info};
use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, RawImage, XObjectId, XObjectTransform};
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn main() {
    let cli: Cli = Cli::parse();
    let start_time = Instant::now();

    if cli.recursive && cli.input_path.is_dir() {
        process_directory(&cli);
    } else {
        process_single_file(&cli);
    }

    let duration: Duration = start_time.elapsed();
    println!("{}", format!("Processing completed in {:.2} seconds", duration.as_secs_f32()).green());
}

fn process_single_file(cli: &Cli) {
    let input_file: &PathBuf = &cli.input_path;
    let file_stem: &str = input_file.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let extension: &str = if cli.pdf { "pdf" } else { input_file.extension().and_then(|s| s.to_str()).unwrap_or("jpeg") };

    let new_name: String = format!("{}_watermark.{}", file_stem, extension);
    let output_file: PathBuf = input_file.with_file_name(new_name);

    println!("{}", format!("Processing image: {}", input_file.display()).blue());

    if let Err(e) = add_watermark(input_file, &cli.watermark, &output_file, &cli.compression, &cli.text_scale, &cli.space_scale, false) {
        eprintln!("{}", format!("Error processing image: {}", e).red());
        std::process::exit(1);
    }

    println!("{}", format!("Image processed successfully: {}", output_file.display()).green());
}

fn process_directory(cli: &Cli) {
    let files: Vec<PathBuf> = collect_image_files(&cli.input_path);
    let total_files: usize = files.len();

    println!("{}", format!("Processing {} images found", total_files).blue());

    let pb: ProgressBar = ProgressBar::new(total_files as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
        .unwrap()
        .progress_chars("#>-"));

    files.par_iter().for_each(|file| {
        let file_stem: &str = file.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let extension: &str = if cli.pdf { "pdf" } else { file.extension().and_then(|s| s.to_str()).unwrap_or("jpeg") };
        let new_name: String = format!("{}_watermark.{}", file_stem, extension);
        let output_file: PathBuf = file.with_file_name(new_name);

        if let Err(e) = add_watermark(file, &cli.watermark, &output_file, &cli.compression, &cli.text_scale, &cli.space_scale, false) {
            error!("{}", format!("Error processing {}: {}", file.display(), e).red());
        } else {
            info!("{}", format!("Image processed successfully: {}", output_file.display()).green());
        }
        pb.inc(1);
    });

    pb.finish_with_message(format!("{}", "Processing completed!".green()));
}

fn collect_image_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path: PathBuf = entry.path();
            if path.is_dir() {
                files.extend(collect_image_files(&path));
            } else if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                if ["jpg", "jpeg", "png", "webp"].contains(&extension.to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }
    files
}

fn add_watermark(image_path: &Path, watermark_text: &str, output_path: &Path, compression: &u8, text_scale: &f32, space_scale: &f32, to_pdf: bool) -> Result<(), Box<dyn Error>> {
    let mut img: DynamicImage = image::open(image_path)?;
    let img_height: u32 = img.height();
    let img_width: u32 = img.width();

    if img_height == 0 || img_width == 0 {
        return Err("Image has invalid dimensions (width or height is 0)".into());
    }

    let font_data = include_bytes!("../assets/OpenSans-Regular.ttf");
    let font: FontRef = FontRef::try_from_slice(font_data).unwrap();

    let mut canva: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(img.width() * 2, img_height * 2);

    let scale: f32 = if img_height as f32 * text_scale <= 0.0 { 0.05 } else { img_height as f32 * text_scale };
    let space_y: f32 = if scale * space_scale <= 1.0 { 1.0 } else { scale * space_scale };

    let text_color = Rgba([128u8, 128u8, 128u8, 150u8]);

    let mut long_watermark: String = String::from(watermark_text);
    long_watermark.push_str("\t");
    long_watermark = long_watermark.repeat(canva.width() as usize / long_watermark.len());

    for i in 0..(canva.height() / space_y as u32) {
        draw_text_mut(
            &mut canva,
            text_color,
            0,
            (i * space_y as u32) as i32,
            scale,
            &font,
            &long_watermark,
        );
    }

    canva = rotate(&canva, ((canva.width() / 2) as f32, (canva.height() / 2) as f32), -45.0_f32.to_radians(), Interpolation::Nearest, Rgba([0, 0, 0, 0]));

    overlay(&mut img, &canva, -((img_width / 2) as i64), -((img_height / 2) as i64));

    if to_pdf {
        fs::write(output_path, make_pdf(img))?;
    } else {
        let mut writer: BufWriter<File> = BufWriter::new(File::create(output_path)?);
        match image_path.extension().and_then(|e| e.to_str()) {
            Some("jpg") | Some("jpeg") => img.write_with_encoder(JpegEncoder::new_with_quality(&mut writer, *compression))?,
            Some("png") => img.write_with_encoder(PngEncoder::new(&mut writer))?,
            Some("webp") => img.write_with_encoder(WebPEncoder::new_lossless(&mut writer))?,
            _ => img.write_with_encoder(JpegEncoder::new_with_quality(&mut writer, *compression))?,
        };
    }
    Ok(())
}

fn make_pdf(image: DynamicImage) -> Vec<u8> {
    let mut doc: PdfDocument = PdfDocument::new("My first PDF");
    let mut warnings: Vec<printpdf::PdfWarnMsg> = Vec::new();

    let raw_image: RawImage = RawImage::decode_from_bytes(image.as_bytes(), &mut warnings)
        .expect("Failed to decode image");
    let image_xobject_id: XObjectId = doc.add_image(&raw_image);

    let page1_contents = vec![
        Op::UseXobject {
            id: image_xobject_id.clone(),
            transform: XObjectTransform::default(),
        },
    ];

    let page1: PdfPage = PdfPage::new(Mm(210.0), Mm(297.0), page1_contents);
    doc.with_pages(vec![page1]).save(&PdfSaveOptions::default(), &mut warnings)
}