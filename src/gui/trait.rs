use std::sync::mpsc;

#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

use crate::{interval::stacktype::r#trait::StackType, msg::FromUi};

pub trait GuiShow<T: StackType> {
    fn show(&mut self, ui: &mut egui::Ui, forward: &mpsc::Sender<FromUi<T>>);
}
