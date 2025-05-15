use crate::pir::respire_harness::FactoryParams;

impl FactoryParams {
    // P=4 D3=128:
    // (128 * floor_log(2, 4)) / 8 = (128 * 2) / 8 = 32 bytes
    pub const fn single_record_32(nu1: usize, nu2: usize) -> Self {
        FactoryParams {
            BATCH_SIZE: 1,
            N_VEC: 1,
            P: 4,
            D3: 128,
            NU1: nu1,
            NU2: nu2,
            Q3: 16 * 16,
            Q2: 16760833,
            D2: 512,
            WIDTH_COMPRESS_MILLIONTHS: 253_600_000,
            T_PROJ_SHORT: 4,
            T_PROJ_LONG: 20,
        }
    }

    pub const fn batch_32(batch_size: usize, n_vec: usize, nu1: usize, nu2: usize) -> Self {
        FactoryParams {
            BATCH_SIZE: batch_size,
            N_VEC: n_vec,
            P: 4,
            D3: 128,
            NU1: nu1,
            NU2: nu2,
            Q3: 8 * 16,
            Q2: 249857,
            D2: 2048,
            WIDTH_COMPRESS_MILLIONTHS: 2_001_000,
            T_PROJ_SHORT: 4,
            T_PROJ_LONG: 20,
        }
    }
}
