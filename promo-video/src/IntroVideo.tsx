import React from "react";
import {
  AbsoluteFill,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
  Sequence,
  Easing,
} from "remotion";

// ─── Design tokens ───
const C = {
  bg: "#06080F",
  surface: "rgba(255,255,255,0.04)",
  surfaceHover: "rgba(255,255,255,0.08)",
  border: "rgba(255,255,255,0.08)",
  accent: "#6366F1", // indigo-500
  accentLight: "#818CF8",
  accentGlow: "rgba(99,102,241,0.35)",
  cyan: "#22D3EE",
  emerald: "#34D399",
  amber: "#FBBF24",
  rose: "#FB7185",
  text: "#F1F5F9",
  textMuted: "#94A3B8",
  textDim: "#64748B",
  mono: "'Consolas', 'SF Mono', 'Fira Code', monospace",
  sans: "'Inter', 'Pretendard', -apple-system, sans-serif",
};

// ─── Helpers ───
const ease = (
  f: number,
  from: number,
  to: number,
  inputRange: [number, number]
) =>
  interpolate(f, inputRange, [from, to], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
    easing: Easing.out(Easing.cubic),
  });

const fadeUp = (f: number, start: number, dur = 18) => ({
  opacity: ease(f, 0, 1, [start, start + dur]),
  transform: `translateY(${ease(f, 40, 0, [start, start + dur])}px)`,
});

// ─── Dot grid background ───
const DotGrid: React.FC<{ opacity?: number }> = ({ opacity = 0.15 }) => (
  <div
    style={{
      position: "absolute",
      inset: 0,
      opacity,
      backgroundImage:
        "radial-gradient(circle, rgba(255,255,255,0.25) 1px, transparent 1px)",
      backgroundSize: "48px 48px",
    }}
  />
);

// ─── Radial glow ───
const Glow: React.FC<{
  x: string;
  y: string;
  color: string;
  size?: number;
  opacity?: number;
}> = ({ x, y, color, size = 600, opacity = 0.3 }) => (
  <div
    style={{
      position: "absolute",
      left: x,
      top: y,
      width: size,
      height: size,
      borderRadius: "50%",
      background: color,
      filter: `blur(${size / 2}px)`,
      opacity,
      transform: "translate(-50%, -50%)",
      pointerEvents: "none",
    }}
  />
);

// ─── Glass card ───
const Glass: React.FC<{
  children: React.ReactNode;
  style?: React.CSSProperties;
}> = ({ children, style }) => (
  <div
    style={{
      background: "rgba(255,255,255,0.03)",
      backdropFilter: "blur(40px)",
      border: "1px solid rgba(255,255,255,0.08)",
      borderRadius: 24,
      ...style,
    }}
  >
    {children}
  </div>
);

// ─── Badge pill ───
const Badge: React.FC<{
  children: string;
  color: string;
  style?: React.CSSProperties;
}> = ({ children, color, style }) => (
  <span
    style={{
      display: "inline-block",
      padding: "8px 20px",
      borderRadius: 100,
      fontSize: 22,
      fontWeight: 600,
      letterSpacing: 0.5,
      background: `${color}18`,
      color,
      border: `1px solid ${color}30`,
      fontFamily: C.sans,
      ...style,
    }}
  >
    {children}
  </span>
);

// ─── Feature card ───
const FeatureCard: React.FC<{
  icon: string;
  title: string;
  desc: string;
  color: string;
  frame: number;
  delay: number;
  fps: number;
}> = ({ icon, title, desc, color, frame, delay, fps }) => {
  const s = spring({ frame: Math.max(0, frame - delay), fps, config: { damping: 14, mass: 0.8 } });
  return (
    <div
      style={{
        width: 320,
        padding: "36px 32px",
        borderRadius: 20,
        background: C.surface,
        border: `1px solid ${C.border}`,
        transform: `scale(${s}) translateY(${(1 - s) * 30}px)`,
        opacity: s,
      }}
    >
      <div
        style={{
          width: 56,
          height: 56,
          borderRadius: 14,
          background: `${color}15`,
          border: `1px solid ${color}30`,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: 28,
          marginBottom: 20,
        }}
      >
        {icon}
      </div>
      <div
        style={{
          fontSize: 28,
          fontWeight: 700,
          color: C.text,
          marginBottom: 8,
          fontFamily: C.sans,
        }}
      >
        {title}
      </div>
      <div
        style={{
          fontSize: 20,
          color: C.textMuted,
          lineHeight: 1.5,
          fontFamily: C.sans,
        }}
      >
        {desc}
      </div>
    </div>
  );
};

// ─── Search result row (with content snippet) ───
const ResultRow: React.FC<{
  icon: string;
  name: string;
  path: string;
  snippet: string;
  hlWord: string;
  score: string;
  frame: number;
  delay: number;
}> = ({ icon, name, path, snippet, hlWord, score, frame, delay }) => {
  const o = ease(frame, 0, 1, [delay, delay + 10]);
  const x = ease(frame, 30, 0, [delay, delay + 10]);
  // Highlight matched word in snippet
  const parts = snippet.split(hlWord);
  return (
    <div
      style={{
        display: "flex",
        alignItems: "flex-start",
        gap: 20,
        padding: "16px 24px",
        borderRadius: 12,
        background: C.surfaceHover,
        opacity: o,
        transform: `translateX(${x}px)`,
        fontFamily: C.sans,
      }}
    >
      <span style={{ fontSize: 28, marginTop: 4 }}>{icon}</span>
      <div style={{ flex: 1 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div style={{ fontSize: 24, fontWeight: 600, color: C.text }}>{name}</div>
          <div style={{ fontSize: 16, color: C.textDim }}>{path}</div>
        </div>
        <div
          style={{
            fontSize: 19,
            color: C.textMuted,
            marginTop: 6,
            lineHeight: 1.4,
            borderLeft: `2px solid ${C.accent}40`,
            paddingLeft: 12,
          }}
        >
          {parts.map((part, i) => (
            <React.Fragment key={i}>
              {part}
              {i < parts.length - 1 && (
                <span
                  style={{
                    color: C.amber,
                    fontWeight: 700,
                    background: `${C.amber}15`,
                    padding: "0 2px",
                    borderRadius: 3,
                  }}
                >
                  {hlWord}
                </span>
              )}
            </React.Fragment>
          ))}
        </div>
      </div>
      <Badge color={C.emerald} style={{ fontSize: 18 }}>
        {score}
      </Badge>
    </div>
  );
};

// ═══════════════════════════════════════════════
export const IntroVideo: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // ─── Scene transition helper ───
  const sceneOut = (sceneEnd: number) =>
    ease(frame, 1, 0, [sceneEnd - 12, sceneEnd]);

  return (
    <AbsoluteFill style={{ backgroundColor: C.bg, fontFamily: C.sans }}>
      {/* ══════ Scene 1: Hero (0-119) ══════ */}
      <Sequence from={0} durationInFrames={120}>
        <AbsoluteFill
          style={{
            justifyContent: "center",
            alignItems: "center",
            opacity: sceneOut(120),
          }}
        >
          <DotGrid opacity={0.08} />
          <Glow x="50%" y="40%" color={C.accent} size={800} opacity={0.2} />
          <Glow x="30%" y="60%" color={C.cyan} size={500} opacity={0.1} />

          {/* Version badge */}
          <div style={{ ...fadeUp(frame, 5), position: "relative" }}>
            <Badge color={C.accent}>v2.1.0</Badge>
          </div>

          {/* Title */}
          <div
            style={{
              ...fadeUp(frame, 12),
              marginTop: 28,
              position: "relative",
            }}
          >
            <h1
              style={{
                fontSize: 130,
                fontWeight: 800,
                margin: 0,
                color: C.text,
                letterSpacing: -3,
                fontFamily: C.sans,
              }}
            >
              Anything
            </h1>
          </div>

          {/* Tagline with typing */}
          <div
            style={{
              ...fadeUp(frame, 22),
              marginTop: 16,
              position: "relative",
            }}
          >
            {(() => {
              const fullText = "파일명이 아닌, 문서 내용을 검색합니다";
              const chars = Math.floor(
                ease(frame, 0, fullText.length, [35, 80])
              );
              return (
                <div
                  style={{
                    fontSize: 44,
                    color: C.textMuted,
                    fontWeight: 400,
                    fontFamily: C.sans,
                  }}
                >
                  {fullText.substring(0, chars)}
                  <span
                    style={{
                      opacity: frame % 16 < 8 ? 1 : 0,
                      color: C.accent,
                      fontWeight: 300,
                    }}
                  >
                    |
                  </span>
                </div>
              );
            })()}
          </div>

          {/* Tech badges */}
          <div
            style={{
              ...fadeUp(frame, 85),
              display: "flex",
              gap: 12,
              marginTop: 36,
              position: "relative",
            }}
          >
            {["Tauri 2", "React 19", "Rust", "ONNX", "SQLite FTS5"].map(
              (t) => (
                <Badge key={t} color={C.textDim}>
                  {t}
                </Badge>
              )
            )}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 2: Search Demo (120-269) ══════ */}
      <Sequence from={120} durationInFrames={150}>
        <AbsoluteFill
          style={{
            justifyContent: "center",
            alignItems: "center",
            padding: "0 120px",
            opacity: sceneOut(270),
          }}
        >
          <DotGrid opacity={0.05} />
          <Glow x="70%" y="30%" color={C.cyan} size={600} opacity={0.12} />

          {/* Section header */}
          <div style={{ ...fadeUp(frame, 122), marginBottom: 36, textAlign: "center" }}>
            <div style={{ fontSize: 22, fontWeight: 600, color: C.cyan, letterSpacing: 3, textTransform: "uppercase", marginBottom: 10, fontFamily: C.sans }}>
              Content Search
            </div>
            <div style={{ fontSize: 52, fontWeight: 800, color: C.text, letterSpacing: -1, fontFamily: C.sans }}>
              문서 <span style={{ color: C.cyan }}>내용</span>을 검색합니다
            </div>
            <div style={{ fontSize: 26, color: C.textMuted, marginTop: 10, fontFamily: C.sans }}>
              파일명만 찾는 게 아닙니다 --- HWPX, DOCX, XLSX, PDF 안의 텍스트까지
            </div>
          </div>

          <div
            style={{
              display: "flex",
              gap: 60,
              width: "100%",
              maxWidth: 1680,
              alignItems: "flex-start",
            }}
          >
            {/* Left: Search UI mockup */}
            <Glass
              style={{
                flex: 1,
                padding: 0,
                overflow: "hidden",
                ...fadeUp(frame, 125),
              }}
            >
              {/* Title bar */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "16px 24px",
                  borderBottom: `1px solid ${C.border}`,
                }}
              >
                <div
                  style={{
                    width: 14,
                    height: 14,
                    borderRadius: 7,
                    background: "#EF4444",
                  }}
                />
                <div
                  style={{
                    width: 14,
                    height: 14,
                    borderRadius: 7,
                    background: "#F59E0B",
                  }}
                />
                <div
                  style={{
                    width: 14,
                    height: 14,
                    borderRadius: 7,
                    background: "#22C55E",
                  }}
                />
                <span
                  style={{
                    marginLeft: 16,
                    fontSize: 18,
                    color: C.textDim,
                    fontFamily: C.sans,
                  }}
                >
                  Anything
                </span>
              </div>

              {/* Search bar */}
              <div style={{ padding: "20px 24px" }}>
                <div
                  style={{
                    background: C.surfaceHover,
                    borderRadius: 12,
                    padding: "16px 24px",
                    display: "flex",
                    alignItems: "center",
                    gap: 12,
                    border: `1px solid ${C.accent}40`,
                  }}
                >
                  <span style={{ fontSize: 22, color: C.textDim }}>
                    {"\u{1F50D}"}
                  </span>
                  <span
                    style={{
                      fontSize: 24,
                      color: C.text,
                      fontFamily: C.sans,
                    }}
                  >
                    {(() => {
                      const q = "예산 집행 현황 보고서";
                      const c = Math.floor(ease(frame, 0, q.length, [140, 168]));
                      return q.substring(0, c);
                    })()}
                    <span
                      style={{
                        opacity:
                          frame > 135 && frame < 175 && frame % 16 < 8
                            ? 1
                            : 0,
                        color: C.accent,
                      }}
                    >
                      |
                    </span>
                  </span>
                </div>
              </div>

              {/* Results */}
              <div
                style={{
                  padding: "0 24px 24px",
                  display: "flex",
                  flexDirection: "column",
                  gap: 8,
                }}
              >
                <ResultRow
                  icon={"\u{1F4C4}"}
                  name="2026 예산집행현황.hwpx"
                  path="C:/업무/재정/"
                  snippet="...3분기 예산 집행률 87.3%로 전년 대비 12%p 상승하였으며, 예산 잔액은..."
                  hlWord="예산 집행"
                  score="98%"
                  frame={frame}
                  delay={175}
                />
                <ResultRow
                  icon={"\u{1F4CA}"}
                  name="1분기_집행실적.xlsx"
                  path="C:/업무/재정/"
                  snippet="...사업별 예산 집행 현황: 인건비 92%, 운영비 78%, 사업비 집행 잔액 4.2억..."
                  hlWord="예산 집행"
                  score="94%"
                  frame={frame}
                  delay={182}
                />
                <ResultRow
                  icon={"\u{1F4D1}"}
                  name="예산운용계획_보고.pdf"
                  path="C:/업무/기획/"
                  snippet="...차년도 예산 집행 계획 수립 시 전년도 집행률을 기반으로 조정..."
                  hlWord="예산 집행"
                  score="91%"
                  frame={frame}
                  delay={189}
                />
              </div>
            </Glass>

            {/* Right: Pipeline explanation */}
            <div
              style={{
                flex: 0.7,
                display: "flex",
                flexDirection: "column",
                gap: 24,
                paddingTop: 40,
              }}
            >
              {[
                {
                  label: "Keyword",
                  desc: "FTS5 + Lindera 형태소 분석",
                  color: C.cyan,
                  d: 190,
                },
                {
                  label: "Semantic",
                  desc: "KoSimCSE 768차원 벡터",
                  color: C.accent,
                  d: 198,
                },
                {
                  label: "Hybrid",
                  desc: "RRF 스코어 병합",
                  color: C.emerald,
                  d: 206,
                },
                {
                  label: "Rerank",
                  desc: "Cross-Encoder 재정렬",
                  color: C.amber,
                  d: 214,
                },
              ].map((step) => (
                <div
                  key={step.label}
                  style={{
                    ...fadeUp(frame, step.d),
                    display: "flex",
                    alignItems: "center",
                    gap: 20,
                  }}
                >
                  <div
                    style={{
                      width: 8,
                      height: 48,
                      borderRadius: 4,
                      background: step.color,
                    }}
                  />
                  <div>
                    <div
                      style={{
                        fontSize: 28,
                        fontWeight: 700,
                        color: step.color,
                        fontFamily: C.mono,
                      }}
                    >
                      {step.label}
                    </div>
                    <div
                      style={{
                        fontSize: 22,
                        color: C.textMuted,
                        fontFamily: C.sans,
                      }}
                    >
                      {step.desc}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 3: Features Grid (270-399) ══════ */}
      <Sequence from={270} durationInFrames={130}>
        <AbsoluteFill
          style={{
            justifyContent: "center",
            alignItems: "center",
            padding: "0 100px",
            opacity: sceneOut(400),
          }}
        >
          <DotGrid opacity={0.06} />
          <Glow x="20%" y="50%" color={C.accent} size={700} opacity={0.15} />
          <Glow x="80%" y="30%" color={C.emerald} size={500} opacity={0.1} />

          {/* Section title */}
          <div style={{ ...fadeUp(frame, 275), marginBottom: 50 }}>
            <div
              style={{
                fontSize: 22,
                fontWeight: 600,
                color: C.accent,
                letterSpacing: 3,
                textTransform: "uppercase",
                marginBottom: 12,
                fontFamily: C.sans,
              }}
            >
              Features
            </div>
            <div
              style={{
                fontSize: 64,
                fontWeight: 800,
                color: C.text,
                letterSpacing: -1,
                fontFamily: C.sans,
              }}
            >
              Everything you need
            </div>
          </div>

          {/* Feature cards grid */}
          <div
            style={{
              display: "flex",
              gap: 24,
              flexWrap: "wrap",
              justifyContent: "center",
              maxWidth: 1400,
            }}
          >
            {[
              {
                icon: "\u{1F916}",
                title: "AI RAG",
                desc: "Gemini 기반 문맥 질의응답",
                color: C.accent,
                d: 288,
              },
              {
                icon: "\u{1F50D}",
                title: "하이브리드 검색",
                desc: "키워드 + 시맨틱 + 재정렬",
                color: C.cyan,
                d: 296,
              },
              {
                icon: "\u{1F4F7}",
                title: "OCR",
                desc: "스캔 PDF 텍스트 추출",
                color: C.emerald,
                d: 304,
              },
              {
                icon: "\u{1F4DD}",
                title: "HWP 변환",
                desc: "kordoc 자동 변환 번들",
                color: C.amber,
                d: 312,
              },
              {
                icon: "\u{1F3F7}\u{FE0F}",
                title: "파일 태그",
                desc: "커스텀 태그로 분류/검색",
                color: C.rose,
                d: 320,
              },
              {
                icon: "\u{1F4C8}",
                title: "실시간 감시",
                desc: "폴더 변경 자동 인덱싱",
                color: C.accentLight,
                d: 328,
              },
              {
                icon: "\u{2696}\u{FE0F}",
                title: "법령 링크",
                desc: "법령 자동 감지 + law.go.kr",
                color: C.cyan,
                d: 336,
              },
              {
                icon: "\u{1F4E4}",
                title: "내보내기",
                desc: "CSV / JSON 다운로드",
                color: C.emerald,
                d: 344,
              },
            ].map((f) => (
              <FeatureCard key={f.title} icon={f.icon} title={f.title} desc={f.desc} color={f.color} delay={f.d} frame={frame} fps={fps} />
            ))}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 4: File formats (400-479) ══════ */}
      <Sequence from={400} durationInFrames={80}>
        <AbsoluteFill
          style={{
            justifyContent: "center",
            alignItems: "center",
            opacity: sceneOut(480),
          }}
        >
          <DotGrid opacity={0.06} />
          <Glow x="50%" y="50%" color={C.accent} size={700} opacity={0.15} />

          <div style={{ ...fadeUp(frame, 405), marginBottom: 48 }}>
            <div
              style={{
                fontSize: 52,
                fontWeight: 800,
                color: C.text,
                letterSpacing: -1,
                fontFamily: C.sans,
              }}
            >
              모든 문서 형식 지원
            </div>
          </div>

          <div
            style={{
              display: "flex",
              gap: 32,
              ...fadeUp(frame, 415),
            }}
          >
            {[
              { ext: ".hwpx", label: "한글", color: "#3B82F6" },
              { ext: ".hwp", label: "한글 (레거시)", color: "#6366F1" },
              { ext: ".docx", label: "워드", color: "#2563EB" },
              { ext: ".xlsx", label: "엑셀", color: "#059669" },
              { ext: ".pdf", label: "PDF", color: "#DC2626" },
              { ext: ".txt", label: "텍스트", color: "#94A3B8" },
            ].map((f, i) => {
              const s = spring({
                frame: Math.max(0, frame - 420 - i * 6),
                fps,
                config: { damping: 14 },
              });
              return (
                <div
                  key={f.ext}
                  style={{
                    textAlign: "center",
                    transform: `scale(${s})`,
                    opacity: s,
                  }}
                >
                  <div
                    style={{
                      width: 100,
                      height: 120,
                      borderRadius: 16,
                      background: `${f.color}15`,
                      border: `2px solid ${f.color}40`,
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      justifyContent: "center",
                      marginBottom: 12,
                    }}
                  >
                    <div
                      style={{
                        fontSize: 22,
                        fontWeight: 800,
                        color: f.color,
                        fontFamily: C.mono,
                      }}
                    >
                      {f.ext}
                    </div>
                  </div>
                  <div
                    style={{
                      fontSize: 18,
                      color: C.textMuted,
                      fontFamily: C.sans,
                    }}
                  >
                    {f.label}
                  </div>
                </div>
              );
            })}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 5: CTA (480-569) ══════ */}
      <Sequence from={480} durationInFrames={90}>
        <AbsoluteFill
          style={{
            justifyContent: "center",
            alignItems: "center",
          }}
        >
          <DotGrid opacity={0.06} />
          <Glow x="50%" y="45%" color={C.accent} size={900} opacity={0.25} />
          <Glow x="40%" y="55%" color={C.cyan} size={500} opacity={0.1} />

          <div style={{ ...fadeUp(frame, 485), textAlign: "center" }}>
            <div
              style={{
                fontSize: 72,
                fontWeight: 800,
                color: C.text,
                letterSpacing: -2,
                fontFamily: C.sans,
              }}
            >
              지금 바로 시작하세요
            </div>
          </div>

          {/* Download button style */}
          <div style={{ ...fadeUp(frame, 498), marginTop: 44 }}>
            <div
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 16,
                padding: "24px 56px",
                borderRadius: 16,
                background: `linear-gradient(135deg, ${C.accent}, ${C.accentLight})`,
                boxShadow: `0 0 60px ${C.accentGlow}`,
                fontSize: 36,
                fontWeight: 700,
                color: "#fff",
                fontFamily: C.sans,
                transform: `scale(${spring({
                  frame: Math.max(0, frame - 500),
                  fps,
                  config: { damping: 12 },
                })})`,
              }}
            >
              {"\u{2B07}\u{FE0F}"} Download for Windows
            </div>
          </div>

          {/* GitHub link */}
          <div style={{ ...fadeUp(frame, 512), marginTop: 28 }}>
            <div
              style={{
                fontFamily: C.mono,
                fontSize: 28,
                color: C.textDim,
                padding: "12px 32px",
                borderRadius: 10,
                background: C.surface,
                border: `1px solid ${C.border}`,
              }}
            >
              github.com/chrisryugj/Docufinder
            </div>
          </div>

          {/* Bottom tech line */}
          <div style={{ ...fadeUp(frame, 525), marginTop: 36 }}>
            <div
              style={{
                display: "flex",
                gap: 32,
                alignItems: "center",
                fontSize: 22,
                color: C.textDim,
                fontFamily: C.sans,
              }}
            >
              <span>Windows 10/11</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>100% 로컬 처리</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>MIT License</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>OTA 자동 업데이트</span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>
    </AbsoluteFill>
  );
};
