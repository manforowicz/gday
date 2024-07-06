use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gday_encryption::EncryptedStream;
use rand::rngs::StdRng;
use rand::RngCore;
use rand::SeedableRng;
use std::io::Read;
use std::io::Write;

pub fn encryption_bench(c: &mut Criterion) {
    // generate pseudorandom data from a seed
    let mut rng = StdRng::seed_from_u64(10);

    let mut nonce = [0; 7];
    let mut key = [0; 32];
    rng.fill_bytes(&mut nonce);
    rng.fill_bytes(&mut key);

    // generate random encrypted data
    let mut random_data = vec![0; 200_000];
    rng.fill_bytes(&mut random_data);
    let mut encrypted_data = vec![0; 300_000];

    c.bench_function("EncryptedStream write 200,000 bytes", |b| {
        b.iter(|| {
            let mut encryptor: EncryptedStream<&mut [u8]> = EncryptedStream::new(
                black_box(&mut encrypted_data[..]),
                black_box(&key),
                black_box(&nonce),
            );
            EncryptedStream::write_all(black_box(&mut encryptor), black_box(&random_data)).unwrap();
            EncryptedStream::flush(black_box(&mut encryptor)).unwrap();
        })
    });
}

pub fn decryption_bench(c: &mut Criterion) {
    // generate pseudorandom data from a seed
    let mut rng = StdRng::seed_from_u64(10);

    let mut nonce = [0; 7];
    let mut key = [0; 32];
    rng.fill_bytes(&mut nonce);
    rng.fill_bytes(&mut key);

    // generate random encrypted data
    let mut random_data = vec![0; 200_000];
    rng.fill_bytes(&mut random_data);
    let mut encrypted_data = Vec::new();
    let mut encryptor: EncryptedStream<&mut Vec<u8>> =
        EncryptedStream::new(&mut encrypted_data, &key, &nonce);
    encryptor.write_all(&random_data).unwrap();
    encryptor.flush().unwrap();

    // read this encrypted data
    let mut read_data = vec![0; 200_000];

    c.bench_function("EncryptedStream read 200,000 bytes", |b| {
        b.iter(|| {
            let mut decryptor = EncryptedStream::new(
                black_box(&encrypted_data[..]),
                black_box(&key),
                black_box(&nonce),
            );
            EncryptedStream::read_exact(black_box(&mut decryptor), black_box(&mut read_data))
                .unwrap()
        })
    });
}

criterion_group!(benches, encryption_bench, decryption_bench);
criterion_main!(benches);
