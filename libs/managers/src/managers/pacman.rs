use crate::utils::alpm::Alpm;
use alpm_utils::DbListExt;
use glob_match::glob_match;
use std::fmt::Display;
use std::io::{BufRead as _, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc::Sender;

pub struct Package {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub database: String,
}
impl From<&alpm::Package> for Package {
    fn from(value: &alpm::Package) -> Self {
        Self {
            name: value.name().into(),
            version: value.version().to_string(),
            description: value.desc().map(|desc| desc.into()),
            url: value.url().map(|url| url.into()),
            database: value.db().map_or("unknown".into(), |db| db.name().into()),
        }
    }
}
impl From<&alpm::Pkg> for Package {
    fn from(value: &alpm::Pkg) -> Self {
        Self {
            name: value.name().into(),
            version: value.version().to_string(),
            description: value.desc().map(|desc| desc.into()),
            url: value.url().map(|url| url.into()),
            database: value.db().map_or("unknown".into(), |db| db.name().into()),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Alpm(Option<alpm::Error>),
    Fs(std::io::Error),
}
impl From<alpm::Error> for Error {
    fn from(value: alpm::Error) -> Self {
        Self::Alpm(Some(value))
    }
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Alpm(Some(err)) => write!(f, "Alpm: {}", err),
            Self::Alpm(None) => write!(f, "Alpm: Unknown error"),
            Self::Fs(err) => write!(f, "File system: {}", err),
        }
    }
}

pub struct Pacman {
    alpm: Arc<Alpm>,
    progress_sender: Option<Sender<String>>,
}
impl Pacman {
    pub fn new() -> Self {
        Self::create(None)
    }
    pub fn with_progress(progress_sender: Sender<String>) -> Self {
        let this = Self::create(Some(progress_sender));
        this
    }

    fn create(progress_sender: Option<Sender<String>>) -> Self {
        let alpm = Arc::new(Alpm::new());
        Self {
            alpm,
            progress_sender,
        }
    }
}
impl super::Manager for Pacman {
    type Package = Package;
    type Error = Error;

    async fn list(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let syncdbs = alpm.syncdbs();
        let localdb = alpm.localdb();
        let packages = localdb
            .pkgs()
            .iter()
            .filter_map(|pkg| {
                let Ok(pkg) = syncdbs.pkg(pkg.name()) else {
                    return None;
                };
                Some(pkg.into())
            })
            .collect();
        Ok(packages)
    }

    async fn find(&self, name: &str) -> Result<Option<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let localdb = alpm.localdb();
        let syncdbs = alpm.syncdbs();
        let package = localdb.pkg(name).map(Package::from);
        let Ok(package) = package else {
            return Ok(None);
        };
        if !syncdbs.pkg(package.name.as_str()).is_ok() {
            return Ok(None);
        }
        Ok(Some(package))
    }

    async fn search(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let syncdbs = alpm.syncdbs();
        let packages = syncdbs
            .iter()
            .flat_map(|db| match db.search([query].iter()) {
                Ok(pkgs) => pkgs.iter().map(|pkg| Ok(Package::from(pkg))).collect(),
                Err(err) => vec![Err(err.into())],
            })
            .collect();
        packages
    }

    async fn search_install(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let syncdbs = alpm.syncdbs();
        let mut packages = Vec::with_capacity(3);
        for suffix in ["", "-git", "-bin"] {
            let name = format!("{}{}", query, suffix);
            for db in syncdbs.iter() {
                if let Ok(pkg) = db.pkg(name.as_str()) {
                    if !packages.iter().any(|p: &Package| p.name == name) {
                        packages.push(pkg.into());
                    }
                }
            }
        }
        Ok(packages)
    }

    /* async fn install(&self, package: &Self::Package) -> Result<(), Self::Error> {
        let mut alpm = self.alpm.lock();
        let syncdbs = alpm.syncdbs();
        let pkg = syncdbs.pkg(package.name.as_str())?;
        let result = {
            alpm.trans_init(alpm::TransFlag::NONE)?;
            alpm.trans_add_pkg(pkg).map_err(|err| err.err)?;
            alpm.trans_prepare().map_err(|(_, err)| err)?;
            alpm.trans_commit().map_err(|(_, err)| err)?;
            Ok(())
        };
        alpm.trans_release()?;
        result
    } */

    async fn install(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("pacman")
            .args(["--noconfirm", "-S", package.name.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Alpm(None))?;

        Ok(())
    }

    async fn uninstall(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("pacman")
            .args(["--noconfirm", "-R", package.name.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Alpm(None))?;

        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let alpm = self.alpm.lock();
        let tmp_dir = tempdir().map_err(Error::Fs)?;
        let tmp_path = &tmp_dir.path();
        let tmp_path_str = tmp_path.to_str().expect("Invalid path");

        std::os::unix::fs::symlink("/var/lib/pacman/local", tmp_path.join("local"))
            .map_err(Error::Fs)?;
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
            .map_err(Error::Fs)?;

        if !status.success() {
            return Err(Error::Alpm(None));
        }

        let output = Command::new("pacman")
            .args(["-Qun", "--dbpath", tmp_path_str])
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
                if alpm.ignorepkgs().iter().any(|p| glob_match(p, name)) {
                    return None;
                }
                alpm.syncdbs()
                    .iter()
                    .find_map(|db| db.pkg(name).ok().map(|p| p.into()))
            })
            .collect();

        Ok(packages)
    }

    async fn count_updates(&self) -> Result<usize, Self::Error> {
        self.list_updates().await.map(|v| v.len())
    }

    async fn update(&self) -> Result<(), Self::Error> {
        let stdout = Command::new("pacman")
            .args(["--noconfirm", "-Syu"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::Alpm(None))?
            .stdout
            .ok_or(Error::Alpm(None))?;

        if let Some(progress_sender) = &self.progress_sender {
            let mut count = 0;
            let mut i = 0;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    continue;
                };

                if line.contains("Packages ") {
                    count = line
                        .split(&['(', ')'][..])
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(|| {
                            eprintln!("Failed to parse package count");
                            0
                        });
                } else if line.contains("upgrading") && count > 0 {
                    if let Some(package_name) = line.split(" ").nth(1) {
                        let _ = progress_sender
                            .send(format!("{}% {}", i * 100 / count, package_name))
                            .await;
                    }

                    i += 1;
                }
            }
            if i < count || count == 0 {
                return Err(Error::Alpm(None));
            }
            let _ = progress_sender.send("100%".into()).await;
        }

        Ok(())
    }
}
