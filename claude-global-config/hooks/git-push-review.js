#!/usr/bin/env node
/**
 * Git Push Review Hook
 * - push 전 변경사항 리마인더
 * - 테스트 실행 확인
 */

const fs = require('fs');
const { execSync } = require('child_process');

function main() {
  try {
    const input = JSON.parse(fs.readFileSync(0, 'utf8'));
    const command = input.tool_input?.command || '';

    // git push 명령인 경우만
    if (!command.includes('git push')) {
      console.log(JSON.stringify({ decision: 'approve' }));
      return;
    }

    const messages = [];
    const cwd = process.cwd();

    // 변경된 파일 수 확인
    try {
      const diffStat = execSync('git diff --stat HEAD~1', {
        cwd,
        encoding: 'utf8',
        timeout: 5000
      });

      const fileCount = (diffStat.match(/\d+ files? changed/g) || []).length;
      if (fileCount > 0) {
        messages.push(`📊 변경 요약:\n${diffStat.trim().split('\n').slice(-1)[0]}`);
      }
    } catch (e) {
      // 무시
    }

    // 푸시 전 확인 메시지
    messages.push('\n✅ 푸시 전 확인:');
    messages.push('   - 테스트 실행 완료?');
    messages.push('   - 코드 리뷰 완료?');
    messages.push('   - console.log 제거?');

    console.log(JSON.stringify({
      decision: 'approve',
      message: messages.join('\n')
    }));
  } catch (error) {
    console.log(JSON.stringify({ decision: 'approve' }));
  }
}

main();
