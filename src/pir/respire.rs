use bitvec::prelude::*;
use itertools::Itertools;
use log::Level::Info;
use log::{info, log_enabled};
use std::cmp::{max, min};
use std::f64::consts::PI;
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rand_distr::num_traits::clamp;

use crate::math::gadget::{
    base_from_len, build_gadget, gadget_inverse, gadget_inverse_scalar, RingElementDecomposable,
};
use crate::math::int_mod::IntMod;
use crate::math::int_mod_cyclo::IntModCyclo;
use crate::math::int_mod_cyclo_crt_eval::IntModCycloCRTEval;
use crate::math::int_mod_cyclo_eval::IntModCycloEval;
use crate::math::matrix::Matrix;

use crate::math::rand_sampled::{RandDiscreteGaussianSampled, RandUniformSampled};
use crate::math::ring_elem::{NormedRingElement, RingElement};
use crate::math::utils::{ceil_log, floor_log, mod_inverse, reverse_bits_fast};

use crate::math::simd_utils::*;
use crate::pir::pir::{PIRRecordBytes, SerdeRound, Stats, PIR};

pub struct RespireImpl<
    const Q1: u64,
    const Q1A: u64,
    const Q1B: u64,
    const D1: usize,
    const Z_GSW: u64,
    const T_GSW: usize,
    const M_GSW: usize,
    const Z_PROJ_SHORT: u64,
    const T_PROJ_SHORT: usize,
    const Z_PROJ_LONG: u64,
    const T_PROJ_LONG: usize,
    const Z_RLWE_TO_GSW: u64,
    const T_RLWE_TO_GSW: usize,
    const M_RLWE_TO_GSW: usize,
    const Z_VECTORIZE: u64,
    const T_VECTORIZE: usize,
    const BATCH_SIZE: usize,
    const N_VEC: usize,
    const ERROR_WIDTH_MILLIONTHS: u64,
    const ERROR_WIDTH_VEC_MILLIONTHS: u64,
    const ERROR_WIDTH_COMPRESS_MILLIONTHS: u64,
    const SECRET_BOUND: u64,
    const SECRET_WIDTH_VEC_MILLIONTHS: u64,
    const SECRET_WIDTH_COMPRESS_MILLIONTHS: u64,
    const P: u64,
    const D3: usize,
    const NU1: usize,
    const NU2: usize,
    const Q3: u64,
    const Q2: u64,
    const D2: usize,
    const T_COMPRESS: usize,
    const Z_COMPRESS: u64,
    const BYTES_PER_RECORD: usize,
> {}

#[allow(non_snake_case)]
pub struct RespireParams {
    pub Q1A: u64,
    pub Q1B: u64,
    pub D1: usize,
    pub T_GSW: usize,
    pub T_PROJ_SHORT: usize,
    pub T_PROJ_LONG: usize,
    pub T_RLWE_TO_GSW: usize,
    pub T_VECTORIZE: usize,
    pub BATCH_SIZE: usize,
    pub N_VEC: usize,
    pub ERROR_WIDTH_MILLIONTHS: u64,
    pub ERROR_WIDTH_VEC_MILLIONTHS: u64,
    pub ERROR_WIDTH_COMPRESS_MILLIONTHS: u64,
    pub SECRET_BOUND: u64,
    pub SECRET_WIDTH_VEC_MILLIONTHS: u64,
    pub SECRET_WIDTH_COMPRESS_MILLIONTHS: u64,
    pub P: u64,
    pub D3: usize,
    pub NU1: usize,
    pub NU2: usize,
    pub Q3: u64,
    pub Q2: u64,
    pub D2: usize,
}

impl RespireParams {
    pub const fn expand(&self) -> RespireParamsExpanded {
        let q1 = self.Q1A * self.Q1B;
        let z_gsw = base_from_len(self.T_GSW, q1);
        let z_proj_short = base_from_len(self.T_PROJ_SHORT, q1);
        let z_proj_long = base_from_len(self.T_PROJ_LONG, q1);
        let z_rlwe_to_gsw = base_from_len(self.T_RLWE_TO_GSW, q1);
        let z_vectorize = base_from_len(self.T_VECTORIZE, q1);
        let z_compress = 2;
        let t_compress = floor_log(z_compress, self.Q2) + 1;
        RespireParamsExpanded {
            Q1: q1,
            Q1A: self.Q1A,
            Q1B: self.Q1B,
            D1: self.D1,
            Z_GSW: z_gsw,
            T_GSW: self.T_GSW,
            M_GSW: 2 * self.T_GSW,
            Z_PROJ_SHORT: z_proj_short,
            T_PROJ_SHORT: self.T_PROJ_SHORT,
            Z_PROJ_LONG: z_proj_long,
            T_PROJ_LONG: self.T_PROJ_LONG,
            Z_RLWE_TO_GSW: z_rlwe_to_gsw,
            T_RLWE_TO_GSW: self.T_RLWE_TO_GSW,
            T_VECTORIZE: self.T_VECTORIZE,
            Z_VECTORIZE: z_vectorize,
            BATCH_SIZE: self.BATCH_SIZE,
            N_VEC: self.N_VEC,
            M_RLWE_TO_GSW: 2 * self.T_RLWE_TO_GSW,
            ERROR_WIDTH_MILLIONTHS: self.ERROR_WIDTH_MILLIONTHS,
            ERROR_WIDTH_VEC_MILLIONTHS: self.ERROR_WIDTH_VEC_MILLIONTHS,
            ERROR_WIDTH_COMPRESS_MILLIONTHS: self.ERROR_WIDTH_COMPRESS_MILLIONTHS,
            SECRET_BOUND: self.SECRET_BOUND,
            SECRET_WIDTH_VEC_MILLIONTHS: self.SECRET_WIDTH_VEC_MILLIONTHS,
            SECRET_WIDTH_COMPRESS_MILLIONTHS: self.SECRET_WIDTH_COMPRESS_MILLIONTHS,
            P: self.P,
            D3: self.D3,
            NU1: self.NU1,
            NU2: self.NU2,
            Q3: self.Q3,
            Q2: self.Q2,
            D2: self.D2,
            T_COMPRESS: t_compress,
            Z_COMPRESS: z_compress,
            BYTES_PER_RECORD: (self.D3 * floor_log(2, self.P)) / 8,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct RespireParamsExpanded {
    pub Q1: u64,
    pub Q1A: u64,
    pub Q1B: u64,
    pub D1: usize,
    pub Z_GSW: u64,
    pub T_GSW: usize,
    pub M_GSW: usize,
    pub Z_PROJ_SHORT: u64,
    pub T_PROJ_SHORT: usize,
    pub Z_PROJ_LONG: u64,
    pub T_PROJ_LONG: usize,
    pub Z_RLWE_TO_GSW: u64,
    pub T_RLWE_TO_GSW: usize,
    pub M_RLWE_TO_GSW: usize,
    pub Z_VECTORIZE: u64,
    pub T_VECTORIZE: usize,
    pub BATCH_SIZE: usize,
    pub N_VEC: usize,
    pub ERROR_WIDTH_MILLIONTHS: u64,
    pub ERROR_WIDTH_VEC_MILLIONTHS: u64,
    pub ERROR_WIDTH_COMPRESS_MILLIONTHS: u64,
    pub SECRET_BOUND: u64,
    pub SECRET_WIDTH_VEC_MILLIONTHS: u64,
    pub SECRET_WIDTH_COMPRESS_MILLIONTHS: u64,
    pub P: u64,
    pub D3: usize,
    pub NU1: usize,
    pub NU2: usize,
    pub Q3: u64,
    pub Q2: u64,
    pub D2: usize,
    pub T_COMPRESS: usize,
    pub Z_COMPRESS: u64,
    pub BYTES_PER_RECORD: usize,
}

#[macro_export]
macro_rules! respire {
    ($params: expr) => {
        $crate::pir::respire::RespireImpl<
            {$params.Q1},
            {$params.Q1A},
            {$params.Q1B},
            {$params.D1},
            {$params.Z_GSW},
            {$params.T_GSW},
            {$params.M_GSW},
            {$params.Z_PROJ_SHORT},
            {$params.T_PROJ_SHORT},
            {$params.Z_PROJ_LONG},
            {$params.T_PROJ_LONG},
            {$params.Z_RLWE_TO_GSW},
            {$params.T_RLWE_TO_GSW},
            {$params.M_RLWE_TO_GSW},
            {$params.Z_VECTORIZE},
            {$params.T_VECTORIZE},
            {$params.BATCH_SIZE},
            {$params.N_VEC},
            {$params.ERROR_WIDTH_MILLIONTHS},
            {$params.ERROR_WIDTH_VEC_MILLIONTHS},
            {$params.ERROR_WIDTH_COMPRESS_MILLIONTHS},
            {$params.SECRET_BOUND},
            {$params.SECRET_WIDTH_VEC_MILLIONTHS},
            {$params.SECRET_WIDTH_COMPRESS_MILLIONTHS},
            {$params.P},
            {$params.D3},
            {$params.NU1},
            {$params.NU2},
            {$params.Q3},
            {$params.Q2},
            {$params.D2},
            {$params.T_COMPRESS},
            {$params.Z_COMPRESS},
            {$params.BYTES_PER_RECORD},
        >
    }
}

macro_rules! respire_impl {
    ($impl_for: ident, $body: tt) => {
        impl<
                const Q1: u64,
                const Q1A: u64,
                const Q1B: u64,
                const D1: usize,
                const Z_GSW: u64,
                const T_GSW: usize,
                const M_GSW: usize,
                const Z_PROJ_SHORT: u64,
                const T_PROJ_SHORT: usize,
                const Z_PROJ_LONG: u64,
                const T_PROJ_LONG: usize,
                const Z_RLWE_TO_GSW: u64,
                const T_RLWE_TO_GSW: usize,
                const M_RLWE_TO_GSW: usize,
                const Z_VECTORIZE: u64,
                const T_VECTORIZE: usize,
                const BATCH_SIZE: usize,
                const N_VEC: usize,
                const ERROR_WIDTH_MILLIONTHS: u64,
                const ERROR_WIDTH_VEC_MILLIONTHS: u64,
                const ERROR_WIDTH_COMPRESS_MILLIONTHS: u64,
                const SECRET_BOUND: u64,
                const SECRET_WIDTH_VEC_MILLIONTHS: u64,
                const SECRET_WIDTH_COMPRESS_MILLIONTHS: u64,
                const P: u64,
                const D3: usize,
                const NU1: usize,
                const NU2: usize,
                const Q3: u64,
                const Q2: u64,
                const D2: usize,
                const T_COMPRESS: usize,
                const Z_COMPRESS: u64,
                const BYTES_PER_RECORD: usize,
            > $impl_for
            for RespireImpl<
                Q1,
                Q1A,
                Q1B,
                D1,
                Z_GSW,
                T_GSW,
                M_GSW,
                Z_PROJ_SHORT,
                T_PROJ_SHORT,
                Z_PROJ_LONG,
                T_PROJ_LONG,
                Z_RLWE_TO_GSW,
                T_RLWE_TO_GSW,
                M_RLWE_TO_GSW,
                Z_VECTORIZE,
                T_VECTORIZE,
                BATCH_SIZE,
                N_VEC,
                ERROR_WIDTH_MILLIONTHS,
                ERROR_WIDTH_VEC_MILLIONTHS,
                ERROR_WIDTH_COMPRESS_MILLIONTHS,
                SECRET_BOUND,
                SECRET_WIDTH_VEC_MILLIONTHS,
                SECRET_WIDTH_COMPRESS_MILLIONTHS,
                P,
                D3,
                NU1,
                NU2,
                Q3,
                Q2,
                D2,
                T_COMPRESS,
                Z_COMPRESS,
                BYTES_PER_RECORD,
            >
        $body
    };
    ($body: tt) => {
        impl<
                const Q1: u64,
                const Q1A: u64,
                const Q1B: u64,
                const D1: usize,
                const Z_GSW: u64,
                const T_GSW: usize,
                const M_GSW: usize,
                const Z_PROJ_SHORT: u64,
                const T_PROJ_SHORT: usize,
                const Z_PROJ_LONG: u64,
                const T_PROJ_LONG: usize,
                const Z_RLWE_TO_GSW: u64,
                const T_RLWE_TO_GSW: usize,
                const M_RLWE_TO_GSW: usize,
                const Z_VECTORIZE: u64,
                const T_VECTORIZE: usize,
                const BATCH_SIZE: usize,
                const N_VEC: usize,
                const ERROR_WIDTH_MILLIONTHS: u64,
                const ERROR_WIDTH_VEC_MILLIONTHS: u64,
                const ERROR_WIDTH_COMPRESS_MILLIONTHS: u64,
                const SECRET_BOUND: u64,
                const SECRET_WIDTH_VEC_MILLIONTHS: u64,
                const SECRET_WIDTH_COMPRESS_MILLIONTHS: u64,
                const P: u64,
                const D3: usize,
                const NU1: usize,
                const NU2: usize,
                const Q3: u64,
                const Q2: u64,
                const D2: usize,
                const T_COMPRESS: usize,
                const Z_COMPRESS: u64,
                const BYTES_PER_RECORD: usize,
            > RespireImpl<
                Q1,
                Q1A,
                Q1B,
                D1,
                Z_GSW,
                T_GSW,
                M_GSW,
                Z_PROJ_SHORT,
                T_PROJ_SHORT,
                Z_PROJ_LONG,
                T_PROJ_LONG,
                Z_RLWE_TO_GSW,
                T_RLWE_TO_GSW,
                M_RLWE_TO_GSW,
                Z_VECTORIZE,
                T_VECTORIZE,
                BATCH_SIZE,
                N_VEC,
                ERROR_WIDTH_MILLIONTHS,
                ERROR_WIDTH_VEC_MILLIONTHS,
                ERROR_WIDTH_COMPRESS_MILLIONTHS,
                SECRET_BOUND,
                SECRET_WIDTH_VEC_MILLIONTHS,
                SECRET_WIDTH_COMPRESS_MILLIONTHS,
                P,
                D3,
                NU1,
                NU2,
                Q3,
                Q2,
                D2,
                T_COMPRESS,
                Z_COMPRESS,
                BYTES_PER_RECORD,
            >
        $body
    };
}

pub trait Respire: PIR {
    // Type aliases
    type RingP;
    type RingQ1;
    type RingQ1Fast;
    type RLWEEncoding;
    type RLWEEncodingSeeded;
    type RLWEEncodingCompressed: SerdeRound;
    type GSWEncoding;
    type EncodingSecret;
    type VecEncodingSecret;
    type VecEncodingSecretQ2;
    type VecEncodingSecretQ2Small;
    type AutoParams<const T: usize>;
    type AutoParamsShort;
    type AutoParamsLong;
    type RLWEToGSWParams;
    type CompressParams;
    type VectorizeParams;
    type VecRLWEEncoding;
    type VecRLWEEncodingSmallTruncated: SerdeRound;

    // A single record
    type Record;
    // Packed records from a single response, after compression
    type RecordPackedSmall;
    // Packed records from a single response, before compression
    type RecordPacked;
    type QueryOne: SerdeRound;
    type QueryOneExpanded;
    type AnswerOne: SerdeRound;
    type AnswerOneCompressed: SerdeRound;

    // Constants
    const PACKED_DIM1_SIZE: usize;
    const PACKED_DIM2_SIZE: usize;
    const PACKED_DB_SIZE: usize;
    const DB_SIZE: usize;
    const N_VEC: usize;
    const PACK_RATIO_DB: usize;
    const PACK_RATIO_RESPONSE: usize;
    const RESPONSE_CHUNK_SIZE: usize;
    const NU1: usize;
    const NU3: usize;
    const NU2: usize;
    const RLWE_COUNT: usize;
    const RLWE_EXPAND_ITERS: usize;
    const GSW_FOLD_COUNT: usize;
    const GSW_ROT_COUNT: usize;
    const GSW_COUNT: usize;
    const GSW_EXPAND_ITERS: usize;

    fn query_one(
        qk: &<Self as PIR>::QueryKey,
        idx: usize,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::QueryOne;
    fn answer_one(
        pp: &<Self as PIR>::PublicParams,
        db: &<Self as PIR>::Database,
        q: &<Self as Respire>::QueryOne,
        qk: Option<&<Self as PIR>::QueryKey>,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::AnswerOne;
    fn answer_compress_chunk(
        pp: &<Self as PIR>::PublicParams,
        chunk: &[<Self as Respire>::AnswerOne],
        qk: Option<&<Self as PIR>::QueryKey>,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::AnswerOneCompressed;
    fn answer_compress_vec(
        pp: &<Self as PIR>::PublicParams,
        vec: &<Self as Respire>::VecRLWEEncoding,
        truncate_len: usize,
    ) -> <Self as Respire>::AnswerOneCompressed;
    fn extract_one(
        qk: &<Self as PIR>::QueryKey,
        r: &<Self as Respire>::AnswerOneCompressed,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> Vec<<Self as PIR>::RecordBytes>;

    fn params() -> RespireParamsExpanded;
    fn params_error_rate_estimate() -> f64;
    fn params_public_param_size() -> usize;
    fn params_query_one_size() -> usize;
    fn params_record_one_size() -> usize;
    fn params_response_one_size(trunc_len: usize) -> usize;
}

#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordBytesImpl<const LEN: usize> {
    it: [u8; LEN],
}

impl<const LEN: usize> Default for RecordBytesImpl<LEN> {
    fn default() -> Self {
        Self { it: [0u8; LEN] }
    }
}

impl<const LEN: usize> PIRRecordBytes for RecordBytesImpl<LEN> {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes.try_into() {
            Ok(bytes) => Some(Self { it: bytes }),
            Err(_) => None,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.it
    }
}

respire_impl!(PIR, {
    // Associated types
    type QueryKey = (
        <Self as Respire>::EncodingSecret,
        <Self as Respire>::VecEncodingSecret,
        <Self as Respire>::VecEncodingSecretQ2Small,
    );
    type PublicParams = (
        (
            Vec<<Self as Respire>::AutoParamsShort>,
            Vec<<Self as Respire>::AutoParamsLong>,
        ),
        <Self as Respire>::RLWEToGSWParams,
        <Self as Respire>::CompressParams,
        <Self as Respire>::VectorizeParams,
    );

    type Query = Vec<<Self as Respire>::QueryOne>;
    type Response = Vec<<Self as Respire>::AnswerOneCompressed>;

    /// We structure the database as `[2] x [D1 / S] x [DIM2_SIZE] x [DIM1_SIZE] x [S]` for optimal first dimension
    /// processing. The outermost pair is the first resp. second CRT projections, packed as two u32 into one u64;
    /// `S` is the SIMD lane count that we can use, i.e. 4 for AVX2.
    type Database = Vec<SimdVec>;
    type DatabaseHint = ();
    type State = ();

    // Public types & constants
    type RecordBytes = RecordBytesImpl<BYTES_PER_RECORD>;
    const BYTES_PER_RECORD: usize = BYTES_PER_RECORD;
    const NUM_RECORDS: usize = Self::DB_SIZE;
    const BATCH_SIZE: usize = BATCH_SIZE;

    fn print_summary() {
        eprintln!(
            "RESPIRE with {} bytes x {} records ({:.3} MiB)",
            Self::BYTES_PER_RECORD,
            Self::NUM_RECORDS,
            (Self::BYTES_PER_RECORD * Self::NUM_RECORDS) as f64 / 1024_f64 / 1024_f64,
        );
        eprintln!("Parameters: {:#?}", Self::params());
        eprintln!(
            "Public param size: {:.3} KiB",
            Self::params_public_param_size() as f64 / 1024_f64
        );
        eprintln!(
            "Query size: {:.3} KiB",
            Self::params_query_size() as f64 / 1024_f64
        );

        let (resp_size, resp_full_vecs, resp_rem) = Self::params_response_info();
        info!(
            "Response: {} record(s) => {} ring elem(s) => {} full vector(s), {} remainder",
            Self::BATCH_SIZE,
            Self::BATCH_SIZE.div_ceil(Self::PACK_RATIO_RESPONSE),
            resp_full_vecs,
            resp_rem
        );

        eprintln!(
            "Response size (batch): {:.3} KiB",
            resp_size as f64 / 1024_f64
        );

        eprintln!(
            "Record size (batch): {:.3} KiB",
            Self::params_record_size() as f64 / 1024_f64
        );
        eprintln!("Rate: {:.3}", Self::params_rate());

        eprintln!(
            "Error rate (estimated): 2^({:.3})",
            Self::params_error_rate_estimate().log2()
        )
    }

    fn encode_db<F: Fn(usize) -> Self::RecordBytes>(
        records_generator: F,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> (Self::Database, Self::DatabaseHint) {
        let begin = Instant::now();
        let records_encoded_generator = |idx: usize| Self::encode_record(&records_generator(idx));

        assert!(Q1A <= u32::MAX as u64);
        assert!(Q1B <= u32::MAX as u64);
        assert_eq!(Self::DB_SIZE % Self::PACK_RATIO_DB, 0);

        let records_packed_generator = |chunk_idx: usize| {
            let chunk = (Self::PACK_RATIO_DB * chunk_idx..Self::PACK_RATIO_DB * (chunk_idx + 1))
                .map(|idx| records_encoded_generator(idx));
            let mut record_packed = IntModCyclo::<D1, P>::zero();
            for (record_in_chunk, record) in chunk.enumerate() {
                for (coeff_idx, coeff) in record.coeff.iter().enumerate() {
                    record_packed.coeff[Self::PACK_RATIO_DB * coeff_idx + record_in_chunk] = *coeff;
                }
            }
            let value = <Self as Respire>::RingQ1Fast::from(&record_packed.include_into::<Q1>());
            let mut packed_value = [0u64; D1];
            for i in 0..D1 {
                packed_value[i] = {
                    let lo = u64::from(value.proj1.evals[i]);
                    let hi = u64::from(value.proj2.evals[i]);
                    (hi << 32) | lo
                };
            }
            packed_value
        };

        // Quick and dirty buffered iterator implementation
        struct BufferedRecordsIterator<T: Clone, F: Fn(usize) -> T, const N: usize> {
            idx: usize,
            size: usize,
            buffer: Vec<T>,
            source: F,
        }
        impl<T: Clone, F: Fn(usize) -> T, const N: usize> BufferedRecordsIterator<T, F, N> {
            fn new(size: usize, source: F) -> Self {
                let buffer = Vec::with_capacity(N);
                Self {
                    idx: 0,
                    size,
                    buffer,
                    source,
                }
            }
        }
        impl<T: Clone, F: Fn(usize) -> T, const N: usize> Iterator for BufferedRecordsIterator<T, F, N> {
            type Item = T;
            fn next(&mut self) -> Option<Self::Item> {
                if self.idx >= self.size {
                    None
                } else {
                    if self.idx % N == 0 {
                        if self.buffer.len() == 0 {
                            // First time
                            for i in 0..N {
                                if self.idx + i >= self.size {
                                    break;
                                }
                                self.buffer.push((self.source)(self.idx + i));
                            }
                        } else {
                            for i in 0..N {
                                if self.idx + i >= self.size {
                                    break;
                                }
                                self.buffer[i] = (self.source)(self.idx + i);
                            }
                        }
                    }
                    let result = self.buffer[self.idx % N].clone();
                    self.idx += 1;
                    Some(result)
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                let rem = self.size - self.idx;
                (rem, Some(rem))
            }
        }
        impl<T: Clone, F: Fn(usize) -> T, const N: usize> ExactSizeIterator
            for BufferedRecordsIterator<T, F, N>
        {
        }

        // 16 MiB buffer, assuming d = 2048
        let records_packed_iter = BufferedRecordsIterator::<_, _, 65536>::new(
            Self::DB_SIZE / Self::PACK_RATIO_DB,
            records_packed_generator,
        );
        assert_eq!(records_packed_iter.len(), Self::PACKED_DB_SIZE);

        info!("Encoding DB...");
        let mut db: Vec<SimdVec>;

        #[cfg(not(target_feature = "avx2"))]
        {
            db = (0..(D1 * Self::PACKED_DB_SIZE)).map(|_| 0_u64).collect();
            for (db_idx, record_packed) in records_packed_iter.enumerate() {
                for eval_vec_idx in 0..D1 {
                    // Transpose the index
                    let (db_i, db_j) = (
                        db_idx / Self::PACKED_DIM2_SIZE,
                        db_idx % Self::PACKED_DIM2_SIZE,
                    );
                    let db_idx_t = db_j * Self::PACKED_DIM1_SIZE + db_i;

                    let to_idx = eval_vec_idx * Self::PACKED_DB_SIZE + db_idx_t;
                    let from_idx = eval_vec_idx;
                    db[to_idx] = record_packed[from_idx];
                }
            }
        }

        #[cfg(target_feature = "avx2")]
        {
            db = (0..((D1 / SIMD_LANES) * Self::PACKED_DB_SIZE))
                .map(|_| Aligned32([0_u64; 4]))
                .collect();

            for (db_idx, record_packed) in records_packed_iter.enumerate() {
                for eval_vec_idx in 0..(D1 / SIMD_LANES) {
                    // Transpose the index
                    let (db_i, db_j) = (
                        db_idx / Self::PACKED_DIM2_SIZE,
                        db_idx % Self::PACKED_DIM2_SIZE,
                    );
                    let db_idx_t = db_j * Self::PACKED_DIM1_SIZE + db_i;

                    let mut db_vec: SimdVec = Aligned32([0_u64; 4]);
                    for lane in 0..SIMD_LANES {
                        let from_idx = eval_vec_idx * SIMD_LANES + lane;
                        db_vec.0[lane] = record_packed[from_idx];
                    }

                    let to_idx = eval_vec_idx * Self::PACKED_DB_SIZE + db_idx_t;
                    db[to_idx] = db_vec;
                }
            }
        }

        let end = Instant::now();
        if let Some(time_stats) = time_stats {
            time_stats.add("encode", end - begin);
        }

        info!("Done processing DB");
        (db, ())
    }

    fn setup(
        time_stats: Option<&mut Stats<Duration>>,
    ) -> (<Self as PIR>::QueryKey, <Self as PIR>::PublicParams) {
        let begin = Instant::now();
        // RLWE/GSW secret
        let s_encode = Self::encode_setup();

        // Vector RLWE secret
        let s_vec: <Self as Respire>::VecEncodingSecret = Self::encode_vec_setup();

        // Compression target
        let s_small: <Self as Respire>::VecEncodingSecretQ2Small = {
            let mut rng = ChaCha20Rng::from_entropy();
            let mut result = Matrix::zero();
            for i in 0..N_VEC {
                result[(i, 0)] = IntModCycloEval::rand_discrete_gaussian::<
                    _,
                    SECRET_WIDTH_COMPRESS_MILLIONTHS,
                >(&mut rng);
            }
            result
        };

        // Vector secret mod Q2 (compression source secret)
        let s_vec_q2 = s_vec.map_ring(|r| {
            IntModCycloEval::from(IntModCyclo::from(
                IntModCyclo::<D1, Q1>::from(r)
                    .coeff
                    .map(|x| IntMod::from(i64::from(x))),
            ))
        });
        let s_small_q2 =
            s_small.map_ring(|r| IntModCycloEval::from(IntModCyclo::from(r).include_dim()));
        let compress_params = Self::compress_setup(&s_vec_q2, &s_small_q2);

        let mut auto_params_short: Vec<<Self as Respire>::AutoParamsShort> =
            Vec::with_capacity(Self::RLWE_EXPAND_ITERS);
        for i in 0..floor_log(2, D1 as u64) {
            let tau_power = (D1 >> i) + 1;
            auto_params_short.push(Self::auto_setup::<T_PROJ_SHORT, Z_PROJ_SHORT>(
                tau_power, &s_encode,
            ));
        }
        let mut auto_params_long: Vec<<Self as Respire>::AutoParamsLong> =
            Vec::with_capacity(Self::GSW_EXPAND_ITERS);
        for i in 0..floor_log(2, D1 as u64) {
            let tau_power = (D1 >> i) + 1;
            auto_params_long.push(Self::auto_setup::<T_PROJ_LONG, Z_PROJ_LONG>(
                tau_power, &s_encode,
            ));
        }

        let rlwe_to_gsw_params = Self::rlwe_to_gsw_setup(&s_encode);
        let vectorize_params = Self::vectorize_setup(&s_encode, &s_vec);

        let end = Instant::now();
        if let Some(time_stats) = time_stats {
            time_stats.add("setup", end - begin);
        }

        (
            (s_encode, s_vec, s_small),
            (
                (auto_params_short, auto_params_long),
                rlwe_to_gsw_params,
                compress_params,
                vectorize_params,
            ),
        )
    }

    fn query(
        qk: &<Self as PIR>::QueryKey,
        indices: &[usize],
        _: &<Self as PIR>::DatabaseHint,
        mut time_stats: Option<&mut Stats<Duration>>,
    ) -> (<Self as PIR>::Query, <Self as PIR>::State) {
        assert_eq!(indices.len(), Self::BATCH_SIZE);
        let q = indices
            .iter()
            .copied()
            .map(|idx| Self::query_one(qk, idx, time_stats.as_deref_mut()))
            .collect_vec();
        (q, ())
    }

    fn answer(
        pp: &<Self as PIR>::PublicParams,
        db: &<Self as PIR>::Database,
        qs: &<Self as PIR>::Query,
        qk: Option<&<Self as PIR>::QueryKey>,
        mut time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as PIR>::Response {
        assert_eq!(qs.len(), Self::BATCH_SIZE);
        let answers = qs
            .iter()
            .map(|q| Self::answer_one(pp, db, q, qk, time_stats.as_deref_mut()))
            .collect_vec();
        let answers_compressed = answers
            .chunks(N_VEC * Self::PACK_RATIO_RESPONSE)
            .map(|chunk| Self::answer_compress_chunk(pp, chunk, qk, time_stats.as_deref_mut()))
            .collect_vec();
        answers_compressed
    }

    fn extract(
        qk: &Self::QueryKey,
        r: &Self::Response,
        _: &Self::State,
        mut time_stats: Option<&mut Stats<Duration>>,
    ) -> Vec<Self::RecordBytes> {
        let mut result = Vec::with_capacity(Self::BATCH_SIZE);
        for r_one in r {
            let extracted = Self::extract_one(qk, r_one, time_stats.as_deref_mut());
            for record in extracted {
                if result.len() < Self::BATCH_SIZE {
                    result.push(record);
                }
            }
        }
        result
    }
});

respire_impl!(Respire, {
    type RingP = IntModCyclo<D1, P>;
    type RingQ1 = IntModCyclo<D1, Q1>;
    type RingQ1Fast = IntModCycloCRTEval<D1, Q1A, Q1B>;
    type RLWEEncoding = Matrix<2, 1, Self::RingQ1Fast>;
    type RLWEEncodingSeeded = ([u8; 32], Self::RingQ1Fast);
    type RLWEEncodingCompressed = ([u8; 32], Vec<IntMod<Q1>>);
    type GSWEncoding = Matrix<2, M_GSW, Self::RingQ1Fast>;

    type EncodingSecret = Self::RingQ1Fast;
    type VecEncodingSecret = Matrix<N_VEC, 1, Self::RingQ1Fast>;
    type VecEncodingSecretQ2 = Matrix<N_VEC, 1, IntModCycloEval<D1, Q2>>;
    type VecEncodingSecretQ2Small = Matrix<N_VEC, 1, IntModCycloEval<D2, Q2>>;
    type AutoParams<const T: usize> = (Matrix<2, T, Self::RingQ1Fast>, usize);
    type AutoParamsShort = Self::AutoParams<T_PROJ_SHORT>;
    type AutoParamsLong = Self::AutoParams<T_PROJ_LONG>;
    type RLWEToGSWParams = Matrix<2, M_RLWE_TO_GSW, Self::RingQ1Fast>;
    type CompressParams = (
        Matrix<1, T_COMPRESS, IntModCycloEval<D1, Q2>>,
        Matrix<N_VEC, T_COMPRESS, IntModCycloEval<D1, Q2>>,
    );
    type VectorizeParams = Vec<(
        Matrix<1, T_VECTORIZE, Self::RingQ1Fast>,
        Matrix<N_VEC, T_VECTORIZE, Self::RingQ1Fast>,
    )>;
    type VecRLWEEncoding = (Self::RingQ1Fast, Matrix<N_VEC, 1, Self::RingQ1Fast>);
    type VecRLWEEncodingSmallTruncated = (
        IntModCyclo<D2, Q2>,
        // Length may be truncated to less than N_VEC
        Vec<IntModCyclo<D2, Q3>>,
    );

    type Record = IntModCyclo<D3, P>;
    type RecordPackedSmall = Matrix<N_VEC, 1, IntModCyclo<D2, P>>;
    type RecordPacked = IntModCyclo<D1, P>;
    type QueryOne = (
        <Self as Respire>::RLWEEncodingCompressed,
        <Self as Respire>::RLWEEncodingCompressed,
    );
    type QueryOneExpanded = (
        Vec<<Self as Respire>::RLWEEncoding>, // first dim
        Vec<<Self as Respire>::GSWEncoding>,  // fold
        Vec<<Self as Respire>::GSWEncoding>,  // rotate
    );
    type AnswerOne = <Self as Respire>::RLWEEncoding;
    type AnswerOneCompressed = <Self as Respire>::VecRLWEEncodingSmallTruncated;

    const PACKED_DIM1_SIZE: usize = 2_usize.pow(NU1 as u32);
    const PACKED_DIM2_SIZE: usize = 2_usize.pow(NU2 as u32);
    const PACKED_DB_SIZE: usize = Self::PACKED_DIM1_SIZE * Self::PACKED_DIM2_SIZE;
    const DB_SIZE: usize = Self::PACKED_DB_SIZE * Self::PACK_RATIO_DB;
    const N_VEC: usize = N_VEC;
    const PACK_RATIO_DB: usize = D1 / D3;
    const PACK_RATIO_RESPONSE: usize = D2 / D3;
    const RESPONSE_CHUNK_SIZE: usize = N_VEC * Self::PACK_RATIO_RESPONSE;
    const NU1: usize = NU1;
    const NU2: usize = NU2;
    const NU3: usize = ceil_log(2, (D1 / D3) as u64);

    const RLWE_COUNT: usize = 1 << NU1;
    const RLWE_EXPAND_ITERS: usize = NU1;
    const GSW_FOLD_COUNT: usize = NU2;
    const GSW_ROT_COUNT: usize = Self::NU3;

    const GSW_COUNT: usize = (Self::GSW_FOLD_COUNT + Self::GSW_ROT_COUNT) * T_GSW;
    const GSW_EXPAND_ITERS: usize = ceil_log(2, Self::GSW_COUNT as u64);

    fn query_one(
        (s_encode, _, _): &<Self as PIR>::QueryKey,
        idx: usize,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::QueryOne {
        let begin = Instant::now();
        assert!(idx < Self::DB_SIZE);
        let last_dims_size = 2usize.pow((Self::NU2 + Self::NU3) as u32);
        let (idx_i, idx_j) = (idx / last_dims_size, idx % last_dims_size);

        let mut mu_rlwe = <Self as Respire>::RingQ1::zero();
        for i in 0..Self::RLWE_COUNT {
            mu_rlwe.coeff[reverse_bits_fast::<D1>(i)] =
                IntMod::<P>::from((i == idx_i) as u64).scale_up_into();
        }

        // [NU2 + NU3] x [T_GSW]
        let mut mu_gsw = <Self as Respire>::RingQ1::zero();

        let mut bits = Vec::with_capacity(NU2);
        let mut idx_j_curr = idx_j;
        for _ in 0..(Self::NU2 + Self::NU3) {
            bits.push(idx_j_curr % 2);
            idx_j_curr /= 2;
        }

        for (bit_idx, bit) in bits.into_iter().rev().enumerate() {
            let mut msg = IntMod::from(bit as u64);
            for gsw_pow in 0..T_GSW {
                let pack_idx = T_GSW * bit_idx + gsw_pow;
                mu_gsw.coeff[reverse_bits_fast::<D1>(pack_idx)] = msg;
                msg *= IntMod::from(Z_GSW);
            }
        }

        let (seed_rlwe, ct1_rlwe) = Self::encode_rlwe_seeded(s_encode, &mu_rlwe);
        let ct1_rlwe_coeff = <Self as Respire>::RingQ1::from(&ct1_rlwe).coeff;
        let (seed_gsw, ct1_gsw) = Self::encode_rlwe_seeded(s_encode, &mu_gsw);
        let ct1_gsw_coeff = <Self as Respire>::RingQ1::from(&ct1_gsw).coeff;
        let compressed_rlwe = (
            seed_rlwe,
            (0..Self::RLWE_COUNT)
                .map(|i| ct1_rlwe_coeff[reverse_bits_fast::<D1>(i)])
                .collect_vec(),
        );
        let compressed_gsw = (
            seed_gsw,
            (0..Self::GSW_COUNT)
                .map(|i| ct1_gsw_coeff[reverse_bits_fast::<D1>(i)])
                .collect_vec(),
        );

        let end = Instant::now();
        if let Some(time_stats) = time_stats {
            time_stats.add("query", end - begin);
        }
        (compressed_rlwe, compressed_gsw)
    }

    fn answer_one(
        pp: &<Self as PIR>::PublicParams,
        db: &<Self as PIR>::Database,
        q: &<Self as Respire>::QueryOne,
        qk: Option<&<Self as PIR>::QueryKey>,
        mut time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::AnswerOne {
        // Query expansion
        let (rlwes, gsws_fold, gsws_rot) =
            Self::answer_query_unpack(pp, q, qk, time_stats.as_deref_mut());
        let rlwe_saved = rlwes[0].clone();

        let i1 = Instant::now();

        // First dimension
        let c_firstdim = Self::answer_first_dim(db, &rlwes);
        let firstdim_saved = c_firstdim[0].clone(); // save for noise logging
        let i2 = Instant::now();

        // Folding
        let c_fold = Self::answer_fold(c_firstdim, gsws_fold.as_slice());
        let i3 = Instant::now();

        // Rotate select
        let c_rot = Self::answer_rotate(&c_fold, gsws_rot.as_slice());
        let i4 = Instant::now();

        // Project
        let c_proj = Self::answer_project(pp, &c_rot);
        let i5 = Instant::now();

        if let Some(time_stats) = time_stats {
            time_stats.add("answer_first_dim", i2 - i1);
            time_stats.add("answer_fold", i3 - i2);
            time_stats.add("answer_rotate", i4 - i3);
            time_stats.add("answer_project", i5 - i4);
        }

        if let Some((s_enc, _, _)) = qk {
            if log_enabled!(Info) {
                let e_rlwe = Self::noise_subgaussian_bits(s_enc, &rlwe_saved);
                let e_firstdim = Self::noise_subgaussian_bits(s_enc, &firstdim_saved);
                let e_fold = Self::noise_subgaussian_bits(s_enc, &c_fold);
                let e_rot = Self::noise_subgaussian_bits(s_enc, &c_rot);
                let e_proj = Self::noise_subgaussian_bits(s_enc, &c_proj);

                info!("measured noise query expanded rlwe: {}", e_rlwe);
                info!("measured noise first dim: {}", e_firstdim);
                info!("measured noise fold: {}", e_fold);
                info!("measured noise rotate: {}", e_rot);
                // TODO: note that project noise is lower on the coefficients that are projected away. So reporting this average is a bit inaccurate.
                info!("measured noise project*: {}", e_proj);
            }
        }

        c_proj
    }

    fn answer_compress_chunk(
        pp: &<Self as PIR>::PublicParams,
        chunk: &[<Self as Respire>::AnswerOne],
        qk: Option<&<Self as PIR>::QueryKey>,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::AnswerOneCompressed {
        let begin = Instant::now();
        let mut scalar_cts = Vec::with_capacity(Self::RESPONSE_CHUNK_SIZE);
        let (_, _, _, vectorize_params) = pp;
        for vec_idx in 0..N_VEC {
            let mut scalar_ct = Matrix::zero();
            for pack_idx in 0..Self::PACK_RATIO_RESPONSE {
                let idx = vec_idx * Self::PACK_RATIO_RESPONSE + pack_idx;
                if idx < chunk.len() {
                    scalar_ct += &Self::rlwe_mul_x_pow(&chunk[idx], pack_idx * (D1 / D2));
                }
            }
            scalar_cts.push(scalar_ct)
        }
        let vec = Self::scal_to_vec(vectorize_params, scalar_cts.as_slice().try_into().unwrap());
        let compressed =
            Self::answer_compress_vec(pp, &vec, chunk.len().div_ceil(Self::PACK_RATIO_RESPONSE));

        let end = Instant::now();
        if let Some(time_stats) = time_stats {
            time_stats.add("answer_compress", end - begin);
        }

        if let Some((_, s_vec, _)) = qk {
            info!(
                "pre compression noise (subgaussian widths): 2^({})",
                Self::noise_subgaussian_bits_vec(s_vec, &vec)
            );
        }
        compressed
    }

    fn answer_compress_vec(
        (_, _, (a_t, b_mat), _): &<Self as PIR>::PublicParams,
        (c_r, c_m): &<Self as Respire>::VecRLWEEncoding,
        truncate_len: usize,
    ) -> <Self as Respire>::AnswerOneCompressed {
        let c_r = IntModCyclo::<D1, Q1>::from(c_r);
        let c_m = c_m.map_ring(|r| IntModCyclo::<D1, Q1>::from(r));
        let mut cr_scaled = IntModCyclo::zero();
        for (cr_scaled_coeff, c0_coeff) in cr_scaled.coeff.iter_mut().zip(c_r.coeff) {
            let numer = Q2 as u128 * u64::from(c0_coeff) as u128;
            let denom = Q1 as u128;
            let div = (numer + denom / 2) / denom;
            *cr_scaled_coeff = IntMod::from(div as u64);
        }
        let g_inv_cr_scaled = gadget_inverse_scalar::<_, Z_COMPRESS, T_COMPRESS>(&cr_scaled)
            .map_ring(|x| IntModCycloEval::from(x));
        let c_r_hat: IntModCyclo<D2, Q2> =
            IntModCyclo::from(&(a_t * &g_inv_cr_scaled)[(0, 0)]).project_dim();
        let c_m_hat_trunc = {
            let b_g_inv = (b_mat * &g_inv_cr_scaled).map_ring(|r| IntModCyclo::from(r));
            let mut result = vec![IntModCyclo::<D1, Q3>::zero(); truncate_len];
            for i in 0..truncate_len {
                for (result_coeff, (c1_coeff, b_t_g_inv_coeff)) in result[i]
                    .coeff
                    .iter_mut()
                    .zip(c_m[(i, 0)].coeff.iter().copied().zip(b_g_inv[(i, 0)].coeff))
                {
                    let numer = Q3 as u128 * Q2 as u128 * u64::from(c1_coeff) as u128
                        + Q1 as u128 * Q3 as u128 * u64::from(b_t_g_inv_coeff) as u128;
                    let denom = Q1 as u128 * Q2 as u128;
                    let div = (numer + denom / 2) / denom;
                    *result_coeff = IntMod::from(div as u64);
                }
            }
            result.into_iter().map(|x| x.project_dim()).collect_vec()
        };
        (c_r_hat, c_m_hat_trunc)
    }

    fn extract_one(
        qk: &<Self as PIR>::QueryKey,
        r: &<Self as Respire>::AnswerOneCompressed,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> Vec<<Self as PIR>::RecordBytes> {
        let begin = Instant::now();
        let ret = Self::extract_bytes_one(&Self::extract_ring_one(qk, r));
        let end = Instant::now();
        if let Some(time_stats) = time_stats {
            time_stats.add("extract", end - begin);
        }
        ret
    }

    fn params() -> RespireParamsExpanded {
        RespireParamsExpanded {
            Q1,
            Q1A,
            Q1B,
            D1,
            Z_GSW,
            T_GSW,
            M_GSW,
            Z_PROJ_SHORT,
            T_PROJ_SHORT,
            Z_PROJ_LONG,
            T_PROJ_LONG,
            Z_RLWE_TO_GSW,
            T_RLWE_TO_GSW,
            Z_VECTORIZE,
            T_VECTORIZE,
            BATCH_SIZE,
            N_VEC,
            M_RLWE_TO_GSW,
            ERROR_WIDTH_MILLIONTHS,
            ERROR_WIDTH_VEC_MILLIONTHS,
            ERROR_WIDTH_COMPRESS_MILLIONTHS,
            SECRET_BOUND,
            SECRET_WIDTH_VEC_MILLIONTHS,
            SECRET_WIDTH_COMPRESS_MILLIONTHS,
            P,
            D3,
            NU1,
            NU2,
            Q3,
            Q2,
            D2,
            T_COMPRESS,
            Z_COMPRESS,
            BYTES_PER_RECORD,
        }
    }

    fn params_error_rate_estimate() -> f64 {
        info!("*** Error estimates (bits) ***");
        // We use square subgaussian widths as units
        let error_width_sq: f64 = ((ERROR_WIDTH_MILLIONTHS as f64) / 1_000_000_f64).powi(2);
        let error_width_vec_sq: f64 = ((ERROR_WIDTH_VEC_MILLIONTHS as f64) / 1_000_000_f64).powi(2);
        let error_width_switch_sq: f64 =
            ((ERROR_WIDTH_COMPRESS_MILLIONTHS as f64) / 1_000_000_f64).powi(2);
        let secret_bound_sq: f64 = (SECRET_BOUND as f64).powi(2);
        let secret_width_vec_sq: f64 =
            ((SECRET_WIDTH_VEC_MILLIONTHS as f64) / 1_000_000_f64).powi(2);
        // let secret_width_switch_sq: f64 =
        //     ((SECRET_WIDTH_COMPRESS_MILLIONTHS as f64) / 1_000_000_f64).powi(2);

        let log_d: usize = ceil_log(2, D1 as u64);

        let e_to_bits = |e: f64| -> f64 { e.log2() / 2_f64 };

        let gadget_factor = |t: usize, z: u64| -> f64 {
            assert!(z >= 2);

            let z_factor = match z {
                2 => {
                    // With probability <= 2^(-48.421), a random binary gadget will have <= 1200 equal 1
                    // N(log(1 - sum(binomial(2048, x) * (1/2)^2048, x, 0, 1200), 2)) = -48.4216972197488
                    const BINARY_FACTOR: f64 = 1200_f64 / 2048_f64;
                    BINARY_FACTOR
                }
                // 3 => {
                //     // With probability <= 2^(-48.506), a random ternary gadget will have <= 1528 coefficients equal +- 1
                //     // N(log(1 - sum(binomial(2048, x) * (2/3)^x * (1/3)^(2048-x), x, 0, 1528), 2)) = -48.5067228501629
                //     const TERNARY_FACTOR: f64 = 1528_f64 / 2048_f64;
                //     TERNARY_FACTOR
                // }
                _ => {
                    // TODO noise: verify this factor is right
                    // const CHERNOFF_FACTOR: f64 = 0.6_f64;
                    const CHERNOFF_FACTOR: f64 = 1.0_f64;
                    ((z / 2) as f64).powi(2) * CHERNOFF_FACTOR
                }
            };

            (t as f64) * z_factor
        };

        let select_noise = |e_gsw_sq: f64, e_reg_sq: f64, depth: usize| -> f64 {
            // m = 2t; t is absorbed into gadget_factor()
            e_reg_sq + (depth as f64) * 2_f64 * (D1 as f64) * gadget_factor(T_GSW, Z_GSW) * e_gsw_sq
        };

        let proj_noise = |e_sq: f64, t_auto: usize, z_auto: u64, depth: usize| -> f64 {
            let ct = (4usize.pow(depth as u32) - 1) / 3;
            e_sq + ct as f64 * (D1 as f64) * gadget_factor(t_auto, z_auto) * error_width_sq
        };

        info!("Initial: {}", e_to_bits(error_width_sq));

        // Query expansion
        let e_reg = proj_noise(error_width_sq, T_PROJ_SHORT, Z_PROJ_SHORT, log_d);
        info!("Query expand rlwe: {}", e_to_bits(e_reg));
        let e_gsw_raw = proj_noise(error_width_sq, T_PROJ_LONG, Z_PROJ_LONG, log_d);
        info!("Query expand GSW (raw): {}", e_to_bits(e_gsw_raw));
        let e_gsw = {
            // RLWE to GSW
            let initial_component = (D1 as f64) * e_gsw_raw * secret_bound_sq;
            let gadget_component =
                // m = 2t; t is absorbed into gadget_factor()
                2_f64 * (D1 as f64) * gadget_factor(T_RLWE_TO_GSW, Z_RLWE_TO_GSW) * error_width_sq;
            let e_converted = initial_component + gadget_component;
            info!(
                "    RLWE to GSW initial component: {}",
                e_to_bits(initial_component),
            );
            info!(
                "    RLWE to GSW gadget component: {}",
                e_to_bits(gadget_component),
            );
            e_converted
        };
        info!("Query expand GSW (converted): {}", e_to_bits(e_gsw));

        // First dimension (NU1)
        let e_firstdim =
            (Self::PACKED_DIM1_SIZE as f64) * (D1 as f64) * ((P / 2) as f64).powi(2) * e_reg;
        info!("First dimension: {}", e_to_bits(e_firstdim));

        // Folding (NU2)
        let e_fold = select_noise(e_gsw, e_firstdim, Self::NU2);
        info!("Fold: {}", e_to_bits(e_fold));

        // Rotating (NU3)
        let e_rot = select_noise(e_gsw, e_fold, Self::NU3);
        info!("Rotate select: {}", e_to_bits(e_rot));

        // Proj/select (NU3) + ring packing
        let e_proj_component = proj_noise(0_f64, T_PROJ_LONG, Z_PROJ_LONG, Self::NU3);
        info!(
            "    Projection *new* error component: {}",
            e_to_bits(e_proj_component)
        );
        let ring_num_records = min(Self::BATCH_SIZE, Self::PACK_RATIO_RESPONSE);
        let e_pack_ring = e_rot + e_proj_component * ring_num_records as f64;
        info!(
            "Ring packing ({} record(s)): {}",
            ring_num_records,
            e_to_bits(e_pack_ring)
        );

        // Vector packing
        let e_pack_vec = {
            // Scalar to vector conversion
            let e_init = e_pack_ring;
            let vec_num_elems = min(
                Self::BATCH_SIZE.div_ceil(Self::PACK_RATIO_RESPONSE),
                Self::N_VEC,
            );
            let e_extra = vec_num_elems as f64
                * (D1 as f64)
                * gadget_factor(T_VECTORIZE, Z_VECTORIZE)
                * error_width_vec_sq;
            info!(
                "    Vector packing *new* error component ({} ring elem(s)): {}",
                vec_num_elems,
                e_to_bits(e_extra),
            );
            let e_full = e_init + e_extra;
            info!("Vector packing: {}", e_to_bits(e_full));
            e_full
        };

        let e_preswitch = e_pack_vec;
        info!("***");
        info!(
            "Preswitch noise: {:.3} total bits; approx {:.3} of margin",
            e_to_bits(e_preswitch),
            (Q1 as f64).log2() - (P as f64).log2() - e_to_bits(e_preswitch) - 3_f64 // 3 bits = 8 widths
        );
        assert_eq!(Z_COMPRESS, 2);
        let e_preswitch = 8_f64 * e_preswitch;

        let e_subg_preswitch = e_preswitch * (Q3 as f64).powi(2) / (Q1 as f64).powi(2);
        let e_subg_gadget = (Q3 as f64).powi(2) / (4_f64 * (Q2 as f64).powi(2))
            * ((D1 as f64) * secret_width_vec_sq
                + 4_f64
                    * (D1 as f64)
                    * gadget_factor(T_COMPRESS, Z_COMPRESS)
                    * error_width_switch_sq);
        let e_subg = e_subg_preswitch + e_subg_gadget;
        let e_round = (2f64 + Q3 as f64 / Q1 as f64 * (Q1 % P) as f64) / 2f64;
        let threshold = Q3 / (2 * P);

        info!(
            "Switch rounding term noise bound (absolute / threshold): {} / {}",
            e_round, threshold
        );

        info!("Switch subgaussian term noise widths (absolute / threshold):");

        info!(
            "    preswitch: {:.3} / {}",
            e_subg_preswitch.sqrt(),
            threshold
        );
        info!("    gadget: {:.3} / {}", e_subg_gadget.sqrt(), threshold);
        info!("    total: {:.3} / {}", e_subg.sqrt(), threshold);

        use std::f64::consts::PI;
        let error_rate = 2_f64
            * (D3 as f64)
            * f64::exp(-PI * (0.5_f64 * (Q3 / P) as f64 - e_round).powi(2) / e_subg);

        info!("Error rate: 2^({})", error_rate.log2());
        info!("***");

        clamp(error_rate, 0_f64, 1_f64)
    }

    fn params_public_param_size() -> usize {
        let automorph_elems = floor_log(2, D1 as u64) * (T_PROJ_SHORT + T_PROJ_LONG);
        let reg_to_gsw_elems = 2 * T_RLWE_TO_GSW;
        let scal_to_vec_elems = N_VEC * T_VECTORIZE;
        let q_elem_size = D1 * ceil_log(2, Q1) / 8;

        let compress_elems = N_VEC * T_COMPRESS;
        let q2_elem_size = D1 * ceil_log(2, Q2) / 8;

        // This code assumes we implement use PRG trick to compress the randomness components of the
        // public params. Technically though, this hasn't been implemented.

        info!(
            "automorph pp: {:.3} KiB",
            (automorph_elems * q_elem_size) as f64 / 1024_f64
        );
        info!(
            "rlwe to GSW pp: {:.3} KiB",
            (reg_to_gsw_elems * q_elem_size) as f64 / 1024_f64
        );
        info!(
            "scal to vec pp: {:.3} KiB",
            (scal_to_vec_elems * q_elem_size) as f64 / 1024_f64
        );
        info!(
            "compress pp: {:.3} KiB",
            (compress_elems * q2_elem_size) as f64 / 1024_f64
        );
        return (automorph_elems + reg_to_gsw_elems + scal_to_vec_elems) * q_elem_size
            + compress_elems * q2_elem_size;
    }

    fn params_query_one_size() -> usize {
        (Self::RLWE_COUNT + Self::GSW_COUNT) * ceil_log(2, Q1) / 8
    }

    fn params_record_one_size() -> usize {
        let log_p = floor_log(2, P);
        D3 * log_p / 8
    }

    fn params_response_one_size(trunc_len: usize) -> usize {
        // Technically we can do ceil(d * (log(q2) + len * log(q1)) by packing into a single large integer.
        // But for simplicity assume each IntMod<Q1> / IntMod<Q2> is serialized individually.
        let log_q1 = ceil_log(2, Q3);
        let log_q2 = ceil_log(2, Q2);
        ((D2 as f64) * (log_q2 as f64 + (trunc_len as f64) * log_q1 as f64) / 8_f64).ceil() as usize
    }
});

respire_impl!({
    pub fn extract_ring_one(
        (_, _, s_small): &<Self as PIR>::QueryKey,
        (c_r_hat, c_m_hat_trunc): &<Self as Respire>::AnswerOneCompressed,
    ) -> <Self as Respire>::RecordPackedSmall {
        let neg_s_small_cr =
            (-&(s_small * &IntModCycloEval::from(c_r_hat))).map_ring(|r| IntModCyclo::from(r));
        let mut result = Matrix::<N_VEC, 1, IntModCyclo<D2, Q3>>::zero();
        for i in 0..c_m_hat_trunc.len() {
            for (result_coeff, neg_s_small_c0_coeff) in result[(i, 0)]
                .coeff
                .iter_mut()
                .zip(neg_s_small_cr[(i, 0)].coeff)
            {
                let numer = Q3 as u128 * u64::from(neg_s_small_c0_coeff) as u128;
                let denom = Q2 as u128;
                let div = (numer + denom / 2) / denom;
                *result_coeff = IntMod::from(div as u64);
            }
            result[(i, 0)] += &c_m_hat_trunc[i];
        }
        result.map_ring(|r| r.round_down_into())
    }

    pub fn extract_bytes_one(
        r: &<Self as Respire>::RecordPackedSmall,
    ) -> Vec<<Self as PIR>::RecordBytes> {
        let mut result = Vec::with_capacity(Self::RESPONSE_CHUNK_SIZE);
        for i in 0..N_VEC {
            for j in 0..Self::PACK_RATIO_RESPONSE {
                let record_coeffs: [IntMod<P>; D3] = r[(i, 0)]
                    .coeff
                    .iter()
                    .copied()
                    .skip(j)
                    .step_by(D2 / D3)
                    .collect_vec()
                    .try_into()
                    .unwrap();
                result.push(RecordBytesImpl {
                    it: Self::decode_record(&IntModCyclo::from(record_coeffs)),
                });
            }
        }
        result
    }

    pub fn answer_query_unpack(
        ((auto_params_short, auto_params_long), rlwe_to_gsw_params, _, _): &<Self as PIR>::PublicParams,
        ((seed_reg, vec_reg), (seed_gsw, vec_gsw)): &<Self as Respire>::QueryOne,
        _: Option<&<Self as PIR>::QueryKey>,
        time_stats: Option<&mut Stats<Duration>>,
    ) -> <Self as Respire>::QueryOneExpanded {
        let inv = <Self as Respire>::RingQ1Fast::from(mod_inverse(D1 as u64, Q1));
        let mut c_rlwes = {
            let mut c1_reg = IntModCyclo::zero();
            for (i, coeff) in vec_reg.iter().copied().enumerate() {
                c1_reg.coeff[reverse_bits_fast::<D1>(i)] = coeff;
            }
            let mut c_reg = Self::rlwe_recover_from_seeded((
                *seed_reg,
                <Self as Respire>::RingQ1Fast::from(&c1_reg),
            ));
            c_reg[(0, 0)] *= &inv;
            c_reg[(1, 0)] *= &inv;
            vec![c_reg]
        };

        let mut c_gsws = {
            let mut c1_gsw = IntModCyclo::zero();
            for (i, coeff) in vec_gsw.iter().copied().enumerate() {
                c1_gsw.coeff[reverse_bits_fast::<D1>(i)] = coeff;
            }
            let mut c_gsw = Self::rlwe_recover_from_seeded((
                *seed_gsw,
                <Self as Respire>::RingQ1Fast::from(&c1_gsw),
            ));
            c_gsw[(0, 0)] *= &inv;
            c_gsw[(1, 0)] *= &inv;
            vec![c_gsw]
        };

        assert_eq!(1 << auto_params_short.len(), D1);
        assert_eq!(1 << auto_params_long.len(), D1);

        let i0 = Instant::now();
        for (i, auto_params_short) in auto_params_short.iter().enumerate() {
            c_rlwes = Self::do_proj_iter::<T_PROJ_SHORT, Z_PROJ_SHORT>(
                i,
                c_rlwes.as_slice(),
                auto_params_short,
            );
            let denom = D1 >> (i + 1);
            c_rlwes.truncate((Self::RLWE_COUNT + denom - 1) / denom);
        }
        assert_eq!(c_rlwes.len(), Self::RLWE_COUNT);

        let i1 = Instant::now();
        for (i, auto_params_long) in auto_params_long.iter().enumerate() {
            c_gsws = Self::do_proj_iter::<T_PROJ_LONG, Z_PROJ_LONG>(
                i,
                c_gsws.as_slice(),
                auto_params_long,
            );
            let denom = D1 >> (i + 1);
            c_gsws.truncate((Self::GSW_COUNT + denom - 1) / denom);
        }
        assert_eq!(c_gsws.len(), Self::GSW_COUNT);

        let i2 = Instant::now();
        let mut c_gsws_iter = c_gsws
            .chunks_exact(T_GSW)
            .map(|cs| Self::rlwe_to_gsw(rlwe_to_gsw_params, cs));

        let c_gsws_fold = (0..Self::GSW_FOLD_COUNT)
            .map(|_| c_gsws_iter.next().unwrap())
            .collect();
        let c_gsws_rot = (0..Self::GSW_ROT_COUNT)
            .map(|_| c_gsws_iter.next().unwrap())
            .collect();
        assert_eq!(c_gsws_iter.next(), None);

        let i3 = Instant::now();

        if let Some(time_stats) = time_stats {
            time_stats.add("answer_query_expand_reg", i1 - i0);
            time_stats.add("answer_query_expand_gsw", i2 - i1);
            time_stats.add("answer_query_expand_reg_to_gsw", i3 - i2);
        }

        // TODO measure and report noise through this phase? Difficult because need to know the exact encoding (since they are not rounded to q/p)
        (c_rlwes, c_gsws_fold, c_gsws_rot)
    }

    pub fn answer_first_dim(
        db: &<Self as PIR>::Database,
        rlwes: &[<Self as Respire>::RLWEEncoding],
    ) -> Vec<<Self as Respire>::RLWEEncoding> {
        assert_eq!(rlwes.len(), Self::PACKED_DIM1_SIZE);

        // Flatten + transpose the ciphertexts
        let mut c0s: Vec<SimdVec> = Vec::with_capacity((D1 / SIMD_LANES) * Self::PACKED_DIM1_SIZE);
        let mut c1s: Vec<SimdVec> = Vec::with_capacity((D1 / SIMD_LANES) * Self::PACKED_DIM1_SIZE);

        #[cfg(not(target_feature = "avx2"))]
        for eval_idx in 0..D1 {
            for c in rlwes.iter() {
                let c0_lo = u64::from(c[(0, 0)].proj1.evals[eval_idx]);
                let c0_hi = u64::from(c[(0, 0)].proj2.evals[eval_idx]);
                c0s.push((c0_hi << 32) | c0_lo);

                let c1_lo = u64::from(c[(1, 0)].proj1.evals[eval_idx]);
                let c1_hi = u64::from(c[(1, 0)].proj2.evals[eval_idx]);
                c1s.push((c1_hi << 32) | c1_lo);
            }
        }

        #[cfg(target_feature = "avx2")]
        for eval_vec_idx in 0..(D1 / SIMD_LANES) {
            for c in rlwes.iter() {
                let mut c0_vec: SimdVec = Aligned32([0_u64; 4]);
                let mut c1_vec: SimdVec = Aligned32([0_u64; 4]);
                for lane_idx in 0..SIMD_LANES {
                    let from_idx = eval_vec_idx * SIMD_LANES + lane_idx;

                    let c0_lo = u64::from(c[(0, 0)].proj1.evals[from_idx]);
                    let c0_hi = u64::from(c[(0, 0)].proj2.evals[from_idx]);
                    c0_vec.0[lane_idx] = (c0_hi << 32) | c0_lo;

                    let c1_lo = u64::from(c[(1, 0)].proj1.evals[from_idx]);
                    let c1_hi = u64::from(c[(1, 0)].proj2.evals[from_idx]);
                    c1_vec.0[lane_idx] = (c1_hi << 32) | c1_lo;
                }
                c0s.push(c0_vec);
                c1s.push(c1_vec);
            }
        }

        // First dimension processing
        let mut result: Vec<<Self as Respire>::RLWEEncoding> = (0..Self::PACKED_DIM2_SIZE)
            .map(|_| <Self as Respire>::RLWEEncoding::zero())
            .collect();

        // Norm is at most max(Q1A, Q1B)^2 for each term
        // Add one for margin
        let reduce_every = 1 << (64 - 2 * ceil_log(2, max(Q1A, Q1B)) - 1);

        // We want to compute the sum over i of ct_i * db_(i, j).
        // Here db_(i, j) are scalars; ct_i are 2 x 1 matrices.

        #[cfg(not(target_feature = "avx2"))]
        for eval_idx in 0..D1 {
            for j in 0..Self::PACKED_DIM2_SIZE {
                let mut sum0_proj1 = 0_u64;
                let mut sum0_proj2 = 0_u64;
                let mut sum1_proj1 = 0_u64;
                let mut sum1_proj2 = 0_u64;

                for i in 0..Self::PACKED_DIM1_SIZE {
                    let lhs0 = c0s[eval_idx * Self::PACKED_DIM1_SIZE + i];
                    let lhs0_proj1 = lhs0 as u32 as u64;
                    let lhs0_proj2 = lhs0 >> 32;

                    let lhs1 = c1s[eval_idx * Self::PACKED_DIM1_SIZE + i];
                    let lhs1_proj1 = lhs1 as u32 as u64;
                    let lhs1_proj2 = lhs1 >> 32;

                    let rhs = db[eval_idx * Self::PACKED_DB_SIZE + j * Self::PACKED_DIM1_SIZE + i];
                    let rhs_proj1 = rhs as u32 as u64;
                    let rhs_proj2 = rhs >> 32;

                    sum0_proj1 += lhs0_proj1 * rhs_proj1;
                    sum0_proj2 += lhs0_proj2 * rhs_proj2;
                    sum1_proj1 += lhs1_proj1 * rhs_proj1;
                    sum1_proj2 += lhs1_proj2 * rhs_proj2;

                    if i % reduce_every == 0 || i == Self::PACKED_DIM1_SIZE - 1 {
                        sum0_proj1 %= Q1A;
                        sum0_proj2 %= Q1B;
                        sum1_proj1 %= Q1A;
                        sum1_proj2 %= Q1B;
                    }
                }

                result[j][(0, 0)].proj1.evals[eval_idx] = IntMod::from(sum0_proj1);
                result[j][(0, 0)].proj2.evals[eval_idx] = IntMod::from(sum0_proj2);
                result[j][(1, 0)].proj1.evals[eval_idx] = IntMod::from(sum1_proj1);
                result[j][(1, 0)].proj2.evals[eval_idx] = IntMod::from(sum1_proj2);
            }
        }

        #[cfg(target_feature = "avx2")]
        for eval_vec_idx in 0..(D1 / SIMD_LANES) {
            use std::arch::x86_64::*;
            unsafe {
                for j in 0..Self::PACKED_DIM2_SIZE {
                    let mut sum0_proj1 = _mm256_setzero_si256();
                    let mut sum0_proj2 = _mm256_setzero_si256();
                    let mut sum1_proj1 = _mm256_setzero_si256();
                    let mut sum1_proj2 = _mm256_setzero_si256();

                    for i in 0..Self::PACKED_DIM1_SIZE {
                        let lhs0_ptr = c0s.get_unchecked(eval_vec_idx * Self::PACKED_DIM1_SIZE + i)
                            as *const SimdVec
                            as *const __m256i;
                        let lhs1_ptr = c1s.get_unchecked(eval_vec_idx * Self::PACKED_DIM1_SIZE + i)
                            as *const SimdVec
                            as *const __m256i;
                        let rhs_ptr = db.get_unchecked(
                            eval_vec_idx * Self::PACKED_DB_SIZE + j * Self::PACKED_DIM1_SIZE + i,
                        ) as *const SimdVec as *const __m256i;

                        let lhs0_proj1 = _mm256_load_si256(lhs0_ptr);
                        let lhs0_proj2 = _mm256_srli_epi64::<32>(lhs0_proj1);
                        let lhs1_proj1 = _mm256_load_si256(lhs1_ptr);
                        let lhs1_proj2 = _mm256_srli_epi64::<32>(lhs1_proj1);
                        let rhs_proj1 = _mm256_load_si256(rhs_ptr);
                        let rhs_proj2 = _mm256_srli_epi64::<32>(rhs_proj1);

                        sum0_proj1 =
                            _mm256_add_epi64(sum0_proj1, _mm256_mul_epu32(lhs0_proj1, rhs_proj1));
                        sum0_proj2 =
                            _mm256_add_epi64(sum0_proj2, _mm256_mul_epu32(lhs0_proj2, rhs_proj2));
                        sum1_proj1 =
                            _mm256_add_epi64(sum1_proj1, _mm256_mul_epu32(lhs1_proj1, rhs_proj1));
                        sum1_proj2 =
                            _mm256_add_epi64(sum1_proj2, _mm256_mul_epu32(lhs1_proj2, rhs_proj2));

                        if i % reduce_every == 0 || i == Self::PACKED_DIM1_SIZE - 1 {
                            let mut tmp0_proj1: SimdVec = Aligned32([0_u64; 4]);
                            let mut tmp0_proj2: SimdVec = Aligned32([0_u64; 4]);
                            let mut tmp1_proj1: SimdVec = Aligned32([0_u64; 4]);
                            let mut tmp1_proj2: SimdVec = Aligned32([0_u64; 4]);
                            _mm256_store_si256(
                                &mut tmp0_proj1 as *mut SimdVec as *mut __m256i,
                                sum0_proj1,
                            );
                            _mm256_store_si256(
                                &mut tmp0_proj2 as *mut SimdVec as *mut __m256i,
                                sum0_proj2,
                            );
                            _mm256_store_si256(
                                &mut tmp1_proj1 as *mut SimdVec as *mut __m256i,
                                sum1_proj1,
                            );
                            _mm256_store_si256(
                                &mut tmp1_proj2 as *mut SimdVec as *mut __m256i,
                                sum1_proj2,
                            );
                            for lane in 0..SIMD_LANES {
                                tmp0_proj1.0[lane] %= Q1A;
                                tmp0_proj2.0[lane] %= Q1B;
                                tmp1_proj1.0[lane] %= Q1A;
                                tmp1_proj2.0[lane] %= Q1B;
                            }
                            sum0_proj1 =
                                _mm256_load_si256(&tmp0_proj1 as *const SimdVec as *const __m256i);
                            sum0_proj2 =
                                _mm256_load_si256(&tmp0_proj2 as *const SimdVec as *const __m256i);
                            sum1_proj1 =
                                _mm256_load_si256(&tmp1_proj1 as *const SimdVec as *const __m256i);
                            sum1_proj2 =
                                _mm256_load_si256(&tmp1_proj2 as *const SimdVec as *const __m256i);
                        }
                    }

                    let sum0_proj1_ptr = result[j][(0, 0)]
                        .proj1
                        .evals
                        .get_unchecked_mut(eval_vec_idx * SIMD_LANES)
                        as *mut IntMod<Q1A>
                        as *mut __m256i;
                    let sum0_proj2_ptr = result[j][(0, 0)]
                        .proj2
                        .evals
                        .get_unchecked_mut(eval_vec_idx * SIMD_LANES)
                        as *mut IntMod<Q1B>
                        as *mut __m256i;
                    let sum1_proj1_ptr = result[j][(1, 0)]
                        .proj1
                        .evals
                        .get_unchecked_mut(eval_vec_idx * SIMD_LANES)
                        as *mut IntMod<Q1A>
                        as *mut __m256i;
                    let sum1_proj2_ptr = result[j][(1, 0)]
                        .proj2
                        .evals
                        .get_unchecked_mut(eval_vec_idx * SIMD_LANES)
                        as *mut IntMod<Q1B>
                        as *mut __m256i;
                    _mm256_store_si256(sum0_proj1_ptr, sum0_proj1);
                    _mm256_store_si256(sum0_proj2_ptr, sum0_proj2);
                    _mm256_store_si256(sum1_proj1_ptr, sum1_proj1);
                    _mm256_store_si256(sum1_proj2_ptr, sum1_proj2);
                }
            }
        }

        result
    }

    pub fn answer_fold(
        first_dim_folded: Vec<<Self as Respire>::RLWEEncoding>,
        gsws: &[<Self as Respire>::GSWEncoding],
    ) -> <Self as Respire>::RLWEEncoding {
        assert_eq!(gsws.len(), Self::NU2);
        let fold_size: usize = 2usize.pow(Self::NU2 as u32);

        let mut curr = first_dim_folded;
        let mut curr_size = fold_size;
        for gsw_idx in 0..NU2 {
            curr.truncate(curr_size);
            for fold_idx in 0..curr_size / 2 {
                curr[fold_idx] = Self::select_hom(
                    &curr[fold_idx],
                    &curr[curr_size / 2 + fold_idx],
                    &gsws[gsw_idx],
                );
            }
            curr_size /= 2;
        }
        curr.remove(0)
    }

    pub fn answer_rotate(
        ct: &<Self as Respire>::RLWEEncoding,
        gsws_rot: &[<Self as Respire>::GSWEncoding],
    ) -> <Self as Respire>::RLWEEncoding {
        assert_eq!(gsws_rot.len(), Self::NU3);
        let mut ct_curr = ct.clone();
        for (iter_num, gsw) in gsws_rot.iter().enumerate() {
            let rot = 1 << (Self::NU3 - 1 - iter_num);
            ct_curr =
                Self::select_hom(&ct_curr, &Self::rlwe_mul_x_pow(&ct_curr, 2 * D1 - rot), gsw);
        }
        ct_curr
    }

    pub fn answer_project(
        ((_, auto_params_long), _, _, _): &<Self as PIR>::PublicParams,
        ct: &<Self as Respire>::RLWEEncoding,
    ) -> <Self as Respire>::RLWEEncoding {
        let mut ct_curr = ct.clone();
        let num_proj = Self::NU3;
        let inv = <Self as Respire>::RingQ1Fast::from(mod_inverse(
            2_usize.pow(num_proj as u32) as u64,
            Q1,
        ));
        ct_curr[(0, 0)] *= &inv;
        ct_curr[(1, 0)] *= &inv;

        for (iter_num, auto_params_long) in auto_params_long.iter().enumerate().take(num_proj) {
            ct_curr = Self::do_proj_iter_one::<T_PROJ_LONG, Z_PROJ_LONG>(
                iter_num,
                &ct_curr,
                auto_params_long,
            );
        }
        ct_curr
    }

    pub fn encode_setup() -> <Self as Respire>::RingQ1Fast {
        let mut rng = ChaCha20Rng::from_entropy();
        let mut result = <Self as Respire>::RingQ1::zero();
        for coeff in result.coeff.iter_mut() {
            *coeff = IntMod::from(rng.gen_range(-(SECRET_BOUND as i64)..(SECRET_BOUND as i64)));
        }
        <Self as Respire>::RingQ1Fast::from(&result)
    }

    pub fn encode_vec_setup() -> <Self as Respire>::VecEncodingSecret {
        let mut rng = ChaCha20Rng::from_entropy();
        let mut result = Matrix::zero();
        for i in 0..N_VEC {
            result[(i, 0)] = <Self as Respire>::RingQ1Fast::from(
                &<Self as Respire>::RingQ1::rand_discrete_gaussian::<_, SECRET_WIDTH_VEC_MILLIONTHS>(
                    &mut rng,
                ),
            );
        }
        result
    }

    pub fn encode_rlwe(
        s_encode: &<Self as Respire>::EncodingSecret,
        mu: &<Self as Respire>::RingQ1,
    ) -> <Self as Respire>::RLWEEncoding {
        let mut rng = ChaCha20Rng::from_entropy();
        let mut c = Matrix::zero();
        c[(0, 0)] = <Self as Respire>::RingQ1Fast::rand_uniform(&mut rng);
        let e = <Self as Respire>::RingQ1Fast::from(
            &<Self as Respire>::RingQ1::rand_discrete_gaussian::<_, ERROR_WIDTH_MILLIONTHS>(
                &mut rng,
            ),
        );
        let mut c1 = &c[(0, 0)] * s_encode;
        c1 += &e;
        c1 += &<Self as Respire>::RingQ1Fast::from(mu);
        c[(1, 0)] = c1;
        c
    }

    pub fn encode_rlwe_seeded(
        s_encode: &<Self as Respire>::EncodingSecret,
        mu: &<Self as Respire>::RingQ1,
    ) -> <Self as Respire>::RLWEEncodingSeeded {
        let mut rng = ChaCha20Rng::from_entropy();
        let seed = rng.gen();
        let c0 = {
            let mut seeded_rng = ChaCha20Rng::from_seed(seed);
            <Self as Respire>::RingQ1Fast::rand_uniform(&mut seeded_rng)
        };
        let e = <Self as Respire>::RingQ1Fast::from(
            &<Self as Respire>::RingQ1::rand_discrete_gaussian::<_, ERROR_WIDTH_MILLIONTHS>(
                &mut rng,
            ),
        );
        let mut c1 = &c0 * s_encode;
        c1 += &e;
        c1 += &<Self as Respire>::RingQ1Fast::from(mu);
        (seed, c1)
    }

    pub fn rlwe_recover_from_seeded(
        (seed, c1): <Self as Respire>::RLWEEncodingSeeded,
    ) -> <Self as Respire>::RLWEEncoding {
        let c0 = {
            let mut seeded_rng = ChaCha20Rng::from_seed(seed);
            <Self as Respire>::RingQ1Fast::rand_uniform(&mut seeded_rng)
        };
        let mut result = Matrix::zero();
        result[(0, 0)] = c0;
        result[(1, 0)] = c1;
        result
    }

    pub fn rand_discrete_gaussian_matrix<
        const WIDTH_MILLIONTHS: u64,
        const N: usize,
        const M: usize,
        T: Rng,
    >(
        rng: &mut T,
    ) -> Matrix<N, M, <Self as Respire>::RingQ1Fast> {
        let mut result = Matrix::zero();
        for r in 0..N {
            for c in 0..M {
                result[(r, c)] = <Self as Respire>::RingQ1Fast::from(
                    &<Self as Respire>::RingQ1::rand_discrete_gaussian::<_, WIDTH_MILLIONTHS>(rng),
                );
            }
        }
        result
    }

    pub fn encode_vec_rlwe(
        s_vec: &<Self as Respire>::VecEncodingSecret,
        mu: &Matrix<N_VEC, 1, <Self as Respire>::RingQ1>,
    ) -> <Self as Respire>::VecRLWEEncoding {
        let mut rng = ChaCha20Rng::from_entropy();
        let c_r = <Self as Respire>::RingQ1Fast::rand_uniform(&mut rng);
        let e = Self::rand_discrete_gaussian_matrix::<ERROR_WIDTH_VEC_MILLIONTHS, N_VEC, 1, _>(
            &mut rng,
        );
        let mut c_m = s_vec * &c_r;
        c_m += &e;
        c_m += &mu.map_ring(|r| <Self as Respire>::RingQ1Fast::from(r));
        (c_r, c_m)
    }

    pub fn decode_rlwe(
        s_encode: &<Self as Respire>::EncodingSecret,
        c: &<Self as Respire>::RLWEEncoding,
    ) -> <Self as Respire>::RingQ1 {
        <Self as Respire>::RingQ1::from(&(&c[(1, 0)] - &(&c[(0, 0)] * s_encode)))
    }

    pub fn decode_vec_rlwe(
        s_vec: &<Self as Respire>::VecEncodingSecret,
        (c_r, c_m): &<Self as Respire>::VecRLWEEncoding,
    ) -> Matrix<N_VEC, 1, <Self as Respire>::RingQ1> {
        (c_m - &(s_vec * c_r)).map_ring(|r| <Self as Respire>::RingQ1::from(r))
    }

    pub fn encode_gsw(
        s_encode: &<Self as Respire>::EncodingSecret,
        mu: &<Self as Respire>::RingQ1,
    ) -> <Self as Respire>::GSWEncoding {
        let mut rng = ChaCha20Rng::from_entropy();
        let a_t: Matrix<1, M_GSW, <Self as Respire>::RingQ1Fast> = Matrix::rand_uniform(&mut rng);
        let e_mat =
            Self::rand_discrete_gaussian_matrix::<ERROR_WIDTH_MILLIONTHS, 1, M_GSW, _>(&mut rng);
        let c_mat: Matrix<2, M_GSW, <Self as Respire>::RingQ1Fast> =
            &Matrix::stack(&a_t, &(&(&a_t * s_encode) + &e_mat))
                + &(&build_gadget::<<Self as Respire>::RingQ1Fast, 2, M_GSW, Z_GSW, T_GSW>()
                    * &<Self as Respire>::RingQ1Fast::from(mu));
        c_mat
    }

    pub fn decode_gsw_scaled(
        s_encode: &<Self as Respire>::EncodingSecret,
        c: &<Self as Respire>::GSWEncoding,
        scale: &<Self as Respire>::RingQ1Fast,
    ) -> <Self as Respire>::RingQ1 {
        let scaled_ident = &Matrix::<2, 2, <Self as Respire>::RingQ1Fast>::identity() * scale;
        let mut s_t = Matrix::<1, 2, <Self as Respire>::RingQ1Fast>::zero();
        s_t[(0, 0)] = (-s_encode).clone();
        s_t[(0, 1)] = <Self as Respire>::RingQ1Fast::one();
        let result_q_fast_mat = &(&s_t * c)
            * &gadget_inverse::<<Self as Respire>::RingQ1Fast, 2, M_GSW, 2, Z_GSW, T_GSW>(
                &scaled_ident,
            );
        let result_q = <Self as Respire>::RingQ1::from(&result_q_fast_mat[(0, 1)]);
        <Self as Respire>::RingQ1::from(result_q)
    }

    pub fn rlwe_sub_hom(
        lhs: &<Self as Respire>::RLWEEncoding,
        rhs: &<Self as Respire>::RLWEEncoding,
    ) -> <Self as Respire>::RLWEEncoding {
        lhs - rhs
    }

    pub fn hybrid_mul_hom(
        rlwe: &<Self as Respire>::RLWEEncoding,
        gsw: &<Self as Respire>::GSWEncoding,
    ) -> <Self as Respire>::RLWEEncoding {
        gsw * &gadget_inverse::<<Self as Respire>::RingQ1Fast, 2, M_GSW, 1, Z_GSW, T_GSW>(rlwe)
    }

    pub fn select_hom(
        c0: &<Self as Respire>::RLWEEncoding,
        c1: &<Self as Respire>::RLWEEncoding,
        b: &<Self as Respire>::GSWEncoding,
    ) -> <Self as Respire>::RLWEEncoding {
        let c1_sub_c0 = Self::rlwe_sub_hom(c1, c0);
        let c1_sub_c0_mul_b = Self::hybrid_mul_hom(&c1_sub_c0, b);
        c0 + &c1_sub_c0_mul_b
    }

    pub fn rlwe_mul_x_pow(
        c: &<Self as Respire>::RLWEEncoding,
        k: usize,
    ) -> <Self as Respire>::RLWEEncoding {
        c.map_ring(|x| x.mul_x_pow(k))
    }

    pub fn gsw_mul_x_pow(
        c: &<Self as Respire>::GSWEncoding,
        k: usize,
    ) -> <Self as Respire>::GSWEncoding {
        c.map_ring(|x| x.mul_x_pow(k))
    }

    pub fn auto_setup<const LEN: usize, const BASE: u64>(
        tau_power: usize,
        s_encode: &<Self as Respire>::RingQ1Fast,
    ) -> <Self as Respire>::AutoParams<LEN> {
        let mut rng = ChaCha20Rng::from_entropy();
        let a_t: Matrix<1, LEN, <Self as Respire>::RingQ1Fast> = Matrix::rand_uniform(&mut rng);
        let e_t =
            Self::rand_discrete_gaussian_matrix::<ERROR_WIDTH_MILLIONTHS, 1, LEN, _>(&mut rng);
        let mut bottom = &a_t * s_encode;
        bottom += &e_t;
        bottom -= &(&build_gadget::<<Self as Respire>::RingQ1Fast, 1, LEN, BASE, LEN>()
            * &s_encode.auto(tau_power));
        (Matrix::stack(&a_t, &bottom), tau_power)
    }

    pub fn auto_hom<const LEN: usize, const BASE: u64>(
        (w_mat, tau_power): &<Self as Respire>::AutoParams<LEN>,
        c: &<Self as Respire>::RLWEEncoding,
    ) -> <Self as Respire>::RLWEEncoding {
        let c0 = &c[(0, 0)];
        let c1 = &c[(1, 0)];
        let mut g_inv_tau_c0 = Matrix::<LEN, 1, <Self as Respire>::RingQ1Fast>::zero();

        <<Self as Respire>::RingQ1Fast as RingElementDecomposable<BASE, LEN>>::decompose_into_mat(
            &c0.auto(*tau_power),
            &mut g_inv_tau_c0,
            0,
            0,
        );
        let mut result = w_mat * &g_inv_tau_c0;

        result[(1, 0)] += &c1.auto(*tau_power);
        result
    }

    ///
    /// Perform a single iteration of the coefficient expansion algorithm, doubling the length of the
    /// input ciphertexts.
    ///
    /// # Parameters
    /// * `which_iter`: the zero-indexed iteration number; the ciphertexts are assumed to be
    ///   encryptions of plaintexts that only have coefficients of degree divisible `2^which_iter`.
    /// * `cts`: the input ciphertexts
    /// * `auto_params`: the automorphism public params, which should have power equal to `D1 / 2^which_iter + 1`
    ///
    pub fn do_proj_iter<const LEN: usize, const BASE: u64>(
        which_iter: usize,
        cts: &[<Self as Respire>::RLWEEncoding],
        auto_params: &<Self as Respire>::AutoParams<LEN>,
    ) -> Vec<<Self as Respire>::RLWEEncoding> {
        assert_eq!(auto_params.1, (D1 >> which_iter) + 1);
        let len = cts.len();
        let mut cts_new = Vec::with_capacity(2 * len);
        cts_new.resize(2 * len, Matrix::zero());
        for (j, ct) in cts.iter().enumerate() {
            let shift_exp = 1 << which_iter;
            let shift_auto_exp = (shift_exp * auto_params.1) % (2 * D1);

            let ct_shifted = Self::rlwe_mul_x_pow(ct, 2 * D1 - shift_exp);
            let ct_auto = Self::auto_hom::<LEN, BASE>(auto_params, ct);
            let ct_auto_shifted = Self::rlwe_mul_x_pow(&ct_auto, 2 * D1 - shift_auto_exp);

            cts_new[2 * j] = ct + &ct_auto;
            cts_new[2 * j + 1] = &ct_shifted + &ct_auto_shifted;
        }
        cts_new
    }

    ///
    /// Same as do_proj_iter, but only does one side of the projection
    ///
    pub fn do_proj_iter_one<const LEN: usize, const BASE: u64>(
        which_iter: usize,
        ct: &<Self as Respire>::RLWEEncoding,
        auto_params: &<Self as Respire>::AutoParams<LEN>,
    ) -> <Self as Respire>::RLWEEncoding {
        assert_eq!(auto_params.1, (D1 >> which_iter) + 1);
        let ct_auto = Self::auto_hom::<LEN, BASE>(auto_params, ct);
        ct + &ct_auto
    }

    pub fn rlwe_to_gsw_setup(
        s_encode: &<Self as Respire>::EncodingSecret,
    ) -> <Self as Respire>::RLWEToGSWParams {
        let mut rng = ChaCha20Rng::from_entropy();
        let a_t = Matrix::<1, M_RLWE_TO_GSW, <Self as Respire>::RingQ1Fast>::rand_uniform(&mut rng);
        let e_mat =
            Self::rand_discrete_gaussian_matrix::<ERROR_WIDTH_MILLIONTHS, 1, M_RLWE_TO_GSW, _>(
                &mut rng,
            );
        let mut bottom = &a_t * s_encode;
        bottom += &e_mat;
        let g_vec = build_gadget::<
            <Self as Respire>::RingQ1Fast,
            1,
            T_RLWE_TO_GSW,
            Z_RLWE_TO_GSW,
            T_RLWE_TO_GSW,
        >();
        let mut s_encode_tensor_g =
            Matrix::<1, M_RLWE_TO_GSW, <Self as Respire>::RingQ1Fast>::zero();
        s_encode_tensor_g.copy_into(&g_vec, 0, T_RLWE_TO_GSW);
        s_encode_tensor_g.copy_into(&(&g_vec * &(-s_encode)), 0, 0);
        bottom -= &(&s_encode_tensor_g * s_encode);

        Matrix::stack(&a_t, &bottom)
    }

    pub fn rlwe_to_gsw(
        v_mat: &<Self as Respire>::RLWEToGSWParams,
        cs: &[<Self as Respire>::RLWEEncoding],
    ) -> <Self as Respire>::GSWEncoding {
        let mut result = Matrix::<2, M_GSW, <Self as Respire>::RingQ1Fast>::zero();
        let mut c_hat = Matrix::<2, T_GSW, <Self as Respire>::RingQ1Fast>::zero();
        for (i, ci) in cs.iter().enumerate() {
            c_hat.copy_into(ci, 0, i);
        }
        let g_inv_c_hat = gadget_inverse::<
            <Self as Respire>::RingQ1Fast,
            2,
            M_RLWE_TO_GSW,
            T_GSW,
            Z_RLWE_TO_GSW,
            T_RLWE_TO_GSW,
        >(&c_hat);
        let v_g_inv_c_hat = v_mat * &g_inv_c_hat;
        result.copy_into(&v_g_inv_c_hat, 0, 0);
        for (i, ci) in cs.iter().enumerate() {
            result.copy_into(ci, 0, T_GSW + i);
        }

        // No permutation needed for scalar RLWE encoding
        result
    }

    pub fn compress_setup(
        s_from: &<Self as Respire>::VecEncodingSecretQ2,
        s_to: &<Self as Respire>::VecEncodingSecretQ2,
    ) -> <Self as Respire>::CompressParams {
        let mut rng = ChaCha20Rng::from_entropy();
        let a_t = Matrix::<1, T_COMPRESS, IntModCycloEval<D1, Q2>>::rand_uniform(&mut rng);
        let e_mat = Matrix::<N_VEC, T_COMPRESS, IntModCycloEval<D1, Q2>>::rand_discrete_gaussian::<
            _,
            ERROR_WIDTH_COMPRESS_MILLIONTHS,
        >(&mut rng);
        let mut b_mat = &(-s_from)
            * &build_gadget::<IntModCycloEval<D1, Q2>, 1, T_COMPRESS, Z_COMPRESS, T_COMPRESS>();
        b_mat += &(s_to * &a_t);
        b_mat += &e_mat;
        (a_t, b_mat)
    }

    pub fn vectorize_setup(
        s_scal: &<Self as Respire>::EncodingSecret,
        s_vec: &<Self as Respire>::VecEncodingSecret,
    ) -> <Self as Respire>::VectorizeParams {
        let mut rng = ChaCha20Rng::from_entropy();
        let mut result = Vec::with_capacity(N_VEC);
        for i in 0..N_VEC {
            let mut unit = Matrix::<N_VEC, 1, <Self as Respire>::RingQ1Fast>::zero();
            unit[(i, 0)] = <Self as Respire>::RingQ1Fast::one();
            let unit = unit;

            let a_t =
                Matrix::<1, T_VECTORIZE, <Self as Respire>::RingQ1Fast>::rand_uniform(&mut rng);
            let e_mat = Self::rand_discrete_gaussian_matrix::<
                ERROR_WIDTH_VEC_MILLIONTHS,
                N_VEC,
                T_VECTORIZE,
                _,
            >(&mut rng);
            let mut bottom = s_vec * &a_t;
            bottom += &e_mat;
            let embedding =
                &(&unit * s_scal) * &build_gadget::<_, 1, T_VECTORIZE, Z_VECTORIZE, T_VECTORIZE>();
            bottom -= &embedding;
            result.push((a_t, bottom));
        }
        result
    }

    pub fn scal_to_vec(
        s_scal_to_vec: &<Self as Respire>::VectorizeParams,
        cs: &[<Self as Respire>::RLWEEncoding; N_VEC],
    ) -> <Self as Respire>::VecRLWEEncoding {
        let mut result_rand = <Self as Respire>::RingQ1Fast::zero();
        let mut result_embed = Matrix::<N_VEC, 1, <Self as Respire>::RingQ1Fast>::zero();
        for (i, c) in cs.iter().enumerate() {
            let c0 = &c[(0, 0)];
            let c1 = &c[(1, 0)];
            let g_inv = gadget_inverse_scalar::<_, Z_VECTORIZE, T_VECTORIZE>(c0);
            result_rand += &(&s_scal_to_vec[i].0 * &g_inv)[(0, 0)];
            result_embed += &(&s_scal_to_vec[i].1 * &g_inv);
            result_embed[(i, 0)] += c1;
        }
        (result_rand, result_embed)
    }

    pub fn encode_record(bytes: &RecordBytesImpl<BYTES_PER_RECORD>) -> <Self as Respire>::Record {
        let bit_iter = BitSlice::<u8, Msb0>::from_slice(&bytes.it);
        let p_bits = floor_log(2, P);
        let coeff = bit_iter
            .chunks(p_bits)
            .map(|c| IntMod::<P>::from(c.iter().fold(0, |acc, b| 2 * acc + *b as u64)))
            .collect_vec();
        let coeff_slice: [IntMod<P>; D3] = coeff.try_into().unwrap();
        IntModCyclo::from(coeff_slice)
    }

    pub fn decode_record(record: &<Self as Respire>::Record) -> [u8; BYTES_PER_RECORD] {
        let p_bits = floor_log(2, P);
        let bit_iter = record.coeff.iter().flat_map(|x| {
            u64::from(*x)
                .into_bitarray::<Msb0>()
                .into_iter()
                .skip(64 - p_bits)
                .take(p_bits)
        });
        let bytes = bit_iter
            .chunks(8)
            .into_iter()
            .map(|c| c.fold(0, |acc, b| 2 * acc + b as u8))
            .collect_vec();
        bytes.try_into().unwrap()
    }

    pub fn noise_variance(
        s_scal: &<Self as Respire>::EncodingSecret,
        c: &<Self as Respire>::RLWEEncoding,
    ) -> f64 {
        let decoded: <Self as Respire>::RingQ1 = Self::decode_rlwe(&s_scal, c);
        let message: <Self as Respire>::RingP = decoded.round_down_into();
        let noise: <Self as Respire>::RingQ1 = &decoded - &message.scale_up_into();

        let mut sum = 0_f64;
        let mut samples = 0_usize;

        for e in noise.coeff.iter() {
            let e_sq = (e.norm() as f64) * (e.norm() as f64);
            sum += e_sq;
            samples += 1;
        }

        sum / samples as f64
    }

    fn variance_to_subgaussian_bits(x: f64) -> f64 {
        // Subgaussian widths = sqrt(2*pi) * (standard deviation)
        (x * (2f64 * PI)).log2() / 2f64
    }

    pub fn noise_subgaussian_bits(
        s_scal: &<Self as Respire>::EncodingSecret,
        c: &<Self as Respire>::RLWEEncoding,
    ) -> f64 {
        Self::variance_to_subgaussian_bits(Self::noise_variance(s_scal, c))
    }

    pub fn noise_subgaussian_bits_vec(
        s_vec: &<Self as Respire>::VecEncodingSecret,
        (cr, cm): &<Self as Respire>::VecRLWEEncoding,
    ) -> f64 {
        let mut total = 0_f64;
        for i in 0..N_VEC {
            let mut fake_ct = Matrix::zero();
            fake_ct[(0, 0)] = cr.clone();
            fake_ct[(1, 0)] = cm[(i, 0)].clone();
            let fake_s = s_vec[(i, 0)].clone();
            total += Self::noise_variance(&fake_s, &fake_ct);
        }
        Self::variance_to_subgaussian_bits(total / N_VEC as f64)
    }

    pub fn params_query_size() -> usize {
        Self::BATCH_SIZE * Self::params_query_one_size()
    }

    pub fn params_record_size() -> usize {
        Self::BATCH_SIZE * Self::params_record_one_size()
    }

    ///
    /// size, number of full vectors, remainder size
    ///
    pub fn params_response_info() -> (usize, usize, usize) {
        let num_ring_elem = Self::BATCH_SIZE.div_ceil(Self::PACK_RATIO_RESPONSE);
        let num_full_vecs = num_ring_elem / Self::N_VEC;
        let num_rem = num_ring_elem % Self::N_VEC;

        let full_vec_size = Self::params_response_one_size(Self::N_VEC);
        let rem_vec_size = if num_rem > 0 {
            Self::params_response_one_size(num_rem)
        } else {
            0
        };
        (
            num_full_vecs * full_vec_size + rem_vec_size,
            num_full_vecs,
            num_rem,
        )
    }

    pub fn params_rate() -> f64 {
        (Self::params_record_size() as f64) / (Self::params_response_info().0 as f64)
    }
});
