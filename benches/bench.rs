use criterion::{black_box, criterion_group, criterion_main, Criterion};
use parts::{types::BlockSize, Gpt};

static GPT: &[u8] = include_bytes!("../tests/data/test_parts_cf");

pub fn gpt(c: &mut Criterion) {
    c.bench_function("GPT", |b| {
        b.iter(|| <Gpt>::from_bytes(black_box(GPT), BlockSize::new(512)))
    });
}

criterion_group!(benches, gpt);
criterion_main!(benches);
