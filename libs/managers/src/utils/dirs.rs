use once_cell::sync::Lazy;
use std::path::PathBuf;
use xdg::BaseDirectories;

pub static DIRS: Lazy<BaseDirectories> =
    Lazy::new(|| BaseDirectories::with_prefix("unipac").expect("Failed to find xdg directories"));

#[cfg(feature = "aur")]
pub static REQUESTER: Lazy<reqwest::Client> = Lazy::new(|| reqwest::Client::new());

#[cfg(feature = "aur")]
pub fn get_aur_extracted_path<N>(name: N) -> std::io::Result<PathBuf>
where
    N: AsRef<str>,
{
    DIRS.create_cache_directory("aur")
        .map(|path| path.join(name.as_ref()))
}

#[cfg(feature = "aur")]
pub async fn download_and_extract_aur_archive<N>(name: N) -> Result<(), Box<dyn std::error::Error>>
where
    N: AsRef<str>,
{
    let extracted_dir = DIRS.create_cache_directory("aur")?;
    let mut response = REQUESTER
        .get(format!(
            "https://aur.archlinux.org/cgit/aur.git/snapshot/{}.tar.gz",
            name.as_ref()
        ))
        .send()
        .await?;
    let mut tar_gz = Vec::new();
    while let Some(chunk) = response.chunk().await.unwrap_or(None) {
        tar_gz.extend_from_slice(&chunk);
    }
    let tar = flate2::read::GzDecoder::new(std::io::Cursor::new(tar_gz));
    let mut archive = tar::Archive::new(tar);
    archive.unpack(extracted_dir)?;
    Ok(())
}

pub fn get_pkgbuild_path(name: &str) -> PathBuf {
    DIRS.get_cache_file(format!("aur/{}/PKGBUILD", name))
}
