pub mod r#static;
pub mod static_config;

#[cfg(not(target_arch = "wasm32"))]
pub mod r#trait;

#[cfg(not(target_arch = "wasm32"))]
pub mod twostep;
