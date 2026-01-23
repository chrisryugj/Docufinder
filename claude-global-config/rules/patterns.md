---
name: patterns
trigger: always
---

# 디자인 패턴 규칙

## React 패턴

### 컴포넌트 구조
```typescript
// 권장 순서
import { useState, useCallback } from 'react';  // 1. React
import { cn } from '@/lib/utils';               // 2. 유틸리티
import type { ButtonProps } from './types';     // 3. 타입

interface Props {                               // 4. Props 정의
  variant?: 'primary' | 'secondary';
  children: React.ReactNode;
}

export function Button({ variant = 'primary', children }: Props) {
  // 5. Hooks
  const [loading, setLoading] = useState(false);

  // 6. Handlers
  const handleClick = useCallback(() => {
    // ...
  }, []);

  // 7. Render
  return (
    <button className={cn('base', variant)}>
      {children}
    </button>
  );
}
```

### Custom Hook 패턴
```typescript
// 네이밍: use + 명사
function useUser(id: string) {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    fetchUser(id)
      .then(setUser)
      .catch(setError)
      .finally(() => setLoading(false));
  }, [id]);

  return { user, loading, error };
}
```

### Compound Component
```typescript
// 관련 컴포넌트 그룹화
const Card = {
  Root: CardRoot,
  Header: CardHeader,
  Body: CardBody,
  Footer: CardFooter,
};

// 사용
<Card.Root>
  <Card.Header>Title</Card.Header>
  <Card.Body>Content</Card.Body>
</Card.Root>
```

## 상태 관리 패턴

### 로컬 상태
```typescript
// 단순 상태: useState
const [count, setCount] = useState(0);

// 복잡한 상태: useReducer
const [state, dispatch] = useReducer(reducer, initialState);
```

### 전역 상태 (Zustand)
```typescript
// store 정의
const useStore = create<State>((set) => ({
  user: null,
  setUser: (user) => set({ user }),
  logout: () => set({ user: null }),
}));

// 사용
const user = useStore((state) => state.user);
```

### 서버 상태 (TanStack Query)
```typescript
// 조회
const { data, isLoading } = useQuery({
  queryKey: ['user', id],
  queryFn: () => fetchUser(id),
});

// 변경
const mutation = useMutation({
  mutationFn: updateUser,
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['user'] });
  },
});
```

## API 패턴

### 에러 처리
```typescript
// Result 타입
type Result<T, E = Error> =
  | { success: true; data: T }
  | { success: false; error: E };

async function fetchUser(id: string): Promise<Result<User>> {
  try {
    const user = await api.get(`/users/${id}`);
    return { success: true, data: user };
  } catch (error) {
    return { success: false, error };
  }
}
```

### Repository 패턴
```typescript
interface UserRepository {
  findById(id: string): Promise<User | null>;
  create(data: CreateUserDTO): Promise<User>;
  update(id: string, data: UpdateUserDTO): Promise<User>;
  delete(id: string): Promise<void>;
}

class ApiUserRepository implements UserRepository {
  async findById(id: string) {
    return api.get(`/users/${id}`);
  }
  // ...
}
```

## Rust 패턴

### Builder 패턴
```rust
pub struct ConfigBuilder {
    timeout: Option<Duration>,
    retries: Option<u32>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self { timeout: None, retries: None }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Config {
        Config {
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
            retries: self.retries.unwrap_or(3),
        }
    }
}
```

### Error 타입
```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),
}
```

## 파일 구조 패턴

### Feature 기반
```
src/
├── features/
│   ├── auth/
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── api.ts
│   │   └── types.ts
│   ├── search/
│   └── settings/
├── components/  # 공용 컴포넌트
├── hooks/       # 공용 훅
└── lib/         # 유틸리티
```

### 계층 기반
```
src/
├── components/  # 프레젠테이션
├── containers/  # 비즈니스 로직
├── services/    # API 호출
├── stores/      # 상태 관리
├── hooks/       # 커스텀 훅
├── types/       # 타입 정의
└── utils/       # 유틸리티
```
