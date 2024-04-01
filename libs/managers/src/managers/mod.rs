#![allow(async_fn_in_trait)]

#[cfg(feature = "aur")]
pub mod aur;
#[cfg(feature = "cargo")]
pub mod cargo;
#[cfg(feature = "flatpak")]
pub mod flatpak;
#[cfg(feature = "pacman")]
pub mod pacman;
#[cfg(feature = "snap")]
pub mod snap;

#[cfg(feature = "aur")]
pub use aur::AUR;
#[cfg(feature = "cargo")]
pub use cargo::Cargo;
#[cfg(feature = "flatpak")]
pub use flatpak::Flatpak;
#[cfg(feature = "pacman")]
pub use pacman::Pacman;
#[cfg(feature = "snap")]
pub use snap::Snap;

pub trait Manager {
    type Package;
    type Error;

    async fn list(&self) -> Result<Vec<Self::Package>, Self::Error>;
    async fn find(&self, name: &str) -> Result<Option<Self::Package>, Self::Error>;
    async fn search(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error>;
    async fn search_install(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error>;
    async fn install(&self, package: &Self::Package) -> Result<(), Self::Error>;
    async fn uninstall(&self, package: &Self::Package) -> Result<(), Self::Error>;
    async fn list_updates(&self) -> Result<Vec<Self::Package>, Self::Error>;
    async fn count_updates(&self) -> Result<usize, Self::Error>;
    async fn update(&self) -> Result<(), Self::Error>;
}
