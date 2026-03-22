import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Calendar, Clock, Loader2 } from "lucide-react";
import { Modal } from "../ui/Modal";
import { Badge } from "../ui/Badge";
import { FileIcon } from "../ui/FileIcon";

interface ExpiryDocument {
  file_path: string;
  file_name: string;
  expiry_date: string;
  days_remaining: number;
  context: string;
  urgency: "expired" | "urgent" | "warning" | "normal";
}

interface ExpiryResponse {
  documents: ExpiryDocument[];
  scan_time_ms: number;
  total_scanned: number;
}

interface ExpiryAlertModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenFile: (path: string) => void;
  showToast: (msg: string, type: "success" | "error" | "info") => string;
}

const URGENCY_CONFIG = {
  expired: { label: "만료됨", variant: "danger" as const, color: "#ef4444" },
  urgent: { label: "7일 이내", variant: "danger" as const, color: "#f97316" },
  warning: { label: "30일 이내", variant: "warning" as const, color: "#eab308" },
  normal: { label: "여유", variant: "success" as const, color: "#22c55e" },
};

export function ExpiryAlertModal({
  isOpen,
  onClose,
  onOpenFile,
  showToast,
}: ExpiryAlertModalProps) {
  const [result, setResult] = useState<ExpiryResponse | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [filter, setFilter] = useState<"all" | "expired" | "urgent" | "warning">("all");

  const handleScan = useCallback(async () => {
    setIsScanning(true);
    setResult(null);
    try {
      const res = await invoke<ExpiryResponse>("scan_expiry_dates");
      setResult(res);
      if (res.documents.length === 0) {
        showToast("만료 관련 날짜가 있는 문서가 없습니다", "info");
      } else {
        const urgent = res.documents.filter((d) => d.urgency !== "normal").length;
        showToast(
          `${res.documents.length}개 문서에서 날짜 발견 (주의 ${urgent}건, ${res.scan_time_ms}ms)`,
          "success"
        );
      }
    } catch (e) {
      showToast(`만료 스캔 실패: ${e}`, "error");
    } finally {
      setIsScanning(false);
    }
  }, [showToast]);

  const filtered = result
    ? filter === "all"
      ? result.documents
      : result.documents.filter((d) => d.urgency === filter)
    : [];

  const counts = result
    ? {
        all: result.documents.length,
        expired: result.documents.filter((d) => d.urgency === "expired").length,
        urgent: result.documents.filter((d) => d.urgency === "urgent").length,
        warning: result.documents.filter((d) => d.urgency === "warning").length,
      }
    : { all: 0, expired: 0, urgent: 0, warning: 0 };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="문서 만료 알림" size="lg">
      <div className="space-y-4">
        {/* 스캔 시작 화면 */}
        {!result && !isScanning && (
          <div className="text-center py-8">
            <div
              className="w-16 h-16 mx-auto mb-4 rounded-2xl flex items-center justify-center"
              style={{ backgroundColor: "var(--color-bg-tertiary)" }}
            >
              <Calendar className="w-8 h-8 opacity-60" style={{ color: "var(--color-text-muted)" }} />
            </div>
            <p className="text-sm mb-1" style={{ color: "var(--color-text-secondary)" }}>
              문서 내 만료일, 유효기간, 계약 종료일 등을 자동으로 찾습니다
            </p>
            <p className="text-xs mb-4" style={{ color: "var(--color-text-muted)" }}>
              키워드: 만료, 유효기간, 까지, 종료, 기한, 계약기간 등
            </p>
            <button
              onClick={handleScan}
              className="px-4 py-2 rounded-lg text-sm font-medium text-white"
              style={{ backgroundColor: "var(--color-accent)" }}
            >
              만료 스캔 시작
            </button>
          </div>
        )}

        {/* 로딩 */}
        {isScanning && (
          <div className="text-center py-12">
            <Loader2
              className="w-8 h-8 mx-auto mb-3 animate-spin"
              style={{ color: "var(--color-accent)" }}
            />
            <p className="text-sm" style={{ color: "var(--color-text-secondary)" }}>
              문서 내 날짜 패턴을 분석 중...
            </p>
          </div>
        )}

        {/* 결과 */}
        {result && !isScanning && (
          <>
            {/* 요약 */}
            <div
              className="flex items-center gap-3 px-3 py-2 rounded-lg text-xs flex-wrap"
              style={{ backgroundColor: "var(--color-bg-tertiary)" }}
            >
              <span>
                스캔: <strong>{result.total_scanned}</strong>개 파일
              </span>
              {counts.expired > 0 && (
                <span style={{ color: "#ef4444" }}>
                  만료: <strong>{counts.expired}</strong>
                </span>
              )}
              {counts.urgent > 0 && (
                <span style={{ color: "#f97316" }}>
                  긴급: <strong>{counts.urgent}</strong>
                </span>
              )}
              {counts.warning > 0 && (
                <span style={{ color: "#eab308" }}>
                  주의: <strong>{counts.warning}</strong>
                </span>
              )}
              <span className="ml-auto" style={{ color: "var(--color-text-muted)" }}>
                {result.scan_time_ms}ms
              </span>
              <button
                onClick={handleScan}
                className="px-2 py-0.5 text-xs rounded border"
                style={{ borderColor: "var(--color-border)" }}
              >
                재스캔
              </button>
            </div>

            {/* 필터 */}
            <div className="flex gap-1 border-b" style={{ borderColor: "var(--color-border)" }}>
              {(["all", "expired", "urgent", "warning"] as const).map((f) => (
                <button
                  key={f}
                  onClick={() => setFilter(f)}
                  className="px-3 py-2 text-xs font-medium border-b-2 transition-colors"
                  style={{
                    borderColor: filter === f ? "var(--color-accent)" : "transparent",
                    color: filter === f ? "var(--color-accent)" : "var(--color-text-muted)",
                  }}
                >
                  {f === "all" ? "전체" : URGENCY_CONFIG[f].label} ({counts[f]})
                </button>
              ))}
            </div>

            {/* 목록 */}
            <div className="max-h-[400px] overflow-y-auto space-y-2">
              {filtered.length === 0 ? (
                <p
                  className="text-center py-8 text-sm"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  해당 조건의 문서가 없습니다
                </p>
              ) : (
                filtered.map((doc, i) => {
                  const config = URGENCY_CONFIG[doc.urgency];
                  return (
                    <div
                      key={i}
                      className="border rounded-lg p-3 hover:bg-[var(--color-bg-tertiary)] transition-colors cursor-pointer"
                      style={{ borderColor: "var(--color-border)" }}
                      onClick={() => onOpenFile(doc.file_path)}
                    >
                      <div className="flex items-center gap-2 mb-1">
                        <FileIcon fileName={doc.file_name} className="w-4 h-4 flex-shrink-0" />
                        <span
                          className="font-medium text-sm truncate"
                          style={{ color: "var(--color-text-primary)" }}
                        >
                          {doc.file_name}
                        </span>
                        <Badge variant={config.variant}>{config.label}</Badge>
                        <span className="ml-auto text-xs flex items-center gap-1" style={{ color: config.color }}>
                          {doc.urgency === "expired" ? (
                            <AlertTriangle className="w-3 h-3" />
                          ) : (
                            <Clock className="w-3 h-3" />
                          )}
                          {doc.days_remaining < 0
                            ? `${Math.abs(doc.days_remaining)}일 전 만료`
                            : doc.days_remaining === 0
                              ? "오늘 만료"
                              : `${doc.days_remaining}일 남음`}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 text-xs" style={{ color: "var(--color-text-muted)" }}>
                        <Calendar className="w-3 h-3 flex-shrink-0" />
                        <span>{doc.expiry_date}</span>
                        <span className="truncate">— {doc.context}</span>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
