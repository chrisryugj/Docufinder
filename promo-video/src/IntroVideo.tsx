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
  bg: "#000000",
  surface: "rgba(255,255,255,0.04)",
  border: "rgba(255,255,255,0.06)",
  accent: "#6366F1",
  accentLight: "#818CF8",
  accentGlow: "rgba(99,102,241,0.4)",
  cyan: "#22D3EE",
  emerald: "#34D399",
  amber: "#FBBF24",
  rose: "#FB7185",
  text: "#FFFFFF",
  textSub: "#E2E8F0",
  textMuted: "#94A3B8",
  textDim: "#475569",
  sans: "'Inter', 'Pretendard', -apple-system, sans-serif",
  mono: "'SF Mono', 'Consolas', 'Fira Code', monospace",
};

// ─── Animation helpers ───
const ease = (f: number, from: number, to: number, range: [number, number]) =>
  interpolate(f, range, [from, to], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
    easing: Easing.out(Easing.cubic),
  });

const easeIn = (f: number, from: number, to: number, range: [number, number]) =>
  interpolate(f, range, [from, to], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
    easing: Easing.in(Easing.cubic),
  });

const fadeUp = (f: number, start: number, dur = 15) => ({
  opacity: ease(f, 0, 1, [start, start + dur]),
  transform: `translateY(${ease(f, 50, 0, [start, start + dur])}px)`,
});

const fadeOut = (f: number, start: number, dur = 10) => ({
  opacity: easeIn(f, 1, 0, [start, start + dur]),
  transform: `translateY(${easeIn(f, 0, -30, [start, start + dur])}px)`,
});

// ─── Ambient glow ───
const Glow: React.FC<{
  x: string; y: string; color: string; size?: number; opacity?: number;
}> = ({ x, y, color, size = 600, opacity = 0.25 }) => (
  <div style={{
    position: "absolute", left: x, top: y,
    width: size, height: size, borderRadius: "50%",
    background: color, filter: `blur(${size * 0.55}px)`,
    opacity, transform: "translate(-50%, -50%)", pointerEvents: "none",
  }} />
);

// ─── Stat number with counter animation ───
const StatNumber: React.FC<{
  value: string; label: string; color: string;
  frame: number; delay: number;
}> = ({ value, label, color, frame, delay }) => {
  const o = ease(frame, 0, 1, [delay, delay + 12]);
  const y = ease(frame, 40, 0, [delay, delay + 12]);
  const scale = ease(frame, 0.9, 1, [delay, delay + 15]);
  return (
    <div style={{
      textAlign: "center", opacity: o,
      transform: `translateY(${y}px) scale(${scale})`,
    }}>
      <div style={{
        fontSize: 96, fontWeight: 800, color,
        fontFamily: C.sans, letterSpacing: -3, lineHeight: 1,
      }}>
        {value}
      </div>
      <div style={{
        fontSize: 24, color: C.textMuted, marginTop: 12,
        fontWeight: 500, fontFamily: C.sans, letterSpacing: 1,
      }}>
        {label}
      </div>
    </div>
  );
};

// ─── Search result row (compact) ───
const ResultRow: React.FC<{
  icon: string; name: string; snippet: string; hlWord: string;
  frame: number; delay: number;
}> = ({ icon, name, snippet, hlWord, frame, delay }) => {
  const o = ease(frame, 0, 1, [delay, delay + 8]);
  const x = ease(frame, 40, 0, [delay, delay + 8]);
  const parts = snippet.split(hlWord);
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 18,
      padding: "14px 22px", borderRadius: 14,
      background: "rgba(255,255,255,0.04)",
      border: `1px solid rgba(255,255,255,0.06)`,
      opacity: o, transform: `translateX(${x}px)`,
      fontFamily: C.sans,
    }}>
      <span style={{ fontSize: 26 }}>{icon}</span>
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 22, fontWeight: 600, color: C.text }}>{name}</div>
        <div style={{
          fontSize: 17, color: C.textMuted, marginTop: 4, lineHeight: 1.4,
        }}>
          {parts.map((part, i) => (
            <React.Fragment key={i}>
              {part}
              {i < parts.length - 1 && (
                <span style={{
                  color: C.amber, fontWeight: 700,
                  background: `${C.amber}18`, padding: "1px 3px", borderRadius: 3,
                }}>{hlWord}</span>
              )}
            </React.Fragment>
          ))}
        </div>
      </div>
    </div>
  );
};

// ═══════════════════════════════════════════════
// Total: 420 frames = 14 seconds @ 30fps
// ═══════════════════════════════════════════════
export const IntroVideo: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  return (
    <AbsoluteFill style={{ backgroundColor: C.bg, fontFamily: C.sans }}>

      {/* ══════ Scene 1: HOOK — Title (0-74) ═══════════════════════ */}
      <Sequence from={0} durationInFrames={75}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          ...fadeOut(frame, 60),
        }}>
          <Glow x="50%" y="45%" color={C.accent} size={900} opacity={0.2} />
          <Glow x="35%" y="55%" color={C.cyan} size={400} opacity={0.08} />

          {/* Logo mark */}
          <div style={{
            ...fadeUp(frame, 3),
            width: 80, height: 80, borderRadius: 22,
            background: `linear-gradient(135deg, ${C.accent}, ${C.accentLight})`,
            display: "flex", alignItems: "center", justifyContent: "center",
            fontSize: 42, fontWeight: 800, color: "#fff",
            boxShadow: `0 0 80px ${C.accentGlow}`,
            marginBottom: 32,
          }}>
            A
          </div>

          {/* Title */}
          <div style={fadeUp(frame, 8)}>
            <div style={{
              fontSize: 140, fontWeight: 800, color: C.text,
              letterSpacing: -5, lineHeight: 1,
            }}>
              Anything
            </div>
          </div>

          {/* One-line hook */}
          <div style={{
            ...fadeUp(frame, 20),
            marginTop: 24,
          }}>
            <div style={{
              fontSize: 36, color: C.textMuted, fontWeight: 400,
              letterSpacing: 0.5,
            }}>
              Local Document Search Engine
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 2: KILLING COPY (75-164) ═════════════════════ */}
      <Sequence from={75} durationInFrames={90}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          ...fadeOut(frame, 150),
        }}>
          <Glow x="50%" y="50%" color={C.cyan} size={800} opacity={0.15} />

          {/* Line 1 */}
          <div style={fadeUp(frame, 78)}>
            <div style={{
              fontSize: 72, fontWeight: 300, color: C.textDim,
              letterSpacing: -1,
            }}>
              파일명이 아닌,
            </div>
          </div>

          {/* Line 2 — the punch */}
          <div style={{
            ...fadeUp(frame, 92),
            marginTop: 8,
          }}>
            <div style={{
              fontSize: 86, fontWeight: 800, letterSpacing: -2,
            }}>
              <span style={{ color: C.text }}>문서 </span>
              <span style={{
                background: `linear-gradient(135deg, ${C.cyan}, ${C.accent})`,
                WebkitBackgroundClip: "text",
                WebkitTextFillColor: "transparent",
              }}>내용</span>
              <span style={{ color: C.text }}>을 검색합니다</span>
            </div>
          </div>

          {/* Supported formats inline */}
          <div style={{
            ...fadeUp(frame, 110),
            marginTop: 32, display: "flex", gap: 16,
          }}>
            {[".hwpx", ".hwp", ".docx", ".xlsx", ".pdf"].map((ext, i) => {
              const s = spring({
                frame: Math.max(0, frame - 112 - i * 3), fps,
                config: { damping: 14 },
              });
              return (
                <div key={ext} style={{
                  padding: "10px 22px", borderRadius: 10,
                  background: "rgba(255,255,255,0.06)",
                  border: "1px solid rgba(255,255,255,0.1)",
                  fontSize: 22, fontWeight: 700, color: C.textSub,
                  fontFamily: C.mono, opacity: s,
                  transform: `scale(${s})`,
                }}>
                  {ext}
                </div>
              );
            })}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 3: SEARCH DEMO (165-269) ═════════════════════ */}
      <Sequence from={165} durationInFrames={105}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          padding: "0 200px",
          ...fadeOut(frame, 255),
        }}>
          <Glow x="50%" y="35%" color={C.accent} size={700} opacity={0.12} />

          {/* Search mockup */}
          <div style={{
            width: "100%", maxWidth: 960,
            ...fadeUp(frame, 168),
          }}>
            {/* Search bar */}
            <div style={{
              background: "rgba(255,255,255,0.04)",
              border: `1px solid ${C.accent}50`,
              borderRadius: 16, padding: "20px 28px",
              display: "flex", alignItems: "center", gap: 14,
              marginBottom: 16,
            }}>
              <span style={{ fontSize: 24, color: C.textDim }}>{"\u{1F50D}"}</span>
              <span style={{ fontSize: 26, color: C.text, fontWeight: 500 }}>
                {(() => {
                  const q = "예산 집행 현황";
                  const c = Math.floor(ease(frame, 0, q.length, [175, 198]));
                  return q.substring(0, c);
                })()}
                <span style={{
                  opacity: frame > 172 && frame < 205 && frame % 16 < 8 ? 1 : 0,
                  color: C.accent, fontWeight: 300,
                }}>|</span>
              </span>
            </div>

            {/* Results */}
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <ResultRow
                icon={"\u{1F4C4}"} name="2026 예산집행현황.hwpx"
                snippet="...3분기 예산 집행률 87.3%로 전년 대비 12%p 상승..."
                hlWord="예산 집행" frame={frame} delay={202}
              />
              <ResultRow
                icon={"\u{1F4CA}"} name="1분기_집행실적.xlsx"
                snippet="...사업별 예산 집행 현황: 인건비 92%, 운영비 78%..."
                hlWord="예산 집행" frame={frame} delay={208}
              />
              <ResultRow
                icon={"\u{1F4D1}"} name="예산운용계획_보고.pdf"
                snippet="...차년도 예산 집행 계획 수립 시 전년도 집행률 기반 조정..."
                hlWord="예산 집행" frame={frame} delay={214}
              />
            </div>

            {/* Speed indicator */}
            <div style={{
              ...fadeUp(frame, 222),
              textAlign: "right", marginTop: 16,
            }}>
              <span style={{
                fontSize: 20, color: C.emerald, fontFamily: C.mono, fontWeight: 600,
              }}>
                3건 · 0.28초
              </span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 4: STATS — Numbers that hit (270-359) ════════ */}
      <Sequence from={270} durationInFrames={90}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          ...fadeOut(frame, 345),
        }}>
          <Glow x="30%" y="50%" color={C.accent} size={600} opacity={0.15} />
          <Glow x="70%" y="50%" color={C.cyan} size={600} opacity={0.12} />

          {/* Section label */}
          <div style={{
            ...fadeUp(frame, 273),
            marginBottom: 60,
          }}>
            <div style={{
              fontSize: 28, fontWeight: 600, color: C.accent,
              letterSpacing: 6, textTransform: "uppercase",
            }}>
              How it works
            </div>
          </div>

          {/* Stats row */}
          <div style={{
            display: "flex", gap: 120, alignItems: "flex-start",
          }}>
            <StatNumber value="768" label="차원 벡터 임베딩" color={C.cyan} frame={frame} delay={280} />

            {/* Divider */}
            <div style={{
              width: 1, height: 100, background: "rgba(255,255,255,0.08)",
              opacity: ease(frame, 0, 1, [286, 294]),
            }} />

            <StatNumber value="4단계" label="하이브리드 파이프라인" color={C.accent} frame={frame} delay={288} />

            <div style={{
              width: 1, height: 100, background: "rgba(255,255,255,0.08)",
              opacity: ease(frame, 0, 1, [294, 302]),
            }} />

            <StatNumber value="100%" label="로컬 · 오프라인" color={C.emerald} frame={frame} delay={296} />
          </div>

          {/* Pipeline steps */}
          <div style={{
            ...fadeUp(frame, 310),
            marginTop: 60, display: "flex", gap: 12, alignItems: "center",
          }}>
            {[
              { label: "Keyword", color: C.cyan },
              { label: "Semantic", color: C.accent },
              { label: "Hybrid RRF", color: C.emerald },
              { label: "Rerank", color: C.amber },
            ].map((step, i) => (
              <React.Fragment key={step.label}>
                {i > 0 && (
                  <div style={{
                    fontSize: 20, color: C.textDim, margin: "0 4px",
                  }}>{"\u{2192}"}</div>
                )}
                <div style={{
                  padding: "10px 24px", borderRadius: 10,
                  background: `${step.color}12`,
                  border: `1px solid ${step.color}30`,
                  fontSize: 20, fontWeight: 700, color: step.color,
                  fontFamily: C.mono,
                }}>
                  {step.label}
                </div>
              </React.Fragment>
            ))}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 5: CTA (360-419) ═════════════════════════════ */}
      <Sequence from={360} durationInFrames={60}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
        }}>
          <Glow x="50%" y="45%" color={C.accent} size={1000} opacity={0.25} />
          <Glow x="45%" y="55%" color={C.cyan} size={500} opacity={0.1} />

          {/* Final title */}
          <div style={fadeUp(frame, 363)}>
            <div style={{
              fontSize: 80, fontWeight: 800, color: C.text,
              letterSpacing: -3, textAlign: "center",
            }}>
              찾고 싶은 건,{"\n"}
              <span style={{
                background: `linear-gradient(135deg, ${C.accent}, ${C.cyan})`,
                WebkitBackgroundClip: "text",
                WebkitTextFillColor: "transparent",
              }}>Anything</span>
            </div>
          </div>

          {/* Download CTA */}
          <div style={{
            ...fadeUp(frame, 378),
            marginTop: 40,
          }}>
            <div style={{
              display: "inline-flex", alignItems: "center", gap: 14,
              padding: "22px 52px", borderRadius: 16,
              background: `linear-gradient(135deg, ${C.accent}, ${C.accentLight})`,
              boxShadow: `0 0 80px ${C.accentGlow}`,
              fontSize: 32, fontWeight: 700, color: "#fff",
              transform: `scale(${spring({
                frame: Math.max(0, frame - 380), fps,
                config: { damping: 12 },
              })})`,
            }}>
              Download for Windows
            </div>
          </div>

          {/* Bottom info */}
          <div style={{
            ...fadeUp(frame, 392),
            marginTop: 32,
          }}>
            <div style={{
              display: "flex", gap: 28, alignItems: "center",
              fontSize: 20, color: C.textDim,
            }}>
              <span>Windows 10/11</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>100% 오프라인</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>MIT License</span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

    </AbsoluteFill>
  );
};
