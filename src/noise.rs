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

// --- Rain ---

struct Raindrop {
    energy: f32,
    decay: f32,    // per-sample decay rate
    bp_state: f32, // bandpass filter state
    bp_freq: f32,  // normalized frequency for this drop
}

impl Raindrop {
    fn new(rng: &mut impl Rng, sample_rate: f32) -> Self {
        // Random pitch: higher = sharper tick, lower = soft plop
        let freq = rng.gen_range(800.0..4000.0);
        // Random decay: 2-8ms
        let decay_ms = rng.gen_range(2.0..8.0);
        let decay = (-1.0 / (sample_rate * decay_ms / 1000.0)).exp();
        Self {
            energy: rng.gen_range(0.3..1.0),
            decay,
            bp_state: 0.0,
            bp_freq: (freq / sample_rate * std::f32::consts::TAU).min(0.8),
        }
    }

    fn sample(&mut self, rng: &mut impl Rng) -> f32 {
        if self.energy < 0.001 {
            return 0.0;
        }
        let noise = rng.gen_range(-1.0..1.0);
        // Simple bandpass via resonant filter
        self.bp_state += self.bp_freq * (noise * self.energy - self.bp_state);
        self.energy *= self.decay;
        self.bp_state
    }

    fn is_dead(&self) -> bool {
        self.energy < 0.001
    }
}

/// Simple comb filter reverb (one tap per channel)
pub struct CombReverb {
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    feedback: f32,
    lp_l: f32, // lowpass on feedback for darker reverb tail
    lp_r: f32,
}

impl CombReverb {
    pub fn new(sample_rate: f32, delay_ms: f32, feedback: f32) -> Self {
        let size = (sample_rate * delay_ms / 1000.0) as usize;
        Self {
            buffer_l: vec![0.0; size],
            buffer_r: vec![0.0; size],
            write_pos: 0,
            feedback,
            lp_l: 0.0,
            lp_r: 0.0,
        }
    }

    /// Second tap for wider stereo (offset from main delay)
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let len = self.buffer_l.len();
        if len == 0 {
            return (l, r);
        }

        let read_pos = self.write_pos;
        let delayed_l = self.buffer_l[read_pos];
        let delayed_r = self.buffer_r[read_pos];

        // Lowpass the feedback for darker tail
        self.lp_l += 0.3 * (delayed_l - self.lp_l);
        self.lp_r += 0.3 * (delayed_r - self.lp_r);

        // Write input + feedback into buffer
        self.buffer_l[self.write_pos] = l + self.lp_l * self.feedback;
        self.buffer_r[self.write_pos] = r + self.lp_r * self.feedback;

        self.write_pos = (self.write_pos + 1) % len;

        // Mix: dry + wet (cross-feed slightly for width)
        let wet_l = delayed_l * 0.6 + delayed_r * 0.15;
        let wet_r = delayed_r * 0.6 + delayed_l * 0.15;

        (l + wet_l, r + wet_r)
    }
}

/// Rain generator — ambient noise + random droplets + reverb
pub struct RainGen {
    droplets: Vec<Raindrop>,
    density: f32,         // drops per second
    base_l: PinkNoise,
    base_r: PinkNoise,
    base_brown_l: BrownNoise,
    base_brown_r: BrownNoise,
    reverb: CombReverb,
    sample_rate: f32,
    // Slow density modulation for "gusts"
    gust_phase: f32,
}

impl RainGen {
    pub fn new(sample_rate: f32, density: f32) -> Self {
        Self {
            droplets: Vec::with_capacity(64),
            density,
            base_l: PinkNoise::new(),
            base_r: PinkNoise::new(),
            base_brown_l: BrownNoise::new(),
            base_brown_r: BrownNoise::new(),
            reverb: CombReverb::new(sample_rate, 73.0, 0.4), // ~73ms delay, 40% feedback
            sample_rate,
            gust_phase: 0.0,
        }
    }

    pub fn sample(&mut self, rng: &mut impl Rng) -> (f32, f32) {
        // Slow density modulation — gusts
        self.gust_phase = (self.gust_phase + 0.02 / self.sample_rate) % 1.0;
        let gust = 1.0 + (self.gust_phase * std::f32::consts::TAU).sin() * 0.4;
        let current_density = self.density * gust;

        // Spawn new droplets (Poisson-ish: probability per sample)
        let spawn_chance = current_density / self.sample_rate;
        if rng.gen::<f32>() < spawn_chance {
            self.droplets.push(Raindrop::new(rng, self.sample_rate));
        }

        // Sum all active droplets
        let mut drop_l = 0.0f32;
        let mut drop_r = 0.0f32;
        for drop in &mut self.droplets {
            let s = drop.sample(rng);
            // Random panning per drop
            let pan = 0.5; // centered-ish, the reverb adds width
            drop_l += s * (1.0 - pan * 0.3);
            drop_r += s * (0.7 + pan * 0.3);
        }
        self.droplets.retain(|d| !d.is_dead());

        // Ambient base: mostly brown (rain wash) + a touch of pink
        let base_l = self.base_brown_l.sample(rng) * 0.35 + self.base_l.sample(rng) * 0.08;
        let base_r = self.base_brown_r.sample(rng) * 0.35 + self.base_r.sample(rng) * 0.08;

        // Apply reverb to droplets only (base stays dry)
        let (wet_l, wet_r) = self.reverb.process(drop_l * 0.5, drop_r * 0.5);

        (base_l + wet_l, base_r + wet_r)
    }
}
