#!/usr/bin/env node
/**
 * Pre-Edit Check Hook
 * - console.log 감지 (경고)
 * - 하드코딩된 시크릿 감지 (차단)
 * - 파일 크기 체크 (경고)
 */

const fs = require('fs');

function main() {
  try {
    const input = JSON.parse(fs.readFileSync(0, 'utf8'));
    const newString = input.tool_input?.new_string || '';
    const filePath = input.tool_input?.file_path || '';

    const violations = [];
    const warnings = [];

    // 1. 하드코딩된 시크릿 패턴 (차단)
    const secretPatterns = [
      { pattern: /api[_-]?key\s*[:=]\s*['"][a-zA-Z0-9_-]{20,}['"]/gi, name: 'API Key' },
      { pattern: /password\s*[:=]\s*['"][^'"]{8,}['"]/gi, name: 'Password' },
      { pattern: /secret\s*[:=]\s*['"][a-zA-Z0-9_-]{20,}['"]/gi, name: 'Secret' },
      { pattern: /token\s*[:=]\s*['"][a-zA-Z0-9_.-]{20,}['"]/gi, name: 'Token' },
      { pattern: /sk-[a-zA-Z0-9]{20,}/g, name: 'OpenAI Key' },
      { pattern: /ghp_[a-zA-Z0-9]{36,}/g, name: 'GitHub Token' },
      { pattern: /-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----/g, name: 'Private Key' },
    ];

    for (const { pattern, name } of secretPatterns) {
      if (pattern.test(newString)) {
        violations.push(`🚨 하드코딩된 ${name} 감지됨! 환경변수를 사용하세요.`);
      }
    }

    // 시크릿 발견 시 차단
    if (violations.length > 0) {
      console.log(JSON.stringify({
        decision: 'block',
        reason: violations.join('\n')
      }));
      return;
    }

    // 2. console.log 감지 (경고)
    // .test, .spec, .stories 파일은 제외
    const isTestFile = /\.(test|spec|stories)\.(ts|tsx|js|jsx)$/.test(filePath);
    if (!isTestFile && /console\.(log|debug|info)\s*\(/.test(newString)) {
      warnings.push('⚠️ console.log 감지됨 - 커밋 전 제거 권장');
    }

    // 3. debugger 문 감지 (경고)
    if (/\bdebugger\b/.test(newString)) {
      warnings.push('⚠️ debugger 문 감지됨 - 제거 필요');
    }

    // 4. TODO/FIXME 감지 (정보)
    if (/\b(TODO|FIXME|HACK|XXX)\b/.test(newString)) {
      warnings.push('📝 TODO/FIXME 주석 감지됨 - 추적 필요');
    }

    // 경고가 있으면 경고와 함께 승인
    if (warnings.length > 0) {
      console.log(JSON.stringify({
        decision: 'approve',
        message: warnings.join('\n')
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
