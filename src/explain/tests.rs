use crate::lexer::Lexer;
use crate::parser::Parser;

use super::{explain, ExplainContext};

fn ex(src: &str) -> String {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    explain(&program, None)
}

fn ex_ctx(src: &str, ctx: &ExplainContext) -> String {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    explain(&program, Some(ctx))
}

// ── Basic selection ─────────────────────────────────────────────

#[test]
fn select_fields() {
    assert_eq!(ex("{ print $1, $2 }"), "use col 1–2");
}

#[test]
fn select_named_columns() {
    assert_eq!(
        ex("{ print $\"host-name\", $\"cpu-usage\" }"),
        "use host-name, cpu-usage",
    );
}

#[test]
fn select_many_fields_summarized() {
    assert_eq!(ex("{ print $1, $2, $3, $4, $5, $6 }"), "use 6 fields");
}

#[test]
fn select_printf() {
    assert_eq!(ex("{ printf \"%s %s\\n\", $1, $2 }"), "use col 1–2");
}

#[test]
fn passthrough() {
    assert_eq!(ex("{ print }"), "generate output");
    assert_eq!(ex("{ print $0 }"), "generate output");
}

// ── Filters ─────────────────────────────────────────────────────

#[test]
fn filter_pattern() {
    assert_eq!(
        ex("/Math/ { print $1, $2 }"),
        "where /Math/, use col 1–2",
    );
}

#[test]
fn filter_comparison() {
    assert_eq!(
        ex("$2 > 90 { print $1 }"),
        "where col 2 > 90, use col 1",
    );
}

// ── Accumulation ────────────────────────────────────────────────

#[test]
fn sum() {
    assert_eq!(
        ex("{ sum += $2 } END { print sum }"),
        "sum col 2",
    );
}

#[test]
fn sum_noncompound() {
    assert_eq!(
        ex("{ total = total + NF }; END {print total}"),
        "sum NF",
    );
}

#[test]
fn frequency() {
    assert_eq!(
        ex("{ a[$1]++ } END { for (k in a) print k }"),
        "freq of col 1",
    );
}

#[test]
fn aggregate_by() {
    assert_eq!(
        ex("{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }"),
        "agg col 2 by col 1",
    );
}

#[test]
fn sum_by_group() {
    assert_eq!(
        ex("{ rev[$region] += $revenue } END { for (r in rev) printf \"%s: %.2f\\n\", r, rev[r] }"),
        "sum $revenue by $region",
    );
}

// ── Chart / stats ───────────────────────────────────────────────

#[test]
fn histogram() {
    assert_eq!(
        ex("{ a[NR]=$1 } END { print plotbox(hist(a)) }"),
        "histogram of col 1",
    );
}

#[test]
fn stats() {
    assert_eq!(
        ex("{ a[NR]=$2 } END { print mean(a), median(a) }"),
        "stats of col 2",
    );
}

#[test]
fn chart_subsumes_stats() {
    assert_eq!(
        ex("{ a[NR]=$1 } END { print plotbox(hist(a)), mean(a) }"),
        "histogram of col 1",
    );
}

#[test]
fn stats_subsumes_sum_by() {
    assert_eq!(
        ex("{ rev[$1] += $2 } END { printf \"%.2f\\n\", mean(rev) }"),
        "stats of col 2",
    );
}

#[test]
fn compound_assign_tracked() {
    assert_eq!(
        ex("{ rev[$1] += $2 } END { print mean(rev) }"),
        "stats of col 2",
    );
}

// ── Count ───────────────────────────────────────────────────────

#[test]
fn count() {
    assert_eq!(ex("END { print NR }"), "count lines");
}

#[test]
fn count_pattern() {
    assert_eq!(ex("/Beth/{n++}; END {print n+0}"), "count /Beth/");
}

// ── Idioms ──────────────────────────────────────────────────────

#[test]
fn dedup() {
    assert_eq!(ex("!seen[$0]++"), "deduplicate by $0");
}

#[test]
fn dedup_multikey() {
    assert_eq!(
        ex("!seen[$1,$2]++"),
        "deduplicate by col 1, col 2",
    );
}

#[test]
fn join() {
    assert_eq!(
        ex("NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}"),
        "join on col 1",
    );
}

#[test]
fn anti_join() {
    assert_eq!(
        ex("NR==FNR{skip[$1]=1; next} !($1 in skip)"),
        "anti-join on col 1",
    );
}

#[test]
fn semi_join() {
    assert_eq!(
        ex("NR==FNR{keep[$1]=1; next} $1 in keep"),
        "semi-join on col 1",
    );
}

// ── Transform ───────────────────────────────────────────────────

#[test]
fn gsub_transform() {
    assert_eq!(
        ex("{ gsub(/foo/, \"bar\"); print }"),
        "gsub /foo/ → \"bar\"",
    );
}

#[test]
fn transform_suppresses_filter_1() {
    assert_eq!(ex("{sub(/\\r$/,\"\")};1"), "sub /\\r$/ → \"\"");
}

// ── Extract ─────────────────────────────────────────────────────

#[test]
fn regex_extract_format() {
    assert_eq!(
        ex("{ match($0, \"pattern\", c); printf \"%s\\n\", c[1] }"),
        "regex extract + format",
    );
}

#[test]
fn jpath_format() {
    assert_eq!(
        ex("{ m = jpath($0, \".method\"); printf \"%s\\n\", m }"),
        "use method",
    );
}

// ── Field operations ────────────────────────────────────────────

#[test]
fn rewrite_fields() {
    assert_eq!(ex("{ $2 = \"\"; print }"), "rewrite fields");
}

#[test]
fn reformat_output() {
    assert_eq!(ex("BEGIN{ORS=\"\\n\\n\"};1"), "reformat output");
}

#[test]
fn number_lines() {
    assert_eq!(ex("{print FNR \"\\t\" $0}"), "number lines");
    assert_eq!(
        ex("{printf \"%5d : %s\\n\", NR, $0}"),
        "number lines",
    );
}

#[test]
fn iterate_fields() {
    assert_eq!(
        ex("{for (i=NF; i>0; i--) printf \"%s \",$i; print \"\"}"),
        "iterate fields",
    );
}

#[test]
fn collect_emit() {
    assert_eq!(
        ex("{ a[NR]=$0 } END { for(i=NR;i>=1;i--) print a[i] }"),
        "collect + emit",
    );
}

// ── Lineage / variable resolution ───────────────────────────────

#[test]
fn lineage_through_vars() {
    assert_eq!(
        ex("{ x = $3 * 2; y = $4 + 1; printf \"%s %d %d\\n\", $1, x, y }"),
        "use col 1, 3, 4",
    );
}

#[test]
fn lineage_coercion() {
    assert_eq!(ex("{ x = $3 + 0; printf \"%d\\n\", x }"), "use col 3");
}

#[test]
fn lineage_named_columns() {
    assert_eq!(
        ex("{ cpu = $\"cpu-usage\" + 0; mem = $\"mem-usage\" + 0; printf \"%s %f %f\\n\", $\"host-name\", cpu, mem }"),
        "use host-name, cpu-usage, mem-usage",
    );
}

#[test]
fn concat_fields() {
    assert_eq!(ex("{ print $1 \" \" $2 }"), "use col 1–2");
    assert_eq!(ex("{ print $1 \":\" $2 \":\" $3 }"), "use col 1–3");
}

#[test]
fn concat_lineage() {
    assert_eq!(ex("{ s = $1 \" - \" $2; print s }"), "use col 1–2");
}

#[test]
fn jpath_lineage() {
    assert_eq!(
        ex("{ m = jpath($0, \".method\"); p = jpath($0, \".path\"); printf \"%s %s\\n\", m, p }"),
        "use method, path",
    );
}

#[test]
fn mixed_direct_and_computed() {
    assert_eq!(ex("{ print $1, $2, length($3) }"), "use col 1–3");
}

#[test]
fn ternary_in_output() {
    assert_eq!(
        ex("{ print ($1 > 50 ? \"high\" : \"low\"), $2 }"),
        "use col 2",
    );
}

#[test]
fn compute_shows_fields() {
    assert_eq!(ex("{ print length($0) }"), "generate output");
    assert_eq!(ex("{ print $1 + $2 }"), "use col 1–2");
}

// ── Multi-fragment ──────────────────────────────────────────────

#[test]
fn multi_fragment() {
    assert_eq!(
        ex("/baz/ { gsub(/foo/, \"bar\"); print }"),
        "gsub /foo/ → \"bar\", where /baz/",
    );
}

#[test]
fn field_iteration_sum() {
    assert_eq!(
        ex("{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}"),
        "sum $i, iterate fields",
    );
}

// ── Timing ──────────────────────────────────────────────────────

#[test]
fn timing() {
    assert_eq!(
        ex("BEGIN { tic(); for(i=0;i<100000;i++) x+=i; printf \"%.4f\\n\",toc() }"),
        "generate output, timed",
    );
}

// ── Environment context ─────────────────────────────────────────

#[test]
fn env_csv_headers() {
    let ctx = ExplainContext::from_cli("csv", true, None, &["sales.csv".into()]);
    assert_eq!(
        ex_ctx("{ sum += $2 } END { print sum }", &ctx),
        "sum col 2 (CSV, headers, sales.csv)",
    );
}

#[test]
fn env_compressed_json() {
    let ctx = ExplainContext::from_cli("json", false, None, &["api.jsonl.gz".into()]);
    assert_eq!(
        ex_ctx("{ a[NR]=$1 } END { print plotbox(hist(a)) }", &ctx),
        "histogram of col 1 (JSON, gzip, api.jsonl.gz)",
    );
}

#[test]
fn env_field_sep() {
    let ctx = ExplainContext::from_cli("line", false, Some(":"), &[]);
    assert_eq!(ex_ctx("{ print $1 }", &ctx), "use col 1 (-F ':')");
}

#[test]
fn env_multiple_files() {
    let ctx = ExplainContext::from_cli(
        "line",
        false,
        None,
        &["a.txt".into(), "b.txt".into(), "c.txt".into()],
    );
    assert_eq!(
        ex_ctx("/foo/ { print }", &ctx),
        "where /foo/ (3 files)",
    );
}

#[test]
fn env_select_no_env() {
    let ctx = ExplainContext::from_cli("line", false, None, &[]);
    assert_eq!(ex_ctx("{ print $1, $2 }", &ctx), "use col 1–2");
}

#[test]
fn env_passthrough_no_env() {
    let ctx = ExplainContext::from_cli("line", false, None, &[]);
    assert_eq!(ex_ctx("{ print }", &ctx), "generate output");
}

#[test]
fn env_idiom_with_context() {
    let ctx = ExplainContext::from_cli("csv", true, None, &["data.csv".into()]);
    assert_eq!(
        ex_ctx("!seen[$0]++", &ctx),
        "deduplicate by $0 (CSV, headers, data.csv)",
    );
}

#[test]
fn env_auto_detected_line_mode_no_noise() {
    let ctx = ExplainContext::from_cli("line", false, None, &["data.txt".into()]);
    assert_eq!(
        ex_ctx("{ sum += $1 } END { print sum }", &ctx),
        "sum col 1 (data.txt)",
    );
}

// ── Render budget ───────────────────────────────────────────────

#[test]
fn render_budget_truncation() {
    use super::{render_desc, Desc, RuleDesc, Flags, Op};

    let desc = Desc {
        begin: vec![],
        rules: vec![RuleDesc {
            filter: None,
            body: vec![
                Op::Histogram("histogram of some very long expression name".into()),
                Op::Where("col 7 ~ /^extremely-long-pattern-that-keeps-going$/".into()),
            ],
        }],
        end: vec![],
        flags: Flags::default(),
    };
    let rendered = render_desc(&desc, 72);
    assert!(
        rendered.len() <= 72,
        "rendered len {} > 72: {rendered}",
        rendered.len()
    );
    assert!(rendered.contains("histogram"));
    assert!(rendered.ends_with('…'));
}

#[test]
fn render_empty() {
    use super::{render_desc, Desc, Flags};

    let desc = Desc {
        begin: vec![],
        rules: vec![],
        end: vec![],
        flags: Flags::default(),
    };
    assert_eq!(render_desc(&desc, 72), "");
}
