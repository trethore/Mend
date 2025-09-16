use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::fs;

fn bench_parse(c: &mut Criterion) {
    let paths = [
        "tests/fixtures/diffs/chatgpt.diff",
        "tests/fixtures/diffs/claude.diff",
        "tests/fixtures/diffs/gemini.diff",
    ];

    let inputs: Vec<String> = paths
        .iter()
        .map(|p| fs::read_to_string(p).expect("read diff"))
        .collect();

    c.bench_function("parser: fixture diffs", |b| {
        b.iter(|| {
            for s in &inputs {
                let patch = mend::parser::parse_patch(black_box(s)).expect("parse");
                black_box(patch);
            }
        })
    });

    // Synthetic larger diff string (repeat body)
    let big = inputs[0].repeat(50);
    c.bench_function("parser: large synthetic", |b| {
        b.iter(|| {
            let patch = mend::parser::parse_patch(black_box(&big)).expect("parse");
            black_box(patch);
        })
    });
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
