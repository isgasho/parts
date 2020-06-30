use criterion::{black_box, criterion_group, criterion_main, Criterion};
use parts::{new_gpt::Gpt as NewGpt, types::BlockSize, Gpt};

static GPT: &[u8] = include_bytes!("../tests/data/test_parts_cf");

pub fn gpt(c: &mut Criterion) {
    let mut group = c.benchmark_group("GPT");
    group.bench_function("Old", |b| {
        b.iter(|| {
            //
            <Gpt>::from_bytes(black_box(GPT), BlockSize::new(512))
        })
    });
    group.bench_function("New", |b| {
        b.iter(|| {
            //
            NewGpt::read(black_box(GPT))
        })
    });
}

criterion_group!(benches, gpt);
criterion_main!(benches);
