use dialoguer::Confirm;
use unipac_managers::managers::*;
use unipac_managers::utils::dirs::{download_and_extract_aur_archive, get_pkgbuild_path};

#[cfg(feature = "pacman")]
pub async fn pacman_pre_update(_packages: &Vec<pacman::Package>) {}

#[cfg(feature = "aur")]
pub async fn aur_pre_update(packages: &Vec<aur::Package>) {
    if !packages.is_empty() {
        let handles = packages
            .iter()
            .map(|p| download_and_extract_aur_archive(&p.name));
        for handle in handles {
            let result = handle.await;
            // .expect("Failed to download and extract package");
            if let Err(err) = result {
                eprintln!("Failed to download and extract package: {}", err);
                std::process::exit(1);
            }
        }
        for package in packages {
            let Ok(might_show_pkgbuild) = Confirm::new()
                .with_prompt(format!(
                    "Do you want to show the PKGBUILD for {}?",
                    package.name
                ))
                .default(false)
                .interact()
            else {
                eprintln!("Failed to read input");
                std::process::exit(1);
            };
            if might_show_pkgbuild {
                let path = get_pkgbuild_path(&package.name);
                let result = std::process::Command::new("less")
                    .arg(path)
                    .spawn()
                    .and_then(|mut p| p.wait());
                if let Err(err) = result {
                    eprintln!("Failed to show PKGBUILD: {}", err);
                }
            }
        }
    }
}

#[cfg(feature = "flatpak")]
pub async fn flatpak_pre_update(_packages: &Vec<flatpak::Package>) {}

#[cfg(feature = "snap")]
pub async fn snap_pre_update(_packages: &Vec<snap::Package>) {}

#[cfg(feature = "cargo")]
pub async fn cargo_pre_update(_packages: &Vec<cargo::Package>) {}
