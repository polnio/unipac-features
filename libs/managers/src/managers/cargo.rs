use super::Manager;
use reqwest::Url;
use semver::Version;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::Display,
    process::{Command, Stdio},
    str::FromStr,
    sync::RwLock,
};
use tokio::sync::mpsc::Sender;

pub type Error = String;

#[derive(Clone)]
pub enum PackageRepository {
    Registry { url: Url },
    Git { url: Url, commit: String },
}
impl FromStr for PackageRepository {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, url) = s
            .split_once("+")
            .ok_or("Failed to parse package repository")?;
        match kind {
            "git" => {
                let (url, commit) = url
                    .split_once("#")
                    .ok_or("Failed to parse package repository")?;
                Ok(Self::Git {
                    url: Url::parse(url).map_err(|_| "Failed to parse package repository")?,
                    commit: commit.into(),
                })
            }
            "registry" => Ok(Self::Registry {
                url: Url::parse(url).map_err(|_| "Failed to parse package repository")?,
            }),
            _ => Err("Failed to parse package repository".into()),
        }
    }
}
impl Display for PackageRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registry { url } => write!(f, "registry+{url}"),
            Self::Git { url, commit } => write!(f, "git+{url}#{commit}"),
        }
    }
}

#[derive(Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub repository: Option<PackageRepository>,
    pub bins: Vec<String>,
}
impl From<GetCratesResponseCrate> for Package {
    fn from(crate_: GetCratesResponseCrate) -> Self {
        Self {
            name: crate_.name,
            version: crate_.max_stable_version,
            repository: crate_.repository.parse().ok(),
            bins: vec![],
        }
    }
}
impl Package {
    fn try_from_config_install(key: String, data: ConfigInstall) -> Result<Self, Error> {
        let mut parts = key.split(" ");
        let name = parts.next().ok_or("Failed to parse package name")?;
        let version = parts.next().ok_or("Failed to parse package version")?;
        let repository = parts.next().ok_or("Failed to parse package repository")?;
        let repository = &repository[1..repository.len() - 1];
        Ok(Package {
            name: name.into(),
            version: version.into(),
            repository: repository.parse().ok(),
            bins: data.bins,
        })
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ConfigInstall {
    version_req: Option<String>,
    bins: Vec<String>,
    features: Vec<String>,
    all_features: bool,
    no_default_features: bool,
    profile: String,
    target: String,
    rustc: String,
}
#[derive(Deserialize)]
struct Config {
    installs: HashMap<String, ConfigInstall>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct GetCratesResponseCrate {
    // created_at: ,
    repository: String,
    description: Option<String>,
    downloads: u64,
    recent_downloads: u64,
    exact_match: bool,
    id: String,
    name: String,
    max_stable_version: String,
}
#[derive(Deserialize)]
struct GetCratesResponse {
    crates: Vec<GetCratesResponseCrate>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct CrateIndexElement {
    name: String,
    #[serde(rename = "vers")]
    version: String,
    #[serde(rename = "cksum")]
    checksum: String,
    rust_version: String,
}

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
}

pub struct Cargo {
    progress_sender: Option<Sender<u8>>,
    http_client: reqwest::Client,
    update_cache: RwLock<Option<Vec<Package>>>,
}
impl Cargo {
    pub fn new() -> Self {
        Self::create(None)
    }
    pub fn with_progress(progress_sender: Sender<u8>) -> Self {
        Self::create(Some(progress_sender))
    }
    fn create(progress_sender: Option<Sender<u8>>) -> Self {
        Self {
            progress_sender,
            http_client: reqwest::Client::builder()
                .user_agent("Unipac <https://github.com/polnio/unipac>")
                .build()
                .expect("Failed to create HTTP client"),
            update_cache: None.into(),
        }
    }

    fn config(&self) -> Result<Config, Error> {
        let home_dir = dirs::home_dir().ok_or("Failed to get home directory")?;
        let config_path = home_dir.join(".cargo/.crates2.json");
        let file = std::fs::File::open(config_path).map_err(|_| "Failed to open config file")?;
        let config: Config = serde_json::from_reader(file).map_err(|_| "Failed to parse config")?;
        Ok(config)
    }
}
impl Manager for Cargo {
    type Package = Package;
    type Error = Error;

    async fn list(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let config = self.config()?;
        let packages = config
            .installs
            .into_iter()
            .map(|(key, data)| Package::try_from_config_install(key, data))
            .collect();

        packages
    }

    async fn find(&self, name: &str) -> Result<Option<Self::Package>, Self::Error> {
        let config = self.config()?;
        let package = config.installs.into_iter().find_map(|(key, data)| {
            let package = Package::try_from_config_install(key, data).ok()?;
            (package.name == name).then_some(package)
        });

        Ok(package)
    }

    async fn search(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let response = self
            .http_client
            .get("https://crates.io/api/v1/crates")
            .query(&[("q", query)])
            .send()
            .await
            .map_err(|err| format!("Failed to send request: {err}"))?;

        if !response.status().is_success() {
            return Err("Failed to send request".into());
        }

        let json: GetCratesResponse = response
            .json()
            .await
            .map_err(|_| "Failed to parse response")?;

        let packages = json.crates.into_iter().map(Package::from).collect();

        Ok(packages)
    }

    async fn search_install(&self, query: &str) -> Result<Vec<Self::Package>, Self::Error> {
        let response = self
            .http_client
            .get("https://crates.io/api/v1/crates")
            .query(&[("q", query)])
            .send()
            .await
            .map_err(|err| format!("Failed to send request: {err}"))?;

        if !response.status().is_success() {
            return Err("Failed to send request".into());
        }

        let json: GetCratesResponse = response
            .json()
            .await
            .map_err(|_| "Failed to parse response")?;

        let packages = json
            .crates
            .into_iter()
            .filter_map(|crate_| crate_.exact_match.then(|| crate_.into()))
            .collect();

        Ok(packages)
    }

    async fn install(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("cargo")
            .args(["install", &package.name, "--version", &package.version])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|err| format!("Failed to install package: {err}"))?;
        Ok(())
    }

    async fn uninstall(&self, package: &Self::Package) -> Result<(), Self::Error> {
        Command::new("cargo")
            .args(["uninstall", &package.name])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut p| p.wait())
            .map_err(|err| format!("Failed to uninstall package: {err}"))?;
        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Self::Package>, Self::Error> {
        let base_url =
            Url::parse("https://raw.githubusercontent.com/rust-lang/crates.io-index/master/")
                .expect("Failed to parse URL");
        let all_packages = self.list().await?;
        let mut packages = Vec::with_capacity(all_packages.len());
        let handles = all_packages
            .into_iter()
            .filter_map(|package| {
                let Some(repository) = package.repository.clone() else {
                    return None;
                };
                let Ok(current_version) = Version::parse(&package.version) else {
                    return None;
                };
                match repository {
                    PackageRepository::Registry { .. } => {
                        let name = package.name.to_lowercase();
                        let relative_url_str = match name.len() {
                            1 => format!("1/{}", &name),
                            2 => format!("2/{}", &name),
                            3 => format!("3/{}/{}", &name[0..1], &name),
                            _ => format!("{}/{}/{}", &name[0..2], &name[2..4], &name),
                        };
                        let Ok(url) = base_url.join(&relative_url_str) else {
                            return None;
                        };
                        Some(tokio::spawn(async move {
                            let Ok(response) = reqwest::get(url).await else {
                                return (package, false);
                            };
                            let Ok(body) = response.text().await else {
                                return (package, false);
                            };
                            let mut versions = body
                                .lines()
                                .filter_map(|line| {
                                    serde_json::from_str::<CrateIndexElement>(line)
                                        .ok()
                                        .and_then(|e| Version::parse(&e.version).ok())
                                })
                                .collect::<Vec<_>>();

                            if current_version.pre.is_empty() {
                                versions.retain(|v| v.pre.is_empty());
                            }
                            versions.sort();
                            let Some(version) = versions.last() else {
                                return (package, false);
                            };
                            let new_package = Package {
                                version: version.to_string(),
                                ..package
                            };
                            (new_package, current_version < *version)
                        }))
                    }
                    PackageRepository::Git { url, commit } => Some(tokio::spawn(async move {
                        let Ok(response) = reqwest::get(url.clone()).await else {
                            return (package, false);
                        };
                        let Ok(latest_commit) =
                            response.json::<CommitResponse>().await.map(|r| r.sha)
                        else {
                            return (package, false);
                        };
                        (package, *commit != latest_commit)
                    })),
                }
            })
            .collect::<Vec<_>>();

        for handle in handles {
            let Ok((package, might_update)) = handle.await else {
                continue;
            };
            if might_update {
                packages.push(package);
            }
        }

        self.update_cache.write().unwrap().replace(packages.clone());

        Ok(packages)
    }

    async fn count_updates(&self) -> Result<usize, Self::Error> {
        self.list_updates().await.map(|v| v.len())
    }

    async fn update(&self) -> Result<(), Self::Error> {
        if self.update_cache.read().unwrap().is_none() {
            self.list_updates().await?;
        }
        let packages = self.update_cache.write().unwrap().take().unwrap();
        for (i, package) in packages.iter().enumerate() {
            if let Some(sender) = self.progress_sender.as_ref() {
                let _ = sender
                    .send((i * 100 / packages.len()).try_into().unwrap())
                    .await;
            }
            self.install(package).await?;
        }
        if let Some(sender) = self.progress_sender.as_ref() {
            let _ = sender.send(100).await;
        }
        Ok(())
    }
}
