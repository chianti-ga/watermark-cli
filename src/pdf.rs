use hayro::{render, InterpreterSettings, Pdf, RenderSettings};
use std::path::Path;
use std::sync::Arc;

pub fn convert_to_image(pdf_path: &Path, output_dir: &Path) {
    let file = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();

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
        std::fs::write(output_path, pixmap.take_png()).unwrap();
    }
}