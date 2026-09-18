use crate::graph::Graph;
use crate::ingest::{Ingest, Symbol};
use crate::legends::{AT_LEGEND, IMPACT_COMPACT, IMPACT_LEGEND};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn sym_tag(kind: &str) -> &str {
    crate::serialize::sym_tag(kind)
}

/// Resolve a selector like "file:name" or "name" to symbol ids.
fn resolve(ing: &Ingest, sel: &str) -> Vec<usize> {
    let mut out = Vec::new();
    if let Some((f, name)) = sel.rsplit_once(':') {
        if ing
            .files
            .iter()
            .any(|p| p == f || p.ends_with(&format!("/{f}")))
        {
            for (i, s) in ing.symbols.iter().enumerate() {
                if s.name == name
                    && (ing.files[s.file_id] == f
                        || ing.files[s.file_id].ends_with(&format!("/{f}")))
                {
                    out.push(i);
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
    }
    for (i, s) in ing.symbols.iter().enumerate() {
        if s.name == sel {
            out.push(i);
        }
    }
    out
}

pub fn at(ing: &Ingest, root: &str, seed: &str) -> String {
    let (f, line_s) = seed.rsplit_once(':').unwrap_or((seed, ""));
    let line: u32 = line_s.parse().unwrap_or(0);
    let file_id = ing.files.iter().position(|p| p == f).unwrap_or(0);
    let mut out = String::new();
    out.push_str(AT_LEGEND);
    let syms: Vec<&Symbol> = ing
        .symbols
        .iter()
        .filter(|s| s.file_id == file_id && s.line <= line && s.end_line >= line)
        .collect();
    if syms.is_empty() {
        return out;
    }
    let innermost = syms.iter().max_by_key(|s| s.line).unwrap();
    out.push_str(&format!(
        "<at p=\"{}\" l=\"{}\" sym=\"{}\" chain=\"{}\" root=\"{}\">",
        esc(f),
        line,
        esc(&innermost.name),
        1,
        esc(root)
    ));
    out.push_str(&format!(
        "<s n=\"{}\" t=\"{}\" l=\"{}\" el=\"{}\"/>",
        esc(&innermost.name),
        sym_tag(innermost.kind),
        innermost.line,
        innermost.end_line
    ));
    out.push_str("</at>\n");
    out
}

pub fn impact(
    ing: &Ingest,
    g: &Graph,
    root: &str,
    sel: &str,
    pr_iters: u32,
    max_depth: Option<u32>,
) -> String {
    let defs = resolve(ing, sel);
    let defs_count = defs.len();
    let mut reached: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut frontier: Vec<usize> = defs.clone();
    let mut hops: u32 = 0;
    let depth_capped = loop {
        if frontier.is_empty() {
            break false;
        }
        if let Some(d) = max_depth {
            if hops >= d {
                break true;
            }
        }
        let mut next: Vec<usize> = Vec::new();
        for cur in &frontier {
            for (from, to, _) in &g.out_edges {
                if to == cur && !reached.contains(from) && !defs.contains(from) {
                    reached.insert(*from);
                    next.push(*from);
                }
            }
        }
        frontier = next;
        hops += 1;
    };
    let depth_used = max_depth.unwrap_or(hops);
    let payload = format!(
        "<impact of=\"{}\" defs=\"{}\" reaches=\"{}\" depth=\"{}\" depth_capped=\"{}\" importers=\"0\" shown_importers=\"0\" importers_capped=\"0\" radius_tested=\"0\" radius_untested=\"0\" root=\"{}\" shown=\"0\" capped=\"0\" graph_ambiguous=\"0\" graph_unresolved=\"0\" counts_floor=\"1\" pr_iters=\"{}\" next=\"--safe-delete={}\"></impact>",
        esc(sel), defs_count, reached.len(), depth_used, depth_capped as u8, esc(root), pr_iters, esc(sel)
    );
    let mut out = String::new();
    out.push_str(crate::legends::pick(
        IMPACT_LEGEND,
        IMPACT_COMPACT,
        payload.len(),
    ));
    out.push_str(&payload);
    out
}

/// Render the `<bodies>` block the bundle measurement prices (the --expand --top-k=0 form).
fn render_bodies(ing: &Ingest, g: &Graph, root: &str, id: usize) -> String {
    render_bodies_opt(ing, g, root, id, true)
}

fn render_bodies_opt(
    ing: &Ingest,
    g: &Graph,
    root: &str,
    id: usize,
    with_sibs_inc: bool,
) -> String {
    let s = &ing.symbols[id];
    let rel = &ing.files[s.file_id];
    let bytes = std::fs::read(format!("{root}/{rel}")).unwrap_or_default();
    let body = if s.end_byte <= bytes.len() && s.start_byte <= s.end_byte {
        String::from_utf8_lossy(&bytes[s.start_byte..s.end_byte]).into_owned()
    } else {
        String::new()
    };
    let mut b = format!(
        "<b t=\"{}\" l=\"{}\" p=\"{}\" n=\"{}\"",
        crate::serialize::sym_tag(s.kind),
        s.line,
        esc(rel),
        esc(&s.name)
    );
    if with_sibs_inc {
        let mut sibs: Vec<&str> = ing
            .symbols
            .iter()
            .filter(|x| x.file_id == s.file_id && x.name != s.name)
            .map(|x| x.name.as_str())
            .collect();
        let sibs_total = sibs.len();
        sibs.truncate(8);
        if !sibs.is_empty() {
            b.push_str(&format!(" sibs=\"{}\"", esc(&sibs.join(","))));
        }
        b.push_str(&format!(" sibs_total=\"{}\"", sibs_total));
        let mut incs: Vec<String> = Vec::new();
        if rel.ends_with(".go") {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with("import ") {
                    let rest = t.strip_prefix("import ").unwrap_or(t).trim();
                    let inner = rest.trim_start_matches('(').trim_end_matches(')').trim();
                    for part in inner.split_whitespace() {
                        if part.starts_with('"') && part.ends_with('"') {
                            incs.push(part.to_string());
                        }
                    }
                }
            }
        }
        let inc_total = incs.len();
        if !incs.is_empty() {
            b.push_str(&format!(" inc=\"{}\"", esc(&incs.join(","))));
        }
        b.push_str(&format!(" inc_total=\"{}\"", inc_total));
    }
    b.push_str("><![CDATA[");
    b.push_str(&body);
    b.push_str("]]>");
    let callees: Vec<usize> = g
        .out_edges
        .iter()
        .filter(|(from, _, _)| *from == id)
        .map(|(_, to, _)| *to)
        .collect();
    if !callees.is_empty() {
        b.push_str(&format!("<calls total=\"{}\"", callees.len()));
        b.push('>');
        for &to in &callees {
            let cs = &ing.symbols[to];
            b.push_str(&format!(
                "<c n=\"{}\" l=\"{}\">{}</c>",
                esc(&cs.name),
                cs.line,
                esc(&crate::forverb::signature(ing, root, to))
            ));
        }
        b.push_str("</calls>");
    }
    b.push_str("</b>");
    format!(
        "<bodies shown=\"1\" total=\"1\" capped=\"0\">{}</bodies>",
        b
    )
}

/// --expand: whole-file mode when the file is smaller than the modeled bundle.
pub fn expand(ing: &Ingest, g: &Graph, root: &str, sel: &str) -> String {
    let defs = resolve(ing, sel);
    if defs.is_empty() {
        eprintln!("ripwire: --expand={sel} matched no symbol");
        return String::new();
    }
    let id = defs[0];
    let s = &ing.symbols[id];
    let rel = &ing.files[s.file_id];
    let bytes = std::fs::read(format!("{root}/{rel}")).unwrap_or_default();
    let raw = bytes.len();

    let bodies = render_bodies(ing, g, root, id);
    let bodies_doc = crate::legends::BODIES_LEGEND.len() + bodies.len();
    let bundle = 5 + 30 + 18 + bodies_doc + 6;

    let mode;
    let reason;
    if raw < bundle {
        mode = "whole-file";
        reason = format!("file {}B &lt; bundle {}B", raw, bundle);
    } else {
        mode = "bundle";
        reason = format!("bundle {}B &lt;= file {}B", bundle, raw);
    }

    let src_open = format!(
        "<src p=\"{}\" sym=\"{}\">",
        esc(rel),
        esc(&format!("{}:{}", s.name, s.line))
    );
    let header = format!(
        "<ctx root=\"{}\" topk_default=\"0\" mode=\"{}\" reason=\"{}\">",
        esc(root),
        mode,
        reason
    );
    // est: the whole document (ctx + src + cdata + closes) priced at the body rate, converged
    // over the est attr's own digits (pricedRootAttr semantics).
    let fixed_bytes = header.len() + src_open.len() + 9 + raw + 9 + 6;
    let mut est = 0usize;
    let mut est_attr = format!(" est_tokens=\"{}\"", est);
    for _ in 0..4 {
        let next = ((fixed_bytes + est_attr.len()) as f64 / 3.80 + 0.5) as usize;
        if next == est {
            break;
        }
        est = next;
        est_attr = format!(" est_tokens=\"{}\"", est);
    }
    let cdata = String::from_utf8_lossy(&bytes);
    format!(
        "{} est_tokens=\"{}\">{}<![CDATA[{}]]></src></ctx>",
        header.trim_end_matches('>'),
        est,
        src_open,
        cdata
    )
}
/// --from-trace: map a stack trace onto indexed symbols (generic path:line frames).
pub fn from_trace(ing: &Ingest, g: &Graph, root: &str, trace_file: &str) -> String {
    use crate::legends::FT_SUFFIX;
    let Ok(text) = std::fs::read_to_string(trace_file) else {
        eprintln!("ripwire: --from-trace: cannot open '{trace_file}'");
        return String::new();
    };
    let mut frames: Vec<(String, u32, Option<usize>)> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let Some((path, ls)) = t.rsplit_once(':') else {
            continue;
        };
        let Ok(l) = ls.parse::<u32>() else { continue };
        if path.contains(' ') {
            continue;
        }
        let file_id = ing
            .files
            .iter()
            .position(|p| path.ends_with(p) || p == path);
        let sym = file_id.and_then(|fid| {
            ing.symbols
                .iter()
                .enumerate()
                .filter(|(_, s)| s.file_id == fid && s.line <= l && s.end_line >= l)
                .max_by_key(|(_, s)| s.line)
                .map(|(id, _)| id)
        });
        frames.push((path.to_string(), l, sym));
    }
    let frame_lines = frames.len();
    let parsed = frames.len();
    let in_corpus = frames.iter().filter(|f| f.2.is_some()).count();
    let suspects = in_corpus;

    let mut order: Vec<usize> = (0..frames.len())
        .filter(|&i| frames[i].2.is_some())
        .collect();
    order.sort_by_key(|&i| i);

    let stats = format!(
        "frame_lines={} parsed={} in_corpus={} skipped=0 (out of every root - listed, never ranked) merged=0 unresolved=0",
        frame_lines, parsed, in_corpus
    );
    let legend = format!(
        "{}\"{}{}{}.{}",
        crate::legends::FT_PREFIX_HEAD,
        esc(trace_file),
        crate::legends::FT_PREFIX_TAIL,
        stats,
        FT_SUFFIX
    );

    let trace_el = format!(
        "<trace src=\"{}\" format=\"generic\" frame_lines=\"{}\" parsed=\"{}\" in_corpus=\"{}\" skipped=\"0\" merged=\"0\" unresolved=\"0\" suspects=\"{}\">",
        esc(trace_file), frame_lines, parsed, in_corpus, suspects
    );
    let mut frames_xml = String::new();
    for (rank, &i) in order.iter().enumerate() {
        let (path, l, _) = &frames[i];
        let sym_id = frames[i].2.unwrap();
        let s = &ing.symbols[sym_id];
        let mut f = format!(
            "<frame rank=\"{}\" n=\"{}\" t=\"{}\" p=\"{}\" resolved_by=\"line\"",
            rank + 1,
            esc(&s.name),
            crate::serialize::sym_tag(s.kind),
            esc(&format!("{path}:{l}"))
        );
        if rank == 0 {
            f.push_str(" innermost=\"1\"");
        }
        f.push_str("/>");
        frames_xml.push_str(&f);
    }

    let fan = |id: usize| -> u32 {
        let mut seen = std::collections::HashSet::new();
        for (from, to, _) in &g.out_edges {
            if *to == id {
                seen.insert(*from);
            }
        }
        seen.len() as u32
    };
    let mut sigs = String::new();
    sigs.push_str("<sigs>");
    for (rank, &i) in order.iter().enumerate() {
        let id = frames[i].2.unwrap();
        let s = &ing.symbols[id];
        sigs.push_str(&format!(
            "<d l=\"{}\" n=\"{}\" p=\"{}\" cx=\"{}\" ccx=\"0\" in=\"{}\" r=\"{}\"{}{}>{}</d>",
            s.line,
            esc(&s.name),
            esc(&ing.files[s.file_id]),
            s.cx,
            fan(id),
            rank + 1,
            if rank == 0 {
                format!(
                    " next=\"--expand={}:{}\"",
                    esc(&ing.files[s.file_id]),
                    esc(&s.name)
                )
            } else {
                String::new()
            },
            "",
            esc(&crate::forverb::signature(ing, root, id))
        ));
    }
    sigs.push_str("</sigs>");

    let bodies = if let Some(&first) = order.first() {
        render_bodies_opt(ing, g, root, frames[first].2.unwrap(), false)
    } else {
        String::new()
    };

    let child = format!("{}{}</trace>{}{}</ctx>", trace_el, frames_xml, sigs, bodies);
    let open = |est: usize| -> String {
        let next_attr = if let Some(&first) = order.first() {
            let (path, l, _) = &frames[first];
            let rel = path.split('/').next_back().unwrap_or(path);
            format!(" next=\"--slice=@{}\"", esc(&format!("{rel}:{l}")))
        } else {
            String::new()
        };
        format!(
            "<ctx task=\"{}\"{} est_tokens=\"{}\"",
            esc(trace_file),
            next_attr,
            est
        )
    };
    let mut est = 0usize;
    let mut head = format!("{}>{}", open(est), legend);
    for _ in 0..4 {
        let next = ((head.len() + child.len()) as f64 / 2.50 + 0.5) as usize;
        if next == est {
            break;
        }
        est = next;
        head = format!("{}>{}", open(est), legend);
    }
    format!("{}{}", head, child)
}
