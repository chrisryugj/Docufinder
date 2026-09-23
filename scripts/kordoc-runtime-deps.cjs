// kordoc 번들에 깔 런타임 의존성 목록. kordoc package.json 이 정본이다.
// 번들 스크립트(bundle-kordoc.ps1 · setup-macos-resources.sh)가 이 출력을 npm install 에 넘긴다.
// 목록을 스크립트에 따로 적어 두면 kordoc 이 요구하는 범위와 어긋난다
// (실측: commander ^13·zod ^3 인데 15·4 가 깔리고, sharp 는 ^0.35 인데 ^0.34 고정).
//
// 사용: node scripts/kordoc-runtime-deps.cjs <kordoc>/package.json [--lite] [--json]
//   --json: {"이름":"범위"} 로 — 윈도우 번들은 범위를 명령줄로 넘기지 않고 package.json dependencies 에 적는다
//   (npm.cmd 를 거치면 cmd.exe 가 ^ 를 이스케이프로 먹는다, bundle-kordoc.ps1 참고)
const pkg = require(require("path").resolve(process.argv[2]));
const lite = process.argv.includes("--lite");
const asJson = process.argv.includes("--json");

// MCP 서버 전용: 앱은 cli.js 만 띄운다.
const SKIP = new Set(["@modelcontextprotocol/sdk"]);
// lite(내부망)는 OCR 네이티브 바이너리를 넣지 않는다 (--ocr·--formula-ocr 를 넘기지 않음).
const LITE_SKIP = new Set(["@hyzyla/pdfium", "onnxruntime-node", "sharp", "@huggingface/transformers"]);

const deps = { ...pkg.dependencies, ...pkg.optionalDependencies };
const picked = Object.entries(deps).filter(([name]) => !SKIP.has(name) && !(lite && LITE_SKIP.has(name)));
if (asJson) process.stdout.write(JSON.stringify(Object.fromEntries(picked)) + "\n");
else process.stdout.write(picked.map(([name, range]) => `${name}@${range}`).join("\n") + "\n");
