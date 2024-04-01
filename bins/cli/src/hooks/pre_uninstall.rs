use unipac_managers::managers::*;

#[cfg(feature = "pacman")]
pub async fn pacman_pre_uninstall(_package: &pacman::Package) {}

#[cfg(feature = "aur")]
pub async fn aur_pre_uninstall(_package: &aur::Package) {}

#[cfg(feature = "flatpak")]
pub async fn flatpak_pre_uninstall(_package: &flatpak::Package) {}

#[cfg(feature = "snap")]
pub async fn snap_pre_uninstall(_package: &snap::Package) {}

#[cfg(feature = "cargo")]
pub async fn cargo_pre_uninstall(_package: &cargo::Package) {}
