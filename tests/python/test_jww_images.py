"""Embedded images (version 700): archive, `^@BM` placements and helpers."""

import struct

import pytest
import ezjww


def bmp(width: int = 2, height: int = 2) -> bytes:
    """A minimal 24-bit BMP (rows padded to 4 bytes)."""
    row = (width * 3 + 3) & ~3
    pixels = bytes([0x10, 0x20, 0x30] * width + [0] * (row - width * 3)) * height
    header = struct.pack("<2sIHHI", b"BM", 54 + len(pixels), 0, 0, 54)
    info = struct.pack("<IiiHHIIiiII", 40, width, height, 1, 24, 0, len(pixels), 2835, 2835, 0, 0)
    return header + info + pixels


def test_embed_and_place_image_round_trip(tmp_path):
    drawing = ezjww.new()
    name = drawing.embed_image("logo.bmp", bmp())
    assert name == "logo.bmp"
    assert drawing.images[0]["name"] == "logo.bmp.gz"
    assert drawing.images[0]["data"][:2] == b"\x1f\x8b"
    placed = drawing.modelspace().add_image("logo.bmp", (10.0, 20.0), 100.0, 64.5161)
    assert placed["content"] == "^@BM%temp%logo.bmp,100,64.5161,0,0,1,0,255,255,255"
    assert (placed["end_x"], placed["end_y"]) == (110.0, 20.0)
    assert (placed["size_x"], placed["size_y"]) == (2.0, 2.0)

    path = tmp_path / "image.jww"
    drawing.saveas(path)
    parsed = ezjww.read_document(str(path))
    assert [image["name"] for image in parsed["images"]] == ["logo.bmp.gz"]
    assert parsed["images"][0]["compressed"] is True
    assert ezjww.decompress_image(parsed["images"][0]) == bmp()
    entity = parsed["entities"][0]
    assert entity["type"] == "TEXT"
    assert entity["image"] == {
        "path": "%temp%logo.bmp",
        "file_name": "logo.bmp",
        "width": 100.0,
        "height": 64.5161,
        "extra": ["0", "0", "1", "0", "255", "255", "255"],
    }
    assert ezjww.image_reference(entity["content"]) == entity["image"]
    assert ezjww.image_reference("plain text") is None
    assert ezjww.readfile(path).images == parsed["images"]
    assert parsed["diagnostics"] == []

    converted = ezjww.to_write_document(parsed)
    assert converted["document"] is not None, converted["diagnostics"]
    assert converted["document"]["images"][0]["name"] == "logo.bmp.gz"
    rewritten = tmp_path / "again.jww"
    rewritten.write_bytes(ezjww.to_jww_bytes(converted["document"]))
    again = ezjww.read_document(str(rewritten))
    assert again["images"] == parsed["images"]
    assert again["entities"] == parsed["entities"]


def test_uncompressed_entries_and_external_links(tmp_path):
    drawing = ezjww.new()
    raw = b"\x89PNG\r\n\x1a\n" + bytes(range(32))
    assert drawing.embed_image("raw.png", raw, compress=False) == "raw.png"
    assert drawing.images[0] == {"name": "raw.png", "data": raw}
    space = drawing.modelspace()
    space.add_image("raw.png", (0.0, 0.0), 30.0, 20.0, angle=90.0)
    space.add_image("C:\\pictures\\site.png", (5.0, 5.0), 10.0, 5.0, embedded=False)
    path = tmp_path / "links.jww"
    drawing.saveas(path)
    parsed = ezjww.read_document(str(path))
    assert parsed["images"][0]["compressed"] is False
    assert ezjww.decompress_image(parsed["images"][0]) == raw
    rotated, linked = parsed["entities"]
    assert rotated["angle"] == 90.0
    assert rotated["end_x"] == pytest.approx(0.0)
    assert rotated["end_y"] == pytest.approx(30.0)
    assert linked["image"]["path"] == "C:\\pictures\\site.png"
    assert linked["image"]["file_name"] == "site.png"


def test_helpers_and_validation():
    assert ezjww.image_reference_content("%temp%a.bmp", 12.5, 7) == (
        "^@BM%temp%a.bmp,12.5,7,0,0,1,0,255,255,255"
    )
    assert ezjww.image_reference_content("a.bmp", 1, 2, ["9", "8"]) == "^@BMa.bmp,1,2,9,8"
    assert ezjww.decompress_image({"name": "x.bmp.gz", "data": ezjww.compress_image(b"xy")}) == b"xy"
    assert ezjww.IMAGE_LIST_TRUNCATED in ezjww.ALL_ISSUE_CODES

    drawing = ezjww.new()
    drawing.modelspace().add_image("missing.bmp", (0.0, 0.0), 10.0, 10.0)
    with pytest.raises(ValueError, match="not embedded"):
        drawing.to_jww_bytes()
    with pytest.raises(ValueError):
        drawing.embed_image("dir/x.bmp", b"x")
    with pytest.raises(ValueError):
        drawing.embed_image("", b"x")
    with pytest.raises(ValueError):
        drawing.embed_image("x.bmp", b"")
    drawing.embed_image("a.bmp", b"x")
    with pytest.raises(ValueError, match="duplicate"):
        drawing.embed_image("a.bmp", b"y")
    with pytest.raises(ValueError):
        drawing.modelspace().add_image("a.bmp", (0.0, 0.0), 0.0, 1.0)
    with pytest.raises(ValueError):
        drawing.modelspace().add_image("sub/a.bmp", (0.0, 0.0), 1.0, 1.0)

    document = ezjww.new_jww_document()
    document["images"] = [{"name": "a.bmp", "data": b"ABC"}]
    from ezjww import _core

    parsed = _core.jww_write_document_to_document(document)
    assert parsed["images"] == [{"name": "a.bmp", "data": b"ABC", "compressed": False}]
    document["images"] = [{"name": "a.bmp", "data": b"ABC", "compressed": True}]
    with pytest.raises(ValueError, match="compressed"):
        ezjww.to_jww_bytes(document)
    document["images"] = [{"name": "a.bmp.gz", "data": b"ABC"}]
    with pytest.raises(ValueError, match="gzip"):
        ezjww.to_jww_bytes(document)
