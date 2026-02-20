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
    assert_eq!(ex("{ print $1, $2 }"), "columns 1–2");
}

#[test]
fn select_named_columns() {
    assert_eq!(
        ex("{ print $\"host-name\", $\"cpu-usage\" }"),
        "host-name, cpu-usage",
    );
}

#[test]
fn select_many_fields_summarized() {
    assert_eq!(ex("{ print $1, $2, $3, $4, $5, $6 }"), "6 columns");
}

#[test]
fn select_printf() {
    assert_eq!(ex("{ printf \"%s %s\\n\", $1, $2 }"), "columns 1–2");
}

#[test]
fn passthrough() {
    assert_eq!(ex("{ print }"), "output");
    assert_eq!(ex("{ print $0 }"), "output");
}

// ── Filters ─────────────────────────────────────────────────────

#[test]
fn filter_pattern() {
    assert_eq!(
        ex("/Math/ { print $1, $2 }"),
        "columns 1–2, where /Math/",
    );
}

#[test]
fn filter_comparison() {
    assert_eq!(
        ex("$2 > 90 { print $1 }"),
        "column 1, where column 2 > 90",
    );
}

// ── Accumulation ────────────────────────────────────────────────

#[test]
fn sum() {
    assert_eq!(
        ex("{ sum += $2 } END { print sum }"),
        "sum of column 2",
    );
}

#[test]
fn sum_noncompound() {
    assert_eq!(
        ex("{ total = total + NF }; END {print total}"),
        "sum of NF",
    );
}

#[test]
fn frequency() {
    assert_eq!(
        ex("{ a[$1]++ } END { for (k in a) print k }"),
        "frequency of column 1",
    );
}

#[test]
fn aggregate_by() {
    assert_eq!(
        ex("{ s[$1]+=$2; c[$1]++ } END { for(k in s) print k, s[k]/c[k] }"),
        "aggregation of column 2 by column 1",
    );
}

#[test]
fn sum_by_group() {
    assert_eq!(
        ex("{ rev[$region] += $revenue } END { for (r in rev) printf \"%s: %.2f\\n\", r, rev[r] }"),
        "sum of $revenue by $region",
    );
}

// ── Chart / stats ───────────────────────────────────────────────

#[test]
fn histogram() {
    assert_eq!(
        ex("{ a[NR]=$1 } END { print plotbox(hist(a)) }"),
        "histogram of column 1",
    );
}

#[test]
fn stats() {
    assert_eq!(
        ex("{ a[NR]=$2 } END { print mean(a), median(a) }"),
        "statistics of column 2",
    );
}

#[test]
fn chart_subsumes_stats() {
    assert_eq!(
        ex("{ a[NR]=$1 } END { print plotbox(hist(a)), mean(a) }"),
        "histogram of column 1",
    );
}

#[test]
fn stats_subsumes_sum_by() {
    assert_eq!(
        ex("{ rev[$1] += $2 } END { printf \"%.2f\\n\", mean(rev) }"),
        "statistics of column 2",
    );
}

#[test]
fn compound_assign_tracked() {
    assert_eq!(
        ex("{ rev[$1] += $2 } END { print mean(rev) }"),
        "statistics of column 2",
    );
}

// ── Count ───────────────────────────────────────────────────────

#[test]
fn count() {
    assert_eq!(ex("END { print NR }"), "line count");
}

#[test]
fn count_pattern() {
    assert_eq!(ex("/Beth/{n++}; END {print n+0}"), "count /Beth/");
}

#[test]
fn count_not_triggered_with_array_accumulation() {
    // Program with both count++ and array accum (by_cust[cust]+=, n_cust[cust]++) must
    // not be reduced to "line count" — try_count_match should require simple counter only.
    let out = ex(r#"
        { count++; by_cust[$1]+=$2; n_cust[$1]++ }
        END { printf "total %d\n", count; for (k in by_cust) print k, by_cust[k], n_cust[k] }
    "#);
    assert!(
        !out.eq("line count"),
        "must not collapse to line count when array accumulation present; got {:?}",
        out,
    );
    assert!(
        out.contains("aggregation") || out.contains("frequency") || out.contains("sum") || out.contains("statistics"),
        "expected aggregation/frequency/sum/statistics in {:?}",
        out,
    );
}

// ── Idioms ──────────────────────────────────────────────────────

#[test]
fn dedup() {
    assert_eq!(ex("!seen[$0]++"), "deduplication by line");
}

#[test]
fn dedup_multikey() {
    assert_eq!(
        ex("!seen[$1,$2]++"),
        "deduplication by column 1, column 2",
    );
}

#[test]
fn join() {
    assert_eq!(
        ex("NR==FNR{price[$1]=$2; next} {print $0, price[$1]+0}"),
        "join on column 1",
    );
}

#[test]
fn anti_join() {
    assert_eq!(
        ex("NR==FNR{skip[$1]=1; next} !($1 in skip)"),
        "rows without a match on column 1",
    );
}

#[test]
fn semi_join() {
    assert_eq!(
        ex("NR==FNR{keep[$1]=1; next} $1 in keep"),
        "matching rows on column 1",
    );
}

// ── Transform ───────────────────────────────────────────────────

#[test]
fn gsub_transform() {
    assert_eq!(
        ex("{ gsub(/foo/, \"bar\"); print }"),
        "replace /foo/ → \"bar\"",
    );
}

#[test]
fn redacted_output_not_bare_var_name() {
    // Print "  original:", $0 and "  redacted:", safe → labels from literals yield "original, redacted"
    let out = ex(r#"
        {
            safe = gensub("token=\\S+", "token=***", "g")
            safe = gensub("[\\w.]+@[\\w.]+", "***@***", "g", safe)
            print "  original:", $0
            print "  redacted:", safe
        }
        "#);
    assert!(out.contains("replace"), "expected replace in {:?}", out);
    assert!(out.contains("original"), "expected 'original' (from literal) in {:?}", out);
    assert!(out.contains("redacted"), "expected 'redacted' (from literal) in {:?}", out);
}

#[test]
fn transform_suppresses_filter_1() {
    assert_eq!(ex("{sub(/\\r$/,\"\")};1"), "replace /\\r$/ → \"\"");
}

#[test]
fn gensub_in_print() {
    // print gensub(...) → "replace ..." only, not "replace ..., gensub"
    let out = ex(r#"{ print gensub("-", " | ", 2) }"#);
    assert!(out.contains("replace"), "expected replace in {:?}", out);
    assert!(!out.eq("transform"), "must not be generic 'transform'");
    assert!(
        !out.ends_with(", gensub") && !out.ends_with(", replace"),
        "must not repeat transform verb as output; got {:?}",
        out,
    );
}

#[test]
fn idiom_ascii_table() {
    // Range from for-loop head only; output from normal collect → range 33..126: i
    let out = ex(r#"
        BEGIN {
            for (i = 33; i <= 126; i++) {
                printf "  %3d  %4s  %s", i, hex(i), chr(i)
                if ((i - 32) % 6 == 0) print ""
            }
            print ""
        }
    "#);
    assert_eq!(out, "range 33..126: i, hex value, character");
}

#[test]
fn idiom_per_char_ord_hex() {
    // C-style for in rule → range; output ref is c (don't expand substr)
    let out = ex(r#"
        { for (i=1; i<=length($0); i++) { c=substr($0,i,1); printf "  %s → %d → %s\n", c, ord(c), hex(ord(c)) } }
    "#);
    assert_eq!(out, "range: c, code point, hex value");
}

#[test]
fn idiom_generic_var_names() {
    let ascii = ex(r#"
        BEGIN { for (k=33; k<=126; k++) printf "%s %s\n", chr(k), hex(k) }
    "#);
    assert_eq!(ascii, "range 33..126: character, hex value");
    let perchar = ex(r#"
        { for (pos=1; pos<=length($0); pos++) { ch=substr($0,pos,1); printf "%s %d %s\n", ch, ord(ch), hex(ord(ch)) } }
    "#);
    assert_eq!(perchar, "range: ch, code point, hex value");
}

// ── Extract ─────────────────────────────────────────────────────

#[test]
fn regex_extract_format() {
    assert_eq!(
        ex("{ match($0, \"pattern\", c); printf \"%s\\n\", c[1] }"),
        "pattern extract + format",
    );
}

#[test]
fn capture_filter_most_significant() {
    // Significance ordering: filter on match() capture is highest; label is generic (no 5xx shortcut).
    let out = ex(r#"
        { match($0, /^(\S+)\s+(\d+)/, c); if (c[2]+0 >= 500) printf "%s %s\n", c[1], c[2] }
    "#);
    assert!(
        out.starts_with("where c[2] ≥ 500"),
        "filter must be first and use generic label; got {:?}",
        out,
    );
    assert!(out.contains("pattern extract"), "expected extract in {:?}", out);
}

#[test]
fn jpath_format() {
    assert_eq!(
        ex("{ m = jpath($0, \".method\"); printf \"%s\\n\", m }"),
        "method",
    );
}

#[test]
fn json_extract_shows_path() {
    // Rule extracts .ms into array; Extract phrase should say what is extracted
    let out = ex(r#"{ ms = jpath($0, ".ms"); lat[NR] = ms } END { print mean(lat) }"#);
    assert!(
        out.contains("JSON extract (.ms)"),
        "extract phrase should include path; got {:?}",
        out,
    );
}

#[test]
fn jpath_loop_for_all_members() {
    // Loop bound n from jpath($0, ".members", m) → "for all members: team, name, role"
    let out = ex(r#"
        BEGIN {
            team = jpath($0, ".team")
            n = jpath($0, ".members", m)
            for (i=1; i<=n; i++) printf "  %s: %s (%s)\n", team, jpath(m[i], ".name"), jpath(m[i], ".role")
        }
    "#);
    assert_eq!(out, "for all members: team, name, role");
}

/// Proves "for all X" is generic: over_key is the jpath path (leading dot trimmed) from the program.
/// No special case for "members"; any path yields "for all <path>: ..." (e.g. "data.rows" for ".data.rows").
#[test]
fn jpath_loop_for_all_generic() {
    // .items → path "items"
    let out = ex(r#"
        BEGIN { k = jpath($0, ".items", arr); for (i=1; i<=k; i++) print jpath(arr[i], ".id") }
    "#);
    assert_eq!(out, "for all items: id");

    // .data.rows → full path "data.rows"
    let out = ex(r#"
        BEGIN { n = jpath($0, ".data.rows", r); for (j=1; j<=n; j++) print jpath(r[j], ".label") }
    "#);
    assert_eq!(out, "for all data.rows: label");

    // .users → path "users"
    let out = ex(r#"
        BEGIN { c = jpath($0, ".users", u); for (i=1; i<=c; i++) printf "%s\n", jpath(u[i], ".login") }
    "#);
    assert_eq!(out, "for all users: login");
}

// ── Field operations ────────────────────────────────────────────

#[test]
fn rewrite_fields() {
    assert_eq!(ex("{ $2 = \"\"; print }"), "rewritten fields");
}

#[test]
fn reformat_output() {
    assert_eq!(ex("BEGIN{ORS=\"\\n\\n\"};1"), "reformat output");
}

#[test]
fn number_lines() {
    assert_eq!(ex("{print FNR \"\\t\" $0}"), "numbered lines");
    assert_eq!(
        ex("{printf \"%5d : %s\\n\", NR, $0}"),
        "numbered lines",
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
        "collected lines",
    );
}

// ── Lineage / variable resolution ───────────────────────────────

#[test]
fn lineage_through_vars() {
    assert_eq!(
        ex("{ x = $3 * 2; y = $4 + 1; printf \"%s %d %d\\n\", $1, x, y }"),
        "columns 1, 3, 4",
    );
}

#[test]
fn lineage_coercion() {
    assert_eq!(ex("{ x = $3 + 0; printf \"%d\\n\", x }"), "column 3");
}

#[test]
fn lineage_named_columns() {
    assert_eq!(
        ex("{ cpu = $\"cpu-usage\" + 0; mem = $\"mem-usage\" + 0; printf \"%s %f %f\\n\", $\"host-name\", cpu, mem }"),
        "host-name, cpu-usage, mem-usage",
    );
}

#[test]
fn lineage_named_columns_with_computed_var() {
    // Server status: host, cpu%, mem%, and computed status (OK/WARNING/CRITICAL)
    assert_eq!(
        ex(r#"
        {
            cpu = $"cpu-usage" + 0; mem = $"mem-usage" + 0
            status = "OK"
            if (cpu > 90 || mem > 90) status = "CRITICAL"
            else if (cpu > 70 || mem > 70) status = "WARNING"
            printf "%-12s cpu=%5.1f%% mem=%5.1f%% [%s]\n", $"host-name", cpu, mem, status
        }
        "#),
        "host-name, cpu-usage, mem-usage, status",
    );
}

#[test]
fn concat_fields() {
    assert_eq!(ex("{ print $1 \" \" $2 }"), "columns 1–2");
    assert_eq!(ex("{ print $1 \":\" $2 \":\" $3 }"), "columns 1–3");
}

#[test]
fn concat_lineage() {
    assert_eq!(ex("{ s = $1 \" - \" $2; print s }"), "columns 1–2");
}

#[test]
fn jpath_lineage() {
    assert_eq!(
        ex("{ m = jpath($0, \".method\"); p = jpath($0, \".path\"); printf \"%s %s\\n\", m, p }"),
        "method, path",
    );
}

#[test]
fn jpath_from_slurped_json() {
    // BEGIN-only: slurp file, then jpath on the slurped string — mention slurp and JSON paths
    let out = ex(r#"
        BEGIN {
            json = slurp("/tmp/pretty.json")
            printf "service=%s version=%s timeout=%sms retries=%s\n",
                jpath(json, ".service"),
                jpath(json, ".version"),
                jpath(json, ".limits.timeout_ms"),
                jpath(json, ".limits.retries")
        }
        "#);
    assert!(out.contains("from JSON:"), "expected 'from JSON:' in {:?}", out);
    assert!(out.contains("slurped"), "expected 'slurped' in {:?}", out);
}

#[test]
fn mixed_direct_and_computed() {
    assert_eq!(ex("{ print $1, $2, length($3) }"), "1, 2, length");
}

#[test]
fn ternary_in_output() {
    assert_eq!(
        ex("{ print ($1 > 50 ? \"high\" : \"low\"), $2 }"),
        "column 2",
    );
}

#[test]
fn compute_shows_fields() {
    assert_eq!(ex("{ print length($0) }"), "length");
    assert_eq!(ex("{ print $1 + $2 }"), "columns 1–2");
}

// ── Multi-fragment ──────────────────────────────────────────────

#[test]
fn multi_fragment() {
    assert_eq!(
        ex("/baz/ { gsub(/foo/, \"bar\"); print }"),
        "replace /foo/ → \"bar\", where /baz/",
    );
}

#[test]
fn field_iteration_sum() {
    assert_eq!(
        ex("{s=0; for (i=1; i<=NF; i++) s=s+$i; print s}"),
        "sum of $i, iterate fields",
    );
}

// ── Timing ──────────────────────────────────────────────────────

#[test]
fn timing() {
    assert_eq!(
        ex("BEGIN { tic(); for(i=0;i<100000;i++) x+=i; printf \"%.4f\\n\",toc() }"),
        "range 0..100000: toc",
    );
}

// ── Environment context ─────────────────────────────────────────

#[test]
fn env_csv_headers() {
    let ctx = ExplainContext::from_cli("csv", true, None, &["sales.csv".into()]);
    assert_eq!(
        ex_ctx("{ sum += $2 } END { print sum }", &ctx),
        "sum of column 2 (CSV, headers, sales.csv)",
    );
}

#[test]
fn env_compressed_json() {
    let ctx = ExplainContext::from_cli("json", false, None, &["api.jsonl.gz".into()]);
    assert_eq!(
        ex_ctx("{ a[NR]=$1 } END { print plotbox(hist(a)) }", &ctx),
        "histogram of column 1 (JSON, gzip, api.jsonl.gz)",
    );
}

#[test]
fn env_field_sep() {
    let ctx = ExplainContext::from_cli("line", false, Some(":"), &[]);
    assert_eq!(ex_ctx("{ print $1 }", &ctx), "column 1 (-F ':')");
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
    assert_eq!(ex_ctx("{ print $1, $2 }", &ctx), "columns 1–2");
}

#[test]
fn env_passthrough_no_env() {
    let ctx = ExplainContext::from_cli("line", false, None, &[]);
    assert_eq!(ex_ctx("{ print }", &ctx), "output");
}

#[test]
fn env_idiom_with_context() {
    let ctx = ExplainContext::from_cli("csv", true, None, &["data.csv".into()]);
    assert_eq!(
        ex_ctx("!seen[$0]++", &ctx),
        "deduplication by line (CSV, headers, data.csv)",
    );
}

#[test]
fn env_auto_detected_line_mode_no_noise() {
    let ctx = ExplainContext::from_cli("line", false, None, &["data.txt".into()]);
    assert_eq!(
        ex_ctx("{ sum += $1 } END { print sum }", &ctx),
        "sum of column 1 (data.txt)",
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
