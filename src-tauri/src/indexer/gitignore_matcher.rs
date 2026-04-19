//! `.gitignore` 기반 제외 매처.
//!
//! 개발자가 `C:\github_project\foo`같은 폴더를 인덱싱하면 `node_modules`, `target`,
//! `dist` 등 빌드 산출물이 계속 증분 인덱싱을 유발함. 루트에 `.git`이 있으면 해당
//! 프로젝트의 `.gitignore` 패턴을 자동 적용하여 이런 파일들을 걸러냄.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// 앱 전역 gitignore 레지스트리 — 스캔/watcher 양쪽에서 공유.
static GLOBAL: Lazy<GitignoreRegistry> = Lazy::new(GitignoreRegistry::new);

/// 전역 레지스트리 접근.
pub fn global() -> &'static GitignoreRegistry {
    &GLOBAL
}

/// 인덱싱 루트 폴더 하나에 대한 gitignore 매처.
///
/// 루트 `.gitignore` + `.git/info/exclude` + 하위 `.gitignore`들을 집합적으로 반영한다.
/// 단순화를 위해 루트의 것만 처리 (하위 nested .gitignore는 공식 ripgrep 동작과 다소 차이).
#[derive(Clone)]
pub struct RootGitignore {
    root: PathBuf,
    matcher: Gitignore,
}

impl RootGitignore {
    /// 루트 폴더의 `.gitignore` + `.git/info/exclude` 로드. `.git`이 없으면 `None`.
    pub fn try_build(root: &Path) -> Option<Self> {
        let git_dir = root.join(".git");
        if !git_dir.exists() {
            return None;
        }

        let mut builder = GitignoreBuilder::new(root);

        let gitignore = root.join(".gitignore");
        if gitignore.exists() {
            if let Some(err) = builder.add(&gitignore) {
                tracing::debug!("gitignore parse warning at {:?}: {}", gitignore, err);
            }
        }

        // .git/info/exclude (개인별 ignore)
        let info_exclude = git_dir.join("info").join("exclude");
        if info_exclude.exists() {
            if let Some(err) = builder.add(&info_exclude) {
                tracing::debug!("info/exclude parse warning at {:?}: {}", info_exclude, err);
            }
        }

        // `.git/` 자체는 항상 제외
        let _ = builder.add_line(None, ".git/");

        match builder.build() {
            Ok(matcher) => {
                tracing::info!(
                    "[gitignore] Loaded rules for project root: {}",
                    root.display()
                );
                Some(Self {
                    root: root.to_path_buf(),
                    matcher,
                })
            }
            Err(e) => {
                tracing::warn!("gitignore build failed at {:?}: {}", root, e);
                None
            }
        }
    }

    /// 주어진 경로가 이 루트의 gitignore 규칙에 매치되면 true.
    /// - `is_dir`가 true면 디렉토리 규칙 매칭
    /// - path가 root의 하위가 아니면 false
    /// - 상위 디렉토리가 ignored면 하위 파일도 ignored로 간주
    ///   (e.g. `node_modules/` 규칙 → `node_modules/foo/bar.js` true)
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        if !path.starts_with(&self.root) {
            return false;
        }
        self.matcher
            .matched_path_or_any_parents(path, is_dir)
            .is_ignore()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// 복수의 인덱싱 루트에 대한 매처들을 들고 있는 레지스트리.
///
/// watch 이벤트에서 경로가 어느 루트 하위인지 찾아서 해당 매처로 평가함.
pub struct GitignoreRegistry {
    /// 루트 경로(정규화) → 매처
    matchers: RwLock<HashMap<PathBuf, RootGitignore>>,
}

impl GitignoreRegistry {
    pub fn new() -> Self {
        Self {
            matchers: RwLock::new(HashMap::new()),
        }
    }

    /// 루트 폴더가 git 프로젝트면 매처 등록. 이미 있으면 갱신.
    /// 반환: 매처가 등록됐으면 true.
    pub fn register_root(&self, root: &Path) -> bool {
        let Some(matcher) = RootGitignore::try_build(root) else {
            return false;
        };
        if let Ok(mut m) = self.matchers.write() {
            m.insert(root.to_path_buf(), matcher);
            true
        } else {
            false
        }
    }

    pub fn unregister_root(&self, root: &Path) {
        if let Ok(mut m) = self.matchers.write() {
            m.remove(root);
        }
    }

    /// 주어진 경로가 등록된 루트들 중 하나의 gitignore에 걸리면 true.
    /// - 가장 깊이 매칭되는 루트 기준
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let Ok(matchers) = self.matchers.read() else {
            return false;
        };
        // 경로 prefix가 가장 긴(=가장 깊이 nested) 루트 선택
        let mut best: Option<&RootGitignore> = None;
        let mut best_len = 0usize;
        for m in matchers.values() {
            if path.starts_with(m.root()) {
                let len = m.root().components().count();
                if len > best_len {
                    best_len = len;
                    best = Some(m);
                }
            }
        }
        best.map(|m| m.is_ignored(path, is_dir)).unwrap_or(false)
    }
}

impl Default for GitignoreRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn returns_none_when_not_git_project() {
        let dir = tempdir().unwrap();
        assert!(RootGitignore::try_build(dir.path()).is_none());
    }

    #[test]
    fn matches_gitignore_rules() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(
            dir.path().join(".gitignore"),
            "node_modules/\n*.log\ndist/\n",
        )
        .unwrap();

        let m = RootGitignore::try_build(dir.path()).expect("should build");

        assert!(m.is_ignored(&dir.path().join("node_modules"), true));
        assert!(m.is_ignored(&dir.path().join("node_modules/foo/bar.js"), false));
        assert!(m.is_ignored(&dir.path().join("app.log"), false));
        assert!(m.is_ignored(&dir.path().join("dist/bundle.js"), false));
        assert!(!m.is_ignored(&dir.path().join("src/main.rs"), false));
    }

    #[test]
    fn excludes_dot_git_always() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        // no .gitignore

        let m = RootGitignore::try_build(dir.path()).expect("should build");
        assert!(m.is_ignored(&dir.path().join(".git"), true));
        assert!(m.is_ignored(&dir.path().join(".git/config"), false));
    }

    #[test]
    fn registry_picks_deepest_root() {
        let outer = tempdir().unwrap();
        let inner = outer.path().join("inner");
        fs::create_dir_all(outer.path().join(".git")).unwrap();
        fs::create_dir_all(&inner).unwrap();
        fs::create_dir_all(inner.join(".git")).unwrap();
        fs::write(outer.path().join(".gitignore"), "*.outer\n").unwrap();
        fs::write(inner.join(".gitignore"), "*.inner\n").unwrap();

        let reg = GitignoreRegistry::new();
        reg.register_root(outer.path());
        reg.register_root(&inner);

        // inner/foo.inner는 inner 매처가 잡아야 함
        assert!(reg.is_ignored(&inner.join("foo.inner"), false));
        // outer/foo.outer는 outer 매처가 잡아야 함
        assert!(reg.is_ignored(&outer.path().join("foo.outer"), false));
        // inner/foo.outer는 inner 매처가 우선 → 매치 안 됨 (inner에는 *.outer 규칙 없음)
        assert!(!reg.is_ignored(&inner.join("foo.outer"), false));
    }
}
