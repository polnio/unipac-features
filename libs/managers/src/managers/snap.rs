use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt::Display;
use std::process::{Command, Stdio};
use std::str::FromStr;
use tokio::sync::mpsc::Sender;

static SEPARATOR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"  +").unwrap());

fn get_next<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Result<String, Error> {
    parts.next().map(String::from).ok_or(Error::Format)
}

#[derive(Debug)]
pub enum Error {
    Format,
    Command,
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format => write!(f, "Format error"),
            Self::Command => write!(f, "Command error"),
        }
    }
}

pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    pub publisher: String,
}
impl FromStr for Package {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = SEPARATOR_REGEX.split(s);
        let name = get_next(&mut parts)?;
        let version = get_next(&mut parts)?;
        parts.next();
        let description = get_next(&mut parts)?;
        Ok(Package {
            name,
            version,
            description,
            publisher: String::new(),
        })
    }
}

pub struct Snap {}
impl Snap {
    pub fn new() -> Self {
        Self {}
    }
    pub fn with_progress(_progress_sender: Sender<u8>) -> Self {
        Self {}
    }
}
impl super::Manager for Snap {
    type Package = Package;
    type Error = Error;

    async fn list(&self) -> Result<Vec<Self::Package>, Self::Error> {
        Command::new("snap")
            .arg("list")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .map_err(|_| Error::Command)?
            .lines()
            .skip(1)
            .map(Package::from_str)
            .collect()
    }

    async fn find(&self, name: &str) -> Result<Option<Self::Package>, Self::Error> {
        let packages = self.list().await?;
        let package = packages
            .into_iter()
            .find(|p| p.name.to_lowercase() == name.to_lowercase());
        Ok(package)
    }

    async fn search(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        Command::new("snap")
            .args(["find", query])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .map_err(|_| Error::Command)?
            .lines()
            .skip(1)
            .filter_map(|s| s.contains("  ").then(|| Package::from_str(s)))
            .collect()
    }

    async fn search_install(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let pkgs = self.search(query).await?;
        let pkgs = pkgs
            .into_iter()
            .find(|p| p.name == query)
            .map_or_else(|| vec![], |pkg| vec![pkg]);
        Ok(pkgs)
    }

    async fn install(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("snap")
            .args(["install", &package.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Command)?;
        Ok(())
    }

    async fn uninstall(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("snap")
            .args(["remove", &package.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Command)?;
        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Self::Package>, Self::Error> {
        Command::new("snap")
            .args(["refresh", "--list"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .map_err(|_| Error::Command)?
            .lines()
            .skip(1)
            .filter_map(|s| s.contains("  ").then(|| Package::from_str(s)))
            .collect()
    }

    async fn count_updates(&self) -> Result<usize, Self::Error> {
        self.list_updates().await.map(|v| v.len())
    }

    async fn update(&self) -> Result<(), Self::Error> {
        Command::new("snap")
            .arg("refresh")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Command)?;
        Ok(())
    }
}
