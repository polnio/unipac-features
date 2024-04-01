use alpm_utils::alpm_with_conf;
use pacmanconf::Config;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicU8, AtomicUsize};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

pub struct Alpm {
    inner: std::sync::Mutex<alpm::Alpm>,
    progress_receiver: tokio::sync::Mutex<Receiver<u8>>,
    current_progress: Arc<AtomicU8>,
    packages_count: Arc<AtomicUsize>,
}

impl Alpm {
    pub fn new() -> Self {
        let config = Config::new().expect("Failed to load pacman config");
        let inner = alpm_with_conf(&config).expect("Failed to initialize alpm");
        let (progress_sender, progress_receiver) = std::sync::mpsc::channel();
        let alpm = Self {
            inner: inner.into(),
            progress_receiver: progress_receiver.into(),
            current_progress: Default::default(),
            packages_count: Default::default(),
        };
        alpm.setup_cbs(progress_sender);
        alpm
    }

    fn setup_cbs(&self, progress_sender: Sender<u8>) {
        let current_progress = self.current_progress.clone();
        let packages_count = self.packages_count.clone();
        self.lock()
            .set_event_cb(progress_sender, move |event, progress_sender| {
                // println!("event: {:?}", event);
                /* let alpm::Event::PackageOperation(event) = event.event() else {
                    return;
                }; */
                match event.event() {
                    alpm::Event::TransactionStart => {
                        current_progress.store(0, Relaxed);
                    }
                    alpm::Event::PackageOperation(_) => {
                        let current_progress = current_progress.fetch_add(1, Relaxed) + 1;
                        let package_count = packages_count.load(Relaxed);
                        let _ = progress_sender.send(current_progress * 50 / package_count as u8);
                    }
                    alpm::Event::TransactionDone => {
                        let _ = progress_sender.send(100);
                    }
                    _ => {}
                }
            });
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, alpm::Alpm> {
        self.inner.lock().unwrap()
    }

    pub async fn recv_progress(&self) -> Option<u8> {
        self.progress_receiver.lock().await.recv().ok()
    }

    pub fn set_packages_count(&self, count: usize) {
        self.packages_count.store(count, Relaxed);
    }
}
