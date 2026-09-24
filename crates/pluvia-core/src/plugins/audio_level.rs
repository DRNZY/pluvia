use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::f64::consts::PI;

/// Target audio property to extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioLevelType {
    RMS,
    Peak,
    FFT,
    BandFreq,
}

/// Channel selection for audio measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioChannel {
    Left,
    Right,
    Avg,
    Sum,
}

/// Linux native AudioLevel plugin emulating Rainmeter's spectrum analyzers and VU meters.
#[derive(Debug, Clone)]
pub struct AudioLevelPlugin {
    level_type: AudioLevelType,
    channel: AudioChannel,
    bands_count: usize,
    band_idx: usize,
    fft_size: usize,
    freq_min: f64,
    freq_max: f64,
    attack_ms: f64,
    decay_ms: f64,
    sample_rate: f64,
    sensitivity: f64,

    raw_rms: f64,
    raw_peak: f64,
    raw_bands: Vec<f64>,

    smoothed_rms: f64,
    smoothed_peak: f64,
    smoothed_bands: Vec<f64>,
    band_center_freqs: Vec<f64>,

    current_value: MeasureValue,
}

impl AudioLevelPlugin {
    /// Create a new AudioLevel measure of the specified type.
    pub fn new(level_type: AudioLevelType) -> Self {
        let bands_count = 16;
        let mut plugin = Self {
            level_type,
            channel: AudioChannel::Avg,
            bands_count,
            band_idx: 0,
            fft_size: 1024,
            freq_min: 20.0,
            freq_max: 20000.0,
            attack_ms: 0.0,
            decay_ms: 0.0,
            sample_rate: 44100.0,
            sensitivity: 35.0,

            raw_rms: 0.0,
            raw_peak: 0.0,
            raw_bands: vec![0.0; bands_count],

            smoothed_rms: 0.0,
            smoothed_peak: 0.0,
            smoothed_bands: vec![0.0; bands_count],
            band_center_freqs: vec![0.0; bands_count],

            current_value: MeasureValue::Number(0.0),
        };
        plugin.recalculate_band_frequencies();
        plugin
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let type_str = config.get("type").unwrap_or("rms").to_ascii_lowercase();
        let level_type = match type_str.as_str() {
            "peak" => AudioLevelType::Peak,
            "fft" | "band" => AudioLevelType::FFT,
            "bandfreq" | "freq" => AudioLevelType::BandFreq,
            _ => AudioLevelType::RMS,
        };

        let mut plugin = Self::new(level_type);

        if let Some(bands) = config.get("bands").and_then(|s| s.parse::<usize>().ok()) {
            plugin = plugin.with_bands(bands);
        }

        if let Some(band) = config
            .get("bandidx")
            .or_else(|| config.get("band"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            plugin = plugin.with_band_idx(band);
        }

        if let Some(fftsize) = config
            .get("fftsize")
            .and_then(|s| s.parse::<usize>().ok())
        {
            plugin = plugin.with_fft_size(fftsize);
        }

        let fmin = config
            .get("freqmin")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(20.0);
        let fmax = config
            .get("freqmax")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(20000.0);
        plugin = plugin.with_freq_range(fmin, fmax);

        let attack = config
            .get("fftattack")
            .or_else(|| config.get("rmsattack"))
            .or_else(|| config.get("peakattack"))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(50.0);
        let decay = config
            .get("fftdecay")
            .or_else(|| config.get("rmsdecay"))
            .or_else(|| config.get("peakdecay"))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(300.0);
        plugin = plugin.with_smoothing(attack, decay);

        let chan_str = config.get("channel").unwrap_or("avg").to_ascii_lowercase();
        let chan = match chan_str.as_str() {
            "l" | "left" => AudioChannel::Left,
            "r" | "right" => AudioChannel::Right,
            "sum" => AudioChannel::Sum,
            _ => AudioChannel::Avg,
        };
        plugin = plugin.with_channel(chan);

        if let Some(sens) = config
            .get("sensitivity")
            .and_then(|s| s.parse::<f64>().ok())
        {
            plugin = plugin.with_sensitivity(sens);
        }

        plugin
    }

    /// Set number of frequency bands.
    pub fn with_bands(mut self, bands: usize) -> Self {
        let count = bands.max(1);
        self.bands_count = count;
        self.raw_bands = vec![0.0; count];
        self.smoothed_bands = vec![0.0; count];
        self.recalculate_band_frequencies();
        self
    }

    /// Set targeted band index.
    pub fn with_band_idx(mut self, idx: usize) -> Self {
        self.band_idx = idx;
        self
    }

    /// Set FFT size (rounded down to power of 2).
    pub fn with_fft_size(mut self, size: usize) -> Self {
        self.fft_size = size.next_power_of_two();
        self
    }

    /// Set frequency range for FFT bands.
    pub fn with_freq_range(mut self, min: f64, max: f64) -> Self {
        self.freq_min = min.max(1.0);
        self.freq_max = max.max(min + 1.0);
        self.recalculate_band_frequencies();
        self
    }

    /// Set attack and decay time in milliseconds for smoothing.
    pub fn with_smoothing(mut self, attack_ms: f64, decay_ms: f64) -> Self {
        self.attack_ms = attack_ms;
        self.decay_ms = decay_ms;
        self
    }

    /// Set audio channel.
    pub fn with_channel(mut self, channel: AudioChannel) -> Self {
        self.channel = channel;
        self
    }

    /// Set decibel sensitivity.
    pub fn with_sensitivity(mut self, sensitivity: f64) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    /// Retrieve sensitivity.
    pub fn sensitivity(&self) -> f64 {
        self.sensitivity
    }

    fn recalculate_band_frequencies(&mut self) {
        self.band_center_freqs = vec![0.0; self.bands_count];
        let ratio = (self.freq_max / self.freq_min).powf(1.0 / self.bands_count as f64);
        let mut lower = self.freq_min;
        for i in 0..self.bands_count {
            let upper = lower * ratio;
            self.band_center_freqs[i] = (lower * upper).sqrt();
            lower = upper;
        }
    }

    /// Feeds mono audio samples into the audio level analyzer.
    pub fn feed_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            self.raw_rms = 0.0;
            self.raw_peak = 0.0;
            for b in self.raw_bands.iter_mut() {
                *b = 0.0;
            }
            return;
        }

        // 1. RMS and Peak
        let mut sum_sq = 0.0f64;
        let mut peak = 0.0f64;
        for &s in samples {
            let val = s as f64;
            let abs_val = val.abs();
            sum_sq += val * val;
            if abs_val > peak {
                peak = abs_val;
            }
        }
        self.raw_rms = (sum_sq / samples.len() as f64).sqrt();
        self.raw_peak = peak;

        // 2. FFT
        let n = self.fft_size.min(samples.len()).next_power_of_two();
        let n = if n > samples.len() { n / 2 } else { n };
        if n >= 8 {
            let mut re: Vec<f64> = Vec::with_capacity(n);
            let mut im: Vec<f64> = vec![0.0; n];

            // Apply Hann window
            for i in 0..n {
                let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n - 1) as f64).cos());
                re.push(samples[i] as f64 * w);
            }

            compute_fft(&mut re, &mut im);

            // Compute magnitudes
            let half = n / 2;
            let mut magnitudes = Vec::with_capacity(half);
            for i in 0..half {
                let mag = (re[i] * re[i] + im[i] * im[i]).sqrt() * 2.0 / n as f64;
                magnitudes.push(mag);
            }

            // Map magnitudes to frequency bands
            let df = self.sample_rate / n as f64;
            let ratio = (self.freq_max / self.freq_min).powf(1.0 / self.bands_count as f64);
            let mut edge_low = self.freq_min;

            for b in 0..self.bands_count {
                let edge_high = edge_low * ratio;
                let k_start = ((edge_low / df).floor() as usize).clamp(0, half.saturating_sub(1));
                let k_end = ((edge_high / df).ceil() as usize).clamp(k_start + 1, half);

                let mut band_max = 0.0f64;
                for k in k_start..k_end {
                    if magnitudes[k] > band_max {
                        band_max = magnitudes[k];
                    }
                }
                let raw_val = if self.sensitivity > 0.0 {
                    if band_max <= 1e-6 {
                        0.0
                    } else {
                        let db = 20.0 * band_max.log10();
                        ((db + self.sensitivity) / self.sensitivity).clamp(0.0, 1.0)
                    }
                } else {
                    band_max.clamp(0.0, 1.0)
                };
                self.raw_bands[b] = raw_val;
                edge_low = edge_high;
            }
        }

        if self.attack_ms <= 0.0 && self.decay_ms <= 0.0 {
            self.smoothed_rms = self.raw_rms;
            self.smoothed_peak = self.raw_peak;
            self.smoothed_bands.copy_from_slice(&self.raw_bands);
        }
    }

    /// Feed stereo audio samples into the audio level analyzer.
    pub fn feed_stereo(&mut self, left: &[f32], right: &[f32]) {
        let len = left.len().min(right.len());
        let mut mixed = Vec::with_capacity(len);
        for i in 0..len {
            let sample = match self.channel {
                AudioChannel::Left => left[i],
                AudioChannel::Right => right[i],
                AudioChannel::Avg => (left[i] + right[i]) * 0.5,
                AudioChannel::Sum => left[i] + right[i],
            };
            mixed.push(sample);
        }
        self.feed_samples(&mixed);
    }

    /// Retrieve computed frequency band values.
    pub fn get_bands(&self) -> &[f64] {
        &self.smoothed_bands
    }

    /// Retrieve specific frequency band value.
    pub fn get_band_value(&self, idx: usize) -> f64 {
        self.smoothed_bands.get(idx).copied().unwrap_or(0.0)
    }

    /// Retrieve smoothed RMS amplitude.
    pub fn get_rms(&self) -> f64 {
        self.smoothed_rms
    }

    /// Retrieve smoothed Peak amplitude.
    pub fn get_peak(&self) -> f64 {
        self.smoothed_peak
    }

    fn apply_smoothing(current: &mut f64, target: f64, attack_ms: f64, decay_ms: f64) {
        if target > *current {
            if attack_ms <= 0.0 {
                *current = target;
            } else {
                let factor = (50.0 / (50.0 + attack_ms)).clamp(0.01, 1.0);
                *current += factor * (target - *current);
            }
        } else if decay_ms <= 0.0 {
            *current = target;
        } else {
            let factor = (50.0 / (50.0 + decay_ms)).clamp(0.01, 1.0);
            *current -= factor * (*current - target);
        }
    }
}

/// Radix-2 in-place Cooley-Tukey FFT.
fn compute_fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    if n <= 1 {
        return;
    }

    // Bit-reversal permutation
    let mut j = 0;
    for i in 0..n - 1 {
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
        let mut k = n / 2;
        while k <= j {
            j -= k;
            k /= 2;
        }
        j += k;
    }

    // Butterfly computations
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = -2.0 * PI / len as f64;
        let w_step_re = angle.cos();
        let w_step_im = angle.sin();

        let mut i = 0;
        while i < n {
            let mut w_re = 1.0;
            let mut w_im = 0.0;
            for k in 0..half {
                let u_re = re[i + k];
                let u_im = im[i + k];
                let v_re = re[i + k + half] * w_re - im[i + k + half] * w_im;
                let v_im = re[i + k + half] * w_im + im[i + k + half] * w_re;

                re[i + k] = u_re + v_re;
                im[i + k] = u_im + v_im;
                re[i + k + half] = u_re - v_re;
                im[i + k + half] = u_im - v_im;

                let next_w_re = w_re * w_step_re - w_im * w_step_im;
                let next_w_im = w_re * w_step_im + w_im * w_step_re;
                w_re = next_w_re;
                w_im = next_w_im;
            }
            i += len;
        }
        len *= 2;
    }
}

impl Measure for AudioLevelPlugin {
    fn update(&mut self) -> MeasureValue {
        // Apply smoothing to RMS, Peak, and Bands
        Self::apply_smoothing(
            &mut self.smoothed_rms,
            self.raw_rms,
            self.attack_ms,
            self.decay_ms,
        );
        Self::apply_smoothing(
            &mut self.smoothed_peak,
            self.raw_peak,
            self.attack_ms,
            self.decay_ms,
        );

        for i in 0..self.bands_count {
            let raw = self.raw_bands.get(i).copied().unwrap_or(0.0);
            if let Some(smoothed) = self.smoothed_bands.get_mut(i) {
                Self::apply_smoothing(smoothed, raw, self.attack_ms, self.decay_ms);
            }
        }

        let num_val = match self.level_type {
            AudioLevelType::RMS => self.smoothed_rms,
            AudioLevelType::Peak => self.smoothed_peak,
            AudioLevelType::FFT => self.get_band_value(self.band_idx),
            AudioLevelType::BandFreq => {
                self.band_center_freqs.get(self.band_idx).copied().unwrap_or(0.0)
            }
        };

        self.current_value = MeasureValue::Number(num_val);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn feed_audio(&mut self, samples: &[f32]) {
        self.feed_samples(samples);
    }
}
