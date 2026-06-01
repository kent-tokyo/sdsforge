#!/usr/bin/env python3
"""
SDS ランダム30件 変換→再変換テスト — r27

language別に均等抽出（各約7-8件）してPDF→JSON→DOCX変換を実行し、
各JSONに quality_check.py を適用して問題を集計する。

Usage:
    python3 tools/roundtrip_random30.py [--seed SEED] [--n N]

Exit code: 0=全変換成功, 1=変換失敗あり
"""

import argparse
import json
import os
import random
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_DIR = SCRIPT_DIR.parent
BIN = PROJECT_DIR / "target" / "release" / "sds-converter"
QC_SCRIPT = SCRIPT_DIR / "quality_check.py"
SDS_BASE = PROJECT_DIR / "references" / "sds"
OUT_BASE = PROJECT_DIR / "references" / "json" / f"random30_{datetime.now().strftime('%Y%m%d_%H%M%S')}"

LANG_MAP = {
    "ja":    8,
    "zh-cn": 8,
    "zh-tw": 7,
    "en":    7,
}


def collect_pdfs_by_lang(base: Path) -> dict[str, list[Path]]:
    """Collect all PDFs per language folder."""
    result = {}
    for lang in ("ja", "en", "zh-cn", "zh-tw"):
        lang_dir = base / lang
        if not lang_dir.exists():
            result[lang] = []
            continue
        pdfs = sorted(lang_dir.rglob("*.pdf"))
        result[lang] = pdfs
    return result


def sample_balanced(pdf_by_lang: dict, n_per_lang: dict, seed: int) -> list[tuple[str, Path]]:
    """Return balanced random sample of (lang, path) pairs."""
    rng = random.Random(seed)
    selected = []
    for lang, n in n_per_lang.items():
        pool = pdf_by_lang.get(lang, [])
        k = min(n, len(pool))
        chosen = rng.sample(pool, k)
        for p in chosen:
            selected.append((lang, p))
    rng.shuffle(selected)
    return selected


def run_to_json(pdf_path: Path, lang: str, out_json: Path) -> tuple[bool, str]:
    """Convert PDF to JSON. Returns (success, stderr)."""
    cmd = [
        str(BIN), "to-json",
        "--input", str(pdf_path),
        "--output", str(out_json),
        "--lang", lang,
    ]
    t0 = time.time()
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=180
        )
        elapsed = time.time() - t0
        ok = result.returncode == 0 and out_json.exists()
        stderr = result.stderr.strip()
        return ok, stderr, elapsed
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (180s)", time.time() - t0
    except Exception as e:
        return False, str(e), time.time() - t0


def run_to_docx(json_path: Path, lang: str, out_docx: Path) -> tuple[bool, str, float]:
    """Convert JSON to DOCX. Returns (success, stderr, elapsed)."""
    cmd = [
        str(BIN), "to-docx",
        "--input", str(json_path),
        "--output", str(out_docx),
        "--lang", lang,
    ]
    t0 = time.time()
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=60
        )
        elapsed = time.time() - t0
        ok = result.returncode == 0 and out_docx.exists()
        return ok, result.stderr.strip(), elapsed
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (60s)", time.time() - t0
    except Exception as e:
        return False, str(e), time.time() - t0


def run_qc(json_path: Path, lang: str) -> dict:
    """Run quality_check.py. Returns parsed JSONL result or error dict."""
    cmd = [
        sys.executable, str(QC_SCRIPT),
        str(json_path), lang,
        "--jsonl",
    ]
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=30
        )
        # Last line of stdout is the JSONL record
        lines = [l for l in result.stdout.strip().splitlines() if l.startswith("{")]
        if lines:
            return json.loads(lines[-1])
        # Parse summary line as fallback
        summary_m = re.search(r"(\d+) CRIT \+ (\d+) HIGH \+ (\d+) MED = (\d+) total",
                               result.stdout)
        if summary_m:
            return {
                "crit": int(summary_m.group(1)),
                "high": int(summary_m.group(2)),
                "med":  int(summary_m.group(3)),
                "total": int(summary_m.group(4)),
                "issues": [],
                "stdout": result.stdout,
            }
        return {"crit": 0, "high": 0, "med": 0, "total": 0, "issues": [], "error": result.stderr}
    except Exception as e:
        return {"crit": 0, "high": 0, "med": 0, "total": 0, "issues": [], "error": str(e)}


def print_bar(label: str, width: int = 60):
    print(f"\n{'='*width}")
    print(f"  {label}")
    print(f"{'='*width}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=int, default=42, help="Random seed")
    parser.add_argument("--n", type=int, default=30, help="Total sample size")
    args = parser.parse_args()

    if not BIN.exists():
        print(f"ERROR: Binary not found: {BIN}", file=sys.stderr)
        print("Run: cargo build --release -p sds-converter", file=sys.stderr)
        sys.exit(1)

    api_key = os.environ.get("ANTHROPIC_API_KEY", "")
    if not api_key:
        print("ERROR: ANTHROPIC_API_KEY not set", file=sys.stderr)
        sys.exit(1)

    # Adjust per-lang quotas to sum to args.n
    total = args.n
    n_per_lang = {}
    base_each = total // 4
    remainder = total - base_each * 4
    langs_sorted = ["ja", "zh-cn", "zh-tw", "en"]
    for i, lang in enumerate(langs_sorted):
        n_per_lang[lang] = base_each + (1 if i < remainder else 0)

    print_bar(f"SDS ランダム{total}件 変換テスト  (seed={args.seed})")
    print(f"出力ディレクトリ: {OUT_BASE}")
    print(f"言語別件数: {n_per_lang}")

    pdf_by_lang = collect_pdfs_by_lang(SDS_BASE)
    for lang, pdfs in pdf_by_lang.items():
        print(f"  {lang}: {len(pdfs)} PDFs available")

    samples = sample_balanced(pdf_by_lang, n_per_lang, args.seed)
    print(f"\n選出件数: {len(samples)}")

    OUT_BASE.mkdir(parents=True, exist_ok=True)

    # --------------------------------------------------------
    # Run conversions
    # --------------------------------------------------------
    results = []
    total_json_ok = 0
    total_docx_ok = 0

    for i, (lang, pdf_path) in enumerate(samples, 1):
        rel = pdf_path.relative_to(SDS_BASE)
        print(f"\n[{i:02d}/{len(samples)}] {lang}/{rel}")

        # Prepare output paths
        out_dir = OUT_BASE / lang / rel.parent.name
        out_dir.mkdir(parents=True, exist_ok=True)
        stem = pdf_path.stem
        out_json = out_dir / f"{stem}.json"
        out_docx = out_dir / f"{stem}.docx"

        # PDF → JSON
        print(f"  PDF→JSON ... ", end="", flush=True)
        json_ok, json_err, json_t = run_to_json(pdf_path, lang, out_json)
        if json_ok:
            print(f"OK ({json_t:.1f}s)")
            total_json_ok += 1
        else:
            print(f"FAIL ({json_t:.1f}s)")
            if json_err:
                print(f"    stderr: {json_err[:200]}")

        # QC
        qc_result = {}
        if json_ok:
            qc_result = run_qc(out_json, lang)
            c = qc_result.get("crit", 0)
            h = qc_result.get("high", 0)
            m = qc_result.get("med", 0)
            status = "✓" if c == 0 and h == 0 else "⚠" if c == 0 else "✗"
            print(f"  QC: {status} CRIT={c} HIGH={h} MED={m}")
            # Print CRIT/HIGH details
            for iss in qc_result.get("issues", []):
                if iss.get("level") in ("CRIT", "HIGH"):
                    print(f"    [{iss['level']}] {iss['rule']}: {iss['message']}")

        # JSON → DOCX
        docx_ok = False
        docx_err = ""
        docx_t = 0.0
        if json_ok:
            print(f"  JSON→DOCX ... ", end="", flush=True)
            docx_ok, docx_err, docx_t = run_to_docx(out_json, lang, out_docx)
            if docx_ok:
                print(f"OK ({docx_t:.1f}s)")
                total_docx_ok += 1
            else:
                print(f"FAIL ({docx_t:.1f}s)")
                if docx_err:
                    print(f"    stderr: {docx_err[:200]}")

        results.append({
            "index": i,
            "lang": lang,
            "pdf": str(rel),
            "json_ok": json_ok,
            "json_err": json_err[:300] if json_err else "",
            "json_t": json_t,
            "qc": qc_result,
            "docx_ok": docx_ok,
            "docx_err": docx_err[:300] if docx_err else "",
            "docx_t": docx_t,
        })

    # --------------------------------------------------------
    # Summary
    # --------------------------------------------------------
    print_bar("サマリー")
    n = len(samples)
    print(f"PDF→JSON: {total_json_ok}/{n} 成功")
    print(f"JSON→DOCX: {total_docx_ok}/{total_json_ok} 成功 (JSON成功分)")

    # QC aggregate
    total_crit = sum(r["qc"].get("crit", 0) for r in results if r["json_ok"])
    total_high = sum(r["qc"].get("high", 0) for r in results if r["json_ok"])
    total_med  = sum(r["qc"].get("med", 0) for r in results if r["json_ok"])
    print(f"QC合計: CRIT={total_crit} HIGH={total_high} MED={total_med}")

    # Per-rule aggregation
    rule_counts: dict[str, dict] = {}
    for r in results:
        if not r["json_ok"]:
            continue
        for iss in r["qc"].get("issues", []):
            rid = iss["rule"]
            if rid not in rule_counts:
                rule_counts[rid] = {"level": iss["level"], "count": 0, "files": []}
            rule_counts[rid]["count"] += 1
            rule_counts[rid]["files"].append(f"{r['lang']}/{r['pdf']}")

    if rule_counts:
        print("\n--- ルール別問題ランキング ---")
        sorted_rules = sorted(rule_counts.items(), key=lambda x: (
            {"CRIT": 0, "HIGH": 1, "MED": 2}.get(x[1]["level"], 3),
            -x[1]["count"]
        ))
        for rid, info in sorted_rules:
            level = info["level"]
            count = info["count"]
            pct = count / total_json_ok * 100 if total_json_ok else 0
            print(f"  [{level}] {rid:40s} {count:3d}件  ({pct:.0f}%)")

    # Failures
    json_fails = [r for r in results if not r["json_ok"]]
    if json_fails:
        print(f"\n--- JSON変換失敗 ({len(json_fails)}件) ---")
        for r in json_fails:
            print(f"  [{r['lang']}] {r['pdf']}")
            if r["json_err"]:
                print(f"    {r['json_err'][:150]}")

    docx_fails = [r for r in results if r["json_ok"] and not r["docx_ok"]]
    if docx_fails:
        print(f"\n--- DOCX変換失敗 ({len(docx_fails)}件) ---")
        for r in docx_fails:
            print(f"  [{r['lang']}] {r['pdf']}")
            if r["docx_err"]:
                print(f"    {r['docx_err'][:150]}")

    # Per-file QC worst cases
    worst = sorted(
        [r for r in results if r["json_ok"]],
        key=lambda x: (-(x["qc"].get("crit", 0)*100 + x["qc"].get("high", 0)*10 + x["qc"].get("med", 0)))
    )[:5]
    print("\n--- QCワースト5件 ---")
    for r in worst:
        c = r["qc"].get("crit", 0)
        h = r["qc"].get("high", 0)
        m = r["qc"].get("med", 0)
        print(f"  [{r['lang']}] {r['pdf']}")
        print(f"    CRIT={c} HIGH={h} MED={m}")
        for iss in r["qc"].get("issues", [])[:5]:
            print(f"    [{iss['level']}] {iss['rule']}: {iss['message'][:80]}")

    # Save JSON report
    report_path = OUT_BASE / "report.json"
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump({
            "timestamp": datetime.now().isoformat(),
            "seed": args.seed,
            "n": n,
            "json_ok": total_json_ok,
            "docx_ok": total_docx_ok,
            "total_crit": total_crit,
            "total_high": total_high,
            "total_med": total_med,
            "rule_counts": {k: {"level": v["level"], "count": v["count"]} for k, v in rule_counts.items()},
            "results": results,
        }, f, ensure_ascii=False, indent=2)
    print(f"\nレポート保存: {report_path}")

    sys.exit(0 if total_json_ok == n else 1)


if __name__ == "__main__":
    main()
