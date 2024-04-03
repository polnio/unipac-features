use alpm_utils::alpm_with_conf;
use pacmanconf::Config;

pub struct Alpm {
    inner: std::sync::Mutex<alpm::Alpm>,
}

unsafe impl Send for Alpm {}
unsafe impl Sync for Alpm {}

impl Alpm {
    pub fn new() -> Self {
        let config = Config::new().expect("Failed to load pacman config");
        let inner = alpm_with_conf(&config).expect("Failed to initialize alpm");
        let alpm = Self {
            inner: inner.into(),
        };
        alpm
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, alpm::Alpm> {
        self.inner.lock().unwrap()
    }
}
