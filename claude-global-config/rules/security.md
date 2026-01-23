---
name: security
trigger: always
priority: critical
---

# 보안 규칙

## 필수 준수 사항 (커밋 전 체크)

### 1. 시크릿 관리
```
❌ 절대 금지:
- 하드코딩된 API 키
- 하드코딩된 비밀번호
- 하드코딩된 토큰
- .env 파일 커밋

✅ 필수:
- 환경변수 사용: process.env.API_KEY
- 시크릿 매니저 사용
- .env.example만 커밋 (값 없이)
```

### 2. 입력 검증
```
✅ 모든 사용자 입력 검증:
- 타입 검사
- 길이 제한
- 허용 문자 검사
- SQL 이스케이프
- HTML 이스케이프
```

### 3. 인젝션 방지
```typescript
// SQL Injection
❌ db.query(`SELECT * FROM users WHERE id = ${userId}`);
✅ db.query('SELECT * FROM users WHERE id = ?', [userId]);

// Command Injection
❌ exec(`ls ${userPath}`);
✅ execFile('ls', [sanitizedPath]);

// XSS
❌ element.innerHTML = userInput;
✅ element.textContent = userInput;
```

### 4. 인증/인가
```
✅ 필수 검사:
- 모든 보호된 엔드포인트에서 인증 확인
- 리소스 접근 전 권한 확인
- 세션 만료 처리
```

### 5. 데이터 보호
```
✅ 민감 데이터:
- 비밀번호는 해시로 저장 (bcrypt, argon2)
- 전송 시 HTTPS 강제
- 로그에 민감 정보 제외
- 에러 메시지에 내부 정보 제외
```

## Tauri 특화 보안

### IPC 핸들러
```rust
// 모든 invoke 핸들러에서 입력 검증
#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    // 경로 검증 필수
    if !is_safe_path(&path) {
        return Err("Invalid path".into());
    }
    // ...
}
```

### 권한 설정
```json
// tauri.conf.json - 최소 권한 원칙
{
  "allowlist": {
    "fs": {
      "scope": ["$APP/*"]  // 앱 디렉토리만
    }
  }
}
```

## 위반 시 처리

```
🔴 Critical (즉시 차단):
- 하드코딩된 시크릿 → 커밋 차단
- SQL 인젝션 패턴 → 코드 수정 필수

🟡 Warning (경고):
- 입력 검증 누락 → 검토 필요
- 권한 체크 없음 → 추가 권장
```

## 보안 사고 대응

```
1. 작업 즉시 중단
2. security-reviewer 에이전트 호출
3. 취약점 수정
4. (시크릿 노출 시) 키 즉시 로테이션
5. 유사 패턴 전체 검색
```
