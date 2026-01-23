#!/usr/bin/env node
/**
 * Post-Edit Lint Hook
 * - TypeScript 파일 편집 후 타입 체크
 * - 린트 에러 표시
 */

const fs = require('fs');
const { execSync } = require('child_process');
const path = require('path');

function main() {
  try {
    const input = JSON.parse(fs.readFileSync(0, 'utf8'));
    const filePath = input.tool_input?.file_path || '';

    // TypeScript 파일인 경우만
    if (!/\.(ts|tsx)$/.test(filePath)) {
      console.log(JSON.stringify({ decision: 'approve' }));
      return;
    }

    const messages = [];
    const cwd = process.cwd();

    // tsconfig.json 존재 확인
    const tsconfigPath = path.join(cwd, 'tsconfig.json');
    if (!fs.existsSync(tsconfigPath)) {
      console.log(JSON.stringify({ decision: 'approve' }));
      return;
    }

    // TypeScript 타입 체크 (해당 파일만)
    try {
      execSync(`npx tsc --noEmit --skipLibCheck ${filePath} 2>&1`, {
        cwd,
        encoding: 'utf8',
        timeout: 10000,
        stdio: ['pipe', 'pipe', 'pipe']
      });
    } catch (error) {
      if (error.stdout) {
        const errors = error.stdout
          .split('\n')
          .filter(line => line.includes('error TS'))
          .slice(0, 5); // 최대 5개만

        if (errors.length > 0) {
          messages.push('❌ TypeScript 에러:');
          errors.forEach(e => messages.push(`   ${e}`));
        }
      }
    }

    if (messages.length > 0) {
      console.log(JSON.stringify({
        decision: 'approve',
        message: messages.join('\n')
      }));
    } else {
      console.log(JSON.stringify({ decision: 'approve' }));
    }
  } catch (error) {
    // 에러 시 승인 (fail-safe)
    console.log(JSON.stringify({ decision: 'approve' }));
  }
}

main();
