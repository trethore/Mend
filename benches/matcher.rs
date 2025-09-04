use criterion::{criterion_group, criterion_main, Criterion, black_box};
use std::fs;

fn load_fixture() -> (Vec<String>, mend::diff::Patch) {
    let source = fs::read_to_string("tests/fixtures/original/Personne.java").expect("source");
    let diff = fs::read_to_string("tests/fixtures/diffs/chatgpt.diff").expect("diff");
    let patch = mend::parser::parse_patch(&diff).expect("parse");
    let source_lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
    (source_lines, patch)
}

fn bench_matcher(c: &mut Criterion) {
    let (source_lines, patch) = load_fixture();
    let clean_source_map: Vec<(usize, String)> = source_lines
        .iter()
        .enumerate()
        .map(|(i, s)| (i, mend::patcher::normalize_line(s)))
        .filter(|(_, s)| !s.is_empty())
        .collect();
    let mut clean_index_map: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, norm) in &clean_source_map {
        clean_index_map.entry(norm.clone()).or_default().push(*idx);
    }

    let file_diff = &patch.diffs[0];
    let hunks = &file_diff.hunks;

    c.bench_function("matcher: strict", |b| {
        b.iter(|| {
            for h in hunks {
                let matches = mend::patcher::find_hunk_location(
                    &source_lines,
                    &clean_source_map,
                    &clean_index_map,
                    h,
                    0,
                    false,
                    0.7,
                );
                black_box(matches);
            }
        })
    });

    c.bench_function("matcher: whitespace", |b| {
        b.iter(|| {
            for h in hunks {
                let matches = mend::patcher::find_hunk_location(
                    &source_lines,
                    &clean_source_map,
                    &clean_index_map,
                    h,
                    1,
                    false,
                    0.7,
                );
                black_box(matches);
            }
        })
    });

    c.bench_function("matcher: anchor heuristic", |b| {
        b.iter(|| {
            for h in hunks {
                let matches = mend::patcher::find_hunk_location(
                    &source_lines,
                    &clean_source_map,
                    &clean_index_map,
                    h,
                    2,
                    false,
                    0.7,
                );
                black_box(matches);
            }
        })
    });
}

criterion_group!(benches, bench_matcher);
criterion_main!(benches);

