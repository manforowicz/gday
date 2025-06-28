#![forbid(unsafe_code)]
#![warn(clippy::all)]

use criterion::BatchSize;
use criterion::{Criterion, criterion_group, criterion_main};
use gday_encryption::EncryptedStream;
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::hint::black_box;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn encryption_bench(c: &mut Criterion) {
    // generate pseudorandom data from a seed
    let mut rng = StdRng::seed_from_u64(10);

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    c.bench_function("EncryptedStream write 200,000 bytes", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let mut key = [0; 32];
                let mut nonce = [0; 7];

                rng.fill_bytes(&mut key);
                rng.fill_bytes(&mut nonce);

                let encrypted = Vec::with_capacity(300_000);
                let encryptor = EncryptedStream::new(encrypted, &key, &nonce);

                let mut random_plaintext = vec![0; 200_000];
                rng.fill_bytes(&mut random_plaintext);

                (encryptor, random_plaintext)
            },
            |(mut stream, random_plaintext)| async move {
                black_box(stream.write_all(&random_plaintext))
                    .await
                    .unwrap();
                black_box(stream.flush()).await.unwrap();
            },
            BatchSize::LargeInput,
        );
    });
}

fn decryption_bench(c: &mut Criterion) {
    // generate pseudorandom data from a seed
    let mut rng = StdRng::seed_from_u64(10);

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    let (key, nonce, ciphertext) = rt.block_on(async {
        let mut key = [0; 32];
        let mut nonce = [0; 7];
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut nonce);

        // generate random encrypted data
        let mut random_data = vec![0; 200_000];
        rng.fill_bytes(&mut random_data);
        let mut ciphertext = Vec::new();
        let mut encryptor: EncryptedStream<&mut Vec<u8>> =
            EncryptedStream::new(&mut ciphertext, &key, &nonce);
        encryptor.write_all(&random_data).await.unwrap();
        encryptor.flush().await.unwrap();

        (key, nonce, ciphertext)
    });

    c.bench_function("EncryptedStream read 200,000 bytes", |b| {
        b.to_async(&rt).iter_batched(
            || {
                (
                    vec![0; 200_000],
                    EncryptedStream::new(&ciphertext[..], &key, &nonce),
                )
            },
            |(mut decrypted, mut decryptor)| async move {
                EncryptedStream::read_exact(black_box(&mut decryptor), black_box(&mut decrypted))
                    .await
                    .unwrap()
            },
            BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, encryption_bench, decryption_bench);
criterion_main!(benches);
