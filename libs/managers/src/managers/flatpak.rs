use std::fmt::Display;
use std::io::{BufRead as _, BufReader};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::Mutex;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone)]
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

fn get_next<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Result<String, Error> {
    parts.next().map(String::from).ok_or(Error::Format)
}

#[derive(Clone)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub branch: String,
    pub description: String,
}
impl FromStr for Package {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split("\t");
        let name = get_next(&mut parts)?;
        let id = get_next(&mut parts)?;
        let version = get_next(&mut parts)?;
        let branch = get_next(&mut parts)?;
        let description = get_next(&mut parts)?;
        Ok(Package {
            id,
            name,
            version,
            branch,
            description,
        })
    }
}

pub struct Flatpak {
    progress_sender: Option<Sender<String>>,
    update_cache: Mutex<Option<Vec<Package>>>,
}
impl Flatpak {
    pub fn new() -> Self {
        Self {
            progress_sender: None,
            update_cache: None.into(),
        }
    }
    pub fn with_progress(progress_sender: Sender<String>) -> Self {
        Self {
            progress_sender: progress_sender.into(),
            update_cache: None.into(),
        }
    }
}
impl super::Manager for Flatpak {
    type Package = Package;
    type Error = Error;

    async fn list(&self) -> Result<Vec<Self::Package>, Self::Error> {
        Command::new("flatpak")
            .arg("list")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .map_err(|_| Error::Command)?
            .lines()
            .filter(|s| s.contains("\t"))
            .map(Package::from_str)
            .collect()
    }

    async fn find(&self, name: &str) -> Result<Option<Self::Package>, Self::Error> {
        let name = name.to_lowercase();
        let packages = self.list().await?;
        let package = packages
            .into_iter()
            .find(|p| p.name.to_lowercase() == name || p.id.contains(&name));
        Ok(package)
    }

    async fn search(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        Command::new("flatpak")
            .args(["search", query])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .map_err(|_| Error::Command)?
            .lines()
            .filter_map(|s| s.contains("\t").then(|| Package::from_str(s)))
            .collect()
    }

    async fn search_install(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let packages = self.search(query).await?;
        let packages = packages
            .into_iter()
            .filter_map(|p| p.name.to_lowercase().contains(query).then(|| p))
            .collect();
        Ok(packages)
    }

    async fn install(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("flatpak")
            .args(["install", "--noninteractive", "--user", package.id.as_str()])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Command)?;
        Ok(())
    }

    async fn uninstall(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("flatpak")
            .args(["uninstall", package.id.as_str()])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|_| Error::Command)?;
        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let packages: Result<Vec<_>, _> = Command::new("flatpak")
            .args(["remote-ls", "--updates"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .map_err(|_| Error::Command)?
            .lines()
            .filter_map(|s| s.contains("\t").then(|| Package::from_str(s)))
            .collect();

        if let Ok(packages) = packages.clone() {
            self.update_cache.lock().unwrap().replace(packages.clone());
        } else {
            self.update_cache.lock().unwrap().take();
        }

        packages
    }

    async fn count_updates(&self) -> Result<usize, Self::Error> {
        self.list_updates().await.map(|v| v.len())
    }

    async fn update(&self) -> Result<(), Self::Error> {
        // let list = self.list_updates().await?;
        let list = { self.update_cache.lock().unwrap().take() };
        let list = if let Some(list) = list {
            list
        } else {
            self.list_updates().await?
        };
        let stdout = Command::new("flatpak")
            .args(["update", "--noninteractive"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::Command)?
            .stdout
            .ok_or(Error::Command)?;
        let mut i = 0;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else {
                continue;
            };
            // println!("{line}");
            if !line.starts_with("Updating ") || !list.iter().any(|p| line.contains(&p.name)) {
                continue;
            }
            if let Some(package_name) = line.split(" ").nth(1) {
                if let Some(progress_sender) = &self.progress_sender {
                    let _ = progress_sender
                        .send(format!("{}% {}", i * 100 / list.len(), package_name))
                        .await;
                }
            }
            i += 1;
        }
        if let Some(progress_sender) = &self.progress_sender {
            let _ = progress_sender.send("100%".into()).await;
        }
        Ok(())
    }
}
