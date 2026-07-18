# sdsforge

**Python优先、Rust驱动的SDS文档转换与MHLW JSON质量评估工具包。**

将安全数据表（SDS）转换为日本厚生劳动省SDS数据交换格式JSON，并提供模式验证、GHS/CAS检查和语料库规模质量评估。

[English](README.md) | [日本語](README_ja.md)

---

## 安装

```bash
pip install sdsforge                   # Python绑定
pip install "sdsforge[analysis]"       # + causasv质量分析
cargo install sdsforge                 # CLI / GUI二进制
```

---

## 快速开始 — Python

```python
import sdsforge

# 仅提取文本（不使用LLM）
text = sdsforge.extract_text("sample.pdf")

# 从URL直接转换
data, report = sdsforge.to_json_url_with_report(
    "https://example.com/sds.pdf", lang="zh-cn",
)

# SDS文档 → MHLW标准JSON
data, report = sdsforge.to_json_with_report(
    "sample.pdf",
    lang="zh-cn",
    strict_mhlw=True,
)

# 获取结构化检查结果
findings = sdsforge.validate(data, strict_mhlw=True)

print(f"已提取章节: {len(report['populated_sections'])}")
print(f"发现问题: {len(findings)} (HIGH: {sum(1 for f in findings if f['level']=='HIGH')})")

# 保存MHLW JSON
sdsforge.write_json(data, "output.json")
```

语料库规模评估（无需人工审核）：

```python
from sdsforge.eval import eval_corpus

df = eval_corpus(
    input_dir="data/sds_raw",
    output_dir="runs/eval_001",
    jobs=8,
)
print(df[["filename", "overall_score", "grade", "high_count"]].head(20))
```

---

## 示例

厚生劳动省官方样本SDS（丙烯基氯 / 塩化アリル）:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
python examples/mhlw_allyl_chloride/convert.py
```

`expected.json`、`expected_report.json`及出处说明 → [`examples/mhlw_allyl_chloride/`](examples/mhlw_allyl_chloride/)

---

## 为什么选择 sdsforge

- **MHLW原生支持**: 直接转换为日本厚生劳动省SDS数据交换格式v1.0（`SDS_Schema_v1.0.json`），并进行官方模式验证。
- **基于证据的提取**: 使用LLM将自由格式SDS文本映射到约200个深层嵌套字段，字段级原文交叉验证可检测幻觉。
- **语料库规模质量评估**: `eval_corpus`处理数百份SDS文档，输出规则失败计数、章节分数和`causasv_features.csv`，无需人工审核。
- **无供应商锁定**: 支持Anthropic Claude、OpenAI GPT、Google Gemini、Mistral、Groq、Cohere及任何OpenAI兼容本地端点。
- **Rust核心**: 提取、模式验证、GHS/CAS检查和DOCX/HTML生成均在原生代码中运行，Python绑定为轻量封装。

---

## MHLW合规性

针对2025年3月31日发布的MHLW SDS数据交换格式v1.0。

| 规则 | 行为 |
|---|---|
| 模式验证 | 根据`SDS_Schema_v1.0.json`进行验证 |
| 空字段删除 | 按§3.3规定删除`""`、`null`、`[]`、`{}` |
| AdditionalInfo | 官方模式外的内容写入`AdditionalInfo.FullText` |
| `--strict-mhlw` | 存在HIGH/CRIT时退出代码1（CLI）或抛出`ValueError`（Python） |
| CRIT/HIGH/MED发现 | 包含规则ID、严重程度、路径、消息的结构化报告 |

**验证规则包括：** GHS H/P代码有效性（GHS Rev.10）、CAS格式及校验位、第2节GHS完整性、第3节成分行对应、UN编号完整性、浓度范围检查、重复代码检测等。

质量基准（30份随机样本，seed=42）：
> CRIT=0 · 平均分89.6 · 主要问题：`S2-HAZARD-NO-PICTOGRAM`、`S15-ZHCN-NO-GB`、`S14-NO-SHIPPING-NAME`

完整规则目录 → [docs/quality-check_zh.md](docs/quality-check_zh.md)

---

## 语料库评估

无需人工审核：

```python
from sdsforge.eval import eval_corpus

df = eval_corpus("data/sds_raw", "runs/eval_001", jobs=8)
```

每个文件的输出：

| 文件 | 内容 |
|---|---|
| `generated/<stem>.json` | MHLW标准JSON |
| `reports/<stem>.json` | ConversionReport（语言、章节、警告） |
| `findings/<stem>.json` | 结构化验证结果 |
| `summary.csv` | 每文件分数和等级 |
| `failures_by_rule.csv` | 规则失败次数和受影响文件数 |

---

## CLI

```bash
# PDF/DOCX/XLSX/HTML/URL → MHLW JSON
sdsforge to-json --input input.pdf --output output.json --lang zh-cn

# 带修正流程和PubChem富集
sdsforge to-json --input input.pdf --output output.json --correct --enrich

# MHLW JSON → Word / HTML / PDF
sdsforge render --input output.json --to docx --output result.docx --lang zh-cn
sdsforge render --input output.json --to html --output result.html --lang zh-cn
sdsforge render --input output.json --to pdf  --output result.pdf  --lang zh-cn

# 严格MHLW模式验证
sdsforge validate --input output.json --strict-mhlw

# 批量处理目录
sdsforge to-json --input-dir data/ --output-dir out/ --jobs 8

# 语料库评估
sdsforge eval-corpus --input-dir data/sds_raw --output-dir runs/eval_001 --jobs 8
```

---

## GUI界面

无参数运行即可打开图形界面：

```bash
sdsforge
```

五个标签页：**转换** · **渲染文档** · **验证** · **提取文本** · **设置**

桌面应用下载：[macOS](https://github.com/kent-tokyo/sdsconv/releases/latest/download/sdsconv-macos.zip) · [Windows](https://github.com/kent-tokyo/sdsconv/releases/latest/download/sdsconv-windows-portable.zip) · `brew install --cask sdsconv`

---

## 支持的输入、语言和后端

**输入格式：** PDF（文本型、CID/Shift-JIS字体、扫描件）· DOCX · XLSX · TXT · HTML · URL

**源语言：** `ja`（JIS Z 7253）· `en`（GHS/OSHA HazCom）· `zh-cn`（GB/T 16483）· `zh-tw`（CNS 15030）

**LLM后端：** Anthropic Claude · OpenAI GPT · Google Gemini · Mistral · Groq · Cohere · 本地（任意OpenAI兼容端点）

---

## 开发者

**Rust库：**

```toml
[dependencies]
sdsforge-core = "0.4"
```

**Crate：** [`sdsforge`](https://crates.io/crates/sdsforge) · [`sdsforge-core`](https://crates.io/crates/sdsforge-core)

**Python包：** [`sdsforge`](https://pypi.org/project/sdsforge/) — `pip install sdsforge`

---

## 安全与隐私

- **云端LLM注意事项**: 使用云端LLM后端时，SDS文档文本将发送至API提供商。请勿将含有机密信息的SDS文档发送至云端API。
- **本地运行**: 使用`--backend local`与任意OpenAI兼容端点（如Ollama）可实现完全离线运行。
- **原始SDS语料库**: 将`corpus/raw/`和`data/sds_raw/`添加至`.gitignore`。仅`corpus/manifest.jsonl`（URL和sha256哈希）可安全提交。
- **REST服务器**: 具有时序攻击防护的Bearer token认证、全IPv6覆盖的SSRF防护、禁用重定向的HTTP客户端、50 MB上传上限。

---

## 与竞品比较

→ [docs/comparison.md](docs/comparison.md)

---

## 参考资料

- [厚生劳动省 — SDS信息交换标准格式公开页面](https://www.mhlw.go.jp/stf/newpage_56484.html)（日语）
- [SDS数据交换格式数据使用手册（PDF）](https://www.mhlw.go.jp/content/11305000/001467068.pdf)（日语）
- [JSON质量检查详细手册 — 53条规则按章节说明](docs/quality-check_zh.md) ([English](docs/quality-check.md) / [日本語](docs/quality-check_ja.md))
- [CHANGELOG](CHANGELOG.md)

---

## 许可证

MIT 或 Apache-2.0 — 任选其一。
