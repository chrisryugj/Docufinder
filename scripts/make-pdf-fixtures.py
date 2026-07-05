#!/usr/bin/env python3
"""docufinder 다페이지 PDF 픽스처 생성기.

- multipage_text.pdf     : 3페이지 텍스트 PDF (Helvetica ASCII)
- multipage_scanned.pdf  : 3페이지 이미지-only PDF (FlateDecode DeviceGray, 텍스트 레이어 없음)
- mixed_text_scanned.pdf : 1페이지 텍스트 + 2페이지 이미지-only 혼합

이미지 페이지의 글자는 Pillow 로 렌더한 한국어 — PaddleOCR 실검증용.
"""
import sys
import zlib
from PIL import Image, ImageDraw, ImageFont

OUT_DIR = sys.argv[1] if len(sys.argv) > 1 else "."


class PdfBuilder:
    def __init__(self):
        self.objects = []  # list[bytes] — 1-indexed

    def add(self, body: bytes) -> int:
        self.objects.append(body)
        return len(self.objects)

    def reserve(self) -> int:
        self.objects.append(b"")
        return len(self.objects)

    def set(self, num: int, body: bytes):
        self.objects[num - 1] = body

    def build(self, root_num: int) -> bytes:
        out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
        offsets = []
        for i, body in enumerate(self.objects, start=1):
            offsets.append(len(out))
            out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
        xref_pos = len(out)
        out += f"xref\n0 {len(self.objects) + 1}\n".encode()
        out += b"0000000000 65535 f \n"
        for off in offsets:
            out += f"{off:010d} 00000 n \n".encode()
        out += (
            f"trailer\n<< /Size {len(self.objects) + 1} /Root {root_num} 0 R >>\n"
            f"startxref\n{xref_pos}\n%%EOF\n"
        ).encode()
        return bytes(out)


def render_text_image(text: str, width=1200, height=360) -> bytes:
    """흰 배경에 검정 한국어 텍스트를 렌더한 raw DeviceGray 바이트."""
    img = Image.new("L", (width, height), 255)
    draw = ImageDraw.Draw(img)
    font = ImageFont.truetype("/System/Library/Fonts/AppleSDGothicNeo.ttc", 72)
    draw.text((60, 130), text, fill=0, font=font)
    return img.tobytes(), width, height


def text_page(pdf, pages_num, font_num, text: str) -> int:
    content = f"BT /F1 24 Tf 72 770 Td ({text}) Tj ET".encode()
    c_num = pdf.add(b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream")
    return pdf.add(
        f"<< /Type /Page /Parent {pages_num} 0 R /MediaBox [0 0 595 842] "
        f"/Resources << /Font << /F1 {font_num} 0 R >> >> /Contents {c_num} 0 R >>".encode()
    )


def image_page(pdf, pages_num, text: str) -> int:
    raw, w, h = render_text_image(text)
    data = zlib.compress(raw, 9)
    img_num = pdf.add(
        f"<< /Type /XObject /Subtype /Image /Width {w} /Height {h} "
        f"/ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode "
        f"/Length {len(data)} >>\nstream\n".encode() + data + b"\nendstream"
    )
    content = b"q 500 0 0 150 48 620 cm /Im1 Do Q"
    c_num = pdf.add(b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream")
    return pdf.add(
        f"<< /Type /Page /Parent {pages_num} 0 R /MediaBox [0 0 595 842] "
        f"/Resources << /XObject << /Im1 {img_num} 0 R >> >> /Contents {c_num} 0 R >>".encode()
    )


def build_doc(page_specs) -> bytes:
    """page_specs: list of ('text', str) | ('image', str)"""
    pdf = PdfBuilder()
    catalog_num = pdf.reserve()
    pages_num = pdf.reserve()
    font_num = pdf.add(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
    )
    kids = []
    for kind, text in page_specs:
        if kind == "text":
            kids.append(text_page(pdf, pages_num, font_num, text))
        else:
            kids.append(image_page(pdf, pages_num, text))
    pdf.set(catalog_num, f"<< /Type /Catalog /Pages {pages_num} 0 R >>".encode())
    kids_str = " ".join(f"{k} 0 R" for k in kids)
    pdf.set(
        pages_num,
        f"<< /Type /Pages /Kids [{kids_str}] /Count {len(kids)} >>".encode(),
    )
    return pdf.build(catalog_num)


def write(name: str, data: bytes):
    path = f"{OUT_DIR}/{name}"
    with open(path, "wb") as f:
        f.write(data)
    print(f"  {path} ({len(data)} bytes)")


write(
    "multipage_text.pdf",
    build_doc(
        [
            ("text", "Alpha page one content here for testing"),
            ("text", "Bravo page two content here for testing"),
            ("text", "Charlie page three content here for testing"),
        ]
    ),
)
write(
    "multipage_scanned.pdf",
    build_doc(
        [
            ("image", "하나 문서 스캔"),
            ("image", "둘 검색 인식"),
            ("image", "셋 한글 복원"),
        ]
    ),
)
write(
    "mixed_text_scanned.pdf",
    build_doc(
        [
            ("text", "This mixed document has a readable first page for sure."),
            ("image", "스캔 전용 페이지"),
        ]
    ),
)
print("done")
