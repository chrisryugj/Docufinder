#!/usr/bin/env node
/**
 * Pre-Compact Hook
 * - 압축 전 현재 상태 자동 저장
 */

const fs = require('fs');
const path = require('path');

function main() {
  const cwd = process.cwd();
  const messages = [];

  // 체크포인트 디렉토리 생성
  const checkpointDir = path.join(cwd, '.claude', 'checkpoints');
  if (!fs.existsSync(checkpointDir)) {
    try {
      fs.mkdirSync(checkpointDir, { recursive: true });
    } catch (e) {
      // 무시
    }
  }

  // 현재 시간으로 체크포인트 생성
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  const checkpointFile = path.join(checkpointDir, `pre-compact-${timestamp}.md`);

  const content = `# Pre-Compact Checkpoint
- 생성 시간: ${new Date().toLocaleString('ko-KR')}
- 이유: 컨텍스트 압축 전 자동 생성

## 노트
이 파일은 컨텍스트 압축 전 자동으로 생성되었습니다.
HANDOFF.md를 참고하여 작업을 이어가세요.
`;

  try {
    fs.writeFileSync(checkpointFile, content);
    messages.push(`💾 체크포인트 저장됨: ${checkpointFile}`);
  } catch (e) {
    // 무시
  }

  if (messages.length > 0) {
    console.log(messages.join('\n'));
  }

  console.log(JSON.stringify({ decision: 'approve' }));
}

main();
