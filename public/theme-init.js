// 테마 플래시 방지: React 렌더링 전에 저장된 테마 적용
try {
  var theme = localStorage.getItem('docufinder-theme');
  if (theme === 'dark') {
    document.documentElement.classList.add('dark');
  } else if (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches) {
    document.documentElement.classList.add('dark');
  }
  // theme === 'light' 또는 없으면 기본 라이트 모드 (아무 클래스 추가 안함)

  // UI 줌 적용
  var zoom = localStorage.getItem('docufinder-ui-zoom');
  if (zoom) {
    document.documentElement.style.zoom = zoom;
  }
} catch(e) {}
