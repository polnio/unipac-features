use unipac_managers::managers::*;

#[cfg(feature = "pacman")]
pub async fn pacman_pre_install(_package: &pacman::Package) {}

#[cfg(feature = "aur")]
pub async fn aur_pre_install(package: &aur::Package) {
    use dialoguer::Confirm;
    use unipac_managers::utils::dirs::{download_and_extract_aur_archive, get_pkgbuild_path};

    let result = download_and_extract_aur_archive(&package.name).await;
    if let Err(err) = result {
        eprintln!(
            "Failed to download and extract package {}: {}",
            package.name, err
        );
        std::process::exit(1);
    }
    let might_show_pkgbuild = Confirm::new()
        .with_prompt(format!(
            "Do you want to show/edit the PKGBUILD for {}?",
            package.name
        ))
        .default(false)
        .interact();
    let might_show_pkgbuild = match might_show_pkgbuild {
        Ok(might_show_pkgbuild) => might_show_pkgbuild,
        Err(err) => {
            eprintln!("Failed to read input: {}", err);
            std::process::exit(1);
        }
    };
    if !might_show_pkgbuild {
        return;
    }
    let path = get_pkgbuild_path(&package.name);
    let editor = std::env::var("EDITOR").unwrap_or("less".into());
    let result = std::process::Command::new(editor)
        .arg(path)
        .spawn()
        .and_then(|mut p| p.wait());
    if let Err(err) = result {
        eprintln!("Failed to open PKGBUILD: {}", err);
    }
    let might_install = Confirm::new()
        .with_prompt(format!("Do you want to install {}?", package.name))
        .default(true)
        .interact();
    let might_install = match might_install {
        Ok(might_install) => might_install,
        Err(err) => {
            eprintln!("Failed to read input: {}", err);
            std::process::exit(1);
        }
    };
    if !might_install {
        std::process::exit(0);
    }
}

#[cfg(feature = "flatpak")]
pub async fn flatpak_pre_install(_package: &flatpak::Package) {}

#[cfg(feature = "snap")]
pub async fn snap_pre_install(_package: &snap::Package) {}

#[cfg(feature = "cargo")]
pub async fn cargo_pre_install(_package: &cargo::Package) {}
