import { memo, useRef, useState, useEffect } from "react";
import { motion, type Variants } from "framer-motion";
import { FolderPlus, Search, Zap, Clock, Terminal } from "lucide-react";
import type { RecentSearch } from "../../types/search";

// --- Spotlight Card Component ---
interface SpotlightCardProps {
  children: React.ReactNode;
  className?: string;
  variants?: Variants;
}

function SpotlightCard({ children, className = "", variants }: SpotlightCardProps) {
  const divRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [opacity, setOpacity] = useState(0);

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!divRef.current) return;
    const rect = divRef.current.getBoundingClientRect();
    setPosition({ x: e.clientX - rect.left, y: e.clientY - rect.top });
  };

  return (
    <motion.div
      ref={divRef}
      onMouseMove={handleMouseMove}
      onMouseEnter={() => setOpacity(1)}
      onMouseLeave={() => setOpacity(0)}
      className={`relative overflow-hidden group ${className}`}
      variants={variants}
    >
      {/* Background Soft Glow */}
      <div
        className="pointer-events-none absolute inset-0 z-40 transition-opacity duration-300"
        style={{
          opacity,
          background: `radial-gradient(400px circle at ${position.x}px ${position.y}px, color-mix(in srgb, var(--color-accent) 10%, transparent), transparent 50%)`,
        }}
      />
      {/* Intense Border Highlight */}
      <div
        className="pointer-events-none absolute inset-0 z-50 transition-opacity duration-300 rounded-[inherit]"
        style={{
          opacity,
          background: `radial-gradient(150px circle at ${position.x}px ${position.y}px, color-mix(in srgb, var(--color-accent) 70%, transparent), transparent 100%)`,
          border: '1.5px solid transparent',
          WebkitMask: 'linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0)',
          WebkitMaskComposite: 'xor',
          maskComposite: 'exclude',
          padding: '1.5px'
        }}
      />
      {children}
    </motion.div>
  );
}

// --- Constants ---
interface WelcomeHeroProps {
  indexedFiles?: number;
  indexedFolders?: number;
  recentSearches?: RecentSearch[];
  onSelectSearch?: (query: string) => void;
  semanticEnabled?: boolean;
  onAddFolder?: () => void;
}

const FILE_TYPES = [
  { label: "HWPX", color: "var(--color-file-hwpx)", bg: "var(--color-file-hwpx-bg)", border: "color-mix(in srgb, var(--color-file-hwpx) 20%, transparent)", yOffset: -5 },
  { label: "DOCX", color: "var(--color-file-docx)", bg: "var(--color-file-docx-bg)", border: "color-mix(in srgb, var(--color-file-docx) 20%, transparent)", yOffset: 5 },
  { label: "XLSX", color: "var(--color-file-xlsx)", bg: "var(--color-file-xlsx-bg)", border: "color-mix(in srgb, var(--color-file-xlsx) 20%, transparent)", yOffset: -2 },
  { label: "PDF", color: "var(--color-file-pdf)", bg: "var(--color-file-pdf-bg)", border: "color-mix(in srgb, var(--color-file-pdf) 20%, transparent)", yOffset: 8 },
  { label: "TXT", color: "var(--color-file-txt)", bg: "var(--color-file-txt-bg)", border: "color-mix(in srgb, var(--color-file-txt) 20%, transparent)", yOffset: -8 },
];

const bentoContainer: Variants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.08, delayChildren: 0.1 }
  }
};

const bentoItem: Variants = {
  hidden: { opacity: 0, y: 30, scale: 0.96 },
  visible: { 
    opacity: 1, y: 0, scale: 1,
    transition: { type: "spring", stiffness: 350, damping: 30 }
  }
};

export const WelcomeHero = memo(function WelcomeHero({
  indexedFiles = 0,
  indexedFolders = 0,
  recentSearches = [],
  onSelectSearch,
  semanticEnabled = false,
  onAddFolder,
}: WelcomeHeroProps) {
  const hasIndex = indexedFiles > 0;
  
  // Performance: Pause animations on blur
  const [isFocused, setIsFocused] = useState(true);
  useEffect(() => {
    const onFocus = () => setIsFocused(true);
    const onBlur = () => setIsFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => { window.removeEventListener("focus", onFocus); window.removeEventListener("blur", onBlur); };
  }, []);

  // Dispatch Ctrl+K to open global search bar
  const triggerSearchFocus = () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true }));
  };

  return (
    <div className="w-full flex items-center justify-center p-4 lg:p-8 select-none relative h-full min-h-[70vh]">
      <motion.div 
        className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-5 w-full max-w-[1240px] m-auto"
        variants={bentoContainer}
        initial="hidden"
        animate="visible"
      >
        {/* Box 1: Main Hero (Col 1-2, Row 1-2) */}
        <SpotlightCard 
          className="md:col-span-2 md:row-span-2 rounded-3xl glass flex flex-col justify-between p-8 sm:p-10 shadow-lg border border-[var(--color-border)] will-change-transform"
          variants={bentoItem}
        >
          {/* Ambient Background Glow */}
          <div className="absolute inset-0 noise-overlay opacity-30 mix-blend-overlay pointer-events-none" />
          <motion.div 
            className="absolute -right-20 -bottom-20 w-96 h-96 rounded-full blur-[100px] pointer-events-none"
            style={{ backgroundColor: "var(--color-accent)", opacity: 0.15 }}
            animate={isFocused ? { scale: [1, 1.2, 1], opacity: [0.15, 0.25, 0.15] } : {}}
            transition={{ duration: 10, repeat: Infinity, ease: "easeInOut" }}
          />

          <div className="relative z-10">
            <div className="flex items-center gap-4 mb-3" style={{ transform: "translateY(5px)" }}>
              <img src="/anything.png" alt="" className="w-12 h-12 object-contain drop-shadow-sm dark:hidden" />
              <img src="/anything-l.png" alt="" className="w-12 h-12 object-contain drop-shadow-sm hidden dark:block" />
            </div>
            
            <h1 className="text-[3.5rem] md:text-[4.5rem] font-extrabold leading-[1.1] text-display mb-2" style={{ letterSpacing: "var(--tracking-hero)", color: "var(--color-text-primary)" }}>
              Anything<span style={{ color: "var(--color-accent)" }}>.</span>
            </h1>
            
            <p className="text-xl md:text-2xl font-medium text-body-tracking mb-6 max-w-sm" style={{ color: "var(--color-text-muted)", letterSpacing: "-0.01em" }}>
              AI, Everything, <span className="font-bold" style={{ color: "var(--color-accent)" }}>Anything.</span>
            </p>

            <p className="text-sm md:text-[15px] font-medium leading-relaxed max-w-sm" style={{ color: "var(--color-text-tertiary)", letterSpacing: "-0.02em" }}>
              내 PC 깊숙이 흩어진 수많은 문서들.<br/>
              이제 AI가 직접 읽고, 완벽한 답을 찾아냅니다.
            </p>
          </div>

          {/* Dummy Central Search Input triggering global search */}
          <div className="relative z-10 mt-auto pt-8">
            <div 
              onClick={triggerSearchFocus}
              className="flex items-center gap-3 w-full bg-white dark:bg-[#111113] border border-[var(--color-border)] rounded-2xl p-4 shadow-sm cursor-text hover:border-[var(--color-accent)] transition-colors group/search"
            >
              <Search className="w-5 h-5" style={{ color: "var(--color-text-muted)" }} />
              <span className="text-sm font-medium" style={{ color: "var(--color-text-muted)" }}>무엇이든 물어보세요...</span>
              <kbd className="ml-auto inline-flex items-center px-2 py-1 rounded text-xs font-bold font-mono shadow-sm group-hover/search:text-[var(--color-accent)] transition-colors" style={{ backgroundColor: "var(--color-bg-tertiary)", border: "1px solid var(--color-border)", color: "var(--color-text-secondary)" }}>
                Ctrl+K
              </kbd>
            </div>
          </div>
        </SpotlightCard>

        {hasIndex ? (
          <>
            {/* Box 2: Stats & Status (Col 3, Row 1) */}
            <SpotlightCard 
              className="rounded-3xl glass p-7 flex flex-col justify-between shadow-lg border border-[var(--color-border)]"
              variants={bentoItem}
            >
              <div className="relative z-10">
                 <div className="flex items-center gap-2 mb-3">
                  <span className="p-1.5 rounded-lg" style={{ backgroundColor: "var(--color-accent-subtle)", color: "var(--color-accent)" }}>
                    <Zap className="w-5 h-5" />
                  </span>
                  <span className="text-sm font-bold tracking-wide uppercase" style={{ color: "var(--color-text-muted)" }}>
                    Indexing Status
                  </span>
                </div>
                
                <div className="space-y-4 mt-6">
                  <div>
                    <div className="text-3xl font-extrabold text-display mb-1" style={{ letterSpacing: "-0.02em", color: "var(--color-text-primary)" }}>
                      {indexedFiles.toLocaleString()}
                    </div>
                    <div className="text-xs font-medium" style={{ color: "var(--color-text-muted)" }}>인덱싱된 전체 문서</div>
                  </div>
                  <div className="flex items-center gap-3 pt-4 border-t" style={{ borderColor: "var(--color-border-subtle)" }}>
                    <div className="flex items-center gap-1.5 min-w-[50%]">
                      <FolderPlus className="w-4 h-4" style={{ color: "var(--color-text-muted)" }} />
                      <span className="text-sm font-semibold" style={{ color: "var(--color-text-secondary)" }}>{indexedFolders} 폴더</span>
                    </div>
                    {semanticEnabled && (
                      <div className="flex items-center gap-1">
                        <span className="w-2 h-2 rounded-full" style={{ backgroundColor: "var(--color-success)", boxShadow: "0 0 6px var(--color-success)" }} />
                        <span className="text-[11px] font-bold" style={{ color: "var(--color-success)" }}>AI Ready</span>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </SpotlightCard>

            {/* Box 3: Supported Formats (Col 4, Row 1) */}
            <SpotlightCard 
              className="rounded-3xl glass p-7 flex flex-col justify-between shadow-lg border border-[var(--color-border)]"
              variants={bentoItem}
            >
              <div className="absolute inset-0 noise-overlay opacity-20 hidden dark:block pointer-events-none" />
              
              <div className="flex items-center gap-2 mb-2 relative z-10">
                <span className="p-1.5 rounded-lg" style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-secondary)" }}>
                  <Terminal className="w-5 h-5" />
                </span>
                <span className="text-sm font-bold tracking-wide uppercase" style={{ color: "var(--color-text-muted)" }}>
                  Formats
                </span>
              </div>

              <div className="flex-1 flex items-center justify-center relative mt-4 z-10">
                <div className="flex items-center justify-center gap-2 flex-wrap max-w-[160px]">
                  {FILE_TYPES.map((ft, i) => (
                    <motion.div
                      key={ft.label}
                      className="px-2.5 py-1.5 rounded-lg text-[11px] font-bold tracking-wide shadow-sm"
                      style={{
                        color: ft.color,
                        backgroundColor: ft.bg,
                        border: `1px solid ${ft.border}`,
                      }}
                      animate={isFocused ? { y: [ft.yOffset, -ft.yOffset, ft.yOffset] } : {}}
                      transition={{ duration: 4 + i, repeat: Infinity, ease: "easeInOut" }}
                    >
                      {ft.label}
                    </motion.div>
                  ))}
                </div>
              </div>
            </SpotlightCard>

            {/* Box 4: Recent Searches (Col 3-4, Row 2) */}
            <SpotlightCard 
              className="md:col-span-2 rounded-3xl glass p-7 flex flex-col justify-between shadow-lg border border-[var(--color-border)]"
              variants={bentoItem}
            >
              <div className="flex items-center justify-between mb-4 relative z-10">
                 <div className="flex items-center gap-2">
                  <span className="p-1.5 rounded-lg" style={{ backgroundColor: "var(--color-bg-tertiary)", color: "var(--color-text-secondary)" }}>
                    <Clock className="w-5 h-5" />
                  </span>
                  <span className="text-sm font-bold tracking-wide uppercase" style={{ color: "var(--color-text-muted)" }}>
                    최근 검색 기록
                  </span>
                </div>
              </div>

              <div className="flex-1 flex flex-col justify-center relative z-10">
                {recentSearches.length > 0 ? (
                  <div className="flex flex-wrap gap-2.5">
                    {recentSearches.slice(0, 7).map((s) => (
                      <button
                        key={s.query}
                        onClick={() => onSelectSearch?.(s.query)}
                        className="group px-4 py-2 text-sm rounded-xl border bg-[var(--color-bg-secondary)] hover:bg-[var(--color-bg-tertiary)] hover:border-[var(--color-accent-subtle)] transition-all flex items-center gap-2 shadow-sm whitespace-nowrap overflow-hidden"
                        style={{ borderColor: "var(--color-border)", color: "var(--color-text-secondary)" }}
                      >
                        <Search className="w-3 h-3 opacity-50 group-hover:opacity-100 group-hover:text-[var(--color-accent)] transition-all" />
                        <span className="truncate max-w-[150px]">{s.query}</span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <div className="flex flex-col items-center justify-center text-center opacity-60 py-2">
                    <Search className="w-6 h-6 mb-2" style={{ color: "var(--color-text-muted)" }} />
                    <p className="text-sm font-medium" style={{ color: "var(--color-text-muted)" }}>아직 기록이 없습니다.</p>
                  </div>
                )}
              </div>
            </SpotlightCard>
          </>
        ) : (
          /* Empty State Onboarding Dashboard (Col 3-4, Row 1-2) */
          <SpotlightCard
            className="md:col-span-2 md:row-span-2 rounded-3xl glass flex flex-col items-center justify-center p-10 text-center shadow-lg border border-[var(--color-accent-subtle)] cursor-pointer hover:border-[var(--color-accent)] transition-all group/onboard"
            variants={bentoItem}
          >
            <div 
              className="absolute inset-0 z-10" 
              onClick={onAddFolder} 
              role="button" 
              aria-label="폴더 추가하기"
            />
            <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,var(--color-accent-subtle)_0%,transparent_60%)] opacity-30 pointer-events-none group-hover/onboard:opacity-50 transition-opacity" />
            
            <motion.div 
              className="w-24 h-24 rounded-3xl mb-8 flex items-center justify-center shadow-xl border border-[var(--color-accent)] bg-white dark:bg-[#1A1A1F] relative z-20 pointer-events-none"
              animate={isFocused ? { y: [-10, 10, -10] } : {}}
              transition={{ duration: 6, repeat: Infinity, ease: "easeInOut" }}
            >
              <FolderPlus className="w-10 h-10" style={{ color: "var(--color-accent)" }} />
              {/* Pulse rings */}
              <div className="absolute inset-0 rounded-3xl border border-[var(--color-accent)] opacity-[0.2] transform scale-[1.2] group-hover/onboard:scale-[1.3] transition-transform" />
              <div className="absolute inset-0 rounded-3xl border border-[var(--color-accent)] opacity-[0.1] transform scale-[1.4] group-hover/onboard:scale-[1.6] transition-transform" />
            </motion.div>
            
            <h2 className="text-2xl font-bold mb-3 relative z-20 pointer-events-none" style={{ color: "var(--color-text-primary)", letterSpacing: "-0.03em" }}>
              인덱싱된 폴더가 없습니다
            </h2>
            
            <p className="text-[15px] font-medium leading-relaxed max-w-sm mb-8 relative z-20 pointer-events-none break-keep" style={{ color: "var(--color-text-muted)" }}>
              이 카드를 클릭하거나 우측 상단의 <strong>폴더 추가 (+)</strong> 버튼을 눌러 
              내 PC의 문서 폴더를 연결해 보세요. 
              연결 즉시 AI가 백그라운드에서 문서 파싱을 시작합니다.
            </p>

            <div className="flex items-center gap-4 relative z-10 opacity-70">
              <div className="w-12 h-[1px]" style={{ backgroundColor: "var(--color-border-hover)" }} />
              <span className="text-xs font-bold uppercase tracking-widest text-[var(--color-text-tertiary)]">1분이면 충분합니다</span>
              <div className="w-12 h-[1px]" style={{ backgroundColor: "var(--color-border-hover)" }} />
            </div>
          </SpotlightCard>
        )}
      </motion.div>
    </div>
  );
});
