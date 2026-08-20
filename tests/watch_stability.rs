use std::{path::Path, sync::Arc, time::Duration};
use unlockm::{
    app::{self, runner::Cancellation},
    config::RunConfig,
};

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
    .expect("stable output was not observed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_write_is_debounced_until_the_last_observable_version() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    let output = temp.path().join("output");
    std::fs::create_dir(&input).unwrap();
    let config = Arc::new(
        RunConfig::new(
            input.clone(),
            Some(output.clone()),
            false,
            false,
            true,
            true,
            false,
            1,
            64,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let input = config.input.clone();
    let output = config.output.clone();
    let cancel = Cancellation::default();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(app::watch::run_until_cancelled_ready(
        config,
        app::Summary::default(),
        cancel.clone(),
        ready_tx,
    ));
    ready_rx.await.unwrap();
    let source = input.join("partial.lrc");
    std::fs::write(&source, b"one").unwrap();
    // The second write is part of the same event burst. The assertion waits on
    // the published state, not a guessed backend delivery delay.
    std::fs::write(&source, b"final").unwrap();
    wait_for(&output.join("partial.lrc"), b"final").await;
    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unchanged_reconciliation_is_quiescent_across_multiple_periods() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    let output = temp.path().join("output");
    std::fs::create_dir(&input).unwrap();
    // This represents an already-converged initial scan. The watch snapshot
    // must not turn it into recurring work on each one-second reconcile tick.
    std::fs::write(input.join("static.lrc"), b"static").unwrap();
    let config = Arc::new(
        RunConfig::new(
            input,
            Some(output.clone()),
            false,
            false,
            true,
            true,
            false,
            1,
            64,
            1,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let cancel = Cancellation::default();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(app::watch::run_until_cancelled_ready(
        config,
        app::Summary::default(),
        cancel.clone(),
        ready_tx,
    ));
    ready_rx.await.unwrap();
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert!(!output.join("static.lrc").exists());
    cancel.cancel();
    let summary = task.await.unwrap().unwrap();
    assert_eq!(summary.copied + summary.converted, 0);
}
