use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn unique_test_dir(prefix: &str) -> PathBuf {
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    let base = std::env::current_dir()
        .expect("current directory should be available")
        .join("target")
        .join("rhoiscribe-tests")
        .join(prefix);
    fs::create_dir_all(&base).expect("test base directory should be created");

    for _ in 0..100 {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!("{}-{}-{}", std::process::id(), suffix, counter));
        if fs::create_dir(&path).is_ok() {
            return path;
        }
    }

    panic!("failed to create unique test directory");
}
