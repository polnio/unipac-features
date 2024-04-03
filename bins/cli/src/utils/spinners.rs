use crate::args::Managers;
use crate::style::*;
// use crate::ARGS;
use indicatif::{MultiProgress, ProgressBar};
use unipac_macros::{for_all, for_all_attrs};

#[for_all_attrs]
#[derive(Clone)]
pub struct Spinners {
    // mp: MultiProgress,
    pub __manager: Option<ProgressBar>,
}

#[for_all_attrs]
pub struct Receivers {
    pub __manager: tokio::sync::broadcast::Receiver<u8>,
}

impl Spinners {
    /* pub fn new() -> Self {
        Self::with_managers(&ARGS.managers)
    } */
    pub fn with_managers(managers: &Managers) -> Self {
        let mp = MultiProgress::new();
        for_all! {
            let __manager_spinner = if managers.__manager {
                Some(mp.add(ProgressBar::new_spinner()))
            } else {
                None
            };
        }
        Self {
            // mp,
            #[cfg(feature = "pacman")]
            pacman: pacman_spinner,
            #[cfg(feature = "aur")]
            aur: aur_spinner,
            #[cfg(feature = "flatpak")]
            flatpak: flatpak_spinner,
            #[cfg(feature = "snap")]
            snap: snap_spinner,
            #[cfg(feature = "cargo")]
            cargo: cargo_spinner,
        }
    }

    #[for_all_attrs]
    pub fn __manager_set_message<M>(&self, message: M)
    where
        M: std::fmt::Display,
    {
        if let Some(spinner) = &self.__manager {
            spinner.set_message(
                __MANAGER_STYLE
                    .apply_to(format!("__Manager: {}", message))
                    .to_string(),
            );
        }
    }

    #[for_all_attrs]
    pub fn __manager_abort(&self) {
        self.__manager_finish_with_message("x")
    }

    #[for_all_attrs]
    pub fn __manager_finish(&self) {
        self.__manager_finish_with_message("")
    }

    #[for_all_attrs]
    pub fn __manager_finish_with_message<M>(&self, message: M)
    where
        M: std::fmt::Display,
    {
        if let Some(spinner) = &self.__manager {
            spinner.finish_with_message(
                __MANAGER_STYLE
                    .apply_to(format!("__Manager: {}", message))
                    .to_string(),
            );
        }
    }

    pub async fn work(&self) {
        let spinners = self.clone();
        let handle = tokio::spawn(async move {
            while !spinners.is_done() {
                for_all! {
                    if let Some(spinner) = &spinners.__manager {
                        if !spinner.is_finished() {
                            spinner.tick();
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        for_all! {
            if let Some(spinner) = &self.__manager {
                spinner.set_message(__MANAGER_STYLE.apply_to("__Manager...").to_string());
            }
        }
        handle.await.unwrap();
    }

    pub fn is_done(&self) -> bool {
        for_all! {
            if !self.__manager.as_ref().map_or(true, ProgressBar::is_finished) {
                return false;
            }
        }
        true
    }
}
