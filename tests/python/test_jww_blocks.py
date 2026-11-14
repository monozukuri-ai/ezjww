"""Large reader block lists, independently encoded beyond the writer's limits."""

import struct

import pytest

import ezjww


def block_archive(count: int) -> tuple[bytes, int]:
    # Reuse only the valid v700 header. Encode the MFC body independently so
    # reader coverage is not constrained by the writer's block-count limit.
    header = ezjww.to_jww_bytes(ezjww.new_jww_document())[:-8]
    base = struct.pack("<IBHHHHH", 0, 1, 1, 0, 0, 0, 0)

    def new_class(name: bytes) -> bytes:
        return struct.pack("<HHH", 0xFFFF, 700, len(name)) + name

    body = bytearray(struct.pack("<H", 1))
    body += new_class(b"CDataBlock")  # class PID 1, object PID 2
    body += base + struct.pack("<5dI", 0, 0, 1, 1, 0, count - 1)
    block_offset = len(header) + len(body)
    body += (
        struct.pack("<H", count)
        if count < 0xFFFF
        else struct.pack("<HI", 0xFFFF, count)
    )
    for index in range(count):
        body += new_class(b"CDataList") if index == 0 else struct.pack("<H", 0x8003)
        body += base + struct.pack("<III", index, 0, 0)
        name = f"B{index}".encode("ascii")
        body += bytes([len(name)]) + name + struct.pack("<H", 1)
        body += new_class(b"CDataSen") if index == 0 else struct.pack("<H", 0x8005)
        body += base + struct.pack("<4d", 0, 0, 10, index % 7 + 1)
    # A real image archive after the definitions checks where parsing resumes.
    body += struct.pack("<I", 1) + b"\x08test.bmp" + struct.pack("<I", 3) + b"BMP"
    return header + body, block_offset


@pytest.mark.parametrize("count", [10_000, 10_001, 65_535])
def test_large_block_lists_keep_definitions_references_and_images(tmp_path, count):
    data, _ = block_archive(count)
    path = tmp_path / "blocks.jww"
    path.write_bytes(data)
    parsed = ezjww.read_document(str(path))
    assert len(parsed["block_defs"]) == count
    for index, block in enumerate(parsed["block_defs"]):
        assert block["number"] == index
        assert block["name"] == f"B{index}"
        assert len(block["entities"]) == 1
        assert block["entities"][0]["end_y"] == index % 7 + 1
    assert parsed["validation"]["resolved_references"] == 1
    assert parsed["validation"]["unresolved_def_numbers"] == []
    assert parsed["images"] == [
        {"name": "test.bmp", "data": b"BMP", "compressed": False}
    ]
    assert parsed["diagnostics"] == []


def test_block_list_diagnostic_is_exposed_to_python_and_audit(tmp_path):
    data, offset = block_archive(1)
    # Drop the first definition; leave a valid count and incomplete class tag.
    path = tmp_path / "truncated.jww"
    path.write_bytes(data[:offset] + b"\x01\x00\xff")
    parsed = ezjww.read_document(str(path))
    assert parsed["block_defs"] == []
    assert parsed["images"] == []
    diagnostic, = parsed["diagnostics"]
    assert diagnostic["code"] == ezjww.BLOCK_LIST_TRUNCATED
    assert diagnostic["severity"] == "error"
    assert diagnostic["action"] == "skipped"
    assert diagnostic["details"]["expected_blocks"] == 1
    assert diagnostic["details"]["parsed_blocks"] == 0
    assert diagnostic["details"]["byte_offset"] == offset + 2
    assert "unexpected EOF" in diagnostic["details"]["error"]
    assert ezjww.BLOCK_LIST_TRUNCATED in ezjww.audit(path)["issue_codes"]
