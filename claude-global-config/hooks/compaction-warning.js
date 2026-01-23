#!/usr/bin/env node
/**
 * Compaction Warning Hook
 * - 컨텍스트 압축 전 경고
 * - HANDOFF 생성 권장
 */

const fs = require('fs');

function main() {
  const messages = [
    '',
    '─'.repeat(50),
    '⚠️  컨텍스트가 압축됩니다!',
    '─'.repeat(50),
    '',
    '권장 조치:',
    '1. /handoff 실행하여 현재 상태 저장',
    '2. 중요한 컨텍스트 메모',
    '3. 필요시 새 세션 시작',
    '',
    '💡 팁: 압축 전 상태가 자동 저장됩니다.',
    '─'.repeat(50),
    ''
  ];

  console.log(messages.join('\n'));
  console.log(JSON.stringify({ decision: 'approve' }));
}

main();
