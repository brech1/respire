use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use respire::math::int_mod::IntMod;
use respire::math::int_mod_cyclo::IntModCyclo;
use respire::math::matrix::Matrix;
use respire::math::rand_sampled::RandUniformSampled;
use respire::pir::pir::{Stats, PIR};
use respire::pir::respire::{RecordBytesImpl, Respire, RespireParams, RespireParamsExpanded};
use respire::respire;
use std::time::Duration;

fn criterion_benchmark(c: &mut Criterion) {
    const SPIRAL_TEST_PARAMS: RespireParamsExpanded = RespireParams {
        Q1A: 268369921,
        Q1B: 249561089,
        D1: 2048,
        T_GSW: 8,
        T_PROJ_SHORT: 8,
        T_PROJ_LONG: 8,
        T_RLWE_TO_GSW: 4,
        T_VECTORIZE: 2,
        BATCH_SIZE: 1,
        N_VEC: 2,
        ERROR_WIDTH_MILLIONTHS: 6_400_000,
        ERROR_WIDTH_VEC_MILLIONTHS: 6_400_000,
        ERROR_WIDTH_COMPRESS_MILLIONTHS: 6_400_000,
        SECRET_BOUND: 7,
        SECRET_WIDTH_VEC_MILLIONTHS: 6_400_000,
        SECRET_WIDTH_COMPRESS_MILLIONTHS: 6_400_000,
        P: 1 << 8,
        D3: 1024,
        NU1: 9,
        NU2: 6,
        Q3: 1 << 10,
        Q2: 2056193,
        D2: 1024,
    }
    .expand();
    type SPIRALTest = respire!(SPIRAL_TEST_PARAMS);

    c.bench_function("pir::automorphism with T_COEFF_REGEV", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let scalar_key = SPIRALTest::encode_setup();
        let auto_key_regev = SPIRALTest::auto_setup::<
            { SPIRAL_TEST_PARAMS.T_PROJ_SHORT },
            { SPIRAL_TEST_PARAMS.Z_PROJ_SHORT },
        >(3, &scalar_key);
        let ct = Matrix::rand_uniform(&mut rng);
        b.iter(|| {
            SPIRALTest::auto_hom::<
                { SPIRAL_TEST_PARAMS.T_PROJ_SHORT },
                { SPIRAL_TEST_PARAMS.Z_PROJ_SHORT },
            >(black_box(&auto_key_regev), black_box(&ct))
        })
    });

    c.bench_function("pir::automorphism with T_COEFF_GSW", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let scalar_key = SPIRALTest::encode_setup();
        let auto_key_regev = SPIRALTest::auto_setup::<
            { SPIRAL_TEST_PARAMS.T_PROJ_LONG },
            { SPIRAL_TEST_PARAMS.Z_PROJ_LONG },
        >(3, &scalar_key);
        let ct = Matrix::rand_uniform(&mut rng);
        b.iter(|| {
            SPIRALTest::auto_hom::<
                { SPIRAL_TEST_PARAMS.T_PROJ_LONG },
                { SPIRAL_TEST_PARAMS.Z_PROJ_LONG },
            >(black_box(&auto_key_regev), black_box(&ct))
        })
    });

    let mut group = c.benchmark_group("pir::do_expand_iter with T_COEFF_REGEV");
    for i in 0..4 {
        group.bench_with_input(BenchmarkId::from_parameter(i), &i, |b, &i| {
            let mut rng = ChaCha20Rng::from_entropy();
            let scalar_key = SPIRALTest::encode_setup();
            let auto_key_regev = SPIRALTest::auto_setup::<
                { SPIRAL_TEST_PARAMS.T_PROJ_SHORT },
                { SPIRAL_TEST_PARAMS.Z_PROJ_SHORT },
            >(SPIRAL_TEST_PARAMS.D1 / (1 << i) + 1, &scalar_key);
            let mut cts = Vec::with_capacity(1 << i);
            for _ in 0..(1 << i) {
                cts.push(Matrix::rand_uniform(&mut rng));
            }
            b.iter(|| {
                SPIRALTest::do_proj_iter::<
                    { SPIRAL_TEST_PARAMS.T_PROJ_SHORT },
                    { SPIRAL_TEST_PARAMS.Z_PROJ_SHORT },
                >(
                    black_box(i),
                    black_box(cts.as_slice()),
                    black_box(&auto_key_regev),
                )
            });
        });
    }
    group.finish();

    let mut group = c.benchmark_group("pir::do_expand_iter with T_COEFF_GSW");
    for i in 0..4 {
        group.bench_with_input(BenchmarkId::from_parameter(i), &i, |b, &i| {
            let mut rng = ChaCha20Rng::from_entropy();
            let scalar_key = SPIRALTest::encode_setup();
            let auto_key_regev = SPIRALTest::auto_setup::<
                { SPIRAL_TEST_PARAMS.T_PROJ_LONG },
                { SPIRAL_TEST_PARAMS.Z_PROJ_LONG },
            >(SPIRAL_TEST_PARAMS.D1 / (1 << i) + 1, &scalar_key);
            let mut cts = Vec::with_capacity(1 << i);
            for _ in 0..(1 << i) {
                cts.push(Matrix::rand_uniform(&mut rng));
            }
            b.iter(|| {
                SPIRALTest::do_proj_iter::<
                    { SPIRAL_TEST_PARAMS.T_PROJ_LONG },
                    { SPIRAL_TEST_PARAMS.Z_PROJ_LONG },
                >(
                    black_box(i),
                    black_box(cts.as_slice()),
                    black_box(&auto_key_regev),
                )
            });
        });
    }
    group.finish();

    c.bench_function("pir::scalar_regev_mul_x_pow", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let ct = Matrix::rand_uniform(&mut rng);
        b.iter(|| SPIRALTest::rlwe_mul_x_pow(black_box(&ct), black_box(101)))
    });

    c.bench_function("pir::regev to gsw", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let s = SPIRALTest::encode_setup();
        let regev_to_gsw_key = SPIRALTest::rlwe_to_gsw_setup(&s);

        let msg: IntModCyclo<{ SPIRAL_TEST_PARAMS.D1 }, { SPIRAL_TEST_PARAMS.Q1 }> =
            IntModCyclo::rand_uniform(&mut rng);
        let mut msg_curr = msg.include_into();
        let mut encrypt_vec = Vec::with_capacity(SPIRAL_TEST_PARAMS.T_GSW);
        for _ in 0..SPIRAL_TEST_PARAMS.T_GSW {
            encrypt_vec.push(SPIRALTest::encode_rlwe(&s, &msg_curr));
            msg_curr *= IntMod::from(SPIRAL_TEST_PARAMS.Z_GSW);
        }

        b.iter(|| {
            SPIRALTest::rlwe_to_gsw(
                black_box(&regev_to_gsw_key),
                black_box(encrypt_vec.as_slice()),
            )
        });
    });

    c.bench_function("pir::regev_sub_hom", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let m1 = Matrix::rand_uniform(&mut rng);
        let m2 = Matrix::rand_uniform(&mut rng);
        b.iter(|| SPIRALTest::rlwe_sub_hom(black_box(&m1), black_box(&m2)));
    });

    c.bench_function("pir::hybrid_mul_hom", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let m1 = Matrix::rand_uniform(&mut rng);
        let m2 = Matrix::rand_uniform(&mut rng);
        b.iter(|| SPIRALTest::hybrid_mul_hom(black_box(&m1), black_box(&m2)));
    });

    c.bench_function("pir::answer_query_expand", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let (qk, pp) = SPIRALTest::setup(None);
        let idx = rng.gen_range(0..SPIRALTest::PACKED_DB_SIZE);
        let q = SPIRALTest::query_one(&qk, idx, None);
        b.iter(|| {
            let time_stats: Option<&mut Stats<Duration>> = None;
            SPIRALTest::answer_query_unpack(
                black_box(&pp),
                black_box(&q),
                black_box(None),
                black_box(time_stats),
            )
        });
    });

    c.bench_function("pir::answer_first_dim", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let db = SPIRALTest::encode_db(
            |_| RecordBytesImpl::<{ SPIRAL_TEST_PARAMS.BYTES_PER_RECORD }>::default(),
            None,
        )
        .0;

        let regevs: Vec<_> = (0..)
            .map(|_| Matrix::rand_uniform(&mut rng))
            .take(1 << SPIRALTest::NU1)
            .collect();
        b.iter(|| SPIRALTest::answer_first_dim(black_box(&db), black_box(regevs.as_slice())));
    });

    c.bench_function("pir::answer_fold", |b| {
        let mut rng = ChaCha20Rng::from_entropy();
        let first_dim_folded: Vec<_> = (0..)
            .map(|_| Matrix::rand_uniform(&mut rng))
            .take(1 << SPIRALTest::NU1)
            .collect();
        let gsws: Vec<_> = (0..)
            .map(|_| Matrix::rand_uniform(&mut rng))
            .take(SPIRALTest::NU2)
            .collect();

        // Note: this includes the time it takes to clone first_dim_folded
        b.iter(|| {
            SPIRALTest::answer_fold(
                black_box(first_dim_folded.clone()),
                black_box(gsws.as_slice()),
            )
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
