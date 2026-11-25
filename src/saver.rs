//! Writes saves on a background thread. Writing safely means waiting for the
//! disk, which on a modest machine is long enough to drop frames, and the game
//! saves at every change of room.

use babel_core::game::profile::Profile;
use babel_core::save;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;

enum Job {
    Store(PathBuf, Box<Profile>),
    /// Answered once everything queued before it is on disk.
    Flush(Sender<()>),
}

pub struct Saver {
    jobs: Option<Sender<Job>>,
    /// Behind a lock because the game state is shared between threads.
    failures: Mutex<Receiver<String>>,
    worker: Option<JoinHandle<()>>,
}

fn work(jobs: &Receiver<Job>, failures: &Sender<String>) {
    let write = |path: PathBuf, profile: Box<Profile>| {
        if let Err(e) = save::store(&path, &profile) {
            let _ = failures.send(e.to_string());
        }
    };
    let mut next = None;
    loop {
        let Some(job) = next.take().or_else(|| jobs.recv().ok()) else {
            return;
        };
        match job {
            Job::Flush(done) => {
                let _ = done.send(());
            }
            Job::Store(mut path, mut profile) => {
                // Of several saves waiting for the same file, only the newest
                // matters.
                loop {
                    match jobs.try_recv() {
                        Ok(Job::Store(newer_path, newer)) if newer_path == path => {
                            (path, profile) = (newer_path, newer);
                        }
                        Ok(other) => {
                            next = Some(other);
                            break;
                        }
                        Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                    }
                }
                write(path, profile);
            }
        }
    }
}

impl Saver {
    pub fn new() -> Saver {
        let (jobs, queue) = channel();
        let (report, failures) = channel();
        let worker = std::thread::Builder::new()
            .name("saver".into())
            .spawn(move || work(&queue, &report))
            .ok();
        Saver {
            // Without a thread, `store` writes on the caller's.
            jobs: worker.is_some().then_some(jobs),
            failures: Mutex::new(failures),
            worker,
        }
    }

    /// Queues a save. A failure is reported later by `failure`.
    pub fn store(&self, path: PathBuf, profile: Profile) -> Result<(), String> {
        let job = Job::Store(path, Box::new(profile));
        let unsent = match &self.jobs {
            Some(jobs) => jobs.send(job).err().map(|e| e.0),
            None => Some(job),
        };
        match unsent {
            Some(Job::Store(path, profile)) => {
                save::store(&path, &profile).map_err(|e| e.to_string())
            }
            _ => Ok(()),
        }
    }

    /// Waits until every queued save is on disk.
    pub fn flush(&self) {
        if let Some(jobs) = &self.jobs {
            let (done, wait) = channel();
            if jobs.send(Job::Flush(done)).is_ok() {
                let _ = wait.recv();
            }
        }
    }

    /// The error of a save that could not be written, if there is one.
    pub fn failure(&self) -> Option<String> {
        self.failures.lock().ok()?.try_recv().ok()
    }
}

impl Drop for Saver {
    fn drop(&mut self) {
        // Closing the queue lets the thread finish what is left and stop.
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::save::LoadOutcome;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hexbabel-saver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn level_on_disk(path: &std::path::Path) -> u32 {
        match save::load(path) {
            LoadOutcome::Loaded(p) => p.player.level,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_newest_save_is_on_disk_after_a_flush_or_a_drop() {
        let dir = temp_dir("newest");
        let path = dir.join("save.ron");
        let saver = Saver::new();
        let mut profile = Profile::new(1);
        for level in 1..=30 {
            profile.player.level = level;
            saver.store(path.clone(), profile.clone()).expect("queued");
        }
        saver.flush();
        assert_eq!(level_on_disk(&path), 30);

        profile.player.level = 31;
        saver.store(path.clone(), profile).expect("queued");
        drop(saver);
        assert_eq!(level_on_disk(&path), 31);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_save_that_cannot_be_written_is_reported() {
        let dir = temp_dir("blocked");
        // A file where the folder should be makes writing impossible.
        let blocker = dir.join("not-a-folder");
        std::fs::write(&blocker, "x").expect("write");
        let saver = Saver::new();
        saver
            .store(blocker.join("save.ron"), Profile::new(2))
            .expect("queued");
        saver.flush();
        assert!(saver.failure().is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
