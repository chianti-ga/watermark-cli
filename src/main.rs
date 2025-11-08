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
mod pdf;
mod gpu;

use crate::cli::{ Cli, Pattern };
use crate::gpu::try_gpu_blend;
use crate::pdf::convert_to_image;

use ab_glyph::{ FontRef, PxScale };
use clap::Parser;
use colored::Colorize;
use image::{ DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage };
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::overlay as overlay_cpu;
use image::ImageEncoder;
use imageproc::drawing::draw_text_mut;
use imageproc::geometric_transformations::{ rotate_about_center, Interpolation };
use indicatif::{ ProgressBar, ProgressStyle };
use log::{ error, info };
use rayon::prelude::*;
use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::{ BufWriter, Write };
use std::path::{ Path, PathBuf };
use std::time::Instant;

/// When GPU mode is "auto", below this pixel count the CPU is faster.
const AUTO_GPU_MIN_PIXELS: u64 = 1_000_000;

fn main() {
    let cli = Cli::parse();

    // ---------- Thread policy ----------
    // If user sets --threads, honor it. Otherwise:
    //  - if --gpu on, default to 1 (let the GPU be the bottleneck)
    //  - else, let Rayon default to available cores.
    let default_threads = if cli.gpu.eq_ignore_ascii_case("on") {
        1
    } else {
        std::thread
            ::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    };
    let threads = cli.threads.unwrap_or(default_threads).max(1);
    // Build global pool once; ignore error if already built.
    let _ = rayon::ThreadPoolBuilder::new().num_threads(threads).build_global();

    let start_time = Instant::now();

    let out_dir = cli.output_path.as_deref();

    if cli.input_path.is_dir() {
        process_directory(&cli, out_dir);
    } else {
        process_single_file(&cli, out_dir);
    }

    let duration = start_time.elapsed();
    println!(
        "{}",
        format!("Processing completed in {:.2} seconds", duration.as_secs_f32()).green()
    );
}

fn process_single_file(cli: &Cli, output_dir: Option<&Path>) {
    let input_file = &cli.input_path;

    let file_stem = input_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let extension = input_file
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("jpeg");
    let new_name = format!("{}_watermark.{}", file_stem, extension);

    let output_file = if let Some(dir) = output_dir {
        fs::create_dir_all(dir).ok();
        dir.join(new_name)
    } else {
        input_file.with_file_name(new_name)
    };

    println!("{}", format!("Processing: {}", input_file.display()).blue());

    if
        input_file
            .extension()
            .and_then(|s| s.to_str())
            .map(|e| e.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false)
    {
        process_pdf(cli);
        return;
    }

    if
        let Err(e) = add_watermark(
            input_file,
            &cli.watermark,
            &output_file,
            cli.compression,
            cli.text_scale,
            cli.space_scale,
            &cli.color,
            cli.opacity,
            &cli.pattern,
            &cli.gpu
        )
    {
        eprintln!("{}", format!("Error: {}", e).red());
        // cleanup partials
        let _ = fs::remove_file(&output_file);
        let _ = fs::remove_file(
            output_file.with_extension(
                output_file
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|e| format!("{}.part", e))
                    .unwrap_or_else(|| "part".into())
            )
        );
        std::process::exit(1);
    }

    println!("{}", format!("Image processed successfully: {}", output_file.display()).green());
}

fn process_pdf(cli: &Cli) {
    let input_file = &cli.input_path;
    let file_stem = input_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let temp_dir = std::env
        ::temp_dir()
        .join("watermark-cli")
        .join(format!("{}_pdf_pages", file_stem));
    fs::create_dir_all(&temp_dir).unwrap();

    println!("{}", format!("Rendering PDF: {}", input_file.display()).blue());
    convert_to_image(input_file, &temp_dir);

    let mut output_dir = cli.input_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    output_dir.push(file_stem);
    fs::create_dir_all(&output_dir).unwrap();

    let batch_cli = Cli {
        input_path: temp_dir.clone(),
        watermark: cli.watermark.clone(),
        compression: cli.compression,
        space_scale: cli.space_scale,
        text_scale: cli.text_scale,
        recursive: true,
        pattern: cli.pattern.clone(),
        output_path: Some(output_dir.clone()),
        color: cli.color.clone(),
        opacity: cli.opacity,
        gpu: cli.gpu.clone(),
        threads: cli.threads,
    };

    process_directory(&batch_cli, Some(output_dir.as_path()));
    fs::remove_dir_all(&temp_dir).ok();
}

fn process_directory(cli: &Cli, output_dir: Option<&Path>) {
    let files = collect_image_files(&cli.input_path, cli.recursive);
    let total_files = files.len();

    println!("{}", format!("Queued {} file(s)", total_files).blue());
    if let Some(dir) = output_dir {
        fs::create_dir_all(dir).ok();
    }

    let pb = ProgressBar::new(total_files as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
            .unwrap()
            .progress_chars("=> ")
    );

    files.par_iter().for_each(|file| {
        let file_stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let extension = file
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("jpeg");
        let new_name = format!("{}_watermark.{}", file_stem, extension);
        let output_file = if let Some(dir) = output_dir {
            dir.join(new_name)
        } else {
            file.with_file_name(new_name)
        };

        if
            let Err(e) = add_watermark(
                file,
                &cli.watermark,
                &output_file,
                cli.compression,
                cli.text_scale,
                cli.space_scale,
                &cli.color,
                cli.opacity,
                &cli.pattern,
                &cli.gpu
            )
        {
            error!("{}", format!("Error processing {}: {}", file.display(), e).red());
            let _ = fs::remove_file(&output_file);
            let _ = fs::remove_file(
                output_file.with_extension(
                    output_file
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|e| format!("{}.part", e))
                        .unwrap_or_else(|| "part".into())
                )
            );
        } else {
            info!("{}", format!("OK: {}", output_file.display()).green());
        }
        pb.inc(1);
    });

    pb.finish_with_message(format!("{}", "Processing completed!".green()));
}

fn collect_image_files(path: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if path.is_file() {
        files.push(path.to_path_buf());
        return files;
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && recursive {
                files.extend(collect_image_files(&p, recursive));
            } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if
                    ["jpg", "jpeg", "png", "webp", "bmp", "tiff", "gif"].contains(
                        &ext.to_lowercase().as_str()
                    )
                {
                    files.push(p);
                }
            }
        }
    }
    files
}

fn parse_color(hex: &str, alpha: u8) -> Result<Rgba<u8>, Box<dyn std::error::Error>> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err("Color must be a 6-digit hex like FF0000".into());
    }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(Rgba([r, g, b, alpha]))
}

#[allow(clippy::too_many_arguments)]
fn add_watermark(
    image_path: &Path,
    watermark_text: &str,
    output_path: &Path,
    compression: u8,
    text_scale: f32,
    space_scale: f32,
    color_hex: &str,
    opacity: u8,
    pattern: &Pattern,
    gpu_mode: &str
) -> Result<(), Box<dyn Error>> {
    let img = image::open(image_path)?;
    let (img_w, img_h) = img.dimensions();
    if img_w == 0 || img_h == 0 {
        return Err("Image has invalid dimensions".into());
    }

    // ---------- Build watermark overlay (memory-lean) ----------
    let diag_side = ((img_w as f64).hypot(img_h as f64).ceil() as u32).max(img_w).max(img_h);
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(diag_side, diag_side, Rgba([0, 0, 0, 0]));

    // Font & sizing
    let font_data = include_bytes!("../assets/OpenSans-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|_| "Invalid TTF font data")?;
    let px_scale = {
        let s = ((img_h as f32) * text_scale).max(1.0);
        PxScale { x: s, y: s }
    };
    let color = parse_color(color_hex, opacity)?;
    let v_gap = (px_scale.y * space_scale).max(1.0) as i32;

    // Long line using spaces so it spans after rotation
    let unit = format!("{}   ", watermark_text);
    let mut long_text = String::with_capacity(4096);
    for _ in 0..128 {
        long_text.push_str(&unit);
    }

    // RNG for Random pattern
    fn rand_unit() -> f32 {
        use std::cell::RefCell;
        thread_local! {
            static SEED: RefCell<u32> = RefCell::new(0x12345678);
        }
        SEED.with(|s| {
            let mut x = *s.borrow();
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *s.borrow_mut() = x;
            ((x as f32) / (u32::MAX as f32)).clamp(0.0, 1.0)
        })
    }

    match pattern {
        Pattern::Diagonal | Pattern::CrossDiagonal => {
            let mut y = 0i32;
            while y < (canvas.height() as i32) {
                draw_text_mut(&mut canvas, color, 0, y, px_scale, &font, &long_text);
                y += v_gap;
            }
            let rotated: RgbaImage = rotate_about_center(
                &canvas,
                -(45f32).to_radians(),
                Interpolation::Nearest,
                Rgba([0, 0, 0, 0])
            );
            canvas = rotated;
            if matches!(pattern, Pattern::CrossDiagonal) {
                let rotated2: RgbaImage = rotate_about_center(
                    &canvas,
                    (90f32).to_radians(),
                    Interpolation::Nearest,
                    Rgba([0, 0, 0, 0])
                );
                canvas = rotated2;
            }
        }
        Pattern::Horizontal => {
            let mut y = 0i32;
            while y < (canvas.height() as i32) {
                draw_text_mut(&mut canvas, color, 0, y, px_scale, &font, &long_text);
                y += v_gap;
            }
        }
        Pattern::Vertical => {
            let mut temp: RgbaImage = ImageBuffer::from_pixel(
                canvas.width(),
                canvas.height(),
                Rgba([0, 0, 0, 0])
            );
            let mut y = 0i32;
            while y < (temp.height() as i32) {
                draw_text_mut(&mut temp, color, 0, y, px_scale, &font, &long_text);
                y += v_gap;
            }
            let rotated: RgbaImage = rotate_about_center(
                &temp,
                (90f32).to_radians(),
                Interpolation::Nearest,
                Rgba([0, 0, 0, 0])
            );
            canvas = rotated;
        }
        Pattern::Random => {
            // Multi-pass random angles and row phases for a truly stochastic look.
            let passes = 3;
            let mut composed: RgbaImage = ImageBuffer::from_pixel(
                canvas.width(),
                canvas.height(),
                Rgba([0, 0, 0, 0])
            );
            for _ in 0..passes {
                let angle_deg = -60.0 + 120.0 * rand_unit();
                let mut temp: RgbaImage = ImageBuffer::from_pixel(
                    canvas.width(),
                    canvas.height(),
                    Rgba([0, 0, 0, 0])
                );
                let mut y = ((v_gap as f32) * rand_unit()) as i32;
                while y < (temp.height() as i32) {
                    let x_off = (px_scale.x * 10.0 * rand_unit()) as i32;
                    draw_text_mut(&mut temp, color, x_off.max(0), y, px_scale, &font, &long_text);
                    let jitter = ((v_gap as f32) * (0.8 + 0.6 * rand_unit())) as i32;
                    y += jitter.max(1);
                }
                let rotated: RgbaImage = rotate_about_center(
                    &temp,
                    angle_deg.to_radians(),
                    Interpolation::Nearest,
                    Rgba([0, 0, 0, 0])
                );
                for (x, y, p) in rotated.enumerate_pixels() {
                    if p.0[3] > 0 {
                        composed.put_pixel(x, y, *p);
                    }
                }
            }
            canvas = composed;
        }
    }

    // Crop centered to the original image size
    let x_off = (canvas.width() as i64) / 2 - (img_w as i64) / 2;
    let y_off = (canvas.height() as i64) / 2 - (img_h as i64) / 2;
    let mut overlay = RgbaImage::new(img_w, img_h);
    for y in 0..img_h {
        let sy = (y_off + (y as i64)) as u32;
        if sy >= canvas.height() {
            continue;
        }
        for x in 0..img_w {
            let sx = (x_off + (x as i64)) as u32;
            if sx >= canvas.width() {
                continue;
            }
            overlay.put_pixel(x, y, *canvas.get_pixel(sx, sy));
        }
    }

    // ---------- Blend selection (GPU prioritized) ----------
    let total_pixels = (img_w as u64) * (img_h as u64);
    let allow_gpu = match gpu_mode {
        s if s.eq_ignore_ascii_case("on") => true, // force GPU unless it hard-fails
        s if s.eq_ignore_ascii_case("off") => false,
        _ => total_pixels >= AUTO_GPU_MIN_PIXELS, // auto heuristic
    };

    let result_img: DynamicImage = if allow_gpu {
        match try_gpu_blend(&img, &DynamicImage::ImageRgba8(overlay.clone())) {
            Ok(Some(gpu_out)) => gpu_out,
            _ => {
                // fallback on hard failure only
                let mut tmp = img.to_rgba8();
                overlay_cpu(&mut tmp, &overlay, 0, 0);
                DynamicImage::ImageRgba8(tmp)
            }
        }
    } else {
        let mut tmp = img.to_rgba8();
        overlay_cpu(&mut tmp, &overlay, 0, 0);
        DynamicImage::ImageRgba8(tmp)
    };

    // ---------- Atomic encode ----------
    write_image_atomic(
        output_path,
        &result_img,
        image_path.extension().and_then(|e| e.to_str()),
        compression
    )?;
    Ok(())
}

// Atomic writer: write to .part then rename
fn write_image_atomic(
    output_path: &Path,
    img: &DynamicImage,
    ext_opt: Option<&str>,
    compression: u8
) -> Result<(), Box<dyn Error>> {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp_path = output_path.with_extension(
        output_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|e| format!("{}.part", e))
            .unwrap_or_else(|| "part".into())
    );

    struct TempGuard {
        path: PathBuf,
        keep: bool,
    }
    impl Drop for TempGuard {
        fn drop(&mut self) {
            if !self.keep {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
    let mut guard = TempGuard {
        path: tmp_path.clone(),
        keep: false,
    };

    let f = OpenOptions::new().create(true).write(true).truncate(true).open(&tmp_path)?;
    let mut writer = BufWriter::new(f);

    let (w, h) = img.dimensions();
    match ext_opt.map(|s| s.to_ascii_lowercase()) {
        Some(ref ext) if ext == "jpg" || ext == "jpeg" => {
            let buf = img.to_rgb8();
            let enc = JpegEncoder::new_with_quality(&mut writer, compression);
            enc.write_image(&buf, w, h, image::ExtendedColorType::Rgb8)?;
        }
        Some(ref ext) if ext == "png" => {
            let buf = img.to_rgba8();
            let enc = PngEncoder::new(&mut writer);
            enc.write_image(&buf, w, h, image::ExtendedColorType::Rgba8)?;
        }
        Some(ref ext) if ext == "webp" => {
            let buf = img.to_rgba8();
            let enc = WebPEncoder::new_lossless(&mut writer);
            enc.write_image(&buf, w, h, image::ExtendedColorType::Rgba8)?;
        }
        _ => {
            let buf = img.to_rgba8();
            let enc = PngEncoder::new(&mut writer);
            enc.write_image(&buf, w, h, image::ExtendedColorType::Rgba8)?;
        }
    }

    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);

    fs::rename(&tmp_path, output_path)?;
    guard.keep = true;
    Ok(())
}