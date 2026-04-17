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

// ─── Tokens ───
const C = {
  bg: "#050505",
  panel: "rgba(255,255,255,0.035)",
  panelBorder: "rgba(255,255,255,0.07)",
  text: "#FFFFFF",
  textSub: "#E2E8F0",
  textMuted: "#94A3B8",
  textDim: "#475569",
  brand: "#2AB573",
  brandLight: "#34D399",
  brandGlow: "rgba(42,181,115,0.45)",
  brandDark: "#1E9960",
  danger: "#F87171",
  dangerGlow: "rgba(248,113,113,0.3)",
  sans: "'Inter', 'Pretendard', -apple-system, sans-serif",
  mono: "'SF Mono', 'Consolas', 'Fira Code', monospace",
};

// ─── Easing — Apple-ish ───
const E_OUT = Easing.bezier(0.16, 1, 0.3, 1);
const E_IN = Easing.bezier(0.7, 0, 0.84, 0);
const E_IO = Easing.bezier(0.83, 0, 0.17, 1);
const E_EXPO = Easing.bezier(0.19, 1, 0.22, 1);

const ease = (
  f: number, from: number, to: number,
  r: [number, number], e = E_OUT
) => interpolate(f, r, [from, to], {
  extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: e,
});

// ═══════════════════════════════════════════════════
// EFFECT LAYERS — quiet, cinematic
// ═══════════════════════════════════════════════════

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

// Soft vignette — corners darker for cinematic depth
const Vignette: React.FC = () => (
  <div style={{
    position: "absolute", inset: 0, pointerEvents: "none",
    background: "radial-gradient(ellipse at center, transparent 55%, rgba(0,0,0,0.55) 100%)",
  }} />
);

// Film grain — very subtle
const Grain: React.FC = () => (
  <div style={{
    position: "absolute", inset: 0, pointerEvents: "none",
    opacity: 0.04, mixBlendMode: "overlay",
    backgroundImage: "radial-gradient(rgba(255,255,255,0.7) 1px, transparent 1px)",
    backgroundSize: "3px 3px",
  }} />
);

// Light sweep — a single wide band of brand-colored light
// slides diagonally once across the screen. Used as a transition.
const LightSweep: React.FC<{
  frame: number; startFrame: number; duration?: number;
  color?: string; width?: number; angle?: number;
}> = ({ frame, startFrame, duration = 22, color = C.brand, width = 420, angle = -18 }) => {
  if (frame < startFrame - 1 || frame > startFrame + duration + 2) return null;
  const travel = 3600;
  const x = ease(frame, -travel / 2, travel / 2, [startFrame, startFrame + duration], E_EXPO);
  const op = ease(frame, 0, 0.9, [startFrame, startFrame + 4])
    * ease(frame, 1, 0, [startFrame + duration - 8, startFrame + duration], E_IN);
  return (
    <AbsoluteFill style={{ pointerEvents: "none", mixBlendMode: "screen", overflow: "hidden" }}>
      <div style={{
        position: "absolute",
        left: "50%", top: "50%",
        width: width, height: 1800,
        marginLeft: -width / 2, marginTop: -900,
        transform: `translateX(${x}px) rotate(${angle}deg)`,
        background: `linear-gradient(90deg, transparent 0%, ${color} 45%, rgba(255,255,255,0.95) 50%, ${color} 55%, transparent 100%)`,
        filter: "blur(24px)",
        opacity: op,
      }} />
    </AbsoluteFill>
  );
};

// Single shockwave ring — used once for brand moment
const Shockwave: React.FC<{
  frame: number; startFrame: number; x?: string; y?: string;
  color?: string; max?: number; dur?: number;
}> = ({ frame, startFrame, x = "50%", y = "50%", color = C.brand, max = 1800, dur = 34 }) => (
  <AbsoluteFill style={{ pointerEvents: "none" }}>
    {[0, 7].map((delay, i) => {
      const s = startFrame + delay;
      const size = ease(frame, 0, max, [s, s + dur], E_EXPO);
      const op = ease(frame, 0.55, 0, [s, s + dur], E_IN);
      return (
        <div key={i} style={{
          position: "absolute", left: x, top: y,
          width: size, height: size, borderRadius: "50%",
          border: `${3 - i}px solid ${color}`,
          transform: "translate(-50%, -50%)",
          opacity: op,
          boxShadow: `0 0 30px ${color}`,
        }} />
      );
    })}
  </AbsoluteFill>
);

// Gentle scene-level flash — no blinding whiteout, just a breath of light
const BreathFlash: React.FC<{
  frame: number; startFrame: number; color?: string; peak?: number; dur?: number;
}> = ({ frame, startFrame, color = C.brand, peak = 0.25, dur = 18 }) => {
  const half = Math.floor(dur / 2);
  const up = ease(frame, 0, peak, [startFrame, startFrame + half], E_OUT);
  const down = ease(frame, 1, 0, [startFrame + half, startFrame + dur], E_IO);
  return (
    <AbsoluteFill style={{
      background: color, opacity: up * down,
      pointerEvents: "none", mixBlendMode: "screen",
    }} />
  );
};

// ═══════════════════════════════════════════════════
// UI ATOMS
// ═══════════════════════════════════════════════════

const SearchBar: React.FC<{
  query: string; typedFromFrame: number; typedToFrame: number; frame: number;
  borderColor: string; glowColor: string;
}> = ({ query, typedFromFrame, typedToFrame, frame, borderColor, glowColor }) => {
  const count = Math.floor(
    ease(frame, 0, query.length, [typedFromFrame, typedToFrame], Easing.linear)
  );
  const blink = frame > typedFromFrame - 4 && frame < typedToFrame + 20 && frame % 14 < 7;

  return (
    <div style={{
      background: "rgba(255,255,255,0.025)",
      border: `1.5px solid ${borderColor}`,
      borderRadius: 22,
      padding: "28px 36px",
      display: "flex", alignItems: "center", gap: 20,
      boxShadow: `0 0 120px ${glowColor}`,
      backdropFilter: "blur(20px)",
    }}>
      <span style={{ fontSize: 32, color: borderColor }}>{"\u{1F50D}"}</span>
      <span style={{
        fontSize: 38, color: C.text, fontWeight: 500,
        letterSpacing: -0.5, fontFamily: C.sans,
      }}>
        {query.substring(0, count)}
        <span style={{
          opacity: blink ? 1 : 0,
          color: borderColor, fontWeight: 300, marginLeft: 2,
        }}>|</span>
      </span>
    </div>
  );
};

const ResultCard: React.FC<{
  icon: string; name: string; body: React.ReactNode; meta: string;
  frame: number; delay: number; emphasize?: boolean;
}> = ({ icon, name, body, meta, frame, delay, emphasize = false }) => {
  const p = ease(frame, 0, 1, [delay, delay + 18], E_EXPO);
  const blur = ease(frame, 6, 0, [delay, delay + 14]);

  return (
    <div style={{
      padding: "24px 30px",
      borderRadius: 20,
      background: emphasize
        ? `linear-gradient(135deg, ${C.brand}22, ${C.brand}05)`
        : "rgba(255,255,255,0.04)",
      border: `1px solid ${emphasize ? `${C.brand}40` : C.panelBorder}`,
      opacity: p,
      transform: `translateY(${(1 - p) * 34}px)`,
      filter: `blur(${blur}px)`,
      boxShadow: emphasize ? `0 24px 80px ${C.brandGlow}` : "none",
      fontFamily: C.sans,
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 20 }}>
        <span style={{ fontSize: 36 }}>{icon}</span>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{
            fontSize: 26, fontWeight: 700, color: C.text,
            letterSpacing: -0.3, whiteSpace: "nowrap",
            overflow: "hidden", textOverflow: "ellipsis",
          }}>
            {name}
          </div>
          <div style={{
            fontSize: 21, color: C.textMuted,
            marginTop: 8, lineHeight: 1.6,
          }}>
            {body}
          </div>
        </div>
        <div style={{
          fontSize: 15, color: C.brand, fontFamily: C.mono, fontWeight: 600,
          opacity: ease(frame, 0, 1, [delay + 8, delay + 18]),
          whiteSpace: "nowrap",
        }}>
          {meta}
        </div>
      </div>
    </div>
  );
};

// Keyword highlight — lights up slowly, no shake, gentle glow breath
const HL: React.FC<{ word: string; frame: number; igniteAt: number }> = ({ word, frame, igniteAt }) => {
  const lit = ease(frame, 0, 1, [igniteAt, igniteAt + 10], E_EXPO);
  return (
    <span style={{
      color: lit > 0.15 ? C.brand : C.textSub,
      fontWeight: 700,
      background: `rgba(42,181,115,${0.08 + lit * 0.22})`,
      padding: "2px 8px",
      borderRadius: 6,
      boxShadow: `0 0 ${lit * 22}px ${C.brandGlow}`,
      display: "inline-block",
    }}>{word}</span>
  );
};

// ═══════════════════════════════════════════════════
// MAIN — 10s @ 30fps = 300 frames · 1920×1080
//
//  ACT I   0.0–1.7s (0–50)    Before: filename search fails
//  TRANS   1.7–2.1s (50–62)   Single light sweep (brand green)
//  ACT II  2.1–6.3s (62–190)  Content search reveal  ★ 4.2s
//  BREATH  6.3–6.7s (190–200) Soft cross-fade + breath flash
//  ACT III 6.7–7.8s (200–235) Punchline typography
//  ACT IV  7.8–9.0s (235–270) Brand reveal (1× shockwave)
//  ACT V   9.0–10.0s (270–300) Tagline + gentle fade
// ═══════════════════════════════════════════════════
export const Prologue: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const globalIntro = ease(frame, 0, 1, [0, 8]);

  return (
    <AbsoluteFill style={{
      backgroundColor: C.bg,
      fontFamily: C.sans,
      opacity: globalIntro,
    }}>

      {/* ═══ ACT I · Before — filename search fails (0-50) ═══ */}
      <Sequence from={0} durationInFrames={56}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          padding: "0 220px",
          opacity: ease(frame, 0, 1, [2, 12]) * ease(frame, 1, 0, [48, 58], E_IN),
          transform: `translateY(${ease(frame, 0, -10, [48, 58], E_IN)}px)`,
        }}>
          <Glow x="50%" y="50%" color={C.danger} size={900} opacity={0.08} />

          <div style={{ width: "100%", maxWidth: 980 }}>
            <div style={{
              fontSize: 16, color: C.textDim, fontFamily: C.mono,
              letterSpacing: 3, textTransform: "uppercase",
              marginBottom: 20,
              opacity: ease(frame, 0, 1, [4, 14]),
            }}>
              Filename search
            </div>

            <SearchBar
              query="계약 해지 조건"
              typedFromFrame={8} typedToFrame={32}
              frame={frame}
              borderColor={`${C.danger}80`}
              glowColor={C.dangerGlow}
            />

            <div style={{
              marginTop: 22,
              padding: "22px 30px",
              borderRadius: 16,
              background: `${C.danger}0d`,
              border: `1px dashed ${C.danger}44`,
              opacity: ease(frame, 0, 1, [34, 44]),
              transform: `translateY(${ease(frame, 14, 0, [34, 44])}px)`,
              display: "flex", alignItems: "center", gap: 16,
            }}>
              <span style={{ fontSize: 26 }}>{"\u{26A0}"}</span>
              <span style={{
                fontSize: 22, color: C.danger, fontWeight: 600,
              }}>
                0 matches — 파일명에 해당 키워드가 없습니다.
              </span>
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ═══ TRANSITION · single light sweep (50-62) ═══ */}
      <LightSweep frame={frame} startFrame={50} duration={22} width={500} angle={-16} />
      <BreathFlash frame={frame} startFrame={56} color={C.brand} peak={0.18} dur={14} />

      {/* ═══ ACT II · Content search reveal (62-190) ═══ */}
      <Sequence from={60} durationInFrames={134}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          padding: "0 180px",
          opacity: ease(frame, 0, 1, [62, 74]) * ease(frame, 1, 0, [184, 194], E_IN),
          transform: `translateY(${ease(frame, 0, -12, [184, 194], E_IN)}px)`,
        }}>
          <Glow x="50%" y="28%" color={C.brand} size={1100} opacity={0.18} />
          <Glow x="50%" y="74%" color={C.brandDark} size={700} opacity={0.08} />

          <div style={{ width: "100%", maxWidth: 1080 }}>
            <div style={{
              fontSize: 16, color: C.brand, fontFamily: C.mono,
              letterSpacing: 3, textTransform: "uppercase",
              marginBottom: 20,
              opacity: ease(frame, 0, 1, [64, 76]),
              transform: `translateY(${ease(frame, 14, 0, [64, 76])}px)`,
            }}>
              Content search · AI
            </div>

            <div style={{
              opacity: ease(frame, 0, 1, [66, 78]),
              transform: `scale(${ease(frame, 0.97, 1, [66, 88], E_EXPO)})`,
            }}>
              <SearchBar
                query="계약 해지 조건"
                typedFromFrame={72} typedToFrame={98}
                frame={frame}
                borderColor={`${C.brand}aa`}
                glowColor={C.brandGlow}
              />
            </div>

            <div style={{
              display: "flex", justifyContent: "space-between", alignItems: "center",
              marginTop: 20, marginBottom: 16,
              opacity: ease(frame, 0, 1, [102, 114]),
            }}>
              <span style={{ fontSize: 19, color: C.textMuted, fontFamily: C.mono }}>
                <span style={{ color: C.brand, fontWeight: 700 }}>3</span> matches in
                <span style={{ color: C.text, fontWeight: 600 }}> 문서 본문</span>
              </span>
              <span style={{
                fontSize: 19, color: C.brand, fontFamily: C.mono, fontWeight: 600,
              }}>
                0.19s · Hybrid (FTS5 + KoSimCSE)
              </span>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              <ResultCard
                icon={"\u{1F4C4}"}
                name="표준_업무협약서_v3.hwpx"
                body={<>...제12조 <HL word="계약 해지" frame={frame} igniteAt={118} />는 상호 협의 후 <span style={{ color: C.textSub }}>30일 이내</span>에 서면으로 통지하여야 한다...</>}
                meta="p.4"
                frame={frame}
                delay={108}
                emphasize
              />
              <ResultCard
                icon={"\u{1F4D1}"}
                name="2026_용역계약_체결_내역.pdf"
                body={<>...<HL word="해지 조건" frame={frame} igniteAt={140} /> 미충족 시 손해배상액은 총 계약금의 10%로 산정한다...</>}
                meta="p.17"
                frame={frame}
                delay={130}
              />
              <ResultCard
                icon={"\u{1F4CA}"}
                name="법무팀_검토의견_2025Q4.docx"
                body={<>...민법 제543조에 따른 <HL word="계약 해지" frame={frame} igniteAt={162} />권 행사 시 유의사항 정리...</>}
                meta="p.2"
                frame={frame}
                delay={152}
              />
            </div>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ═══ BREATH · soft cross-fade (190-200) ═══ */}
      <BreathFlash frame={frame} startFrame={190} color={C.brand} peak={0.15} dur={14} />

      {/* ═══ ACT III · Punchline — quiet, monumental (200-235) ═══ */}
      <Sequence from={198} durationInFrames={40}>
        <AbsoluteFill style={{
          justifyContent: "center", alignItems: "center",
          opacity: ease(frame, 0, 1, [200, 210]) * ease(frame, 1, 0, [228, 236], E_IN),
        }}>
          <Glow x="50%" y="50%" color={C.brand} size={1400} opacity={0.14} />

          <div style={{
            fontSize: 60, fontWeight: 300, color: C.textMuted,
            letterSpacing: -1.5,
            opacity: ease(frame, 0, 1, [202, 212]) * ease(frame, 1, 0.35, [226, 234], E_IN),
            transform: `translateY(${ease(frame, 16, 0, [202, 214], E_EXPO)}px)`,
          }}>
            파일명이 아닌,
          </div>

          <div style={{
            fontSize: 116, fontWeight: 800, letterSpacing: -4, lineHeight: 1.05,
            marginTop: 6,
            opacity: ease(frame, 0, 1, [210, 220]),
            transform: `translateY(${ease(frame, 22, 0, [210, 224], E_EXPO)}px) scale(${ease(frame, 0.96, 1, [210, 230], E_EXPO)})`,
          }}>
            <span style={{ color: C.text }}>문서의 </span>
            <span style={{
              color: C.brand,
              textShadow: `0 0 ${56 + Math.sin((frame - 214) / 5) * 14}px ${C.brandGlow}`,
            }}>속</span>
            <span style={{ color: C.text }}>을 읽습니다.</span>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* ═══ LIGHT SWEEP · transition into brand (228-250) ═══ */}
      <LightSweep frame={frame} startFrame={230} duration={22} width={560} angle={-18} />

      {/* ═══ ACT IV · Brand reveal — single shockwave impact (234-270) ═══ */}
      <Sequence from={232} durationInFrames={42}>
        <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
          <Glow x="50%" y="46%" color={C.brand} size={1500} opacity={0.25} />
          <Glow x="50%" y="62%" color={C.brandDark} size={800} opacity={0.1} />

          {/* App icon — gentle spring drop */}
          <div style={{
            marginBottom: 28,
            transform: `scale(${spring({ frame: Math.max(0, frame - 244), fps, config: { damping: 14, mass: 0.8, stiffness: 160 } })})`,
            opacity: ease(frame, 0, 1, [244, 254]),
          }}>
            <Img src={staticFile("icon.png")} style={{
              width: 120, height: 120, objectFit: "contain",
              filter: `drop-shadow(0 0 30px ${C.brandGlow})`,
            }} />
          </div>

          {/* Wordmark — clip-path wipe from left */}
          <div style={{
            fontSize: 192, fontWeight: 900, color: C.text,
            letterSpacing: -7, lineHeight: 1,
            clipPath: `inset(0 ${ease(frame, 100, 0, [248, 272], E_EXPO)}% 0 0)`,
            opacity: ease(frame, 0, 1, [246, 256]),
            textShadow: `0 0 70px rgba(255,255,255,0.2)`,
          }}>
            Anything<span style={{
              color: C.brand,
              textShadow: `0 0 ${32 + Math.sin((frame - 260) / 4) * 12}px ${C.brandGlow}`,
            }}>.</span>
          </div>

          {/* Underline sweep */}
          <div style={{
            height: 2,
            width: ease(frame, 0, 540, [258, 276], E_EXPO),
            background: `linear-gradient(90deg, transparent, ${C.brand}, transparent)`,
            marginTop: 16,
            opacity: ease(frame, 0, 1, [258, 266]),
            boxShadow: `0 0 20px ${C.brandGlow}`,
          }} />
        </AbsoluteFill>

        {/* THE moment — single shockwave as the wordmark lands */}
        <Shockwave frame={frame} startFrame={248} color={C.brand} max={1700} dur={36} />
        <BreathFlash frame={frame} startFrame={248} color={C.brand} peak={0.22} dur={20} />
      </Sequence>

      {/* ═══ ACT V · Tagline + gentle fade (270-300) ═══ */}
      <Sequence from={268} durationInFrames={32}>
        <AbsoluteFill style={{
          justifyContent: "flex-end", alignItems: "center",
          paddingBottom: 130,
        }}>
          <div style={{
            fontSize: 34, fontWeight: 500, letterSpacing: 2,
            opacity: ease(frame, 0, 1, [270, 282])
                    * ease(frame, 1, 0, [294, 300], E_IN),
            transform: `translateY(${ease(frame, 14, 0, [270, 282], E_EXPO)}px)`,
          }}>
            <span style={{ color: C.textMuted }}>AI, Everything, </span>
            <span style={{ color: C.brand, fontWeight: 700 }}>Anything.</span>
          </div>
        </AbsoluteFill>
      </Sequence>

      <Vignette />
      <Grain />
    </AbsoluteFill>
  );
};
