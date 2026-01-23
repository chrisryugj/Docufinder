---
name: coding-style
trigger: always
---

# 코딩 스타일 규칙

## 일반 원칙

### 파일/함수 크기
```
📏 제한:
- 함수: 50줄 이하
- 파일: 800줄 이하
- 클래스: 300줄 이하
- 파라미터: 4개 이하
```

### 네이밍
```
✅ 명확하고 의미있는 이름:
- 변수: 명사/명사구 (user, activeUsers)
- 함수: 동사/동사구 (getUser, calculateTotal)
- 불리언: is/has/can 접두사 (isActive, hasPermission)
- 상수: UPPER_SNAKE_CASE (MAX_RETRY_COUNT)

❌ 피해야 할 이름:
- 한 글자 (i, j는 루프에서만 허용)
- 축약어 (usr, cnt, tmp)
- 의미없는 접미사 (dataInfo, userObject)
```

### 불변성
```typescript
// ✅ const 우선
const user = getUser();
const users = [...existingUsers, newUser];
const updated = { ...user, name: newName };

// ❌ let/변이 지양
let user = getUser();
users.push(newUser);
user.name = newName;
```

## TypeScript

### 타입 안전성
```typescript
// ✅ 명시적 타입
function getUser(id: string): Promise<User | null> { }

// ❌ any 금지
function getUser(id: any): any { }

// ✅ unknown 사용 (필요시)
function parseJson(str: string): unknown { }
```

### 인터페이스
```typescript
// ✅ interface 선호 (확장 가능)
interface User {
  id: string;
  name: string;
}

// type은 유니온/인터섹션에 사용
type Status = 'active' | 'inactive';
type AdminUser = User & { role: 'admin' };
```

### Null 안전성
```typescript
// ✅ Optional chaining
const name = user?.profile?.name;

// ✅ Nullish coalescing
const value = input ?? defaultValue;

// ❌ 느슨한 비교
if (value == null) { }
```

## Rust

### 에러 처리
```rust
// ✅ Result와 ? 연산자
fn read_file(path: &str) -> Result<String, MyError> {
    let content = fs::read_to_string(path)?;
    Ok(content)
}

// ❌ unwrap() 지양
let content = fs::read_to_string(path).unwrap();
```

### Clippy 준수
```bash
# 경고 0개 유지
cargo clippy -- -D warnings
```

## 주석

### 언제 작성하나
```
✅ 필요한 경우:
- 복잡한 비즈니스 로직 설명
- 의도가 명확하지 않은 코드
- TODO/FIXME (임시 코드)
- 공개 API 문서화

❌ 불필요한 경우:
- 코드가 명확히 설명하는 것
- 변경 이력 (git 사용)
// i를 1 증가 (이런 주석)
```

### 언어
```
- 코드/변수명: 영어
- 주석: 한국어 가능
- 커밋 메시지: 한국어 가능
```

## 포맷팅

### 들여쓰기
```
- 2 spaces (JS/TS)
- 4 spaces (Rust, Python)
```

### Import 순서
```typescript
// 1. 외부 라이브러리
import React from 'react';
import { useQuery } from '@tanstack/react-query';

// 2. 내부 모듈 (절대 경로)
import { Button } from '@/components/ui';
import { useAuth } from '@/hooks';

// 3. 상대 경로
import { helper } from './utils';

// 4. 타입 (별도)
import type { User } from '@/types';
```
