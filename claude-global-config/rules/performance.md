---
name: performance
trigger: always
---

# 성능 규칙

## 컨텍스트 관리 (Claude 세션)

### 토큰 임계점
```
| 토큰 | 상태 | 조치 |
|------|------|------|
| ~50k | ✅ 안전 | 정상 작업 |
| ~70k | ⚠️ 주의 | /checkpoint 권장 |
| ~85k | 🟡 경고 | /handoff 권장 |
| ~95k | 🔴 위험 | /handoff 필수 |
```

### 토큰 절약
```
✅ 권장:
- 필요한 파일 부분만 읽기
- 구체적인 검색 패턴 사용
- 간결한 응답 요청
- 한 번 읽은 내용 기억 활용

❌ 피해야 할 것:
- 큰 파일 전체 읽기
- 넓은 범위 검색
- 같은 파일 반복 읽기
- 불필요하게 긴 출력
```

### MCP 관리
```
활성 MCP 80개 이하 유지
불필요한 MCP 비활성화
```

## 코드 성능

### 알고리즘 복잡도
```
✅ 권장:
- O(n) 또는 O(n log n)
- 적절한 자료구조 선택

⚠️ 주의:
- O(n²) 이상 → 최적화 검토
- 중첩 루프 → 개선 가능성 확인
```

### 데이터베이스
```
✅ 필수:
- N+1 쿼리 방지
- 적절한 인덱스 사용
- 필요한 컬럼만 SELECT
- 페이지네이션 적용

❌ 금지:
- SELECT *
- 루프 안에서 쿼리
- 인덱스 없는 WHERE
```

### React 성능
```typescript
// ✅ 불필요한 리렌더링 방지
const MemoizedComponent = React.memo(Component);

// ✅ 비싼 계산 메모이제이션
const computed = useMemo(() => expensiveCalc(data), [data]);

// ✅ 콜백 안정화
const handleClick = useCallback(() => {
  onClick(id);
}, [id, onClick]);

// ✅ 가상화 (긴 리스트)
import { FixedSizeList } from 'react-window';
```

### 번들 최적화
```
✅ 권장:
- 코드 스플리팅 (lazy loading)
- 트리 셰이킹
- 이미지 최적화 (WebP, lazy loading)
- 불필요한 의존성 제거

// 동적 임포트
const HeavyComponent = lazy(() => import('./HeavyComponent'));
```

## 비동기 처리

### 병렬 실행
```typescript
// ✅ 독립적 작업 병렬 처리
const [users, products] = await Promise.all([
  fetchUsers(),
  fetchProducts()
]);

// ❌ 불필요한 순차 실행
const users = await fetchUsers();
const products = await fetchProducts();
```

### 에러 처리
```typescript
// ✅ Promise.allSettled (일부 실패 허용)
const results = await Promise.allSettled(promises);

// 실패한 것만 필터링
const failed = results.filter(r => r.status === 'rejected');
```

## 캐싱

### 클라이언트 캐싱
```typescript
// React Query / SWR
const { data } = useQuery({
  queryKey: ['user', id],
  queryFn: () => fetchUser(id),
  staleTime: 5 * 60 * 1000, // 5분
});
```

### 메모이제이션
```typescript
// 비싼 계산 캐싱
const memoizedFn = useMemo(() => {
  return heavyComputation(input);
}, [input]);
```

## 모니터링

### 성능 지표
```
- First Contentful Paint (FCP) < 1.8s
- Largest Contentful Paint (LCP) < 2.5s
- Time to Interactive (TTI) < 3.8s
- Cumulative Layout Shift (CLS) < 0.1
```

### 프로파일링
```bash
# React 프로파일러
React DevTools → Profiler

# 번들 분석
npx webpack-bundle-analyzer
npx vite-bundle-visualizer
```
