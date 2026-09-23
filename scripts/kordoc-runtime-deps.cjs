// kordoc 번들에 깔 런타임 의존성 목록. kordoc package.json 이 정본이다.
// 번들 스크립트(bundle-kordoc.ps1 · setup-macos-resources.sh)가 이 출력을 npm install 에 넘긴다.
// 목록을 스크립트에 따로 적어 두면 kordoc 이 요구하는 범위와 어긋난다
// (실측: commander ^13·zod ^3 인데 15·4 가 깔리고, sharp 는 ^0.35 인데 ^0.34 고정).
//
// 사용: node scripts/kordoc-runtime-deps.cjs <kordoc>/package.json [--lite]
const pkg = require(require("path").resolve(process.argv[2]));
const lite = process.argv.includes("--lite");

// MCP 서버 전용: 앱은 cli.js 만 띄운다.
const SKIP = new Set(["@modelcontextprotocol/sdk"]);
// lite(내부망)는 OCR 네이티브 바이너리를 넣지 않는다 (--ocr·--formula-ocr 를 넘기지 않음).
const LITE_SKIP = new Set(["@hyzyla/pdfium", "onnxruntime-node", "sharp", "@huggingface/transformers"]);

const deps = { ...pkg.dependencies, ...pkg.optionalDependencies };
const out = Object.entries(deps)
  .filter(([name]) => !SKIP.has(name) && !(lite && LITE_SKIP.has(name)))
  .map(([name, range]) => `${name}@${range}`);
process.stdout.write(out.join("\n") + "\n");
