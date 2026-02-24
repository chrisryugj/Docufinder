// 테마 플래시 방지: React 렌더링 전에 저장된 테마 적용
try {
  var settings = localStorage.getItem('app-settings');
  if (settings) {
    var parsed = JSON.parse(settings);
    if (parsed.theme === 'dark' || (!parsed.theme && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
      document.documentElement.classList.add('dark');
    }
  } else if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
    document.documentElement.classList.add('dark');
  }
} catch(e) {}
