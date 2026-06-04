// 레이아웃 컴포넌트 barrel export
export { Header } from "./Header";
export { StatusBar } from "./StatusBar";
export { ErrorBanner } from "./ErrorBanner";
// AppModals 는 App.tsx 에서 lazy(동적 import)로만 쓰여 배럴 정적 export 를 제거했다 (P2-1).
// 배럴에 남기면 정적 import 로 묶여 lazy 청크 분리가 무효화된다.
export { FloatingUI } from "./FloatingUI";
