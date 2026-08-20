use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use unlockm::{
    app::{self, runner::Cancellation},
    config::RunConfig,
};

fn config(input: PathBuf, output: PathBuf, overwrite: bool) -> Arc<RunConfig> {
    Arc::new(
        RunConfig::new(
            input,
            Some(output),
            false,
            false,
            overwrite,
            true,
            false,
            2,
            64,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap(),
    )
}

async fn wait_for(path: &Path, expected: &[u8]) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if std::fs::read(path).ok().as_deref() == Some(expected) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("watcher did not publish observable output");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recursive_rename_and_modify_use_the_batch_copy_pipeline() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    let output = temp.path().join("output");
    std::fs::create_dir_all(input.join("nested")).unwrap();
    let c = config(input.clone(), output.clone(), true);
    let input = c.input.clone();
    let output = c.output.clone();
    let cancel = Cancellation::default();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(app::watch::run_until_cancelled_ready(
        c,
        app::Summary::default(),
        cancel.clone(),
        ready_tx,
    ));
    ready_rx
        .await
        .expect("watcher should subscribe before the test creates files");

    let pending = input.join("nested/pending.tmp");
    std::fs::write(&pending, b"first").unwrap();
    let source = input.join("nested/song.lrc");
    std::fs::rename(&pending, &source).unwrap();
    let destination = output.join("nested/song.lrc");
    wait_for(&destination, b"first").await;

    std::fs::write(&source, b"second").unwrap();
    wait_for(&destination, b"second").await;
    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owned_temps_are_filtered_and_cancellation_leaves_no_temp_publish() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    let output = temp.path().join("output");
    std::fs::create_dir(&input).unwrap();
    let c = config(input.clone(), output.clone(), false);
    let input = c.input.clone();
    let output = c.output.clone();
    let cancel = Cancellation::default();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(app::watch::run_until_cancelled_ready(
        c,
        app::Summary::default(),
        cancel.clone(),
        ready_tx,
    ));
    ready_rx.await.unwrap();
    std::fs::write(input.join(".unlockm-stage-owned"), b"ignore").unwrap();
    std::fs::write(input.join("real.lrc"), b"kept").unwrap();
    wait_for(&output.join("real.lrc"), b"kept").await;
    assert!(!output.join(".unlockm-stage-owned").exists());
    cancel.cancel();
    task.await.unwrap().unwrap();
    let leftovers = std::fs::read_dir(&output)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".unlockm-"))
        .count();
    assert_eq!(leftovers, 0);
}
