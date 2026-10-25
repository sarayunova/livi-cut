//! The LiVi Cut wordmark, drawn as text until LiVi's logo artwork is added.

/// The product name the wordmark shows.
pub const NAME: &str = "LiVi Cut";

/// Draw the wordmark `height` points tall with its left edge at `left_center` (vertically centred).
/// Returns the drawn rect.
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, dark: bool) -> Option<egui::Rect> {
    let ink = if dark { egui::Color32::WHITE } else { egui::Color32::from_rgb(0x1a, 0x1a, 0x1a) };
    // Cap height is roughly 0.7 of the font size; size the text so its capitals fill `height`.
    let font = crate::theme::Tokens::semibold(height / 0.7);
    let r = ui.painter().text(left_center, egui::Align2::LEFT_CENTER, NAME, font, ink);
    Some(r)
}
