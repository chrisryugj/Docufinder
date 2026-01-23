#!/usr/bin/env node
/**
 * Session End Hook
 * - 세션 상태 저장
 * - HANDOFF 생성 제안
 */

const fs = require('fs');
const path = require('path');

function main() {
  const cwd = process.cwd();
  const messages = [];

  // HANDOFF.md 존재 여부 확인
  const handoffPath = path.join(cwd, 'HANDOFF.md');
  const hasHandoff = fs.existsSync(handoffPath);

  messages.push('');
  messages.push('─'.repeat(50));
  messages.push('📋 세션 종료');
  messages.push('─'.repeat(50));

  if (!hasHandoff) {
    messages.push('💡 다음 세션을 위해 /handoff 실행을 권장합니다.');
  } else {
    messages.push('✅ HANDOFF.md가 존재합니다.');
    messages.push('   다음 세션에서 "HANDOFF.md 읽고 이어서" 로 컨텍스트 복원 가능');
  }

  messages.push('─'.repeat(50));
  messages.push('');

  console.log(messages.join('\n'));
  console.log(JSON.stringify({ decision: 'approve' }));
}

main();
