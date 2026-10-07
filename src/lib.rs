pub mod backend;
pub mod engine;
pub mod interval;
pub mod keystate;
pub mod neighbourhood;
pub mod notename;
pub mod reference;
pub mod strategy;
pub mod util;

#[cfg(not(target_arch = "wasm32"))]
pub mod bindable;

#[cfg(not(target_arch = "wasm32"))]
pub mod config;

#[cfg(not(target_arch = "wasm32"))]
pub mod custom_serde;

#[cfg(not(target_arch = "wasm32"))]
pub mod maybeconnected;

#[cfg(not(target_arch = "wasm32"))]
pub mod msg;

#[cfg(not(target_arch = "wasm32"))]
pub mod process;

#[cfg(not(target_arch = "wasm32"))]
pub mod gui;

#[cfg(not(target_arch = "wasm32"))]
pub mod run;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
