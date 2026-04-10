import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Modal } from "../ui/Modal";
import { Button } from "../ui/Button";

// ── Types ─────────────────────────────────────────────

interface SuggestedFolder {
  path: string;
  label: string;
  category: "known" | "drive";
  exists: boolean;
}

interface AutoIndexPromptProps {
  isOpen: boolean;
  onClose: () => void;
  onAutoIndex: () => Promise<unknown>;
  onSelectFolder: () => Promise<unknown>;
  onIndexFolderByPath?: (path: string) => Promise<void>;
}

type View = "main" | "folder-select";

// ── 아이콘 ─────────────────────────────────────────────

const BoltIcon = () => (
  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
);

const FolderIcon = () => (
  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
  </svg>
);

const CheckIcon = () => (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const PlusIcon = () => (
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
  </svg>
);

// ── 컴포넌트 ───────────────────────────────────────────

export function AutoIndexPrompt({
  isOpen,
  onClose,
  onAutoIndex,
  onSelectFolder: _onSelectFolder,
  onIndexFolderByPath,
}: AutoIndexPromptProps) {
  const [view, setView] = useState<View>("main");
  const [suggested, setSuggested] = useState<SuggestedFolder[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loadingSuggested, setLoadingSuggested] = useState(false);
  const [isStarting, setIsStarting] = useState(false);

  // 모달 닫힐 때 상태 리셋
  useEffect(() => {
    if (!isOpen) {
      setView("main");
      setSelected(new Set());
    }
  }, [isOpen]);

  // 폴더 선택 뷰로 전환 시 추천 폴더 로드
  useEffect(() => {
    if (view !== "folder-select") return;
    setLoadingSuggested(true);
    invoke<SuggestedFolder[]>("get_suggested_folders")
      .then((folders) => {
        setSuggested(folders);
        // known 폴더(바탕화면, 문서 등) 기본 선택
        const defaults = new Set(
          folders.filter((f) => f.category === "known").map((f) => f.path)
        );
        setSelected(defaults);
      })
      .catch(() => setSuggested([]))
      .finally(() => setLoadingSuggested(false));
  }, [view]);

  const toggleFolder = useCallback((path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  // 직접 선택 (네이티브 폴더 피커 → 선택 목록에 추가)
  const handlePickFolder = useCallback(async () => {
    const result = await open({
      directory: true,
      multiple: true,
      title: "폴더 선택",
    }).catch(() => null);
    if (!result) return;
    const paths = Array.isArray(result) ? result : [result];
    setSelected((prev) => {
      const next = new Set(prev);
      paths.forEach((p) => next.add(p));
      return next;
    });
  }, []);

  // 선택한 폴더들 인덱싱 시작
  const handleStartIndexing = useCallback(async () => {
    if (selected.size === 0 || !onIndexFolderByPath) return;
    setIsStarting(true);
    onClose();
    for (const path of selected) {
      await onIndexFolderByPath(path).catch(() => {});
    }
    setIsStarting(false);
  }, [selected, onIndexFolderByPath, onClose]);

  // 전체 드라이브 인덱싱
  const handleAutoIndex = useCallback(async () => {
    onClose();
    await onAutoIndex();
  }, [onClose, onAutoIndex]);

  const knownFolders = suggested.filter((f) => f.category === "known");
  const driveFolders = suggested.filter((f) => f.category === "drive");

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={view === "main" ? "문서 검색 시작" : "폴더 선택"}
      size="md"
      closable
    >
      {view === "main" ? (
        /* ── 메인 뷰: 큰 카드 2개 ── */
        <div className="space-y-4">
          <p className="text-sm text-center" style={{ color: "var(--color-text-secondary)" }}>
            어떻게 인덱싱을 시작할까요?
          </p>

          <div className="grid grid-cols-2 gap-3 pt-1">
            {/* 전체 인덱싱 카드 */}
            <button
              onClick={handleAutoIndex}
              className="flex flex-col items-center gap-3 p-5 rounded-xl border-2 transition-all duration-150 hover:shadow-md active:scale-[0.98] text-left group"
              style={{
                borderColor: "var(--color-accent)",
                backgroundColor: "var(--color-accent-light)",
                color: "var(--color-accent)",
              }}
            >
              <div
                className="w-14 h-14 rounded-xl flex items-center justify-center transition-colors"
                style={{ backgroundColor: "var(--color-accent)", color: "white" }}
              >
                <BoltIcon />
              </div>
              <div className="text-center">
                <p className="font-semibold text-sm" style={{ color: "var(--color-accent)" }}>
                  전체 인덱싱
                </p>
                <p className="text-[11px] mt-1 leading-snug" style={{ color: "var(--color-text-secondary)" }}>
                  모든 드라이브 자동 스캔<br />시스템 폴더 자동 제외
                </p>
              </div>
            </button>

            {/* 폴더 선택 카드 */}
            <button
              onClick={() => setView("folder-select")}
              className="flex flex-col items-center gap-3 p-5 rounded-xl border-2 transition-all duration-150 hover:shadow-md hover:border-[var(--color-accent)] active:scale-[0.98] text-left group"
              style={{
                borderColor: "var(--color-border)",
                backgroundColor: "var(--color-bg-secondary)",
                color: "var(--color-text-primary)",
              }}
            >
              <div
                className="w-14 h-14 rounded-xl flex items-center justify-center transition-colors"
                style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-secondary)" }}
              >
                <FolderIcon />
              </div>
              <div className="text-center">
                <p className="font-semibold text-sm" style={{ color: "var(--color-text-primary)" }}>
                  폴더 선택
                </p>
                <p className="text-[11px] mt-1 leading-snug" style={{ color: "var(--color-text-secondary)" }}>
                  원하는 폴더만 직접 선택<br />추천 폴더 또는 직접 지정
                </p>
              </div>
            </button>
          </div>

          <div className="pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
            <button
              className="w-full text-center text-xs py-1.5 transition-colors hover:text-[var(--color-text-secondary)]"
              style={{ color: "var(--color-text-muted)" }}
              onClick={onClose}
            >
              나중에 할게요
            </button>
          </div>
        </div>
      ) : (
        /* ── 폴더 선택 뷰 ── */
        <div className="space-y-4">
          {/* 뒤로 가기 */}
          <button
            onClick={() => setView("main")}
            className="flex items-center gap-1.5 text-xs transition-colors hover:text-[var(--color-text-primary)] -mt-1"
            style={{ color: "var(--color-text-muted)" }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="15 18 9 12 15 6" />
            </svg>
            처음으로
          </button>

          {loadingSuggested ? (
            <div className="flex justify-center py-8">
              <div className="w-5 h-5 border-2 border-[var(--color-accent)] border-t-transparent rounded-full animate-spin" />
            </div>
          ) : (
            <div className="space-y-3">
              {/* known 폴더 */}
              {knownFolders.length > 0 && (
                <div>
                  <p className="text-[10px] font-semibold uppercase tracking-wider mb-2" style={{ color: "var(--color-text-muted)" }}>
                    추천 폴더
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {knownFolders.map((f) => {
                      const isChecked = selected.has(f.path);
                      return (
                        <button
                          key={f.path}
                          onClick={() => toggleFolder(f.path)}
                          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-100 border"
                          style={{
                            backgroundColor: isChecked ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
                            borderColor: isChecked ? "var(--color-accent)" : "var(--color-border)",
                            color: isChecked ? "var(--color-accent)" : "var(--color-text-secondary)",
                          }}
                        >
                          {isChecked && <CheckIcon />}
                          {f.label}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* 드라이브 */}
              {driveFolders.length > 0 && (
                <div>
                  <p className="text-[10px] font-semibold uppercase tracking-wider mb-2" style={{ color: "var(--color-text-muted)" }}>
                    드라이브
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {driveFolders.map((f) => {
                      const isChecked = selected.has(f.path);
                      return (
                        <button
                          key={f.path}
                          onClick={() => toggleFolder(f.path)}
                          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-100 border"
                          style={{
                            backgroundColor: isChecked ? "var(--color-accent-light)" : "var(--color-bg-tertiary)",
                            borderColor: isChecked ? "var(--color-accent)" : "var(--color-border)",
                            color: isChecked ? "var(--color-accent)" : "var(--color-text-secondary)",
                          }}
                        >
                          {isChecked && <CheckIcon />}
                          {f.label}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* 직접 선택 버튼 */}
              <div>
                <p className="text-[10px] font-semibold uppercase tracking-wider mb-2" style={{ color: "var(--color-text-muted)" }}>
                  직접 추가
                </p>
                <button
                  onClick={handlePickFolder}
                  className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs border border-dashed transition-colors hover:border-[var(--color-accent)] hover:text-[var(--color-accent)]"
                  style={{
                    borderColor: "var(--color-border)",
                    color: "var(--color-text-muted)",
                  }}
                >
                  <PlusIcon />
                  폴더 직접 선택...
                </button>
              </div>

              {/* 선택된 폴더 표시 (suggested 외 추가된 폴더들) */}
              {(() => {
                const extraPaths = [...selected].filter(
                  (p) => !suggested.some((s) => s.path === p)
                );
                return extraPaths.length > 0 ? (
                  <div className="flex flex-wrap gap-1.5">
                    {extraPaths.map((p) => {
                      const name = p.replace(/\\/g, "/").split("/").filter(Boolean).pop() || p;
                      return (
                        <span
                          key={p}
                          className="flex items-center gap-1 px-2 py-1 rounded text-[11px] border"
                          style={{
                            backgroundColor: "var(--color-accent-light)",
                            borderColor: "var(--color-accent)",
                            color: "var(--color-accent)",
                          }}
                        >
                          <CheckIcon />
                          {name}
                          <button
                            onClick={() => toggleFolder(p)}
                            className="ml-0.5 opacity-60 hover:opacity-100"
                          >
                            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
                              <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                            </svg>
                          </button>
                        </span>
                      );
                    })}
                  </div>
                ) : null;
              })()}
            </div>
          )}

          {/* 하단 액션 */}
          <div className="flex items-center justify-between pt-2 border-t" style={{ borderColor: "var(--color-border)" }}>
            <span className="text-xs" style={{ color: "var(--color-text-muted)" }}>
              {selected.size > 0 ? `${selected.size}개 폴더 선택됨` : "폴더를 선택하세요"}
            </span>
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={onClose}>
                취소
              </Button>
              <Button
                size="sm"
                disabled={selected.size === 0 || isStarting}
                isLoading={isStarting}
                onClick={handleStartIndexing}
              >
                인덱싱 시작
              </Button>
            </div>
          </div>
        </div>
      )}
    </Modal>
  );
}
