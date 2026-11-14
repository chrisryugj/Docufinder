/**
 * 들여쓰기 코드 블록(4칸 이상 들여쓴 줄) 구문을 끈다.
 *
 * kordoc 은 HWP·HWPX 문단 앞 공백(한글 들여쓰기)을 마크다운에 그대로 남긴다. CommonMark 는 4칸 이상
 * 들여쓴 줄을 코드 블록으로 읽어 본문 문단이 고정폭 상자로 보였다(kordoc 4.24 부터 HWP5 도 들여쓰기를
 * 보존해 HWP 200건 중 9건). 문서 미리보기의 코드는 ``` 펜스뿐이라 이 구문 하나만 끈다.
 */
export function remarkNoIndentedCode(this: { data(): Record<string, unknown> }): void {
  const data = this.data();
  const extensions = (data.micromarkExtensions ??= []) as unknown[];
  extensions.push({ disable: { null: ["codeIndented"] } });
}
