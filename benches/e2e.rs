use criterion::{criterion_group, criterion_main, Criterion, black_box};
use std::fs;

fn bench_e2e_apply(c: &mut Criterion) {
    let source = fs::read_to_string("tests/fixtures/original/Personne.java").expect("source");
    let diff = fs::read_to_string("tests/fixtures/diffs/chatgpt.diff").expect("diff");
    let patch = mend::parser::parse_patch(&diff).expect("parse");
    let file_diff = &patch.diffs[0];

    c.bench_function("e2e: apply chatgpt.diff to Personne.java (no IO)", |b| {
        b.iter(|| {
            let mut source_lines: Vec<String> = source.lines().map(|s| s.to_string()).collect();
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

            for hunk in file_diff.hunks.iter().rev() {
                let matches = mend::patcher::find_hunk_location(
                    &source_lines,
                    &clean_source_map,
                    &clean_index_map,
                    hunk,
                    2,
                    false,
                    0.7,
                );
                if let Some(chosen) = matches.first() {
                    source_lines = mend::patcher::apply_hunk(
                        &source_lines,
                        hunk,
                        chosen.start_index,
                        chosen.matched_length,
                    );
                }
            }
            black_box(source_lines);
        })
    });
}

criterion_group!(benches, bench_e2e_apply);
criterion_main!(benches);

