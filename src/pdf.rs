use hayro::{ render, InterpreterSettings, Pdf, RenderSettings };
use std::path::Path;
use std::sync::Arc;

/// Render each PDF page to PNGs in `output_dir/rendered_*.png`
pub fn convert_to_image(pdf_path: &Path, output_dir: &Path) {
    let file = std::fs::read(pdf_path).expect("Failed to read PDF");
    let data = Arc::new(file);
    let pdf = Pdf::new(data).expect("Failed to parse PDF");

    let interpreter_settings = InterpreterSettings::default();
    let render_settings = RenderSettings {
        x_scale: 2.0,
        y_scale: 2.0,
        ..Default::default()
    };

    std::fs::create_dir_all(output_dir).ok();

    for (idx, page) in pdf.pages().iter().enumerate() {
        let pixmap = render(page, &interpreter_settings, &render_settings);
        let output_path = output_dir.join(format!("rendered_{idx}.png"));
        std::fs::write(output_path, pixmap.take_png()).expect("Failed to write PNG");
    }
}