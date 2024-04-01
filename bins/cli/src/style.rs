use once_cell::sync::Lazy;

#[cfg(feature = "pacman")]
pub static PACMAN_STYLE: Lazy<console::Style> = Lazy::new(|| console::Style::new().blue());
#[cfg(feature = "aur")]
pub static AUR_STYLE: Lazy<console::Style> = Lazy::new(|| console::Style::new().red());
#[cfg(feature = "flatpak")]
pub static FLATPAK_STYLE: Lazy<console::Style> = Lazy::new(|| console::Style::new().green());
#[cfg(feature = "snap")]
pub static SNAP_STYLE: Lazy<console::Style> = Lazy::new(|| console::Style::new().yellow());
#[cfg(feature = "cargo")]
pub static CARGO_STYLE: Lazy<console::Style> = Lazy::new(|| console::Style::new().red());
