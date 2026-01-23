#!/usr/bin/env node
/**
 * Session Start Hook
 * - HANDOFF.md 존재 확인 및 알림
 * - 패키지 매니저 자동 감지
 */

const fs = require('fs');
const path = require('path');

function main() {
  const cwd = process.cwd();
  const messages = [];

  // 1. HANDOFF.md 확인
  const handoffPath = path.join(cwd, 'HANDOFF.md');
  if (fs.existsSync(handoffPath)) {
    const stats = fs.statSync(handoffPath);
    const modifiedDate = stats.mtime.toLocaleString('ko-KR');
    messages.push(`📋 HANDOFF.md 발견 (수정: ${modifiedDate})`);
    messages.push('   → "HANDOFF.md 읽고 이어서 작업해줘" 로 이전 세션 컨텍스트를 복원할 수 있습니다.');
  }

  // 2. 패키지 매니저 감지
  const packageManagers = {
    'pnpm-lock.yaml': 'pnpm',
    'yarn.lock': 'yarn',
    'package-lock.json': 'npm',
    'bun.lockb': 'bun',
    'Cargo.lock': 'cargo',
  };

  const detected = [];
  for (const [lockFile, pm] of Object.entries(packageManagers)) {
    if (fs.existsSync(path.join(cwd, lockFile))) {
      detected.push(pm);
    }
  }

  if (detected.length > 0) {
    messages.push(`📦 패키지 매니저: ${detected.join(', ')}`);
  }

  // 3. 프로젝트 타입 감지
  const projectTypes = [];
  if (fs.existsSync(path.join(cwd, 'tauri.conf.json')) ||
      fs.existsSync(path.join(cwd, 'src-tauri/tauri.conf.json'))) {
    projectTypes.push('Tauri');
  }
  if (fs.existsSync(path.join(cwd, 'next.config.js')) ||
      fs.existsSync(path.join(cwd, 'next.config.mjs'))) {
    projectTypes.push('Next.js');
  }
  if (fs.existsSync(path.join(cwd, 'vite.config.ts')) ||
      fs.existsSync(path.join(cwd, 'vite.config.js'))) {
    projectTypes.push('Vite');
  }
  if (fs.existsSync(path.join(cwd, 'Cargo.toml'))) {
    projectTypes.push('Rust');
  }

  if (projectTypes.length > 0) {
    messages.push(`🛠️  프로젝트 타입: ${projectTypes.join(' + ')}`);
  }

  // 출력
  if (messages.length > 0) {
    console.log('\n' + '─'.repeat(50));
    console.log('🚀 세션 시작');
    console.log('─'.repeat(50));
    messages.forEach(msg => console.log(msg));
    console.log('─'.repeat(50) + '\n');
  }

  // 항상 승인
  console.log(JSON.stringify({ decision: 'approve' }));
}

main();
