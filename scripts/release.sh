#!/usr/bin/env bash
# release.sh — 릴리즈 태그를 푸시하기 전에 ci.yml 과 동일한 검증을 로컬에서 강제한다.
#
# 왜: publish.yml 의 validate gate 는 GitHub 에서만 돈다. 검증 안 된 태그를
#     푸시하면 CI/빌드가 뒤늦게 실패 → hotfix 커밋 → 태그 재푸시 악순환이 반복됐다
#     (예: v2.6.17 은 cargo fmt --check 실패로 release 중단). 이 스크립트는 그
#     검증을 태그 생성 *전* 으로 당겨, 깨진 태그가 애초에 푸시되지 않게 한다.
#
# 사용법: 버전 bump + CHANGELOG + 커밋을 main 에 끝낸 상태에서
#     ./scripts/release.sh 2.6.19
#
# 검증이 전부 통과해야만 태그를 만들고 푸시한다. 하나라도 실패하면 태그 생성 안 함.
set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "사용법: ./scripts/release.sh <version>   예: ./scripts/release.sh 2.6.19" >&2
  exit 1
fi
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "❌ 버전 형식 오류: '$VERSION' (X.Y.Z 형식이어야 함)" >&2
  exit 1
fi
TAG="v$VERSION"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

step() { printf '\n\033[1;36m▶ %s\033[0m\n' "$1"; }
fail() { printf '\033[1;31m❌ %s\033[0m\n' "$1" >&2; exit 1; }

# ── 0. 사전 상태 점검 ────────────────────────────────────────────────
step "사전 상태 점검"
[[ -z "$(git status --porcelain)" ]] \
  || fail "커밋 안 된 변경이 있다. 버전 bump / CHANGELOG 를 먼저 커밋해라."
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[[ "$BRANCH" == "main" ]] || fail "main 브랜치가 아니다 (현재: $BRANCH)."
git fetch origin --quiet
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] \
  || fail "로컬 main 이 origin/main 과 다르다. pull / push 로 먼저 동기화해라."
! git rev-parse "$TAG" >/dev/null 2>&1 || fail "태그 $TAG 가 이미 존재한다."

# package.json 버전이 인자와 일치하는지 — bump 누락 방지
PKG_VER="$(node -p "require('./package.json').version")"
[[ "$PKG_VER" == "$VERSION" ]] \
  || fail "package.json 버전($PKG_VER) 이 $VERSION 과 다르다. 버전 bump 를 먼저 적용해라."
echo "  OK — main / origin 동기화 / package.json=$VERSION / 태그 미존재"

# ── 1. 프론트엔드 검증 (ci.yml: check-frontend) ─────────────────────
step "프론트엔드 빌드 (tsc + vite)"
pnpm install --frozen-lockfile
pnpm run build

# ── 2. Rust 검증 (ci.yml: check-backend) ────────────────────────────
# 주의: 로컬은 macOS 라 #[cfg(windows)] 코드는 컴파일되지 않는다 — Windows 전용
# 컴파일 에러는 CI 의 validate gate 가 최종 차단한다. cargo fmt 는 OS 무관하게
# 전 파일을 검사하므로 v2.6.17 류 포맷 실패는 여기서 잡힌다.
step "Rust fmt / check / clippy / test"
(
  cd src-tauri
  cargo fmt -- --check
  cargo check --all-targets
  cargo clippy --all-targets -- -D warnings
  cargo test
)

# ── 3. 태그 생성 + 푸시 ─────────────────────────────────────────────
step "검증 전부 통과 — $TAG 태그 생성 + 푸시"
git tag "$TAG"
git push origin "$TAG"

printf '\n\033[1;32m🚀 %s 푸시 완료. GitHub Actions 빌드 시작됨:\033[0m\n' "$TAG"
echo "   https://github.com/chrisryugj/Docufinder/actions"
