use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ruby_references::{cache::Cache, cached_file::CachedFile};

pub fn criterion_benchmark(c: &mut Criterion) {
    let cache_dir = std::path::Path::new("tmp/cache");
    let path = std::path::Path::new("src/parser/mod.rs");
    //let path = std::path::Path::new("tests/fixtures/small-app/bin/bundle");
    let cached_file = CachedFile {
        cache_dir: cache_dir.to_path_buf(),
    };
    c.bench_function("getmod", |b| b.iter(|| cached_file.get(black_box(&path))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
