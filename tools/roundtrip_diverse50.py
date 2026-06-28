#!/usr/bin/env python3
"""
SDS メーカー横断50件 変換→再変換テスト

各言語×メーカーディレクトリから1件ずつランダム選出し、メーカー多様性を確保。
合計が50を超える場合はランダムにサブサンプリング。

Usage:
    python3 tools/roundtrip_diverse50.py [--seed SEED] [--n N]

Exit code: 0=全変換成功, 1=変換失敗あり
"""

import argparse
import json
import os
import random
import re
import subprocess
import sys
import time
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_DIR = SCRIPT_DIR.parent
BIN = PROJECT_DIR / "target" / "release" / "sdsconv"
QC_SCRIPT = SCRIPT_DIR / "quality_check.py"
SDS_BASE = PROJECT_DIR / "references" / "sds"
OUT_BASE = PROJECT_DIR / "references" / "json" / f"diverse50_{datetime.now().strftime('%Y%m%d_%H%M%S')}"

LANGS = ("ja", "en", "zh-cn", "zh-tw")


def collect_by_manufacturer(base: Path) -> list[tuple[str, str, Path]]:
    """Return list of (lang, manufacturer, pdf_path) — one random PDF per mfr."""
    entries = []
    for lang in LANGS:
        lang_dir = base / lang
        if not lang_dir.exists():
            continue
        for mfr_dir in sorted(lang_dir.iterdir()):
            if not mfr_dir.is_dir():
                continue
            pdfs = sorted(mfr_dir.rglob("*.pdf"))
            if pdfs:
                entries.append((lang, mfr_dir.name, pdfs))
    return entries


def run_to_json(pdf_path: Path, lang: str, out_json: Path) -> tuple[bool, str, float]:
    """Convert PDF to JSON. Uses process-group kill on timeout to handle macOS subprocess hang."""
    cmd = [str(BIN), "to-json", "--input", str(pdf_path), "--output", str(out_json), "--lang", lang]
    t0 = time.time()
    import os, signal
    _TIMEOUT = 300  # raised from 180 to 300; process-group kill prevents macOS hang
    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,  # new process group → killpg kills all threads/children
        )
        try:
            stdout_b, stderr_b = proc.communicate(timeout=_TIMEOUT)
            elapsed = time.time() - t0
            ok = proc.returncode == 0 and out_json.exists()
            return ok, stderr_b.decode("utf-8", errors="replace").strip(), elapsed
        except subprocess.TimeoutExpired:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except ProcessLookupError:
                pass
            proc.communicate()
            return False, f"TIMEOUT ({_TIMEOUT}s)", time.time() - t0
    except Exception as e:
        return False, str(e), time.time() - t0


def run_to_docx(json_path: Path, lang: str, out_docx: Path) -> tuple[bool, str, float]:
    cmd = [str(BIN), "to-docx", "--input", str(json_path), "--output", str(out_docx), "--lang", lang]
    t0 = time.time()
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        elapsed = time.time() - t0
        ok = result.returncode == 0 and out_docx.exists()
        return ok, result.stderr.strip(), elapsed
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (60s)", time.time() - t0
    except Exception as e:
        return False, str(e), time.time() - t0


def run_qc(json_path: Path, lang: str) -> dict:
    cmd = [sys.executable, str(QC_SCRIPT), str(json_path), lang, "--jsonl"]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        lines = [l for l in result.stdout.strip().splitlines() if l.startswith("{")]
        if lines:
            return json.loads(lines[-1])
        summary_m = re.search(r"(\d+) CRIT \+ (\d+) HIGH \+ (\d+) MED = (\d+) total", result.stdout)
        if summary_m:
            return {"crit": int(summary_m.group(1)), "high": int(summary_m.group(2)),
                    "med": int(summary_m.group(3)), "total": int(summary_m.group(4)), "issues": []}
        return {"crit": 0, "high": 0, "med": 0, "total": 0, "issues": [], "error": result.stderr}
    except Exception as e:
        return {"crit": 0, "high": 0, "med": 0, "total": 0, "issues": [], "error": str(e)}


def print_bar(label: str, width: int = 64):
    print(f"\n{'='*width}\n  {label}\n{'='*width}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--n", type=int, default=50)
    args = parser.parse_args()

    if not BIN.exists():
        print(f"ERROR: Binary not found: {BIN}", file=sys.stderr)
        sys.exit(1)
    if not os.environ.get("ANTHROPIC_API_KEY"):
        print("ERROR: ANTHROPIC_API_KEY not set", file=sys.stderr)
        sys.exit(1)

    rng = random.Random(args.seed)
    entries = collect_by_manufacturer(SDS_BASE)

    # Sample 1 PDF per manufacturer
    candidates = []
    for lang, mfr, pdfs in entries:
        chosen = rng.choice(pdfs)
        candidates.append((lang, mfr, chosen))

    # Show coverage
    print_bar(f"SDS メーカー横断{args.n}件テスト  (seed={args.seed})")
    lang_counts = {}
    for lang, mfr, _ in candidates:
        lang_counts[lang] = lang_counts.get(lang, 0) + 1
    print(f"メーカー総数: {len(candidates)} ディレクトリ")
    for lang, cnt in sorted(lang_counts.items()):
        print(f"  {lang}: {cnt}メーカー")

    # Subsample to N, keeping language balance
    if len(candidates) > args.n:
        # Stratified subsample: pick proportionally per language
        by_lang = {}
        for item in candidates:
            by_lang.setdefault(item[0], []).append(item)
        total = len(candidates)
        selected = []
        for lang, items in sorted(by_lang.items()):
            quota = max(1, round(len(items) / total * args.n))
            chosen = rng.sample(items, min(quota, len(items)))
            selected.extend(chosen)
        # Trim / top-up to exactly args.n
        rng.shuffle(selected)
        selected = selected[:args.n]
    else:
        selected = candidates
        rng.shuffle(selected)

    # Final language breakdown
    final_langs = {}
    for lang, mfr, _ in selected:
        final_langs[lang] = final_langs.get(lang, 0) + 1
    print(f"\n選出 {len(selected)}件  言語別: {final_langs}")
    print(f"出力ディレクトリ: {OUT_BASE}")
    OUT_BASE.mkdir(parents=True, exist_ok=True)

    results = []
    total_json_ok = 0
    total_docx_ok = 0

    for i, (lang, mfr, pdf_path) in enumerate(selected, 1):
        rel = pdf_path.relative_to(SDS_BASE)
        print(f"\n[{i:02d}/{len(selected)}] [{lang}/{mfr}] {pdf_path.name}")

        out_dir = OUT_BASE / lang / mfr
        out_dir.mkdir(parents=True, exist_ok=True)
        out_json = out_dir / f"{pdf_path.stem}.json"
        out_docx = out_dir / f"{pdf_path.stem}.docx"

        print(f"  PDF→JSON ... ", end="", flush=True)
        json_ok, json_err, json_t = run_to_json(pdf_path, lang, out_json)
        if json_ok:
            print(f"OK ({json_t:.1f}s)")
            total_json_ok += 1
        else:
            print(f"FAIL ({json_t:.1f}s)")
            if json_err:
                print(f"    stderr: {json_err[:200]}")

        qc_result = {}
        if json_ok:
            qc_result = run_qc(out_json, lang)
            c = qc_result.get("crit", 0)
            h = qc_result.get("high", 0)
            m = qc_result.get("med", 0)
            status = "✓" if c == 0 and h == 0 else "⚠" if c == 0 else "✗"
            print(f"  QC: {status} CRIT={c} HIGH={h} MED={m}")
            for iss in qc_result.get("issues", []):
                if iss.get("level") in ("CRIT", "HIGH"):
                    print(f"    [{iss['level']}] {iss['rule']}: {iss['message'][:80]}")

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
            "index": i, "lang": lang, "manufacturer": mfr, "pdf": str(rel),
            "json_ok": json_ok, "json_err": json_err[:300] if json_err else "",
            "json_t": json_t, "qc": qc_result,
            "docx_ok": docx_ok, "docx_err": docx_err[:300] if docx_err else "", "docx_t": docx_t,
        })

    # ── Summary ──
    print_bar("サマリー")
    n = len(selected)
    print(f"PDF→JSON: {total_json_ok}/{n} 成功")
    print(f"JSON→DOCX: {total_docx_ok}/{total_json_ok} 成功 (JSON成功分)")

    total_crit = sum(r["qc"].get("crit", 0) for r in results if r["json_ok"])
    total_high = sum(r["qc"].get("high", 0) for r in results if r["json_ok"])
    total_med  = sum(r["qc"].get("med", 0) for r in results if r["json_ok"])
    print(f"QC合計: CRIT={total_crit} HIGH={total_high} MED={total_med}")

    # Rule ranking
    rule_counts: dict[str, dict] = {}
    for r in results:
        if not r["json_ok"]:
            continue
        for iss in r["qc"].get("issues", []):
            rid = iss["rule"]
            if rid not in rule_counts:
                rule_counts[rid] = {"level": iss["level"], "count": 0, "mfrs": set()}
            rule_counts[rid]["count"] += 1
            rule_counts[rid]["mfrs"].add(f"{r['lang']}/{r['manufacturer']}")

    if rule_counts:
        print("\n--- ルール別問題ランキング ---")
        sorted_rules = sorted(rule_counts.items(), key=lambda x: (
            {"CRIT": 0, "HIGH": 1, "MED": 2}.get(x[1]["level"], 3), -x[1]["count"]
        ))
        for rid, info in sorted_rules:
            level = info["level"]
            count = info["count"]
            pct = count / total_json_ok * 100 if total_json_ok else 0
            n_mfrs = len(info["mfrs"])
            print(f"  [{level}] {rid:42s} {count:3d}件  ({pct:.0f}%)  {n_mfrs}メーカー")

    # Language breakdown of issues
    print("\n--- 言語別QCサマリー ---")
    for lang in LANGS:
        lang_results = [r for r in results if r["lang"] == lang and r["json_ok"]]
        if not lang_results:
            continue
        lc = sum(r["qc"].get("crit", 0) for r in lang_results)
        lh = sum(r["qc"].get("high", 0) for r in lang_results)
        lm = sum(r["qc"].get("med", 0) for r in lang_results)
        n_lang = len(lang_results)
        mfrs = set(r["manufacturer"] for r in lang_results)
        print(f"  {lang:6s} ({n_lang}件 / {len(mfrs)}メーカー): CRIT={lc} HIGH={lh} MED={lm}")

    # Manufacturer breakdown of worst cases
    mfr_scores = {}
    for r in results:
        if not r["json_ok"]:
            continue
        key = f"{r['lang']}/{r['manufacturer']}"
        c = r["qc"].get("crit", 0)
        h = r["qc"].get("high", 0)
        m = r["qc"].get("med", 0)
        mfr_scores[key] = mfr_scores.get(key, 0) + c * 100 + h * 10 + m

    print("\n--- メーカー別ワースト10 ---")
    worst_mfrs = sorted(mfr_scores.items(), key=lambda x: -x[1])[:10]
    for mfr, score in worst_mfrs:
        mfr_results = [r for r in results if f"{r['lang']}/{r['manufacturer']}" == mfr and r["json_ok"]]
        for r in mfr_results:
            c = r["qc"].get("crit", 0)
            h = r["qc"].get("high", 0)
            m = r["qc"].get("med", 0)
            print(f"  {mfr:35s} CRIT={c} HIGH={h} MED={m}")
            for iss in r["qc"].get("issues", [])[:3]:
                if iss.get("level") in ("CRIT", "HIGH"):
                    print(f"    [{iss['level']}] {iss['rule']}: {iss['message'][:70]}")

    # Failures
    json_fails = [r for r in results if not r["json_ok"]]
    if json_fails:
        print(f"\n--- JSON変換失敗 ({len(json_fails)}件) ---")
        for r in json_fails:
            print(f"  [{r['lang']}/{r['manufacturer']}] {r['pdf']}")

    # Save report
    report_path = OUT_BASE / "report.json"
    serializable = [{**r, "qc": {k: (list(v) if isinstance(v, set) else v)
                                  for k, v in r["qc"].items()}}
                     for r in results]
    # Fix sets in rule_counts
    rc_serial = {k: {**v, "mfrs": list(v["mfrs"])} for k, v in rule_counts.items()}
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump({
            "timestamp": datetime.now().isoformat(), "seed": args.seed, "n": n,
            "json_ok": total_json_ok, "docx_ok": total_docx_ok,
            "total_crit": total_crit, "total_high": total_high, "total_med": total_med,
            "rule_counts": {k: {"level": v["level"], "count": v["count"], "n_manufacturers": len(v["mfrs"])}
                            for k, v in rule_counts.items()},
            "results": results,
        }, f, ensure_ascii=False, indent=2, default=str)
    print(f"\nレポート保存: {report_path}")

    sys.exit(0 if total_json_ok == n else 1)


if __name__ == "__main__":
    main()
