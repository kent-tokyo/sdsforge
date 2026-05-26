#!/usr/bin/env python3
"""
Generate src/schema/generated.rs from the MHLW SDS Excel definition file.
Usage: python3 tools/generate_schema.py
"""

import re
import sys
from collections import defaultdict
from pathlib import Path

try:
    import openpyxl
except ImportError:
    print("Install openpyxl first: pip3 install openpyxl", file=sys.stderr)
    sys.exit(1)

EXCEL_PATH = Path(__file__).parent.parent / "references" / "SDS_データ交換フォーマット項目定義書.xlsx"
OUTPUT_PATH = Path(__file__).parent.parent / "src" / "schema" / "generated.rs"

# Definitions section prefix
DEFS = "definitions"

# Rust reserved keywords — field names that clash must be prefixed with r#
RUST_KEYWORDS = {
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for",
    "if", "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut",
    "override", "priv", "pub", "ref", "return", "self", "static", "struct", "super",
    "trait", "true", "try", "type", "typeof", "union", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
}


# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------

def to_snake_case(name: str) -> str:
    """Convert a key name (PascalCase/camelCase/special-chars) to snake_case."""
    # Replace special characters
    name = name.replace("-", "_").replace("/", "_")
    # Insert underscore between an uppercase sequence and a following uppercase+lowercase
    s1 = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    # Insert underscore between lowercase/digit and uppercase
    s2 = re.sub(r"([a-z\d])([A-Z])", r"\1_\2", s1)
    result = s2.lower()
    # Ensure it's a valid Rust identifier (can't start with digit)
    if result and result[0].isdigit():
        result = "_" + result
    return result


def to_safe_field_name(snake: str) -> str:
    """Escape Rust keywords with r#."""
    if snake in RUST_KEYWORDS:
        return f"r#{snake}"
    return snake


def to_pascal(name: str) -> str:
    """Convert a name to PascalCase, handling camelCase, hyphens, etc."""
    # Split on underscores, hyphens, slashes
    parts = re.split(r"[-_/]", name)
    return "".join(p[0].upper() + p[1:] if p else "" for p in parts)


def path_to_rust_type(path: tuple) -> str:
    """Convert a path tuple to a Rust struct type name."""
    if path == ("(root)",):
        return "SdsRoot"
    if path[0] == DEFS:
        # Strip 'definitions' prefix; join remaining segments in PascalCase
        def_segments = path[1:]
        return "".join(to_pascal(seg) for seg in def_segments)
    # Full path concatenation, each segment PascalCase-ified
    return "".join(to_pascal(seg) for seg in path)


def definition_rust_type(ref: str) -> str:
    """Extract the Rust type name from a #/definitions/... reference."""
    # e.g. "#/definitions/AdditionalInfo" -> "AdditionalInfo"
    # e.g. "#/definitions/gazetteNo" -> "GazetteNo"
    name = ref.split("/")[-1]
    return to_pascal(name)


# ---------------------------------------------------------------------------
# Parse Excel
# ---------------------------------------------------------------------------

def parse_excel(path: Path) -> list[dict]:
    """Return a list of row dicts for all data rows (row 4 onward)."""
    wb = openpyxl.load_workbook(path)
    ws = wb.active
    rows = []
    for excel_row in ws.iter_rows(min_row=4):
        vals = [str(c.value) if c.value is not None else "" for c in excel_row]
        # Path segments in columns C-I (index 2-8)
        path = tuple(v for v in vals[2:9] if v)
        if not path:
            continue
        row_type = vals[12]  # 種類 column
        if row_type not in ("見出し", "項目呼び出し", "文字列", "数値", "整数", "真偽値"):
            continue
        rows.append({
            "path": path,
            "key": path[-1],           # last segment = JSON key name
            "repeat": vals[11] == "●",
            "type": row_type,
            "ref": vals[19],           # #/definitions/... or ""
        })
    return rows


# ---------------------------------------------------------------------------
# Build struct tree
# ---------------------------------------------------------------------------

def get_parent_path(path: tuple) -> tuple | None:
    """Return the parent path for a given path."""
    if path == ("(root)",):
        return None
    if len(path) == 1:
        # Top-level items (Datasheet, Identification, ..., definitions) → children of (root)
        return ("(root)",)
    return path[:-1]


def build_struct_map(rows: list[dict]) -> dict[tuple, list[dict]]:
    """
    Returns a dict: parent_path -> list of child row dicts.
    Only 見出し paths are parents (structs).
    """
    # Collect all 見出し paths
    struct_paths = {r["path"] for r in rows if r["type"] == "見出し"}

    # For each row, find its immediate parent 見出し path
    children_of: dict[tuple, list[dict]] = defaultdict(list)
    for row in rows:
        parent = get_parent_path(row["path"])
        if parent is None:
            continue
        # Only add as child if parent is a struct path
        if parent in struct_paths:
            children_of[parent].append(row)

    return dict(children_of)


# ---------------------------------------------------------------------------
# Rust code generation
# ---------------------------------------------------------------------------

def rust_field_type(row: dict) -> str:
    """Return the Rust type string (without Option<>/Vec<> wrapping)."""
    t = row["type"]
    if t == "文字列":
        return "String"
    elif t == "数値":
        return "f64"
    elif t == "整数":
        return "i64"
    elif t == "真偽値":
        return "bool"
    elif t == "見出し":
        return path_to_rust_type(row["path"])
    elif t == "項目呼び出し":
        return definition_rust_type(row["ref"])
    else:
        return "String"  # fallback


def wrap_type(inner: str, repeat: bool) -> str:
    if repeat:
        return f"Option<Vec<{inner}>>"
    else:
        return f"Option<{inner}>"


def emit_struct(path: tuple, children: list[dict]) -> list[str]:
    """Emit Rust struct definition lines."""
    struct_name = path_to_rust_type(path)
    lines = []
    lines.append("#[derive(Debug, Clone, Serialize, Deserialize, Default)]")
    lines.append(f"pub struct {struct_name} {{")

    seen_fields: set[str] = set()
    for child in children:
        key = child["key"]
        snake = to_snake_case(key)
        field_name = to_safe_field_name(snake)  # r#use, r#type, etc.

        # Deduplicate fields (same key at same level; use the snake form as the dedup key)
        if snake in seen_fields:
            continue
        seen_fields.add(snake)

        inner_type = rust_field_type(child)
        full_type = wrap_type(inner_type, child["repeat"])

        # Serde annotations — always include rename + skip_serializing_if.
        # For Option<String> fields that LLMs sometimes return as arrays, add
        # flex_string_opt so a JSON array is joined into a single string rather
        # than causing a deserialization error that skips the whole section.
        #
        # Fields that get flex_string_opt:
        #   - FullText (non-array variant) — LLMs sometimes wrap single-section text in []
        #   - Substance / Condition in HazardousDecompositionProducts — LLMs list
        #     decomposition products as arrays (e.g. ["CO2", "NH3", ...])
        is_plain_string = (inner_type == "String" and not child["repeat"])
        is_vec_string  = (inner_type == "String" and child["repeat"])
        flex_string_keys = {"FullText", "Substance", "Condition"}
        if is_vec_string and key == "FullText":
            # AdditionalInfo.FullText is Vec<String>; accept bare strings too
            lines.append(
                f'    #[serde(rename = "{key}", skip_serializing_if = "Option::is_none",\n'
                f'            default, deserialize_with = "crate::schema::serde_flex::flex_vec_string_opt")]'
            )
        elif is_plain_string and key in flex_string_keys:
            lines.append(
                f'    #[serde(rename = "{key}", skip_serializing_if = "Option::is_none",\n'
                f'            default, deserialize_with = "crate::schema::serde_flex::flex_string_opt")]'
            )
        else:
            lines.append(f'    #[serde(rename = "{key}", skip_serializing_if = "Option::is_none")]')
        lines.append(f"    pub {field_name}: {full_type},")

    lines.append("}")
    return lines


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    print(f"Reading {EXCEL_PATH} ...", file=sys.stderr)
    rows = parse_excel(EXCEL_PATH)
    print(f"  Parsed {len(rows)} rows", file=sys.stderr)

    struct_map = build_struct_map(rows)
    print(f"  Found {len(struct_map)} struct definitions", file=sys.stderr)

    # Collect all struct paths, ordered: definitions first, then by path
    # This ensures forward declarations work (definitions are used by main structs)
    def sort_key(path):
        is_def = 1 if path[0] == DEFS else 2
        is_root = 3 if path == ("(root)",) else is_def
        return (is_root, path)

    all_struct_paths = sorted(struct_map.keys(), key=sort_key)

    # Generate code
    output_lines = [
        "//! AUTO-GENERATED by tools/generate_schema.py — do not edit manually.",
        "//! Run `python3 tools/generate_schema.py` to regenerate.",
        "",
        "#![allow(non_snake_case)]",
        "",
        "use serde::{Deserialize, Serialize};",
        "",
    ]

    for path in all_struct_paths:
        # Skip the 'definitions' header row itself (it has no useful fields as a struct)
        if path == (DEFS,):
            continue

        children = struct_map.get(path, [])
        struct_lines = emit_struct(path, children)
        output_lines.extend(struct_lines)
        output_lines.append("")

    code = "\n".join(output_lines)

    print(f"Writing {OUTPUT_PATH} ...", file=sys.stderr)
    OUTPUT_PATH.write_text(code, encoding="utf-8")

    struct_count = sum(1 for line in output_lines if line.startswith("pub struct"))
    print(f"  Generated {struct_count} structs", file=sys.stderr)
    print("Done.", file=sys.stderr)


if __name__ == "__main__":
    main()
