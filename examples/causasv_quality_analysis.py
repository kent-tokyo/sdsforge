"""
causasv quality analysis — SDS JSON 品質劣化の因果寄与分析サンプル

Usage:
    pip install causasv pandas
    python3 examples/causasv_quality_analysis.py runs/eval_YYYYMMDD/causasv_features.csv

causasv は DAG に沿った Asymmetric Shapley Value (ASV) で特徴量寄与を計算します。
通常の SHAP と異なり、因果グラフの順序に従うため
「スキャン PDF だから悪いのか」「言語差なのか」「LLM モデル差なのか」を分離できます。
"""

import sys
import pandas as pd
import numpy as np

try:
    import causasv
except ImportError:
    print("causasv が見つかりません。pip install causasv を実行してください。")
    sys.exit(1)

# ---------------------------------------------------------------------------
# 1. データ読み込み
# ---------------------------------------------------------------------------

csv_path = sys.argv[1] if len(sys.argv) > 1 else "runs/eval/causasv_features.csv"
df = pd.read_csv(csv_path)
print(f"Loaded {len(df)} rows from {csv_path}")
print(df[["filename", "overall_score", "grade", "critical_count", "high_count"]].head(10))

# ---------------------------------------------------------------------------
# 2. 特徴量エンジニアリング
# ---------------------------------------------------------------------------

# ターゲット: スコア 80 未満を品質劣化とみなす二値変数
df["quality_fail"] = (df["overall_score"] < 80) | (df["critical_count"] > 0)

# ファイルタイプをダミー化
df["is_pdf"]  = (df["file_type"] == "pdf").astype(int)
df["is_docx"] = (df["file_type"] == "docx").astype(int)

# 言語をダミー化
df["lang_ja"]   = (df["source_language"] == "ja").astype(int)
df["lang_zh_cn"]= (df["source_language"] == "zh-CN").astype(int)
df["lang_zh_tw"]= (df["source_language"] == "zh-TW").astype(int)
df["lang_en"]   = (df["source_language"] == "en").astype(int)

feature_cols = [
    "is_pdf", "is_docx",
    "lang_ja", "lang_zh_cn", "lang_zh_tw", "lang_en",
    "file_size_kb", "text_length_chars",
    "cas_count_in_source", "h_code_count_in_source",
    "p_code_count_in_source", "un_count_in_source",
    "cas_coverage", "h_code_coverage", "p_code_coverage",
    "populated_section_count", "empty_section_count",
]

X = df[feature_cols].fillna(0)
y = df["quality_fail"].astype(int)

# ---------------------------------------------------------------------------
# 3. DAG 定義 (因果順序)
#
# file_type ──→ text_length ──→ section_coverage ──→ quality_fail
#                  ↑                  ↑
# file_size ───────┘           cas/h/p coverage ──────→ quality_fail
# language ──────────────────→ section_coverage
# ---------------------------------------------------------------------------

dag = {
    "is_pdf":                  [],
    "is_docx":                 [],
    "file_size_kb":            ["is_pdf", "is_docx"],
    "text_length_chars":       ["is_pdf", "is_docx", "file_size_kb"],
    "lang_ja":                 [],
    "lang_zh_cn":              [],
    "lang_zh_tw":              [],
    "lang_en":                 [],
    "cas_count_in_source":     ["text_length_chars"],
    "h_code_count_in_source":  ["text_length_chars"],
    "p_code_count_in_source":  ["text_length_chars"],
    "un_count_in_source":      ["text_length_chars"],
    "cas_coverage":            ["cas_count_in_source", "lang_ja", "lang_zh_cn"],
    "h_code_coverage":         ["h_code_count_in_source", "lang_ja", "lang_zh_cn"],
    "p_code_coverage":         ["p_code_count_in_source"],
    "populated_section_count": ["text_length_chars", "lang_ja", "lang_zh_cn", "lang_zh_tw", "lang_en"],
    "empty_section_count":     ["populated_section_count"],
}

# ---------------------------------------------------------------------------
# 4. causasv による ASV 計算
# ---------------------------------------------------------------------------

print("\n=== ASV 計算中 (DAG-aware Shapley Values) ===")
from sklearn.ensemble import GradientBoostingClassifier

model = GradientBoostingClassifier(n_estimators=100, max_depth=3, random_state=42)
model.fit(X, y)

asv_values = causasv.compute(
    model=model,
    X=X,
    dag=dag,
    feature_names=feature_cols,
    n_samples=min(200, len(X)),
)

# ---------------------------------------------------------------------------
# 5. 結果表示
# ---------------------------------------------------------------------------

mean_asv = np.abs(asv_values).mean(axis=0)
ranking = sorted(zip(feature_cols, mean_asv), key=lambda x: x[1], reverse=True)

print("\n=== 品質劣化への寄与ランキング (mean |ASV|) ===")
print(f"{'特徴量':<30} {'寄与':>10}")
print("-" * 42)
for feat, val in ranking:
    bar = "█" * int(val * 200)
    print(f"{feat:<30} {val:>8.4f}  {bar}")

print(f"\n上位3要因:")
for i, (feat, val) in enumerate(ranking[:3], 1):
    print(f"  {i}. {feat}: {val:.4f}")

# ---------------------------------------------------------------------------
# 6. Section-specific failure analysis
# ---------------------------------------------------------------------------

print("\n=== セクション別スコア分析 ===")
for col in ["critical_count", "high_count", "medium_count"]:
    print(f"  {col}: mean={df[col].mean():.1f}, max={df[col].max()}")

print(f"\nGrade distribution:\n{df['grade'].value_counts().sort_index()}")
