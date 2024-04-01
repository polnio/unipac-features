use crate::utils::alpm::Alpm;
use crate::utils::dirs::{get_aur_extracted_path, DIRS};
use alpm_utils::DbListExt as _;
use raur::Raur;
use std::fmt::Display;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc::Sender;

pub struct Package {
    pub name: String,
    pub version: String,
}
impl From<raur::Package> for Package {
    fn from(package: raur::Package) -> Self {
        Self {
            name: package.name,
            version: package.version,
        }
    }
}
impl From<alpm::Package<'_>> for Package {
    fn from(package: alpm::Package) -> Self {
        Self {
            name: package.name().into(),
            version: package.version().to_string(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Alpm(Option<alpm::Error>),
    Raur(Option<raur::Error>),
    Fs(Option<std::io::Error>),
    Command(&'static str, Option<std::io::Error>),
}
impl From<raur::Error> for Error {
    fn from(error: raur::Error) -> Self {
        Self::Raur(Some(error))
    }
}
impl From<alpm::Error> for Error {
    fn from(error: alpm::Error) -> Self {
        Self::Alpm(Some(error))
    }
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Alpm(Some(err)) => write!(f, "Alpm: {}", err),
            Self::Alpm(None) => write!(f, "Alpm: Unknown error"),
            Self::Raur(Some(err)) => write!(f, "Raur: {}", err),
            Self::Raur(None) => write!(f, "Raur: Unknown error"),
            Self::Fs(Some(err)) => write!(f, "Fs: {}", err),
            Self::Fs(None) => write!(f, "Fs: Unknown error"),
            Self::Command(command, Some(err)) => write!(f, "Command \"{}\": {}", command, err),
            Self::Command(command, None) => write!(f, "Command \"{}\": Unknown error", command),
        }
    }
}

pub struct AUR {
    alpm: Arc<Alpm>,
    raur: raur::Handle,
    progress_sender: Option<Sender<u8>>,
}
impl AUR {
    pub fn new() -> Self {
        Self::create(None)
    }
    pub fn with_progress(progress_sender: Sender<u8>) -> Self {
        let this = Self::create(Some(progress_sender.clone()));
        tokio::task::spawn({
            let alpm = this.alpm.clone();
            async move {
                println!("Starting watching");
                while let Some(progress) = alpm.recv_progress().await {
                    let _ = progress_sender.send(progress).await;
                }
                println!("Stopped watching");
            }
        });
        this
    }

    fn create(progress_sender: Option<Sender<u8>>) -> Self {
        let alpm = Arc::new(Alpm::new());
        Self {
            alpm,
            raur: raur::Handle::new(),
            progress_sender,
        }
    }
}
impl super::Manager for AUR {
    type Package = Package;
    type Error = Error;

    async fn list(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let syncdbs = alpm.syncdbs();
        let localdb = alpm.localdb();
        let packages = localdb
            .pkgs()
            .iter()
            .filter(|pkg| !syncdbs.pkg(pkg.name()).is_ok())
            .map(Package::from)
            .collect();
        Ok(packages)
    }

    async fn find(&self, name: &str) -> Result<Option<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let localdb = alpm.localdb();
        let syncdbs = alpm.syncdbs();
        let package = localdb.pkg(name).map(|pkg| Package::from(pkg));
        let Ok(package) = package else {
            return Ok(None);
        };
        if syncdbs.pkg(package.name.as_str()).is_ok() {
            return Ok(None);
        }
        Ok(Some(package))
    }

    async fn search(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let pkgs = self.raur.search(query).await?;
        let packages = pkgs.into_iter().map(Package::from).collect();
        Ok(packages)
    }

    async fn search_install(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let pkgs = self.raur.search_by(query, raur::SearchBy::Name).await?;
        let pkg = pkgs
            .into_iter()
            .filter_map(|p| {
                let matches = p.name == query
                    || p.name == format!("{}-bin", query)
                    || p.name == format!("{}-git", query);
                matches.then(|| Package::from(p))
            })
            .collect();
        Ok(pkg)
    }

    async fn install(&self, package: &Self::Package) -> Result<(), Self::Error> {
        let version = &self
            .raur
            .info(&[package.name.as_str()])
            .await?
            .into_iter()
            .next()
            .ok_or(Error::Raur(None))?
            .version;

        let path = get_aur_extracted_path(&package.name).map_err(|err| Error::Fs(err.into()))?;
        println!("Extracted path: {}", path.display());
        Command::new("makepkg")
            .uid(1000)
            .current_dir(path.clone())
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|err| Error::Command("makepkg", err.into()))?;

        let mut alpm = self.alpm.lock();
        alpm.trans_init(alpm::TransFlag::NONE)?;
        let result = {
            let filepath = path.join(format!(
                "{}-{}-{}.tar.zst",
                package.name,
                version,
                std::env::consts::ARCH
            ));
            let filename = filepath.to_str().ok_or(Error::Fs(None))?;
            let pkg = alpm.pkg_load(filename, true, alpm.local_file_siglevel())?;
            alpm.trans_add_pkg(pkg).map_err(|err| err.err)?;
            alpm.trans_prepare().map_err(|(_, err)| err)?;
            alpm.trans_commit().map_err(|(_, err)| err)?;
            Ok(())
        };
        alpm.trans_release()?;
        result
    }

    async fn uninstall(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("pacman")
            .args(["--noconfirm", "-R", package.name.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Alpm(None))?;

        let Ok(dir) = DIRS.create_cache_directory("aur") else {
            eprintln!("Failed to create cache directory");
            return Ok(());
        };
        let path = dir.join(&package.name);
        if std::fs::remove_dir_all(path).is_err() {
            eprintln!("Failed to remove package cache directory");
            return Ok(());
        }

        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let tmp_dir = tempdir().map_err(|err| Error::Fs(err.into()))?;
        let tmp_path = &tmp_dir.path();
        let tmp_path_str = tmp_path.to_str().expect("Invalid path");

        std::os::unix::fs::symlink("/var/lib/pacman/local", tmp_path.join("local"))
            .map_err(|err| Error::Fs(err.into()))?;
        let status = Command::new("fakeroot")
            .args([
                "--",
                "pacman",
                "-Sy",
                "--dbpath",
                tmp_path_str,
                "--logfile",
                "/dev/null",
                "--noconfirm",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| Error::Alpm(None))?;

        if !status.success() {
            return Err(Error::Alpm(None));
        }

        let output = Command::new("pacman")
            .args(["-Qum", "--dbpath", tmp_path_str])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .map_err(|_| Error::Alpm(None))?;

        let packages = output
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let name = parts.next()?;
                if alpm.ignorepkgs().iter().any(|p| p == name) {
                    return None;
                }
                alpm.syncdbs()
                    .iter()
                    .find_map(|db| db.pkg(name).ok())
                    .map(Package::from)
            })
            .collect();

        Ok(packages)
    }

    async fn count_updates(&self) -> Result<usize, Self::Error> {
        self.list_updates().await.map(|v| v.len())
    }

    async fn update(&self) -> Result<(), Self::Error> {
        let updates = self.list_updates().await?;
        if let Some(progress_sender) = &self.progress_sender {
            let _ = progress_sender.send(0).await;
        }
        for (i, package) in updates.iter().enumerate() {
            self.install(package).await?;
            if let Some(progress_sender) = &self.progress_sender {
                let _ = progress_sender
                    .send(((i + 1) * 100 / updates.len()).try_into().unwrap())
                    .await;
            }
        }
        Ok(())
    }
}
