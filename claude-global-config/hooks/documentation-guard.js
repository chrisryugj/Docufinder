#!/usr/bin/env node
/**
 * Documentation Guard Hook
 * - 불필요한 문서 파일 생성 방지
 * - README.md는 항상 허용
 */

const fs = require('fs');
const path = require('path');

function main() {
  try {
    const input = JSON.parse(fs.readFileSync(0, 'utf8'));
    const tool = input.tool || '';
    const filePath = input.tool_input?.file_path || '';

    // Write 도구이고 .md 파일인 경우만 체크
    if (tool !== 'Write' || !filePath.endsWith('.md')) {
      console.log(JSON.stringify({ decision: 'approve' }));
      return;
    }

    const fileName = path.basename(filePath);

    // 허용되는 문서 파일들
    const allowedDocs = [
      'README.md',
      'CHANGELOG.md',
      'CONTRIBUTING.md',
      'LICENSE.md',
      'HANDOFF.md',
      'CLAUDE.md',
      'API.md',
      'ARCHITECTURE.md',
    ];

    // 허용된 문서거나 특정 디렉토리 내 문서는 허용
    const isAllowed = allowedDocs.includes(fileName) ||
                      filePath.includes('/docs/') ||
                      filePath.includes('/.claude/') ||
                      filePath.includes('/memory/');

    if (!isAllowed) {
      console.log(JSON.stringify({
        decision: 'approve',
        message: `📝 새 문서 파일 생성: ${fileName}\n   README.md 또는 docs/ 폴더에 통합하는 것을 권장합니다.`
      }));
    } else {
      console.log(JSON.stringify({ decision: 'approve' }));
    }
  } catch (error) {
    console.log(JSON.stringify({ decision: 'approve' }));
  }
}

main();
