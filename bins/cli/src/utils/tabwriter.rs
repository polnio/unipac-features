use crate::style::*;
use unipac_managers::managers::*;

#[cfg(feature = "pacman")]
pub fn pacman_to_string(package: &pacman::Package) -> String {
    format!(
        "{}\t{}\t{}\t{}\n",
        PACMAN_STYLE.apply_to("Pacman:"),
        package.database,
        package.name,
        package.version
    )
}

#[cfg(feature = "aur")]
pub fn aur_to_string(package: &aur::Package) -> String {
    format!(
        "{}\t{}\t{}\n",
        AUR_STYLE.apply_to("AUR:"),
        package.name,
        package.version,
    )
}

#[cfg(feature = "flatpak")]
pub fn flatpak_to_string(package: &flatpak::Package) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        FLATPAK_STYLE.apply_to("Flatpak:"),
        package.id,
        package.name,
        package.version,
        package.description
    )
}

#[cfg(feature = "snap")]
pub fn snap_to_string(package: &snap::Package) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        SNAP_STYLE.apply_to("Snap:"),
        package.name,
        package.version,
        package.publisher,
        package.description
    )
}

#[cfg(feature = "cargo")]
pub fn cargo_to_string(package: &cargo::Package) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        CARGO_STYLE.apply_to("Cargo:"),
        package.name,
        package.version,
        package
            .repository
            .as_ref()
            .map(|url| url.to_string())
            .unwrap_or_default(),
        package.bins.join(", "),
    )
}
