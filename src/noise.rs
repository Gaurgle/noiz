use rand::Rng;

/// White noise — uniform random samples
pub struct WhiteNoise;

impl WhiteNoise {
    pub fn sample(rng: &mut impl Rng) -> f32 {
        rng.gen_range(-1.0..1.0)
    }
}

/// Pink noise — Voss-McCartney algorithm (16 octaves)
pub struct PinkNoise {
    rows: [f32; 16],
    running_sum: f32,
    index: u32,
}

impl PinkNoise {
    pub fn new() -> Self {
        Self {
            rows: [0.0; 16],
            running_sum: 0.0,
            index: 0,
        }
    }

    pub fn sample(&mut self, rng: &mut impl Rng) -> f32 {
        let num_rows = 16;
        self.index = self.index.wrapping_add(1);

        // Find which rows to update based on trailing zeros
        let changed = (self.index).trailing_zeros().min(num_rows - 1);
        let row = changed as usize;

        self.running_sum -= self.rows[row];
        let new_val: f32 = rng.gen_range(-1.0..1.0);
        self.running_sum += new_val;
        self.rows[row] = new_val;

        // Normalize (16 rows contribute, plus one white sample)
        let white: f32 = rng.gen_range(-1.0..1.0);
        (self.running_sum + white) / (num_rows as f32 + 1.0)
    }
}

/// Brown noise — integrated white noise with drift limiting
pub struct BrownNoise {
    value: f32,
    lp: f32, // single-pole lowpass state
}

impl BrownNoise {
    pub fn new() -> Self {
        Self { value: 0.0, lp: 0.0 }
    }

    pub fn sample(&mut self, rng: &mut impl Rng) -> f32 {
        let step: f32 = rng.gen_range(-0.12..0.12);
        self.value = (self.value + step).clamp(-0.8, 0.8);
        // Single-pole lowpass to cut top-end. Lower coeff = darker
        self.lp += 0.06 * (self.value - self.lp);
        self.lp
    }
}

/// Multi-source noise generator
pub struct NoiseGen {
    pub pink_l: PinkNoise,
    pub pink_r: PinkNoise,
    pub brown_l: BrownNoise,
    pub brown_r: BrownNoise,
}

impl NoiseGen {
    pub fn new() -> Self {
        Self {
            pink_l: PinkNoise::new(),
            pink_r: PinkNoise::new(),
            brown_l: BrownNoise::new(),
            brown_r: BrownNoise::new(),
        }
    }

    /// Generate a stereo sample (L, R) for the given noise mix
    pub fn sample(&mut self, rng: &mut impl Rng, mix: &NoiseMix) -> (f32, f32) {
        let mut l = 0.0f32;
        let mut r = 0.0f32;

        if mix.white > 0.0 {
            // White noise has more perceived energy than pink/brown, scale down to balance
            l += WhiteNoise::sample(rng) * mix.white * 0.35;
            r += WhiteNoise::sample(rng) * mix.white * 0.35;
        }
        if mix.pink > 0.0 {
            l += self.pink_l.sample(rng) * mix.pink;
            r += self.pink_r.sample(rng) * mix.pink;
        }
        if mix.brown > 0.0 {
            l += self.brown_l.sample(rng) * mix.brown;
            r += self.brown_r.sample(rng) * mix.brown;
        }

        (l, r)
    }
}

#[derive(Clone, Copy)]
pub struct NoiseMix {
    pub white: f32,
    pub pink: f32,
    pub brown: f32,
}

impl NoiseMix {
    pub fn white() -> Self {
        Self { white: 1.0, pink: 0.0, brown: 0.0 }
    }
    pub fn pink() -> Self {
        Self { white: 0.0, pink: 1.0, brown: 0.0 }
    }
    pub fn brown() -> Self {
        Self { white: 0.0, pink: 0.0, brown: 1.0 }
    }

    /// Lerp between two mixes for crossfading
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self {
            white: a.white * (1.0 - t) + b.white * t,
            pink: a.pink * (1.0 - t) + b.pink * t,
            brown: a.brown * (1.0 - t) + b.brown * t,
        }
    }
}

