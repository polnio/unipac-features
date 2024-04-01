pub const MANAGERS_COUNT: usize = {
    let mut count = 0;
    if cfg!(feature = "pacman") {
        count += 1;
    }
    if cfg!(feature = "aur") {
        count += 1;
    }
    if cfg!(feature = "flatpak") {
        count += 1;
    }
    if cfg!(feature = "snap") {
        count += 1;
    }
    if cfg!(feature = "git") {
        count += 1;
    }
    if cfg!(feature = "cargo") {
        count += 1;
    }
    count
};

pub const MANAGERS: [&str; MANAGERS_COUNT] = [
    #[cfg(feature = "pacman")]
    "pacman",
    #[cfg(feature = "aur")]
    "aur",
    #[cfg(feature = "flatpak")]
    "flatpak",
    #[cfg(feature = "snap")]
    "snap",
    #[cfg(feature = "git")]
    "git",
    #[cfg(feature = "cargo")]
    "cargo",
];

pub const PACKAGES: [&str; MANAGERS_COUNT] = [
    #[cfg(feature = "pacman")]
    "Pacman",
    #[cfg(feature = "aur")]
    "AUR",
    #[cfg(feature = "flatpak")]
    "Flatpak",
    #[cfg(feature = "snap")]
    "Snap",
    #[cfg(feature = "git")]
    "Git",
    #[cfg(feature = "cargo")]
    "Cargo",
];
