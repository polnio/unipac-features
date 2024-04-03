use crate::args::Managers;
use crate::hooks::*;
use crate::style::*;
use crate::utils::sudo::elevate;
use crate::utils::tabwriter::*;
use crate::{args::ARGS, utils::spinners::Spinners};
use dialoguer::{Confirm, Select};
use std::fmt::Display;
use std::io::Write as _;
use std::sync::Arc;
use tabwriter::TabWriter;
use unipac_macros::{for_all, for_all_attrs};
use unipac_managers::managers::{self, Manager};

#[for_all_attrs]
#[derive(Default)]
struct Packages {
    pub __manager: Vec<managers::__manager::Package>,
}
impl Packages {
    fn total(&self) -> usize {
        let mut total = 0;
        for_all! {
            total += self.__manager.len();
        }
        total
    }
    fn to_msg<T>(n: &Vec<T>) -> String {
        n.len().to_string()
    }
}

#[for_all_attrs]
#[derive(Default)]
struct Package {
    pub __manager: Option<managers::__manager::Package>,
}
impl Package {
    /* fn total(&self) -> usize {
        let mut total = 0;
        for_all! {
            if self.__manager.is_some() {
                total += 1
            }
        }
        total
    } */
    fn to_msg<T>(_n: &Option<T>) -> String {
        // n.len().to_string()
        String::default()
    }
}

#[for_all_attrs]
#[derive(Default)]
struct Counts {
    pub __manager: usize,
}
impl Counts {
    fn total(&self) -> usize {
        let mut total = 0;
        for_all! {
            total += self.__manager;
        }
        total
    }
    fn to_msg(n: &usize) -> String {
        n.to_string()
    }
}

#[for_all_attrs]
enum Error {
    __Manager(managers::__manager::Error),
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "pacman")]
            Self::Pacman(err) => write!(f, "Pacman: {}", err),
            #[cfg(feature = "aur")]
            Self::AUR(err) => write!(f, "AUR: {}", err),
            #[cfg(feature = "flatpak")]
            Self::Flatpak(err) => write!(f, "Flatpak: {}", err),
            #[cfg(feature = "snap")]
            Self::Snap(err) => write!(f, "Snap: {}", err),
            #[cfg(feature = "cargo")]
            Self::Cargo(err) => write!(f, "Cargo: {}", err),
        }
    }
}

fn get_spinners(managers: &Managers) -> Option<Spinners> {
    if !ARGS.no_interactive {
        Some(Spinners::with_managers(managers))
    } else {
        None
    }
}

macro_rules! execute {
    ([$($to_clone:expr,)*], $fn:ident, [$($arg:expr,)*]) => {
        execute!([$($to_clone,)*], $fn, [$($arg,)*], ARGS.managers)
    };
    ([$($to_clone:expr,)*], $fn:ident, [$($arg:expr,)*], $managers:expr) => {{
        let spinners = get_spinners(&$managers);
        let mut errors = Vec::new();
        for_all! {
            let (__manager_handle, __manager_receiver) = if $managers.__manager {
                let spinners = spinners.clone();
                $(let $to_clone = $to_clone.clone();)*
                let (progress_sender, progress_receiver) = tokio::sync::mpsc::channel(1);
                let handle = tokio::spawn(async move {
                    let manager = managers::__Manager::with_progress(progress_sender);
                    let packages_result = manager.$fn($($arg,)*).await;
                    if let Some(spinners) = spinners {
                        if packages_result.as_ref().is_ok() {
                            spinners.__manager_finish();
                        } else {
                            spinners.__manager_abort();
                        }
                    }
                    packages_result
                });
                (Some(handle), Some(progress_receiver))
            } else {
                (None, None)
            };
        }
        if let Some(spinners) = spinners {
            let local = tokio::task::LocalSet::new();
            for_all! {
                let spinners_clone = spinners.clone();
                if let Some(mut receiver) = __manager_receiver {
                    local.spawn_local(async move {
                        while let Some(progress) = receiver.recv().await {
                            spinners_clone.__manager_set_message(progress);
                        }
                    });
                }
            }
            tokio::join!(local, spinners.work());

            for_all! {
                if let Some(handle) = __manager_handle {
                    let result = handle.await.unwrap();
                    if let Err(err) = result {
                        errors.push(Error::__Manager(err));
                    }
                }
            }
        }
        errors
    }};
}

macro_rules! get_results {
    ([$($to_clone:expr,)*], $fn:ident, $result:ident, [$($arg:expr,)*]) => {{
        let spinners = get_spinners(&ARGS.managers);
        for_all! {
            let __manager_handle = if ARGS.managers.__manager {
                let spinners = spinners.clone();
                $(let $to_clone = $to_clone.clone();)*
                let result = tokio::spawn(async move {
                    let manager = managers::__Manager::new();
                    let packages_result = manager.$fn($($arg,)*).await;
                    if let Some(spinners) = &spinners {
                        if let Ok(result) = &packages_result {
                            spinners.__manager_finish_with_message($result::to_msg(result));
                        } else {
                            spinners.__manager_abort();
                        }
                    }
                    packages_result
                });
                Some(result)
            } else {
                None
            };
        }
        if let Some(spinners) = spinners {
            spinners.work().await;
        }
        let mut results = $result::default();
        for_all! {
            if let Some(handle) = __manager_handle {
                match handle.await.unwrap() {
                    Ok(result) => {
                        results.__manager = result;
                    }
                    Err(err) => {
                        eprintln!("__Manager: {}", err);
                    }
                }
            }
        }
        results
    }};
}

fn print_packages(packages: &Packages) {
    let mut tw = TabWriter::new(std::io::stdout());
    let mut output = String::new();
    for_all! {
        if !packages.__manager.is_empty() {
            let str = packages.__manager
                .iter()
                .map(__manager_to_string)
                .fold(String::new(), |acc, s| acc + &s);
            output.push_str(&format!("{}\n", str));
        }
    }
    println!("\n");
    write!(&mut tw, "{}", output).expect("failed to write output");
    tw.flush().expect("failed to flush output");
}

pub async fn list() {
    let packages = get_results!([], list, Packages, []);
    print_packages(&packages);
}

pub async fn search(query: &str) {
    let query: Arc<str> = Arc::from(query);
    let packages = get_results!([query,], search, Packages, [(&query),]);
    print_packages(&packages);
}

pub async fn list_updates() {
    let packages = get_results!([], list_updates, Packages, []);
    if packages.total() == 0 {
        println!("Aucune mise à jour disponible.");
        return;
    }
    print_packages(&packages);
}

pub async fn install(query: &str) {
    elevate();
    let query: Arc<str> = Arc::from(query);
    let packages = get_results!([query,], search_install, Packages, [&query,]);
    println!("\n");
    if packages.total() == 0 {
        println!("Aucun paquet trouvé.");
        return;
    }
    let mut options: Vec<String> = Vec::with_capacity(packages.total());
    #[cfg(feature = "pacman")]
    for package in &packages.pacman {
        options.push(format!(
            "{}: {} {} {}",
            PACMAN_STYLE.apply_to("Pacman"),
            package.database,
            package.name,
            package.version,
        ));
    }
    #[cfg(feature = "aur")]
    for package in &packages.aur {
        options.push(format!(
            "{}: {} {}",
            AUR_STYLE.apply_to("AUR"),
            package.name,
            package.version,
        ))
    }
    #[cfg(feature = "flatpak")]
    for package in &packages.flatpak {
        options.push(format!(
            "{}: {} {}",
            FLATPAK_STYLE.apply_to("Flatpak"),
            package.name,
            package.version,
        ))
    }
    #[cfg(feature = "snap")]
    for package in &packages.snap {
        options.push(format!(
            "{}: {} {}",
            SNAP_STYLE.apply_to("Snap"),
            package.name,
            package.version,
        ))
    }
    #[cfg(feature = "cargo")]
    for package in &packages.cargo {
        options.push(format!(
            "{}: {} {}",
            CARGO_STYLE.apply_to("Cargo"),
            package.name,
            package.version,
        ))
    }
    if options.is_empty() {
        println!("No packages found.");
        return;
    }
    let selection = Select::new()
        .with_prompt("Which package do you want to install?")
        .items(&options)
        .default(0)
        .interact()
        .expect("Failed to read input");

    let mut i = 0;
    for_all! {
        let len = packages.__manager.len();
        if len == 0 || selection - i > len {
            i += len;
        } else {
            let package = &packages.__manager[selection - i];
            __manager_pre_install(&package).await;
            let manager = managers::__Manager::new();
            let result = manager.install(package).await;
            if let Err(err) = result {
                eprintln!("Failed to install {}: {}", package.name, err);
            }
            return;
        }
    }
    // Just for the linter
    if i == 0 {}
    println!("No packages found.");
}

pub async fn uninstall(query: &str) {
    elevate();
    let query: Arc<str> = Arc::from(query);
    let packages = get_results!([query,], find, Package, [&query,]);
    for_all! {
        if let Some(package) = &packages.__manager {
            println!("{}", __manager_to_string(package));
            let might_uninstall = Confirm::new()
                .with_prompt("Do you want to uninstall this package?")
                .default(true)
                .interact();
            let might_uninstall = match might_uninstall {
                Ok(might_uninstall) => might_uninstall,
                Err(err) => {
                    eprintln!("Failed to read input: {}", err);
                    std::process::exit(1);
                }
            };
            if !might_uninstall {
                std::process::exit(0);
            }
            __manager_pre_uninstall(package).await;
            let manager = managers::__Manager::new();
            let result = manager.uninstall(package).await;
            if let Err(err) = result {
                eprintln!("Failed to uninstall {}: {}", package.name, err);
            }
            return;
        }
    }
    println!("No packages found.");
}

pub async fn count_updates() {
    let counts = get_results!([], count_updates, Counts, []);
    if ARGS.no_interactive {
        println!("{}", counts.total());
    } else {
        println!("Vous avez {} mises à jour.", counts.total());
    }
}

pub async fn update(_query: Option<&str>) {
    elevate();
    let packages = get_results!([], list_updates, Packages, []);
    if packages.total() == 0 {
        println!("Aucune mise à jour disponible.");
        return;
    }
    print_packages(&packages);
    for_all! {
        __manager_pre_update(&packages.__manager).await;
    }
    let might_install = Confirm::new()
        .with_prompt("Do you want to install these packages?")
        .default(true)
        .interact()
        .expect("Failed to read input");

    if !might_install {
        return;
    }

    let mut managers = Managers::default();
    for_all! {
        managers.__manager = !packages.__manager.is_empty();
    }

    let errors = execute!([], update, [], managers);
    for error in errors {
        println!("{error}");
    }
}
