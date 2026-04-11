import React from "react";
import {
  AbsoluteFill,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
  Sequence,
  Easing,
  Img,
  staticFile,
} from "remotion";

// ─── Design tokens (matching actual app branding) ───
const C = {
  bg: "#0A0A0A",
  surface: "rgba(255,255,255,0.04)",
  border: "rgba(255,255,255,0.06)",
  // Brand green from the app icon
  brand: "#2AB573",
  brandLight: "#34D399",
  brandGlow: "rgba(42,181,115,0.35)",
  brandDark: "#1E9960",
  // Secondary
  slate: "#64748B",
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

// ─── App icon (original PNG) ───
const AppIcon: React.FC<{ size?: number; frame: number }> = ({ size = 120, frame }) => {
  const s = spring({ frame, fps: 30, config: { damping: 12, mass: 0.8 } });
  return (
    <div style={{
      width: size, height: size,
      transform: `scale(${s})`, opacity: s,
    }}>
      <Img
        src={staticFile("icon.png")}
        style={{ width: size, height: size, objectFit: "contain" }}
      />
    </div>
  );
};

// ─── Stat number ───
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
        fontSize: 88, fontWeight: 800, color,
        fontFamily: C.sans, letterSpacing: -3, lineHeight: 1,
      }}>
        {value}
      </div>
      <div style={{
        fontSize: 22, color: C.textMuted, marginTop: 12,
        fontWeight: 500, fontFamily: C.sans, letterSpacing: 1,
      }}>
        {label}
      </div>
    </div>
  );
};

// ─── Search result row ───
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
      border: "1px solid rgba(255,255,255,0.06)",
      opacity: o, transform: `translateX(${x}px)`, fontFamily: C.sans,
    }}>
      <span style={{ fontSize: 26 }}>{icon}</span>
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 22, fontWeight: 600, color: C.text }}>{name}</div>
        <div style={{ fontSize: 17, color: C.textMuted, marginTop: 4, lineHeight: 1.4 }}>
          {parts.map((part, i) => (
            <React.Fragment key={i}>
              {part}
              {i < parts.length - 1 && (
                <span style={{
                  color: C.brand, fontWeight: 700,
                  background: `${C.brand}18`, padding: "1px 3px", borderRadius: 3,
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
// Total: 450 frames = 15 seconds @ 30fps
// ═══════════════════════════════════════════════
export const IntroVideo: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  return (
    <AbsoluteFill style={{ backgroundColor: C.bg, fontFamily: C.sans }}>

      {/* ══════ Scene 1: BRAND INTRO (0-89) ════════════════════════ */}
      <Sequence from={0} durationInFrames={90}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          ...fadeOut(frame, 75),
        }}>
          <Glow x="50%" y="42%" color={C.brand} size={900} opacity={0.15} />
          <Glow x="60%" y="55%" color={C.brandDark} size={500} opacity={0.08} />

          {/* App icon */}
          <div style={{ marginBottom: 36 }}>
            <AppIcon size={130} frame={Math.max(0, frame - 3)} />
          </div>

          {/* Title */}
          <div style={fadeUp(frame, 10)}>
            <div style={{
              fontSize: 130, fontWeight: 900, color: C.text,
              letterSpacing: -4, lineHeight: 1,
            }}>
              Anything<span style={{ color: C.brand }}>.</span>
            </div>
          </div>

          {/* Tagline — matching actual branding */}
          <div style={{ ...fadeUp(frame, 25), marginTop: 20 }}>
            <div style={{ fontSize: 38, fontWeight: 400, letterSpacing: 1 }}>
              <span style={{ color: C.textMuted }}>AI, Everything, </span>
              <span style={{ color: C.brand, fontWeight: 700 }}>Anything.</span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 2: KILLING COPY (90-194) ═════════════════════ */}
      <Sequence from={90} durationInFrames={105}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          ...fadeOut(frame, 180),
        }}>
          <Glow x="50%" y="48%" color={C.brand} size={800} opacity={0.12} />

          {/* Line 1 — light */}
          <div style={fadeUp(frame, 93)}>
            <div style={{
              fontSize: 68, fontWeight: 300, color: C.textDim,
              letterSpacing: -1,
            }}>
              파일명이 아닌,
            </div>
          </div>

          {/* Line 2 — the punch */}
          <div style={{ ...fadeUp(frame, 108), marginTop: 8 }}>
            <div style={{ fontSize: 84, fontWeight: 800, letterSpacing: -2 }}>
              <span style={{ color: C.text }}>문서 </span>
              <span style={{
                color: C.brand,
                textShadow: `0 0 60px ${C.brandGlow}`,
              }}>내용</span>
              <span style={{ color: C.text }}>을 검색합니다</span>
            </div>
          </div>

          {/* Format badges */}
          <div style={{ ...fadeUp(frame, 128), marginTop: 36, display: "flex", gap: 14 }}>
            {[".hwpx", ".hwp", ".docx", ".xlsx", ".pdf"].map((ext, i) => {
              const s = spring({
                frame: Math.max(0, frame - 130 - i * 3), fps,
                config: { damping: 14 },
              });
              return (
                <div key={ext} style={{
                  padding: "10px 24px", borderRadius: 10,
                  background: "rgba(255,255,255,0.05)",
                  border: "1px solid rgba(255,255,255,0.1)",
                  fontSize: 22, fontWeight: 700, color: C.textSub,
                  fontFamily: C.mono, opacity: s, transform: `scale(${s})`,
                }}>
                  {ext}
                </div>
              );
            })}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 3: SEARCH DEMO (195-314) ═════════════════════ */}
      <Sequence from={195} durationInFrames={120}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          padding: "0 200px",
          ...fadeOut(frame, 300),
        }}>
          <Glow x="50%" y="35%" color={C.brand} size={700} opacity={0.1} />

          {/* Search mockup */}
          <div style={{ width: "100%", maxWidth: 960, ...fadeUp(frame, 198) }}>
            {/* Search bar */}
            <div style={{
              background: "rgba(255,255,255,0.04)",
              border: `1px solid ${C.brand}50`,
              borderRadius: 16, padding: "20px 28px",
              display: "flex", alignItems: "center", gap: 14,
              marginBottom: 16,
            }}>
              <span style={{ fontSize: 24, color: C.brand }}>{"\u{1F50D}"}</span>
              <span style={{ fontSize: 26, color: C.text, fontWeight: 500 }}>
                {(() => {
                  const q = "예산 집행 현황";
                  const c = Math.floor(ease(frame, 0, q.length, [205, 230]));
                  return q.substring(0, c);
                })()}
                <span style={{
                  opacity: frame > 202 && frame < 238 && frame % 16 < 8 ? 1 : 0,
                  color: C.brand, fontWeight: 300,
                }}>|</span>
              </span>
            </div>

            {/* Results */}
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <ResultRow
                icon={"\u{1F4C4}"} name="2026 예산집행현황.hwpx"
                snippet="...3분기 예산 집행률 87.3%로 전년 대비 12%p 상승..."
                hlWord="예산 집행" frame={frame} delay={235}
              />
              <ResultRow
                icon={"\u{1F4CA}"} name="1분기_집행실적.xlsx"
                snippet="...사업별 예산 집행 현황: 인건비 92%, 운영비 78%..."
                hlWord="예산 집행" frame={frame} delay={242}
              />
              <ResultRow
                icon={"\u{1F4D1}"} name="예산운용계획_보고.pdf"
                snippet="...차년도 예산 집행 계획 수립 시 전년도 집행률 기반 조정..."
                hlWord="예산 집행" frame={frame} delay={249}
              />
            </div>

            {/* Speed */}
            <div style={{ ...fadeUp(frame, 258), textAlign: "right", marginTop: 16 }}>
              <span style={{
                fontSize: 20, color: C.brand, fontFamily: C.mono, fontWeight: 600,
              }}>
                3건 · 0.28초
              </span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 4: STATS (315-404) ═══════════════════════════ */}
      <Sequence from={315} durationInFrames={90}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          ...fadeOut(frame, 390),
        }}>
          <Glow x="30%" y="50%" color={C.brand} size={600} opacity={0.12} />
          <Glow x="70%" y="50%" color={C.brandDark} size={600} opacity={0.08} />

          {/* Section label */}
          <div style={{ ...fadeUp(frame, 318), marginBottom: 56 }}>
            <div style={{
              fontSize: 26, fontWeight: 600, color: C.brand,
              letterSpacing: 6, textTransform: "uppercase",
            }}>
              How it works
            </div>
          </div>

          {/* Stats */}
          <div style={{ display: "flex", gap: 110, alignItems: "flex-start" }}>
            <StatNumber value="768" label="Dimension Embeddings" color={C.brand} frame={frame} delay={325} />
            <div style={{
              width: 1, height: 90, background: "rgba(255,255,255,0.08)",
              opacity: ease(frame, 0, 1, [330, 338]),
            }} />
            <StatNumber value="4-Stage" label="Hybrid Pipeline" color={C.brandLight} frame={frame} delay={333} />
            <div style={{
              width: 1, height: 90, background: "rgba(255,255,255,0.08)",
              opacity: ease(frame, 0, 1, [338, 346]),
            }} />
            <StatNumber value="100%" label="Local · Offline" color={C.text} frame={frame} delay={341} />
          </div>

          {/* Pipeline flow */}
          <div style={{
            ...fadeUp(frame, 355), marginTop: 56,
            display: "flex", gap: 10, alignItems: "center",
          }}>
            {[
              { label: "Keyword", sub: "FTS5" },
              { label: "Semantic", sub: "KoSimCSE" },
              { label: "Hybrid", sub: "RRF" },
              { label: "Rerank", sub: "Cross-Encoder" },
            ].map((step, i) => (
              <React.Fragment key={step.label}>
                {i > 0 && (
                  <div style={{ fontSize: 18, color: C.textDim, margin: "0 2px" }}>{"\u{2192}"}</div>
                )}
                <div style={{
                  padding: "10px 22px", borderRadius: 10,
                  background: `${C.brand}10`,
                  border: `1px solid ${C.brand}25`,
                  textAlign: "center",
                }}>
                  <div style={{
                    fontSize: 19, fontWeight: 700, color: C.brand, fontFamily: C.mono,
                  }}>{step.label}</div>
                  <div style={{
                    fontSize: 14, color: C.textDim, marginTop: 2, fontFamily: C.mono,
                  }}>{step.sub}</div>
                </div>
              </React.Fragment>
            ))}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ══════ Scene 5: CTA (405-479) ═════════════════════════════ */}
      <Sequence from={405} durationInFrames={75}>
        <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
          <Glow x="50%" y="42%" color={C.brand} size={1000} opacity={0.2} />
          <Glow x="45%" y="58%" color={C.brandDark} size={600} opacity={0.08} />

          {/* Icon small */}
          <div style={{ marginBottom: 24 }}>
            <AppIcon size={80} frame={Math.max(0, frame - 408)} />
          </div>

          {/* Final punch */}
          <div style={fadeUp(frame, 412)}>
            <div style={{
              fontSize: 76, fontWeight: 800, color: C.text,
              letterSpacing: -3, textAlign: "center", lineHeight: 1.15,
            }}>
              찾고 싶은 건,
              <br />
              <span style={{
                color: C.brand,
                textShadow: `0 0 80px ${C.brandGlow}`,
              }}>Anything.</span>
            </div>
          </div>

          {/* Download CTA */}
          <div style={{ ...fadeUp(frame, 428), marginTop: 40 }}>
            <div style={{
              display: "inline-flex", alignItems: "center", gap: 14,
              padding: "22px 52px", borderRadius: 16,
              background: C.brand,
              boxShadow: `0 0 80px ${C.brandGlow}`,
              fontSize: 30, fontWeight: 700, color: "#fff",
              transform: `scale(${spring({
                frame: Math.max(0, frame - 430), fps,
                config: { damping: 12 },
              })})`,
            }}>
              Download for Windows
            </div>
          </div>

          {/* Bottom info */}
          <div style={{ ...fadeUp(frame, 442), marginTop: 28 }}>
            <div style={{
              display: "flex", gap: 28, alignItems: "center",
              fontSize: 20, color: C.textDim,
            }}>
              <span>Windows 10/11</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>100% Offline</span>
              <span style={{ color: C.border }}>{"\u{2022}"}</span>
              <span>Open Source</span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

    </AbsoluteFill>
  );
};
