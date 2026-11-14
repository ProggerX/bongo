use serde::Deserialize;
use std::process::ExitCode;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

pub(crate) const STATE_REST: u8 = 0;
pub(crate) const STATE_LEFT: u8 = 1;
pub(crate) const STATE_RIGHT: u8 = 2;

#[derive(Debug, Deserialize, Clone)]
struct Imgs {
    pub rest: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Input {
    pub device: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct Config {
    pub imgs: Option<Imgs>,
    pub input: Option<Input>,
    pub window: Option<Window>,
}

#[derive(Debug, Deserialize, Clone)]
struct Window {
    pub position: Option<String>,
    pub size: Option<u32>,
}

impl Config {
    fn image_size(&self, width: u32, height: u32) -> (u32, u32) {
        let size = self
            .window
            .as_ref()
            .and_then(|window| window.size)
            .unwrap_or(64)
            .clamp(1, 2048);
        let longest = u64::from(width.max(height).max(1));
        let scaled = |dimension| {
            ((u64::from(dimension) * u64::from(size) + longest / 2) / longest).max(1) as u32
        };
        (scaled(width), scaled(height))
    }
}

fn expand_path(path_option: Option<String>) -> Option<String> {
    let path = path_option?;
    if path.starts_with('~')
        && let Ok(home) = std::env::var("HOME")
    {
        return Some(path.replacen('~', &home, 1));
    }
    Some(path)
}

fn load_config() -> Config {
    let path = expand_path(Some("~/.config/bongo.toml".to_owned())).unwrap();
    match std::fs::read_to_string(path) {
        Ok(data) => toml::from_str(&data).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

fn main() -> ExitCode {
    let config = load_config();
    #[cfg(target_os = "linux")]
    match linux::run(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Failed to start Bongo: {error}");
            ExitCode::FAILURE
        }
    }
    #[cfg(target_os = "macos")]
    ExitCode::from(u8::from(macos::run(config)))
}
