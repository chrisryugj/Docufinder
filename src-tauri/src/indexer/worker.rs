#![allow(dead_code)]

use std::path::PathBuf;
use tokio::sync::mpsc;

/// 백그라운드 인덱싱 워커
pub struct IndexWorker {
    tx: mpsc::Sender<IndexTask>,
}

pub enum IndexTask {
    IndexFile(PathBuf),
    RemoveFile(PathBuf),
    ReindexFolder(PathBuf),
}

impl IndexWorker {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel::<IndexTask>(100);

        // 백그라운드 태스크 스폰
        tokio::spawn(async move {
            while let Some(task) = rx.recv().await {
                match task {
                    IndexTask::IndexFile(path) => {
                        tracing::info!("Indexing file: {:?}", path);
                        // TODO: 파싱 → FTS 인덱싱 → 벡터 인덱싱
                    }
                    IndexTask::RemoveFile(path) => {
                        tracing::info!("Removing from index: {:?}", path);
                        // TODO: DB에서 삭제
                    }
                    IndexTask::ReindexFolder(path) => {
                        tracing::info!("Reindexing folder: {:?}", path);
                        // TODO: 폴더 내 모든 파일 재인덱싱
                    }
                }
            }
        });

        Self { tx }
    }

    pub async fn submit(&self, task: IndexTask) -> Result<(), mpsc::error::SendError<IndexTask>> {
        self.tx.send(task).await
    }
}
