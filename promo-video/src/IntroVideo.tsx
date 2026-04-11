import React from 'react';
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig, Sequence } from 'remotion';

export const IntroVideo: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Scene 1 animations
  const titleOpacity = interpolate(frame, [0, 15], [0, 1], { extrapolateRight: 'clamp' });
  const titleY = interpolate(frame, [0, 15], [50, 0], { extrapolateRight: 'clamp' });

  const subtitleProgress = spring({
    frame: frame - 15,
    fps,
    config: { damping: 200 },
  });

  const fullText = "내 컴퓨터의 모든 문서를 한 번에 검색";
  const displayedCharacters = Math.floor(
    interpolate(frame, [30, 85], [0, fullText.length], { extrapolateRight: 'clamp' })
  );

  return (
    <AbsoluteFill style={{ backgroundColor: '#0F172A', color: '#E2E8F0', fontFamily: 'sans-serif' }}>

      {/* Scene 1: Title & Hero */}
      <Sequence from={0} durationInFrames={90}>
        <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center' }}>
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              transform: `translateY(${titleY}px)`,
              opacity: titleOpacity,
            }}
          >
            <h1 style={{ fontSize: 110, fontWeight: 'bold', margin: 0, color: '#F8FAFC' }}>
              Anything{' '}
              <span style={{ color: '#38BDF8' }}>v2.1.0</span>
            </h1>
            <h2 style={{ fontSize: 48, fontWeight: 'normal', margin: '24px 0 0 0', height: 60 }}>
              {fullText.substring(0, displayedCharacters)}
              <span style={{ opacity: frame % 15 < 7 ? 1 : 0, color: '#38BDF8' }}>|</span>
            </h2>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* Scene 2: Search Demo */}
      <Sequence from={90} durationInFrames={150}>
        <AbsoluteFill style={{ alignItems: 'center', justifyContent: 'center' }}>
          <div
            style={{
              fontSize: 56,
              fontWeight: 'bold',
              opacity: interpolate(frame, [90, 105], [0, 1], { extrapolateRight: 'clamp' }),
            }}
          >
            키워드 + AI 시맨틱 + RAG 질의응답
          </div>

          <div
            style={{
              marginTop: 40,
              backgroundColor: '#1E293B',
              padding: 40,
              borderRadius: 20,
              border: '2px solid #334155',
              width: '80%',
              opacity: interpolate(frame, [110, 125], [0, 1], { extrapolateRight: 'clamp' }),
              transform: `translateY(${interpolate(frame, [110, 125], [50, 0], { extrapolateRight: 'clamp' })}px)`,
            }}
          >
            <div style={{ fontSize: 36, color: '#94A3B8', marginBottom: 16 }}>검색</div>
            <div style={{ fontSize: 48, color: '#F8FAFC' }}>
              {(() => {
                const query = "예산 집행 현황 보고서";
                const chars = Math.floor(interpolate(frame, [130, 160], [0, query.length], { extrapolateRight: 'clamp' }));
                return query.substring(0, chars);
              })()}
              <span style={{ opacity: frame > 125 && frame < 170 && frame % 15 < 7 ? 1 : 0, color: '#38BDF8' }}>|</span>
            </div>

            <Sequence from={50}>
              <div
                style={{
                  marginTop: 30,
                  opacity: interpolate(frame, [170, 185], [0, 1], { extrapolateRight: 'clamp' }),
                }}
              >
                <div style={{ fontSize: 36, color: '#38BDF8', marginBottom: 12 }}>
                  FTS5 키워드 + KoSimCSE 벡터 + RRF 병합 + Cross-Encoder 재정렬
                </div>
                <div style={{ fontSize: 32, color: '#94A3B8' }}>
                  HWPX / DOCX / XLSX / PDF / TXT — 12개 파일 매칭
                </div>
              </div>
            </Sequence>
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* Scene 3: Features */}
      <Sequence from={240} durationInFrames={120}>
        <AbsoluteFill style={{ alignItems: 'center', justifyContent: 'center' }}>
          <h1 style={{ fontSize: 72, fontWeight: 'bold', color: '#38BDF8' }}>
            What's New in v2.1.0
          </h1>

          <div
            style={{
              display: 'flex',
              flexWrap: 'wrap',
              justifyContent: 'center',
              gap: 32,
              marginTop: 40,
              padding: 40,
            }}
          >
            {[
              'AI RAG 질의응답',
              'OCR 스캔 PDF',
              'HWP 자동 변환',
              '파일 태그',
              '법령 링크',
              '실시간 감시',
            ].map((feat, i) => (
              <div
                key={i}
                style={{
                  backgroundColor: '#1E293B',
                  padding: '28px 44px',
                  borderRadius: 15,
                  fontSize: 42,
                  fontWeight: 'bold',
                  border: '1px solid #334155',
                  transform: `scale(${spring({
                    frame: frame - 250 - i * 8,
                    fps,
                    config: { damping: 12 },
                  })})`,
                  opacity: frame > 250 + i * 8 ? 1 : 0,
                }}
              >
                {feat}
              </div>
            ))}
          </div>
        </AbsoluteFill>
      </Sequence>

      {/* Scene 4: Outro */}
      <Sequence from={360} durationInFrames={90}>
        <AbsoluteFill style={{ alignItems: 'center', justifyContent: 'center' }}>
          <div
            style={{
              fontSize: 64,
              fontWeight: 'bold',
              opacity: interpolate(frame, [360, 375], [0, 1], { extrapolateRight: 'clamp' }),
            }}
          >
            지금 바로 시작하세요
          </div>

          <div
            style={{
              fontFamily: 'Consolas, monospace',
              backgroundColor: '#020617',
              color: '#38BDF8',
              padding: '28px 56px',
              fontSize: 52,
              borderRadius: 12,
              marginTop: 36,
              border: '2px dashed #334155',
              transform: `scale(${spring({ frame: frame - 380, fps, config: { damping: 15 } })})`,
            }}
          >
            github.com/chrisryugj/Docufinder
          </div>

          <div
            style={{
              fontSize: 36,
              color: '#94A3B8',
              marginTop: 36,
              opacity: interpolate(frame, [400, 415], [0, 1], { extrapolateRight: 'clamp' }),
            }}
          >
            Windows 데스크톱 앱 &middot; Tauri + React &middot; 100% 로컬 검색
          </div>
        </AbsoluteFill>
      </Sequence>
    </AbsoluteFill>
  );
};
